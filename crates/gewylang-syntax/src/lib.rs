#![forbid(unsafe_code)]

//! Product-independent GewyLang source, package, parser, and frontend model.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

mod diagnostics;
mod entry;
mod frontend;
mod function_types;
mod package;
mod parser;
mod parsing;
mod source_graph;

pub use diagnostics::{
    pipeline_available_steps_message, pipeline_declared_functions_message,
    pipeline_declared_kind_conflict_message, pipeline_declared_params_message,
    pipeline_inferred_kind_conflict_message, pipeline_scope_names_message,
    pipeline_unknown_placeholder_message,
};
pub use entry::{load_file_with_package_context, parse_file, parse_str, parse_str_with_package};
pub use frontend::{
    FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendFunctionParam,
    FrontendGraphEdge, FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind,
    FrontendIncludeSource, FrontendIncludeSourceKind, FrontendModuleSummary, FrontendUseEdge,
    summarize_frontend_file, summarize_frontend_str, summarize_frontend_str_with_package,
    summarize_pipeline_module,
};
pub use function_types::{format_pipeline_function_signature, pipeline_value_kind_text};
pub use gewylang_contract::{
    GEWYLANG_SYNTAX_VERSION, MAX_GEWYLANG_EXPANDED_VALUE_BYTES, MAX_GEWYLANG_EXPANSION_STEPS,
    MAX_GEWYLANG_FUNCTION_EXPANSION_DEPTH, MAX_GEWYLANG_INCLUDE_DEPTH,
    MAX_GEWYLANG_PACKAGE_MANIFEST_BYTES, MAX_GEWYLANG_SOURCE_BYTES,
    MAX_GEWYLANG_SOURCE_GRAPH_BYTES, MAX_GEWYLANG_SOURCE_GRAPH_FILES, PACKAGE_MANIFEST_FILE,
};
pub use package::{PackageContext, build_lockfile};
pub use parser::parse_pipeline_function_head;
pub use parsing::{
    is_pipeline_identifier, parse_pipeline_call, parse_pipeline_function_signature,
    parse_pipeline_let_binding, parse_pipeline_literal, parse_pipeline_literal_cow,
    parse_pipeline_single_arg, parse_pipeline_use_call,
};

/// A product-neutral source, package, or syntax failure with stable source anchors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxError {
    Located {
        line: usize,
        column: Option<usize>,
        inner: Box<SyntaxError>,
    },
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Io(String),
}

impl SyntaxError {
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

    pub fn root(&self) -> &SyntaxError {
        match self {
            Self::Located { inner, .. } => inner.root(),
            other => other,
        }
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Located {
                line,
                column,
                inner,
            } => match column {
                Some(column) => write!(formatter, "{inner} at {line}:{column}"),
                None => write!(formatter, "{inner} at line {line}"),
            },
            Self::InvalidLine(line) => write!(formatter, "invalid line: {line}"),
            Self::MissingField(field) => write!(formatter, "missing field: {field}"),
            Self::InvalidValue(value) | Self::Io(value) => formatter.write_str(value),
        }
    }
}

impl std::error::Error for SyntaxError {}

