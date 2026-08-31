use gewyvern::gewyc::{RenderFormat, compile_diagnostics_report_file, render_diagnostics_report};
use gewyvern::machine_error::{ErrorCategory, MachineError};

use crate::runtime_events::{EVENT_DIAGNOSTICS_COMPILE_FAILED, EVENT_DIAGNOSTICS_REQUIRES_DSL};
use crate::runtime_logging::log_error_event;
use crate::{Cli, UiLocale};

pub(crate) fn render_diagnostics_mode(cli: &Cli, locale: UiLocale) -> Result<String, MachineError> {
    let path = cli.dsl_path.as_deref().ok_or_else(|| {
        log_error_event(
            "diagnostics",
            EVENT_DIAGNOSTICS_REQUIRES_DSL,
            &[],
            "diagnostics mode requires a dsl path",
        );
        MachineError::new(
            "diagnostics_dsl_required",
            ErrorCategory::Input,
            locale.msg("diagnostics_requires_dsl"),
            false,
            2,
        )
    })?;
    let report = compile_diagnostics_report_file(path).map_err(|error| {
        let detail = format!("{error:?}");
        log_error_event(
            "diagnostics",
            EVENT_DIAGNOSTICS_COMPILE_FAILED,
            &[("path", path.to_string()), ("error", detail.clone())],
            "failed to compile diagnostics report",
        );
        MachineError::new(
            "diagnostics_compile_failed",
            ErrorCategory::Input,
            locale.msgf("binding_diagnostics_failed", &detail, None),
            false,
            2,
        )
    })?;
    Ok(if cli.json {
        render_diagnostics_report(&report, RenderFormat::Json)
    } else {
        render_diagnostics_report(&report, RenderFormat::Text)
    })
}
