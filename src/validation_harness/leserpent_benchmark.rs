use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_json,
    run_cargo_status, run_command_output_with_timeout, validation_log, value_at,
};

const COLD_OPEN_P95_BUDGET_MS: f64 = 250.0;
const RUNTIME_LIST_P50_BUDGET_MS: f64 = 5.0;
const EFFECT_ENQUEUE_BUDGET_MS: f64 = 5_000.0;
const EFFECT_ENQUEUE_MIN_PER_SECOND: f64 = 2_000.0;
const LANGUAGE_PARSE_P50_BUDGET_MS: f64 = 5.0;
const LANGUAGE_LOWER_P50_BUDGET_MS: f64 = 5.0;
const LANGUAGE_VM_START_P50_BUDGET_MS: f64 = 20.0;
const LANGUAGE_FULL_PIPELINE_P50_BUDGET_MS: f64 = 30.0;
const LANGUAGE_PIPELINE_COMPONENT_RATIO_MAX: f64 = 2.0;
const UI_DOCUMENT_P50_BUDGET_MS: f64 = 20.0;
const UI_PATCH_P50_BUDGET_MS: f64 = 100.0;
const UI_PATCH_TO_DOCUMENT_RATIO_MAX: f64 = 4.0;
const UI_CODEC_P50_BUDGET_MS: f64 = 50.0;
const UI_DOCUMENT_MAX_BYTES: u64 = 2 * 1024 * 1024;
const RELEASE_BINARY_MAX_BYTES: u64 = 32 * 1024 * 1024;
const REMOTE_INCREMENTAL_P50_BUDGET_MS: f64 = 10.0;
const REMOTE_INCREMENTAL_RATIO_MAX: f64 = 0.60;
const REMOTE_INCREMENTAL_ALLOCATION_RATIO_MAX: f64 = 0.60;
const DOTNET_BENCHMARK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub fn run_leserpent_benchmark_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-benchmark"));
    fs::create_dir_all(&out_dir)?;
    let dotnet_artifacts = out_dir.join("dotnet-artifacts");
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(&dotnet_artifacts)?;
    }

    validation_log("leserpent benchmark [1/5]: runtime persistence and effect throughput");
    let runtime = run_cargo_json(
        &[
            "run".into(),
            "--quiet".into(),
            "--release".into(),
            "--locked".into(),
            "-p".into(),
            "leserpent-runtime".into(),
            "--example".into(),
            "runtime_benchmark".into(),
        ],
        &out_dir.join("runtime-benchmark.json"),
    )?;
    validate_runtime_benchmark(&runtime)?;

    validation_log("leserpent benchmark [2/5]: Leselang parse, lower, and VM pipeline");
    let language = run_cargo_json(
        &[
            "run".into(),
            "--quiet".into(),
            "--release".into(),
            "--locked".into(),
            "-p".into(),
            "leselang-vm".into(),
            "--example".into(),
            "language_benchmark".into(),
        ],
        &out_dir.join("language-benchmark.json"),
    )?;
    validate_language_benchmark(&language)?;

    validation_log("leserpent benchmark [3/5]: renderer-neutral UI document, patch, and codec");
    let ui = run_cargo_json(
        &[
            "run".into(),
            "--quiet".into(),
            "--release".into(),
            "--locked".into(),
            "-p".into(),
            "leselang-ui".into(),
            "--example".into(),
            "ui_benchmark".into(),
        ],
        &out_dir.join("ui-benchmark.json"),
    )?;
    validate_ui_benchmark(&ui)?;

    validation_log("leserpent benchmark [4/5]: .NET incremental workspace-log projection");
    let remote_client = run_dotnet_json(
        "apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj",
        &["--benchmark-workspace-logs"],
        &out_dir.join("remote-workspace-log-benchmark.json"),
        &dotnet_artifacts,
    )?;
    validate_remote_client_benchmark(&remote_client)?;
    fs::remove_dir_all(&dotnet_artifacts)?;

    validation_log("leserpent benchmark [5/5]: release CLI and daemon binary size");
    run_cargo_status(
        &[
            "build".into(),
            "--quiet".into(),
            "--release".into(),
            "--locked".into(),
            "-p".into(),
            "leserpent-cli".into(),
            "-p".into(),
            "leserpentd".into(),
        ],
        &out_dir.join("release-build.log"),
    )?;
    let binaries = release_binary_manifest()?;
    for binary in &binaries {
        if binary["bytes"].as_u64().unwrap_or(u64::MAX) > RELEASE_BINARY_MAX_BYTES {
            return Err(ValidationError::new(format!(
                "release binary '{}' exceeds the {} byte benchmark budget",
                binary["name"].as_str().unwrap_or("unknown"),
                RELEASE_BINARY_MAX_BYTES
            )));
        }
    }
    fs::write(
        out_dir.join("binary-manifest.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "max_binary_bytes": RELEASE_BINARY_MAX_BYTES,
            "binaries": binaries,
        }))?,
    )?;

    let summary = json!({
        "schema_version": 1,
        "host": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
        "budgets": {
            "cold_open_p95_ms": COLD_OPEN_P95_BUDGET_MS,
            "runtime_list_p50_ms": RUNTIME_LIST_P50_BUDGET_MS,
            "effect_enqueue_ms": EFFECT_ENQUEUE_BUDGET_MS,
            "effect_enqueue_min_per_second": EFFECT_ENQUEUE_MIN_PER_SECOND,
            "language_parse_p50_ms": LANGUAGE_PARSE_P50_BUDGET_MS,
            "language_lower_p50_ms": LANGUAGE_LOWER_P50_BUDGET_MS,
            "language_vm_start_p50_ms": LANGUAGE_VM_START_P50_BUDGET_MS,
            "language_full_pipeline_p50_ms": LANGUAGE_FULL_PIPELINE_P50_BUDGET_MS,
            "language_pipeline_component_ratio_max": LANGUAGE_PIPELINE_COMPONENT_RATIO_MAX,
            "ui_document_p50_ms": UI_DOCUMENT_P50_BUDGET_MS,
            "ui_patch_p50_ms": UI_PATCH_P50_BUDGET_MS,
            "ui_patch_to_document_ratio_max": UI_PATCH_TO_DOCUMENT_RATIO_MAX,
            "ui_codec_p50_ms": UI_CODEC_P50_BUDGET_MS,
            "ui_document_max_bytes": UI_DOCUMENT_MAX_BYTES,
            "release_binary_max_bytes": RELEASE_BINARY_MAX_BYTES,
            "remote_incremental_p50_ms": REMOTE_INCREMENTAL_P50_BUDGET_MS,
            "remote_incremental_ratio_max": REMOTE_INCREMENTAL_RATIO_MAX,
            "remote_incremental_allocation_ratio_max":
                REMOTE_INCREMENTAL_ALLOCATION_RATIO_MAX,
            "dotnet_benchmark_timeout_seconds": DOTNET_BENCHMARK_TIMEOUT.as_secs(),
        },
        "runtime": runtime,
        "language": language,
        "ui": ui,
        "remote_client": remote_client,
        "binaries": binaries,
        "status": "passed",
        "comparison_policy": "compare timing only within the same host class",
    });
    fs::write(
        out_dir.join("benchmark-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-benchmark",
            "files": [
                "runtime-benchmark.json", "language-benchmark.json", "ui-benchmark.json",
                "remote-workspace-log-benchmark.json", "release-build.log",
                "binary-manifest.json", "benchmark-summary.json",
            ],
        }))?,
    )?;

    Ok(ValidationReport {
        name: "Leserpent bounded performance and size shelf".into(),
        out_dir,
        checks: vec![
            "sqlite_cold_open_p95_budget".into(),
            "runtime_list_p50_budget".into(),
            "effect_batch_throughput_budget".into(),
            "language_parse_p50_budget".into(),
            "language_lower_p50_budget".into(),
            "language_vm_start_p50_budget".into(),
            "language_full_pipeline_p50_budget".into(),
            "language_pipeline_component_ratio_budget".into(),
            "ui_document_p50_budget".into(),
            "ui_patch_apply_p50_budget".into(),
            "ui_patch_to_document_ratio_budget".into(),
            "ui_codec_p50_budget".into(),
            "remote_incremental_log_p50_budget".into(),
            "remote_incremental_log_ratio_budget".into(),
            "remote_incremental_log_allocation_budget".into(),
            "remote_incremental_log_window_integrity".into(),
            "release_binary_size_budget".into(),
            "same_host_class_comparison_policy".into(),
            "isolated_dotnet_artifacts".into(),
            "bounded_dotnet_benchmark_subprocess".into(),
        ],
    })
}

