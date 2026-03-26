//! BEACON telemetry dashboard.
//!
//! Polls the Yamcs archive for recent BEACON packets and renders a professional
//! live telemetry dashboard using egui + egui_plot: KPI cards with sparklines,
//! filled area charts, radial gauges, boolean status indicators, and rich tables.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use egui_plot::{Bar, BarChart, Legend, Line, PlotPoints, Plot};
use serde_json::Value;

use crate::yamcs_client::{YamcsClient, YamcsConfig};

// ─── Data model ─────────────────────────────────────────────────────────────

/// A single parameter value extracted from a BEACON packet.
#[derive(Debug, Clone)]
pub enum BeaconFieldValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    StringVal(String),
}

impl BeaconFieldValue {
    /// Try to interpret the value as an f64 (for plotting).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            BeaconFieldValue::Float(v) => Some(*v),
            BeaconFieldValue::Int(v) => Some(*v as f64),
            BeaconFieldValue::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
            BeaconFieldValue::StringVal(_) => None,
        }
    }

    pub fn display_str(&self) -> String {
        match self {
            BeaconFieldValue::Float(v) => format!("{:.4}", v),
            BeaconFieldValue::Int(v) => v.to_string(),
            BeaconFieldValue::Bool(v) => v.to_string(),
            BeaconFieldValue::StringVal(s) => s.clone(),
        }
    }
}

/// One snapshot in time: the values of all parameters from one BEACON packet.
#[derive(Debug, Clone)]
pub struct BeaconSnapshot {
    /// ISO-8601 generation time string from Yamcs.
    pub generation_time: String,
    /// Parsed unix timestamp (seconds since epoch) for plotting on the x-axis.
    pub timestamp_s: f64,
    /// All extracted parameter values keyed by parameter name.
    pub fields: HashMap<String, BeaconFieldValue>,
}

/// Ring-buffer of recent snapshots.
#[derive(Debug, Default)]
pub struct BeaconHistory {
    pub snapshots: Vec<BeaconSnapshot>,
    /// Maximum number of snapshots to retain.
    pub capacity: usize,
}

impl BeaconHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, snap: BeaconSnapshot) {
        if self
            .snapshots
            .iter()
            .any(|s| s.generation_time == snap.generation_time)
        {
            return;
        }
        if self.snapshots.len() >= self.capacity {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snap);
        self.snapshots
            .sort_by(|a, b| a.timestamp_s.partial_cmp(&b.timestamp_s).unwrap());
    }

    /// Collect all unique field names seen across all snapshots.
    pub fn field_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .snapshots
            .iter()
            .flat_map(|s| s.fields.keys().cloned())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        names.sort();
        names
    }
}

// ─── Yamcs polling ──────────────────────────────────────────────────────────

/// Message sent from the background poller thread to the UI thread.
pub enum BeaconPollMessage {
    Snapshots(Vec<BeaconSnapshot>),
    Error(String),
}

fn parse_extract_response(json: &Value) -> Option<BeaconSnapshot> {
    let gen_time = json
        .get("generationTime")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let timestamp_s = iso8601_to_unix_s(&gen_time).unwrap_or(0.0);

    let params = json.get("parameter").and_then(|v| v.as_array())?;

    let mut fields = HashMap::new();
    for param in params {
        let name = param
            .get("id")
            .and_then(|id| id.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.rsplit('/').next().unwrap_or(s).to_string());
        let Some(name) = name else { continue };

        let eng = match param.get("engValue") {
            Some(v) => v,
            None => continue,
        };

        let type_str = eng.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let value = match type_str {
            "FLOAT" => eng
                .get("floatValue")
                .and_then(|v| v.as_f64())
                .map(BeaconFieldValue::Float),
            "DOUBLE" => eng
                .get("doubleValue")
                .and_then(|v| v.as_f64())
                .map(BeaconFieldValue::Float),
            "UINT32" | "UINT64" => eng
                .get("uint64Value")
                .or_else(|| eng.get("uint32Value"))
                .and_then(|v| v.as_u64())
                .map(|v| BeaconFieldValue::Int(v as i64)),
            "SINT32" | "SINT64" => eng
                .get("sint64Value")
                .or_else(|| eng.get("sint32Value"))
                .and_then(|v| v.as_i64())
                .map(BeaconFieldValue::Int),
            "BOOLEAN" => eng
                .get("booleanValue")
                .and_then(|v| v.as_bool())
                .map(BeaconFieldValue::Bool),
            "STRING" | "ENUMERATED" => eng
                .get("stringValue")
                .and_then(|v| v.as_str())
                .map(|s| BeaconFieldValue::StringVal(s.to_string())),
            _ => {
                if let Some(f) = eng.get("floatValue").and_then(|v| v.as_f64()) {
                    Some(BeaconFieldValue::Float(f))
                } else if let Some(i) = eng.get("sint64Value").and_then(|v| v.as_i64()) {
                    Some(BeaconFieldValue::Int(i))
                } else if let Some(s) = eng.get("stringValue").and_then(|v| v.as_str()) {
                    Some(BeaconFieldValue::StringVal(s.to_string()))
                } else {
                    None
                }
            }
        };

        if let Some(v) = value {
            fields.insert(name, v);
        }
    }

    if fields.is_empty() && gen_time.is_empty() {
        return None;
    }

    Some(BeaconSnapshot {
        generation_time: gen_time,
        timestamp_s,
        fields,
    })
}

