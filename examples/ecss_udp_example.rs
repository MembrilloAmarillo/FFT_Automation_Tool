use anyhow::Result;
use rust_and_vulkan::{
    AutomationConfig, AutomationEngine, CommandDefinition, Commands, HkStructureId, Pus331Entry,
    YamcsClient, YamcsConfig,
};
use serde_json::json;

fn make_config() -> YamcsConfig {
    YamcsConfig {
        base_url: std::env::var("YAMCS_URL").unwrap_or_else(|_| "http://localhost:8090".into()),
        instance: std::env::var("YAMCS_INSTANCE").unwrap_or_else(|_| "myproject".into()),
        processor: std::env::var("YAMCS_PROCESSOR").unwrap_or_else(|_| "realtime".into()),
        username: std::env::var("YAMCS_USER").unwrap_or_else(|_| "guest".into()),
        password: std::env::var("YAMCS_PASS").unwrap_or_else(|_| "guest".into()),
        origin: "rust-automation".into(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let example = args.get(1).map(String::as_str).unwrap_or("list");

    match example {
        "list" => example_list_commands().await?,
        "resolve" => example_resolve_command().await?,
        "alive" => example_are_you_alive().await?,
        "eps_on" => example_eps_channel_on().await?,
        "eps_off" => example_eps_channel_off().await?,
        "set_time" => example_set_time().await?,
        "hk_interval" => example_hk_interval().await?,
        "hk_query" => example_hk_query().await?,
        "enable_events" => example_enable_events().await?,
        "disable_events" => example_disable_events().await?,
        "dry_run" => example_dry_run().await?,
        "automation_toml" => example_automation_toml().await?,
        "automation_json" => example_automation_json().await?,
        "automation_code" => example_automation_code().await?,
        _ => {
            println!("Usage: cargo run --example send_examples -- <example_name>");
            println!();
            println!("Available examples:");
            println!("  list             - List all commands from YAMCS MDB");
            println!("  resolve          - Resolve a short command name to its qualified MDB name");
            println!("  alive            - Send PUS17.1 are-you-alive");
            println!("  eps_on           - Turn on EPS output bus channel");
            println!("  eps_off          - Turn off EPS output bus channel");
            println!("  set_time         - Update spacecraft UNIX time");
            println!("  hk_interval      - Change HK collection intervals (PUS331)");
            println!("  hk_query         - Query HK collection intervals (PUS333)");
            println!("  enable_events    - Enable event reports (PUS55)");
            println!("  disable_events   - Disable event reports (PUS56)");
            println!("  dry_run          - Dry-run a command without sending");
            println!("  automation_toml  - Run a TOML-defined automation plan");
            println!("  automation_json  - Run a JSON-defined automation plan");
            println!("  automation_code  - Run a code-defined automation plan");
        }
    }

    Ok(())
}

async fn example_list_commands() -> Result<()> {
    println!("=== Listing MDB Commands ===");
    let client = YamcsClient::new(make_config())?;
    let infos = client.list_command_infos().await?;
    println!("Found {} commands:", infos.len());
    for cmd in &infos {
        println!("  name={:?}  qualified={:?}", cmd.name, cmd.qualified_name);
    }
    Ok(())
}

async fn example_resolve_command() -> Result<()> {
    println!("=== Resolving Command Names ===");
    let client = YamcsClient::new(make_config())?;

    for name in &[
        "PUS_8_1_EPS_OUTPUT_BUS_CHANN_ELON",
        "PUS_8_1_SYSTEM_CHANGE_TIME",
        "PUS_3_31",
        "PUS_3_33",
        "PUS_5_5",
        "PUS_5_6",
    ] {
        match client.resolve_command_name(name).await {
            Ok(resolved) => println!("  {} -> {}", name, resolved),
            Err(e) => println!("  {} -> ERROR: {}", name, e),
        }
    }

    Ok(())
}

async fn example_are_you_alive() -> Result<()> {
    println!("=== PUS17.1 Are-You-Alive ===");
    let client = YamcsClient::new(make_config())?;
    let cmd = Commands::pus_17_1();
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_eps_channel_on() -> Result<()> {
    println!("=== EPS Output Bus Channel ON ===");
    let client = YamcsClient::new(make_config())?;
    let channel_id: u8 = 3;
    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(channel_id);
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_eps_channel_off() -> Result<()> {
    println!("=== EPS Output Bus Channel OFF ===");
    let client = YamcsClient::new(make_config())?;
    let channel_id: u8 = 3;
    let cmd = Commands::pus_8_1_eps_output_bus_channel_off(channel_id);
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_set_time() -> Result<()> {
    println!("=== Set Spacecraft UNIX Time ===");
    let client = YamcsClient::new(make_config())?;
    let unix_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    println!("Setting time to UNIX: {}", unix_time);
    let cmd = Commands::pus_8_1_system_change_time(unix_time);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_hk_interval() -> Result<()> {
    println!("=== PUS331 Change HK Collection Intervals ===");
    let client = YamcsClient::new(make_config())?;
    let cmd = Commands::pus_3_31(vec![
        Pus331Entry::new(HkStructureId::EpsSys, 10),
        Pus331Entry::new(HkStructureId::Transceiver, 30),
        Pus331Entry::new(HkStructureId::Adcs, 60),
    ])?;
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_hk_query() -> Result<()> {
    println!("=== PUS333 Query HK Collection Intervals ===");
    let client = YamcsClient::new(make_config())?;
    let cmd = Commands::pus_3_33(vec![HkStructureId::EpsSys])?;
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_enable_events() -> Result<()> {
    println!("=== PUS55 Enable Event Reports ===");
    let client = YamcsClient::new(make_config())?;
    let event_ids: Vec<u16> = vec![1];
    let cmd = Commands::pus_5_5(event_ids)?;
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_disable_events() -> Result<()> {
    println!("=== PUS56 Disable Event Reports ===");
    let client = YamcsClient::new(make_config())?;
    let event_ids: Vec<u16> = vec![1];
    let cmd = Commands::pus_5_6(event_ids)?;
    println!("Sending: {} with args: {}", cmd.name, cmd.args);
    let response = client.issue_command_value(&cmd.name, cmd.args).await?;
    println!("Response: {:#}", response);
    Ok(())
}

async fn example_dry_run() -> Result<()> {
    println!("=== Dry-Run Command ===");
    let client = YamcsClient::new(make_config())?;

    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(3);
    println!("Dry-running: {} with args: {}", cmd.name, cmd.args);

    let response = client.dry_run_command_value(&cmd.name, cmd.args).await?;
    println!("Dry-run response: {:#}", response);
    Ok(())
}

async fn example_automation_toml() -> Result<()> {
    println!("=== Automation from TOML ===");

    let toml_str = r#"
[yamcs]
base_url = "http://localhost:8090"
instance = "myproject"
processor = "realtime"
username = "guest"
password = "guest"
origin = "rust-automation"

timeout_ms = 30000
stop_on_error = true
repeat_count = 1
dry_run_all_first = false

[[commands]]
name = "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON"
args = { Channel_ID = 3 }
delay_ms = 500
retry_count = 2
description = "Turn on EPS channel 3"

[[commands]]
name = "PUS_8_1_SYSTEM_CHANGE_TIME"
args = { Time = 1762000000 }
delay_ms = 500
retry_count = 1
description = "Set spacecraft time"

[[commands]]
name = "PUS_3_33"
args = { N = 1, HK_Structure_ID = [5] }
delay_ms = 500
description = "Query EpsSys HK interval"
"#;

    let engine = AutomationEngine::from_toml_str(toml_str)?;
    println!(
        "Plan: {} commands, estimated duration: {} ms",
        engine.config().command_count(),
        engine.config().estimated_duration_ms()
    );
    let stats = engine.execute().await?;
    print_stats(&stats);
    Ok(())
}

async fn example_automation_json() -> Result<()> {
    println!("=== Automation from JSON ===");

    let json_str = r#"{
  "yamcs": {
    "base_url": "http://localhost:8090",
    "instance": "myproject",
    "processor": "realtime",
    "username": "guest",
    "password": "guest",
    "origin": "rust-automation"
  },
  "timeout_ms": 20000,
  "stop_on_error": true,
  "repeat_count": 1,
  "dry_run_all_first": false,
  "commands": [
    {
      "name": "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON",
      "args": { "Channel_ID": 5 },
      "delay_ms": 300,
      "retry_count": 1,
      "description": "Turn on EPS channel 5"
    },
    {
      "name": "PUS_5_5",
      "args": { "N": 1, "Event_ID": [1] },
      "delay_ms": 300,
      "description": "Enable event reports 1 and 2"
    }
  ]
}"#;

    let engine = AutomationEngine::from_json_str(json_str)?;
    println!(
        "Plan: {} commands, estimated duration: {} ms",
        engine.config().command_count(),
        engine.config().estimated_duration_ms()
    );
    let stats = engine.execute().await?;
    print_stats(&stats);
    Ok(())
}

async fn example_automation_code() -> Result<()> {
    println!("=== Automation from Code ===");

    let cfg = make_config();

    let hk_cmd = Commands::pus_3_31(vec![
        Pus331Entry::new(HkStructureId::EpsSys, 10),
        Pus331Entry::new(HkStructureId::Transceiver, 30),
    ])?;

    let hk_query = Commands::pus_3_33(vec![HkStructureId::EpsSys, HkStructureId::Transceiver])?;

    let plan = AutomationConfig {
        yamcs: cfg,
        timeout_ms: 30_000,
        stop_on_error: true,
        repeat_count: 1,
        dry_run_all_first: false,
        commands: vec![
            CommandDefinition::new("PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON")
                .with_args(json!({ "Channel_ID": 3 }))
                .with_delay(500)
                .with_retry(2)
                .with_description("Turn on EPS channel 3"),
            CommandDefinition::new("PUS_8_1_SYSTEM_CHANGE_TIME")
                .with_args(json!({ "Time": current_unix_time() }))
                .with_delay(500)
                .with_retry(1)
                .with_description("Sync spacecraft time"),
            CommandDefinition::new(&hk_cmd.name)
                .with_args(hk_cmd.args)
                .with_delay(500)
                .with_description("Update HK collection intervals"),
            CommandDefinition::new(&hk_query.name)
                .with_args(hk_query.args)
                .with_delay(500)
                .verify_packet("HK")
                .with_verify_timeout(5_000)
                .with_verify_poll_interval(500)
                .with_verify_packet_limit(20)
                .with_description("Query HK intervals with verification"),
        ],
    };

    println!(
        "Plan: {} commands, estimated duration: {} ms",
        plan.command_count(),
        plan.estimated_duration_ms()
    );

    let engine = AutomationEngine::new(plan)?;
    let stats = engine.execute().await?;
    print_stats(&stats);
    Ok(())
}

fn current_unix_time() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

fn print_stats(stats: &rust_and_vulkan::ExecutionStats) {
    println!("--- Execution Stats ---");
    println!(
        "  successful: {}  failed: {}  elapsed: {} ms  success_rate: {:.1}%",
        stats.successful,
        stats.failed,
        stats.elapsed_ms,
        stats.success_rate()
    );
    if !stats.command_times.is_empty() {
        println!("  per-command timing:");
        let mut times: Vec<(&String, &u64)> = stats.command_times.iter().collect();
        times.sort_by_key(|(name, _)| name.as_str());
        for (name, ms) in times {
            println!("    {}: {} ms", name, ms);
        }
    }
}