fn validate_runtime_benchmark(value: &Value) -> Result<(), ValidationError> {
    require_exact_u64(value, &["schema_version"], 1)?;
    require_exact_u64(value, &["workload", "runtime_count"], 256)?;
    require_exact_u64(value, &["workload", "effect_count"], 10_000)?;
    require_max(
        value,
        &["metrics", "cold_open_p95_ms"],
        COLD_OPEN_P95_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "runtime_list_p50_ms"],
        RUNTIME_LIST_P50_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "effect_enqueue_ms"],
        EFFECT_ENQUEUE_BUDGET_MS,
    )?;
    require_min(
        value,
        &["metrics", "effect_enqueue_per_second"],
        EFFECT_ENQUEUE_MIN_PER_SECOND,
    )
}

fn validate_language_benchmark(value: &Value) -> Result<(), ValidationError> {
    require_exact_u64(value, &["schema_version"], 1)?;
    require_exact_u64(value, &["workload", "branch_count"], 64)?;
    require_exact_u64(value, &["workload", "iterations"], 500)?;
    require_exact_u64(value, &["workload", "source_bytes"], 5_371)?;
    require_exact_u64(value, &["workload", "token_count"], 1_674)?;
    require_exact_u64(value, &["metrics", "effect_request_count"], 64)?;
    require_max(
        value,
        &["metrics", "parse_p50_ms"],
        LANGUAGE_PARSE_P50_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "lower_p50_ms"],
        LANGUAGE_LOWER_P50_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "vm_start_p50_ms"],
        LANGUAGE_VM_START_P50_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "full_pipeline_p50_ms"],
        LANGUAGE_FULL_PIPELINE_P50_BUDGET_MS,
    )?;
    let component_total = finite_number_at(value, &["metrics", "parse_p50_ms"])?
        + finite_number_at(value, &["metrics", "lower_p50_ms"])?
        + finite_number_at(value, &["metrics", "vm_start_p50_ms"])?;
    let full_pipeline = finite_number_at(value, &["metrics", "full_pipeline_p50_ms"])?;
    if component_total == 0.0
        || full_pipeline / component_total > LANGUAGE_PIPELINE_COMPONENT_RATIO_MAX
    {
        return Err(ValidationError::new(format!(
            "benchmark language pipeline/component ratio is {:.3}, above budget {:.3}",
            full_pipeline / component_total,
            LANGUAGE_PIPELINE_COMPONENT_RATIO_MAX
        )));
    }
    Ok(())
}

