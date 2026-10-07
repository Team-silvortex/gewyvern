//! Owned native-effect source construction, not opaque graph typing or dispatch.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, Span};

use crate::ir::Computation;
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

pub enum HostSourceError<Error> {
    Source(SourceCallError<Error>),
    NotCall { span: Span },
    Output { span: Span, error: StructureError },
    Preparation { span: Span, error: Error },
    Admission { span: Span, error: Error },
}

impl<Error> fmt::Display for HostSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "native host source shape or limits are invalid",
            Self::NotCall { .. } => "native host source requires a call",
            Self::Output { .. } => "native host source root exceeds physical limits",
            Self::Preparation { .. } => "native host source preparation failed",
            Self::Admission { .. } => "native host source admission failed",
        })
    }
}
impl<Error> fmt::Debug for HostSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HostSourceError<Error> {}

pub type NativeHostSource<HostEffect, Type, Error> = Result<(HostEffect, Type), Error>;
pub type HostSourceResult<Node, Type, Error> = Result<(Node, Type), HostSourceError<Error>>;
type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

/// Prepare one original native effect and require once-only native admission.
/// Whole cold physical AST/name/text/argument limits precede every hook, including
/// source hidden in unselected native operands. No native source schema is inferred
/// from a callee name: the adapter validates exact signatures, literal domains,
/// versions/grants, original operation/result identity and its lexical policy.
///
/// One Host language root at depth zero is checked before preparation. This meter
/// does NOT inspect opaque effect payloads, nested native graphs or allocation.
/// Prepare must bound native growth/work before construction; mandatory admission
/// must corroborate the entire opaque graph and original result observation under
/// current policy. Source admission and one root do not certify native safety.
/// The same original AST/effect/type are borrowed into admission once, then moved
/// unchanged into the Host node and returned observation. No native Clone/Debug/
/// PartialEq/serde/Send bound, name registry, second tree or type-copy factory.
///
/// Hooks must not execute effects, accept replies or grant runtime authority.
/// Failure/unwind releases core-owned native parts without partial output, retries,
/// refunds or rollback of external work. Private errors remain matchable but are
/// never formatted or exposed through source chains. Native callbacks, interior
/// mutation, allocation and Drop/unwind remain trusted and cannot be preempted.
/// Bound ingress first, and revalidate full types/domains/live policy before use.
/// This is native leaf construction, not complete opaque-flow typing, a generic
/// recursive compiler, runtime request preparation, durable recovery or dispatch.
pub fn lower_host_source<'source, Field, Operation, HostEffect, IrResult, Type, Error>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    prepare: impl FnOnce(&'source Expression) -> NativeHostSource<HostEffect, Type, Error>,
    admit: impl FnOnce(&'source Expression, &HostEffect, &Type) -> Result<(), Error>,
) -> HostSourceResult<Node<Field, Operation, HostEffect, IrResult>, Type, Error> {
    if !valid_limits(limits) {
        return Err(HostSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits).map_err(HostSourceError::Source)?;
    lower_preflighted_host_source(expression, limits, prepare, admit)
}

// Only the product's already-admitted parser/declaration path skips cold source
// scanning, preserving its recursive native diagnostic and source-counter order.
pub(crate) fn lower_preflighted_host_source<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Type,
    Error,
>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    prepare: impl FnOnce(&'source Expression) -> NativeHostSource<HostEffect, Type, Error>,
    admit: impl FnOnce(&'source Expression, &HostEffect, &Type) -> Result<(), Error>,
) -> HostSourceResult<Node<Field, Operation, HostEffect, IrResult>, Type, Error> {
    if !valid_limits(limits) {
        return Err(HostSourceError::Source(SourceCallError::InvalidLimits));
    }
    let at = span(expression);
    if !matches!(expression, Expression::Call { .. }) {
        return Err(HostSourceError::NotCall { span: at });
    }
    StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth)
        .visit(0, 0, 0)
        .map_err(|error| HostSourceError::Output { span: at, error })?;
    let (effect, ty) =
        prepare(expression).map_err(|error| HostSourceError::Preparation { span: at, error })?;
    admit(expression, &effect, &ty)
        .map_err(|error| HostSourceError::Admission { span: at, error })?;
    Ok((
        Computation::Host {
            effect: Box::new(effect),
        },
        ty,
    ))
}
