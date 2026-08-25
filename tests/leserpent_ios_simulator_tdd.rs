use serde_json::Value;
use std::fs;

fn proof() -> Value {
    let path = "docs/fixtures/leserpent_ios26_simulator_macos_arm64_20260821.json";
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
    serde_json::from_str(&source).unwrap_or_else(|error| panic!("failed to parse {path}: {error}"))
}

#[test]
fn ios_simulator_proof_is_non_vacuous_and_fail_closed() {
    let proof = proof();
    assert_eq!(proof["schema_version"], 1);
    assert_eq!(proof["proof"], "leserpent-ios26-simulator");
    assert_eq!(proof["result"], "passed");
    assert_eq!(proof["toolchain"]["dotnet_sdk"], "10.0.300");
    assert_eq!(proof["toolchain"]["simulator_runtime"], "iOS 26.5");

    let package = &proof["package"];
    assert_eq!(package["application_id"], "org.gewyvern.leserpent");
    assert_eq!(package["version"], "1.16.0");
    assert_eq!(package["minimum_ios"], "15.0");
    assert_eq!(package["device_families"].as_array().unwrap().len(), 2);
    for artifact in ["debug_simulator_app", "release_device_app"] {
        assert!(package[artifact]["allocated_kib"].as_u64().unwrap() > 10_000);
        assert!(package[artifact]["executable_bytes"].as_u64().unwrap() > 1_000_000);
        assert_eq!(
            package[artifact]["executable_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert_eq!(package[artifact]["architecture"], "arm64");
    }
    let release = &package["release_device_app"];
    assert_eq!(release["aot_data_present"], true);
    assert_eq!(release["trim_mode"], "full");
    assert_eq!(release["il_stripping"], true);
    assert_eq!(release["debug_proof_switches_absent"], true);
    assert_eq!(release["production_signed"], false);

    let runtime = &proof["simulator_runtime"];
    assert_eq!(runtime["phone"]["cold_launch_succeeded"], true);
    assert_eq!(runtime["phone"]["cold_relaunch_new_process"], true);
    assert_eq!(runtime["phone"]["hot_resume_process_stable"], true);
    assert_eq!(
        runtime["phone"]["active_ui_restored_after_hot_resume"],
        true
    );
    assert_eq!(
        runtime["phone"]["hot_resume_screenshot_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(runtime["tablet"]["cold_launch_succeeded"], true);
    assert_eq!(runtime["app_fault_log_entries"], 0);

    let keychain = &proof["keychain"];
    assert_eq!(
        keychain["native_argument_source"],
        "NSProcessInfo.arguments"
    );
    assert_eq!(keychain["keychain_round_trip"], true);
    assert_eq!(keychain["endpoint_opaque_alias"], true);
    assert_eq!(keychain["delete_verified"], true);
    assert_eq!(keychain["sensitive_values_retained"], false);

    let matrix = proof["layout_matrix"].as_array().unwrap();
    assert_eq!(matrix.len(), 3);
    for fixture in matrix {
        assert_eq!(fixture["initial_content_visible"], true);
        assert_eq!(fixture["overlap_detected"], false);
        assert_eq!(fixture["screenshot_sha256"].as_str().unwrap().len(), 64);
    }
    assert!(matrix.iter().any(|fixture| {
        fixture["content_size"] == "accessibility-extra-extra-extra-large"
            && fixture["vertical_reflow"] == true
            && fixture["scroll_container_retained"] == true
            && fixture["controls_reachable_by_scroll"] == true
    }));
    assert!(
        matrix
            .iter()
            .any(|fixture| { fixture["width_class"] == "expanded" && fixture["two_pane"] == true })
    );

    let native = &proof["native_ui_document"];
    assert_eq!(native["renderer_neutral_projection"], true);
    assert_eq!(native["mounted_on_first_render"], true);
    assert_eq!(native["native_uikit_controls"], true);
    assert_eq!(native["heartbeat_stable_render_gate"], true);
    assert_eq!(proof["security"]["debug_probe_release_stripped"], true);
    assert!(!proof["remaining"].as_array().unwrap().is_empty());
}
