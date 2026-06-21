use crate::certificate_state_cli::try_run_certificate_state_command;
use crate::cli::Cli;
use crate::external_analysis::set_external_analysis_config;
use crate::runtime_config::{RuntimeConfigFile, apply_runtime_path_overrides, load_runtime_config};
use crate::runtime_events::{
    EVENT_LEGACY_CONFIG_COPIED, EVENT_LEGACY_ENTRIES_MIGRATED, EVENT_RUNTIME_CONFIG_LOADED,
    EVENT_RUNTIME_ROOTS_PREPARED,
};
use crate::runtime_logging::{init_runtime_logger, log_info_event, log_warn_event};
use crate::runtime_migration::{RuntimeMigrationReport, prepare_runtime_layout};

pub(crate) fn bootstrap_cli(args: Vec<String>) -> Cli {
    let migration_report = prepare_runtime_layout().unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let runtime_config = load_runtime_config().unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    apply_runtime_path_overrides(&runtime_config);
    if let Some(exit_code) = try_run_certificate_state_command(&args) {
        std::process::exit(exit_code);
    }
    let cli = Cli::from_args_with_defaults(args, runtime_config.defaults.clone()).unwrap_or_else(
        |message| {
            eprintln!("{message}");
            std::process::exit(2);
        },
    );
    initialize_runtime_logging(&cli);
    log_runtime_bootstrap(&runtime_config, &migration_report);
    set_external_analysis_config(cli.external_analysis_config());
    cli
}

fn initialize_runtime_logging(cli: &Cli) {
    let mut logging_config = cli.logging_config();
    if logging_config.log_file.is_none() {
        logging_config.log_file = Some(gewyvern::runtime_layout::default_runtime_log_path());
    }
    init_runtime_logger(logging_config).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
}

fn log_runtime_bootstrap(
    runtime_config: &RuntimeConfigFile,
    migration_report: &RuntimeMigrationReport,
) {
    if let Some(path) = runtime_config.source_path.as_ref() {
        if runtime_config.used_legacy_path {
            log_warn_event(
                "config",
                EVENT_RUNTIME_CONFIG_LOADED,
                &[("path", path.display().to_string())],
                "loaded legacy runtime config",
            );
        } else {
            log_info_event(
                "config",
                EVENT_RUNTIME_CONFIG_LOADED,
                &[("path", path.display().to_string())],
                "loaded runtime config",
            );
        }
    }
    if !migration_report.created_roots.is_empty() {
        log_info_event(
            "startup",
            EVENT_RUNTIME_ROOTS_PREPARED,
            &[(
                "roots",
                migration_report
                    .created_roots
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )],
            "prepared runtime roots",
        );
    }
    if let Some(path) = migration_report.copied_config_to.as_ref() {
        log_info_event(
            "startup",
            EVENT_LEGACY_CONFIG_COPIED,
            &[("path", path.display().to_string())],
            "copied legacy runtime config into standard root",
        );
    }
    if migration_report.copied_protocol_entries > 0
        || migration_report.copied_dsl_entries > 0
        || migration_report.copied_certificate_entries > 0
        || migration_report.copied_certificate_state_entries > 0
    {
        log_info_event(
            "startup",
            EVENT_LEGACY_ENTRIES_MIGRATED,
            &[
                (
                    "protocols",
                    migration_report.copied_protocol_entries.to_string(),
                ),
                ("dsl", migration_report.copied_dsl_entries.to_string()),
                (
                    "certificates",
                    migration_report.copied_certificate_entries.to_string(),
                ),
                (
                    "certificate_state",
                    migration_report
                        .copied_certificate_state_entries
                        .to_string(),
                ),
            ],
            "migrated legacy runtime entries",
        );
    }
}
