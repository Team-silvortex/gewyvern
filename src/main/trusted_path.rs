pub(super) fn next_trusted_path_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, String> {
    let value = args
        .next()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    if value.trim().is_empty() || value.starts_with("--") {
        return Err(format!("{flag} requires a non-empty path value"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{flag} requires a path without control characters"));
    }
    if !value.contains('/') && !value.contains('\\') {
        return Err(format!(
            "{flag} requires a filesystem path (for example ./engine or /usr/bin/engine)"
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::next_trusted_path_value;

    fn consume_value(values: Vec<&str>, flag: &str) -> Result<String, String> {
        let mut iter = values.into_iter().map(String::from);
        next_trusted_path_value(&mut iter, flag)
    }

    #[test]
    fn rejects_control_characters_in_trusted_path_value() {
        assert!(consume_value(vec!["engine\nbin"], "--external-engine-bin").is_err());
    }
}
