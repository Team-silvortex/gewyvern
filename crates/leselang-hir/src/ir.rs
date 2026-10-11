//! Shared control IR with explicit host-owned operation, field, effect and result types.
//!
//! Nodes are mutable, unchecked data, not executable continuations or authority.
//! Decoding does not validate names, bounds, types, schemas or native host payloads.
//! Receiving adapters bound ingress before decoding and validate all cold branches
//! before evaluation/dispatch. Native Clone/Debug/serde/Send requirements apply only
//! when using those traits, not when constructing or inspecting this IR.
//! Debug and serialization may expose native payloads; hosts own redaction and codecs.

use leselang_runtime_core::{
    BinaryOperator, ScalarArgumentType, ScalarType, ScalarValue, UnaryOperator,
};
use serde::{Deserialize, Serialize};
use std::iter::FusedIterator;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    Sequence,
    Parallel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedBranch<Expression, ResultType> {
    pub name: String,
    pub value: Expression,
    pub result_type: ResultType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedArgument<Expression> {
    pub name: String,
    pub value: Expression,
}

/// One control-node vocabulary; each host supplies all four explicit type parameters.
///
/// Host slots have no implicit reference-product defaults. Result types are declared
/// metadata, not proven types; operation keys are not execution grants. A Host effect
/// is opaque to the language-node walk and is validated separately by its adapter.
/// The reference Computation path is a direct specialization, not a conversion tree.
///
/// ```
/// use leselang_hir::ir::Computation;
/// use leselang_runtime_core::ScalarValue;
/// type DeviceIr = Computation<u8, u32, (), &'static str>;
/// let value = DeviceIr::Literal { value: ScalarValue::Integer(7) };
/// assert!(value.is_pure());
/// assert_eq!(value.children().count(), 0);
/// ```
///
/// ```
/// use leselang_hir::ir::Computation::{self, Literal};
/// use leselang_runtime_core::ScalarValue;
/// let value: Computation<(), (), (), ()> = Literal { value: ScalarValue::None };
/// assert!(value.is_pure());
/// ```
///
/// ```compile_fail
/// use leselang_hir::ir::Computation;
/// use leselang_runtime_core::ScalarValue;
/// struct Native;
/// let value: Computation<Native, Native, Native, Native> = Computation::Literal {
///     value: ScalarValue::None };
/// let wire = serde_json::to_string(&value).unwrap();
/// ```
///
/// ```compile_fail
/// use std::rc::Rc;
/// use leselang_hir::ir::Computation;
/// fn requires_send<T: Send>(_: T) {}
/// let value: Computation<(), (), Rc<()>, ()> = Computation::Host {
///     effect: Box::new(Rc::new(())) };
/// requires_send(value);
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Computation<Field, Operation, HostEffect, ResultType> {
    Literal {
        value: ScalarValue,
    },
    Strings {
        items: Vec<Self>,
    },
    Local {
        name: String,
    },
    Field {
        value: Box<Self>,
        field: Field,
    },
    Member {
        group: String,
        name: String,
        operation: Operation,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    Unary {
        operator: UnaryOperator,
        value: Box<Self>,
    },
    Bind {
        name: String,
        value: Box<Self>,
        body: Box<Self>,
    },
    Loop {
        name: String,
        initial: Box<Self>,
        condition: Box<Self>,
        next: Box<Self>,
        limit: u64,
    },
    Fold {
        name: String,
        item: String,
        items: Box<Self>,
        initial: Box<Self>,
        next: Box<Self>,
        limit: u64,
    },
    Choose {
        when: Box<Self>,
        then: Box<Self>,
        otherwise: Box<Self>,
    },
    Recover {
        value: Box<Self>,
        fallback: Box<Self>,
    },
    Host {
        effect: Box<HostEffect>,
    },
    Call {
        operation: Operation,
        arguments: Vec<ComputedArgument<Self>>,
    },
    Group {
        group_kind: GroupKind,
        branches: Vec<ComputedBranch<Self, ResultType>>,
    },
}

struct ChildNodes<'node, Field, Operation, HostEffect, ResultType> {
    node: &'node Computation<Field, Operation, HostEffect, ResultType>,
    front: usize,
    back: usize,
}

impl<'node, Field, Operation, HostEffect, ResultType>
    ChildNodes<'node, Field, Operation, HostEffect, ResultType>
{
    fn get(
        &self,
        index: usize,
    ) -> Option<&'node Computation<Field, Operation, HostEffect, ResultType>> {
        use Computation::*;
        match self.node {
            Binary { left, right, .. } => [left.as_ref(), right.as_ref()].get(index).copied(),
            Bind { value, body, .. } => [value.as_ref(), body.as_ref()].get(index).copied(),
            Recover { value, fallback } => [value.as_ref(), fallback.as_ref()].get(index).copied(),
            Unary { value, .. } | Field { value, .. } => (index == 0).then_some(value.as_ref()),
            Loop {
                initial,
                condition,
                next,
                ..
            } => [initial.as_ref(), condition.as_ref(), next.as_ref()]
                .get(index)
                .copied(),
            Fold {
                items,
                initial,
                next,
                ..
            } => [items.as_ref(), initial.as_ref(), next.as_ref()]
                .get(index)
                .copied(),
            Choose {
                when,
                then,
                otherwise,
            } => [when.as_ref(), then.as_ref(), otherwise.as_ref()]
                .get(index)
                .copied(),
            Strings { items } => items.get(index),
            Call { arguments, .. } => arguments.get(index).map(|argument| &argument.value),
            Group { branches, .. } => branches.get(index).map(|branch| &branch.value),
            Literal { .. } | Local { .. } | Member { .. } | Host { .. } => None,
        }
    }
}

