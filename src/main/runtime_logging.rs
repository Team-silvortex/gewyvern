use std::fs::{File, OpenOptions, create_dir_all, metadata, remove_file, rename};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
use std::{
    fs::{read_to_string, remove_dir_all},
    thread::sleep,
    time::Duration,
};

const DEFAULT_LOG_MAX_BYTES: usize = 1_048_576;
const DEFAULT_LOG_MAX_FILES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoggingConfig {
    pub(crate) level: LogLevel,
    pub(crate) log_to_stderr: bool,
    pub(crate) log_file: Option<PathBuf>,
    pub(crate) max_bytes: usize,
    pub(crate) max_files: usize,
}

pub(crate) type LogField<'a> = (&'a str, String);

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Warn,
            log_to_stderr: true,
            log_file: None,
            max_bytes: DEFAULT_LOG_MAX_BYTES,
            max_files: DEFAULT_LOG_MAX_FILES,
        }
    }
}

struct RuntimeLogger {
    level: LogLevel,
    log_to_stderr: bool,
    log_file: Option<Mutex<RotatingLogFile>>,
}

static LOGGER: OnceLock<RuntimeLogger> = OnceLock::new();

struct RotatingLogFile {
    path: PathBuf,
    max_bytes: usize,
    max_files: usize,
    file: Option<File>,
}

impl LogLevel {
    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            other => Err(format!(
                "unsupported log level '{other}'; expected error, warn, info, or debug"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
        }
    }

    fn enabled_by(self, configured: Self) -> bool {
        self.rank() <= configured.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warn => 2,
            Self::Info => 3,
            Self::Debug => 4,
        }
    }
}

pub(crate) fn init_runtime_logger(config: LoggingConfig) -> Result<(), String> {
    let log_file = match config.log_file.as_ref() {
        Some(path) => Some(Mutex::new(RotatingLogFile::new(
            path.to_path_buf(),
            config.max_bytes,
            config.max_files,
        )?)),
        None => None,
    };
    let logger = RuntimeLogger {
        level: config.level,
        log_to_stderr: config.log_to_stderr,
        log_file,
    };
    LOGGER
        .set(logger)
        .map_err(|_| "runtime logger was already initialized".to_string())
}

#[allow(dead_code)]
pub(crate) fn log_error(target: &str, message: impl AsRef<str>) {
    log_message(LogLevel::Error, target, message.as_ref());
}

