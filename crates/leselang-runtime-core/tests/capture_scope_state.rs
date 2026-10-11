use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::{
    CaptureScopeError as Error, CaptureScopeHooks, CaptureScopeSite as Site, CapturedBindings,
    Fuel, ProjectedBinding, ProjectedGroupBinding, ScalarBinding, ScalarValue, ScopeFrame,
    with_captured_scope, with_captured_scope_state,
};

type Capture = CapturedBindings<u8, u8, u8>;
fn capture() -> Capture {
    CapturedBindings {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "scalar".into(),
            value: ScalarValue::Integer(1),
        }],
        results: vec![ProjectedBinding {
            name: "result".into(),
            result: 2,
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: 3,
        }],
        body: 5,
    }
}

struct Slot {
    id: u8,
    drops: Rc<Cell<usize>>,
    poison: bool,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.poison, "trusted native slot destructor");
    }
}
struct Failure {
    id: u8,
    drops: Rc<Cell<usize>>,
}
impl Drop for Failure {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Context {
    fuel: Fuel,
    meter_address: usize,
    events: Vec<u8>,
    drops: Rc<Cell<usize>>,
    errors: Rc<Cell<usize>>,
    refuse: Option<u8>,
    unwind: Option<u8>,
    poison: Option<u8>,
}
fn context(fuel: u64) -> Context {
    Context {
        fuel: Fuel::new(fuel),
        meter_address: 0,
        events: vec![],
        drops: Rc::new(Cell::new(0)),
        errors: Rc::new(Cell::new(0)),
        refuse: None,
        unwind: None,
        poison: None,
    }
}
fn cost(id: u8) -> u64 {
    match id {
        1 => 2,
        2 => 3,
        3 => 5,
        4 => 7,
        5 => 11,
        _ => 1,
    }
}
impl Context {
    fn prepare(&mut self, id: u8) -> Result<Slot, Failure> {
        assert_eq!((&self.fuel as *const Fuel) as usize, self.meter_address);
        self.events.push(id);
        let error = || Failure {
            id,
            drops: self.errors.clone(),
        };
        self.fuel.charge(cost(id)).map_err(|_| error())?;
        assert_ne!(self.unwind, Some(id), "trusted native context hook");
        if self.refuse == Some(id) {
            return Err(error());
        }
        Ok(Slot {
            id,
            drops: self.drops.clone(),
            poison: self.poison == Some(id),
        })
    }
}

fn run<'a>(
    frame: &'a Capture,
    scope: &mut ScopeFrame<'_, 'a, Slot>,
    context: &mut Context,
) -> Result<Slot, Error<Failure>> {
    context.meter_address = (&context.fuel as *const Fuel) as usize;
    with_captured_scope_state(
        frame,
        scope,
        context,
        CaptureScopeHooks {
            scalar: |_: &ScalarValue, context: &mut Context| context.prepare(1),
            result: |value: &u8, context: &mut Context| context.prepare(*value),
            group: |value: &u8, context: &mut Context| context.prepare(*value),
            pending: |context: &mut Context| context.prepare(4),
            body: |value: &u8, scope: &mut ScopeFrame<'_, 'a, Slot>, context: &mut Context| {
                assert_eq!(scope.get("scalar").unwrap().id, 1);
                assert_eq!(scope.get("result").unwrap().id, 2);
                assert_eq!(scope.get("group").unwrap().id, 3);
                assert_eq!(scope.get("pending").unwrap().id, 4);
                context.prepare(*value)
            },
        },
    )
}
fn refusal<Value, Failure>(result: Result<Value, Failure>) -> Failure {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected refusal without Debug bounds"),
    }
}

#[test]
fn every_hook_and_body_share_original_move_only_gui_local_context_and_meter() {
    let frame = capture();
    let mut context = context(50);
    let mut bindings = vec![];
    let mut scope = ScopeFrame::new(&mut bindings);
    let output = run(&frame, &mut scope, &mut context).unwrap();
    assert_eq!(context.events, [1, 2, 3, 4, 5]);
    assert_eq!(context.fuel.remaining(), 22);
    assert_eq!(context.drops.get(), 4);
    assert_eq!(output.id, 5);
    assert_eq!(scope.len(), 0);
    drop(output);
    assert_eq!(context.drops.get(), 5);
}

