use super::*;

pub(super) fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|err| format!("failed to read stdin: {err}"))?;
        return Ok(buffer);
    }

    fs::read_to_string(path).map_err(|err| format!("failed to read '{}': {err}", path))
}

pub(super) fn read_url(url: &str) -> Result<String, String> {
    let (host, port, path) = parse_http_url(url)?;
    http_get(&host, port, &path)
}

pub(super) fn http_get(host: &str, port: u16, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect((host, port))
        .map_err(|err| format!("failed to connect to {}:{}: {err}", host, port))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("failed to configure read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("failed to configure write timeout: {err}"))?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n\r\n",
        path, host
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("failed to send request: {err}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read response: {err}"))?;

    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid HTTP response: missing header separator".to_string())?;
    let status_line = head
        .lines()
        .next()
        .ok_or_else(|| "invalid HTTP response: missing status line".to_string())?;
    if !status_line.contains(" 200 ") {
        return Err(format!("unexpected HTTP response: {status_line}"));
    }
    Ok(body.to_string())
}

pub(super) fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "only http:// URLs are currently supported".to_string())?;
    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, path)) => (host_port, format!("/{}", path)),
        None => (rest, "/".to_string()),
    };
    if host_port.is_empty() {
        return Err("http URL is missing a host".to_string());
    }
    let (host, port) = match host_port.split_once(':') {
        Some((host, port)) => {
            let parsed = port
                .parse::<u16>()
                .map_err(|_| format!("invalid port in URL: {}", port))?;
            (host.to_string(), parsed)
        }
        None => (host_port.to_string(), 80),
    };
    if host.is_empty() {
        return Err("http URL is missing a host".to_string());
    }
    Ok((host, port, path))
}

pub(super) fn extract_target_path_segments(targets_json: &str) -> Result<Vec<String>, String> {
    let mut rest = targets_json;
    let mut segments = Vec::new();
    let needle = "\"path_segment\":\"";
    while let Some(index) = rest.find(needle) {
        let start = index + needle.len();
        let tail = &rest[start..];
        let end = tail
            .find('"')
            .ok_or_else(|| "invalid targets payload: unterminated path_segment".to_string())?;
        segments.push(tail[..end].to_string());
        rest = &tail[end..];
    }
    if segments.is_empty() {
        return Err("targets payload did not contain any path_segment values".to_string());
    }
    Ok(segments)
}

pub(super) fn extract_named_string_fields(input: &str, key: &str) -> Vec<String> {
    let mut rest = input;
    let mut values = Vec::new();
    let needle = format!("\"{}\":\"", key);
    while let Some(index) = rest.find(&needle) {
        let start = index + needle.len();
        let tail = &rest[start..];
        let Some(end) = tail.find('"') else {
            break;
        };
        values.push(tail[..end].to_string());
        rest = &tail[end..];
    }
    values
}

pub(super) fn extract_named_numeric_fields(input: &str, key: &str) -> Vec<String> {
    let mut rest = input;
    let mut values = Vec::new();
    let needle = format!("\"{}\":", key);
    while let Some(index) = rest.find(&needle) {
        let start = index + needle.len();
        let tail = &rest[start..];
        let end = tail
            .find(|ch: char| [',', '}', ']'].contains(&ch))
            .unwrap_or(tail.len());
        let value = tail[..end].trim();
        if !value.is_empty() && value != "null" {
            values.push(value.to_string());
        }
        rest = &tail[end..];
    }
    values
}

#[derive(Clone, Debug, Default)]
pub(super) struct RecommendationHints {
    pub(super) support_score: Option<f64>,
    pub(super) train_count: Option<u64>,
    pub(super) last_trained_unix_ms: Option<u128>,
    pub(super) score_margin: Option<f64>,
    pub(super) runner_up_label: Option<String>,
    pub(super) runner_up_score: Option<f64>,
    pub(super) runner_up_train_count: Option<u64>,
    pub(super) runner_up_last_trained_unix_ms: Option<u128>,
}

#[derive(Clone, Debug)]
pub(super) struct MergedRecommendationEntry {
    pub(super) name: String,
    pub(super) producer_stage: String,
    pub(super) producer_pass: String,
    pub(super) count: usize,
    pub(super) hints: RecommendationHints,
}

pub(super) fn extract_json_value(input: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let index = input.find(&needle)?;
    let mut tail = &input[index + needle.len()..];
    tail = tail.trim_start();
    let first = tail.chars().next()?;
    match first {
        '{' => {
            let mut depth = 0usize;
            for (offset, ch) in tail.char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(tail[..=offset].to_string());
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        '[' => {
            let mut depth = 0usize;
            for (offset, ch) in tail.char_indices() {
                match ch {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(tail[..=offset].to_string());
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        'n' if tail.starts_with("null") => Some("null".to_string()),
        '"' => {
            let rest = &tail[1..];
            let end = rest.find('"')?;
            Some(format!("\"{}\"", &rest[..end]))
        }
        _ => {
            let end = tail
                .find(|ch: char| [',', '}', ']'].contains(&ch))
                .unwrap_or(tail.len());
            Some(tail[..end].trim().to_string())
        }
    }
}

pub(super) fn split_top_level_json_items(array_json: &str) -> Vec<String> {
    let inner = array_json
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    if inner.trim().is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    for (index, ch) in inner.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if brace_depth == 0 && bracket_depth == 0 => {
                items.push(inner[start..index].trim().to_string());
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(inner[start..].trim().to_string());
    items.into_iter().filter(|item| !item.is_empty()).collect()
}

pub(super) fn unescape_json_string(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some(other) => output.push(other),
                None => output.push('\\'),
            }
        } else {
            output.push(ch);
        }
    }
    output
}

pub(super) fn parse_json_string_value(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed == "null" {
        return None;
    }
    Some(unescape_json_string(trimmed.trim_matches('"')))
}

pub(super) fn escape_json_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
