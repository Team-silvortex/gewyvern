use std::fs;
use std::path::PathBuf;

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, run_cargo_status};

const FUZZ_SEED: u64 = 0x6c65_7365_6c61_6e67;
const SOURCE_CASES: usize = 2_048;
const CONTINUATION_CASES: usize = 2_048;

pub fn run_leselang_fuzz_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leselang-fuzz"));
    fs::create_dir_all(&out_dir)?;

    run_cargo_status(
        &[
            "test".to_string(),
            "--test".to_string(),
            "leselang_fuzz_tdd".to_string(),
            "--".to_string(),
            "--nocapture".to_string(),
        ],
        &out_dir.join("run.log"),
    )?;

    fs::write(
        out_dir.join("fuzz-config.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "seed": FUZZ_SEED,
            "source_cases": SOURCE_CASES,
            "continuation_cases": CONTINUATION_CASES,
            "total_cases": SOURCE_CASES + CONTINUATION_CASES,
            "test_target": "leselang_fuzz_tdd",
        }))?,
    )?;
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leselang-fuzz",
            "files": ["fuzz-config.json", "run.log"],
        }))?,
    )?;

    Ok(ValidationReport {
        name: "Leselang deterministic fuzz shelf".to_string(),
        out_dir,
        checks: vec![
            "utf8_parser_lossless_spans".to_string(),
            "hir_vm_bounded_start".to_string(),
            "continuation_decode_fail_closed".to_string(),
            "deterministic_seed_replay".to_string(),
        ],
    })
}
