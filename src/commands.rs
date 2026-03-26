use crate::error::Result;
use crate::xtce_types::{
    ensure_count_matches, hk_array_value, pus203_array_value, pus331_array_value,
    pus81_body_array_value, u16_array_value, u24, HkStructureId, Pus203Entry, Pus331Entry,
    Pus81BodyEntry,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCommand {
    pub name: String,
    pub args: Value,
    pub description: Option<String>,
}

impl PreparedCommand {
    pub fn new(name: impl Into<String>, args: Value) -> Self {
        Self {
            name: name.into(),
            args,
            description: None,
        }
    }

    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

pub struct Commands;

impl Commands {
    pub fn pus_17_1() -> PreparedCommand {
        PreparedCommand::new("PUS_17_1", json!({})).with_description("Are-you-alive request")
    }

    pub fn pus_8_1(function_id: u16) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1",
            json!({
                "Function_ID": function_id,
            }),
        )
        .with_description("Generic PUS 8-1 command")
    }

    pub fn pus_8_1_eps_output_bus_channel_on(channel_id: u8) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON",
            json!({
                "Channel_ID": channel_id,
            }),
        )
        .with_description("Turn on EPS output bus channel")
    }

    pub fn pus_8_1_eps_output_bus_channel_off(channel_id: u8) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_OFF",
            json!({ "Channel_ID": channel_id }),
        )
        .with_description("Turn off EPS output bus channel")
    }

    pub fn pus_8_1_system_change_time(unix_time: u32) -> PreparedCommand {
        PreparedCommand::new("PUS_8_1_SYSTEM_CHANGE_TIME", json!({ "Time": unix_time }))
            .with_description("Update spacecraft UNIX time")
    }

    pub fn pus_8_1_eps_correct_time(delta_seconds: i32) -> PreparedCommand {
        PreparedCommand::new("PUS_8_1_EPS_CORRECT_TIME", json!({ "Time": delta_seconds }))
            .with_description("Correct EPS time")
    }

    pub fn pus_8_1_pay_1_stop_time_id(stop_time: u32) -> Result<PreparedCommand> {
        Ok(PreparedCommand::new(
            "PUS_8_1_PAY1_STOP_TIME_ID",
            json!({ "Stop_Time": u24(stop_time)? }),
        )
        .with_description("Set payload 1 stop time"))
    }

    pub fn pus_8_1_pay_2_stop_time_id(stop_time: u32) -> Result<PreparedCommand> {
        Ok(PreparedCommand::new(
            "PUS_8_1_PAY2_STOP_TIME_ID",
            json!({ "Stop_Time": u24(stop_time)? }),
        )
        .with_description("Set payload 2 stop time"))
    }

    pub fn pus_8_1_pay_1_download_exp(packet_id: u32) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_PAY1_DOWNLOAD_EXP",
            json!({ "PacketID": packet_id }),
        )
        .with_description("Download payload 1 experiment packet")
    }

    pub fn pus_8_1_pay_2_download_exp(packet_id: u32) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_PAY2_DOWNLOAD_EXP",
            json!({ "PacketID": packet_id }),
        )
        .with_description("Download payload 2 experiment packet")
    }

    pub fn pus_8_1_end_of_mission() -> PreparedCommand {
        PreparedCommand::new("PUS_8_1_END_OF_MISSION", json!({})).with_description("End of mission")
    }

    pub fn pus_8_1_end_of_mission_2(decrypted_val: u64) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_END_OF_MISSION_2",
            json!({ "DecryptedVal": decrypted_val }),
        )
        .with_description("End of mission 2 with decryption")
    }

    pub fn pus_8_1_end_of_mission_3(decrypted_val: u64) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1_END_OF_MISSION_3",
            json!({ "DecryptedVal": decrypted_val }),
        )
        .with_description("End of mission 3 with decryption")
    }

    pub fn pus_3_31(entries: Vec<Pus331Entry>) -> Result<PreparedCommand> {
        let n = entries.len();
        ensure_count_matches(n, &entries, "PUS331Body")?;

        Ok(PreparedCommand::new(
            "PUS_3_31",
            json!({
                "N": n,
                "PUS_3_31_Body": pus331_array_value(&entries),
            }),
        )
        .with_description("Change HK collection intervals"))
    }

    pub fn pus_3_33(hk_ids: Vec<HkStructureId>) -> Result<PreparedCommand> {
        let n = hk_ids.len();
        ensure_count_matches(n, &hk_ids, "HK_Structure_ID")?;

        Ok(PreparedCommand::new(
            "PUS_3_33",
            json!({
                "N": n,
                "HK_Structure_ID": hk_array_value(&hk_ids),
            }),
        )
        .with_description("Query HK collection interval information"))
    }

    pub fn pus_5_5(event_ids: Vec<u16>) -> Result<PreparedCommand> {
        let n = event_ids.len();
        ensure_count_matches(n, &event_ids, "EventID")?;

        Ok(PreparedCommand::new(
            "PUS_5_5",
            json!({
                "N": n,
                "Event_ID": u16_array_value(&event_ids),
            }),
        )
        .with_description("Enable event reports"))
    }

    pub fn pus_5_6(event_ids: Vec<u16>) -> Result<PreparedCommand> {
        let n = event_ids.len();
        ensure_count_matches(n, &event_ids, "EventID")?;

        Ok(PreparedCommand::new(
            "PUS_5_6",
            json!({
                "N": n,
                "Event_ID": u16_array_value(&event_ids),
            }),
        )
        .with_description("Disable event reports"))
    }

    pub fn pus_24_1(hk: HkStructureId, parameter_ids: Vec<u16>) -> Result<PreparedCommand> {
        let n = parameter_ids.len();
        ensure_count_matches(n, &parameter_ids, "ParameterID")?;

        Ok(PreparedCommand::new(
            "PUS_24_1",
            json!({
                "HK_Structure_ID": hk.as_u16(),
                "N": n,
                "Parameter_ID": u16_array_value(&parameter_ids),
            }),
        )
        .with_description("Enable parameter collection for HK structure"))
    }

    pub fn pus_24_2(hk: HkStructureId, parameter_ids: Vec<u16>) -> Result<PreparedCommand> {
        let n = parameter_ids.len();
        ensure_count_matches(n, &parameter_ids, "ParameterID")?;

        Ok(PreparedCommand::new(
            "PUS_24_2",
            json!({
                "HK_Structure_ID": hk.as_u16(),
                "N": n,
                "Parameter_ID": u16_array_value(&parameter_ids),
            }),
        )
        .with_description("Disable parameter collection for HK structure"))
    }

    pub fn pus_24_3(hk: HkStructureId) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_24_3",
            json!({
                "HK_Structure_ID": hk.as_u16(),
            }),
        )
        .with_description("Query active parameters for HK structure")
    }

    // -----------------------------------------------------------------------
    // PUS Service 4 — Statistics monitoring
    // -----------------------------------------------------------------------

    /// PUS 4-1: Request statistics report for all HK parameters.
    ///
    /// `reset_flag` — 0 = do not reset, 1 = reset after reporting.
    pub fn pus_4_1(reset_flag: u8) -> PreparedCommand {
        PreparedCommand::new("PUS_4_1", json!({ "Reset_Flag": reset_flag }))
            .with_description("Request statistics report for all HK parameters")
    }

    /// PUS 4-3: Reset all statistics data.
    pub fn pus_4_3() -> PreparedCommand {
        PreparedCommand::new("PUS_4_3", json!({})).with_description("Reset all HK statistics data")
    }

    /// PUS 4-4: Enable periodic statistics reporting.
    ///
    /// `reporting_interval` — interval in seconds (uint32).
    pub fn pus_4_4(reporting_interval: u32) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_4_4",
            json!({ "Reporting_Interval": reporting_interval }),
        )
        .with_description("Enable periodic HK statistics reporting")
    }

    /// PUS 4-5: Stop periodic statistics reporting.
    pub fn pus_4_5() -> PreparedCommand {
        PreparedCommand::new("PUS_4_5", json!({}))
            .with_description("Stop periodic HK statistics reporting")
    }

    // -----------------------------------------------------------------------
    // PUS Service 5 — Event reporting
    // -----------------------------------------------------------------------

    /// PUS 5-7: Request list of disabled event reports.
    pub fn pus_5_7() -> PreparedCommand {
        PreparedCommand::new("PUS_5_7", json!({}))
            .with_description("Request list of disabled event reports")
    }

    // -----------------------------------------------------------------------
    // PUS Service 8 — Function Management (additional variants)
    // -----------------------------------------------------------------------

    /// PUS 8-1 variable-size: Function call with N + 32-bit argument array.
    ///
    /// `function_id` — 16-bit function identifier.
    /// `body` — variable-length list of 32-bit function arguments.
    pub fn pus_8_1_variable_size(
        function_id: u16,
        body: Vec<Pus81BodyEntry>,
    ) -> Result<PreparedCommand> {
        let n = body.len();
        ensure_count_matches(n, &body, "PUS_8_1_Body")?;
        Ok(PreparedCommand::new(
            "PUS_8_1_Variable_Size",
            json!({
                "Function_ID": function_id,
                "N": n,
                "PUS_8_1_Body": pus81_body_array_value(&body),
            }),
        )
        .with_description("PUS 8-1 variable-size function call"))
    }

    /// PUS 8-1 SYSTEM_COMPRESS_FILE: Compress a file on the spacecraft.
    ///
    /// `file_src` — path to the source file (e.g. "/flash0/hk.bin").
    pub fn pus_8_1_system_compress_file(file_src: impl Into<String>) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_8_1__SYSTEM_COMPRESS_FILE",
            json!({ "FileSrc": file_src.into() }),
        )
        .with_description("Compress file on spacecraft")
    }

    // -----------------------------------------------------------------------
    // PUS Service 20 — Parameter management
    // -----------------------------------------------------------------------

    /// PUS 20-1: Get parameter values.
    ///
    /// `parameter_ids` — list of 16-bit Parameter_ID values to read.
    pub fn pus_20_1(parameter_ids: Vec<u16>) -> Result<PreparedCommand> {
        let n = parameter_ids.len();
        ensure_count_matches(n, &parameter_ids, "Parameter_ID")?;
        Ok(PreparedCommand::new(
            "PUS_20_1",
            json!({
                "N": n,
                "Parameter_ID": u16_array_value(&parameter_ids),
            }),
        )
        .with_description("Get parameter values"))
    }

    /// PUS 20-3: Set parameter values.
    ///
    /// `entries` — list of (Parameter_ID, Value) pairs to write.
    pub fn pus_20_3(entries: Vec<Pus203Entry>) -> Result<PreparedCommand> {
        let n = entries.len();
        ensure_count_matches(n, &entries, "TC_20_3_Body")?;
        Ok(PreparedCommand::new(
            "PUS_20_3",
            json!({
                "N": n,
                "TC_20_3_Body": pus203_array_value(&entries),
            }),
        )
        .with_description("Set parameter values"))
    }

    // -----------------------------------------------------------------------
    // PUS Service 21 — Request sequencing
    // -----------------------------------------------------------------------

    /// PUS 21-1 (base): Load an inline sequence header.
    ///
    /// `sequence_id` — 16-bit sequence identifier.
    /// `n` — number of commands in the sequence (u8).
    pub fn pus_21_1_base(sequence_id: u16, n: u8) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_21_1_Base",
            json!({
                "Sequence_ID": sequence_id,
                "N": n,
            }),
        )
        .with_description("Load inline sequence header")
    }

    /// PUS 21-2: Load sequence by file reference.
    ///
    /// `sequence_id` — 16-bit sequence identifier.
    /// `repository_path` — path on the spacecraft filesystem (e.g. "/flash0").
    /// `file_name` — file name within the repository path.
    pub fn pus_21_2(
        sequence_id: u16,
        repository_path: impl Into<String>,
        file_name: impl Into<String>,
    ) -> PreparedCommand {
        PreparedCommand::new(
            "PUS_21_2",
            json!({
                "Sequence_ID": sequence_id,
                "Repository_Path": repository_path.into(),
                "File_Name": file_name.into(),
            }),
        )
        .with_description("Load sequence by file reference")
    }

    /// PUS 21-3: Unload sequence.
    pub fn pus_21_3(sequence_id: u16) -> PreparedCommand {
        PreparedCommand::new("PUS_21_3", json!({ "Sequence_ID": sequence_id }))
            .with_description("Unload sequence")
    }

    /// PUS 21-4: Activate (execute) sequence.
    pub fn pus_21_4(sequence_id: u16) -> PreparedCommand {
        PreparedCommand::new("PUS_21_4", json!({ "Sequence_ID": sequence_id }))
            .with_description("Activate sequence")
    }

    /// PUS 21-5: Hold (pause) sequence.
    pub fn pus_21_5(sequence_id: u16) -> PreparedCommand {
        PreparedCommand::new("PUS_21_5", json!({ "Sequence_ID": sequence_id }))
            .with_description("Hold (pause) sequence")
    }

    /// PUS 21-6: Resume sequence.
    pub fn pus_21_6() -> PreparedCommand {
        PreparedCommand::new("PUS_21_6", json!({})).with_description("Resume sequence")
    }

    /// PUS 21-13: Abort sequence.
    pub fn pus_21_13() -> PreparedCommand {
        PreparedCommand::new("PUS_21_13", json!({})).with_description("Abort sequence")
    }
}
