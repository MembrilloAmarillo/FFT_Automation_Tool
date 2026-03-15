use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::net::UdpSocket;
use std::path::Path;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::time::Duration;

use crate::error::YamcsTcError;
use crate::yamcs_client::{YamcsClient, YamcsConfig};

#[derive(Error, Debug)]
pub enum AutomationError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("toml parse error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("json parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("execution error: {0}")]
    ExecutionError(String),

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("verification error: {0}")]
    VerificationError(String),
}

impl From<YamcsTcError> for AutomationError {
    fn from(value: YamcsTcError) -> Self {
        match value {
            YamcsTcError::Validation(msg) => AutomationError::ValidationError(msg),
            YamcsTcError::Verification(msg) => AutomationError::VerificationError(msg),
            YamcsTcError::Config(msg) => AutomationError::ConfigError(msg),
            other => AutomationError::NetworkError(other.to_string()),
        }
    }
}

pub type AutomationResult<T> = Result<T, AutomationError>;

fn current_unix_time_secs() -> AutomationResult<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AutomationError::ExecutionError(format!("system time error: {}", e)))?;
    Ok(now.as_secs() as i64)
}

fn replace_unquoted_token(input: &str, token: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while i < input.len() {
        if !in_string && input[i..].starts_with(token) {
            out.push_str(replacement);
            i += token.len();
            escaped = false;
            continue;
        }

        let ch = input[i..].chars().next().unwrap_or('\0');
        let ch_len = ch.len_utf8();

        if ch == '"' && !escaped {
            in_string = !in_string;
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }

        out.push(ch);
        i += ch_len;
    }

    out
}

fn preprocess_toml_dynamic_tokens(toml_str: &str) -> AutomationResult<String> {
    let now = current_unix_time_secs()?.to_string();
    Ok(replace_unquoted_token(toml_str, "get_current_time()", &now))
}

fn resolve_dynamic_value(value: &Value) -> AutomationResult<Value> {
    match value {
        Value::String(s) if s.trim() == "get_current_time()" => {
            Ok(json!(current_unix_time_secs()?))
        }
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(resolve_dynamic_value(v)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), resolve_dynamic_value(v)?);
            }
            Ok(Value::Object(out))
        }
        _ => Ok(value.clone()),
    }
}

fn default_args() -> Value {
    json!({})
}

fn default_commander_host() -> String {
    "127.0.0.1".to_string()
}

fn default_commander_port() -> u16 {
    8092
}

fn default_commander_timeout_ms() -> u64 {
    600
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommanderUdpConfig {
    #[serde(default = "default_commander_host")]
    pub host: String,
    #[serde(default = "default_commander_port")]
    pub port: u16,
    #[serde(default = "default_commander_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for CommanderUdpConfig {
    fn default() -> Self {
        Self {
            host: default_commander_host(),
            port: default_commander_port(),
            timeout_ms: default_commander_timeout_ms(),
        }
    }
}

fn normalize_commander_name(name: &str) -> Option<&'static str> {
    match name.trim() {
        "HPC_SEND" | "COMMAND_HPC_SEND" => Some("HPC_SEND"),
        "FTP_LIST" | "COMMAND_FTP_LIST" => Some("FTP_LIST"),
        "FTP_DOWNLOAD" | "COMMAND_FTP_DOWNLOAD" => Some("FTP_DOWNLOAD"),
        "DELETE_FILE" | "COMMAND_DELETE_FILE" => Some("DELETE_FILE"),
        "DELETE_ALL" | "COMMAND_DELETE_ALL" => Some("DELETE_ALL"),
        _ => None,
    }
}

fn is_commander_command(name: &str) -> bool {
    normalize_commander_name(name).is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub name: String,
    #[serde(default = "default_args")]
    pub args: Value,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default)]
    pub retry_count: u8,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub verify_packet_name: Option<String>,
    #[serde(default)]
    pub verify_packet_name_exact: bool,
    #[serde(default)]
    pub verify_timeout_ms: u64,
    #[serde(default)]
    pub verify_poll_interval_ms: u64,
    #[serde(default)]
    pub verify_packet_limit: usize,
    #[serde(default)]
    pub dry_run_first: bool,
}

