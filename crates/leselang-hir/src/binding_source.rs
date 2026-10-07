//! Bounded source-binding orchestration, not full source lowering or authority.

use std::fmt;

use leselang_runtime_core::{ScalarType, ScopeFrame, StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_BINDINGS, PureTypeError, valid_local_name};
use crate::scalar_source::{ScalarSourceError, binding_source, preflight_bindings};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingSourceLimits {
    pub source: SourceCallLimits,
    pub max_bindings: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingSourcePhase {
    Value,
    Body,
    Finish,
}

pub enum BindingSourceError<Error> {
    Source(ScalarSourceError<Error>),
    Output {
        phase: BindingSourcePhase,
        error: StructureError,
    },
    Native {
        phase: BindingSourcePhase,
        error: Error,
    },
}
impl<Error> fmt::Display for BindingSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "binding source has invalid shape or lexical policy",
            Self::Output { .. } => "binding output exceeds physical limits",
            Self::Native { .. } => "native binding source lowering failed",
        })
    }
}
impl<Error> fmt::Debug for BindingSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for BindingSourceError<Error> {}

/// Exact borrowed AST operands after cold physical/name/lexical admission.
/// Not a type, native schema, runtime scope or authority certificate.
pub struct BindingSourceForm<'source> {
    source: crate::scalar_source::BindingSource<'source>,
    span: Span,
}
impl<'source> BindingSourceForm<'source> {
    pub fn binding(&self) -> &'source NamedArgument {
        self.source.binding
    }
    pub fn body(&self) -> &'source Expression {
        self.source.body
    }
    pub fn span(&self) -> Span {
        self.span
    }
    /// Construct one language Bind, moving original native slots and buffers.
    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        value: Node<Field, Operation, HostEffect, IrResult>,
        body: Node<Field, Operation, HostEffect, IrResult>,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        self.source.construct(value, body)
    }
}
impl fmt::Debug for BindingSourceForm<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BindingSourceForm(<source>)")
    }
}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

pub type BindingOperand<Node, Metadata, Error> = Result<(Node, Metadata), Error>;

/// Explicit native child compiler and binding policy, with no accept-all defaults.
/// Value metadata may own a nonclone result declaration or group export buffer.
/// Body receives that same metadata mutably so its adapter may move it into a
/// guarded native type frame. The core neither creates nor mutates runtime locals.
/// Finish corroborates all types/cold schemas, closed opaque graphs, capture/flow
/// rules, helper-return rewrites and source-weight/canonical/authority policy.
/// A native callback is trusted code, not fuel-bounded, preempted or sandboxed.
pub trait BindingSourceAdapter<'source, Field, Operation, HostEffect, IrResult> {
    type Value;
    type Result;
    type Error;

    fn lower_value(
        &mut self,
        source: &'source NamedArgument,
    ) -> BindingOperand<Node<Field, Operation, HostEffect, IrResult>, Self::Value, Self::Error>;

    fn lower_body(
        &mut self,
        source: &BindingSourceForm<'source>,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        metadata: &mut Self::Value,
    ) -> BindingOperand<Node<Field, Operation, HostEffect, IrResult>, Self::Result, Self::Error>;

    fn finish(
        &mut self,
        source: &BindingSourceForm<'source>,
        value: Node<Field, Operation, HostEffect, IrResult>,
        metadata: Self::Value,
        body: Node<Field, Operation, HostEffect, IrResult>,
        result: Self::Result,
    ) -> BindingOperand<Node<Field, Operation, HostEffect, IrResult>, Self::Result, Self::Error>;
}

type Lowering<Node, ResultType, Error> = Result<(Node, ResultType), BindingSourceError<Error>>;

pub(crate) fn physical<Field, Operation, HostEffect, IrResult>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
    depth: usize,
    budget: &mut StructureBudget,
) -> Result<(), StructureError> {
    let mut pending = vec![(expression, depth)];
    while let Some((node, depth)) = pending.pop() {
        budget.visit(depth, 0, 0)?;
        for child in node.children() {
            budget.check_pending(pending.len(), 1)?;
            pending.push((child, depth + 1));
        }
    }
    Ok(())
}

