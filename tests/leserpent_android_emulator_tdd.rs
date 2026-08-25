use serde_json::Value;
use std::fs;

fn proof() -> Value {
    let path = "docs/fixtures/leserpent_android_api36_emulator_macos_arm64_20260821.json";
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
    serde_json::from_str(&source).unwrap_or_else(|error| panic!("failed to parse {path}: {error}"))
}

#[test]
fn android_emulator_proof_is_non_vacuous_and_fail_closed() {
    let proof = proof();
    assert_eq!(proof["schema_version"], 1);
    assert_eq!(proof["proof"], "leserpent-android-api36-emulator");
    assert_eq!(proof["result"], "passed");
    assert_eq!(proof["toolchain"]["android_compile_sdk"], 36);
    assert_eq!(proof["emulator_runtime"]["api"], 36);
    assert_eq!(proof["emulator_runtime"]["abi"], "arm64-v8a");
    assert_eq!(proof["emulator_runtime"]["fatal_log_entries"], 0);
    assert!(
        proof["emulator_runtime"]["release_cold_start_ms"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        proof["emulator_runtime"]["release_hot_resume_ms"]
            .as_u64()
            .unwrap()
            > 0
    );

    let package = &proof["package"];
    assert_eq!(package["application_id"], "org.gewyvern.leserpent");
    assert_eq!(package["version"], "1.16.0");
    assert_eq!(package["release_aot"], true);
    assert_eq!(package["production_signed"], false);
    assert_eq!(package["release_apk"]["archive_valid"], true);
    assert_eq!(package["release_aab"]["archive_valid"], true);
    assert_eq!(
        package["release_aab"]["aot_abis"].as_array().unwrap().len(),
        2
    );
    for artifact in ["release_apk", "release_aab"] {
        assert!(package[artifact]["bytes"].as_u64().unwrap() > 1_000_000);
        assert_eq!(package[artifact]["sha256"].as_str().unwrap().len(), 64);
    }

    let matrix = proof["layout_matrix"].as_array().unwrap();
    assert_eq!(matrix.len(), 5);
    for fixture in matrix {
        assert_eq!(fixture["controls_visible"], true);
        assert_eq!(fixture["overlap_detected"], false);
    }
    assert!(
        matrix
            .iter()
            .any(|fixture| { fixture["width_class"] == "expanded" && fixture["two_pane"] == true })
    );
    assert!(matrix.iter().any(|fixture| {
        fixture["fixture"] == "short-landscape" && fixture["two_pane"] == false
    }));
    assert!(matrix.iter().any(|fixture| fixture["font_scale"] == 1.5));

    let native = &proof["native_ui_document"];
    assert_eq!(native["renderer_neutral_projection"], true);
    assert_eq!(native["mounted_on_first_render"], true);
    assert_eq!(native["fleet_heading"], "Remote runtimes");
    assert_eq!(
        native["empty_projection"],
        "No runtime projection available."
    );
    assert_eq!(native["short_landscape_visible_after_setup_collapse"], true);
    for guarantee in [
        "immutable_binding",
        "parameterized_form_event_routing",
        "typed_workspace_query",
        "typed_deployment",
        "operation_generation_fence",
    ] {
        assert_eq!(native["host_conformance"][guarantee], true);
    }

    assert_eq!(proof["ime"]["primary_action_visible_above_ime"], true);
    assert_eq!(proof["security"]["release_flag_secure"], true);
    assert_eq!(proof["security"]["release_screen_capture_blocked"], true);
    assert_eq!(
        proof["security"]["release_ui_capture_rejected_by_build"],
        true
    );
    assert_eq!(proof["security"]["operator_error_allowlist"], true);
    let checks = proof["checks"].as_array().unwrap();
    for required in [
        "renderer-neutral-native-controls",
        "shared-ui-document-first-render",
        "native-parameterized-form-event-routing",
        "operator-error-allowlist",
    ] {
        assert!(
            checks.iter().any(|check| check == required),
            "Android proof lost {required}"
        );
    }
    assert!(!proof["remaining"].as_array().unwrap().is_empty());
}
