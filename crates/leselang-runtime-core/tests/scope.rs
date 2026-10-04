use leselang_runtime_core::{ScopeError, ScopeFrame};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn borrowed_names_and_nonclone_values_retain_original_storage_and_order() {
    struct Opaque(Box<u64>);
    let name = String::from("private-name");
    let value = Opaque(Box::new(7));
    let value_pointer = &*value.0 as *const u64;
    let mut bindings = vec![];
    let mut frame = ScopeFrame::new(&mut bindings);
    assert!(frame.is_empty());
    let slot = frame.push(&name, value).unwrap();
    assert_eq!(slot, 0);
    assert_eq!(frame.bindings()[0].0.as_ptr(), name.as_ptr());
    assert_eq!(&*frame.get(&name).unwrap().0 as *const u64, value_pointer);
    *frame.get_local_mut(slot).unwrap().0 = 9;
    let (key, moved) = frame.pop().unwrap();
    assert_eq!(key.as_ptr(), name.as_ptr());
    assert_eq!(&*moved.0 as *const u64, value_pointer);
    assert_eq!(*moved.0, 9);
    assert!(frame.is_empty());
}

#[test]
fn frame_relative_slots_and_pop_never_reach_the_visible_parent_prefix() {
    let mut bindings = vec![("outer", 7)];
    {
        let mut frame = ScopeFrame::new(&mut bindings);
        assert_eq!(frame.len(), 1);
        assert_eq!(frame.local_len(), 0);
        assert_eq!(frame.get("outer"), Some(&7));
        assert_eq!(frame.pop(), None);
        for slot in [0, 1, usize::MAX - 1, usize::MAX] {
            assert_eq!(frame.get_local_mut(slot), None);
        }
        assert_eq!(frame.push("local", 3), Ok(0));
        *frame.get_local_mut(0).unwrap() = 4;
        assert_eq!(frame.get("outer"), Some(&7));
        assert_eq!(frame.bindings(), [("outer", 7), ("local", 4)]);
        assert_eq!(frame.get_local_mut(usize::MAX), None);
        assert_eq!(frame.pop(), Some(("local", 4)));
        assert_eq!(frame.pop(), None);
    }
    assert_eq!(bindings, [("outer", 7)]);
}

#[test]
fn active_duplicates_leave_prefix_unchanged_but_closed_names_can_be_reused() {
    let mut bindings = vec![("outer", 7)];
    let mut frame = ScopeFrame::new(&mut bindings);
    assert_eq!(frame.push("outer", 9), Err(ScopeError::DuplicateBinding));
    assert_eq!(frame.bindings(), [("outer", 7)]);
    frame.push("local", 1).unwrap();
    {
        let mut inner = frame.nested();
        assert_eq!(inner.push("local", 2), Err(ScopeError::DuplicateBinding));
        assert_eq!(inner.local_len(), 0);
        inner.push("temporary", 3).unwrap();
    }
    assert_eq!(frame.get("temporary"), None);
    assert_eq!(frame.push("temporary", 4), Ok(1));
    assert_eq!(frame.pop(), Some(("temporary", 4)));
    assert_eq!(frame.push("temporary", 5), Ok(1));
    assert_eq!(frame.local_len(), 2);
}

#[test]
fn nested_success_and_error_paths_automatically_restore_their_own_bases() {
    fn fail(frame: &mut ScopeFrame<'_, '_, u64>) -> Result<(), &'static str> {
        let mut inner = frame.nested();
        inner.push("temporary", 2).unwrap();
        let mut deeper = inner.nested();
        deeper.push("deep", 3).unwrap();
        Err("calculation failed")
    }
    let mut bindings = vec![("outer", 7)];
    {
        let mut frame = ScopeFrame::new(&mut bindings);
        frame.push("local", 1).unwrap();
        assert_eq!(fail(&mut frame), Err("calculation failed"));
        assert_eq!(frame.bindings(), [("outer", 7), ("local", 1)]);
        {
            let mut inner = frame.nested();
            inner.push("temporary", 4).unwrap();
            assert_eq!(inner.pop(), Some(("temporary", 4)));
        }
        assert_eq!(frame.local_len(), 1);
    }
    assert_eq!(bindings, [("outer", 7)]);
}

struct Tracked {
    id: &'static str,
    drops: Rc<RefCell<Vec<&'static str>>>,
    panic: bool,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.panic, "adapter destructor failed");
    }
}
fn tracked(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>, panic: bool) -> Tracked {
    Tracked {
        id,
        drops: Rc::clone(drops),
        panic,
    }
}

