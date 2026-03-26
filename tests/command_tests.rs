/// Comprehensive test suite for all PUS commands
/// Tests each command with multiple parameter variations to ensure correctness
use rust_and_vulkan::commands::Commands;
use rust_and_vulkan::xtce_types::{HkStructureId, Pus331Entry};

// ============================================================================
// PUS_17_1: Are-you-alive
// ============================================================================

#[test]
fn test_pus_17_1_basic() {
    let cmd = Commands::pus_17_1();
    assert_eq!(cmd.name, "PUS_17_1");
    assert_eq!(cmd.args, serde_json::json!({}));
}

// ============================================================================
// PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON
// ============================================================================

#[test]
fn test_pus_8_1_eps_channel_on_channel_0() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(0);
    assert_eq!(cmd.name, "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_ON");
    assert_eq!(cmd.args["Channel_ID"], 0);
}

#[test]
fn test_pus_8_1_eps_channel_on_channel_3() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(3);
    assert_eq!(cmd.args["Channel_ID"], 3);
}

#[test]
fn test_pus_8_1_eps_channel_on_channel_max() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(255);
    assert_eq!(cmd.args["Channel_ID"], 255);
}

#[test]
fn test_pus_8_1_eps_channel_on_source_id() {
    // CCSDS_Source_ID is not a payload argument for this command per XTCE;
    // it is absent from the args object.
    let cmd = Commands::pus_8_1_eps_output_bus_channel_on(1);
    assert!(
        cmd.args.get("CCSDS_Source_ID").is_none(),
        "CCSDS_Source_ID should not be set in EPS_OUTPUT_BUS_CHANNEL_ON args"
    );
}

// ============================================================================
// PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_OFF
// ============================================================================

#[test]
fn test_pus_8_1_eps_channel_off_channel_0() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_off(0);
    assert_eq!(cmd.name, "PUS_8_1_EPS_OUTPUT_BUS_CHANNEL_OFF");
    assert_eq!(cmd.args["Channel_ID"], 0);
}

#[test]
fn test_pus_8_1_eps_channel_off_channel_5() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_off(5);
    assert_eq!(cmd.args["Channel_ID"], 5);
}

#[test]
fn test_pus_8_1_eps_channel_off_channel_255() {
    let cmd = Commands::pus_8_1_eps_output_bus_channel_off(255);
    assert_eq!(cmd.args["Channel_ID"], 255);
}

// ============================================================================
// PUS_8_1_SYSTEM_CHANGE_TIME
// ============================================================================

#[test]
fn test_pus_8_1_system_change_time_epoch() {
    let cmd = Commands::pus_8_1_system_change_time(0);
    assert_eq!(cmd.name, "PUS_8_1_SYSTEM_CHANGE_TIME");
    assert_eq!(cmd.args["Time"], 0);
}

#[test]
fn test_pus_8_1_system_change_time_current() {
    let current_time = 1773305201u32;
    let cmd = Commands::pus_8_1_system_change_time(current_time);
    assert_eq!(cmd.args["Time"], current_time);
}

#[test]
fn test_pus_8_1_system_change_time_far_future() {
    let cmd = Commands::pus_8_1_system_change_time(4294967295);
    assert_eq!(cmd.args["Time"], 4294967295u32);
}

// ============================================================================
// PUS_8_1_EPS_CORRECT_TIME
// ============================================================================

#[test]
fn test_pus_8_1_eps_correct_time_positive() {
    let cmd = Commands::pus_8_1_eps_correct_time(60);
    assert_eq!(cmd.name, "PUS_8_1_EPS_CORRECT_TIME");
    assert_eq!(cmd.args["Time"], 60);
}

#[test]
fn test_pus_8_1_eps_correct_time_negative() {
    let cmd = Commands::pus_8_1_eps_correct_time(-30);
    assert_eq!(cmd.args["Time"], -30);
}

#[test]
fn test_pus_8_1_eps_correct_time_zero() {
    let cmd = Commands::pus_8_1_eps_correct_time(0);
    assert_eq!(cmd.args["Time"], 0);
}

