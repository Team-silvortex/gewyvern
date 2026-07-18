use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::command::ValidationError;

pub fn parse_bounded_unique_key_values(
    output: &str,
    context: &str,
    allowed_keys: &[&str],
) -> Result<BTreeMap<String, String>, ValidationError> {
    const MAX_BYTES: usize = 8 * 1024;
    const MAX_LINES: usize = 32;

    if output.len() > MAX_BYTES {
        return Err(ValidationError::new(format!(
            "{context} exceeds {MAX_BYTES} bytes"
        )));
    }
    let mut values = BTreeMap::new();
    for (index, line) in output.lines().enumerate() {
        if index >= MAX_LINES {
            return Err(ValidationError::new(format!(
                "{context} exceeds {MAX_LINES} lines"
            )));
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| ValidationError::new(format!("{context} contains a malformed entry")))?;
        if key.is_empty()
            || key.chars().any(char::is_control)
            || value.chars().any(char::is_control)
        {
            return Err(ValidationError::new(format!(
                "{context} contains an invalid key or value"
            )));
        }
        if !allowed_keys.contains(&key) {
            return Err(ValidationError::new(format!(
                "{context} contains unexpected key {key}"
            )));
        }
        if values.insert(key.to_string(), value.to_string()).is_some() {
            return Err(ValidationError::new(format!(
                "{context} contains duplicate key {key}"
            )));
        }
    }
    Ok(values)
}

pub fn read_bounded_unique_key_value_file(
    path: &Path,
    context: &str,
    allowed_keys: &[&str],
) -> Result<BTreeMap<String, String>, ValidationError> {
    let contents = read_bounded_text_file(path, context, 8 * 1024)?;
    parse_bounded_unique_key_values(&contents, context, allowed_keys)
}

pub fn read_bounded_phase_timings(
    path: &Path,
    context: &str,
    allowed_keys: &[&str],
    required_keys: &[&str],
) -> Result<Vec<(String, f64)>, ValidationError> {
    const MAX_SECONDS: f64 = 24.0 * 60.0 * 60.0;

    let values = read_bounded_unique_key_value_file(path, context, allowed_keys)?;
    for key in required_keys {
        if !values.contains_key(*key) {
            return Err(ValidationError::new(format!("{context} missing {key}")));
        }
    }
    values
        .into_iter()
        .map(|(name, value)| {
            let seconds = value.parse::<f64>().map_err(|_| {
                ValidationError::new(format!("{context} {name} is not a valid number"))
            })?;
            if !seconds.is_finite() || !(0.0..=MAX_SECONDS).contains(&seconds) {
                return Err(ValidationError::new(format!(
                    "{context} {name} must be finite and between 0 and {MAX_SECONDS} seconds"
                )));
            }
            Ok((name, seconds))
        })
        .collect()
}

pub fn read_bounded_json_file(
    path: &Path,
    context: &str,
    max_bytes: u64,
) -> Result<serde_json::Value, ValidationError> {
    let body = read_bounded_text_file(path, context, max_bytes)?;
    serde_json::from_str(&body).map_err(|error| {
        ValidationError::new(format!(
            "failed to parse {context} '{}': {error}",
            path.display()
        ))
    })
}

pub fn read_bounded_nonempty_lines(
    path: &Path,
    context: &str,
    max_bytes: u64,
    max_lines: usize,
    max_line_bytes: usize,
) -> Result<Vec<String>, ValidationError> {
    let body = read_bounded_text_file(path, context, max_bytes)?;
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() > max_lines
        || lines
            .iter()
            .any(|line| line.len() > max_line_bytes || line.chars().any(char::is_control))
    {
        return Err(ValidationError::new(format!(
            "{context} exceeds its bounded line contract"
        )));
    }
    Ok(lines.into_iter().map(ToOwned::to_owned).collect())
}

fn read_bounded_text_file(
    path: &Path,
    context: &str,
    max_bytes: u64,
) -> Result<String, ValidationError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ValidationError::new(format!(
            "failed to inspect {context} '{}': {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ValidationError::new(format!(
            "{context} must be a regular non-symlink file: {}",
            path.display()
        )));
    }
    if metadata.len() > max_bytes {
        return Err(ValidationError::new(format!(
            "{context} '{}' exceeds {max_bytes} bytes",
            path.display()
        )));
    }
    fs::read_to_string(path).map_err(|error| {
        ValidationError::new(format!(
            "failed to read {context} '{}': {error}",
            path.display()
        ))
    })
}
