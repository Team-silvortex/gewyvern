use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::command::{
    ValidationError, ValidationReport, assert_eq_bool, default_out_dir, repo_root, run_cargo_json,
};

pub fn run_registry_validation(
    out_dir: Option<PathBuf>,
    limit: Option<usize>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("registry-validation"));
    fs::create_dir_all(&out_dir)?;

    let limit = limit.or_else(limit_from_env);
    let packages = discover_packages(limit)?;
    let mut passed = Vec::new();
    let mut failures = Vec::new();

    for (index, package) in packages.iter().enumerate() {
        let rel = package_rel(package)?;
        let main = package
            .parent()
            .ok_or_else(|| ValidationError::new("package path has no parent"))?
            .join("main.gewy");
        let output_path = out_dir.join(format!("case-{:04}.json", index + 1));

        match validate_package(&main, &output_path) {
            Ok(()) => passed.push(rel),
            Err(err) => failures.push(format!("{rel} :: {err}")),
        }
    }

    write_summary(&out_dir, packages.len(), &passed, &failures)?;

    if !failures.is_empty() {
        return Err(ValidationError::new(format!(
            "registry validation found {} failing package(s); see {}",
            failures.len(),
            out_dir.join("failed.txt").display()
        )));
    }

    Ok(ValidationReport {
        name: "registry validation".to_string(),
        out_dir,
        checks: passed,
    })
}

fn discover_packages(limit: Option<usize>) -> Result<Vec<PathBuf>, ValidationError> {
    let mut packages = Vec::new();
    collect_packages(&repo_root().join("protocols"), &mut packages)?;
    packages.sort();

    if let Some(limit) = limit.filter(|limit| *limit > 0) {
        packages.truncate(limit);
    }

    Ok(packages)
}

fn collect_packages(dir: &Path, packages: &mut Vec<PathBuf>) -> Result<(), ValidationError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_packages(&path, packages)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("gewy.pkg") {
            packages.push(path);
        }
    }
    Ok(())
}

fn validate_package(main: &Path, output_path: &Path) -> Result<(), ValidationError> {
    if !main.exists() {
        return Err(ValidationError::new(format!(
            "missing main.gewy at {}",
            main.display()
        )));
    }

    let envelope = run_gewyc_envelope(main, output_path)?;
    assert_stage_ok(&envelope, "parse_ok", "parse")?;
    assert_stage_ok(&envelope, "validation_ok", "validation")?;
    assert_stage_ok(&envelope, "diagnostics_ok", "diagnostics")
}

fn assert_stage_ok(
    value: &Value,
    status_field: &str,
    legacy_field: &str,
) -> Result<(), ValidationError> {
    if assert_eq_bool(value, &["payload", "stages", "status", status_field], true).is_ok() {
        return Ok(());
    }

    assert_eq_bool(value, &["payload", "stages", legacy_field, "ok"], true)
        .map_err(|_| ValidationError::new(format!("{legacy_field}_failed")))
}

fn run_gewyc_envelope(main: &Path, output_path: &Path) -> Result<Value, ValidationError> {
    let cargo_args = vec![
        "run".to_string(),
        "--quiet".to_string(),
        "-p".to_string(),
        "gewyc".to_string(),
        "--".to_string(),
        "envelope".to_string(),
        main.display().to_string(),
        "--json".to_string(),
    ];
    run_cargo_json(&cargo_args, output_path)
}

fn package_rel(package: &Path) -> Result<String, ValidationError> {
    let rel = package
        .parent()
        .ok_or_else(|| ValidationError::new("package path has no parent"))?
        .strip_prefix(repo_root())
        .map_err(|err| ValidationError::new(err.to_string()))?;
    Ok(rel.display().to_string())
}

fn limit_from_env() -> Option<usize> {
    env::var("GEWY_REGISTRY_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|limit| *limit > 0)
}

fn write_summary(
    out_dir: &Path,
    total: usize,
    passed: &[String],
    failures: &[String],
) -> Result<(), ValidationError> {
    fs::write(out_dir.join("passed.txt"), passed.join("\n"))?;
    fs::write(out_dir.join("failed.txt"), failures.join("\n"))?;
    fs::write(
        out_dir.join("README.txt"),
        format!(
            "gewyvern native registry validation\n\
             ==================================\n\n\
             total={total}\n\
             passed={}\n\
             failed={}\n\n\
             Each case-NNNN.json file is a `gewyc envelope --json` artifact.\n\
             A passing package must report parse_ok, validation_ok, and diagnostics_ok.\n",
            passed.len(),
            failures.len()
        ),
    )?;
    Ok(())
}
