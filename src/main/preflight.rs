use crate::history_view::render_history_index;
use crate::runtime_events::EVENT_HISTORY_RENDER_FAILED;
use crate::runtime_logging::log_error_event;
use crate::{
    Cli, UiLocale, list_entries_json, list_entries_text, list_protocols_json, list_protocols_text,
    write_or_print,
};

pub(crate) fn handle_cli_preflight(cli: &Cli, locale: UiLocale) -> bool {
    if cli.list_protocols {
        let rendered = if cli.json {
            list_protocols_json()
        } else {
            list_protocols_text()
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return true;
    }

    if cli.list_history {
        let rendered = render_history_index(cli.json).unwrap_or_else(|message| {
            log_error_event(
                "history",
                EVENT_HISTORY_RENDER_FAILED,
                &[("error", message.clone())],
                "failed to render history index",
            );
            eprintln!("{message}");
            std::process::exit(2);
        });
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return true;
    }

    if let Some(protocol) = cli.list_entries.as_deref() {
        let rendered = if cli.json {
            list_entries_json(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        } else {
            list_entries_text(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return true;
    }

    false
}
