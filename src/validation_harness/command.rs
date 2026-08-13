use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::bounded_process::{OutputLimits, run_command_output as run_bounded_command_output};

static VALIDATION_JSON_MODE: AtomicBool = AtomicBool::new(false);

const CARGO_COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const COMMAND_CAPTURE_LIMIT_BYTES: usize = 32 * 1024 * 1024;

pub(super) const DOTNET_PROOF_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub(super) const PROOF_FIXTURE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub(super) const TOOL_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const VALIDATION_HELPER_TIMEOUT: Duration = Duration::from_secs(30);

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

pub fn set_validation_json_mode(enabled: bool) {
    VALIDATION_JSON_MODE.store(enabled, Ordering::Relaxed);
}

pub fn validation_json_mode() -> bool {
    VALIDATION_JSON_MODE.load(Ordering::Relaxed)
}

pub fn validation_log(message: impl AsRef<str>) {
    if !validation_json_mode() {
        println!("{}", message.as_ref());
    }
}

pub fn validation_command_stdout() -> Stdio {
    if validation_json_mode() {
        Stdio::null()
    } else {
        Stdio::inherit()
    }
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn default_out_dir(name: &str) -> PathBuf {
    repo_root().join("target").join("validation").join(name)
}

pub fn run_command_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
    description: &str,
) -> Result<Output, ValidationError> {
    run_command_output_with_limits(
        command,
        timeout,
        COMMAND_CAPTURE_LIMIT_BYTES,
        COMMAND_CAPTURE_LIMIT_BYTES,
        description,
    )
}

fn run_command_output_with_limits(
    command: &mut Command,
    timeout: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    description: &str,
) -> Result<Output, ValidationError> {
    run_bounded_command_output(
        command,
        timeout,
        OutputLimits::new(max_stdout_bytes, max_stderr_bytes),
        description,
    )
    .map_err(|error| ValidationError::new(error.to_string()))
}

