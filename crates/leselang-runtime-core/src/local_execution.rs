//! Optional, thread-local lifetime fencing for native host work.

use std::{
    cell::Cell,
    fmt,
    rc::{Rc, Weak},
};

/// Unique owner of a local execution's cooperative liveness fence.
///
/// Closing or dropping the owner revokes all leases permanently. A lease cannot
/// keep the owner alive, reopen it, or close another execution. No host callback,
/// clock, scheduler, global counter, lock, or language fuel is involved. Creation
/// allocates one bounded state cell; hosts bound aggregate scopes and leases.
///
/// This is not authority or cancellation delivery: native work must check its
/// lease immediately before acting, alongside live schema/grants/provenance.
/// It cannot interrupt an active callback, undo effects, or provide durable or
/// cross-thread identity. Forgotten owners and process abort do not run Drop.
/// Use [`Self::bind`] to revoke before an owned session's native cleanup, not just
/// when a separately declared owner eventually leaves its surrounding scope.
///
/// ```
/// use leselang_runtime_core::LocalExecutionScope;
/// let scope = LocalExecutionScope::new();
/// let lease = scope.lease();
/// let session = scope.bind(vec![1, 2]);
/// assert!(lease.is_open());
/// drop(session);
/// assert!(!lease.is_open());
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::LocalExecutionScope;
/// let owner = LocalExecutionScope::new();
/// let duplicate = owner.clone();
/// ```
#[must_use = "retain the unique execution owner or bind it to the owned session"]
pub struct LocalExecutionScope {
    open: Rc<Cell<bool>>,
}

impl LocalExecutionScope {
    pub fn new() -> Self {
        Self {
            open: Rc::new(Cell::new(true)),
        }
    }

    pub fn lease(&self) -> LocalExecutionLease {
        LocalExecutionLease {
            open: Rc::downgrade(&self.open),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Idempotent and permanent. Does not cancel, drop or inspect host payloads.
    pub fn close(&self) -> bool {
        self.open.replace(false)
    }

    /// Retain a native value without Clone, Debug, Send or 'static requirements.
    /// Wrapper destruction revokes before dropping its currently owned value,
    /// including during unwinding. Explicit mutation remains host-controlled.
    pub fn bind<Value>(self, value: Value) -> ScopedLocalExecution<Value> {
        ScopedLocalExecution { scope: self, value }
    }
}

impl Default for LocalExecutionScope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LocalExecutionScope {
    fn drop(&mut self) {
        self.close();
    }
}

impl fmt::Debug for LocalExecutionScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalExecutionScope")
            .field("open", &self.is_open())
            .finish()
    }
}

/// Cloneable, non-owning observation of one local scope, never a dispatch grant.
///
/// `is_open` is a snapshot; host callbacks may re-enter and revoke after a check.
/// Recheck at actual dispatch/re-entry. Leases deliberately remain !Send/!Sync.
/// Their allocation identity survives closure without addresses or integer IDs
/// being exposed. Equality of scope alone does not authenticate a payload.
///
/// ```compile_fail
/// use leselang_runtime_core::LocalExecutionScope;
/// let lease = LocalExecutionScope::new().lease();
/// std::thread::spawn(move || lease.is_open());
/// ```
#[derive(Clone)]
#[must_use = "check local liveness alongside native authority before acting"]
pub struct LocalExecutionLease {
    open: Weak<Cell<bool>>,
}

impl LocalExecutionLease {
    pub fn is_open(&self) -> bool {
        self.open.upgrade().is_some_and(|open| open.get())
    }

    /// Process-local correlation only; still valid for comparing closed leases.
    pub fn same_scope(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.open, &other.open)
    }
}

impl fmt::Debug for LocalExecutionLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalExecutionLease")
            .field("open", &self.is_open())
            .finish()
    }
}

/// Native ownership wrapper, not another session implementation or evaluator.
///
/// Drop revokes before the value's destructor, including during unwinding.
/// Explicit closure retains the value for host cancellation or reconciliation.
/// Session terminal states are not inferred: hosts close on completion/failure,
/// deliver cancellation to their collectors/work queues, and route late replies.
/// Catching an unwind while retaining this wrapper does not close it; the host
/// must explicitly close after an uncertain outcome before permitting more work.
/// Native destructors remain trusted; a second panic while unwinding may abort.
#[must_use = "retain the guarded execution until completion or deliberate cancellation"]
pub struct ScopedLocalExecution<Value> {
    // Rust drops fields in declaration order: revoke before arbitrary native cleanup.
    scope: LocalExecutionScope,
    value: Value,
}

impl<Value> ScopedLocalExecution<Value> {
    pub fn lease(&self) -> LocalExecutionLease {
        self.scope.lease()
    }
    pub fn is_open(&self) -> bool {
        self.scope.is_open()
    }
    pub fn close(&self) -> bool {
        self.scope.close()
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Does not close the fence. Close first if replacing/dropping native state
    /// through this borrow must observe revocation; arbitrary host code is trusted.
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// Revoke before handing the original value back; does not run its destructor.
    pub fn into_inner(self) -> Value {
        let Self { scope, value } = self;
        drop(scope);
        value
    }
}

impl<Value> fmt::Debug for ScopedLocalExecution<Value> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedLocalExecution")
            .field("open", &self.is_open())
            .finish_non_exhaustive()
    }
}
