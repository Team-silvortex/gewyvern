use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{Fuel, ScopeFrame, with_owned_scope_state};

struct Native {
    id: usize,
    poison: bool,
    drops: Rc<RefCell<Vec<usize>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.poison, "native cleanup unwind");
    }
}
fn native(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    Native {
        id,
        poison: false,
        drops: drops.clone(),
    }
}
struct Context {
    fuel: Fuel,
    original: *const Fuel,
    mutations: Rc<Cell<usize>>,
}
impl Context {
    fn charge(&mut self) {
        assert_eq!(self.original, &raw const self.fuel);
        self.mutations.set(self.mutations.get() + 1);
        assert!(self.fuel.charge(1).is_ok());
    }
}

#[test]
fn owned_scope_moves_gui_local_values_and_cleans_suffix_then_prefix_before_output() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut context = Context {
        fuel: Fuel::new(2),
        original: std::ptr::null(),
        mutations: Rc::new(Cell::new(0)),
    };
    context.original = &raw const context.fuel;
    let output = with_owned_scope_state(
        vec![("prefix", native(1, &drops))],
        &mut context,
        |scope, state| {
            assert_eq!(scope.get("prefix").unwrap().id, 1);
            assert_eq!(scope.local_len(), 0);
            state.charge();
            assert!(scope.push("local", native(2, &drops)).is_ok());
            state.charge();
            Ok::<_, Native>(native(3, &drops))
        },
    )
    .unwrap_or_else(|_| panic!("unexpected native error"));
    assert_eq!(*drops.borrow(), [2, 1]);
    assert_eq!(context.fuel.remaining(), 0);
    assert_eq!(context.mutations.get(), 2);
    drop(output);
    assert_eq!(*drops.borrow(), [2, 1, 3]);
}

#[test]
fn ordinary_refusal_returns_original_opaque_error_after_all_owned_scope_cleanup() {
    let drops = Rc::new(RefCell::new(vec![]));
    let output = with_owned_scope_state(
        vec![("prefix", native(1, &drops))],
        &mut (),
        |scope, _: &mut ()| {
            assert!(scope.push("local", native(2, &drops)).is_ok());
            Err::<Native, _>(native(4, &drops))
        },
    );
    assert_eq!(*drops.borrow(), [2, 1]);
    let Err(error) = output else {
        panic!("native error expected")
    };
    assert_eq!(error.id, 4);
    drop(error);
    assert_eq!(*drops.borrow(), [2, 1, 4]);
}

#[test]
fn suffix_or_prefix_destructor_unwind_releases_already_owned_output_or_error() {
    for poison in [1, 2] {
        for success in [false, true] {
            let drops = Rc::new(RefCell::new(vec![]));
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_owned_scope_state(
                    vec![(
                        "prefix",
                        Native {
                            id: 1,
                            poison: poison == 1,
                            drops: drops.clone(),
                        },
                    )],
                    &mut (),
                    |scope, _: &mut ()| {
                        assert!(
                            scope
                                .push(
                                    "local",
                                    Native {
                                        id: 2,
                                        poison: poison == 2,
                                        drops: drops.clone()
                                    }
                                )
                                .is_ok()
                        );
                        if success {
                            Ok(native(3, &drops))
                        } else {
                            Err(native(4, &drops))
                        }
                    },
                )
            }));
            assert!(result.is_err());
            for id in [1, 2, if success { 3 } else { 4 }] {
                assert!(drops.borrow().contains(&id), "missing {id} poison {poison}");
            }
            assert_eq!(drops.borrow().len(), 3);
        }
    }
}

#[test]
fn body_unwind_consumes_the_entire_prefix_without_refund_or_context_rollback() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut context = Context {
        fuel: Fuel::new(2),
        original: std::ptr::null(),
        mutations: Rc::new(Cell::new(0)),
    };
    context.original = &raw const context.fuel;
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_owned_scope_state(
            vec![("prefix", native(1, &drops))],
            &mut context,
            |scope, state| -> Result<(), Native> {
                state.charge();
                assert!(scope.push("local", native(2, &drops)).is_ok());
                panic!("native body unwind");
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(*drops.borrow(), [2, 1]);
    assert_eq!(context.fuel.remaining(), 1);
    assert_eq!(context.mutations.get(), 1);
}

#[test]
fn unsized_original_context_is_reborrowed_without_default_clone_or_wrapping() {
    let mut state = [2_u64, 0];
    let answer = with_owned_scope_state(
        vec![("prefix", 7_u64)],
        &mut state[..],
        |scope, state: &mut [u64]| {
            state[0] -= 1;
            state[1] += 1;
            Ok::<_, ()>(*scope.get("prefix").unwrap())
        },
    )
    .unwrap();
    assert_eq!(answer, 7);
    assert_eq!(state, [1, 1]);
}

#[test]
fn nested_owned_entry_reuses_original_meter_and_does_not_consume_outer_values_early() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut fuel = Fuel::new(2);
    let output = with_owned_scope_state(
        vec![("outer", native(1, &drops))],
        &mut fuel,
        |scope, fuel| {
            let inner =
                with_owned_scope_state(vec![("inner", native(2, &drops))], fuel, |inner, fuel| {
                    assert_eq!(inner.get("inner").unwrap().id, 2);
                    fuel.charge(1)?;
                    Ok::<_, leselang_runtime_core::FuelExhausted>(native(3, &drops))
                })?;
            assert_eq!(*drops.borrow(), [2]);
            assert_eq!(scope.get("outer").unwrap().id, 1);
            fuel.charge(1)?;
            Ok::<_, leselang_runtime_core::FuelExhausted>(inner)
        },
    )
    .unwrap();
    assert_eq!(*drops.borrow(), [2, 1]);
    assert_eq!(fuel.remaining(), 0);
    drop(output);
    assert_eq!(*drops.borrow(), [2, 1, 3]);
}

#[test]
fn unrelated_stable_native_borrows_can_outlive_the_consumed_scope() {
    struct View<'a>(&'a str, Rc<()>);
    let payload = String::from("stable native schema");
    let name = String::from("original name");
    fn run<'a>(name: &str, payload: &'a str) -> &'a str {
        with_owned_scope_state(
            vec![(name, View(payload, Rc::new(())))],
            &mut (),
            |scope: &mut ScopeFrame<'_, '_, View<'a>>, _: &mut ()| {
                let view = scope.get(name).unwrap();
                assert_eq!(Rc::strong_count(&view.1), 1);
                Ok::<_, ()>(view.0)
            },
        )
        .unwrap()
    }
    assert_eq!(run(&name, &payload).as_ptr(), payload.as_ptr());
}

#[test]
fn owned_prefix_is_not_implicit_duplicate_name_type_or_authority_admission() {
    let output = with_owned_scope_state(
        vec![("not an identifier", 1), ("not an identifier", 2)],
        &mut (),
        |scope, _: &mut ()| {
            assert_eq!(scope.len(), 2);
            assert_eq!(scope.get("not an identifier"), Some(&1));
            assert!(scope.push("not an identifier", 3).is_err());
            Ok::<_, ()>(())
        },
    );
    assert!(output.is_ok());
}