fn validate_ui_benchmark(value: &Value) -> Result<(), ValidationError> {
    require_exact_u64(value, &["schema_version"], 1)?;
    require_exact_u64(value, &["workload", "runtime_count"], 256)?;
    require_exact_u64(value, &["workload", "ui_node_count"], 1_539)?;
    require_exact_u64(value, &["metrics", "patch_operations"], 2)?;
    require_max(
        value,
        &["metrics", "document_p50_ms"],
        UI_DOCUMENT_P50_BUDGET_MS,
    )?;
    require_max(value, &["metrics", "patch_p50_ms"], UI_PATCH_P50_BUDGET_MS)?;
    let document_p50_ms = finite_number_at(value, &["metrics", "document_p50_ms"])?;
    let patch_p50_ms = finite_number_at(value, &["metrics", "patch_p50_ms"])?;
    if document_p50_ms == 0.0 || patch_p50_ms / document_p50_ms > UI_PATCH_TO_DOCUMENT_RATIO_MAX {
        return Err(ValidationError::new(format!(
            "benchmark UI patch/document ratio is {:.3}, above budget {:.3}",
            patch_p50_ms / document_p50_ms,
            UI_PATCH_TO_DOCUMENT_RATIO_MAX
        )));
    }
    require_max(value, &["metrics", "codec_p50_ms"], UI_CODEC_P50_BUDGET_MS)?;
    require_max(
        value,
        &["metrics", "encoded_document_bytes"],
        UI_DOCUMENT_MAX_BYTES as f64,
    )
}

