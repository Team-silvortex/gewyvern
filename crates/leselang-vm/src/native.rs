//! Product-free compilation and synchronous host-effect re-entry facade.
//!
//! These are exact shared types and implementations, not conversion wrappers or
//! a second VM. Hosts define catalogs, typed replies, live authority, dispatch,
//! aggregate bounds and cancellation delivery. Sessions borrow admitted IR and
//! retain the original fuel meter across reply-gated slices. They do not install
//! a timer, worker, global lock, journal or implicit async source semantics.
//! `LocalVm` opts into shared local scope ownership around the same session;
//! the unwrapped `Vm` keeps its original ownership and auto-trait behavior.
//! Coordinators may restore binding or final pending VMs under the same borrowed
//! local scope as their prepared collectors. Raw VMs do not retain that scope;
//! `try_accept_in_scope`, `poll_in_scope` and `cancel_in_scope` reuse `LocalVm`'s
//! lifecycle fence when passed that same owner. Keep the raw VM in a scope-bound
//! owner for drop ordering; ordinary unscoped methods remain unscoped. Dispatch
//! lease checks and external cancellation still belong to the host.
//! Scoped host-work/outcome methods also share `LocalVm`'s original pending-state
//! fence and meter. Native outcome classification and correct Fuel use remain
//! trusted adapter policy; a returned output is not an accepted receipt.
//!
//! The current native compiler/session profiles are not full reference-language,
//! durable-recovery or wire/auth/GUI compatibility certificates. Feature
//! unification may restore product dependencies; this is not a security sandbox.

pub use leselang_hir as hir;
pub use leselang_hir::effect_session::HostWorkOutcome;
pub use leselang_hir::effect_stack_session::StackedControlSession as Vm;
pub use leselang_hir::effect_stack_session::StackedSessionRestoreError;
pub use leselang_hir::local_effect_session::LocalStackedControlSession as LocalVm;
pub use leselang_runtime_core as core;
pub use leselang_syntax as syntax;
