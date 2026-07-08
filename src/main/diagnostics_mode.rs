use gewyvern::gewyc::{RenderFormat, compile_diagnostics_report_file, render_diagnostics_report};

use crate::runtime_events::{EVENT_DIAGNOSTICS_COMPILE_FAILED, EVENT_DIAGNOSTICS_REQUIRES_DSL};
use crate::runtime_logging::log_error_event;
use crate::{Cli, UiLocale};

pub(crate) fn render_diagnostics_mode(cli: &Cli, locale: UiLocale) -> String {
    let path = cli.dsl_path.as_deref().unwrap_or_else(|| {
        log_error_event(
            "diagnostics",
            EVENT_DIAGNOSTICS_REQUIRES_DSL,
            &[],
            "diagnostics mode requires a dsl path",
        );
        eprintln!("{}", locale.msg("diagnostics_requires_dsl"));
        std::process::exit(2);
    });
    let report = compile_diagnostics_report_file(path).unwrap_or_else(|err| {
        log_error_event(
            "diagnostics",
            EVENT_DIAGNOSTICS_COMPILE_FAILED,
            &[("path", path.to_string()), ("error", format!("{err:?}"))],
            "failed to compile diagnostics report",
        );
        eprintln!(
            "{}",
            locale.msgf("binding_diagnostics_failed", &format!("{err:?}"), None)
        );
        std::process::exit(2);
    });
    if cli.json {
        render_diagnostics_report(&report, RenderFormat::Json)
    } else {
        render_diagnostics_report(&report, RenderFormat::Text)
    }
}
