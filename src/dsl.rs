use crate::fragment::{RegistryError, builtin_registry_ref};
use crate::template::TemplateBinding;

mod entry;
mod function_types;
mod legacy;
mod pipeline;
mod predicate;
mod semantic_host;

pub use self::entry::{
    compile_file, parse_file_unvalidated, parse_file_with_frontend_unvalidated,
    parse_str_unvalidated, parse_str_with_frontend_unvalidated,
};
pub(crate) use self::entry::{
    load_file_with_package_context, parse_str_unvalidated_with_package,
    parse_str_with_frontend_unvalidated_with_package,
};
use self::pipeline::lower_pipeline_module_to_assignments;
pub(crate) use self::predicate::{parse_flow_predicate, parse_reason_key_event};
pub use gewylang_contract::{
    GEWYLANG_ANALYSIS_IR_VERSION, GEWYLANG_BINDING_IR_VERSION, GEWYLANG_EXPANDED_AST_VERSION,
    GEWYLANG_LANGUAGE_ID, GEWYLANG_SYNTAX_VERSION, GewyLangContractStamp, GewyLangStage,
    MAX_GEWYLANG_INCLUDE_DEPTH, MAX_GEWYLANG_SOURCE_BYTES, MAX_GEWYLANG_SOURCE_GRAPH_BYTES,
    MAX_GEWYLANG_SOURCE_GRAPH_FILES, PACKAGE_MANIFEST_FILE,
};
pub use gewylang_syntax::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendFunctionParam,
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind,
    FrontendIncludeSource, FrontendIncludeSourceKind, FrontendModuleSummary, FrontendUseEdge,
};
pub(crate) use gewylang_syntax::{PackageContext, PipelineModule, summarize_pipeline_module};

pub fn build_lockfile(path: &str) -> Result<String, DslError> {
    gewylang_syntax::build_lockfile(path).map_err(DslError::from)
}

pub fn read_file(path: &str) -> Result<String, DslError> {
    gewylang_syntax::read_file(path).map_err(DslError::from)
}

pub fn summarize_frontend_file(path: &str) -> Result<FrontendModuleSummary, DslError> {
    gewylang_syntax::summarize_frontend_file(path).map_err(DslError::from)
}

pub fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
    gewylang_syntax::summarize_frontend_str(input).map_err(DslError::from)
}

pub(crate) fn summarize_frontend_str_with_package(
    input: &str,
    package: &PackageContext,
) -> Result<FrontendModuleSummary, DslError> {
    gewylang_syntax::summarize_frontend_str_with_package(input, package).map_err(DslError::from)
}

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

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_str_unvalidated(input)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn validate_compiled_binding(binding: &TemplateBinding) -> Result<(), RegistryError> {
    builtin_registry_ref().validate_binding(binding)
}

impl From<gewylang_syntax::SyntaxError> for DslError {
    fn from(error: gewylang_syntax::SyntaxError) -> Self {
        match error {
            gewylang_syntax::SyntaxError::Located {
                line,
                column,
                inner,
            } => Self::Located {
                line,
                column,
                inner: Box::new(Self::from(*inner)),
            },
            gewylang_syntax::SyntaxError::InvalidLine(line) => Self::InvalidLine(line),
            gewylang_syntax::SyntaxError::MissingField(field) => Self::MissingField(field),
            gewylang_syntax::SyntaxError::InvalidValue(value) => Self::InvalidValue(value),
            gewylang_syntax::SyntaxError::Io(error) => Self::Io(error),
        }
    }
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