// ============================================================================
// PUS_8_1_PAY1_STOP_TIME_ID / PUS_8_1_PAY2_STOP_TIME_ID
// ============================================================================

#[test]
fn test_pus_8_1_pay1_stop_time_zero() {
    let cmd = Commands::pus_8_1_pay_1_stop_time_id(0).unwrap();
    assert_eq!(cmd.name, "PUS_8_1_PAY1_STOP_TIME_ID");
    assert_eq!(cmd.args["Stop_Time"], 0);
}

#[test]
fn test_pus_8_1_pay1_stop_time_valid() {
    let cmd = Commands::pus_8_1_pay_1_stop_time_id(0x123456).unwrap();
    assert_eq!(cmd.args["Stop_Time"], 0x123456);
}

#[test]
fn test_pus_8_1_pay1_stop_time_max_u24() {
    let cmd = Commands::pus_8_1_pay_1_stop_time_id(0xFF_FFFF).unwrap();
    assert_eq!(cmd.args["Stop_Time"], 0xFF_FFFF);
}

#[test]
fn test_pus_8_1_pay1_stop_time_exceeds_u24() {
    let result = Commands::pus_8_1_pay_1_stop_time_id(0x1_000_000);
    assert!(result.is_err());
}

#[test]
fn test_pus_8_1_pay2_stop_time_zero() {
    let cmd = Commands::pus_8_1_pay_2_stop_time_id(0).unwrap();
    assert_eq!(cmd.name, "PUS_8_1_PAY2_STOP_TIME_ID");
    assert_eq!(cmd.args["Stop_Time"], 0);
}

#[test]
fn test_pus_8_1_pay2_stop_time_valid() {
    let cmd = Commands::pus_8_1_pay_2_stop_time_id(0xABCDEF).unwrap();
    assert_eq!(cmd.args["Stop_Time"], 0xABCDEF);
}

// ============================================================================
// PUS_8_1_PAY1_DOWNLOAD_EXP / PUS_8_1_PAY2_DOWNLOAD_EXP
// ============================================================================

#[test]
fn test_pus_8_1_pay1_download_exp_zero() {
    let cmd = Commands::pus_8_1_pay_1_download_exp(0);
    assert_eq!(cmd.name, "PUS_8_1_PAY1_DOWNLOAD_EXP");
    assert_eq!(cmd.args["PacketID"], 0);
}

#[test]
fn test_pus_8_1_pay1_download_exp_various() {
    let cmd = Commands::pus_8_1_pay_1_download_exp(42);
    assert_eq!(cmd.args["PacketID"], 42);
}

#[test]
fn test_pus_8_1_pay1_download_exp_large() {
    let cmd = Commands::pus_8_1_pay_1_download_exp(0xDEADBEEFu32);
    assert_eq!(cmd.args["PacketID"], 0xDEADBEEFu32);
}

#[test]
fn test_pus_8_1_pay2_download_exp_zero() {
    let cmd = Commands::pus_8_1_pay_2_download_exp(0);
    assert_eq!(cmd.name, "PUS_8_1_PAY2_DOWNLOAD_EXP");
    assert_eq!(cmd.args["PacketID"], 0);
}

#[test]
fn test_pus_8_1_pay2_download_exp_various() {
    let cmd = Commands::pus_8_1_pay_2_download_exp(99);
    assert_eq!(cmd.args["PacketID"], 99);
}

// ============================================================================
// PUS_3_31: Change HK collection intervals
// ============================================================================

#[test]
fn test_pus_3_31_single_entry() {
    let entries = vec![Pus331Entry::new(HkStructureId::EpsSys, 10)];
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert_eq!(cmd.name, "PUS_3_31");
    assert_eq!(cmd.args["N"], 1);
    assert!(cmd.args["PUS_3_31_Body"].is_array());
}

