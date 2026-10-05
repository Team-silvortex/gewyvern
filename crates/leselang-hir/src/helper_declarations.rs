//! Borrowed scalar declaration admission, not lowering or execution authority.

use std::{collections::HashSet, fmt};

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, Function, MAX_FUNCTION_PARAMETERS, MAX_FUNCTIONS, Span};

use crate::helper_source::{HelperParameterError, helper_parameters};
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, valid_local_name};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperDeclarationLimits {
    pub max_functions: usize,
    pub max_parameters: usize,
    pub max_source_nodes: usize,
    pub max_source_depth: usize,
}

pub enum HelperDeclarationError<Error> {
    InvalidLimits,
    FunctionLimit,
    InvalidEntry,
    InvalidName {
        declaration_index: usize,
        span: Span,
    },
    ReservedName {
        declaration_index: usize,
        span: Span,
    },
    DuplicateName {
        declaration_index: usize,
        span: Span,
    },
    Policy {
        declaration_index: usize,
        span: Span,
        error: Error,
    },
    Parameters {
        declaration_index: usize,
        span: Span,
        error: HelperParameterError,
    },
    Source {
        declaration_index: usize,
        span: Span,
        error: StructureError,
    },
    MissingEntry {
        span: Span,
    },
    EntryParameters {
        declaration_index: usize,
        span: Span,
    },
}
impl<Error> fmt::Display for HelperDeclarationError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper declaration limits exceed safety ceilings",
            Self::FunctionLimit => "function declaration count exceeds its limit",
            Self::InvalidEntry => "designated entry name is invalid",
            Self::InvalidName { .. } => "function declaration name is invalid",
            Self::ReservedName { .. } => "function declaration name is reserved",
            Self::DuplicateName { .. } => "function declaration name is duplicated",
            Self::Policy { .. } => "native declaration name policy failed",
            Self::Parameters { .. } => "function scalar signature is invalid",
            Self::Source { .. } => "function declaration source exceeds physical bounds",
            Self::MissingEntry { .. } => "designated entry declaration is missing",
            Self::EntryParameters { .. } => "designated entry cannot have parameters",
        })
    }
}
impl<Error> fmt::Debug for HelperDeclarationError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperDeclarationError<Error> {}

/// Exact borrowed entry/helpers and language name reservations. Supplied order is
/// retained, not dependency order; reservations are an unordered membership set.
/// No names, AST nodes or literal buffers are copied. This observation has no
/// template registry, cache, type certificate, version, grant or execution state.
/// Debug reveals counts only, never names, parameters, source or policy payloads.
#[must_use = "consume the declaration observation or deliberately discard it"]
pub struct HelperDeclarations<'source> {
    entry: &'source Function,
    helpers: Vec<&'source Function>,
    reserved_names: HashSet<&'source str>,
}
impl fmt::Debug for HelperDeclarations<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperDeclarations")
            .field("helpers", &self.helpers.len())
            .field("reserved_names", &self.reserved_names.len())
            .finish()
    }
}
impl<'source> HelperDeclarations<'source> {
    pub fn entry(&self) -> &'source Function {
        self.entry
    }
    pub fn helpers(&self) -> &[&'source Function] {
        &self.helpers
    }
    pub fn reserved_names(&self) -> impl ExactSizeIterator<Item = &'source str> + '_ {
        self.reserved_names.iter().copied()
    }
}

