use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    OrderedOutcomeSummary, OutcomeOrder as Order, OutcomeSummaryError as Error,
};

#[test]
fn explicit_cold_member_limits_and_empty_summaries_do_not_construct_stops() {
    assert!(matches!(
        OrderedOutcomeSummary::<()>::new(Order::Sequential, 65, 1),
        Err(Error::InvalidLimits)
    ));
    for order in [Order::Sequential, Order::Parallel] {
        let summary = OrderedOutcomeSummary::<()>::new(order, 0, 0)
            .unwrap()
            .finish(|_, _| panic!())
            .unwrap();
        assert_eq!(summary.observed_members, 0);
        assert_eq!(summary.output_items, 0);
        assert!(summary.first_stop.is_none());
    }
}

#[test]
fn inclusive_sixty_four_member_limit_refuses_and_closes_without_truncation() {
    let mut pass = OrderedOutcomeSummary::<()>::new(Order::Sequential, 64, 64).unwrap();
    for _ in 0..64 {
        pass.observe_success(1, |_, _| panic!()).unwrap();
    }
    assert_eq!(
        pass.observe_success(0, |_, _| panic!()),
        Err(Error::MemberLimit)
    );
    assert_eq!(pass.has_stop(), Err(Error::Closed));
    assert_eq!(pass.observe_stop(()), Err(Error::Closed));
    assert!(matches!(pass.finish(|_, _| panic!()), Err(Error::Closed)));
}

#[test]
fn sequential_first_stop_and_first_output_overflow_follow_original_observation_order() {
    for stop_first in [false, true] {
        let calls = Cell::new(0);
        let mut pass = OrderedOutcomeSummary::new(Order::Sequential, 4, 2).unwrap();
        if stop_first {
            pass.observe_stop("native-stop").unwrap();
        }
        pass.observe_success(2, |_, _| panic!()).unwrap();
        pass.observe_success(1, |items, limit| {
            calls.set(calls.get() + 1);
            assert_eq!((items, limit), (3, 2));
            "output-stop"
        })
        .unwrap();
        if !stop_first {
            pass.observe_stop("native-stop").unwrap();
        }
        pass.observe_success(1, |_, _| panic!()).unwrap();
        let summary = pass.finish(|_, _| panic!()).unwrap();
        assert_eq!(summary.output_items, 4);
        assert_eq!(summary.observed_members, 4);
        assert_eq!(
            summary.first_stop,
            Some(if stop_first {
                "native-stop"
            } else {
                "output-stop"
            })
        );
        assert_eq!(calls.get(), usize::from(!stop_first));
    }
}

#[test]
fn parallel_output_overflow_is_deferred_and_overrides_original_first_stop_only_at_finish() {
    let mut pass = OrderedOutcomeSummary::new(Order::Parallel, 4, 2).unwrap();
    pass.observe_stop("first-native-stop").unwrap();
    pass.observe_success(2, |_, _| panic!()).unwrap();
    pass.observe_stop("later-native-stop").unwrap();
    pass.observe_success(3, |_, _| panic!()).unwrap();
    let calls = Cell::new(0);
    let summary = pass
        .finish(|items, limit| {
            calls.set(calls.get() + 1);
            assert_eq!((items, limit), (5, 2));
            "aggregate-output-stop"
        })
        .unwrap();
    assert_eq!(summary.first_stop, Some("aggregate-output-stop"));
    assert_eq!(calls.get(), 1);
}

#[test]
fn parallel_without_overflow_keeps_first_declared_stop_not_the_lowest_identity() {
    let mut pass = OrderedOutcomeSummary::new(Order::Parallel, 3, 1).unwrap();
    pass.observe_stop(90).unwrap();
    pass.observe_stop(2).unwrap();
    pass.observe_success(1, |_, _| panic!()).unwrap();
    assert_eq!(pass.finish(|_, _| panic!()).unwrap().first_stop, Some(90));
}

#[test]
fn full_width_counts_saturate_and_zero_weight_observations_remain_members() {
    for order in [Order::Sequential, Order::Parallel] {
        let mut pass = OrderedOutcomeSummary::<()>::new(order, 3, usize::MAX).unwrap();
        for items in [usize::MAX, 1, 0] {
            pass.observe_success(items, |_, _| panic!()).unwrap();
        }
        let summary = pass.finish(|_, _| panic!()).unwrap();
        assert_eq!(summary.output_items, usize::MAX);
        assert_eq!(summary.observed_members, 3);
        assert!(summary.first_stop.is_none());
    }
}

