use super::*;

pub(super) fn http_transactions_text(transactions: &[HttpTransactionView]) -> String {
    let locale = UiLocale::detect();
    if transactions.is_empty() {
        return locale.none().into();
    }

    let mut text = String::new();
    for (index, tx) in transactions.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        text.push_str("http_transaction#");
        text.push_str(&tx.id.0.to_string());
        text.push_str(": client=");
        if let Some(process) = tx.client_process.as_ref() {
            let _ = write!(text, "{}(pid={})", process.comm, process.pid);
        } else {
            text.push_str(locale.none());
        }
        text.push_str(" server=");
        if let Some(process) = tx.server_process.as_ref() {
            let _ = write!(text, "{}(pid={})", process.comm, process.pid);
        } else {
            text.push_str(locale.none());
        }
        text.push_str(" verdict=");
        text.push_str(http_transaction_verdict_label(&tx.verdict));
        text.push_str(" severity=");
        text.push_str(
            tx.severity
                .as_ref()
                .map(module_severity_label)
                .unwrap_or_else(|| locale.none()),
        );
        text.push_str(" degraded=");
        text.push_str(if tx.degraded { "true" } else { "false" });
        text.push_str(" suspect_sides=");
        if tx.suspect_sides.is_empty() {
            text.push_str(locale.none());
        } else {
            for (side_index, side) in tx.suspect_sides.iter().enumerate() {
                if side_index > 0 {
                    text.push(',');
                }
                text.push_str(http_suspect_side_label(side));
            }
        }
        text.push_str(" phases=");
        if tx.phases.is_empty() {
            text.push_str(locale.none());
        } else {
            push_joined_strings(&mut text, &tx.phases, ",");
        }
        text.push_str(" components=");
        if tx.components.is_empty() {
            text.push_str(locale.none());
        } else {
            for (component_index, component) in tx.components.iter().enumerate() {
                if component_index > 0 {
                    text.push(',');
                }
                text.push_str(http_component_kind_label(&component.kind));
                text.push(':');
                text.push_str(&operation_label(&component.operation));
            }
        }
        text.push_str(" summaries=");
        if tx.finding_summaries.is_empty() {
            push_joined_strings(&mut text, &tx.summaries, "|");
        } else {
            push_joined_strings(&mut text, &tx.finding_summaries, "|");
        }
    }
    text
}

pub(super) fn http_transactions_json(transactions: &[HttpTransactionView]) -> String {
    let mut json = String::from("[");
    for (index, transaction) in transactions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_http_transaction_json(&mut json, transaction);
    }
    json.push(']');
    json
}
