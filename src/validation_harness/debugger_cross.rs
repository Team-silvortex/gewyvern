use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};

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
    write_evidence_index(&out_dir)?;

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
    let session = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--debug-session",
            "--json",
        ],
        out_dir.join("http-request.debug-session.json"),
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
    )?;
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_posture", "state"],
        "healthy",
    )?;
    assert_eq_str(
        &session,
        &[
            "recommended_focus",
            "debugger_posture",
            "recommended_action",
        ],
        "observe_stable_baseline",
    )?;
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "kind"],
        "observe",
    )?;
    require_non_empty_string(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "command"],
    )?;
    assert_eq_bool(
        &session,
        &["recommended_focus", "debugger_route", "escalation_allowed"],
        false,
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
    let session = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--debug-session",
            "--json",
        ],
        out_dir.join("http-connect-denied.debug-session.json"),
    )?;
    let envelope = run_gewyc_envelope(&dsl, out_dir.join("http-connect-denied.envelope.json"))?;

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
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_posture", "state"],
        "needs_evidence",
    )?;
    assert_eq_str(
        &session,
        &[
            "recommended_focus",
            "debugger_posture",
            "recommended_action",
        ],
        "collect_missing_runtime_evidence",
    )?;
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "kind"],
        "open_anomaly_flow",
    )?;
    require_non_empty_string(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "command"],
    )?;
    assert_eq_bool(
        &session,
        &["recommended_focus", "debugger_route", "escalation_allowed"],
        false,
    )?;
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
    )?;
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
    let session = run_gewyvern_json(
        &[
            "--protocol",
            "socks5",
            "--entry",
            "auth-connect-denied",
            "--debug-session",
            "--json",
        ],
        out_dir.join("socks5-auth-connect-denied.debug-session.json"),
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
    )?;
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_posture", "state"],
        "needs_evidence",
    )?;
    assert_eq_str(
        &session,
        &[
            "recommended_focus",
            "debugger_posture",
            "recommended_action",
        ],
        "collect_missing_runtime_evidence",
    )?;
    assert_eq_str(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "kind"],
        "open_anomaly_flow",
    )?;
    require_non_empty_string(
        &session,
        &["recommended_focus", "debugger_route", "primary_step", "command"],
    )?;
    assert_eq_bool(
        &session,
        &["recommended_focus", "debugger_route", "escalation_allowed"],
        false,
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

fn require_non_empty_string(value: &Value, path: &[&str]) -> Result<(), ValidationError> {
    let actual = string_at(value, path)?;
    if actual.trim().is_empty() {
        return Err(ValidationError::new(format!(
            "expected non-empty string at `{}`",
            path.join(".")
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
         - local debug-session JSON\n\
         - gewyc envelope JSON\n\n\
         Fast index:\n\
         - evidence-index.json\n\n\
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

fn write_evidence_index(out_dir: &std::path::Path) -> Result<(), ValidationError> {
    let index = json!({
        "runner": "gewyvern_validate debugger-cross",
        "status": "ok",
        "cases": [
            indexed_runtime_case(out_dir, "http-request", true)?,
            indexed_runtime_case(out_dir, "http-connect-denied", true)?,
            indexed_runtime_case(out_dir, "socks5-auth-connect-denied", false)?,
            indexed_invalid_gewy_case(out_dir)?,
        ],
    });
    let pretty = serde_json::to_vec_pretty(&index)?;
    fs::write(out_dir.join("evidence-index.json"), pretty)?;
    Ok(())
}

fn indexed_runtime_case(
    out_dir: &std::path::Path,
    label: &str,
    has_envelope: bool,
) -> Result<Value, ValidationError> {
    let summary_file = format!("{label}.summary.json");
    let console_file = format!("{label}.console.json");
    let session_file = format!("{label}.debug-session.json");
    let summary = read_json(out_dir, &summary_file)?;
    let console = read_json(out_dir, &console_file)?;
    let session = read_json(out_dir, &session_file)?;

    let envelope_file = if has_envelope {
        Some(format!("{label}.envelope.json"))
    } else {
        None
    };
    let envelope_status = match envelope_file.as_deref() {
        Some(file) => Some(indexed_envelope_status(&read_json(out_dir, file)?)?),
        None => None,
    };

    Ok(json!({
        "label": label,
        "kind": "runtime",
        "surfaces": {
            "summary": summary_file,
            "console": console_file,
            "debug_session": session_file,
            "envelope": envelope_file,
        },
        "summary": {
            "primary_module_family": optional_string(&summary, &["primary_module_family"]),
            "primary_module_kind": optional_string(&summary, &["primary_module_kind"]),
            "primary_failure_mode": optional_string(&summary, &["primary_failure_mode"]),
            "primary_failure_basis": optional_string(&summary, &["primary_failure_basis"]),
            "operator_guidance_action": optional_string(&summary, &["operator_guidance_action"]),
        },
        "console": {
            "status": optional_string(&console, &["recommended_focus", "status"]),
            "first_missing_transition": optional_string(
                &console,
                &["recommended_focus", "first_missing_transition"],
            ),
        },
        "debugger_posture": {
            "posture_state": optional_string(
                &session,
                &["recommended_focus", "debugger_posture", "state"],
            ),
            "recommended_action": optional_string(
                &session,
                &["recommended_focus", "debugger_posture", "recommended_action"],
            ),
            "confidence": optional_string(
                &session,
                &["recommended_focus", "debugger_posture", "confidence"],
            ),
        },
        "debugger_route": {
            "primary_step": optional_string(
                &session,
                &["recommended_focus", "debugger_route", "primary_step", "kind"],
            ),
            "primary_command": optional_string(
                &session,
                &["recommended_focus", "debugger_route", "primary_step", "command"],
            ),
            "fallback_step": optional_string(
                &session,
                &["recommended_focus", "debugger_route", "fallback_step", "kind"],
            ),
            "fallback_command": optional_string(
                &session,
                &["recommended_focus", "debugger_route", "fallback_step", "command"],
            ),
            "escalation_allowed": optional_bool(
                &session,
                &["recommended_focus", "debugger_route", "escalation_allowed"],
            ),
            "reason": optional_string(
                &session,
                &["recommended_focus", "debugger_route", "reason"],
            ),
        },
        "next_steps": value_at(&session, &["recommended_focus", "next_steps"])?
            .as_array()
            .map(|steps| {
                steps.iter()
                    .map(|step| json!({
                        "kind": step.get("kind").and_then(Value::as_str),
                        "command": step.get("command").and_then(Value::as_str),
                        "reason": step.get("reason").and_then(Value::as_str),
                    }))
                    .collect::<Vec<_>>()
            }),
        "envelope_status": envelope_status,
    }))
}

fn indexed_invalid_gewy_case(out_dir: &std::path::Path) -> Result<Value, ValidationError> {
    let envelope_file = "invalid-gewy.envelope.json";
    let envelope = read_json(out_dir, envelope_file)?;
    Ok(json!({
        "label": "invalid-gewy",
        "kind": "toolchain-negative",
        "surfaces": {
            "envelope": envelope_file,
        },
        "envelope_status": indexed_envelope_status(&envelope)?,
        "first_finding": value_at(&envelope, &["payload", "findings", "findings"])?
            .as_array()
            .and_then(|items| items.first())
            .cloned(),
    }))
}

fn indexed_envelope_status(envelope: &Value) -> Result<Value, ValidationError> {
    Ok(json!({
        "parse_ok": value_at(envelope, &["payload", "stages", "status", "parse_ok"])?
            .as_bool(),
        "validation_ok": value_at(envelope, &["payload", "stages", "status", "validation_ok"])?
            .as_bool(),
        "diagnostics_ok": value_at(envelope, &["payload", "stages", "status", "diagnostics_ok"])?
            .as_bool(),
    }))
}

fn read_json(out_dir: &std::path::Path, file_name: &str) -> Result<Value, ValidationError> {
    let bytes = fs::read(out_dir.join(file_name))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn optional_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(ToOwned::to_owned)
}

fn optional_bool(value: &Value, path: &[&str]) -> Option<bool> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_bool()
}