struct Stop {
    bytes: Box<[u8]>,
    id: usize,
    drops: Rc<RefCell<Vec<usize>>>,
    panic: bool,
}
impl Drop for Stop {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.panic, "native stop cleanup");
    }
}
fn stop(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Stop {
    Stop {
        bytes: b"private-stop".to_vec().into_boxed_slice(),
        id,
        drops: drops.clone(),
        panic: false,
    }
}

#[test]
fn move_only_gui_local_stops_keep_payload_identity_and_release_ignored_stops_before_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let original = stop(1, &drops);
    let pointer = original.bytes.as_ptr();
    let mut pass = OrderedOutcomeSummary::new(Order::Parallel, 2, 10).unwrap();
    pass.observe_stop(original).unwrap();
    pass.observe_stop(stop(2, &drops)).unwrap();
    assert_eq!(*drops.borrow(), [2]);
    assert!(!format!("{pass:?}").contains("private-stop"));
    let summary = pass.finish(|_, _| panic!()).unwrap();
    let original = summary.first_stop.unwrap();
    assert_eq!(original.bytes.as_ptr(), pointer);
    drop(original);
    assert_eq!(*drops.borrow(), [2, 1]);
}

#[test]
fn sequential_native_factory_unwind_closes_the_pass_without_refill_or_retry() {
    let mut pass = OrderedOutcomeSummary::<()>::new(Order::Sequential, 2, 0).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = pass.observe_success(1, |_, _| panic!("native factory"));
        }))
        .is_err()
    );
    assert_eq!(pass.has_stop(), Err(Error::Closed));
    assert_eq!(pass.observe_success(0, |_, _| panic!()), Err(Error::Closed));
}

#[test]
fn ignored_stop_cleanup_unwind_closes_and_releases_original_partial_stop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut pass = OrderedOutcomeSummary::new(Order::Sequential, 2, 1).unwrap();
    pass.observe_stop(stop(1, &drops)).unwrap();
    let mut ignored = stop(2, &drops);
    ignored.panic = true;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = pass.observe_stop(ignored);
        }))
        .is_err()
    );
    assert_eq!(pass.has_stop(), Err(Error::Closed));
    assert_eq!(*drops.borrow(), [2, 1]);
}

struct Owner(Rc<Cell<usize>>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        panic!("uncalled native factory cleanup");
    }
}

#[test]
fn uncalled_factory_cleanup_unwind_closes_success_before_state_reopens() {
    let calls = Rc::new(Cell::new(0));
    let owner = Owner(calls.clone());
    let mut pass = OrderedOutcomeSummary::<()>::new(Order::Parallel, 1, 0).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = pass.observe_success(1, move |_, _| {
                let _ = &owner;
            });
        }))
        .is_err()
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(pass.has_stop(), Err(Error::Closed));
}

#[test]
fn replaced_parallel_stop_cleanup_unwind_releases_new_unpublished_stop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut original = stop(1, &drops);
    original.panic = true;
    let mut pass = OrderedOutcomeSummary::new(Order::Parallel, 2, 0).unwrap();
    pass.observe_stop(original).unwrap();
    pass.observe_success(1, |_, _| panic!()).unwrap();
    let handed_off = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = pass.finish(|_, _| stop(2, &drops));
            handed_off.set(true);
        }))
        .is_err()
    );
    assert!(!handed_off.get());
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn finish_uncalled_factory_cleanup_unwind_releases_unpublished_summary_and_stop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut pass = OrderedOutcomeSummary::new(Order::Sequential, 1, 0).unwrap();
    pass.observe_stop(stop(1, &drops)).unwrap();
    let owners = Rc::new(Cell::new(0));
    let owner = Owner(owners.clone());
    let handed_off = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = pass.finish(move |_, _| {
                let _ = &owner;
                panic!("must not run");
            });
            handed_off.set(true);
        }))
        .is_err()
    );
    assert_eq!(owners.get(), 1);
    assert!(!handed_off.get());
    assert_eq!(*drops.borrow(), [1]);
}