impl<'node, Field, Operation, HostEffect, ResultType> Iterator
    for ChildNodes<'node, Field, Operation, HostEffect, ResultType>
{
    type Item = &'node Computation<Field, Operation, HostEffect, ResultType>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        self.get(index)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n >= self.back - self.front {
            self.front = self.back;
            return None;
        }
        self.front += n;
        self.next()
    }

    fn count(self) -> usize {
        self.back - self.front
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<Field, Operation, HostEffect, ResultType> DoubleEndedIterator
    for ChildNodes<'_, Field, Operation, HostEffect, ResultType>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.get(self.back)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if n >= self.back - self.front {
            self.front = self.back;
            return None;
        }
        self.back -= n;
        self.next_back()
    }
}

impl<Field, Operation, HostEffect, ResultType> ExactSizeIterator
    for ChildNodes<'_, Field, Operation, HostEffect, ResultType>
{
}

impl<Field, Operation, HostEffect, ResultType> FusedIterator
    for ChildNodes<'_, Field, Operation, HostEffect, ResultType>
{
}

impl<Field, Operation, HostEffect, ResultType>
    Computation<Field, Operation, HostEffect, ResultType>
{
    /// Bridge a trusted inferred scalar type to native signature preflight.
    ///
    /// Purity includes cold children; a direct literal is borrowed without cloning.
    /// The supplied type is not inferred or trusted by this method, and unknown
    /// locals/fields/operations are not validated. Bound structure before this
    /// purity walk, whose branching cursor stack may allocate. The receiving schema checks
    /// literal/type consistency; evaluated values still require domain preflight.
    pub fn scalar_argument_type(&self, scalar_type: Option<ScalarType>) -> ScalarArgumentType<'_> {
        ScalarArgumentType {
            scalar_type,
            is_pure: self.is_pure(),
            literal: match self {
                Self::Literal { value } => Some(value),
                _ => None,
            },
        }
    }

    /// Borrow direct language children in declaration order, including cold branches.
    /// This allocation-free structural inspection does not evaluate guards, charge
    /// fuel, inspect native metadata/effect graphs, or approve types/authority.
    /// Length/count and forward/reverse skips are constant-time metadata queries.
    /// Exact length is not an ingress bound or proof of native payload validity.
    pub fn children(
        &self,
    ) -> impl DoubleEndedIterator<Item = &Self> + ExactSizeIterator + FusedIterator {
        let back = match self {
            Self::Binary { .. } | Self::Bind { .. } | Self::Recover { .. } => 2,
            Self::Unary { .. } | Self::Field { .. } => 1,
            Self::Loop { .. } | Self::Fold { .. } | Self::Choose { .. } => 3,
            Self::Strings { items } => items.len(),
            Self::Call { arguments, .. } => arguments.len(),
            Self::Group { branches, .. } => branches.len(),
            Self::Literal { .. } | Self::Local { .. } | Self::Member { .. } | Self::Host { .. } => {
                0
            }
        };
        ChildNodes {
            node: self,
            front: 0,
            back,
        }
    }

    /// Classify language nodes only, without native callbacks. Host/Call/Group
    /// nodes are effectful even if empty, invalid or in a cold branch. Member is
    /// a result reference, not a new effect. Bound structure before this walk;
    /// only open branching ancestors retain cursors, whose stack may allocate.
    /// Leaves and unary chains need no scratch allocation. This is not a lasting
    /// purity certificate, structural bound, or native payload admission.
    pub fn is_pure(&self) -> bool {
        let mut frames = Vec::new();
        let mut expression = self;
        loop {
            if matches!(
                expression,
                Self::Host { .. } | Self::Call { .. } | Self::Group { .. }
            ) {
                return false;
            }
            let mut children = expression.children().rev();
            if let Some(child) = children.next() {
                if children.len() != 0 {
                    frames.push(children);
                }
                expression = child;
                continue;
            }
            let mut next = None;
            while let Some(siblings) = frames.last_mut() {
                if let Some(sibling) = siblings.next() {
                    next = Some(sibling);
                    if siblings.len() == 0 {
                        frames.pop();
                    }
                    break;
                }
                frames.pop();
            }
            let Some(child) = next else {
                return true;
            };
            expression = child;
        }
    }
}