#[allow(dead_code)]
pub(crate) fn log_error_event(target: &str, event: &str, fields: &[LogField<'_>], message: &str) {
    log_message_with_fields(LogLevel::Error, target, event, fields, message);
}

#[allow(dead_code)]
pub(crate) fn log_warn(target: &str, message: impl AsRef<str>) {
    log_message(LogLevel::Warn, target, message.as_ref());
}

pub(crate) fn log_warn_event(target: &str, event: &str, fields: &[LogField<'_>], message: &str) {
    log_message_with_fields(LogLevel::Warn, target, event, fields, message);
}

#[allow(dead_code)]
pub(crate) fn log_info(target: &str, message: impl AsRef<str>) {
    log_message(LogLevel::Info, target, message.as_ref());
}

pub(crate) fn log_info_event(target: &str, event: &str, fields: &[LogField<'_>], message: &str) {
    log_message_with_fields(LogLevel::Info, target, event, fields, message);
}

#[allow(dead_code)]
pub(crate) fn log_debug(target: &str, message: impl AsRef<str>) {
    log_message(LogLevel::Debug, target, message.as_ref());
}

#[allow(dead_code)]
pub(crate) fn log_debug_event(target: &str, event: &str, fields: &[LogField<'_>], message: &str) {
    log_message_with_fields(LogLevel::Debug, target, event, fields, message);
}

fn log_message(level: LogLevel, target: &str, message: &str) {
    log_message_with_fields(level, target, "message", &[], message);
}

fn log_message_with_fields(
    level: LogLevel,
    target: &str,
    event: &str,
    fields: &[LogField<'_>],
    message: &str,
) {
    let Some(logger) = LOGGER.get() else {
        if level.enabled_by(LogLevel::Warn) {
            eprintln!("{}", format_record(level, target, event, fields, message));
        }
        return;
    };
    logger.write(level, target, event, fields, message);
}

impl RuntimeLogger {
    fn write(
        &self,
        level: LogLevel,
        target: &str,
        event: &str,
        fields: &[LogField<'_>],
        message: &str,
    ) {
        if !level.enabled_by(self.level) {
            return;
        }
        let record = format_record(level, target, event, fields, message);
        if self.log_to_stderr {
            eprintln!("{record}");
        }
        if let Some(file) = self.log_file.as_ref() {
            if let Ok(mut file) = file.lock() {
                let _ = file.write_record(&record);
            }
        }
    }
}

impl RotatingLogFile {
    fn new(path: PathBuf, max_bytes: usize, max_files: usize) -> Result<Self, String> {
        Ok(Self {
            file: Some(open_log_file(&path)?),
            path,
            max_bytes,
            max_files,
        })
    }

    fn write_record(&mut self, record: &str) -> Result<(), String> {
        self.rotate_if_needed(record.len() + 1)?;
        if let Some(file) = self.file.as_mut() {
            writeln!(file, "{record}").map_err(|err| {
                format!(
                    "failed to write runtime log '{}': {err}",
                    self.path.display()
                )
            })?;
        }
        Ok(())
    }

    fn rotate_if_needed(&mut self, pending_bytes: usize) -> Result<(), String> {
        if self.max_bytes == 0 {
            return Ok(());
        }
        let current_bytes = metadata(&self.path)
            .map(|meta| meta.len() as usize)
            .unwrap_or(0);
        if current_bytes + pending_bytes <= self.max_bytes {
            return Ok(());
        }

        if let Some(mut file) = self.file.take() {
            let _ = file.flush();
        }

        self.rotate_archives()?;
        self.file = Some(open_log_file(&self.path)?);
        Ok(())
    }

    fn rotate_archives(&self) -> Result<(), String> {
        if self.max_files > 0 {
            let oldest = rotated_log_path(&self.path, self.max_files);
            if oldest.exists() {
                remove_file(&oldest).map_err(|err| {
                    format!(
                        "failed to remove rotated runtime log '{}': {err}",
                        oldest.display()
                    )
                })?;
            }

            for index in (1..self.max_files).rev() {
                let source = rotated_log_path(&self.path, index);
                if source.exists() {
                    let target = rotated_log_path(&self.path, index + 1);
                    rename(&source, &target).map_err(|err| {
                        format!(
                            "failed to rotate runtime log '{}' to '{}': {err}",
                            source.display(),
                            target.display()
                        )
                    })?;
                }
            }

            if self.path.exists() {
                let first = rotated_log_path(&self.path, 1);
                rename(&self.path, &first).map_err(|err| {
                    format!(
                        "failed to rotate runtime log '{}' to '{}': {err}",
                        self.path.display(),
                        first.display()
                    )
                })?;
            }
        } else if self.path.exists() {
            remove_file(&self.path).map_err(|err| {
                format!(
                    "failed to truncate oversized runtime log '{}': {err}",
                    self.path.display()
                )
            })?;
        }

        Ok(())
    }
}

fn format_record(
    level: LogLevel,
    target: &str,
    event: &str,
    fields: &[LogField<'_>],
    message: &str,
) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let sanitized = message.replace('\n', " | ");
    let event = sanitize_field_value(event);
    let field_suffix = format_fields(fields);
    format!(
        "ts={}.{:03} level={} target={} event={}{} msg={}",
        ts.as_secs(),
        ts.subsec_millis(),
        level.label(),
        target,
        event,
        field_suffix,
        sanitized
    )
}

fn format_fields(fields: &[LogField<'_>]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    fields
        .iter()
        .map(|(key, value)| {
            format!(
                " {}={}",
                sanitize_field_key(key),
                sanitize_field_value(value)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn sanitize_field_key(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "field".to_string()
    } else {
        sanitized
    }
}

fn sanitize_field_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "|")
        .replace(' ', "_")
        .replace('"', "'")
}

fn open_log_file(path: &Path) -> Result<File, String> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create runtime log directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            format!(
                "failed to open runtime log file '{}': {err}",
                path.display()
            )
        })
}

fn rotated_log_path(path: &Path, index: usize) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}.{index}"))
        .unwrap_or_else(|| format!("runtime.log.{index}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gewyvern-runtime-logging-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn log_level_parser_accepts_expected_values() {
        assert_eq!(LogLevel::from_str("error").unwrap(), LogLevel::Error);
        assert_eq!(LogLevel::from_str("warn").unwrap(), LogLevel::Warn);
        assert_eq!(LogLevel::from_str("info").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::from_str("debug").unwrap(), LogLevel::Debug);
        assert!(LogLevel::from_str("trace").is_err());
    }

    #[test]
    fn runtime_logger_file_output_filters_by_level() {
        let root = temp_dir("file");
        let path = root.join("runtime.log");
        let logger = RuntimeLogger {
            level: LogLevel::Warn,
            log_to_stderr: false,
            log_file: Some(Mutex::new(
                RotatingLogFile::new(path.clone(), DEFAULT_LOG_MAX_BYTES, DEFAULT_LOG_MAX_FILES)
                    .unwrap(),
            )),
        };

        logger.write(
            LogLevel::Info,
            "test",
            "filtered",
            &[],
            "info should be filtered",
        );
        logger.write(
            LogLevel::Warn,
            "test",
            "accepted",
            &[],
            "warn should be written",
        );
        logger.write(
            LogLevel::Error,
            "test",
            "accepted",
            &[],
            "error should be written",
        );

        let content = read_to_string(&path).unwrap();
        assert!(!content.contains("info should be filtered"));
        assert!(content.contains("warn should be written"));
        assert!(content.contains("error should be written"));

        remove_dir_all(&root).unwrap();
    }

    #[test]
    fn runtime_logger_rotates_and_retains_recent_files() {
        let root = temp_dir("rotate");
        let path = root.join("runtime.log");
        let logger = RuntimeLogger {
            level: LogLevel::Debug,
            log_to_stderr: false,
            log_file: Some(Mutex::new(
                RotatingLogFile::new(path.clone(), 80, 2).unwrap(),
            )),
        };

        logger.write(LogLevel::Info, "rotate", "entry", &[], "entry-1");
        sleep(Duration::from_millis(2));
        logger.write(LogLevel::Info, "rotate", "entry", &[], "entry-2");
        sleep(Duration::from_millis(2));
        logger.write(LogLevel::Info, "rotate", "entry", &[], "entry-3");
        sleep(Duration::from_millis(2));
        logger.write(LogLevel::Info, "rotate", "entry", &[], "entry-4");

        let current = read_to_string(&path).unwrap();
        let rotated_1 = read_to_string(rotated_log_path(&path, 1)).unwrap();
        let rotated_2 = read_to_string(rotated_log_path(&path, 2)).unwrap();

        assert!(current.contains("entry-4"));
        assert!(rotated_1.contains("entry-3"));
        assert!(rotated_2.contains("entry-2"));
        assert!(!rotated_2.contains("entry-1"));
        assert!(!rotated_log_path(&path, 3).exists());

        remove_dir_all(&root).unwrap();
    }

    #[test]
    fn format_record_includes_event_and_fields() {
        let record = format_record(
            LogLevel::Info,
            "serve",
            "session_start",
            &[
                ("socket", "tcp:127.0.0.1:9910".to_string()),
                ("max sessions", "32 clients".to_string()),
            ],
            "accepted session",
        );

        assert!(record.contains("target=serve"));
        assert!(record.contains("event=session_start"));
        assert!(record.contains("socket=tcp:127.0.0.1:9910"));
        assert!(record.contains("max_sessions=32_clients"));
        assert!(record.contains("msg=accepted session"));
    }
}
