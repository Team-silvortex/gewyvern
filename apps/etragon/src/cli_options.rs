use super::*;

pub(super) fn default_python_config() -> PythonWorkerConfig {
    PythonWorkerConfig {
        python_bin: "python3".into(),
        worker_script: default_python_worker_script(),
        state_file: None,
    }
}

pub(super) fn parse_native_options(
    args: &[String],
) -> Result<(NativeLearningConfig, usize), String> {
    let mut config = NativeLearningConfig::default();
    let mut index = 0;
    while let Some(option) = args.get(index) {
        if option != "--state" {
            break;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| "missing value for --state".to_string())?;
        config.state_file = Some(PathBuf::from(value));
        index += 2;
    }
    Ok((config, index))
}

pub(super) fn parse_native_train_options(
    args: &[String],
) -> Result<(f64, NativeLearningConfig), String> {
    let mut weight = 1.0;
    let mut config = NativeLearningConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--weight" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --weight".to_string())?;
                weight = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid value for --weight: {value}"))?;
                if !weight.is_finite() || weight <= 0.0 {
                    return Err("--weight must be a finite number greater than 0".to_string());
                }
                index += 2;
            }
            "--state" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --state".to_string())?;
                config.state_file = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err(format!("unknown option for train command: {other}")),
        }
    }
    Ok((weight, config))
}

pub(super) fn parse_native_batch_train_options(
    args: &[String],
) -> Result<(Option<String>, f64, NativeLearningConfig), String> {
    let mut filter_prefix = None;
    let mut weight = 1.0;
    let mut config = NativeLearningConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--filter" => {
                filter_prefix = Some(required_option_value(args, index, "--filter")?.to_string());
                index += 2;
            }
            "--weight" => {
                weight = parse_positive_f64(required_option_value(args, index, "--weight")?)?;
                index += 2;
            }
            "--state" => {
                config.state_file = Some(PathBuf::from(required_option_value(
                    args, index, "--state",
                )?));
                index += 2;
            }
            other => return Err(format!("unknown option for train command: {other}")),
        }
    }
    Ok((filter_prefix, weight, config))
}

pub(super) fn parse_native_watch_options(
    args: &[String],
) -> Result<(u64, usize, Option<String>, NativeLearningConfig), String> {
    let mut interval_ms = 1000;
    let mut cycles = 0;
    let mut filter_prefix = None;
    let mut config = NativeLearningConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--interval-ms" => {
                let value = required_option_value(args, index, "--interval-ms")?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid interval for --interval-ms: {value}"))?;
                index += 2;
            }
            "--cycles" => {
                let value = required_option_value(args, index, "--cycles")?;
                cycles = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid cycle count for --cycles: {value}"))?;
                index += 2;
            }
            "--filter" => {
                filter_prefix = Some(required_option_value(args, index, "--filter")?.to_string());
                index += 2;
            }
            "--state" => {
                config.state_file = Some(PathBuf::from(required_option_value(
                    args, index, "--state",
                )?));
                index += 2;
            }
            other => return Err(format!("unknown option for watch command: {other}")),
        }
    }
    Ok((interval_ms, cycles, filter_prefix, config))
}

pub(super) type NativeDaemonOptions = (
    String,
    u64,
    Option<String>,
    NativeLearningConfig,
    Option<PathBuf>,
);

pub(super) fn parse_native_daemon_options(args: &[String]) -> Result<NativeDaemonOptions, String> {
    let mut bind_addr = "127.0.0.1:4321".to_string();
    let mut interval_ms = 1000;
    let mut filter_prefix = None;
    let mut config = NativeLearningConfig::default();
    let mut daemon_state_file = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bind" => {
                bind_addr = required_option_value(args, index, "--bind")?.to_string();
                index += 2;
            }
            "--interval-ms" => {
                let value = required_option_value(args, index, "--interval-ms")?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid interval for --interval-ms: {value}"))?;
                index += 2;
            }
            "--filter" => {
                filter_prefix = Some(required_option_value(args, index, "--filter")?.to_string());
                index += 2;
            }
            "--state" => {
                config.state_file = Some(PathBuf::from(required_option_value(
                    args, index, "--state",
                )?));
                index += 2;
            }
            "--daemon-state" => {
                daemon_state_file = Some(PathBuf::from(required_option_value(
                    args,
                    index,
                    "--daemon-state",
                )?));
                index += 2;
            }
            other => return Err(format!("unknown option for daemon command: {other}")),
        }
    }
    Ok((
        bind_addr,
        interval_ms,
        filter_prefix,
        config,
        daemon_state_file,
    ))
}

fn required_option_value<'a>(
    args: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("missing value for {option}"))
}