fn iso8601_to_unix_s(s: &str) -> Option<f64> {
    let s = s.trim_end_matches('Z').trim_end_matches("+00:00");
    let (date_part, time_part) = if let Some(idx) = s.find('T') {
        (&s[..idx], &s[idx + 1..])
    } else {
        return None;
    };
    let date_parts: Vec<&str> = date_part.split('-').collect();
    if date_parts.len() < 3 {
        return None;
    }
    let year: i64 = date_parts[0].parse().ok()?;
    let month: i64 = date_parts[1].parse().ok()?;
    let day: i64 = date_parts[2].parse().ok()?;

    let time_parts: Vec<&str> = time_part.splitn(3, ':').collect();
    if time_parts.len() < 3 {
        return None;
    }
    let hour: i64 = time_parts[0].parse().ok()?;
    let minute: i64 = time_parts[1].parse().ok()?;
    let sec_f: f64 = time_parts[2].parse().ok()?;
    let sec = sec_f.trunc() as i64;
    let frac = sec_f.fract();

    let days = days_since_epoch(year, month, day)?;
    let total_s = days * 86400 + hour * 3600 + minute * 60 + sec;
    Some(total_s as f64 + frac)
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    let m = if month < 3 { month + 12 } else { month };
    let y = if month < 3 { year - 1 } else { year };
    let jdn = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m - 457) / 5 + day;
    Some(jdn - 2440588)
}

pub fn spawn_beacon_poller(
    config: YamcsConfig,
    poll_interval_s: u64,
    history_limit: usize,
) -> mpsc::Receiver<BeaconPollMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime for beacon poller");
        rt.block_on(async move {
            let client = match YamcsClient::new(config) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(BeaconPollMessage::Error(format!(
                        "Failed to create YamcsClient: {}",
                        e
                    )));
                    return;
                }
            };

            loop {
                match client.recent_packets_by_name("BEACON", history_limit).await {
                    Ok(json) => {
                        let snaps = parse_beacon_poll_response(&client, &json).await;
                        if !snaps.is_empty() {
                            let _ = tx.send(BeaconPollMessage::Snapshots(snaps));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(BeaconPollMessage::Error(format!(
                            "BEACON poll error: {}",
                            e
                        )));
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_s)).await;
            }
        });
    });
    rx
}

async fn parse_beacon_poll_response(client: &YamcsClient, json: &Value) -> Vec<BeaconSnapshot> {
    let mut out = Vec::new();

    if let Some(snap) = parse_extract_response(json) {
        out.push(snap);
    }

    if let Some(packets) = json.get("packet").and_then(|v| v.as_array()) {
        for pkt in packets {
            if let Some(snap) = parse_extract_response(pkt) {
                out.push(snap);
            }
        }
    }

    let _ = client;
    out
}

// ─── Professional colour palette ────────────────────────────────────────────

/// Modern, vibrant colour palette inspired by professional dashboards.
const PALETTE: &[egui::Color32] = &[
    egui::Color32::from_rgb(0x00, 0xB4, 0xD8), // Vivid sky blue
    egui::Color32::from_rgb(0xFF, 0x6B, 0x6B), // Coral red
    egui::Color32::from_rgb(0x51, 0xCF, 0x66), // Emerald green
    egui::Color32::from_rgb(0xFF, 0xA9, 0x4D), // Warm amber
    egui::Color32::from_rgb(0xCC, 0x5D, 0xE8), // Orchid purple
    egui::Color32::from_rgb(0x20, 0xC9, 0x97), // Teal
    egui::Color32::from_rgb(0xF7, 0x83, 0xAC), // Rose pink
    egui::Color32::from_rgb(0x74, 0xB9, 0xFF), // Periwinkle blue
    egui::Color32::from_rgb(0xFD, 0xCB, 0x6E), // Soft gold
    egui::Color32::from_rgb(0xA2, 0x9B, 0xFE), // Lavender
    egui::Color32::from_rgb(0x6C, 0x5C, 0xE7), // Deep indigo
    egui::Color32::from_rgb(0x00, 0xCE, 0xC9), // Cyan
];

fn field_color(idx: usize) -> egui::Color32 {
    PALETTE[idx % PALETTE.len()]
}

/// Create a semi-transparent version of a colour for fills.
fn color_with_alpha(c: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

// ─── Theme constants ────────────────────────────────────────────────────────

const CARD_BG: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x1E, 0x2E);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x8B, 0x8B, 0xA0);
const TEXT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(0xE8, 0xE8, 0xF0);
const STATUS_GREEN: egui::Color32 = egui::Color32::from_rgb(0x51, 0xCF, 0x66);
const STATUS_RED: egui::Color32 = egui::Color32::from_rgb(0xFF, 0x6B, 0x6B);
const STATUS_YELLOW: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xE0, 0x66);
const GAUGE_BG: egui::Color32 = egui::Color32::from_rgb(0x2A, 0x2A, 0x40);
const SEPARATOR_COLOR: egui::Color32 = egui::Color32::from_rgb(0x3A, 0x3A, 0x50);

// ─── View mode ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DashboardView {
    Overview,
    Charts,
    Gauges,
    Table,
}

// ─── Dashboard widget ────────────────────────────────────────────────────────

