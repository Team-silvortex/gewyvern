use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[derive(Debug)]
pub struct ValidationError {
    message: String,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ValidationError {}

impl From<std::io::Error> for ValidationError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<serde_json::Error> for ValidationError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err.to_string())
    }
}

pub struct ValidationReport {
    pub name: String,
    pub out_dir: PathBuf,
    pub checks: Vec<String>,
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn default_out_dir(name: &str) -> PathBuf {
    repo_root().join("target").join("validation").join(name)
}

pub fn run_cargo_json(args: &[String], output_path: &Path) -> Result<Value, ValidationError> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(repo_root())
        .args(args)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run cargo: {err}")))?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, &output.stdout)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        fs::write(output_path.with_extension("stderr.txt"), stderr.as_bytes())?;
        return Err(ValidationError::new(format!(
            "cargo command failed with status {}: {}",
            output.status, stderr
        )));
    }

    serde_json::from_slice(&output.stdout).map_err(|err| {
        ValidationError::new(format!(
            "failed to parse JSON from {}: {err}",
            output_path.display()
        ))
    })
}

pub fn run_cargo_status(args: &[String], output_path: &Path) -> Result<(), ValidationError> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(repo_root())
        .args(args)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run cargo: {err}")))?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut transcript = Vec::new();
    transcript.extend_from_slice(b"stdout:\n");
    transcript.extend_from_slice(&output.stdout);
    transcript.extend_from_slice(b"\n\nstderr:\n");
    transcript.extend_from_slice(&output.stderr);
    fs::write(output_path, transcript)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ValidationError::new(format!(
            "cargo command failed with status {}: {}",
            output.status, stderr
        )));
    }

    Ok(())
}

pub fn value_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, ValidationError> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key).ok_or_else(|| {
            ValidationError::new(format!("missing JSON field `{}`", path.join(".")))
        })?;
    }
    Ok(cursor)
}

pub fn string_at(value: &Value, path: &[&str]) -> Result<String, ValidationError> {
    value_at(value, path)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| ValidationError::new(format!("expected string at `{}`", path.join("."))))
}

pub fn bool_at(value: &Value, path: &[&str]) -> Result<bool, ValidationError> {
    value_at(value, path)?
        .as_bool()
        .ok_or_else(|| ValidationError::new(format!("expected bool at `{}`", path.join("."))))
}

pub fn assert_eq_str(value: &Value, path: &[&str], expected: &str) -> Result<(), ValidationError> {
    let actual = string_at(value, path)?;
    if actual != expected {
        return Err(ValidationError::new(format!(
            "expected `{}` to be `{expected}`, got `{actual}`",
            path.join(".")
        )));
    }
    Ok(())
}

pub fn assert_eq_bool(value: &Value, path: &[&str], expected: bool) -> Result<(), ValidationError> {
    let actual = bool_at(value, path)?;
    if actual != expected {
        return Err(ValidationError::new(format!(
            "expected `{}` to be `{expected}`, got `{actual}`",
            path.join(".")
        )));
    }
    Ok(())
}

pub fn assert_array_contains_str(
    value: &Value,
    path: &[&str],
    expected: &str,
) -> Result<(), ValidationError> {
    let Some(items) = value_at(value, path)?.as_array() else {
        return Err(ValidationError::new(format!(
            "expected array at `{}`",
            path.join(".")
        )));
    };

    let found = items.iter().any(|item| item.as_str() == Some(expected));
    if !found {
        return Err(ValidationError::new(format!(
            "expected `{}` to contain `{expected}`",
            path.join(".")
        )));
    }
    Ok(())
}
