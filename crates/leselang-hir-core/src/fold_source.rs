//! Native fold compilation stages, not collection traversal or source admission.

use std::fmt;

use leselang_runtime_core::{ScalarType, ScopeFrame};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::PureTypeError;
use crate::scalar_source::{
    FoldSource, Operand, ScalarSourceError, fold_items, fold_next, fold_source, scalar_operand,
};
use crate::source_call::span;

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type FoldSourceChild<Node, Error> = Result<(Node, Option<ScalarType>), Error>;
pub type FoldSourceInput<Node, Metadata, Error> =
    Result<(Node, Option<ScalarType>, Metadata), Error>;
type StageResult<Stage, Error> = Result<Stage, ScalarSourceError<Error>>;
type Prepared<'source, Node, Metadata, Error> =
    StageResult<PreparedFoldSource<'source, Node, Metadata>, Error>;
type Lowered<'source, Node, Metadata, Error> =
    StageResult<LoweredFoldSource<'source, Node, Metadata>, Error>;
type Output<Node, Metadata, Error> = StageResult<(Node, Metadata), Error>;

/// Original aligned fold operands. This is not a physical-source, lexical or
/// type receipt; callers must check their active scope before preparation.
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::BoundFoldSource;
/// let plan = BoundFoldSource { source: todo!() };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::BoundFoldSource;
/// let plan: BoundFoldSource<'_> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "prepare the collection or deliberately discard its alignment"]
pub struct BoundFoldSource<'source> {
    source: FoldSource<'source>,
}

/// Original collection IR paired with its source and checked literal limit.
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::FoldItemsSource;
/// let plan: FoldItemsSource<'_, ()> = FoldItemsSource {
///     source: todo!(), items: (), limit: 0,
/// };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::FoldItemsSource;
/// let plan: FoldItemsSource<'_, ()> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "prepare the initial state or deliberately discard the collection"]
pub struct FoldItemsSource<'source, Node> {
    source: FoldSource<'source>,
    items: Node,
    limit: u64,
}

/// Original collection, initial IR and move-only state observation. Private
/// fields keep the observed scalar state paired with its original source.
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::PreparedFoldSource;
/// let plan: PreparedFoldSource<'_, (), ()> = PreparedFoldSource {
///     collection: todo!(), initial: (), metadata: (), state_type: todo!(),
/// };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::PreparedFoldSource;
/// let plan: PreparedFoldSource<'_, (), ()> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "lower the cold next operand or deliberately discard the fold"]
pub struct PreparedFoldSource<'source, Node, Metadata> {
    collection: FoldItemsSource<'source, Node>,
    initial: Node,
    metadata: Metadata,
    state_type: ScalarType,
}

/// Original next IR and type observation, not yet admitted. Release the native
/// lexical guard before `finish` checks purity/state type and assembles the fold.
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::LoweredFoldSource;
/// let plan: LoweredFoldSource<'_, (), ()> = LoweredFoldSource {
///     prepared: todo!(), next: (), next_type: None,
/// };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::fold_source::LoweredFoldSource;
/// let plan: LoweredFoldSource<'_, (), ()> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "finish after scope cleanup or deliberately discard the fold"]
pub struct LoweredFoldSource<'source, Node, Metadata> {
    prepared: PreparedFoldSource<'source, Node, Metadata>,
    next: Node,
    next_type: Option<ScalarType>,
}

impl fmt::Debug for BoundFoldSource<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundFoldSource")
    }
}
impl<Node> fmt::Debug for FoldItemsSource<'_, Node> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FoldItemsSource")
    }
}
impl<Node, Metadata> fmt::Debug for PreparedFoldSource<'_, Node, Metadata> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedFoldSource")
    }
}
impl<Node, Metadata> fmt::Debug for LoweredFoldSource<'_, Node, Metadata> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LoweredFoldSource")
    }
}

/// Align one state plus `items`, `item`, `next` and `limit`. The item must be a
/// literal local name, bounded and distinct from the state. The caller selects
/// the fold operation; descendants and callee are not scanned by this alignment.
/// Use `scalar_source::lower_scalar_source_with_scope` for whole cold physical
/// and lexical admission. No execution, fuel, resource reservation or authority
/// is granted by these native stages.
pub fn bind_fold_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> StageResult<BoundFoldSource<'_>, Error> {
    fold_source(arguments, at).map(|source| BoundFoldSource { source })
}

impl BoundFoldSource<'_> {
    /// Check both names against the current borrowed scope, before limits and
    /// child compilation. Some(n) includes the prefix and two new locals. None
    /// deliberately omits a local quota: the native caller must bound it elsewhere.
    /// This observation does not reserve slots or authorize delayed scope entry.
    pub fn check_scope<Value, Error>(
        &self,
        scope: &ScopeFrame<'_, '_, Value>,
        max_bindings: Option<usize>,
    ) -> Result<(), ScalarSourceError<Error>> {
        self.source.check_scope(scope, max_bindings)
    }
}

