use std::borrow::Cow;

use gewylang_syntax::PipelineUseCall;

mod lowering;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineKeywordArg<'a> {
    value: Cow<'a, str>,
    value_column: usize,
}

pub(super) use gewylang_syntax::looks_like_pipeline_keyword_arg;

pub(super) use lowering::lower_pipeline_module_to_assignments;