/// Parsed, include-expanded GewyLang pipeline syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineModule {
    pub package_scope: String,
    pub module_doc: Option<String>,
    pub template_doc: Option<String>,
    pub template: Option<PipelineCall>,
    pub body: Vec<PipelineCall>,
    pub functions: BTreeMap<String, PipelineFunction>,
    pub include_sources: Vec<FrontendIncludeSource>,
    pub include_edges: Vec<FrontendGraphEdge>,
    pub use_edges: Vec<FrontendUseEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineFunction {
    pub doc: Option<String>,
    pub params: Vec<PipelineParam>,
    pub local_bindings: Vec<PipelineLetBinding>,
    pub body: Vec<PipelineCall>,
    pub source_id: String,
    pub package_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineParam {
    pub name: String,
    pub default_value: Option<String>,
    pub declared_kind: Option<PipelineValueKind>,
    pub inferred_kind: Option<PipelineValueKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineCall {
    pub line_no: usize,
    pub column_no: usize,
    pub name: String,
    pub args: Vec<String>,
    pub arg_columns: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipelineFunctionBodySyntax {
    Block,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineLetBinding {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipelineValueKind {
    Atom,
    Bool,
    U64,
    Predicate,
    Narrative,
    Stage,
    KeyEvent,
    Phase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineProvidedArg {
    pub raw: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineUseCall {
    pub function_name: String,
    pub positional_args: Vec<PipelineProvidedArg>,
    pub named_args: BTreeMap<String, PipelineProvidedArg>,
}

pub fn looks_like_pipeline_keyword_arg(arg: &str) -> bool {
    let arg = arg.trim();
    if arg.starts_with(':') || arg.starts_with('"') {
        return false;
    }
    arg.split_once(':')
        .is_some_and(|(key, _)| !key.trim().is_empty())
}

pub fn read_file(path: &str) -> Result<String, SyntaxError> {
    read_source_file(Path::new(path))
}

pub(crate) fn read_source_file(path: &Path) -> Result<String, SyntaxError> {
    read_bounded_utf8_file(path, MAX_GEWYLANG_SOURCE_BYTES, "gewylang source")
}

pub(crate) fn read_bounded_utf8_file(
    path: &Path,
    max_bytes: usize,
    label: &str,
) -> Result<String, SyntaxError> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|err| SyntaxError::Io(err.to_string()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(SyntaxError::InvalidValue(format!(
            "{label} path '{}' is not a regular file",
            path.display()
        )));
    }
    if path_metadata.len() > max_bytes as u64 {
        return Err(bounded_file_too_large(label, max_bytes));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options
        .open(path)
        .map_err(|err| SyntaxError::Io(err.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|err| SyntaxError::Io(err.to_string()))?;
    if !metadata.is_file() {
        return Err(SyntaxError::InvalidValue(format!(
            "{label} path '{}' is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(bounded_file_too_large(label, max_bytes));
    }

    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(max_bytes)
            .min(max_bytes)
            .saturating_add(1),
    );
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|err| SyntaxError::Io(err.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(bounded_file_too_large(label, max_bytes));
    }
    String::from_utf8(bytes).map_err(|err| SyntaxError::Io(err.to_string()))
}

pub fn validate_source_size(input: &str) -> Result<(), SyntaxError> {
    if input.len() > MAX_GEWYLANG_SOURCE_BYTES {
        return Err(gewylang_source_too_large());
    }
    Ok(())
}

fn gewylang_source_too_large() -> SyntaxError {
    bounded_file_too_large("gewylang source", MAX_GEWYLANG_SOURCE_BYTES)
}

fn bounded_file_too_large(label: &str, max_bytes: usize) -> SyntaxError {
    SyntaxError::InvalidValue(format!("{label} exceeds {max_bytes} bytes"))
}

pub fn strip_comments_preserve_layout(input: &str) -> Result<Cow<'_, str>, SyntaxError> {
    if !input.as_bytes().contains(&b'#') && !input.as_bytes().windows(2).any(|pair| pair == b"/*") {
        return Ok(Cow::Borrowed(input));
    }

    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    let mut in_block_comment = false;
    let mut block_comment_start = None;
    let mut line = 1usize;
    let mut column = 1usize;

    while let Some(ch) = chars.next() {
        if in_block_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                output.push(' ');
                output.push(' ');
                chars.next();
                in_block_comment = false;
                column += 2;
            } else if ch == '\n' {
                output.push('\n');
                line += 1;
                column = 1;
            } else {
                output.push(' ');
                column += ch.len_utf8();
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
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += ch.len_utf8();
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            column += 1;
            continue;
        }

        if ch == '/' && matches!(chars.peek(), Some('*')) {
            output.push(' ');
            output.push(' ');
            chars.next();
            in_block_comment = true;
            block_comment_start = Some((line, column));
            column += 2;
            continue;
        }

        if ch == '#' {
            output.push(' ');
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    line += 1;
                    column = 1;
                    break;
                }
                output.push(' ');
                column += next.len_utf8();
            }
            continue;
        }

        output.push(ch);
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf8();
        }
    }

    if in_block_comment {
        let (line, column) = block_comment_start.unwrap_or((line, column));
        return Err(
            SyntaxError::InvalidValue("unclosed pipeline block comment".into())
                .at_line_column(line, Some(column)),
        );
    }
    Ok(Cow::Owned(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gewylang_source_size_is_bounded() {
        assert!(validate_source_size(&"x".repeat(MAX_GEWYLANG_SOURCE_BYTES)).is_ok());
        assert_eq!(
            validate_source_size(&"x".repeat(MAX_GEWYLANG_SOURCE_BYTES + 1)),
            Err(SyntaxError::InvalidValue(format!(
                "gewylang source exceeds {MAX_GEWYLANG_SOURCE_BYTES} bytes"
            )))
        );
    }

    #[cfg(unix)]
    #[test]
    fn gewylang_source_reader_rejects_non_regular_files() {
        use std::os::unix::net::UnixListener;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gewy-source-socket-{}-{unique}",
            std::process::id()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let err = read_file(path.to_str().unwrap()).unwrap_err();
        drop(listener);
        std::fs::remove_file(&path).unwrap();

        assert!(
            matches!(err, SyntaxError::InvalidValue(message) if message.contains("is not a regular file"))
        );
    }

    #[test]
    fn comments_preserve_source_layout_and_string_hashes() {
        let input = "template(:demo) # tail\n|> include(\"./a#b.gewy\")\n/* block\ncomment */\n";
        let stripped = strip_comments_preserve_layout(input).unwrap();
        assert!(stripped.contains("template(:demo)"));
        assert!(stripped.contains("\"./a#b.gewy\""));
        assert_eq!(input.lines().count(), stripped.lines().count());
        assert!(!stripped.contains("tail"));
        assert!(!stripped.contains("block"));
    }

    #[test]
    fn comment_free_sources_are_borrowed() {
        let input = "template :borrowed\n|> window :default_5s\n";
        assert!(matches!(
            strip_comments_preserve_layout(input).unwrap(),
            Cow::Borrowed(source) if std::ptr::eq(source, input)
        ));
    }

    #[test]
    fn unclosed_block_comments_keep_the_opening_anchor() {
        let input = "template :demo\n  /* never closed\n|> window :default_5s\n";
        assert_eq!(
            strip_comments_preserve_layout(input),
            Err(
                SyntaxError::InvalidValue("unclosed pipeline block comment".into())
                    .at_line_column(2, Some(3))
            )
        );
    }
}