fn validate_remote_client_benchmark(value: &Value) -> Result<(), ValidationError> {
    require_exact_u64(value, &["schema_version"], 1)?;
    require_exact_u64(value, &["workload", "iterations"], 500)?;
    require_exact_u64(value, &["workload", "full_log_count"], 256)?;
    require_exact_u64(value, &["workload", "incremental_log_count"], 8)?;
    require_exact_u64(value, &["metrics", "merged_log_count"], 256)?;
    require_max(
        value,
        &["metrics", "incremental_snapshot_p50_ms"],
        REMOTE_INCREMENTAL_P50_BUDGET_MS,
    )?;
    require_max(
        value,
        &["metrics", "incremental_to_full_ratio"],
        REMOTE_INCREMENTAL_RATIO_MAX,
    )?;
    require_max(
        value,
        &["metrics", "incremental_allocation_ratio"],
        REMOTE_INCREMENTAL_ALLOCATION_RATIO_MAX,
    )
}

fn run_dotnet_json(
    project: &str,
    app_args: &[&str],
    output_path: &std::path::Path,
    dotnet_artifacts: &std::path::Path,
) -> Result<Value, ValidationError> {
    let mut command = Command::new("dotnet");
    command
        .current_dir(repo_root())
        .args([
            "run",
            "--project",
            project,
            "--configuration",
            "Release",
            "--verbosity",
            "quiet",
        ])
        .arg("--artifacts-path")
        .arg(dotnet_artifacts)
        .arg("--")
        .args(app_args);
    let output = run_command_output_with_timeout(
        &mut command,
        DOTNET_BENCHMARK_TIMEOUT,
        "remote-client benchmark",
    )?;
    fs::write(output_path, &output.stdout)?;
    if !output.status.success() {
        fs::write(output_path.with_extension("stderr.txt"), &output.stderr)?;
        return Err(ValidationError::new(format!(
            "remote-client benchmark failed with status {}",
            output.status
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        ValidationError::new(format!(
            "failed to parse remote-client benchmark JSON: {error}"
        ))
    })
}

fn release_binary_manifest() -> Result<Vec<Value>, ValidationError> {
    ["leserpent", "leserpentd"]
        .into_iter()
        .map(|name| {
            let path = release_binary_path(name);
            let bytes = fs::metadata(&path)
                .map_err(|error| {
                    ValidationError::new(format!(
                        "release binary '{}' is unavailable: {error}",
                        path.display()
                    ))
                })?
                .len();
            Ok(json!({"name": name, "path": path.display().to_string(), "bytes": bytes}))
        })
        .collect()
}

fn release_binary_path(name: &str) -> PathBuf {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let configured = env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    let target_dir = match configured {
        Some(path) if path.is_absolute() => path,
        Some(path) => repo_root().join(path),
        None => repo_root().join("target"),
    };
    target_dir.join("release").join(format!("{name}{suffix}"))
}

fn require_exact_u64(value: &Value, path: &[&str], expected: u64) -> Result<(), ValidationError> {
    let actual = value_at(value, path)?.as_u64().ok_or_else(|| {
        ValidationError::new(format!(
            "benchmark metric '{}' is not an integer",
            path.join(".")
        ))
    })?;
    if actual != expected {
        return Err(ValidationError::new(format!(
            "benchmark workload '{}' changed from {expected} to {actual}",
            path.join(".")
        )));
    }
    Ok(())
}

fn require_max(value: &Value, path: &[&str], maximum: f64) -> Result<(), ValidationError> {
    let actual = finite_number_at(value, path)?;
    if actual > maximum {
        return Err(ValidationError::new(format!(
            "benchmark metric '{}' is {actual:.3}, above budget {maximum:.3}",
            path.join(".")
        )));
    }
    Ok(())
}

fn require_min(value: &Value, path: &[&str], minimum: f64) -> Result<(), ValidationError> {
    let actual = finite_number_at(value, path)?;
    if actual < minimum {
        return Err(ValidationError::new(format!(
            "benchmark metric '{}' is {actual:.3}, below budget {minimum:.3}",
            path.join(".")
        )));
    }
    Ok(())
}

fn finite_number_at(value: &Value, path: &[&str]) -> Result<f64, ValidationError> {
    let actual = value_at(value, path)?.as_f64().ok_or_else(|| {
        ValidationError::new(format!(
            "benchmark metric '{}' is not numeric",
            path.join(".")
        ))
    })?;
    if !actual.is_finite() || actual < 0.0 {
        return Err(ValidationError::new(format!(
            "benchmark metric '{}' is not a finite non-negative number",
            path.join(".")
        )));
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_budget_rejects_slow_or_changed_workloads() {
        let healthy = json!({
            "schema_version": 1,
            "workload": {"runtime_count": 256, "effect_count": 10_000},
            "metrics": {
                "cold_open_p95_ms": 20.0, "runtime_list_p50_ms": 1.0,
                "effect_enqueue_ms": 700.0, "effect_enqueue_per_second": 14_000.0,
            }
        });
        assert!(validate_runtime_benchmark(&healthy).is_ok());
        let mut slow = healthy.clone();
        slow["metrics"]["cold_open_p95_ms"] = json!(COLD_OPEN_P95_BUDGET_MS + 1.0);
        assert!(validate_runtime_benchmark(&slow).is_err());
        let mut changed = healthy;
        changed["workload"]["effect_count"] = json!(9_999);
        assert!(validate_runtime_benchmark(&changed).is_err());
    }

    #[test]
    fn ui_budget_rejects_large_or_slow_documents() {
        let healthy = json!({
            "schema_version": 1,
            "workload": {"runtime_count": 256, "ui_node_count": 1_539},
            "metrics": {
                "document_p50_ms": 2.0, "patch_p50_ms": 6.0,
                "codec_p50_ms": 4.0, "encoded_document_bytes": 280_000,
                "patch_operations": 2,
            }
        });
        assert!(validate_ui_benchmark(&healthy).is_ok());
        let mut large = healthy.clone();
        large["metrics"]["encoded_document_bytes"] = json!(UI_DOCUMENT_MAX_BYTES + 1);
        assert!(validate_ui_benchmark(&large).is_err());
        let mut inefficient = healthy.clone();
        inefficient["metrics"]["patch_p50_ms"] =
            json!(2.0 * (UI_PATCH_TO_DOCUMENT_RATIO_MAX + 0.1));
        assert!(validate_ui_benchmark(&inefficient).is_err());
        let mut slow = healthy;
        slow["metrics"]["patch_p50_ms"] = json!(UI_PATCH_P50_BUDGET_MS + 1.0);
        assert!(validate_ui_benchmark(&slow).is_err());
    }

    #[test]
    fn language_budget_rejects_slow_or_changed_pipeline() {
        let healthy = json!({
            "schema_version": 1,
            "workload": {
                "branch_count": 64, "iterations": 500,
                "source_bytes": 5_371, "token_count": 1_674,
            },
            "metrics": {
                "parse_p50_ms": 0.1, "lower_p50_ms": 0.1,
                "vm_start_p50_ms": 0.4, "full_pipeline_p50_ms": 0.7,
                "effect_request_count": 64,
            }
        });
        assert!(validate_language_benchmark(&healthy).is_ok());
        let mut changed = healthy.clone();
        changed["workload"]["branch_count"] = json!(63);
        assert!(validate_language_benchmark(&changed).is_err());
        let mut vacuous = healthy.clone();
        vacuous["metrics"]["effect_request_count"] = json!(0);
        assert!(validate_language_benchmark(&vacuous).is_err());
        let mut inefficient = healthy.clone();
        inefficient["metrics"]["full_pipeline_p50_ms"] =
            json!(0.6 * (LANGUAGE_PIPELINE_COMPONENT_RATIO_MAX + 0.1));
        assert!(validate_language_benchmark(&inefficient).is_err());
        let mut slow = healthy;
        slow["metrics"]["vm_start_p50_ms"] = json!(LANGUAGE_VM_START_P50_BUDGET_MS + 1.0);
        assert!(validate_language_benchmark(&slow).is_err());
    }

    #[test]
    fn remote_incremental_budget_rejects_changed_or_full_cost_workloads() {
        let healthy = json!({
            "schema_version": 1,
            "workload": {
                "iterations": 500,
                "full_log_count": 256,
                "incremental_log_count": 8,
            },
            "metrics": {
                "incremental_snapshot_p50_ms": 0.4,
                "incremental_to_full_ratio": 0.2,
                "incremental_allocation_ratio": 0.15,
                "merged_log_count": 256,
            }
        });
        assert!(validate_remote_client_benchmark(&healthy).is_ok());
        let mut changed = healthy.clone();
        changed["workload"]["incremental_log_count"] = json!(9);
        assert!(validate_remote_client_benchmark(&changed).is_err());
        let mut full_cost = healthy;
        full_cost["metrics"]["incremental_to_full_ratio"] =
            json!(REMOTE_INCREMENTAL_RATIO_MAX + 0.01);
        assert!(validate_remote_client_benchmark(&full_cost).is_err());
    }
}
