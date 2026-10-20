use leselang_runtime_core::{LocalExecutionLease, LocalExecutionScope};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn owner_close_and_drop_permanently_revoke_non_owning_leases() {
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    let copied = lease.clone();
    let other = LocalExecutionScope::new();
    assert!(lease.is_open() && copied.is_open());
    assert!(lease.same_scope(&copied));
    assert!(!lease.same_scope(&other.lease()));
    assert!(owner.close());
    assert!(!owner.close());
    assert!(!owner.lease().is_open());
    assert!(!lease.is_open() && !copied.is_open());
    drop(owner);
    assert!(lease.same_scope(&copied));
    assert!(other.is_open());
    let abandoned = other.lease();
    drop(other);
    assert!(!abandoned.is_open());
}

struct Native<'a> {
    lease: LocalExecutionLease,
    drops: &'a Cell<usize>,
    panic_on_drop: bool,
    value: Rc<Cell<usize>>,
}
impl Drop for Native<'_> {
    fn drop(&mut self) {
        assert!(
            !self.lease.is_open(),
            "native cleanup must observe revocation"
        );
        self.drops.set(self.drops.get() + 1);
        assert!(!self.panic_on_drop, "native cleanup failed");
    }
}

#[test]
fn bound_owner_revokes_before_move_only_borrowed_native_cleanup() {
    let drops = Cell::new(0);
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    let value = Rc::new(Cell::new(7));
    let mut guarded = owner.bind(Native {
        lease: lease.clone(),
        drops: &drops,
        panic_on_drop: false,
        value: value.clone(),
    });
    assert!(guarded.lease().same_scope(&lease));
    guarded.value_mut().value.set(9);
    assert_eq!(guarded.value().value.get(), 9);
    // Debug has no native Debug bound and never formats the native payload.
    assert_eq!(
        format!("{guarded:?}"),
        "ScopedLocalExecution { open: true, .. }"
    );
    drop(guarded);
    assert!(!lease.is_open());
    assert_eq!(drops.get(), 1);
    assert_eq!(Rc::strong_count(&value), 1);
}

#[test]
fn explicit_close_keeps_original_payload_until_host_cleanup() {
    let owner = LocalExecutionScope::default();
    let payload = vec![1_u8, 2, 3].into_boxed_slice();
    let pointer = payload.as_ptr();
    let guarded = owner.bind(payload);
    let lease = guarded.lease();
    assert!(guarded.close());
    assert!(!guarded.close());
    assert!(!guarded.is_open());
    assert_eq!(guarded.value().as_ptr(), pointer);
    let original = guarded.into_inner();
    assert!(!lease.is_open());
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(&*original, &[1, 2, 3]);
}

#[test]
fn extraction_closes_before_returning_a_still_owned_native_value() {
    let drops = Cell::new(0);
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    let native = owner
        .bind(Native {
            lease: lease.clone(),
            drops: &drops,
            panic_on_drop: false,
            value: Rc::new(Cell::new(1)),
        })
        .into_inner();
    assert!(!lease.is_open());
    assert_eq!(drops.get(), 0);
    drop(native);
    assert_eq!(drops.get(), 1);
}

#[test]
fn scope_unwind_revokes_before_cleanup_without_poisoning_a_sibling() {
    let drops = Cell::new(0);
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    let sibling = LocalExecutionScope::new();
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let _guarded = owner.bind(Native {
            lease: lease.clone(),
            drops: &drops,
            panic_on_drop: false,
            value: Rc::new(Cell::new(1)),
        });
        panic!("host callback failed");
    }));
    assert!(caught.is_err());
    assert!(!lease.is_open());
    assert_eq!(drops.get(), 1);
    assert!(sibling.is_open());
}

#[test]
fn native_cleanup_unwind_cannot_rearm_an_abandoned_execution() {
    let drops = Cell::new(0);
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    let guarded = owner.bind(Native {
        lease: lease.clone(),
        drops: &drops,
        panic_on_drop: true,
        value: Rc::new(Cell::new(1)),
    });
    assert!(catch_unwind(AssertUnwindSafe(|| drop(guarded))).is_err());
    assert!(!lease.is_open());
    assert_eq!(drops.get(), 1);
}

#[test]
fn closed_scope_binding_and_repeated_queries_cannot_grant_or_execute_work() {
    let owner = LocalExecutionScope::new();
    assert!(owner.close());
    let lease = owner.lease();
    let guarded = owner.bind(11);
    for _ in 0..64 {
        assert!(!lease.is_open());
        assert!(!guarded.is_open());
        assert!(!guarded.close());
        assert_eq!(*guarded.value(), 11);
    }
    assert_eq!(format!("{lease:?}"), "LocalExecutionLease { open: false }");
}

#[test]
fn borrowed_parts_reuse_original_scope_and_retain_move_only_native_owner() {
    let drops = Cell::new(0);
    let scope = LocalExecutionScope::new();
    let lease = scope.lease();
    let sibling = LocalExecutionScope::new();
    let mut owner = scope.bind(Native {
        lease: lease.clone(),
        drops: &drops,
        panic_on_drop: false,
        value: Rc::new(Cell::new(7)),
    });
    let pointer = Rc::as_ptr(&owner.value().value);
    {
        let (borrowed, native) = owner.as_parts_mut();
        assert!(borrowed.lease().same_scope(&lease));
        assert_eq!(Rc::as_ptr(&native.value), pointer);
        native.value.set(11);
        assert!(borrowed.close());
        assert!(!lease.is_open());
        assert_eq!(drops.get(), 0);
    }
    assert!(!owner.is_open());
    assert_eq!(owner.value().value.get(), 11);
    assert!(sibling.is_open());
    drop(owner);
    assert_eq!(drops.get(), 1);
}

#[test]
fn borrowing_parts_does_not_implicitly_close_or_claim_to_fence_host_unwind() {
    let mut owner = LocalExecutionScope::new().bind(Box::new(3));
    let pointer = &**owner.value() as *const i32;
    let lease = owner.lease();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let (scope, value) = owner.as_parts_mut();
            assert!(scope.is_open());
            assert_eq!(&**value as *const i32, pointer);
            **value = 9;
            panic!("host must reconcile arbitrary native mutation");
        }))
        .is_err()
    );
    assert!(lease.is_open());
    assert_eq!(**owner.value(), 9);
    owner.close();
    let (scope, value) = owner.as_parts_mut();
    assert!(!scope.is_open());
    assert_eq!(&**value as *const i32, pointer);
}
