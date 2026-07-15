use std::collections::BTreeMap;

mod lowering;
mod parsing;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineProvidedArg {
    raw: String,
    value: String,
}

pub(super) struct PipelineUseCall {
    function_name: String,
    positional_args: Vec<PipelineProvidedArg>,
    named_args: BTreeMap<String, PipelineProvidedArg>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineKeywordArg {
    value: String,
    value_column: usize,
}

fn looks_like_pipeline_keyword_arg(arg: &str) -> bool {
    let arg = arg.trim();
    if arg.starts_with(':') || arg.starts_with('"') {
        return false;
    }
    arg.split_once(':')
        .is_some_and(|(key, _)| !key.trim().is_empty())
}

pub(super) use lowering::lower_pipeline_module_to_assignments;
pub(super) use parsing::{
    parse_pipeline_call, parse_pipeline_function_signature, parse_pipeline_let_binding,
    parse_pipeline_single_arg, push_pipeline_function_call,
};
