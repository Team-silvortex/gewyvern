//! Native single-form scalar construction, not recursive source admission.

use std::fmt;

use leselang_runtime_core::ScalarType;
use leselang_syntax::Expression;

use crate::ir::Computation;
use crate::scalar_source::{Operand, ScalarSourceError, ScalarSourceResult, lower_form, primitive};
use crate::source_call::span;

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ScalarFormChild<Node, Error> = Result<(Node, Option<ScalarType>), Error>;

/// One original literal or recognized scalar call root, not a source/type or
/// authority receipt. Fields are private and the source is borrowed unchanged.
///
/// ```compile_fail
/// use leselang_hir_core::scalar_form::BoundScalarForm;
/// let form = BoundScalarForm { expression: todo!() };
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::scalar_form::BoundScalarForm;
/// let form: BoundScalarForm<'_> = serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "lower the scalar form or deliberately discard its classification"]
pub struct BoundScalarForm<'source> {
    expression: &'source Expression,
}

impl fmt::Debug for BoundScalarForm<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundScalarForm")
    }
}

/// Classify only the original root using the shared scalar vocabulary. References,
/// control forms and unknown/native callees return None for caller dispatch.
/// Shape, literal bounds and child checks remain deferred to lowering; this
/// neither scans descendants nor authorizes a named operation. Use the strict
/// `scalar_source::lower_scalar_source_with_scope` entry for bounded recursive
/// source/lexical admission instead of this native compiler boundary.
pub fn bind_scalar_form(expression: &Expression) -> Option<BoundScalarForm<'_>> {
    primitive(expression).then_some(BoundScalarForm { expression })
}

impl<'source> BoundScalarForm<'source> {
    /// Consume the original form through the shared scalar constructor. Native
    /// lowering owns bounded cold-source/IR construction, lexical types, opaque
    /// observations and current policy; bound returned IR before its purity walk.
    /// Language purity is derived from IR, never accepted as a native boolean.
    ///
    /// Binary/recovery children compile in semantic order before their joint
    /// type gate, including lazy/cold operands. String-list labels and entries
    /// retain their sequential checks. Literal list/optional-string construction
    /// moves owned buffers; no native Clone/Debug/serde/Send bound is added.
    /// No expression is evaluated, runtime error recovered, effect dispatched,
    /// quota refunded or native side effect rolled back. Refusal/unwind stops
    /// later callbacks and drops owned parts without retry. Native work, metadata
    /// mutation, allocation and Drop/unwind remain trusted and not preempted.
    pub fn lower<Field, Operation, HostEffect, IrResult, Error>(
        self,
        mut lower: impl FnMut(
            &'source Expression,
        )
            -> ScalarFormChild<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
        lower_form(self.expression, |expression| {
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
        })
    }
}
