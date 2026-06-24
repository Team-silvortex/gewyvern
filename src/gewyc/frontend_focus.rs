use super::*;

mod json;
mod text;

pub(super) fn frontend_focus_text(focus: FrontendFocus) -> &'static str {
    match focus {
        FrontendFocus::Functions => "functions",
        FrontendFocus::Includes => "includes",
        FrontendFocus::Graph => "graph",
        FrontendFocus::Expansion => "expansion",
    }
}

pub(super) fn frontend_text(frontend: Option<&FrontendReport>) -> String {
    text::frontend_text(frontend)
}

pub(super) fn frontend_report_text(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    text::frontend_report_text(report, focus)
}

pub(super) fn frontend_report_text_compact(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    text::frontend_report_text_compact(report, focus)
}

pub(super) fn frontend_report_json(
    report: &FrontendReport,
    focus: Option<FrontendFocus>,
) -> String {
    json::frontend_report_json(report, focus)
}

pub(super) fn frontend_json(frontend: Option<&FrontendReport>) -> String {
    json::frontend_json(frontend)
}
