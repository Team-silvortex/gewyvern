use super::super::super::FrontendReport;
use super::super::super::explain_support::json_string;

pub(super) fn explain_authoring_context(frontend: &FrontendReport) -> String {
    let module_doc = explain_doc_text(frontend.module_doc.as_deref(), "no module doc");
    let template_doc = explain_doc_text(frontend.template_doc.as_deref(), "no template doc");
    let functions = explain_documented_functions(frontend, "none");
    format!(
        "module_doc={} ; template_doc={} ; documented_functions={}",
        module_doc, template_doc, functions
    )
}

pub(super) fn explain_authoring_context_json(frontend: &FrontendReport) -> String {
    format!(
        "{{\"module_doc\":{},\"template_doc\":{},\"documented_functions\":[{}]}}",
        frontend
            .module_doc
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        frontend
            .template_doc
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        frontend
            .function_nodes
            .iter()
            .filter_map(|node| node.doc.as_ref().map(|_| json_string(&node.name)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn explain_doc_text(doc: Option<&str>, fallback: &str) -> String {
    doc.unwrap_or(fallback).replace('\n', " / ")
}

pub(super) fn explain_documented_functions(frontend: &FrontendReport, fallback: &str) -> String {
    let documented = frontend
        .function_nodes
        .iter()
        .filter_map(|node| node.doc.as_ref().map(|_| node.name.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    if documented.is_empty() {
        fallback.to_string()
    } else {
        documented
    }
}
