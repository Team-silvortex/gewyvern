use crate::history_view::render_history_index;
use crate::runtime_events::EVENT_HISTORY_RENDER_FAILED;
use crate::runtime_logging::log_error_event;
use crate::{
    Cli, UiLocale, list_entries_json, list_entries_text, list_protocols_json, list_protocols_text,
    write_or_print,
};
use gewyvern::machine_error::{ErrorCategory, MachineError};

pub(crate) fn handle_cli_preflight(cli: &Cli, locale: UiLocale) -> Result<bool, MachineError> {
    if cli.list_protocols {
        let rendered = if cli.json {
            list_protocols_json()
        } else {
            list_protocols_text()
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale)?;
        return Ok(true);
    }

    if cli.list_history {
        let rendered = render_history_index(cli.json).map_err(|message| {
            log_error_event(
                "history",
                EVENT_HISTORY_RENDER_FAILED,
                &[("error", message.clone())],
                "failed to render history index",
            );
            MachineError::new(
                "history_render_failed",
                ErrorCategory::Runtime,
                message,
                false,
                2,
            )
        })?;
        write_or_print(&rendered, cli.out_path.as_deref(), locale)?;
        return Ok(true);
    }

    if let Some(protocol) = cli.list_entries.as_deref() {
        let rendered = if cli.json {
            list_entries_json(protocol)
        } else {
            list_entries_text(protocol)
        }
        .ok_or_else(|| {
            MachineError::new(
                "cli_unsupported_protocol",
                ErrorCategory::Input,
                locale.msgf("unsupported_protocol", protocol, None),
                false,
                2,
            )
        })?;
        write_or_print(&rendered, cli.out_path.as_deref(), locale)?;
        return Ok(true);
    }

    Ok(false)
}