fn parse_positive_f64(value: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("invalid numeric value: {value}"))?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err("numeric value must be a finite number greater than 0".to_string());
    }
    Ok(parsed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TrainingEvent {
    pub(super) label: String,
    pub(super) weight: String,
    pub(super) trained_unix_ms: u128,
    pub(super) scope: String,
}

pub(super) const TRAINING_HISTORY_LIMIT: usize = 8;
pub(super) const DAEMON_STATE_TARGET_LIMIT: usize = 24;
pub(super) const DAEMON_STATE_INPUT_JSON_LIMIT: usize = 32 * 1024;
pub(super) const DAEMON_STATE_TARGET_OUTPUT_JSON_LIMIT: usize = 16 * 1024;
pub(super) const DAEMON_STATE_LATEST_OUTPUT_JSON_LIMIT: usize = 64 * 1024;

pub(super) fn parse_train_options(
    args: &[String],
) -> Result<(Option<String>, f64, PythonWorkerConfig), String> {
    let mut filter_prefix = None;
    let mut weight = 1.0f64;
    let mut config = default_python_config();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--filter" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --filter".to_string())?;
                filter_prefix = Some(value.clone());
                index += 2;
            }
            "--weight" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --weight".to_string())?;
                weight = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid value for --weight: {value}"))?;
                if weight <= 0.0 {
                    return Err("--weight must be > 0".to_string());
                }
                index += 2;
            }
            "--python-worker" | "--python-bin" | "--python-state" => {
                index += consume_python_worker_option(args, index, &mut config)?;
            }
            other => return Err(format!("unknown option for train command: {other}")),
        }
    }
    Ok((filter_prefix, weight, config))
}

pub(super) fn parse_python_worker_options(
    args: &[String],
) -> Result<(PythonWorkerConfig, usize), String> {
    let mut config = default_python_config();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--python-worker" | "--python-bin" | "--python-state" => {
                index += consume_python_worker_option(args, index, &mut config)?;
            }
            _ => break,
        }
    }
    Ok((config, index))
}

pub(super) fn parse_watch_options(
    args: &[String],
) -> Result<(u64, usize, Option<String>, PythonWorkerConfig), String> {
    let mut interval_ms = 1000u64;
    let mut cycles = 0usize;
    let mut filter_prefix = None;
    let mut config = default_python_config();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--interval-ms" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --interval-ms".to_string())?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid interval for --interval-ms: {value}"))?;
                index += 2;
            }
            "--cycles" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --cycles".to_string())?;
                cycles = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid cycle count for --cycles: {value}"))?;
                index += 2;
            }
            "--filter" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --filter".to_string())?;
                filter_prefix = Some(value.clone());
                index += 2;
            }
            "--python-worker" | "--python-bin" | "--python-state" => {
                index += consume_python_worker_option(args, index, &mut config)?;
            }
            other => return Err(format!("unknown option for watch command: {other}")),
        }
    }
    Ok((interval_ms, cycles, filter_prefix, config))
}

pub(super) type DaemonOptions = (
    String,
    u64,
    Option<String>,
    PythonWorkerConfig,
    Option<PathBuf>,
);

pub(super) fn parse_daemon_options(args: &[String]) -> Result<DaemonOptions, String> {
    let mut bind_addr = "127.0.0.1:4321".to_string();
    let mut interval_ms = 1000u64;
    let mut filter_prefix = None;
    let mut config = default_python_config();
    let mut daemon_state_file = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bind" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --bind".to_string())?;
                bind_addr = value.clone();
                index += 2;
            }
            "--interval-ms" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --interval-ms".to_string())?;
                interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid interval for --interval-ms: {value}"))?;
                index += 2;
            }
            "--filter" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --filter".to_string())?;
                filter_prefix = Some(value.clone());
                index += 2;
            }
            "--python-worker" | "--python-bin" | "--python-state" => {
                index += consume_python_worker_option(args, index, &mut config)?;
            }
            "--daemon-state" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --daemon-state".to_string())?;
                daemon_state_file = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err(format!("unknown option for daemon command: {other}")),
        }
    }
    Ok((
        bind_addr,
        interval_ms,
        filter_prefix,
        config,
        daemon_state_file,
    ))
}

fn consume_python_worker_option(
    args: &[String],
    index: usize,
    config: &mut PythonWorkerConfig,
) -> Result<usize, String> {
    match args[index].as_str() {
        "--python-worker" => {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "missing value for --python-worker".to_string())?;
            config.worker_script = value.into();
        }
        "--python-bin" => {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "missing value for --python-bin".to_string())?;
            config.python_bin = value.clone();
        }
        "--python-state" => {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "missing value for --python-state".to_string())?;
            config.state_file = Some(value.into());
        }
        other => return Err(format!("unsupported python worker option: {other}")),
    }
    Ok(2)
}