#[test]
fn every_exhaustion_boundary_stops_before_later_hooks_without_refund_or_fresh_meter() {
    let frame = capture();
    for grant in 0..=29 {
        let mut context = context(grant);
        let mut bindings = vec![];
        let outcome = run(&frame, &mut ScopeFrame::new(&mut bindings), &mut context);
        let mut spent = 0;
        let mut called = 0;
        for id in 1..=5 {
            called += 1;
            if grant - spent < cost(id) {
                break;
            }
            spent += cost(id);
        }
        assert_eq!(context.fuel.remaining(), grant - spent, "grant {grant}");
        assert_eq!(context.events.len(), called);
        assert!(bindings.is_empty());
        if spent == 28 {
            assert!(outcome.is_ok());
        } else {
            let error = refusal(outcome);
            assert!(
                matches!(&error, Error::Native { error, .. } if usize::from(error.id) == called)
            );
            drop(error);
            assert_eq!(context.errors.get(), 1);
        }
    }
}

#[test]
fn duplicate_preparation_keeps_original_debit_and_native_refusal_precedence() {
    for (name, site, spent, events) in [
        ("scalar", Site::Result { index: 0 }, 5, 2),
        ("result", Site::Group { index: 0 }, 10, 3),
        ("pending", Site::Pending, 17, 4),
    ] {
        let mut frame = capture();
        match site {
            Site::Result { .. } => frame.results[0].name = name.into(),
            Site::Group { .. } => frame.groups[0].name = name.into(),
            _ => frame.name = "scalar".into(),
        }
        let mut context = context(50);
        let mut bindings = vec![];
        assert!(
            matches!(run(&frame, &mut ScopeFrame::new(&mut bindings), &mut context),
            Err(Error::Duplicate { site: actual }) if actual == site)
        );
        assert_eq!(context.fuel.remaining(), 50 - spent);
        assert_eq!(context.events.len(), events);
        assert!(bindings.is_empty());
    }
    let mut frame = capture();
    frame.results[0].name = "scalar".into();
    let mut context = context(50);
    context.refuse = Some(2);
    let mut bindings = vec![];
    assert!(matches!(
        run(&frame, &mut ScopeFrame::new(&mut bindings), &mut context),
        Err(Error::Native {
            site: Site::Result { index: 0 },
            ..
        })
    ));
    assert_eq!(context.fuel.remaining(), 45);
}

#[test]
fn native_refusal_and_unwind_preserve_context_mutation_and_restore_only_scope_suffix() {
    let frame = capture();
    for id in 1..=5 {
        for unwind in [false, true] {
            let mut context = context(50);
            if unwind {
                context.unwind = Some(id);
            } else {
                context.refuse = Some(id);
            }
            let parent = Slot {
                id: 0,
                drops: context.drops.clone(),
                poison: false,
            };
            let mut bindings = vec![("parent", parent)];
            let mut scope = ScopeFrame::new(&mut bindings);
            let outcome = catch_unwind(AssertUnwindSafe(|| run(&frame, &mut scope, &mut context)));
            if unwind {
                assert!(outcome.is_err());
            } else {
                let error = refusal(outcome.unwrap());
                assert!(
                    matches!(&error, Error::Native { error, .. } if error.id == id && Rc::ptr_eq(&error.drops, &context.errors))
                );
                drop(error);
                assert_eq!(context.errors.get(), 1);
            }
            assert_eq!(
                context.fuel.remaining(),
                50 - (1..=id).map(cost).sum::<u64>()
            );
            assert_eq!(context.events, (1..=id).collect::<Vec<_>>());
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.get("parent").unwrap().id, 0);
        }
    }
}

#[test]
fn nested_entry_reborrows_the_same_context_and_original_meter_without_aliasing_or_reset() {
    let outer = capture();
    let inner: CapturedBindings<(), (), ()> = CapturedBindings {
        name: "nested".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: (),
    };
    let mut context = context(50);
    context.meter_address = (&context.fuel as *const Fuel) as usize;
    let mut bindings = vec![];
    fn enter<'a>(
        outer: &'a Capture,
        inner: &'a CapturedBindings<(), (), ()>,
        bindings: &mut Vec<(&'a str, Slot)>,
        context: &mut Context,
    ) -> Result<Slot, Error<Failure>> {
        with_captured_scope_state(
            outer,
            &mut ScopeFrame::new(bindings),
            context,
            CaptureScopeHooks {
                scalar: |_: &ScalarValue, c: &mut Context| c.prepare(1),
                result: |_: &u8, c: &mut Context| c.prepare(2),
                group: |_: &u8, c: &mut Context| c.prepare(3),
                pending: |c: &mut Context| c.prepare(4),
                body: |_: &'a u8, scope: &mut ScopeFrame<'_, 'a, Slot>, c: &mut Context| {
                    drop(c.prepare(5)?);
                    let output = with_captured_scope_state(
                        inner,
                        scope,
                        c,
                        CaptureScopeHooks {
                            scalar: |_: &ScalarValue, _: &mut Context| unreachable!(),
                            result: |_: &(), _: &mut Context| unreachable!(),
                            group: |_: &(), _: &mut Context| unreachable!(),
                            pending: |c: &mut Context| c.prepare(6),
                            body: |_: &'a (),
                                   scope: &mut ScopeFrame<'_, 'a, Slot>,
                                   c: &mut Context| {
                                assert_eq!(scope.get("pending").unwrap().id, 4);
                                assert_eq!(scope.get("nested").unwrap().id, 6);
                                c.prepare(7)
                            },
                        },
                    )
                    .map_err(|e| match e {
                        Error::Native { error, .. } => error,
                        _ => unreachable!(),
                    })?;
                    assert!(scope.get("nested").is_none());
                    Ok(output)
                },
            },
        )
    }
    let output = enter(&outer, &inner, &mut bindings, &mut context).unwrap();
    assert_eq!(context.events, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(context.fuel.remaining(), 20);
    assert_eq!(output.id, 7);
    assert!(bindings.is_empty());
}