pub fn run_cargo_json(args: &[String], output_path: &Path) -> Result<Value, ValidationError> {
    let cargo = cargo_command_from_env("CARGO")?;
    let mut command = Command::new(cargo);
    command.current_dir(repo_root()).args(args);
    let output =
        run_command_output_with_timeout(&mut command, CARGO_COMMAND_TIMEOUT, "cargo command")?;

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
    let cargo = cargo_command_from_env("CARGO")?;
    let mut command = Command::new(cargo);
    command.current_dir(repo_root()).args(args);
    let output =
        run_command_output_with_timeout(&mut command, CARGO_COMMAND_TIMEOUT, "cargo command")?;

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

pub fn cargo_command_from_env(var_name: &str) -> Result<String, ValidationError> {
    let command = env::var(var_name).unwrap_or_else(|_| "cargo".to_string());
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new(format!(
            "{var_name} must not be empty"
        )));
    }
    if trimmed != command {
        return Err(ValidationError::new(format!(
            "{var_name} must not have surrounding whitespace"
        )));
    }
    if trimmed.chars().any(|ch| {
        ch.is_whitespace()
            || matches!(ch, ';' | '&' | '|' | '`' | '$' | '<' | '>' | '(' | ')' | '{' | '}')
    }) {
        return Err(ValidationError::new(format!(
            "{var_name} must be a single executable path without shell metacharacters"
        )));
    }
    if trimmed != "cargo" && !trimmed.contains('/') && !trimmed.contains('\\') {
        return Err(ValidationError::new(format!(
            "{var_name} must be an executable path; got `{trimmed}`"
        )));
    }
    if trimmed != "cargo" {
        let metadata = fs::metadata(trimmed)
            .map_err(|error| ValidationError::new(format!("{var_name} command path missing: {error}")))?;
        if !metadata.is_file() {
            return Err(ValidationError::new(format!(
                "{var_name} must point to a regular file"
            )));
        }
    }
    Ok(trimmed.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Instant;

    const TIMEOUT_PROBE_ENV: &str = "GEWYVERN_VALIDATION_TIMEOUT_PROBE";
    const PIPE_HOLDER_PROBE_ENV: &str = "GEWYVERN_VALIDATION_PIPE_HOLDER_PROBE";
    const OUTPUT_PROBE_ENV: &str = "GEWYVERN_VALIDATION_OUTPUT_PROBE";

    #[test]
    fn bounded_command_child_probe() {
        if let Ok(delay_ms) = env::var(TIMEOUT_PROBE_ENV) {
            thread::sleep(Duration::from_millis(delay_ms.parse().unwrap()));
        } else if let Ok(output_bytes) = env::var(OUTPUT_PROBE_ENV) {
            use std::io::Write as _;

            let bytes = vec![b'x'; output_bytes.parse().unwrap()];
            std::io::stdout().write_all(&bytes).unwrap();
        } else if env::var_os(PIPE_HOLDER_PROBE_ENV).is_some() {
            let mut pipe_holder = Command::new(env::current_exe().unwrap())
                .arg("bounded_command_child_probe")
                .env(TIMEOUT_PROBE_ENV, "1500")
                .spawn()
                .unwrap();
            thread::spawn(move || {
                let _ = pipe_holder.wait();
            });
        }
    }

    #[test]
    fn bounded_command_collects_successful_output() {
        let mut command = Command::new(env::current_exe().unwrap());
        command.arg("bounded_command_child_probe");

        let output = run_command_output_with_timeout(
            &mut command,
            Duration::from_secs(5),
            "validation timeout success probe",
        )
        .unwrap();

        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("test result: ok"));
    }

    #[test]
    fn bounded_command_terminates_a_hung_child() {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .arg("bounded_command_child_probe")
            .env(TIMEOUT_PROBE_ENV, "5000");
        let started = Instant::now();

        let error = run_command_output_with_timeout(
            &mut command,
            Duration::from_millis(200),
            "validation timeout failure probe",
        )
        .unwrap_err();

        assert!(error.to_string().contains("timed out after 0.200s"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_command_times_out_when_a_descendant_holds_output_open() {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .arg("bounded_command_child_probe")
            .env(PIPE_HOLDER_PROBE_ENV, "1");
        let started = Instant::now();

        let error = run_command_output_with_timeout(
            &mut command,
            Duration::from_millis(500),
            "validation inherited-pipe probe",
        )
        .unwrap_err();

        assert!(error.to_string().contains("while draining stdout"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_command_rejects_oversized_captured_output() {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .arg("bounded_command_child_probe")
            .arg("--nocapture")
            .env(OUTPUT_PROBE_ENV, "4096");

        let error = run_command_output_with_limits(
            &mut command,
            Duration::from_secs(5),
            1024,
            1024,
            "validation output limit probe",
        )
        .unwrap_err();

        assert!(error.to_string().contains("stdout"));
        assert!(error.to_string().contains("exceeded 1024 bytes"));
    }

    #[test]
    fn cargo_command_from_env_requires_path_or_default() {
        const TEST_VAR: &str = "GEWY_CARGO_VALIDATION_TEST";
        unsafe {
            std::env::remove_var(TEST_VAR);
        }
        assert_eq!(cargo_command_from_env(TEST_VAR).unwrap(), "cargo");

        unsafe {
            std::env::set_var(TEST_VAR, "my-cargo");
        }
        assert!(
            cargo_command_from_env(TEST_VAR).is_err(),
            "bare command names should be rejected"
        );

        let current = std::env::current_exe().unwrap();
        let current = current.to_string_lossy().to_string();
        unsafe {
            std::env::set_var(TEST_VAR, &current);
        }
        assert_eq!(cargo_command_from_env(TEST_VAR).unwrap(), current);
        unsafe {
            std::env::remove_var(TEST_VAR);
        }
    }
}