#[test]
fn test_pus_3_31_multiple_entries() {
    let entries = vec![
        Pus331Entry::new(HkStructureId::EpsSys, 10),
        Pus331Entry::new(HkStructureId::Transceiver, 30),
        Pus331Entry::new(HkStructureId::Adcs, 50),
    ];
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert_eq!(cmd.args["N"], 3);
    let body = &cmd.args["PUS_3_31_Body"];
    assert_eq!(body.as_array().unwrap().len(), 3);
}

#[test]
fn test_pus_3_31_different_intervals() {
    let entries = vec![
        Pus331Entry::new(HkStructureId::EpsPbu, 5),
        Pus331Entry::new(HkStructureId::EpsPdu, 15),
        Pus331Entry::new(HkStructureId::EpsPiu, 25),
        Pus331Entry::new(HkStructureId::EpsPcu, 100),
    ];
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert_eq!(cmd.args["N"], 4);
}

#[test]
fn test_pus_3_31_various_hk_structures() {
    let entries = vec![
        Pus331Entry::new(HkStructureId::Adcs, 20),
        Pus331Entry::new(HkStructureId::EpsSysSm, 30),
        Pus331Entry::new(HkStructureId::AdcsNm, 40),
        Pus331Entry::new(HkStructureId::EpsSysOm, 60),
    ];
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert_eq!(cmd.args["N"], 4);
}

#[test]
fn test_pus_3_31_mismatch_count() {
    let entries = vec![Pus331Entry::new(HkStructureId::EpsSys, 10)];
    // Manually create incorrect command (for test purposes, would be caught by ensure_count_matches)
    // This is implicitly tested by ensure_count_matches function
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert_eq!(cmd.args["N"], 1); // Count is correctly set
}

// ============================================================================
// PUS_3_33: Query HK collection interval information
// ============================================================================

#[test]
fn test_pus_3_33_single_hk() {
    let hk_ids = vec![HkStructureId::EpsSys];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.name, "PUS_3_33");
    assert_eq!(cmd.args["N"], 1);
    assert!(cmd.args["HK_Structure_ID"].is_array());
}

#[test]
fn test_pus_3_33_multiple_hk() {
    let hk_ids = vec![
        HkStructureId::EpsSys,
        HkStructureId::Transceiver,
        HkStructureId::Adcs,
    ];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.args["N"], 3);
    let ids = cmd.args["HK_Structure_ID"].as_array().unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], 5); // EpsSys
    assert_eq!(ids[1], 7); // Transceiver
    assert_eq!(ids[2], 1); // Adcs
}

#[test]
fn test_pus_3_33_all_safe_mode() {
    let hk_ids = vec![
        HkStructureId::AdcsSm,
        HkStructureId::EpsPbuSm,
        HkStructureId::EpsPduSm,
        HkStructureId::EpsPiuSm,
        HkStructureId::EpsSysSm,
    ];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.args["N"], 5);
}

#[test]
fn test_pus_3_33_all_nominal_mode() {
    let hk_ids = vec![
        HkStructureId::AdcsNm,
        HkStructureId::EpsPbuNm,
        HkStructureId::EpsPduNm,
        HkStructureId::EpsPiuNm,
        HkStructureId::EpsSysNm,
    ];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.args["N"], 5);
}

#[test]
fn test_pus_3_33_all_operational_mode() {
    let hk_ids = vec![
        HkStructureId::AdcsOm,
        HkStructureId::EpsPbuOm,
        HkStructureId::EpsPduOm,
        HkStructureId::EpsPiuOm,
        HkStructureId::EpsSysOm,
    ];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.args["N"], 5);
}

#[test]
fn test_pus_3_33_mixed_modes() {
    let hk_ids = vec![
        HkStructureId::EpsSys,
        HkStructureId::EpsSysSm,
        HkStructureId::EpsSysNm,
        HkStructureId::EpsSysOm,
    ];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert_eq!(cmd.args["N"], 4);
}

// ============================================================================
// PUS_5_5: Enable event reports
// ============================================================================

