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
    if !value.contains('/') && !value.contains('\\') {
        return Err(format!(
            "{flag} requires a filesystem path (for example ./engine or /usr/bin/engine)"
        ));
    }
    Ok(value)
}
