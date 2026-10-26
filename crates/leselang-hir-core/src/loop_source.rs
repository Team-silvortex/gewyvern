//! Native loop compilation stages, not source admission or loop execution.

use std::fmt;

use leselang_runtime_core::ScalarType;
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::scalar_source::{
    LoopBody, LoopSource, Operand, ScalarSourceError, loop_source, lower_loop_body, scalar_operand,
};
use crate::source_call::span;

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type LoopSourceInput<Node, Metadata, Error> =
    Result<(Node, Option<ScalarType>, Metadata), Error>;
pub type LoopSourceChild<Node, Error> = Result<(Node, Option<ScalarType>), Error>;
type Prepared<'source, Node, Metadata, Error> =
    Result<PreparedLoopSource<'source, Node, Metadata>, ScalarSourceError<Error>>;
type Lowered<'source, Node, Metadata, Error> =
    Result<LoweredLoopSource<'source, Node, Metadata>, ScalarSourceError<Error>>;

/// Original aligned loop operands; not a lexical, physical-source or type receipt.
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::BoundLoopSource;
/// let plan = BoundLoopSource { source: todo!() };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::BoundLoopSource;
/// let plan: BoundLoopSource<'_> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "prepare the loop or deliberately discard its source alignment"]
pub struct BoundLoopSource<'source> {
    source: LoopSource<'source>,
}

/// Retains the original initial node, metadata and checked literal limit. Private
/// fields keep them paired with the original source and observed state type.
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::PreparedLoopSource;
/// let plan = PreparedLoopSource {
///     source: todo!(), initial: (), metadata: (), state_type: todo!(), limit: 0,
/// };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::PreparedLoopSource;
/// let plan: PreparedLoopSource<'_, (), ()> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "lower both cold body operands or deliberately discard the initial state"]
pub struct PreparedLoopSource<'source, Node, Metadata> {
    source: LoopSource<'source>,
    initial: Node,
    metadata: Metadata,
    state_type: ScalarType,
    limit: u64,
}

/// A paired initial state and body, consumed only after native lexical guards
/// have been released. This is not complete IR typing or execution authority.
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::LoweredLoopSource;
/// let plan = LoweredLoopSource { prepared: todo!(), body: todo!() };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::loop_source::LoweredLoopSource;
/// let plan: LoweredLoopSource<'_, (), ()> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "finish the paired loop or deliberately discard its owned nodes"]
pub struct LoweredLoopSource<'source, Node, Metadata> {
    prepared: PreparedLoopSource<'source, Node, Metadata>,
    body: LoopBody<Node>,
}

impl fmt::Debug for BoundLoopSource<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BoundLoopSource")
    }
}
impl<Node, Metadata> fmt::Debug for PreparedLoopSource<'_, Node, Metadata> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PreparedLoopSource")
    }
}
impl<Node, Metadata> fmt::Debug for LoweredLoopSource<'_, Node, Metadata> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LoweredLoopSource")
    }
}

/// Align exactly one named state plus `while`, `next` and `limit`, checking the
/// state name without inspecting cold descendants. The caller selects the loop
/// operation and must reject shadowing/active-binding overflow before preparation.
/// Limits are checked during preparation, after native scope policy, preserving
/// native diagnostic priority. No source/IR quota, execution fuel or runtime
/// reservation is implied. Use `scalar_source::lower_scalar_source_with_scope`
/// for whole eager source/lexical scanning and bounded recursive scalar lowering.
pub fn bind_loop_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<BoundLoopSource<'_>, ScalarSourceError<Error>> {
    loop_source(arguments, at).map(|source| BoundLoopSource { source })
}

impl<'source> BoundLoopSource<'source> {
    pub fn binding(&self) -> &'source NamedArgument {
        self.source.binding
    }

    /// Check the literal iteration bound before calling the initial lowerer once.
    /// The lowerer owns bounded cold-source/IR construction, complete native types
    /// and live policy. Scalar type is an observation, never inferred from opaque
    /// metadata. Language purity is derived from the returned IR, not a host flag.
    /// Bound that IR before returning it: the purity walk is not a resource meter.
    /// Original IR and metadata move unchanged, without Clone/Debug/serde/Send.
    pub fn prepare<Field, Operation, HostEffect, IrResult, Metadata, Error>(
        self,
        lower: impl FnOnce(
            &'source Expression,
        ) -> LoopSourceInput<
            Node<Field, Operation, HostEffect, IrResult>,
            Metadata,
            Error,
        >,
    ) -> Prepared<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata, Error> {
        let limit = self.source.limit()?;
        let expression = &self.source.binding.value;
        let (value, scalar_type, metadata) =
            lower(expression).map_err(|error| ScalarSourceError::Native {
                span: span(expression),
                error,
            })?;
        let (initial, state_type) = scalar_operand(
            Operand {
                pure: value.is_pure(),
                value,
                scalar_type,
            },
            span(expression),
        )?;
        Ok(PreparedLoopSource {
            source: self.source,
            initial,
            metadata,
            state_type,
            limit,
        })
    }
}

impl<'source, Node, Metadata> PreparedLoopSource<'source, Node, Metadata> {
    pub fn binding(&self) -> &'source NamedArgument {
        self.source.binding
    }

    /// Borrow the original observation for a guarded native lexical frame. It is
    /// not a copied type registry; native interior mutation remains trusted.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

impl<'source, Field, Operation, HostEffect, IrResult, Metadata>
    PreparedLoopSource<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata>
{
    /// Lower/check condition, then next, once each even for zero iterations or a
    /// false literal condition. Native callbacks own bounded construction and
    /// complete type/domain checks. The shared language gates derive IR purity,
    /// require a boolean condition and preserve the initial scalar state type.
    /// No evaluation, reordering, retries or native-side-effect rollback occurs.
    /// Failure/unwind releases owned nodes/metadata; adapters must guard scopes
    /// and revalidate current policy, including when preparation was delayed.
    pub fn lower_body<Error>(
        self,
        mut lower: impl FnMut(
            &'source Expression,
        )
            -> LoopSourceChild<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> Lowered<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata, Error> {
        let body = lower_loop_body(&self.source, self.state_type, |expression| {
            let (value, scalar_type) =
                lower(expression).map_err(|error| ScalarSourceError::Native {
                    span: span(expression),
                    error,
                })?;
            Ok(Operand {
                pure: value.is_pure(),
                value,
                scalar_type,
            })
        })?;
        Ok(LoweredLoopSource {
            prepared: self,
            body,
        })
    }
}

impl<Field, Operation, HostEffect, IrResult, Metadata>
    LoweredLoopSource<'_, Node<Field, Operation, HostEffect, IrResult>, Metadata>
{
    /// Assemble the original parts after native scope cleanup. No body from a
    /// different preparation or arbitrary iteration limit can be substituted.
    /// Complete generated-IR/source-cost/type checks still belong to the caller;
    /// construction does not execute the loop or grant authority to its result.
    pub fn finish(self) -> (Node<Field, Operation, HostEffect, IrResult>, Metadata) {
        let Self { prepared, body } = self;
        (
            prepared
                .source
                .construct(prepared.initial, body, prepared.limit),
            prepared.metadata,
        )
    }
}