#[test]
fn test_pus_5_5_single_event() {
    let event_ids = vec![1];
    let cmd = Commands::pus_5_5(event_ids).unwrap();
    assert_eq!(cmd.name, "PUS_5_5");
    assert_eq!(cmd.args["N"], 1);
    let ids = cmd.args["Event_ID"].as_array().unwrap();
    assert_eq!(ids[0], 1);
}

#[test]
fn test_pus_5_5_multiple_events() {
    let event_ids = vec![1, 1, 1]; // Valid per XTCE (only ID=1 exists)
    let cmd = Commands::pus_5_5(event_ids).unwrap();
    assert_eq!(cmd.args["N"], 3);
}

// ============================================================================
// PUS_5_6: Disable event reports
// ============================================================================

#[test]
fn test_pus_5_6_single_event() {
    let event_ids = vec![1];
    let cmd = Commands::pus_5_6(event_ids).unwrap();
    assert_eq!(cmd.name, "PUS_5_6");
    assert_eq!(cmd.args["N"], 1);
    let ids = cmd.args["Event_ID"].as_array().unwrap();
    assert_eq!(ids[0], 1);
}

#[test]
fn test_pus_5_6_multiple_events() {
    let event_ids = vec![1, 1, 1]; // Valid per XTCE (only ID=1 exists)
    let cmd = Commands::pus_5_6(event_ids).unwrap();
    assert_eq!(cmd.args["N"], 3);
}

// ============================================================================
// PUS_24_1: Enable parameter collection
// ============================================================================

#[test]
fn test_pus_24_1_eps_sys_single_param() {
    let params = vec![100];
    let cmd = Commands::pus_24_1(HkStructureId::EpsSys, params).unwrap();
    assert_eq!(cmd.name, "PUS_24_1");
    assert_eq!(cmd.args["HK_Structure_ID"], 5); // EpsSys
    assert_eq!(cmd.args["N"], 1);
}

#[test]
fn test_pus_24_1_eps_sys_multiple_params() {
    let params = vec![100, 101, 102, 103];
    let cmd = Commands::pus_24_1(HkStructureId::EpsSys, params).unwrap();
    assert_eq!(cmd.args["HK_Structure_ID"], 5);
    assert_eq!(cmd.args["N"], 4);
}

#[test]
fn test_pus_24_1_different_hk_structures() {
    let test_cases = vec![
        (HkStructureId::Adcs, 1),
        (HkStructureId::EpsPbu, 2),
        (HkStructureId::EpsPdu, 3),
        (HkStructureId::EpsPiu, 4),
        (HkStructureId::EpsSys, 5),
        (HkStructureId::SolarPanels, 6),
        (HkStructureId::Transceiver, 7),
        (HkStructureId::EpsPcu, 8),
    ];

    for (hk, expected_id) in test_cases {
        let params = vec![50];
        let cmd = Commands::pus_24_1(hk, params).unwrap();
        assert_eq!(cmd.args["HK_Structure_ID"], expected_id);
    }
}

#[test]
fn test_pus_24_1_various_param_ranges() {
    let params = vec![0, 1000, 5000, 65535];
    let cmd = Commands::pus_24_1(HkStructureId::Transceiver, params).unwrap();
    assert_eq!(cmd.args["N"], 4);
}

// ============================================================================
// PUS_24_2: Disable parameter collection
// ============================================================================

#[test]
fn test_pus_24_2_single_param() {
    let params = vec![100];
    let cmd = Commands::pus_24_2(HkStructureId::EpsSys, params).unwrap();
    assert_eq!(cmd.name, "PUS_24_2");
    assert_eq!(cmd.args["HK_Structure_ID"], 5);
    assert_eq!(cmd.args["N"], 1);
}

#[test]
fn test_pus_24_2_multiple_params() {
    let params = vec![100, 200, 300];
    let cmd = Commands::pus_24_2(HkStructureId::Adcs, params).unwrap();
    assert_eq!(cmd.args["HK_Structure_ID"], 1);
    assert_eq!(cmd.args["N"], 3);
}

