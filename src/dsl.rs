use crate::fragment::{RegistryError, builtin_registry};
use crate::template::TemplateBinding;
use std::collections::BTreeMap;
use std::fs;

mod diagnostics;
mod entry;
mod frontend;
mod function_types;
mod legacy;
mod package;
mod parser;
mod pipeline;
mod predicate;

pub use self::entry::{
    compile_file, parse_file_unvalidated, parse_file_with_frontend_unvalidated,
    parse_str_unvalidated, parse_str_with_frontend_unvalidated,
};
pub(crate) use self::entry::{
    load_file_with_package_context, parse_str_with_frontend_unvalidated_with_package,
};
pub use self::frontend::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendFunctionParam,
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind,
    FrontendIncludeSource, FrontendIncludeSourceKind, FrontendModuleSummary, FrontendUseEdge,
    summarize_frontend_file, summarize_frontend_str,
};
pub(crate) use self::frontend::summarize_frontend_str_with_package;
use self::function_types::PipelineValueKind;
use self::package::PackageContext;
pub use self::package::build_lockfile;
use self::parser::{parse_pipeline_function_head, parse_pipeline_module};
use self::pipeline::{lower_pipeline_module_to_legacy, parse_pipeline_single_arg};
pub(crate) use self::predicate::{parse_flow_predicate, parse_reason_key_event};

pub const PACKAGE_MANIFEST_FILE: &str = "gewy.pkg";

#[derive(Debug, Eq, PartialEq)]
pub enum DslError {
    Located {
        line: usize,
        column: Option<usize>,
        inner: Box<DslError>,
    },
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Registry(RegistryError),
    Io(String),
}

pub fn read_file(path: &str) -> Result<String, DslError> {
    fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))
}

pub(super) fn strip_comments_preserve_layout(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_block_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                output.push(' ');
                output.push(' ');
                chars.next();
                in_block_comment = false;
            } else if ch == '\n' {
                output.push('\n');
            } else {
                output.push(' ');
            }
            continue;
        }

        if in_string {
            output.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' && matches!(chars.peek(), Some('*')) {
            output.push(' ');
            output.push(' ');
            chars.next();
            in_block_comment = true;
            continue;
        }

        if ch == '#' {
            output.push(' ');
            while let Some(next) = chars.next() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
                output.push(' ');
            }
            continue;
        }

        output.push(ch);
    }

    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineModule {
    package_scope: String,
    module_doc: Option<String>,
    template_doc: Option<String>,
    template: Option<PipelineCall>,
    body: Vec<PipelineCall>,
    functions: BTreeMap<String, PipelineFunction>,
    include_sources: Vec<FrontendIncludeSource>,
    include_edges: Vec<FrontendGraphEdge>,
    use_edges: Vec<FrontendUseEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineFunction {
    doc: Option<String>,
    params: Vec<PipelineParam>,
    local_bindings: Vec<PipelineLetBinding>,
    body: Vec<PipelineCall>,
    source_id: String,
    package_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineParam {
    name: String,
    default_value: Option<String>,
    declared_kind: Option<PipelineValueKind>,
    inferred_kind: Option<PipelineValueKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineCall {
    line_no: usize,
    column_no: usize,
    name: String,
    args: Vec<String>,
    arg_columns: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipelineFunctionBodySyntax {
    Block,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineLetBinding {
    name: String,
    value: String,
}

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_str_unvalidated(input)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn validate_compiled_binding(binding: &TemplateBinding) -> Result<(), RegistryError> {
    builtin_registry().validate_binding(binding)
}

impl DslError {
    pub fn at_line(self, line: usize) -> Self {
        self.at_line_column(line, None)
    }

    pub fn at_line_column(self, line: usize, column: Option<usize>) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column: existing_column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: existing_column.or(column),
                inner,
            },
            other => Self::Located {
                line,
                column,
                inner: Box::new(other),
            },
        }
    }

    pub fn reanchor_line_column(self, line: usize, column_offset: usize) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: column.map(|value| value + column_offset.saturating_sub(1)),
                inner,
            },
            other => Self::Located {
                line,
                column: Some(column_offset),
                inner: Box::new(other),
            },
        }
    }

    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Located { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn column(&self) -> Option<usize> {
        match self {
            Self::Located { column, .. } => *column,
            _ => None,
        }
    }

    pub fn root(&self) -> &DslError {
        match self {
            Self::Located { inner, .. } => inner.root(),
            other => other,
        }
    }
}

fn parse_bool(value: &str) -> Result<bool, DslError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DslError::InvalidValue(format!("invalid bool '{other}'"))),
    }
}

fn split_top_level_with_columns(
    input: &str,
    delimiter: char,
    base_column: usize,
) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                let raw = &input[start..idx];
                let trimmed = raw.trim();
                let leading = raw.find(trimmed).unwrap_or(0);
                parts.push((base_column + start + leading, trimmed.to_string()));
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    let raw = &input[start..];
    let trimmed = raw.trim();
    let leading = raw.find(trimmed).unwrap_or(0);
    parts.push((base_column + start + leading, trimmed.to_string()));
    parts
}

#[cfg(test)]
mod tests {
    use super::legacy::{parse_reason_rule, parse_rule};

    #[test]
    fn parse_rule_reanchors_invalid_stage_column() {
        let input = "process_bound;not_a_stage;static:test;true";
        let err = parse_rule(input).expect_err("invalid stage");
        assert_eq!(err.column(), Some(input.find("not_a_stage").unwrap() + 1));
    }

    #[test]
    fn parse_rule_reanchors_composite_predicate_child_column() {
        let input = "all(process_bound, packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1);connect_flow;static:test;true";
        let err = parse_rule(input).expect_err("invalid composite predicate child");
        assert_eq!(err.column(), Some(input.find("byte_at").unwrap() + 1));
    }

    #[test]
    fn parse_reason_rule_reanchors_invalid_key_event_column() {
        let input = "process_bound;not_a_reason_event;static:test;true";
        let err = parse_reason_rule(input).expect_err("invalid reason key event");
        assert_eq!(
            err.column(),
            Some(input.find("not_a_reason_event").unwrap() + 1)
        );
    }

    #[test]
    fn strip_comments_keeps_string_hashes_and_newlines() {
        let input = "template(:demo) # tail\n|> include(\"./a#b.gewy\")\n/* block\ncomment */\n";
        let stripped = super::strip_comments_preserve_layout(input);
        assert!(stripped.contains("template(:demo)"));
        assert!(stripped.contains("\"./a#b.gewy\""));
        assert_eq!(input.lines().count(), stripped.lines().count());
        assert!(!stripped.contains("tail"));
        assert!(!stripped.contains("block"));
    }
}
