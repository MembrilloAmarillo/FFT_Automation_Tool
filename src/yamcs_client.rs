use crate::error::{Result, YamcsTcError};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::time::{sleep, Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamcsConfig {
    pub base_url: String,
    pub instance: String,
    pub processor: String,
    pub username: String,
    pub password: String,
    pub origin: String,
}

impl YamcsConfig {
    pub fn validate(&self) -> Result<()> {
        if self.base_url.trim().is_empty() {
            return Err(YamcsTcError::Config("base_url cannot be empty".into()));
        }

        if self.instance.trim().is_empty() {
            return Err(YamcsTcError::Config("instance cannot be empty".into()));
        }

        if self.processor.trim().is_empty() {
            return Err(YamcsTcError::Config("processor cannot be empty".into()));
        }

        if self.origin.trim().is_empty() {
            return Err(YamcsTcError::Config("origin cannot be empty".into()));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct YamcsClient {
    cfg: YamcsConfig,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCommandRequest {
    pub args: Value,
    pub origin: String,
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: i32,
    #[serde(rename = "dryRun")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: Option<String>,
    #[serde(rename = "qualifiedName")]
    pub qualified_name: Option<String>,
    #[serde(rename = "shortDescription")]
    pub short_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketId {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketInfo {
    pub id: Option<PacketId>,
    #[serde(rename = "generationTime")]
    pub generation_time: Option<String>,
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: Option<u32>,
}

#[derive(Debug, Clone)]
struct CommandDefinition {
    raw: Value,
}

impl CommandDefinition {
    fn base_command(&self) -> Option<String> {
        self.raw
            .get("baseCommand")
            .and_then(|bc| {
                bc.get("qualifiedName")
                    .or_else(|| bc.get("name"))
                    .and_then(|v| v.as_str())
            })
            .map(|s| s.to_string())
    }

    fn argument_names(&self) -> Vec<String> {
        self.raw
            .get("argument")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|arg| {
                        arg.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn assignments(&self) -> Vec<(String, Value)> {
        self.raw
            .get("argumentAssignment")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let name = a.get("name").and_then(|n| n.as_str())?;
                        let value = a.get("value")?.clone();
                        Some((name.to_string(), value))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    fn payload_arg_names(&self) -> Vec<String> {
        // Yamcs HTTP API embeds the argument inline: entry.argument.name
        self.raw
            .get("commandContainer")
            .and_then(|cc| cc.get("entry"))
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|entry| {
                        entry
                            .get("argument")
                            .and_then(|a| a.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Sum of sizeInBits for every argument entry in this command's container.
    fn container_bits(&self) -> u32 {
        self.raw
            .get("commandContainer")
            .and_then(|cc| cc.get("entry"))
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|entry| {
                        entry
                            .get("argument")
                            .and_then(|a| a.get("type"))
                            .and_then(|t| t.get("dataEncoding"))
                            .and_then(|de| de.get("sizeInBits"))
                            .and_then(|b| b.as_u64())
                            .unwrap_or(0) as u32
                    })
                    .sum()
            })
            .unwrap_or(0)
    }

    /// True if this command's container has an entry for the given argument name.
    fn has_arg_in_container(&self, arg_name: &str) -> bool {
        self.raw
            .get("commandContainer")
            .and_then(|cc| cc.get("entry"))
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter().any(|entry| {
                    entry
                        .get("argument")
                        .and_then(|a| a.get("name"))
                        .and_then(|n| n.as_str())
                        == Some(arg_name)
                })
            })
            .unwrap_or(false)
    }
}

impl YamcsClient {
    pub fn new(cfg: YamcsConfig) -> Result<Self> {
        cfg.validate()?;

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self { cfg, http })
    }

    pub fn config(&self) -> &YamcsConfig {
        &self.cfg
    }

    fn mdb_commands_url(&self) -> String {
        format!(
            "{}/api/mdb/{}/commands",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance
        )
    }

    fn mdb_command_url(&self, command_name: &str) -> String {
        format!(
            "{}/api/mdb/{}/commands{}",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance,
            if command_name.starts_with('/') {
                command_name.to_string()
            } else {
                format!("/{}", command_name)
            }
        )
    }

    fn issue_command_url(&self, command_name: &str) -> String {
        format!(
            "{}/api/processors/{}/{}/commands{}",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance,
            self.cfg.processor,
            if command_name.starts_with('/') {
                command_name.to_string()
            } else {
                format!("/{}", command_name)
            }
        )
    }

    fn packets_url(&self, limit: usize) -> String {
        format!(
            "{}/api/archive/{}/packets?order=desc&limit={}",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance,
            limit
        )
    }

    fn packets_by_name_url(&self, packet_name: &str, gentime: &str, seqnum: usize) -> String {
        format!(
            "{}/api/archive/{}/packets/{}/{}/{}:extract",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance,
            packet_name,
            gentime,
            seqnum
        )
    }

    fn container_url(&self, container_path: &str) -> String {
        format!(
            "{}/api/mdb/{}/containers/{}",
            self.cfg.base_url.trim_end_matches('/'),
            self.cfg.instance,
            container_path.trim_start_matches('/')
        )
    }

    pub async fn list_commands(&self) -> Result<Value> {
        let res = self
            .http
            .get(self.mdb_commands_url())
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn list_command_infos(&self) -> Result<Vec<CommandInfo>> {
        let value = self.list_commands().await?;

        if let Some(commands) = value.get("commands").and_then(|v| v.as_array()) {
            let parsed: std::result::Result<Vec<CommandInfo>, _> = commands
                .iter()
                .cloned()
                .map(serde_json::from_value)
                .collect();
            return parsed.map_err(YamcsTcError::from);
        }

        if let Some(commands) = value.as_array() {
            let parsed: std::result::Result<Vec<CommandInfo>, _> = commands
                .iter()
                .cloned()
                .map(serde_json::from_value)
                .collect();
            return parsed.map_err(YamcsTcError::from);
        }

        Err(YamcsTcError::Command(
            "unexpected response shape from list_commands".into(),
        ))
    }

    pub async fn resolve_command_name(&self, requested_name: &str) -> Result<String> {
        let requested = requested_name.trim();
        if requested.is_empty() {
            return Err(YamcsTcError::Validation(
                "requested command name cannot be empty".into(),
            ));
        }

        let requested_no_slash = requested.trim_start_matches('/');
        let requested_with_slash = if requested.starts_with('/') {
            requested.to_string()
        } else {
            format!("/{}", requested)
        };

        let commands = self.list_command_infos().await?;

        if let Some(cmd) = commands
            .iter()
            .find(|c| c.name.as_deref() == Some(requested_no_slash))
        {
            return Ok(cmd
                .qualified_name
                .clone()
                .or_else(|| cmd.name.clone().map(|n| format!("/{}", n)))
                .unwrap_or_else(|| requested_with_slash.clone()));
        }

        if let Some(cmd) = commands.iter().find(|c| {
            c.qualified_name.as_deref() == Some(requested)
                || c.qualified_name.as_deref() == Some(requested_with_slash.as_str())
        }) {
            return Ok(cmd
                .qualified_name
                .clone()
                .or_else(|| cmd.name.clone().map(|n| format!("/{}", n)))
                .unwrap_or_else(|| requested_with_slash.clone()));
        }

        let trailing_matches: Vec<&CommandInfo> = commands
            .iter()
            .filter(|c| {
                c.qualified_name
                    .as_deref()
                    .map(|q| q.rsplit('/').next() == Some(requested_no_slash))
                    .unwrap_or(false)
                    || c.name.as_deref() == Some(requested_no_slash)
            })
            .collect();

        if trailing_matches.len() == 1 {
            let cmd = trailing_matches[0];
            return Ok(cmd
                .qualified_name
                .clone()
                .or_else(|| cmd.name.clone().map(|n| format!("/{}", n)))
                .unwrap());
        }

        if trailing_matches.len() > 1 {
            return Err(YamcsTcError::Command(format!(
                "ambiguous command name '{}': multiple MDB matches found",
                requested
            )));
        }

        Err(YamcsTcError::Command(format!(
            "command '{}' not found in MDB",
            requested
        )))
    }

    pub async fn get_command(&self, command_name: &str) -> Result<Value> {
        let resolved = self.resolve_command_name(command_name).await?;
        let res = self
            .http
            .get(self.mdb_command_url(&resolved))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    async fn get_command_definition(&self, command_name: &str) -> Result<CommandDefinition> {
        let raw = self.get_command(command_name).await?;
        Ok(CommandDefinition { raw })
    }

    async fn get_command_definition_by_qualified_name(
        &self,
        qualified_name: &str,
    ) -> Result<CommandDefinition> {
        let res = self
            .http
            .get(self.mdb_command_url(qualified_name))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        let raw: Value = res.json().await?;
        Ok(CommandDefinition { raw })
    }

    async fn merged_command_chain(&self, command_name: &str) -> Result<Vec<CommandDefinition>> {
        let mut chain = Vec::new();
        let mut current = self.get_command_definition(command_name).await?;

        loop {
            let base = current.base_command();
            chain.push(current);

            let Some(next_name) = base else {
                break;
            };

            current = self
                .get_command_definition_by_qualified_name(&next_name)
                .await?;
        }

        chain.reverse();
        Ok(chain)
    }

    async fn build_final_args(
        &self,
        command_name: &str,
        user_args: Value,
    ) -> Result<(String, Map<String, Value>)> {
        let resolved = self.resolve_command_name(command_name).await?;
        let chain = self.merged_command_chain(&resolved).await?;

        let user_obj = user_args
            .as_object()
            .ok_or_else(|| YamcsTcError::Validation("user args must be a JSON object".into()))?;

        // Args fixed by ArgumentAssignment — never send these, Yamcs fills them in.
        let fixed_args: std::collections::HashSet<String> = chain
            .iter()
            .flat_map(|cmd| cmd.assignments().into_iter().map(|(name, _)| name))
            .collect();

        // All arg names declared anywhere in the chain.
        let declared_args: std::collections::HashSet<String> =
            chain.iter().flat_map(|cmd| cmd.argument_names()).collect();

        let mut final_args = Map::<String, Value>::new();

        // Forward all user-supplied args that are declared and not fixed.
        // This lets callers override any header field (e.g. CCSDS_Source_ID).
        for (k, v) in user_obj {
            if !declared_args.contains(k) {
                continue; // unknown to the MDB, ignore
            }
            if fixed_args.contains(k) {
                continue; // Yamcs fills this via argumentAssignment
            }
            final_args.insert(k.clone(), v.clone());
        }

        // Auto-compute CCSDS_Packet_Data_Length if required but not supplied.
        // It has no initialValue in the MDB, so Yamcs will reject the command
        // without it. Value = (total bits of everything after the primary header
        // in bytes) - 1, per CCSDS packet structure.
        const PDL: &str = "CCSDS_Packet_Data_Length";
        if declared_args.contains(PDL) && !fixed_args.contains(PDL) && !final_args.contains_key(PDL)
        {
            // Sum bits from every container in the chain EXCEPT the one that
            // holds CCSDS_Packet_Data_Length (the CCSDS primary header).
            let total_bits: u32 = chain
                .iter()
                .filter(|cmd| !cmd.has_arg_in_container(PDL))
                .map(|cmd| cmd.container_bits())
                .sum();
            if total_bits > 0 {
                let pdl_value = (total_bits / 8).saturating_sub(1);
                final_args.insert(PDL.to_string(), json!(pdl_value));
            }
        }

        Ok((resolved, final_args))
    }

    pub async fn issue_command_value(&self, command_name: &str, args: Value) -> Result<Value> {
        let (resolved, final_args) = self.build_final_args(command_name, args).await?;

        let body = IssueCommandRequest {
            args: Value::Object(final_args),
            origin: self.cfg.origin.clone(),
            sequence_number: 0,
            dry_run: false,
        };

        let res = self
            .http
            .post(self.issue_command_url(&resolved))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn dry_run_command_value(&self, command_name: &str, args: Value) -> Result<Value> {
        let (resolved, final_args) = self.build_final_args(command_name, args).await?;
        let body = IssueCommandRequest {
            args: Value::Object(final_args),
            origin: self.cfg.origin.clone(),
            sequence_number: 0,
            dry_run: true,
        };

        let res = self
            .http
            .post(self.issue_command_url(&resolved))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn get_container(&self, container_path: &str) -> Result<Value> {
        let res = self
            .http
            .get(self.container_url(container_path))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn recent_packets(&self, limit: usize) -> Result<Value> {
        let res = self
            .http
            .get(self.packets_url(limit))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn list_packages(&self) -> Result<Value> {
        let res = self
            .http
            .get(format!(
                "{}/api/archive/{}/packets",
                self.cfg.base_url.trim_end_matches('/'),
                self.cfg.instance
            ))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn recent_packets_by_name(&self, packet_name: &str, limit: usize) -> Result<Value> {
        let packages = self.list_packages().await?;

        let mut gen_time: &str = "";
        let mut seq_num: usize = 0;
        let mut full_packet_name = packet_name;

        for pkg in packages
            .get("packet")
            .and_then(|v| v.as_array())
            .ok_or_else(|| YamcsTcError::Verification("missing packet array in packages".into()))?
        {
            // check if the packet name matches the one we're looking for, and if so print the whole package for debugging
            // the part to check is the last keyword after the path, e.g. for /UCF/PUS/PUS_1_1 it would be PUS_1_1
            //
            if let Some(name) = pkg
                .get("id")
                .and_then(|id| id.get("name"))
                .and_then(|n| n.as_str())
            {
                if name.rsplit('/').next() == Some(packet_name) {
                    full_packet_name = name;
                    eprintln!("Found package for '{}': {:#}", packet_name, pkg);
                    gen_time = pkg
                        .get("generationTime")
                        .and_then(|gt| gt.as_str())
                        .ok_or_else(|| {
                            YamcsTcError::Verification(format!(
                                "missing generationTime for packet '{}'",
                                packet_name
                            ))
                        })?;
                    seq_num = pkg
                        .get("sequenceNumber")
                        .and_then(|sn| sn.as_u64())
                        .ok_or_else(|| {
                            YamcsTcError::Verification(format!(
                                "missing sequenceNumber for packet '{}'",
                                packet_name
                            ))
                        })? as usize;
                    break;
                }
            }
        }

        let encoded_full_packet_name = full_packet_name.to_string().replace("/", "%2F");

        let res = self
            .http
            .get(self.packets_by_name_url(encoded_full_packet_name.as_str(), gen_time, seq_num))
            .basic_auth(&self.cfg.username, Some(&self.cfg.password))
            .send()
            .await?
            .error_for_status()?;

        Ok(res.json().await?)
    }

    pub async fn recent_packet_infos(&self, limit: usize) -> Result<Vec<PacketInfo>> {
        let data = self.recent_packets(limit).await?;
        Self::parse_packet_infos(data)
    }

    pub async fn recent_packet_infos_by_name(
        &self,
        packet_name: &str,
        limit: usize,
    ) -> Result<Vec<PacketInfo>> {
        let data = self.recent_packets_by_name(packet_name, limit).await?;
        Self::parse_packet_infos(data)
    }

    fn parse_packet_infos(data: Value) -> Result<Vec<PacketInfo>> {
        let packets = data
            .get("packet")
            .and_then(|v| v.as_array())
            .ok_or_else(|| YamcsTcError::Verification("missing packet array".into()))?;

        let parsed: std::result::Result<Vec<PacketInfo>, _> = packets
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect();

        parsed.map_err(YamcsTcError::from)
    }

    pub async fn verify_packet_name_contains(
        &self,
        expected_fragment: &str,
        limit: usize,
    ) -> Result<bool> {
        let packets = self.recent_packet_infos(limit).await?;

        Ok(packets.iter().any(|p| {
            p.id.as_ref()
                .and_then(|id| id.name.as_deref())
                .map(|n| n.contains(expected_fragment))
                .unwrap_or(false)
        }))
    }

    pub async fn verify_packet_name_contains_with_polling(
        &self,
        expected_fragment: &str,
        limit: usize,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<bool> {
        let started = Instant::now();

        loop {
            if self
                .verify_packet_name_contains(expected_fragment, limit)
                .await?
            {
                return Ok(true);
            }

            if started.elapsed() >= timeout {
                return Ok(false);
            }

            sleep(poll_interval).await;
        }
    }

    pub async fn verify_packet_name_exact_with_polling(
        &self,
        expected_name: &str,
        limit: usize,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<bool> {
        let started = Instant::now();

        loop {
            let packets = self.recent_packet_infos(limit).await?;
            let found = packets.iter().any(|p| {
                p.id.as_ref()
                    .and_then(|id| id.name.as_deref())
                    .map(|n| n == expected_name)
                    .unwrap_or(false)
            });

            if found {
                return Ok(true);
            }

            if started.elapsed() >= timeout {
                return Ok(false);
            }

            sleep(poll_interval).await;
        }
    }

    pub async fn issue_simple(&self, command_name: &str) -> Result<Value> {
        self.issue_command_value(command_name, json!({})).await
    }
}