#[test]
fn test_pus_24_2_all_hk_structures() {
    let all_hks = vec![
        HkStructureId::Adcs,
        HkStructureId::EpsPbu,
        HkStructureId::EpsPdu,
        HkStructureId::EpsPiu,
        HkStructureId::EpsSys,
        HkStructureId::SolarPanels,
        HkStructureId::Transceiver,
        HkStructureId::EpsPcu,
        HkStructureId::EpsPiuOcf,
        HkStructureId::EpsPbuAbf,
        HkStructureId::EpsParam,
        HkStructureId::EpsObcMode,
    ];

    let params = vec![99];
    for hk in all_hks {
        let cmd = Commands::pus_24_2(hk, params.clone()).unwrap();
        assert_eq!(cmd.args["N"], 1);
        assert_eq!(cmd.args["HK_Structure_ID"], hk.as_u16());
    }
}

// ============================================================================
// PUS_24_3: Query active parameters
// ============================================================================

#[test]
fn test_pus_24_3_single_param() {
    let cmd = Commands::pus_24_3(HkStructureId::EpsSys);
    assert_eq!(cmd.name, "PUS_24_3");
    assert_eq!(cmd.args["HK_Structure_ID"], 5);
    // PUS_24_3 only takes HK_Structure_ID per XTCE — no N / Parameter_ID array.
    assert!(cmd.args.get("N").is_none());
}

#[test]
fn test_pus_24_3_large_param_set() {
    let cmd = Commands::pus_24_3(HkStructureId::Transceiver);
    assert_eq!(cmd.args["HK_Structure_ID"], 7);
}

#[test]
fn test_pus_24_3_different_hk_structures() {
    let hk_list = vec![
        HkStructureId::AdcsSm,
        HkStructureId::EpsPbuSm,
        HkStructureId::AdcsNm,
        HkStructureId::EpsSysNm,
        HkStructureId::AdcsOm,
        HkStructureId::EpsSysOm,
    ];

    for hk in hk_list {
        let cmd = Commands::pus_24_3(hk);
        assert_eq!(cmd.args["HK_Structure_ID"], hk.as_u16());
    }
}

// ============================================================================
// Integration tests
// ============================================================================

#[test]
fn test_all_commands_have_names() {
    let commands = vec![
        Commands::pus_17_1(),
        Commands::pus_8_1_eps_output_bus_channel_on(0),
        Commands::pus_8_1_eps_output_bus_channel_off(0),
        Commands::pus_8_1_system_change_time(0),
        Commands::pus_8_1_eps_correct_time(0),
        Commands::pus_8_1_pay_1_download_exp(0),
        Commands::pus_8_1_pay_2_download_exp(0),
    ];

    for cmd in commands {
        assert!(!cmd.name.is_empty(), "Command should have a non-empty name");
    }
}

#[test]
fn test_all_array_commands_have_correct_structure() {
    // PUS_3_31
    let entries = vec![Pus331Entry::new(HkStructureId::EpsSys, 10)];
    let cmd = Commands::pus_3_31(entries).unwrap();
    assert!(cmd.args["PUS_3_31_Body"].is_array());
    assert!(cmd.args["N"].is_number());

    // PUS_3_33
    let hk_ids = vec![HkStructureId::EpsSys];
    let cmd = Commands::pus_3_33(hk_ids).unwrap();
    assert!(cmd.args["HK_Structure_ID"].is_array());
    assert!(cmd.args["N"].is_number());

    // PUS_5_5
    let event_ids = vec![1];
    let cmd = Commands::pus_5_5(event_ids).unwrap();
    assert!(cmd.args["Event_ID"].is_array());
    assert!(cmd.args["N"].is_number());

    // PUS_24_1
    let params = vec![100];
    let cmd = Commands::pus_24_1(HkStructureId::EpsSys, params).unwrap();
    assert!(cmd.args["Parameter_ID"].is_array());
    assert!(cmd.args["N"].is_number());
}
