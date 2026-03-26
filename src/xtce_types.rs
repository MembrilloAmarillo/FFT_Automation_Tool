use crate::error::{Result, YamcsTcError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Full Function_ID_t enumeration from XTCE.
///
/// EPS channels:  512–534
/// PAY1:           0–7
/// PAY2:         256–263
/// SYSTEM:       768–778
/// EOM:          776–778  (overlap with SYSTEM range, same numeric space)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u16)]
pub enum FunctionId {
    // --- PAY1 (0..7) ---
    UcfPay1Start = 0,
    UcfPay1Stop = 1,
    UcfPay1SetMode = 2,
    UcfPay1Reset = 3,
    UcfPay1GetStatus = 4,
    UcfPay1GetHk = 5,
    UcfPay1StopTime = 6,
    UcfPay1DownloadPacket = 7,

    // --- PAY2 (256..263) ---
    UcfPay2Start = 256,
    UcfPay2Stop = 257,
    UcfPay2SetMode = 258,
    UcfPay2Reset = 259,
    UcfPay2GetStatus = 260,
    UcfPay2GetHk = 261,
    UcfPay2StopTime = 262,
    UcfPay2DownloadPacket = 263,

    // --- EPS (512..534) ---
    UcfEpsOutputBusChannelOn = 516,
    UcfEpsOutputBusChannelOff = 517,
    UcfEpsCorrectTime = 533,

    // --- SYSTEM (768..778) ---
    UcfSystemGetHk = 768,
    UcfSystemReset = 769,
    UcfSystemSetMode = 770,
    UcfSystemGetStatus = 771,
    UcfSystemSaveConfig = 772,
    UcfSystemLoadConfig = 773,
    UcfSystemChangeTime = 774,
    UcfSystemCompressFile = 775,
    UcfSystemEndOfMission = 776,
    UcfSystemEndOfMission2 = 777,
    UcfSystemEndOfMission3 = 778,
}

impl FunctionId {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Full HK_Structure_ID_Type enumeration from XTCE.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u16)]
pub enum HkStructureId {
    // Normal-mode (1..12)
    Adcs = 1,
    EpsPbu = 2,
    EpsPdu = 3,
    EpsPiu = 4,
    EpsSys = 5,
    SolarPanels = 6,
    Transceiver = 7,
    EpsPcu = 8,
    EpsPiuOcf = 9,
    EpsPbuAbf = 10,
    EpsParam = 11,
    EpsObcMode = 12,
    // Safe-mode (17..21)
    AdcsSm = 17,
    EpsPbuSm = 18,
    EpsPduSm = 19,
    EpsPiuSm = 20,
    EpsSysSm = 21,
    // Nominal-mode (33..37)
    AdcsNm = 33,
    EpsPbuNm = 34,
    EpsPduNm = 35,
    EpsPiuNm = 36,
    EpsSysNm = 37,
    // Orbit-mode (49..53)
    AdcsOm = 49,
    EpsPbuOm = 50,
    EpsPduOm = 51,
    EpsPiuOm = 52,
    EpsSysOm = 53,
    // Transceiver diagnostic mode
    TransceiverDm = 71,
}

impl HkStructureId {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

// ---------------------------------------------------------------------------
// PUS 3-31 body entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pus331Entry {
    #[serde(rename = "HK_Parameter_Report_Structure_ID")]
    pub hk_parameter_report_structure_id: u16,
    #[serde(rename = "Collection_Interval")]
    pub collection_interval: u32,
}

impl Pus331Entry {
    pub fn new(hk: HkStructureId, interval: u32) -> Self {
        Self {
            hk_parameter_report_structure_id: hk.as_u16(),
            collection_interval: interval,
        }
    }
}

// ---------------------------------------------------------------------------
// PUS 20-3 body entry  (Parameter_ID + Value)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pus203Entry {
    #[serde(rename = "Parameter_ID")]
    pub parameter_id: u16,
    #[serde(rename = "Value")]
    pub value: u32,
}

impl Pus203Entry {
    pub fn new(parameter_id: u16, value: u32) -> Self {
        Self {
            parameter_id,
            value,
        }
    }
}

// ---------------------------------------------------------------------------
// PUS 8-1 variable-size body entry  (array of u32 function arguments)
// ---------------------------------------------------------------------------

/// A single 32-bit word in the body of a PUS_8_1_Variable_Size command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pus81BodyEntry {
    #[serde(rename = "Arg")]
    pub arg: u32,
}

impl Pus81BodyEntry {
    pub fn new(arg: u32) -> Self {
        Self { arg }
    }
}

// ---------------------------------------------------------------------------
// Helper: validate N == items.len()
// ---------------------------------------------------------------------------

pub fn ensure_count_matches<T>(n: usize, items: &[T], field: &str) -> Result<()> {
    if n != items.len() {
        return Err(YamcsTcError::Validation(format!(
            "{} count {} does not match provided items {}",
            field,
            n,
            items.len()
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Range-limited integer helpers
// ---------------------------------------------------------------------------

pub fn u24(value: u32) -> Result<u32> {
    if value > 0x00FF_FFFF {
        return Err(YamcsTcError::Validation(format!(
            "value {} exceeds u24 range",
            value
        )));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// JSON array helpers
// ---------------------------------------------------------------------------

pub fn hk_array_value(items: &[HkStructureId]) -> Value {
    Value::Array(items.iter().map(|i| json!(*i as u16)).collect())
}

pub fn u16_array_value(items: &[u16]) -> Value {
    Value::Array(items.iter().map(|i| json!(*i)).collect())
}

pub fn pus331_array_value(items: &[Pus331Entry]) -> Value {
    Value::Array(items.iter().map(|i| json!(i)).collect())
}

pub fn pus203_array_value(items: &[Pus203Entry]) -> Value {
    Value::Array(items.iter().map(|i| json!(i)).collect())
}

pub fn pus81_body_array_value(items: &[Pus81BodyEntry]) -> Value {
    Value::Array(items.iter().map(|i| json!(i)).collect())
}
