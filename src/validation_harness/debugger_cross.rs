use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::command::{
    ValidationError, ValidationReport, assert_array_contains_str, assert_eq_bool, assert_eq_str,
    default_out_dir, repo_root, run_cargo_json, string_at, value_at,
};

pub fn run_debugger_cross_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("debugger-cross-validation"));
    fs::create_dir_all(&out_dir)?;

    let mut checks = Vec::new();
    check_http_request(&out_dir)?;
    checks.push("http-request".to_string());

    check_http_connect_denied(&out_dir)?;
    checks.push("http-connect-denied".to_string());

    check_socks5_auth_connect_denied(&out_dir)?;
    checks.push("socks5-auth-connect-denied".to_string());

    check_invalid_gewylang(&out_dir)?;
    checks.push("invalid-gewy".to_string());

    write_readme(&out_dir)?;

    Ok(ValidationReport {
        name: "debugger cross validation".to_string(),
        out_dir,
        checks,
    })
}

fn check_http_request(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    let dsl = repo_root().join("dsl").join("http_request_path.gewy");
    let summary = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--json",
            "--summary-only",
        ],
        out_dir.join("http-request.summary.json"),
    )?;
    let console = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--debugger-console",
            "--json",
        ],
        out_dir.join("http-request.console.json"),
    )?;
    let envelope = run_gewyc_envelope(&dsl, out_dir.join("http-request.envelope.json"))?;

    assert_eq_str(&summary, &["primary_module_family"], "request-response")?;
    assert_eq_str(&summary, &["primary_failure_mode"], "none")?;
    assert_eq_str(&summary, &["operator_guidance_action"], "manual_review")?;
    assert_eq_str(&console, &["recommended_focus", "status"], "healthy")?;
    assert_fields_match(&summary, &console, "primary_module_family")?;
    assert_fields_match(&summary, &console, "primary_failure_mode")?;
    assert_fields_match(&summary, &console, "operator_guidance_action")?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "parse_ok"],
        true,
    )?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "validation_ok"],
        true,
    )?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "diagnostics_ok"],
        true,
    )
}

fn check_http_connect_denied(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    let dsl = repo_root()
        .join("dsl")
        .join("http_connect_denied_path.gewy");
    let summary = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--json",
            "--summary-only",
        ],
        out_dir.join("http-connect-denied.summary.json"),
    )?;
    let console = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--debugger-console",
            "--json",
        ],
        out_dir.join("http-connect-denied.console.json"),
    )?;

    assert_eq_str(&summary, &["primary_module_family"], "relay")?;
    assert_eq_str(&summary, &["primary_failure_basis"], "missing_transition")?;
    assert_eq_str(
        &summary,
        &["operator_guidance_action"],
        "collect_more_runtime_evidence",
    )?;
    assert_array_contains_str(
        &summary,
        &["missing_transitions"],
        "resolve_upstream->connect",
    )?;
    assert_eq_str(&console, &["recommended_focus", "status"], "attention")?;
    assert_eq_str(
        &console,
        &["recommended_focus", "first_missing_transition"],
        "resolve_upstream->connect",
    )?;
    assert_fields_match(&summary, &console, "operator_guidance_action")?;

    if string_at(&console, &["recommended_focus", "operator_guidance_action"])? == "manual_review" {
        return Err(ValidationError::new(
            "negative CONNECT validation incorrectly requested manual_review",
        ));
    }
    Ok(())
}

