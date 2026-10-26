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
    let mut frames = Vec::new();
    let mut queued = 0usize;
    let mut current = (expression, depth);
    loop {
        let (node, depth) = current;
        budget.visit(depth, 0, 0)?;
        // Keep the former LIFO order and whole sibling-root budget fence,
        // but retain one cursor per branching ancestor instead of all siblings.
        let mut children = node.children().rev();
        budget.check_pending(queued, children.len())?;
        if let Some(child) = children.next() {
            let child_depth = depth.checked_add(1).ok_or(StructureError::DepthLimit)?;
            queued = queued
                .checked_add(children.len())
                .ok_or(StructureError::NodeLimit)?;
            if children.len() != 0 {
                frames.push((children, child_depth));
            }
            current = (child, child_depth);
            continue;
        }
        let mut next = None;
        while let Some((siblings, sibling_depth)) = frames.last_mut() {
            if let Some(sibling) = siblings.next() {
                queued = queued.checked_sub(1).ok_or(StructureError::NodeLimit)?;
                next = Some((sibling, *sibling_depth));
                if siblings.len() == 0 {
                    frames.pop();
                }
                break;
            }
            frames.pop();
        }
        let Some(next) = next else {
            return Ok(());
        };
        current = next;
    }
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

#[cfg(test)]
mod physical_cursor_tests {
    use super::physical;
    use crate::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
    use leselang_runtime_core::{
        BinaryOperator, ScalarValue, StructureBudget, StructureError, UnaryOperator,
    };

