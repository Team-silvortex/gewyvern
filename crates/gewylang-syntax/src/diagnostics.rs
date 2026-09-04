pub fn pipeline_available_steps_message() -> &'static str {
    "Available pipeline steps: template, window, reason, reason_model, fragment, operation, program_model, param, evidence, program_rule, reason_rule, include, use."
}

pub fn pipeline_declared_functions_message(function_names: &[String]) -> String {
    if function_names.is_empty() {
        return "No reusable pipeline functions are declared in this module.".to_string();
    }
    format!(
        "Declared pipeline functions in this module: {}.",
        function_names.join(", ")
    )
}

pub fn pipeline_declared_params_message(
    function_signature: &str,
    param_names: &[String],
) -> String {
    if param_names.is_empty() {
        return format!("{function_signature} does not declare any parameters.");
    }
    format!(
        "Declared parameters for {function_signature}: {}.",
        param_names.join(", ")
    )
}

pub fn pipeline_scope_names_message(context: &str, names: &[String]) -> String {
    if names.is_empty() {
        return format!("{context} does not have any parameters or local bindings in scope.");
    }
    format!("Names in scope for {context}: {}.", names.join(", "))
}

pub fn pipeline_unknown_placeholder_message(context: &str, key: &str, names: &[String]) -> String {
    format!(
        "unknown pipeline placeholder '${key}' while expanding {context}. {}",
        pipeline_scope_names_message(context, names)
    )
}

pub fn pipeline_declared_kind_conflict_message(
    function_signature: &str,
    param_name: &str,
    declared_kind: &str,
    inferred_kind: &str,
) -> String {
    format!(
        "pipeline parameter '{param_name}' in {function_signature} declares kind '{declared_kind}' but is used like '{inferred_kind}'. Align the function body with the declared kind or update the annotation."
    )
}

pub fn pipeline_inferred_kind_conflict_message(
    function_signature: &str,
    param_name: &str,
    left_kind: &str,
    right_kind: &str,
) -> String {
    format!(
        "pipeline parameter '{param_name}' in {function_signature} is inferred inconsistently as both {left_kind} and {right_kind}. Split the values into separate parameters or keep every use-site in the same value family."
    )
}