#[test]
fn ordinary_cleanup_releases_nested_bindings_in_reverse_insertion_order_once() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = vec![("outer", tracked("outer", &drops, false))];
    {
        let mut frame = ScopeFrame::new(&mut bindings);
        frame.push("a", tracked("a", &drops, false)).unwrap();
        {
            let mut inner = frame.nested();
            inner.push("b", tracked("b", &drops, false)).unwrap();
            inner.push("c", tracked("c", &drops, false)).unwrap();
        }
        frame.push("d", tracked("d", &drops, false)).unwrap();
    }
    assert_eq!(*drops.borrow(), ["c", "b", "d", "a"]);
    assert_eq!(bindings.len(), 1);
    drop(bindings);
    assert_eq!(*drops.borrow(), ["c", "b", "d", "a", "outer"]);
}

#[test]
fn body_unwind_drops_every_local_but_keeps_parent_values_and_names() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = vec![("outer", tracked("outer", &drops, false))];
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let mut frame = ScopeFrame::new(&mut bindings);
        frame.push("a", tracked("a", &drops, false)).unwrap();
        let mut inner = frame.nested();
        inner.push("b", tracked("b", &drops, false)).unwrap();
        panic!("evaluation unwound");
    }));
    assert!(unwind.is_err());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "outer");
    assert_eq!(*drops.borrow(), ["b", "a"]);
}

#[test]
fn destructor_unwind_cannot_leave_detached_local_bindings_visible_again() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = vec![("outer", tracked("outer", &drops, false))];
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let mut frame = ScopeFrame::new(&mut bindings);
        frame.push("a", tracked("a", &drops, false)).unwrap();
        frame.push("b", tracked("b", &drops, false)).unwrap();
        frame.push("panic", tracked("panic", &drops, true)).unwrap();
    }));
    assert!(unwind.is_err());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, "outer");
    let mut released = drops.borrow().clone();
    released.sort();
    assert_eq!(released, ["a", "b", "panic"]);
    let mut frame = ScopeFrame::new(&mut bindings);
    frame.push("a", tracked("new-a", &drops, false)).unwrap();
    assert_eq!(frame.local_len(), 1);
}

#[test]
fn rejected_duplicate_releases_only_the_new_value_and_errors_echo_no_name() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = vec![("private-name", tracked("outer", &drops, false))];
    let mut frame = ScopeFrame::new(&mut bindings);
    let error = frame
        .push("private-name", tracked("rejected", &drops, false))
        .unwrap_err();
    assert_eq!(error, ScopeError::DuplicateBinding);
    assert_eq!(
        error.to_string(),
        "lexical binding duplicates an active name"
    );
    assert!(!format!("{error:?}").contains("private-name"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(*drops.borrow(), ["rejected"]);
    assert_eq!(frame.len(), 1);
    assert_eq!(frame.local_len(), 0);
}

#[test]
fn metadata_debug_does_not_require_or_invoke_value_formatters() {
    struct Private(Rc<Cell<usize>>);
    impl std::fmt::Debug for Private {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.set(self.0.get() + 1);
            panic!("payload formatter must not run")
        }
    }
    let calls = Rc::new(Cell::new(0));
    let mut bindings = vec![("private-name", Private(Rc::clone(&calls)))];
    let mut frame = ScopeFrame::new(&mut bindings);
    frame
        .push("private-local", Private(Rc::clone(&calls)))
        .unwrap();
    assert_eq!(
        format!("{frame:?}"),
        "ScopeFrame { visible_bindings: 2, local_bindings: 1 }"
    );
    assert_eq!(calls.get(), 0);
    struct NoDebug;
    let mut opaque = vec![("opaque", NoDebug)];
    assert!(format!("{:?}", ScopeFrame::new(&mut opaque)).contains("visible_bindings: 1"));
}

#[test]
fn worker_handoff_is_conditional_and_gui_local_values_need_no_send_bound() {
    let mut bindings = vec![("outer", 7)];
    std::thread::scope(|threads| {
        let mut frame = ScopeFrame::new(&mut bindings);
        frame.push("worker", 9).unwrap();
        threads
            .spawn(move || {
                assert_eq!(frame.get("worker"), Some(&9));
            })
            .join()
            .unwrap();
    });
    assert_eq!(bindings, [("outer", 7)]);
    let state = Rc::new(Cell::new(1));
    let mut local = vec![];
    let mut frame = ScopeFrame::new(&mut local);
    frame.push("gui", Rc::clone(&state)).unwrap();
    frame.get("gui").unwrap().set(2);
    assert_eq!(state.get(), 2);
}

#[test]
fn prefix_validation_and_slot_lifetimes_remain_explicit_adapter_responsibilities() {
    let mut bindings = vec![("duplicate", 1), ("duplicate", 2)];
    let mut frame = ScopeFrame::new(&mut bindings);
    assert_eq!(frame.get("duplicate"), Some(&1));
    assert_eq!(
        frame.push("duplicate", 3),
        Err(ScopeError::DuplicateBinding)
    );
    let first = frame.push("temporary", 4).unwrap();
    assert_eq!(frame.pop(), Some(("temporary", 4)));
    let reused = frame.push("different", 5).unwrap();
    assert_eq!(first, reused);
    assert_eq!(frame.get_local_mut(first), Some(&mut 5));
}