impl CommandDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: json!({}),
            delay_ms: 0,
            retry_count: 0,
            description: String::new(),
            metadata: HashMap::new(),
            verify_packet_name: None,
            verify_packet_name_exact: false,
            verify_timeout_ms: 5_000,
            verify_poll_interval_ms: 500,
            verify_packet_limit: 10,
            dry_run_first: false,
        }
    }

    pub fn with_args(mut self, args: Value) -> Self {
        self.args = args;
        self
    }

    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn with_retry(mut self, retry_count: u8) -> Self {
        self.retry_count = retry_count;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn verify_packet(mut self, packet_name: impl Into<String>) -> Self {
        self.verify_packet_name = Some(packet_name.into());
        self
    }

    pub fn verify_packet_exact(mut self, enabled: bool) -> Self {
        self.verify_packet_name_exact = enabled;
        self
    }

    pub fn with_verify_timeout(mut self, timeout_ms: u64) -> Self {
        self.verify_timeout_ms = timeout_ms;
        self
    }

    pub fn with_verify_poll_interval(mut self, poll_interval_ms: u64) -> Self {
        self.verify_poll_interval_ms = poll_interval_ms;
        self
    }

    pub fn with_verify_packet_limit(mut self, limit: usize) -> Self {
        self.verify_packet_limit = limit;
        self
    }

    pub fn with_dry_run_first(mut self, enabled: bool) -> Self {
        self.dry_run_first = enabled;
        self
    }

    pub fn validate(&self) -> AutomationResult<()> {
        if self.name.trim().is_empty() {
            return Err(AutomationError::ValidationError(
                "command name cannot be empty".into(),
            ));
        }

        if !self.args.is_object() {
            return Err(AutomationError::ValidationError(
                "command args must be a JSON object".into(),
            ));
        }

        if self.verify_poll_interval_ms == 0 && self.verify_packet_name.is_some() {
            return Err(AutomationError::ValidationError(
                "verify_poll_interval_ms must be > 0 when packet verification is enabled".into(),
            ));
        }

        if self.verify_packet_limit == 0 && self.verify_packet_name.is_some() {
            return Err(AutomationError::ValidationError(
                "verify_packet_limit must be > 0 when packet verification is enabled".into(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    #[serde(default)]
    pub yamcs: Option<YamcsConfig>,
    #[serde(default)]
    pub commander_udp: CommanderUdpConfig,
    pub commands: Vec<CommandDefinition>,
    #[serde(default)]
    pub timeout_ms: u64,
    #[serde(default = "default_stop_on_error")]
    pub stop_on_error: bool,
    #[serde(default)]
    pub repeat_count: u8,
    #[serde(default)]
    pub dry_run_all_first: bool,
}

fn default_stop_on_error() -> bool {
    true
}

impl AutomationConfig {
    pub fn validate(&self) -> AutomationResult<()> {
        if self.commands.is_empty() {
            return Err(AutomationError::ConfigError(
                "no commands defined in configuration".into(),
            ));
        }

        let has_non_commander = self.commands.iter().any(|c| !is_commander_command(&c.name));

        if has_non_commander {
            let yamcs = self.yamcs.as_ref().ok_or_else(|| {
                AutomationError::ConfigError(
                    "yamcs configuration is required for non-commander commands".into(),
                )
            })?;

            yamcs
                .validate()
                .map_err(|e| AutomationError::ConfigError(e.to_string()))?;
        }

        for cmd in &self.commands {
            cmd.validate()?;
        }

        Ok(())
    }

    pub fn estimated_duration_ms(&self) -> u64 {
        self.commands.iter().map(|c| c.delay_ms).sum()
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub successful: usize,
    pub failed: usize,
    pub elapsed_ms: u64,
    pub command_times: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    CommandStarted {
        index: usize,
        total: usize,
        name: String,
        description: String,
    },
    CommandSucceeded {
        index: usize,
        total: usize,
        name: String,
        elapsed_ms: u64,
    },
    CommandFailed {
        index: usize,
        total: usize,
        name: String,
        error: String,
    },
}

impl ExecutionStats {
    pub fn success_rate(&self) -> f32 {
        let total = self.successful + self.failed;
        if total == 0 {
            0.0
        } else {
            (self.successful as f32 / total as f32) * 100.0
        }
    }
}

pub struct AutomationEngine {
    config: AutomationConfig,
    client: Option<YamcsClient>,
}

impl AutomationEngine {
    pub fn new(config: AutomationConfig) -> AutomationResult<Self> {
        config.validate()?;
        let client = match &config.yamcs {
            Some(yamcs) => Some(YamcsClient::new(yamcs.clone())?),
            None => None,
        };
        Ok(Self { config, client })
    }

    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> AutomationResult<Self> {
        let contents = fs::read_to_string(path)?;
        Self::from_toml_str(&contents)
    }

    pub fn from_json_file<P: AsRef<Path>>(path: P) -> AutomationResult<Self> {
        let contents = fs::read_to_string(path)?;
        Self::from_json_str(&contents)
    }

    /// Parse a TOML automation config.
    /// Uses a two-step TOML → JSON conversion to avoid toml 0.8 / serde_json::Value
    /// incompatibilities (TOML integers are i64; serde_json::Value needs JSON round-trip).
    pub fn from_toml_str(toml_str: &str) -> AutomationResult<Self> {
        let toml_str = preprocess_toml_dynamic_tokens(toml_str)?;
        let toml_val: toml::Value =
            toml::from_str(&toml_str).map_err(|e| AutomationError::TomlError(e))?;
        let json_str =
            serde_json::to_string(&toml_val).map_err(|e| AutomationError::JsonError(e))?;
        let config: AutomationConfig =
            serde_json::from_str(&json_str).map_err(|e| AutomationError::JsonError(e))?;
        Self::new(config)
    }

    pub fn from_json_str(json_str: &str) -> AutomationResult<Self> {
        let config: AutomationConfig =
            serde_json::from_str(json_str).map_err(|e| AutomationError::JsonError(e))?;
        Self::new(config)
    }

    async fn dry_run_if_enabled(
        &self,
        cmd: &CommandDefinition,
        args: &Value,
    ) -> AutomationResult<()> {
        let Some(client) = &self.client else {
            return Err(AutomationError::ConfigError(
                "yamcs client is not configured".into(),
            ));
        };

        if self.config.dry_run_all_first || cmd.dry_run_first {
            client
                .dry_run_command_value(&cmd.name, args.clone())
                .await?;
        }
        Ok(())
    }

    async fn verify_if_requested(&self, cmd: &CommandDefinition) -> AutomationResult<()> {
        let Some(client) = &self.client else {
            return Err(AutomationError::ConfigError(
                "yamcs client is not configured".into(),
            ));
        };

        let Some(expected_packet) = &cmd.verify_packet_name else {
            return Ok(());
        };

        let timeout = Duration::from_millis(cmd.verify_timeout_ms.max(1));
        let poll = Duration::from_millis(cmd.verify_poll_interval_ms.max(1));
        let limit = cmd.verify_packet_limit.max(1);

        let ok = if cmd.verify_packet_name_exact {
            client
                .verify_packet_name_exact_with_polling(expected_packet, limit, timeout, poll)
                .await?
        } else {
            client
                .verify_packet_name_contains_with_polling(expected_packet, limit, timeout, poll)
                .await?
        };

        if !ok {
            return Err(AutomationError::VerificationError(format!(
                "expected packet '{}' was not observed within {} ms",
                expected_packet, cmd.verify_timeout_ms
            )));
        }

        Ok(())
    }

    fn send_commander_udp_line(&self, line: &str) -> AutomationResult<String> {
        let target = format!(
            "{}:{}",
            self.config.commander_udp.host, self.config.commander_udp.port
        );

        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| AutomationError::NetworkError(format!("udp bind failed: {}", e)))?;
        socket
            .set_read_timeout(Some(std::time::Duration::from_millis(
                self.config.commander_udp.timeout_ms.max(1),
            )))
            .map_err(|e| {
                AutomationError::NetworkError(format!("udp set_read_timeout failed: {}", e))
            })?;

        socket.send_to(line.as_bytes(), &target).map_err(|e| {
            AutomationError::NetworkError(format!("udp send_to {} failed: {}", target, e))
        })?;

        let mut buf = [0u8; 512];
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => Ok(String::from_utf8_lossy(&buf[..n]).trim().to_string()),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Err(AutomationError::NetworkError(format!(
                    "udp timeout waiting for ack from {}",
                    target
                )))
            }
            Err(e) => Err(AutomationError::NetworkError(format!(
                "udp recv_from failed: {}",
                e
            ))),
        }
    }

    fn build_commander_line(
        &self,
        cmd: &CommandDefinition,
        args: &Value,
    ) -> AutomationResult<String> {
        let canonical = normalize_commander_name(&cmd.name).ok_or_else(|| {
            AutomationError::ValidationError(format!("unsupported commander command: {}", cmd.name))
        })?;

        let arg = |k: &str| {
            args.get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        match canonical {
            "HPC_SEND" => Ok("HPC_SEND".to_string()),
            "FTP_LIST" => {
                let path = arg("path").or_else(|| arg("Arg0")).ok_or_else(|| {
                    AutomationError::ValidationError(
                        "FTP_LIST requires args.path (or args.Arg0)".into(),
                    )
                })?;
                Ok(format!("FTP_LIST {}", path))
            }
            "FTP_DOWNLOAD" => {
                let remote = arg("remote").or_else(|| arg("Arg0")).ok_or_else(|| {
                    AutomationError::ValidationError(
                        "FTP_DOWNLOAD requires args.remote (or args.Arg0)".into(),
                    )
                })?;
                let local = arg("local").or_else(|| arg("Arg1")).ok_or_else(|| {
                    AutomationError::ValidationError(
                        "FTP_DOWNLOAD requires args.local (or args.Arg1)".into(),
                    )
                })?;
                Ok(format!("FTP_DOWNLOAD {} {}", remote, local))
            }
            "DELETE_FILE" => {
                let path = arg("path").or_else(|| arg("Arg0")).ok_or_else(|| {
                    AutomationError::ValidationError(
                        "DELETE_FILE requires args.path (or args.Arg0)".into(),
                    )
                })?;
                Ok(format!("DELETE_FILE {}", path))
            }
            "DELETE_ALL" => {
                let prefix = arg("prefix").or_else(|| arg("Arg0")).ok_or_else(|| {
                    AutomationError::ValidationError(
                        "DELETE_ALL requires args.prefix (or args.Arg0)".into(),
                    )
                })?;
                Ok(format!("DELETE_ALL {}", prefix))
            }
            _ => unreachable!(),
        }
    }

    async fn execute_commander_command(&self, cmd: &CommandDefinition) -> AutomationResult<()> {
        let resolved_args = resolve_dynamic_value(&cmd.args)?;
        let line = self.build_commander_line(cmd, &resolved_args)?;
        let reply = self.send_commander_udp_line(&line)?;
        if reply.starts_with("ERROR") {
            return Err(AutomationError::ExecutionError(format!(
                "commander rejected '{}': {}",
                line, reply
            )));
        }
        Ok(())
    }

    async fn execute_command(&self, cmd: &CommandDefinition) -> AutomationResult<()> {
        if is_commander_command(&cmd.name) {
            return self.execute_commander_command(cmd).await;
        }

        let Some(client) = &self.client else {
            return Err(AutomationError::ConfigError(
                "yamcs client is not configured".into(),
            ));
        };

        let resolved_args = resolve_dynamic_value(&cmd.args)?;
        self.dry_run_if_enabled(cmd, &resolved_args).await?;
        client.issue_command_value(&cmd.name, resolved_args).await?;
        self.verify_if_requested(cmd).await?;
        Ok(())
    }

    async fn execute_with_retry(&self, cmd: &CommandDefinition) -> AutomationResult<()> {
        let mut last_error: Option<AutomationError> = None;

        for attempt in 0..=cmd.retry_count {
            match self.execute_command(cmd).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < cmd.retry_count {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AutomationError::ExecutionError(format!("failed to execute '{}'", cmd.name))
        }))
    }

    pub async fn execute_with_progress<F>(
        &self,
        mut on_event: F,
    ) -> AutomationResult<ExecutionStats>
    where
        F: FnMut(ExecutionEvent),
    {
        let start = Instant::now();
        let mut stats = ExecutionStats::default();

        let repeat_count = if self.config.repeat_count == 0 {
            1
        } else {
            self.config.repeat_count as usize
        };
        let total = repeat_count * self.config.commands.len();
        let mut index = 0usize;

        for _ in 0..repeat_count {
            for cmd in &self.config.commands {
                index += 1;
                on_event(ExecutionEvent::CommandStarted {
                    index,
                    total,
                    name: cmd.name.clone(),
                    description: cmd.description.clone(),
                });

                if self.config.timeout_ms > 0
                    && start.elapsed().as_millis() > self.config.timeout_ms as u128
                {
                    return Err(AutomationError::ExecutionError(
                        "execution timeout exceeded".into(),
                    ));
                }

                if cmd.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(cmd.delay_ms)).await;
                }

                let cmd_start = Instant::now();
                match self.execute_with_retry(cmd).await {
                    Ok(_) => {
                        let elapsed = cmd_start.elapsed().as_millis() as u64;
                        stats.command_times.insert(cmd.name.clone(), elapsed);
                        stats.successful += 1;
                        on_event(ExecutionEvent::CommandSucceeded {
                            index,
                            total,
                            name: cmd.name.clone(),
                            elapsed_ms: elapsed,
                        });
                    }
                    Err(e) => {
                        stats.failed += 1;
                        on_event(ExecutionEvent::CommandFailed {
                            index,
                            total,
                            name: cmd.name.clone(),
                            error: e.to_string(),
                        });
                        if self.config.stop_on_error {
                            stats.elapsed_ms = start.elapsed().as_millis() as u64;
                            return Err(e);
                        }
                    }
                }
            }
        }

        stats.elapsed_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    pub async fn execute(&self) -> AutomationResult<ExecutionStats> {
        self.execute_with_progress(|_| {}).await
    }

    pub fn config(&self) -> &AutomationConfig {
        &self.config
    }

    pub fn client(&self) -> Option<&YamcsClient> {
        self.client.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        replace_unquoted_token, resolve_dynamic_value, AutomationConfig, AutomationEngine,
    };
    use serde_json::json;
    use std::net::UdpSocket;
    use std::sync::mpsc;

    #[test]
    fn replace_unquoted_time_token_in_toml() {
        let input = "Time = get_current_time()\nLiteral = \"get_current_time()\"\n";
        let out = replace_unquoted_token(input, "get_current_time()", "123");
        assert!(out.contains("Time = 123"));
        assert!(out.contains("Literal = \"get_current_time()\""));
    }

    #[test]
    fn resolve_dynamic_time_string_to_number() {
        let input = json!({
            "Time": "get_current_time()",
            "Nested": {
                "Other": 1
            }
        });
        let out = resolve_dynamic_value(&input).expect("resolution should succeed");
        assert!(out["Time"].is_i64() || out["Time"].is_u64());
        assert_eq!(out["Nested"]["Other"], json!(1));
    }

    #[test]
    fn commander_only_config_without_yamcs_is_valid() {
        let cfg = json!({
            "commander_udp": {"host": "127.0.0.1", "port": 8092},
            "commands": [{"name": "HPC_SEND"}],
            "stop_on_error": true
        });
        let config: AutomationConfig = serde_json::from_value(cfg).expect("config parse");
        assert!(AutomationEngine::new(config).is_ok());
    }

    #[test]
    fn non_commander_without_yamcs_fails_validation() {
        let cfg = json!({
            "commands": [{"name": "PUS_17_1"}],
            "stop_on_error": true
        });
        let config: AutomationConfig = serde_json::from_value(cfg).expect("config parse");
        let err = match AutomationEngine::new(config) {
            Ok(_) => panic!("should fail without yamcs"),
            Err(e) => e,
        };
        assert!(err
            .to_string()
            .contains("yamcs configuration is required for non-commander commands"));
    }

    #[test]
    fn execute_commander_ftp_list_sends_udp_line() {
        let server = UdpSocket::bind("127.0.0.1:0").expect("bind udp server");
        server
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set timeout");
        let addr = server.local_addr().expect("local addr");

        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            if let Ok((n, from)) = server.recv_from(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(msg);
                let _ = server.send_to(b"QUEUED seq=1 cmd=2", from);
            }
        });

        let cfg = json!({
            "commander_udp": {"host": "127.0.0.1", "port": addr.port(), "timeout_ms": 800},
            "commands": [{"name": "FTP_LIST", "args": {"path": "/logs"}}],
            "stop_on_error": true
        });
        let config: AutomationConfig = serde_json::from_value(cfg).expect("config parse");
        let engine = AutomationEngine::new(config).expect("engine");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.execute());
        assert!(result.is_ok());

        let received = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("should receive udp command");
        assert_eq!(received, "FTP_LIST /logs");
    }

    #[test]
    fn execute_commander_alias_command_name_works() {
        let server = UdpSocket::bind("127.0.0.1:0").expect("bind udp server");
        server
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set timeout");
        let addr = server.local_addr().expect("local addr");

        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            if let Ok((n, from)) = server.recv_from(&mut buf) {
                let msg = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(msg);
                let _ = server.send_to(b"QUEUED seq=2 cmd=2", from);
            }
        });

        let cfg = json!({
            "commander_udp": {"host": "127.0.0.1", "port": addr.port(), "timeout_ms": 800},
            "commands": [{"name": "COMMAND_FTP_LIST", "args": {"path": "/hk"}}],
            "stop_on_error": true
        });
        let config: AutomationConfig = serde_json::from_value(cfg).expect("config parse");
        let engine = AutomationEngine::new(config).expect("engine");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.execute());
        assert!(result.is_ok());

        let received = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("should receive udp command");
        assert_eq!(received, "FTP_LIST /hk");
    }

    #[test]
    fn execute_commander_times_out_without_ack() {
        let server = UdpSocket::bind("127.0.0.1:0").expect("bind udp server");
        server
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set timeout");
        let addr = server.local_addr().expect("local addr");

        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            // Intentionally receive and do NOT send ack back.
            let _ = server.recv_from(&mut buf);
        });

        let cfg = json!({
            "commander_udp": {"host": "127.0.0.1", "port": addr.port(), "timeout_ms": 100},
            "commands": [{"name": "HPC_SEND", "retry_count": 0}],
            "stop_on_error": true
        });
        let config: AutomationConfig = serde_json::from_value(cfg).expect("config parse");
        let engine = AutomationEngine::new(config).expect("engine");

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(engine.execute());
        assert!(result.is_err());
        assert!(result
            .err()
            .expect("err")
            .to_string()
            .contains("udp timeout waiting for ack"));
    }
}