/// All state needed by the BEACON dashboard window.
pub struct BeaconDashboard {
    pub history: BeaconHistory,
    pub rx: Option<mpsc::Receiver<BeaconPollMessage>>,
    pub last_error: Option<String>,
    pub running: bool,
    pub selected_fields: HashMap<String, bool>,
    /// Current view tab.
    pub view: DashboardView,
    /// Which numeric field is selected for the radial gauge (index).
    pub gauge_field_idx: usize,
}

impl Default for BeaconDashboard {
    fn default() -> Self {
        Self {
            history: BeaconHistory::new(200),
            rx: None,
            last_error: None,
            running: false,
            selected_fields: HashMap::new(),
            view: DashboardView::Overview,
            gauge_field_idx: 0,
        }
    }
}

impl BeaconDashboard {
    pub fn poll_rx(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    BeaconPollMessage::Snapshots(snaps) => {
                        for s in snaps {
                            self.history.push(s);
                        }
                        self.last_error = None;
                    }
                    BeaconPollMessage::Error(e) => {
                        self.last_error = Some(e);
                    }
                }
            }
        }
    }

    pub fn start_poller(&mut self, config: YamcsConfig) {
        if self.running {
            return;
        }
        let rx = spawn_beacon_poller(config, 5, 50);
        self.rx = Some(rx);
        self.running = true;
    }

    /// Render the complete professional dashboard.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        self.poll_rx();

        // ── Professional status bar ─────────────────────────────────────
        self.render_status_bar(ui);

        ui.add_space(4.0);

        let field_names = self.history.field_names();
        if field_names.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Awaiting BEACON telemetry data...")
                        .size(16.0)
                        .color(TEXT_DIM),
                );
                ui.add_space(8.0);
                ui.spinner();
            });
            return;
        }

        // Ensure every discovered field has a selection entry.
        for name in &field_names {
            self.selected_fields.entry(name.clone()).or_insert(true);
        }

        // ── Navigation tabs ─────────────────────────────────────────────
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (label, mode) in [
                ("Overview", DashboardView::Overview),
                ("Charts", DashboardView::Charts),
                ("Gauges", DashboardView::Gauges),
                ("Table", DashboardView::Table),
            ] {
                let is_active = self.view == mode;
                let text = if is_active {
                    egui::RichText::new(label).strong().color(egui::Color32::WHITE)
                } else {
                    egui::RichText::new(label).color(TEXT_DIM)
                };
                if ui.selectable_label(is_active, text).clicked() {
                    self.view = mode;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Field visibility toggle
                ui.menu_button(
                    egui::RichText::new("Fields").color(TEXT_DIM),
                    |ui| {
                        for (idx, name) in field_names.iter().enumerate() {
                            let selected = self.selected_fields.entry(name.clone()).or_insert(true);
                            let color = field_color(idx);
                            let label = egui::RichText::new(name).color(color).strong();
                            ui.checkbox(selected, label);
                        }
                    },
                );
            });
        });

        self.render_separator(ui);

        // ── Classify fields ─────────────────────────────────────────────
        let mut numeric_fields: Vec<(usize, String)> = Vec::new();
        let mut bool_fields: Vec<(usize, String)> = Vec::new();
        let mut string_fields: Vec<(usize, String)> = Vec::new();

        for (idx, name) in field_names.iter().enumerate() {
            if !*self.selected_fields.get(name.as_str()).unwrap_or(&true) {
                continue;
            }
            let kind = self
                .history
                .snapshots
                .iter()
                .rev()
                .find_map(|s| s.fields.get(name.as_str()));
            match kind {
                Some(BeaconFieldValue::Float(_)) | Some(BeaconFieldValue::Int(_)) => {
                    numeric_fields.push((idx, name.clone()))
                }
                Some(BeaconFieldValue::Bool(_)) => bool_fields.push((idx, name.clone())),
                Some(BeaconFieldValue::StringVal(_)) => string_fields.push((idx, name.clone())),
                None => {}
            }
        }

        match self.view {
            DashboardView::Overview => {
                self.render_overview(ui, &numeric_fields, &bool_fields, &string_fields);
            }
            DashboardView::Charts => {
                self.render_charts_view(ui, &numeric_fields, &bool_fields);
            }
            DashboardView::Gauges => {
                self.render_gauges_view(ui, &numeric_fields);
            }
            DashboardView::Table => {
                self.render_table_view(ui, &numeric_fields, &bool_fields, &string_fields);
            }
        }
    }

    // ─── Status bar ─────────────────────────────────────────────────────

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Connection status dot
                    if self.running {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(10.0, 10.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(rect.center(), 5.0, STATUS_GREEN);
                        ui.label(
                            egui::RichText::new("LIVE")
                                .size(11.0)
                                .strong()
                                .color(STATUS_GREEN),
                        );
                    } else {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(10.0, 10.0),
                            egui::Sense::hover(),
                        );
                        ui.painter()
                            .circle_filled(rect.center(), 5.0, TEXT_DIM);
                        ui.label(
                            egui::RichText::new("IDLE")
                                .size(11.0)
                                .color(TEXT_DIM),
                        );
                    }

                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("|").size(11.0).color(SEPARATOR_COLOR),
                    );
                    ui.add_space(12.0);

                    // Snapshot count
                    let count = self.history.snapshots.len();
                    ui.label(
                        egui::RichText::new(format!("{} samples", count))
                            .size(11.0)
                            .color(TEXT_DIM),
                    );

                    // Time range
                    if count >= 2 {
                        let first = self.history.snapshots.first().unwrap();
                        let last = self.history.snapshots.last().unwrap();
                        let span = last.timestamp_s - first.timestamp_s;
                        let span_str = if span < 60.0 {
                            format!("{:.0}s", span)
                        } else if span < 3600.0 {
                            format!("{:.1}m", span / 60.0)
                        } else {
                            format!("{:.1}h", span / 3600.0)
                        };
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!("({})", span_str))
                                .size(11.0)
                                .color(TEXT_DIM),
                        );
                    }

                    // Error display on the right
                    if let Some(err) = &self.last_error {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(err)
                                        .size(11.0)
                                        .color(STATUS_RED),
                                );
                            },
                        );
                    }
                });
            });
    }

    fn render_separator(&self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        let rect = ui.available_rect_before_wrap();
        let y = rect.top();
        ui.painter().line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, SEPARATOR_COLOR),
        );
        ui.add_space(4.0);
    }

    // ─── Overview ───────────────────────────────────────────────────────

    fn render_overview(
        &mut self,
        ui: &mut egui::Ui,
        numeric_fields: &[(usize, String)],
        bool_fields: &[(usize, String)],
        string_fields: &[(usize, String)],
    ) {
        // KPI cards row
        if !numeric_fields.is_empty() {
            self.render_kpi_cards(ui, numeric_fields);
            ui.add_space(8.0);
        }

        // Main chart (all numeric overlaid with area fill)
        if !numeric_fields.is_empty() {
            self.render_main_area_chart(ui, numeric_fields);
            ui.add_space(8.0);
        }

        // Boolean indicators and string table side by side
        let has_bools = !bool_fields.is_empty();
        let has_strings = !string_fields.is_empty();

        if has_bools || has_strings {
            ui.horizontal(|ui| {
                if has_bools {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("STATUS FLAGS")
                                .size(11.0)
                                .strong()
                                .color(TEXT_DIM),
                        );
                        ui.add_space(4.0);
                        self.render_bool_indicators(ui, bool_fields);
                    });
                    if has_strings {
                        ui.add_space(20.0);
                    }
                }
                if has_strings {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("ENUMERATIONS")
                                .size(11.0)
                                .strong()
                                .color(TEXT_DIM),
                        );
                        ui.add_space(4.0);
                        self.render_string_table(ui, string_fields);
                    });
                }
            });
        }
    }

    // ─── KPI cards with sparklines ──────────────────────────────────────

    fn render_kpi_cards(&self, ui: &mut egui::Ui, numeric_fields: &[(usize, String)]) {
        // Show up to 6 KPI cards in a horizontal row
        let max_cards = 6.min(numeric_fields.len());
        let available_width = ui.available_width();
        let card_width = ((available_width - (max_cards as f32 - 1.0) * 6.0) / max_cards as f32)
            .max(100.0);

        ui.horizontal_wrapped(|ui| {
            for (idx, name) in numeric_fields.iter().take(max_cards) {
                let color = field_color(*idx);
                self.render_single_kpi_card(ui, name, color, card_width);
            }
        });
    }

    fn render_single_kpi_card(
        &self,
        ui: &mut egui::Ui,
        field_name: &str,
        color: egui::Color32,
        width: f32,
    ) {
        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(width - 24.0);

                // Field label
                ui.label(
                    egui::RichText::new(field_name)
                        .size(10.0)
                        .strong()
                        .color(TEXT_DIM),
                );

                // Collect values for this field
                let values: Vec<f64> = self
                    .history
                    .snapshots
                    .iter()
                    .filter_map(|s| s.fields.get(field_name).and_then(|v| v.as_f64()))
                    .collect();

                if values.is_empty() {
                    ui.label(egui::RichText::new("--").size(20.0).color(TEXT_BRIGHT));
                    return;
                }

                let current = *values.last().unwrap();
                let prev = if values.len() >= 2 {
                    values[values.len() - 2]
                } else {
                    current
                };

                // Current value (big)
                ui.label(
                    egui::RichText::new(format_value(current))
                        .size(20.0)
                        .strong()
                        .color(TEXT_BRIGHT),
                );

                // Delta indicator
                let delta = current - prev;
                if delta.abs() > f64::EPSILON {
                    let (arrow, delta_color) = if delta > 0.0 {
                        ("^", STATUS_GREEN)
                    } else {
                        ("v", STATUS_RED)
                    };
                    ui.label(
                        egui::RichText::new(format!("{} {}", arrow, format_value(delta.abs())))
                            .size(10.0)
                            .color(delta_color),
                    );
                }

                // Mini sparkline
                if values.len() >= 2 {
                    let sparkline_height = 24.0;
                    let sparkline_width = width - 24.0;
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(sparkline_width, sparkline_height),
                        egui::Sense::hover(),
                    );

                    let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
                    let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let range = (max_val - min_val).max(f64::EPSILON);

                    let painter = ui.painter_at(rect);

                    // Draw filled sparkline area
                    let n = values.len();
                    let mut polygon_points = Vec::with_capacity(n + 2);

                    for (i, &v) in values.iter().enumerate() {
                        let x = rect.left() + (i as f32 / (n - 1).max(1) as f32) * rect.width();
                        let y = rect.bottom()
                            - ((v - min_val) / range) as f32 * rect.height();
                        polygon_points.push(egui::pos2(x, y));
                    }

                    // Close the polygon at the bottom
                    let mut fill_points = polygon_points.clone();
                    fill_points.push(egui::pos2(rect.right(), rect.bottom()));
                    fill_points.push(egui::pos2(rect.left(), rect.bottom()));

                    painter.add(egui::Shape::convex_polygon(
                        fill_points,
                        color_with_alpha(color, 40),
                        egui::Stroke::NONE,
                    ));

                    // Draw the line on top
                    if polygon_points.len() >= 2 {
                        let line_shapes: Vec<_> = polygon_points
                            .windows(2)
                            .map(|w| egui::Shape::line_segment(
                                [w[0], w[1]],
                                egui::Stroke::new(1.5, color),
                            ))
                            .collect();
                        for s in line_shapes {
                            painter.add(s);
                        }
                    }

                    // Dot on the last value
                    if let Some(last_pt) = polygon_points.last() {
                        painter.circle_filled(*last_pt, 3.0, color);
                    }
                }

                // Min/Max range
                if values.len() >= 2 {
                    let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
                    let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} - {}",
                            format_value(min_val),
                            format_value(max_val)
                        ))
                        .size(9.0)
                        .color(TEXT_DIM),
                    );
                }
            });
    }

    // ─── Main area chart ────────────────────────────────────────────────

    fn render_main_area_chart(
        &self,
        ui: &mut egui::Ui,
        numeric_fields: &[(usize, String)],
    ) {
        ui.label(
            egui::RichText::new("TELEMETRY TIMELINE")
                .size(11.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(2.0);

        let snapshots = &self.history.snapshots;
        let t0 = snapshots.first().map(|s| s.timestamp_s).unwrap_or(0.0);

        Plot::new("beacon_overview_chart")
            .height(220.0)
            .legend(Legend::default())
            .show_background(true)
            .show_grid(true)
            .allow_drag(true)
            .allow_zoom(true)
            .allow_scroll(true)
            .x_axis_label("Time (s)")
            .y_axis_label("Value")
            .show(ui, |plot_ui| {
                for (idx, name) in numeric_fields {
                    let color = field_color(*idx);
                    let points: Vec<[f64; 2]> = snapshots
                        .iter()
                        .filter_map(|s| {
                            s.fields
                                .get(name.as_str())
                                .and_then(|v| v.as_f64())
                                .map(|y| [s.timestamp_s - t0, y])
                        })
                        .collect();

                    if points.is_empty() {
                        continue;
                    }

                    let pp: PlotPoints = points.into();

                    // Filled area line
                    plot_ui.line(
                        Line::new(name.as_str(), pp)
                            .color(color)
                            .width(2.5)
                            .fill(0.0)
                            .fill_alpha(0.15),
                    );
                }
            });
    }

    // ─── Boolean indicators ─────────────────────────────────────────────

    fn render_bool_indicators(&self, ui: &mut egui::Ui, bool_fields: &[(usize, String)]) {
        for (idx, name) in bool_fields {
            let color = field_color(*idx);
            let current = self
                .history
                .snapshots
                .iter()
                .rev()
                .find_map(|s| s.fields.get(name.as_str()))
                .and_then(|v| v.as_f64())
                .map(|v| v > 0.5)
                .unwrap_or(false);

            egui::Frame::new()
                .fill(CARD_BG)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status LED
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(12.0, 12.0),
                            egui::Sense::hover(),
                        );
                        let led_color = if current { STATUS_GREEN } else { STATUS_RED };
                        ui.painter().circle_filled(rect.center(), 6.0, led_color);
                        // Glow effect
                        ui.painter().circle_filled(
                            rect.center(),
                            8.0,
                            color_with_alpha(led_color, 40),
                        );

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(name)
                                .size(12.0)
                                .color(color),
                        );
                        ui.label(
                            egui::RichText::new(if current { "ON" } else { "OFF" })
                                .size(11.0)
                                .strong()
                                .color(if current { STATUS_GREEN } else { STATUS_RED }),
                        );
                    });
                });
            ui.add_space(2.0);
        }
    }

    // ─── String/enum table ──────────────────────────────────────────────

    fn render_string_table(&self, ui: &mut egui::Ui, string_fields: &[(usize, String)]) {
        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                egui::Grid::new("beacon_strings_pro")
                    .striped(true)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        // Header
                        ui.label(egui::RichText::new("Parameter").size(10.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Current").size(10.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Previous").size(10.0).strong().color(TEXT_DIM));
                        ui.end_row();

                        for (idx, name) in string_fields {
                            let color = field_color(*idx);
                            let vals: Vec<&str> = self
                                .history
                                .snapshots
                                .iter()
                                .rev()
                                .filter_map(|s| {
                                    if let Some(BeaconFieldValue::StringVal(v)) =
                                        s.fields.get(name.as_str())
                                    {
                                        Some(v.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .take(2)
                                .collect();

                            let current = vals.first().copied().unwrap_or("--");
                            let previous = vals.get(1).copied().unwrap_or("--");
                            let changed = current != previous && previous != "--";

                            ui.label(egui::RichText::new(name.as_str()).color(color).strong());
                            ui.label(
                                egui::RichText::new(current)
                                    .color(TEXT_BRIGHT)
                                    .strong(),
                            );

                            let prev_color = if changed { STATUS_YELLOW } else { TEXT_DIM };
                            ui.label(egui::RichText::new(previous).color(prev_color));
                            ui.end_row();
                        }
                    });
            });
    }

    // ─── Charts view (individual per-field charts) ──────────────────────

    fn render_charts_view(
        &self,
        ui: &mut egui::Ui,
        numeric_fields: &[(usize, String)],
        bool_fields: &[(usize, String)],
    ) {
        let snapshots = &self.history.snapshots;
        let t0 = snapshots.first().map(|s| s.timestamp_s).unwrap_or(0.0);

        if !numeric_fields.is_empty() {
            ui.label(
                egui::RichText::new("NUMERIC CHANNELS")
                    .size(11.0)
                    .strong()
                    .color(TEXT_DIM),
            );
            ui.add_space(4.0);

            // Individual chart per field (2 columns)
            let chart_height = 160.0;
            let cols = if numeric_fields.len() > 1 { 2 } else { 1 };
            let chart_w = (ui.available_width() - 8.0) / cols as f32;

            let mut field_iter = numeric_fields.iter().peekable();
            while field_iter.peek().is_some() {
                ui.horizontal(|ui| {
                    for _ in 0..cols {
                        if let Some((idx, name)) = field_iter.next() {
                            let color = field_color(*idx);
                            ui.vertical(|ui| {
                                ui.set_width(chart_w);

                                // Field header with latest value
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(name.as_str())
                                            .size(12.0)
                                            .strong()
                                            .color(color),
                                    );
                                    if let Some(last) = snapshots
                                        .iter()
                                        .rev()
                                        .find_map(|s| s.fields.get(name.as_str()))
                                    {
                                        ui.label(
                                            egui::RichText::new(last.display_str())
                                                .size(12.0)
                                                .color(TEXT_BRIGHT),
                                        );
                                    }
                                });

                                let points: Vec<[f64; 2]> = snapshots
                                    .iter()
                                    .filter_map(|s| {
                                        s.fields
                                            .get(name.as_str())
                                            .and_then(|v| v.as_f64())
                                            .map(|y| [s.timestamp_s - t0, y])
                                    })
                                    .collect();

                                if points.is_empty() {
                                    ui.label("No data");
                                    return;
                                }

                                let min_y = points
                                    .iter()
                                    .map(|p| p[1])
                                    .fold(f64::INFINITY, f64::min);

                                let pp: PlotPoints = points.into();

                                Plot::new(format!("chart_{}", name))
                                    .height(chart_height)
                                    .show_background(true)
                                    .show_grid(true)
                                    .allow_drag(true)
                                    .allow_zoom(true)
                                    .show_axes([true, true])
                                    .show(ui, |plot_ui| {
                                        // Filled area
                                        plot_ui.line(
                                            Line::new(name.as_str(), pp)
                                                .color(color)
                                                .width(2.0)
                                                .fill(min_y as f32)
                                                .fill_alpha(0.2),
                                        );
                                    });
                            });
                        }
                    }
                });
                ui.add_space(8.0);
            }
        }

        // Boolean step chart with bar chart visualization
        if !bool_fields.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("BOOLEAN FLAGS TIMELINE")
                    .size(11.0)
                    .strong()
                    .color(TEXT_DIM),
            );
            ui.add_space(4.0);

            Plot::new("beacon_bool_pro")
                .height(100.0 + bool_fields.len() as f32 * 20.0)
                .legend(Legend::default())
                .show_grid([true, false])
                .allow_drag(true)
                .allow_zoom(true)
                .show(ui, |plot_ui| {
                    for (field_idx, (idx, name)) in bool_fields.iter().enumerate() {
                        let color = field_color(*idx);
                        let offset = field_idx as f64 * 1.5;

                        // Use bars to show ON/OFF states with colored blocks
                        let bars: Vec<Bar> = snapshots
                            .iter()
                            .filter_map(|s| {
                                s.fields
                                    .get(name.as_str())
                                    .and_then(|v| v.as_f64())
                                    .map(|y| {
                                        let t = s.timestamp_s - t0;
                                        let val = y * 1.0 + offset;
                                        Bar::new(t, val)
                                            .fill(if y > 0.5 {
                                                color
                                            } else {
                                                color_with_alpha(color, 30)
                                            })
                                            .stroke(egui::Stroke::new(0.5, color))
                                            .width(2.0)
                                    })
                            })
                            .collect();

                        if !bars.is_empty() {
                            plot_ui.bar_chart(
                                BarChart::new(name.as_str(), bars).color(color),
                            );
                        }
                    }
                });
        }
    }

    // ─── Gauges view ────────────────────────────────────────────────────

    fn render_gauges_view(
        &self,
        ui: &mut egui::Ui,
        numeric_fields: &[(usize, String)],
    ) {
        if numeric_fields.is_empty() {
            ui.label(
                egui::RichText::new("No numeric fields to display")
                    .color(TEXT_DIM),
            );
            return;
        }

        ui.label(
            egui::RichText::new("RADIAL GAUGES")
                .size(11.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(8.0);

        // Draw gauges in a wrapping layout
        let gauge_size = 140.0;

        ui.horizontal_wrapped(|ui| {
            for (idx, name) in numeric_fields {
                let color = field_color(*idx);

                // Get all values for this field to compute range
                let values: Vec<f64> = self
                    .history
                    .snapshots
                    .iter()
                    .filter_map(|s| s.fields.get(name.as_str()).and_then(|v| v.as_f64()))
                    .collect();

                if values.is_empty() {
                    continue;
                }

                let current = *values.last().unwrap();
                let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
                let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let range = (max_val - min_val).max(f64::EPSILON);
                let fraction = ((current - min_val) / range).clamp(0.0, 1.0);

                self.render_radial_gauge(ui, name, color, current, fraction as f32, gauge_size);
            }
        });

        ui.add_space(12.0);

        // Distribution bar chart
        ui.label(
            egui::RichText::new("VALUE DISTRIBUTION")
                .size(11.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(4.0);

        if !numeric_fields.is_empty() {
            let bars: Vec<Bar> = numeric_fields
                .iter()
                .enumerate()
                .filter_map(|(bar_idx, (idx, name))| {
                    let current = self
                        .history
                        .snapshots
                        .iter()
                        .rev()
                        .find_map(|s| s.fields.get(name.as_str()).and_then(|v| v.as_f64()))?;
                    let color = field_color(*idx);
                    Some(
                        Bar::new(bar_idx as f64, current)
                            .name(name.as_str())
                            .fill(color_with_alpha(color, 180))
                            .stroke(egui::Stroke::new(1.5, color))
                            .width(0.7),
                    )
                })
                .collect();

            if !bars.is_empty() {
                Plot::new("beacon_distribution")
                    .height(180.0)
                    .show_grid([false, true])
                    .allow_drag(false)
                    .allow_zoom(false)
                    .show(ui, |plot_ui| {
                        plot_ui.bar_chart(
                            BarChart::new("Current Values", bars),
                        );
                    });
            }
        }
    }

    fn render_radial_gauge(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        color: egui::Color32,
        value: f64,
        fraction: f32,
        size: f32,
    ) {
        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(size);
                ui.set_height(size + 30.0);

                // Label at top
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(name)
                            .size(10.0)
                            .strong()
                            .color(TEXT_DIM),
                    );
                });

                // Draw the gauge arc
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(size, size - 10.0),
                    egui::Sense::hover(),
                );

                let painter = ui.painter_at(rect);
                let center = rect.center();
                let radius = (size * 0.4).min(rect.height() * 0.45);

                // Background arc (270 degrees, from 135 to 405)
                let arc_start = std::f32::consts::PI * 0.75; // 135 degrees
                let arc_span = std::f32::consts::PI * 1.5; // 270 degrees

                let bg_segments = 60;
                for i in 0..bg_segments {
                    let t0 = i as f32 / bg_segments as f32;
                    let t1 = (i + 1) as f32 / bg_segments as f32;
                    let a0 = arc_start + t0 * arc_span;
                    let a1 = arc_start + t1 * arc_span;
                    let p0 = center + egui::vec2(a0.cos(), a0.sin()) * radius;
                    let p1 = center + egui::vec2(a1.cos(), a1.sin()) * radius;
                    painter.line_segment([p0, p1], egui::Stroke::new(8.0, GAUGE_BG));
                }

                // Value arc
                let value_segments = (bg_segments as f32 * fraction) as usize;
                for i in 0..value_segments {
                    let t0 = i as f32 / bg_segments as f32;
                    let t1 = (i + 1) as f32 / bg_segments as f32;
                    let a0 = arc_start + t0 * arc_span;
                    let a1 = arc_start + t1 * arc_span;
                    let p0 = center + egui::vec2(a0.cos(), a0.sin()) * radius;
                    let p1 = center + egui::vec2(a1.cos(), a1.sin()) * radius;

                    // Gradient from color to brighter version
                    let seg_t = t0 / fraction.max(0.01);
                    let r = lerp_u8(color.r(), 255, (seg_t * 0.3) as f64);
                    let g = lerp_u8(color.g(), 255, (seg_t * 0.1) as f64);
                    let b = lerp_u8(color.b(), 255, (seg_t * 0.1) as f64);
                    let seg_color = egui::Color32::from_rgb(r, g, b);

                    painter.line_segment([p0, p1], egui::Stroke::new(8.0, seg_color));
                }

                // Glow at the tip
                if value_segments > 0 {
                    let tip_angle =
                        arc_start + (value_segments as f32 / bg_segments as f32) * arc_span;
                    let tip = center + egui::vec2(tip_angle.cos(), tip_angle.sin()) * radius;
                    painter.circle_filled(tip, 6.0, color);
                    painter.circle_filled(tip, 10.0, color_with_alpha(color, 50));
                }

                // Center dot
                painter.circle_filled(center, 4.0, TEXT_DIM);

                // Value text in center
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format_value(value))
                            .size(16.0)
                            .strong()
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        egui::RichText::new(format!("{:.0}%", fraction * 100.0))
                            .size(10.0)
                            .color(color),
                    );
                });
            });
    }

    // ─── Table view ─────────────────────────────────────────────────────

    fn render_table_view(
        &self,
        ui: &mut egui::Ui,
        numeric_fields: &[(usize, String)],
        bool_fields: &[(usize, String)],
        string_fields: &[(usize, String)],
    ) {
        ui.label(
            egui::RichText::new("ALL PARAMETERS")
                .size(11.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(4.0);

        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                egui::Grid::new("beacon_full_table")
                    .striped(true)
                    .spacing([20.0, 4.0])
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        // Header
                        ui.label(egui::RichText::new("Parameter").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Type").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Current").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Min").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Max").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Avg").size(11.0).strong().color(TEXT_DIM));
                        ui.label(egui::RichText::new("Trend").size(11.0).strong().color(TEXT_DIM));
                        ui.end_row();

                        // Numeric fields
                        for (idx, name) in numeric_fields {
                            let color = field_color(*idx);
                            let values: Vec<f64> = self
                                .history
                                .snapshots
                                .iter()
                                .filter_map(|s| {
                                    s.fields.get(name.as_str()).and_then(|v| v.as_f64())
                                })
                                .collect();

                            let current = values.last().copied().unwrap_or(0.0);
                            let min_v = values.iter().copied().fold(f64::INFINITY, f64::min);
                            let max_v = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                            let avg = if values.is_empty() {
                                0.0
                            } else {
                                values.iter().sum::<f64>() / values.len() as f64
                            };

                            ui.label(egui::RichText::new(name.as_str()).color(color).strong());
                            ui.label(egui::RichText::new("NUM").size(10.0).color(
                                egui::Color32::from_rgb(0x74, 0xB9, 0xFF),
                            ));
                            ui.label(
                                egui::RichText::new(format_value(current))
                                    .color(TEXT_BRIGHT)
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(format_value(min_v)).color(TEXT_DIM));
                            ui.label(egui::RichText::new(format_value(max_v)).color(TEXT_DIM));
                            ui.label(egui::RichText::new(format_value(avg)).color(TEXT_DIM));

                            // Trend (last 5 values)
                            let trend = self.compute_trend(name);
                            let (trend_text, trend_color) = match trend {
                                Trend::Up => ("Rising", STATUS_GREEN),
                                Trend::Down => ("Falling", STATUS_RED),
                                Trend::Stable => ("Stable", TEXT_DIM),
                            };
                            ui.label(
                                egui::RichText::new(trend_text)
                                    .size(10.0)
                                    .color(trend_color),
                            );
                            ui.end_row();
                        }

                        // Boolean fields
                        for (idx, name) in bool_fields {
                            let color = field_color(*idx);
                            let current = self
                                .history
                                .snapshots
                                .iter()
                                .rev()
                                .find_map(|s| s.fields.get(name.as_str()))
                                .and_then(|v| v.as_f64())
                                .map(|v| v > 0.5)
                                .unwrap_or(false);

                            ui.label(egui::RichText::new(name.as_str()).color(color).strong());
                            ui.label(egui::RichText::new("BOOL").size(10.0).color(
                                egui::Color32::from_rgb(0x51, 0xCF, 0x66),
                            ));
                            let state_color = if current { STATUS_GREEN } else { STATUS_RED };
                            ui.label(
                                egui::RichText::new(if current { "TRUE" } else { "FALSE" })
                                    .color(state_color)
                                    .strong(),
                            );
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.end_row();
                        }

                        // String fields
                        for (idx, name) in string_fields {
                            let color = field_color(*idx);
                            let current = self
                                .history
                                .snapshots
                                .iter()
                                .rev()
                                .find_map(|s| {
                                    if let Some(BeaconFieldValue::StringVal(v)) =
                                        s.fields.get(name.as_str())
                                    {
                                        Some(v.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| "--".to_string());

                            ui.label(egui::RichText::new(name.as_str()).color(color).strong());
                            ui.label(egui::RichText::new("STR").size(10.0).color(
                                egui::Color32::from_rgb(0xFF, 0xA9, 0x4D),
                            ));
                            ui.label(
                                egui::RichText::new(current).color(TEXT_BRIGHT).strong(),
                            );
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.label(egui::RichText::new("--").color(TEXT_DIM));
                            ui.end_row();
                        }
                    });
            });

        // Recent snapshots timeline
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("RECENT PACKETS")
                .size(11.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(4.0);

        let last_snapshots: Vec<_> = self.history.snapshots.iter().rev().take(10).collect();

        egui::Frame::new()
            .fill(CARD_BG)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                for snap in &last_snapshots {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&snap.generation_time)
                                .size(10.0)
                                .color(TEXT_DIM)
                                .monospace(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("{} fields", snap.fields.len()))
                                .size(10.0)
                                .color(TEXT_DIM),
                        );
                    });
                }
            });
    }

    // ─── Helpers ────────────────────────────────────────────────────────

    fn compute_trend(&self, field_name: &str) -> Trend {
        let values: Vec<f64> = self
            .history
            .snapshots
            .iter()
            .rev()
            .take(5)
            .filter_map(|s| s.fields.get(field_name).and_then(|v| v.as_f64()))
            .collect();

        if values.len() < 2 {
            return Trend::Stable;
        }

        // Simple linear regression direction
        let n = values.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;
        for (i, &v) in values.iter().rev().enumerate() {
            let x = i as f64;
            sum_x += x;
            sum_y += v;
            sum_xy += x * v;
            sum_x2 += x * x;
        }
        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < f64::EPSILON {
            return Trend::Stable;
        }
        let slope = (n * sum_xy - sum_x * sum_y) / denom;

        // Normalize slope by value range to determine significance
        let min_v = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max_v = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = (max_v - min_v).max(f64::EPSILON);
        let normalized = slope / range;

        if normalized > 0.05 {
            Trend::Up
        } else if normalized < -0.05 {
            Trend::Down
        } else {
            Trend::Stable
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Trend {
    Up,
    Down,
    Stable,
}

// ─── Utility functions ──────────────────────────────────────────────────────

fn format_value(v: f64) -> String {
    let abs = v.abs();
    if abs == 0.0 {
        "0".to_string()
    } else if abs >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else if abs >= 100.0 {
        format!("{:.1}", v)
    } else if abs >= 1.0 {
        format!("{:.2}", v)
    } else if abs >= 0.01 {
        format!("{:.3}", v)
    } else {
        format!("{:.4}", v)
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f64 * (1.0 - t) + b as f64 * t) as u8
}
