use etragon::provider::{
    AnalysisSource, ChatCompletionsProvider, MAX_CONFIG_BYTES, MAX_INPUT_BYTES, ProviderConfig,
    ProviderError,
    batch::{BatchOptions, analyze_target_batch},
    targets::TargetIndexSource,
};
use silvortex_bounded_io::read_bounded_utf8_regular_file;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(super) fn analyze_provider_json(path: &str, config_path: &str) -> Result<String, String> {
    let config = load_provider_config(config_path)?;
    let input = if path == "-" {
        let mut input = String::new();
        io::stdin()
            .take(MAX_INPUT_BYTES as u64 + 1)
            .read_to_string(&mut input)
            .map_err(|_| ProviderError::InvalidInput.to_string())?;
        if input.len() > MAX_INPUT_BYTES {
            return Err(ProviderError::TooLarge.to_string());
        }
        input
    } else {
        read_bounded_utf8_regular_file(Path::new(path), MAX_INPUT_BYTES as u64)
            .map_err(|_| ProviderError::InvalidInput.to_string())?
    };
    let provider = ChatCompletionsProvider::new(config).map_err(|e| e.to_string())?;
    let output = provider.analyze_json(&input).map_err(|e| e.to_string())?;
    Ok(etragon::engine_output_json(&output))
}

pub(super) fn load_provider_config(config_path: &str) -> Result<ProviderConfig, String> {
    let config = read_bounded_utf8_regular_file(Path::new(config_path), MAX_CONFIG_BYTES)
        .map_err(|_| ProviderError::InvalidConfig.to_string())?;
    ProviderConfig::from_json(&config).map_err(|e| e.to_string())
}

struct UrlOptions {
    config: ProviderConfig,
    source_ca: Option<PathBuf>,
    batch: BatchOptions,
}

fn parse_url_options(args: &[String], batch_mode: bool) -> Result<UrlOptions, String> {
    let mut provider_path = None;
    let mut source_ca = None;
    let mut batch = BatchOptions::default();
    let mut seen = std::collections::HashSet::new();
    for pair in args.chunks(2) {
        if pair.len() != 2 || !seen.insert(pair[0].as_str()) {
            return Err("etragon_provider_invalid_options".into());
        }
        match pair[0].as_str() {
            "--provider" => provider_path = Some(pair[1].as_str()),
            "--source-ca" => source_ca = Some(PathBuf::from(&pair[1])),
            "--filter" if batch_mode => batch.filter = Some(pair[1].clone()),
            "--max-targets" if batch_mode => {
                batch.max_targets = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_target_limit")?
            }
            "--max-inferences" if batch_mode => {
                batch.max_inferences = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_inference_limit")?
            }
            _ => return Err("etragon_provider_unsupported_option".into()),
        }
    }
    batch.validate().map_err(|e| e.to_string())?;
    let config = load_provider_config(provider_path.ok_or("etragon_provider_config_required")?)?;
    Ok(UrlOptions {
        config,
        source_ca,
        batch,
    })
}

pub(super) fn analyze_provider_url(
    url: &str,
    args: &[String],
    batch_mode: bool,
) -> Result<super::cli_commands::CliOutput, String> {
    let options = parse_url_options(args, batch_mode)?;
    let provider = ChatCompletionsProvider::new(options.config).map_err(|e| e.to_string())?;
    if batch_mode {
        let source =
            TargetIndexSource::new(url.into(), options.source_ca).map_err(|e| e.to_string())?;
        let report = analyze_target_batch(&source, &provider, &options.batch)
            .map_err(|e| format!("etragon_provider_batch_rejected: {e}"))?;
        Ok(super::cli_commands::CliOutput {
            exit_code: if report.complete { 0 } else { 2 },
            output: report.to_json().map_err(|e| e.to_string())?,
        })
    } else {
        let source =
            AnalysisSource::new(url.into(), options.source_ca).map_err(|e| e.to_string())?;
        let input = source.fetch_json().map_err(|e| {
            e.to_string()
                .replace("etragon_provider_", "etragon_source_")
        })?;
        let output = provider.analyze_json(&input).map_err(|e| e.to_string())?;
        Ok(super::cli_commands::CliOutput {
            output: etragon::engine_output_json(&output),
            exit_code: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_options_reject_unknown_duplicate_invalid_or_resident_only_flags_before_config_io() {
        for flags in [
            vec!["--provider", "missing", "--provider", "missing"],
            vec!["--max-inferences", "0"],
            vec!["--max-inferences", "9"],
            vec!["--max-targets", "65"],
            vec!["--filter", ""],
            vec!["--filter", "bad\n"],
            vec!["--bind", "127.0.0.1:0"],
            vec!["--interval-ms", "1000"],
            vec!["--state", "old-state"],
            vec!["--daemon-state", "old-state"],
            vec!["--source-ca"],
        ] {
            let args = flags.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert!(parse_url_options(&args, true).is_err());
        }
        for flag in ["--max-inferences", "--max-targets", "--filter"] {
            assert_eq!(
                parse_url_options(&[flag.into(), "1".into()], false)
                    .err()
                    .unwrap(),
                "etragon_provider_unsupported_option"
            );
        }
    }
}