fn check_socks5_auth_connect_denied(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    let summary = run_gewyvern_json(
        &[
            "--protocol",
            "socks5",
            "--entry",
            "auth-connect-denied",
            "--json",
            "--summary-only",
        ],
        out_dir.join("socks5-auth-connect-denied.summary.json"),
    )?;
    let console = run_gewyvern_json(
        &[
            "--protocol",
            "socks5",
            "--entry",
            "auth-connect-denied",
            "--debugger-console",
            "--json",
        ],
        out_dir.join("socks5-auth-connect-denied.console.json"),
    )?;

    assert_eq_str(&summary, &["primary_module_kind"], "proxy_negotiation")?;
    assert_eq_str(&summary, &["primary_failure_basis"], "missing_transition")?;
    assert_eq_str(
        &summary,
        &["operator_guidance_action"],
        "collect_more_runtime_evidence",
    )?;
    assert_eq_str(&console, &["recommended_focus", "status"], "attention")?;
    assert_fields_match(&summary, &console, "primary_failure_mode")?;
    assert_fields_match(&summary, &console, "operator_guidance_action")?;
    assert_eq_str(
        &console,
        &["recommended_focus", "first_missing_transition"],
        "resolve_upstream->connect",
    )
}

fn check_invalid_gewylang(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    let bad_dir = out_dir.join("tmp-invalid");
    fs::create_dir_all(&bad_dir)?;
    let bad_dsl = bad_dir.join("invalid.gewy");
    fs::write(&bad_dsl, "this is not valid gewylang ???\n")?;

    let envelope = run_gewyc_envelope(&bad_dsl, out_dir.join("invalid-gewy.envelope.json"))?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "parse_ok"],
        false,
    )?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "validation_ok"],
        false,
    )?;
    assert_eq_bool(
        &envelope,
        &["payload", "stages", "status", "diagnostics_ok"],
        false,
    )?;

    let findings = value_at(&envelope, &["payload", "findings", "findings"])?
        .as_array()
        .ok_or_else(|| ValidationError::new("expected invalid Gewylang findings array"))?;
    let first = findings
        .first()
        .ok_or_else(|| ValidationError::new("invalid Gewylang did not emit a finding"))?;
    assert_eq_str(first, &["stage"], "parse")?;
    assert_eq_str(first, &["severity"], "error")?;

    fs::remove_dir_all(&bad_dir)?;
    Ok(())
}

fn run_gewyvern_json(args: &[&str], output_path: PathBuf) -> Result<Value, ValidationError> {
    let mut cargo_args = vec!["run".to_string(), "--quiet".to_string(), "--".to_string()];
    cargo_args.extend(args.iter().map(|arg| (*arg).to_string()));
    run_cargo_json(&cargo_args, &output_path)
}

fn run_gewyc_envelope(
    dsl_path: &std::path::Path,
    output_path: PathBuf,
) -> Result<Value, ValidationError> {
    let cargo_args = vec![
        "run".to_string(),
        "--quiet".to_string(),
        "-p".to_string(),
        "gewyc".to_string(),
        "--".to_string(),
        "envelope".to_string(),
        dsl_path.display().to_string(),
        "--json".to_string(),
    ];
    run_cargo_json(&cargo_args, &output_path)
}

fn assert_fields_match(
    summary: &Value,
    console: &Value,
    field: &str,
) -> Result<(), ValidationError> {
    let expected = string_at(summary, &[field])?;
    let actual = string_at(console, &["recommended_focus", field])?;
    if actual != expected {
        return Err(ValidationError::new(format!(
            "console recommended_focus.{field} `{actual}` did not match summary `{expected}`"
        )));
    }
    Ok(())
}

fn write_readme(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("README.txt"),
        "gewyvern native debugger cross validation\n\
         ==========================================\n\n\
         This bundle was produced by `gewyvern_validate debugger-cross`.\n\n\
         Surfaces compared:\n\
         - gewyvern summary JSON\n\
         - local debugger console JSON\n\
         - gewyc envelope JSON\n\n\
         Positive case:\n\
         - http-request\n\n\
         Negative cases:\n\
         - http-connect-denied\n\
         - socks5-auth-connect-denied\n\
         - invalid-gewy\n\n\
         Protocol-negative cases must stay in attention / collect-more-evidence posture.\n\
         Invalid Gewylang must fail parsing before validation or diagnostics claim success.\n",
    )?;
    Ok(())
}
