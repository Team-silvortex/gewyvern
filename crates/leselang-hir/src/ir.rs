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

impl<Field, Operation, HostEffect, ResultType>
    Computation<Field, Operation, HostEffect, ResultType>
{
    /// Bridge a trusted inferred scalar type to native signature preflight.
    ///
    /// Purity includes cold children; a direct literal is borrowed without cloning.
    /// The supplied type is not inferred or trusted by this method, and unknown
    /// locals/fields/operations are not validated. Bound structure before this
    /// purity walk, which allocates its frontier. The receiving schema checks
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
    pub fn children(&self) -> impl DoubleEndedIterator<Item = &Self> + FusedIterator {
        let children: [Option<&Self>; 3] = match self {
            Self::Binary { left, right, .. } => [Some(left), Some(right), None],
            Self::Unary { value, .. } | Self::Field { value, .. } => [Some(value), None, None],
            Self::Bind { value, body, .. } => [Some(value), Some(body), None],
            Self::Recover { value, fallback } => [Some(value), Some(fallback), None],
            Self::Loop {
                initial,
                condition,
                next,
                ..
            } => [Some(initial), Some(condition), Some(next)],
            Self::Fold {
                items,
                initial,
                next,
                ..
            } => [Some(items), Some(initial), Some(next)],
            Self::Choose {
                when,
                then,
                otherwise,
            } => [Some(when), Some(then), Some(otherwise)],
            Self::Literal { .. }
            | Self::Strings { .. }
            | Self::Local { .. }
            | Self::Member { .. }
            | Self::Host { .. }
            | Self::Call { .. }
            | Self::Group { .. } => [None, None, None],
        };
        let arguments = match self {
            Self::Call { arguments, .. } => arguments.as_slice(),
            _ => &[],
        };
        let branches = match self {
            Self::Group { branches, .. } => branches.as_slice(),
            _ => &[],
        };
        let items = match self {
            Self::Strings { items } => items.as_slice(),
            _ => &[],
        };
        children
            .into_iter()
            .flatten()
            .chain(arguments.iter().map(|argument| &argument.value))
            .chain(branches.iter().map(|branch| &branch.value))
            .chain(items.iter())
    }

    /// Classify language nodes only, without native callbacks. Host/Call/Group
    /// nodes are effectful even if empty, invalid or in a cold branch. Member is
    /// a result reference, not a new effect. Bound structure before this walk;
    /// its pending frontier allocates and this is not a lasting purity certificate.
    pub fn is_pure(&self) -> bool {
        let mut pending = vec![self];
        while let Some(expression) = pending.pop() {
            if matches!(
                expression,
                Self::Host { .. } | Self::Call { .. } | Self::Group { .. }
            ) {
                return false;
            }
            pending.extend(expression.children());
        }
        true
    }
}