#[test]
fn unsized_context_uses_explicit_mutable_reborrows_without_default_or_owned_state() {
    let frame: CapturedBindings<(), (), ()> = CapturedBindings {
        name: "pending".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: (),
    };
    let mut context = [9_u64, 0];
    let mut bindings = vec![];
    let output = with_captured_scope_state(
        &frame,
        &mut ScopeFrame::new(&mut bindings),
        &mut context[..],
        CaptureScopeHooks {
            scalar: |_: &ScalarValue, _: &mut [u64]| Ok::<u64, ()>(0),
            result: |_: &(), _: &mut [u64]| Ok(0),
            group: |_: &(), _: &mut [u64]| Ok(0),
            pending: |c: &mut [u64]| {
                c[0] -= 2;
                c[1] += 1;
                Ok(c[0])
            },
            body: |_: &(), scope: &mut ScopeFrame<'_, '_, u64>, c: &mut [u64]| {
                assert_eq!(scope.get("pending"), Some(&7));
                c[0] -= 3;
                c[1] += 1;
                Ok(c[0])
            },
        },
    )
    .unwrap();
    assert_eq!(output, 4);
    assert_eq!(context, [4, 2]);
    assert!(bindings.is_empty());
}

#[test]
fn destructor_unwind_releases_output_or_error_and_does_not_refund_original_context() {
    let frame = capture();
    for refuse in [false, true] {
        let mut context = context(50);
        context.poison = Some(1);
        context.refuse = refuse.then_some(5);
        let mut bindings = vec![];
        assert!(
            catch_unwind(AssertUnwindSafe(|| run(
                &frame,
                &mut ScopeFrame::new(&mut bindings),
                &mut context
            )))
            .is_err()
        );
        assert_eq!(context.fuel.remaining(), 22);
        assert_eq!(context.events, [1, 2, 3, 4, 5]);
        assert_eq!(context.errors.get(), usize::from(refuse));
        assert_eq!(context.drops.get(), if refuse { 4 } else { 5 });
        assert!(bindings.is_empty());
    }
}

#[test]
fn uncalled_once_hook_destructor_unwind_releases_early_native_error_before_handoff() {
    let frame = capture();
    let mut context = context(50);
    context.refuse = Some(1);
    context.meter_address = (&context.fuel as *const Fuel) as usize;
    let poison = Slot {
        id: 99,
        drops: context.drops.clone(),
        poison: true,
    };
    let mut bindings = vec![];
    assert!(
        catch_unwind(AssertUnwindSafe(|| with_captured_scope_state(
            &frame,
            &mut ScopeFrame::new(&mut bindings),
            &mut context,
            CaptureScopeHooks {
                scalar: |_: &ScalarValue, c: &mut Context| c.prepare(1),
                result: |_: &u8, _: &mut Context| unreachable!(),
                group: |_: &u8, _: &mut Context| unreachable!(),
                pending: move |_: &mut Context| {
                    drop(poison);
                    unreachable!()
                },
                body: |_: &u8,
                       _: &mut ScopeFrame<'_, '_, Slot>,
                       _: &mut Context|
                 -> Result<(), Failure> { unreachable!() },
            },
        )))
        .is_err()
    );
    assert_eq!(context.events, [1]);
    assert_eq!(context.fuel.remaining(), 48);
    assert_eq!(context.errors.get(), 1);
    assert_eq!(context.drops.get(), 1);
    assert!(bindings.is_empty());
}

