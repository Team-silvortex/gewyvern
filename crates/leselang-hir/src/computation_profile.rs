//! Explicit host policy for specialized computation observations.
//!
//! A profile belongs to its host-effect type, so an adapter in another crate can
//! implement it for its own local type without defining inherent methods on a
//! foreign IR alias. There are no default product slots or blanket policies.
//!
//! These are host observations, not admission proofs. Even a positive result
//! grants no dispatch, capability, replay or durable commit authority. Profiles
//! preserve their own bounds, cold-branch rules and fault ordering. A profile
//! callback may fail or unwind; forwarding does not catch, retry or cache it.
//! Hosts that do not need this compatibility surface use the generic IR and
//! explicit compiler/evaluator APIs directly.
//!
//! ```compile_fail
//! use leselang_hir::ir::Computation;
//! use leselang_runtime_core::ScalarValue;
//! let tree: Computation<(), (), (), ()> = Computation::Literal {
//!     value: ScalarValue::None,
//! };
//! // No host policy was selected for the effect type `()`.
//! tree.validate_structure().unwrap();
//! ```

use crate::ir::Computation;
use std::collections::BTreeSet;

/// Host-owned policy; native slot types need not implement Clone, Debug or serde.
///
/// Implementations are responsible for validating untrusted trees before using
/// recursive classification. Read-only observations may not be used as a
/// replacement for the execution owner's current authorization checks.
pub trait ComputationProfile<Field, Operation, ResultType>: Sized {
    type Error;
    type GroupLocal;
    type Capability: Ord;

    fn validate_structure(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> Result<(), Self::Error>;

    fn is_atomic_tail(expression: &Computation<Field, Operation, Self, ResultType>) -> bool;

    fn prepared_atomic_operation(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> Option<Operation>;

    fn is_atomic_capture(expression: &Computation<Field, Operation, Self, ResultType>) -> bool;

    fn is_result_chain(expression: &Computation<Field, Operation, Self, ResultType>) -> bool;

    fn atomic_chain_bound(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> Option<usize>;

    fn atomic_flow_bound(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> Option<usize>;

    fn is_result_flow(expression: &Computation<Field, Operation, Self, ResultType>) -> bool;

    fn can_return_without_suspending(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> bool;

    fn validate_in_scope(
        expression: &Computation<Field, Operation, Self, ResultType>,
        scope: &[(String, ResultType)],
    ) -> Result<ResultType, Self::Error>;

    fn validate_in_group_scope(
        expression: &Computation<Field, Operation, Self, ResultType>,
        scope: &[(String, ResultType)],
        groups: &[Self::GroupLocal],
    ) -> Result<ResultType, Self::Error>;

    fn required_capabilities(
        expression: &Computation<Field, Operation, Self, ResultType>,
    ) -> BTreeSet<Self::Capability>;
}

impl<Field, Operation, HostEffect, ResultType> Computation<Field, Operation, HostEffect, ResultType>
where
    HostEffect: ComputationProfile<Field, Operation, ResultType>,
{
    /// Checks profile-defined structure, not lexical types or live authority.
    #[inline]
    pub fn validate_structure(&self) -> Result<(), HostEffect::Error> {
        HostEffect::validate_structure(self)
    }

    /// Profile classification of a pure-prepared atomic tail, not an admission proof.
    #[inline]
    pub fn is_atomic_tail(&self) -> bool {
        HostEffect::is_atomic_tail(self)
    }

    /// Profile observation of one prepared operation, without preparing or dispatching it.
    #[inline]
    pub fn prepared_atomic_operation(&self) -> Option<Operation> {
        HostEffect::prepared_atomic_operation(self)
    }

    /// Profile classification of an atomic suspension with a captured result.
    #[inline]
    pub fn is_atomic_capture(&self) -> bool {
        HostEffect::is_atomic_capture(self)
    }

    /// Profile classification of a bounded result chain.
    #[inline]
    pub fn is_result_chain(&self) -> bool {
        HostEffect::is_result_chain(self)
    }

    /// Profile maximum chain length; does not reserve graph slots.
    #[inline]
    pub fn atomic_chain_bound(&self) -> Option<usize> {
        HostEffect::atomic_chain_bound(self)
    }

    /// Profile bound including cold paths and scalar exits; does not reserve slots.
    #[inline]
    pub fn atomic_flow_bound(&self) -> Option<usize> {
        HostEffect::atomic_flow_bound(self)
    }

    /// Profile classification of a result flow with possible pure exits.
    #[inline]
    pub fn is_result_flow(&self) -> bool {
        HostEffect::is_result_flow(self)
    }

    /// Profile observation of a possible return without another suspension.
    #[inline]
    pub fn can_return_without_suspending(&self) -> bool {
        HostEffect::can_return_without_suspending(self)
    }

    /// Revalidates against the original borrowed lexical scope using host policy.
    #[inline]
    pub fn validate_in_scope(
        &self,
        scope: &[(String, ResultType)],
    ) -> Result<ResultType, HostEffect::Error> {
        HostEffect::validate_in_scope(self, scope)
    }

    /// Revalidates against borrowed lexical and group signatures using host policy.
    #[inline]
    pub fn validate_in_group_scope(
        &self,
        scope: &[(String, ResultType)],
        groups: &[HostEffect::GroupLocal],
    ) -> Result<ResultType, HostEffect::Error> {
        HostEffect::validate_in_group_scope(self, scope, groups)
    }

    /// Returns profile-declared capability names, never execution grants.
    #[inline]
    pub fn required_capabilities(&self) -> BTreeSet<HostEffect::Capability> {
        HostEffect::required_capabilities(self)
    }
}