/// Lower exactly one source Bind through once-only Value -> Body -> Finish hooks.
/// Whole cold AST/name/literal and lexical bind/loop/fold checks precede all hooks;
/// initializers cannot see their own new name, sibling scopes do not leak, and
/// active prefix names must be bounded/unique. Prefix conveys names only, not
/// inferred types, values, authority or automatically installed native frames.
/// Helpers/unknown extension-local namespaces remain native compiler policy.
///
/// Inclusive ceilings are 16,384 physical source/output nodes, depth 64, 64 call
/// operands and 1,024 active names. Zero source/output nodes rejects a root; zero
/// bindings rejects a Bind; depth zero cannot contain its operands. Before Body,
/// the original value IR fits at depth one with one Bind root and a body root
/// reserved. Before Finish, body IR joins the same aggregate meter at depth one.
/// Finish output is then physically checked afresh because a host may rewrite
/// helper returns. No folded/native source weight or expansion refund is implied.
///
/// Metadata, child Boxes, vector buffers and native slots move unchanged, with no
/// native Clone/Debug/serde/Send bounds. Callbacks see original AST references and
/// value metadata, not a copied native tree. Failure/unwind drops consumed parts
/// once and stops later hooks: no partial output, retry, reservation rollback or
/// cleanup of native side effects. Adapters guard their mutable frames even on
/// failure/unwind; the core borrows no runtime frame. Native errors are matchable,
/// never formatted or exposed as private source chains. Bound ingress before AST/
/// IR construction; native allocation, metadata mutation, Drop and unwind remain
/// host-owned. Generated IR language/type/schema validation and fresh live checks
/// remain mandatory. This is orchestration, not a complete independent compiler,
/// interpreter, received-result acceptance, registry or suspension lifecycle.
pub fn lower_binding_source<'source, Field, Operation, HostEffect, IrResult, Adapter>(
    expression: &'source Expression,
    prefix: &[&'source str],
    limits: BindingSourceLimits,
    adapter: &mut Adapter,
) -> Lowering<Node<Field, Operation, HostEffect, IrResult>, Adapter::Result, Adapter::Error>
where
    Adapter: BindingSourceAdapter<'source, Field, Operation, HostEffect, IrResult>,
{
    let source_error = |error| BindingSourceError::Source(error);
    if !valid_limits(limits.source) || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS {
        return Err(source_error(ScalarSourceError::Source(
            SourceCallError::InvalidLimits,
        )));
    }
    preflight(expression, limits.source)
        .map_err(ScalarSourceError::Source)
        .map_err(source_error)?;
    let source = binding_source(expression).map_err(source_error)?;
    let at = span(expression);
    if prefix.len() > limits.max_bindings {
        return Err(source_error(ScalarSourceError::Bindings {
            span: at,
            error: PureTypeError::BindingLimit,
        }));
    }
    for (index, name) in prefix.iter().enumerate() {
        let error = if !valid_local_name(name) {
            Some(PureTypeError::InvalidName)
        } else if prefix[..index].contains(name) {
            Some(PureTypeError::ShadowedBinding)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(source_error(ScalarSourceError::Bindings {
                span: at,
                error,
            }));
        }
    }
    let mut names = prefix
        .iter()
        .map(|name| (*name, ScalarType::None))
        .collect();
    let mut scope = ScopeFrame::new(&mut names);
    preflight_bindings(expression, &mut scope, limits.max_bindings).map_err(source_error)?;
    let source = BindingSourceForm { source, span: at };
    let output_error = |phase, error| BindingSourceError::Output { phase, error };
    let native_error = |phase, error| BindingSourceError::Native { phase, error };
    if limits.source.max_lowered_depth == 0 {
        return Err(output_error(
            BindingSourcePhase::Value,
            StructureError::DepthLimit,
        ));
    }
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    budget
        .visit(0, 0, 0)
        .map_err(|error| output_error(BindingSourcePhase::Value, error))?;
    budget
        .check_pending(0, 2)
        .map_err(|error| output_error(BindingSourcePhase::Value, error))?;
    let (value, mut metadata) = adapter
        .lower_value(source.binding())
        .map_err(|error| native_error(BindingSourcePhase::Value, error))?;
    physical(&value, 1, &mut budget)
        .and_then(|()| budget.check_pending(0, 1))
        .map_err(|error| output_error(BindingSourcePhase::Value, error))?;
    let (body, result) = adapter
        .lower_body(&source, &value, &mut metadata)
        .map_err(|error| native_error(BindingSourcePhase::Body, error))?;
    physical(&body, 1, &mut budget)
        .map_err(|error| output_error(BindingSourcePhase::Body, error))?;
    let (output, result) = adapter
        .finish(&source, value, metadata, body, result)
        .map_err(|error| native_error(BindingSourcePhase::Finish, error))?;
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    physical(&output, 0, &mut budget)
        .map_err(|error| output_error(BindingSourcePhase::Finish, error))?;
    Ok((output, result))
}