#[test]
fn remaining_mutable_hook_destructor_unwind_releases_output_or_error_after_scope_cleanup() {
    let frame = capture();
    for refuse in [false, true] {
        let mut context = context(50);
        context.refuse = refuse.then_some(5);
        context.meter_address = (&context.fuel as *const Fuel) as usize;
        let poison = Slot {
            id: 99,
            drops: context.drops.clone(),
            poison: true,
        };
        let mut bindings = vec![];
        assert!(
            catch_unwind(AssertUnwindSafe(|| with_captured_scope_state(
                &frame,
                &mut ScopeFrame::new(&mut bindings),
                &mut context,
                CaptureScopeHooks {
                    scalar: move |_: &ScalarValue, c: &mut Context| {
                        let _whole_capture = &poison;
                        assert_eq!(poison.id, 99);
                        c.prepare(1)
                    },
                    result: |v: &u8, c: &mut Context| c.prepare(*v),
                    group: |v: &u8, c: &mut Context| c.prepare(*v),
                    pending: |c: &mut Context| c.prepare(4),
                    body: |v: &u8, _: &mut ScopeFrame<'_, '_, Slot>, c: &mut Context| c.prepare(*v),
                },
            )))
            .is_err()
        );
        assert_eq!(context.events, [1, 2, 3, 4, 5]);
        assert_eq!(context.fuel.remaining(), 22);
        assert_eq!(context.errors.get(), usize::from(refuse));
        assert_eq!(context.drops.get(), if refuse { 5 } else { 6 });
        assert!(bindings.is_empty());
    }
}

#[test]
fn legacy_unit_state_wrapper_also_releases_output_and_error_before_original_hook_cleanup() {
    let frame = capture();
    for refuse in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let errors = Rc::new(Cell::new(0));
        let slot = |id| Slot {
            id,
            drops: drops.clone(),
            poison: false,
        };
        let poison = Slot {
            id: 99,
            drops: drops.clone(),
            poison: true,
        };
        let mut values = vec![];
        assert!(
            catch_unwind(AssertUnwindSafe(|| with_captured_scope(
                &frame,
                &mut ScopeFrame::new(&mut values),
                move |_: &ScalarValue| {
                    let _whole = &poison;
                    Ok(slot(1))
                },
                |id: &u8| Ok(slot(*id)),
                |id: &u8| Ok(slot(*id)),
                || Ok(slot(4)),
                |id: &u8, _: &mut ScopeFrame<'_, '_, Slot>| {
                    if refuse {
                        Err(Failure {
                            id: *id,
                            drops: errors.clone(),
                        })
                    } else {
                        Ok(slot(*id))
                    }
                },
            )))
            .is_err()
        );
        assert_eq!(errors.get(), usize::from(refuse));
        assert_eq!(drops.get(), if refuse { 5 } else { 6 });
        assert!(values.is_empty());
    }
}

#[test]
fn unrelated_borrowed_native_data_and_move_only_once_hooks_keep_context_and_output_ownership() {
    struct Native(String);
    let frame: CapturedBindings<Native, Native, Native> = CapturedBindings {
        name: "pending".into(),
        locals: vec![],
        groups: vec![],
        results: vec![ProjectedBinding {
            name: "saved".into(),
            result: Native("original".into()),
        }],
        body: Native("body".into()),
    };
    let mut context = context(50);
    fn enter<'a>(
        frame: &'a CapturedBindings<Native, Native, Native>,
        context: &mut Context,
    ) -> Result<&'a str, Error<()>> {
        let pending = Box::new(13_u8);
        let once_body = Box::new(17_u8);
        let mut bindings = vec![];
        let output = with_captured_scope_state(
            frame,
            &mut ScopeFrame::new(&mut bindings),
            context,
            CaptureScopeHooks {
                scalar: |_: &ScalarValue, _: &mut Context| Ok::<_, ()>("scalar"),
                result: |v: &'a Native, c: &mut Context| {
                    c.events.push(2);
                    Ok(v.0.as_str())
                },
                group: |_: &Native, _: &mut Context| Ok("group"),
                pending: move |c: &mut Context| {
                    c.events.push(*pending);
                    drop(pending);
                    Ok("pending")
                },
                body: move |v: &'a Native,
                            scope: &mut ScopeFrame<'_, 'a, &'a str>,
                            c: &mut Context| {
                    c.events.push(*once_body);
                    drop(once_body);
                    assert_eq!(scope.get("saved"), Some(&"original"));
                    Ok(v.0.as_str())
                },
            },
        );
        assert!(bindings.is_empty());
        output
    }
    let output = enter(&frame, &mut context).unwrap();
    assert_eq!(output.as_ptr(), frame.body.0.as_ptr());
    assert_eq!(context.events, [2, 13, 17]);
    assert_eq!(context.fuel.remaining(), 50);
}
