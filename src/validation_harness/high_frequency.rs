use std::fs;
use std::path::{Path, PathBuf};

use super::command::{
    ValidationError, ValidationReport, assert_eq_str, default_out_dir, repo_root, run_cargo_json,
    run_cargo_status,
};

struct SummaryCase {
    label: &'static str,
    args: &'static [&'static str],
    expected_kind: &'static str,
    expected_guidance: &'static str,
}

const SUMMARY_CASES: &[SummaryCase] = &[
    SummaryCase {
        label: "http-request",
        args: &["--dsl", "dsl/http_request_path.gewy"],
        expected_kind: "http_request_response",
        expected_guidance: "manual_review",
    },
    SummaryCase {
        label: "tls-client",
        args: &["--dsl", "dsl/tls_client_path.gewy"],
        expected_kind: "tls_handshake",
        expected_guidance: "manual_review",
    },
    SummaryCase {
        label: "ssh-session",
        args: &["--dsl", "dsl/ssh_session_path.gewy"],
        expected_kind: "remote_access_session",
        expected_guidance: "collect_more_runtime_evidence",
    },
    SummaryCase {
        label: "socks5-auth",
        args: &["--dsl", "dsl/socks5_auth_path.gewy"],
        expected_kind: "proxy_authentication",
        expected_guidance: "collect_more_runtime_evidence",
    },
    SummaryCase {
        label: "postgres-query",
        args: &["--protocol", "postgres", "--entry", "query"],
        expected_kind: "database_query",
        expected_guidance: "collect_more_runtime_evidence",
    },
];

const MIXED_FLOW_TESTS: &[&str] = &[
    "mixed_dns_tls_http_profile_stays_ambiguous_and_low_confidence",
    "mixed_proxy_tunnel_and_upstream_request_exposes_competing_hypotheses",
    "mixed_quic_http3_hy2_profile_stays_conservative",
];

pub fn run_high_frequency_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("high-frequency-validation"));
    fs::create_dir_all(&out_dir)?;

    let mut checks = Vec::new();

    for case in SUMMARY_CASES {
        run_summary_case(case, &out_dir)?;
        checks.push(case.label.to_string());
    }

    for test_name in MIXED_FLOW_TESTS {
        run_mixed_flow_test(test_name, &out_dir)?;
        checks.push((*test_name).to_string());
    }

    write_readme(&out_dir, &checks)?;

    Ok(ValidationReport {
        name: "high-frequency validation".to_string(),
        out_dir,
        checks,
    })
}

fn run_summary_case(case: &SummaryCase, out_dir: &Path) -> Result<(), ValidationError> {
    let output_path = out_dir.join(format!("{}.summary.json", case.label));
    let summary = run_gewyvern_summary(case.args, &output_path)?;

    assert_eq_str(&summary, &["primary_module_kind"], case.expected_kind)?;
    assert_eq_str(
        &summary,
        &["operator_guidance_action"],
        case.expected_guidance,
    )
}

fn run_gewyvern_summary(
    args: &[&str],
    output_path: &Path,
) -> Result<serde_json::Value, ValidationError> {
    let mut cargo_args = vec!["run".to_string(), "--quiet".to_string(), "--".to_string()];
    for arg in args {
        if let Some(relative) = arg.strip_prefix("dsl/") {
            cargo_args.push(repo_root().join("dsl").join(relative).display().to_string());
        } else {
            cargo_args.push((*arg).to_string());
        }
    }
    cargo_args.push("--json".to_string());
    cargo_args.push("--summary-only".to_string());
    run_cargo_json(&cargo_args, output_path)
}

fn run_mixed_flow_test(test_name: &str, out_dir: &Path) -> Result<(), ValidationError> {
    let cargo_args = vec![
        "test".to_string(),
        "--quiet".to_string(),
        "--bin".to_string(),
        "gewyvern".to_string(),
        test_name.to_string(),
    ];
    run_cargo_status(
        &cargo_args,
        &out_dir.join(format!("{}.test.log", test_name.replace('_', "-"))),
    )
}

fn write_readme(out_dir: &Path, checks: &[String]) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("README.txt"),
        format!(
            "gewyvern native high-frequency validation\n\
             =========================================\n\n\
             checks={}\n\n\
             This bundle validates high-frequency protocol summaries and mixed-flow\n\
             conservatism tests without shell-owned assertions.\n",
            checks.len()
        ),
    )?;
    Ok(())
}