fn collect_source_names<'source>(
    function: &'source Function,
    limits: HelperDeclarationLimits,
    names: &mut HashSet<&'source str>,
) -> Result<(), (Span, StructureError)> {
    let mut budget = StructureBudget::new(limits.max_source_nodes, limits.max_source_depth);
    let mut pending = vec![(&function.body, 0)];
    while let Some((expression, depth)) = pending.pop() {
        let fail = |error| (crate::source_call::span(expression), error);
        budget.visit(depth, 0, 0).map_err(fail)?;
        match expression {
            Expression::Call {
                callee, arguments, ..
            } => {
                budget
                    .check_pending(pending.len(), arguments.len())
                    .map_err(fail)?;
                names.extend(arguments.iter().map(|argument| argument.name.as_str()));
                // An unused fold item still reserves its quoted binding label.
                if callee == "fold"
                    && let Some(Expression::String { value, .. }) = arguments
                        .iter()
                        .find(|argument| argument.name == "item")
                        .map(|argument| &argument.value)
                {
                    names.insert(value);
                }
                pending.extend(
                    arguments
                        .iter()
                        .map(|argument| (&argument.value, depth + 1)),
                );
            }
            Expression::Reference { name, .. } => {
                names.insert(name);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Admit headers and complete physical source for every supplied declaration,
/// including the entry and unused helpers. Entry naming and reserved-name policy
/// are explicit host choices; names use exact bounded identifiers, without case
/// folding, normalization or implicit builtin/operation lookup. The designated
/// entry is mandatory and parameterless. Scalar signatures reuse the six closed
/// tokens and eight-parameter ceiling. Limits are inclusive: at most 32 functions,
/// 16,384 physical nodes per body and depth 64; zero nodes denies bodies, zero
/// depth permits leaves, and zero functions produces MissingEntry on empty input.
///
/// Check ceilings/count/entry before callbacks. For each declaration in original
/// order, check its name, call policy once, reject duplicates, check signature,
/// then walk its complete body. Physical visits and pending-frontier bounds
/// precede reservation growth. Missing/parameterized entry checks follow the
/// entire forest, preserving declaration error priority. A policy failure/unwind
/// yields no partial observation or retry; native side effects are not rolled
/// back. Native work and input allocation/ingress remain caller responsibilities.
///
/// Reservations borrow parameter names, physical argument labels and references,
/// plus the first quoted fold item, even in cold paths. Function/callee names and
/// ordinary string literals do not reserve variables. This does not validate
/// argument labels, literal bytes, source spans, lexical types, operator schemas,
/// native grants or return types. Dependency cycles/entry calls, helper expansion,
/// canonical source and complete cold lowering remain separate mandatory checks.
pub fn accept_helper_declarations<'source, Error>(
    declarations: &[&'source Function],
    entry: &str,
    limits: HelperDeclarationLimits,
    mut reserved: impl FnMut(&str) -> Result<bool, Error>,
) -> Result<HelperDeclarations<'source>, HelperDeclarationError<Error>> {
    if limits.max_functions > MAX_FUNCTIONS
        || limits.max_parameters > MAX_FUNCTION_PARAMETERS
        || limits.max_source_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_source_depth > MAX_TYPE_INFERENCE_DEPTH
    {
        return Err(HelperDeclarationError::InvalidLimits);
    }
    if declarations.len() > limits.max_functions {
        return Err(HelperDeclarationError::FunctionLimit);
    }
    if !valid_local_name(entry) {
        return Err(HelperDeclarationError::InvalidEntry);
    }
    let mut names = HashSet::new();
    for (declaration_index, function) in declarations.iter().enumerate() {
        let span = function.span;
        if !valid_local_name(&function.name) {
            return Err(HelperDeclarationError::InvalidName {
                declaration_index,
                span,
            });
        }
        if reserved(&function.name).map_err(|error| HelperDeclarationError::Policy {
            declaration_index,
            span,
            error,
        })? {
            return Err(HelperDeclarationError::ReservedName {
                declaration_index,
                span,
            });
        }
        if declarations[..declaration_index]
            .iter()
            .any(|previous| previous.name == function.name)
        {
            return Err(HelperDeclarationError::DuplicateName {
                declaration_index,
                span,
            });
        }
        helper_parameters(function, limits.max_parameters).map_err(|error| {
            let span = match error {
                HelperParameterError::InvalidName { index }
                | HelperParameterError::DuplicateName { index }
                | HelperParameterError::UnknownType { index } => function.parameters[index].span,
                _ => span,
            };
            HelperDeclarationError::Parameters {
                declaration_index,
                span,
                error,
            }
        })?;
        names.extend(
            function
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str()),
        );
        collect_source_names(function, limits, &mut names).map_err(|(span, error)| {
            HelperDeclarationError::Source {
                declaration_index,
                span,
                error,
            }
        })?;
    }
    let entry_index = declarations
        .iter()
        .position(|function| function.name == entry)
        .ok_or_else(|| HelperDeclarationError::MissingEntry {
            span: declarations
                .first()
                .map(|function| function.span)
                .unwrap_or(Span { start: 0, end: 0 }),
        })?;
    let entry = declarations[entry_index];
    if !entry.parameters.is_empty() {
        return Err(HelperDeclarationError::EntryParameters {
            declaration_index: entry_index,
            span: entry.span,
        });
    }
    Ok(HelperDeclarations {
        entry,
        helpers: declarations
            .iter()
            .copied()
            .filter(|function| !std::ptr::eq(*function, entry))
            .collect(),
        reserved_names: names,
    })
}