impl<'source> BoundFoldSource<'source> {
    /// Check the literal iteration limit, then lower and admit collection IR
    /// once, before the initial state regardless of source argument order.
    /// Native callbacks must bound cold source/generated IR before returning,
    /// validate complete types/domains and current policy, and never execute
    /// effects here. Purity is derived from IR, not an adapter-supplied flag;
    /// the purity walk is not a resource meter or opaque-type certificate.
    pub fn prepare_items<Field, Operation, HostEffect, IrResult, Error>(
        self,
        lower: impl FnOnce(
            &'source Expression,
        )
            -> FoldSourceChild<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> StageResult<FoldItemsSource<'source, Node<Field, Operation, HostEffect, IrResult>>, Error>
    {
        let limit = self.source.limit()?;
        let expression = self.source.items;
        let (value, scalar_type) =
            lower(expression).map_err(|error| ScalarSourceError::Native {
                span: span(expression),
                error,
            })?;
        let items = fold_items(
            Operand {
                pure: value.is_pure(),
                value,
                scalar_type,
            },
            span(expression),
        )?;
        Ok(FoldItemsSource {
            source: self.source,
            items,
            limit,
        })
    }
}

impl<'source, Field, Operation, HostEffect, IrResult>
    FoldItemsSource<'source, Node<Field, Operation, HostEffect, IrResult>>
{
    /// Compile the initial state outside its own two-local frame, once. The
    /// original metadata moves unchanged; it is not inferred from the scalar tag.
    /// Native allocation, metadata interior mutation and Drop/unwind are trusted.
    pub fn prepare_initial<Metadata, Error>(
        self,
        lower: impl FnOnce(
            &'source Expression,
        ) -> FoldSourceInput<
            Node<Field, Operation, HostEffect, IrResult>,
            Metadata,
            Error,
        >,
    ) -> Prepared<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata, Error> {
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
        Ok(PreparedFoldSource {
            collection: self,
            initial,
            metadata,
            state_type,
        })
    }
}

impl<'source, Node, Metadata> PreparedFoldSource<'source, Node, Metadata> {
    pub fn binding(&self) -> &'source NamedArgument {
        self.collection.source.binding
    }
    pub fn item(&self) -> &'source str {
        self.collection.source.item
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn binding_error<Error>(&self, error: PureTypeError) -> ScalarSourceError<Error> {
        self.collection.source.binding_error(error)
    }
}

impl<'source, Field, Operation, HostEffect, IrResult, Metadata>
    PreparedFoldSource<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata>
{
    /// Compile next once even for an empty collection or zero limit. The adapter
    /// guards both native locals and rechecks current policy. Purity/state checks
    /// remain deferred until finish, after native scope cleanup. No retry, loop
    /// execution or rollback of callback side effects occurs on refusal/unwind.
    pub fn lower_next<Error>(
        self,
        lower: impl FnOnce(
            &'source Expression,
        )
            -> FoldSourceChild<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> Lowered<'source, Node<Field, Operation, HostEffect, IrResult>, Metadata, Error> {
        let expression = self.collection.source.next;
        let (next, next_type) = lower(expression).map_err(|error| ScalarSourceError::Native {
            span: span(expression),
            error,
        })?;
        Ok(LoweredFoldSource {
            prepared: self,
            next,
            next_type,
        })
    }
}

impl<Field, Operation, HostEffect, IrResult, Metadata>
    LoweredFoldSource<'_, Node<Field, Operation, HostEffect, IrResult>, Metadata>
{
    /// Check next and assemble the paired original nodes after scope cleanup.
    /// Another body's observation or an unchecked limit cannot be substituted.
    /// No native Clone/Debug/serde/Send bound, second IR tree or type-copy factory
    /// is needed. Full generated-IR/source-cost/type and live policy checks remain
    /// native responsibilities; this result carries no execution authority.
    pub fn finish<Error>(
        self,
    ) -> Output<Node<Field, Operation, HostEffect, IrResult>, Metadata, Error> {
        let Self {
            prepared,
            next,
            next_type,
        } = self;
        let PreparedFoldSource {
            collection,
            initial,
            metadata,
            state_type,
        } = prepared;
        let next = fold_next(
            Operand {
                pure: next.is_pure(),
                value: next,
                scalar_type: next_type,
            },
            state_type,
            span(collection.source.next),
        )?;
        Ok((
            collection
                .source
                .construct(collection.items, initial, next, collection.limit),
            metadata,
        ))
    }
}