    type Node = Computation<(), (), (), ()>;
    fn leaf() -> Node {
        Node::Literal {
            value: ScalarValue::None,
        }
    }
    fn unary(value: Node) -> Node {
        Node::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(value),
        }
    }
    fn legacy(
        root: &Node,
        depth: usize,
        budget: &mut StructureBudget,
    ) -> Result<(), StructureError> {
        let mut pending = vec![(root, depth)];
        while let Some((node, depth)) = pending.pop() {
            budget.visit(depth, 0, 0)?;
            for child in node.children() {
                budget.check_pending(pending.len(), 1)?;
                pending.push((child, depth + 1));
            }
        }
        Ok(())
    }
    fn cases() -> Vec<Node> {
        vec![
            leaf(),
            Node::Local { name: "x".into() },
            Node::Member {
                group: "g".into(),
                name: "m".into(),
                operation: (),
            },
            Node::Host {
                effect: Box::new(()),
            },
            Node::Field {
                value: Box::new(leaf()),
                field: (),
            },
            unary(unary(unary(leaf()))),
            Node::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(unary(unary(leaf()))),
                right: Box::new(Node::Strings {
                    items: (0..8).map(|_| leaf()).collect(),
                }),
            },
            Node::Bind {
                name: "x".into(),
                value: Box::new(unary(leaf())),
                body: Box::new(leaf()),
            },
            Node::Recover {
                value: Box::new(leaf()),
                fallback: Box::new(unary(leaf())),
            },
            Node::Loop {
                name: "x".into(),
                initial: Box::new(leaf()),
                condition: Box::new(unary(leaf())),
                next: Box::new(leaf()),
                limit: 0,
            },
            Node::Fold {
                name: "x".into(),
                item: "y".into(),
                items: Box::new(leaf()),
                initial: Box::new(unary(leaf())),
                next: Box::new(leaf()),
                limit: 0,
            },
            Node::Choose {
                when: Box::new(unary(leaf())),
                then: Box::new(leaf()),
                otherwise: Box::new(unary(unary(leaf()))),
            },
            Node::Strings { items: vec![] },
            Node::Strings {
                items: vec![
                    leaf(),
                    unary(leaf()),
                    Node::Strings {
                        items: vec![leaf(), leaf()],
                    },
                ],
            },
            Node::Call {
                operation: (),
                arguments: (0..3)
                    .map(|index| ComputedArgument {
                        name: index.to_string(),
                        value: unary(leaf()),
                    })
                    .collect(),
            },
            Node::Group {
                group_kind: GroupKind::Parallel,
                branches: (0..3)
                    .map(|index| ComputedBranch {
                        name: index.to_string(),
                        value: unary(leaf()),
                        result_type: (),
                    })
                    .collect(),
            },
            Node::Group {
                group_kind: GroupKind::Sequence,
                branches: vec![],
            },
        ]
    }

    #[test]
    fn cursor_physical_walk_matches_legacy_counts_and_error_precedence_across_limits() {
        for root in cases() {
            for max_nodes in 0..=28 {
                for max_depth in 0..=5 {
                    for depth in 0..=4 {
                        for prefix in 0..=max_nodes.min(3) {
                            let mut current = StructureBudget::new(max_nodes, max_depth);
                            let mut old = StructureBudget::new(max_nodes, max_depth);
                            for _ in 0..prefix {
                                current.visit(0, 0, 0).unwrap();
                                old.visit(0, 0, 0).unwrap();
                            }
                            assert_eq!(
                                physical(&root, depth, &mut current),
                                legacy(&root, depth, &mut old)
                            );
                            assert_eq!(current.visited(), old.visited());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cursor_physical_walk_keeps_reverse_cold_order_and_whole_sibling_root_fences() {
        let root = Node::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(unary(unary(unary(leaf())))),
            right: Box::new(Node::Strings {
                items: (0..10).map(|_| leaf()).collect(),
            }),
        };
        let mut budget = StructureBudget::new(8, 2);
        assert_eq!(
            physical(&root, 0, &mut budget),
            Err(StructureError::NodeLimit)
        );
        assert_eq!(budget.visited(), 2);
        let mut budget = StructureBudget::new(16, 2);
        assert_eq!(
            physical(&root, 0, &mut budget),
            Err(StructureError::DepthLimit)
        );
        assert_eq!(budget.visited(), 14);
    }

    #[test]
    fn cursor_physical_walk_bounds_wide_nodes_without_clamping_or_consuming_original_ir() {
        let root = Node::Call {
            operation: (),
            arguments: (0..16_383)
                .map(|index| ComputedArgument {
                    name: index.to_string(),
                    value: leaf(),
                })
                .collect(),
        };
        let Node::Call { arguments, .. } = &root else {
            panic!()
        };
        let buffer = arguments.as_ptr();
        let mut budget = StructureBudget::new(16_384, 1);
        assert_eq!(physical(&root, 0, &mut budget), Ok(()));
        assert_eq!(budget.visited(), 16_384);
        let mut short = StructureBudget::new(16_383, 1);
        assert_eq!(
            physical(&root, 0, &mut short),
            Err(StructureError::NodeLimit)
        );
        assert_eq!(short.visited(), 1);
        assert_eq!(arguments.as_ptr(), buffer);
        assert_eq!(root.children().len(), 16_383);
        let mut shifted = StructureBudget::new(16_387, 3);
        for _ in 0..3 {
            shifted.visit(0, 0, 0).unwrap();
        }
        assert_eq!(physical(&root, 2, &mut shifted), Ok(()));
        assert_eq!(shifted.visited(), 16_387);
    }

    #[test]
    fn cursor_physical_walk_returns_structured_depth_overflow_and_keeps_node_error_priority() {
        let mut leaf_budget = StructureBudget::new(1, usize::MAX);
        assert_eq!(physical(&leaf(), usize::MAX, &mut leaf_budget), Ok(()));
        let root = unary(leaf());
        let mut depth = StructureBudget::new(2, usize::MAX);
        assert_eq!(
            physical(&root, usize::MAX, &mut depth),
            Err(StructureError::DepthLimit)
        );
        assert_eq!(depth.visited(), 1);
        let mut nodes = StructureBudget::new(1, usize::MAX);
        assert_eq!(
            physical(&root, usize::MAX, &mut nodes),
            Err(StructureError::NodeLimit)
        );
        assert_eq!(nodes.visited(), 1);
        let mut rejected_root = StructureBudget::new(0, usize::MAX - 1);
        assert_eq!(
            physical(&root, usize::MAX, &mut rejected_root),
            Err(StructureError::DepthLimit)
        );
        assert_eq!(rejected_root.visited(), 0);
    }
}
