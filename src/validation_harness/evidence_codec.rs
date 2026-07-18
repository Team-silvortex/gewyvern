use std::collections::BTreeMap;

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
