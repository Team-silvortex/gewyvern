use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationFault, EffectEvaluationLimits,
    },
    effect_reentry::{
        CapturedEffectHooks, CapturedEffectResult, ResumableEffectOutcome,
        resume_captured_effects_in_scope,
    },
    ir::Computation,
    pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits, PureValue},
};
use leselang_runtime_core::{
    BinaryOperator, CalculationFailure, CaptureScopeError, CaptureScopeSite, CapturedBindings,
    Fuel, ProjectedBinding, ProjectedGroupBinding, ScalarBinding, ScalarError, ScalarValue,
    ScopeFrame,
};

// Native schemas, saved slots, requests and captures intentionally have no derived traits.
struct Operation;
struct Opaque;
struct Row {
    number: u64,
    reads: Rc<Cell<usize>>,
}
struct Group(Row);
struct Error(String);
#[derive(Default)]
struct Drops {
    requests: Cell<usize>,
    captures: Cell<usize>,
}
struct Request(Rc<Drops>);
impl Drop for Request {
    fn drop(&mut self) {
        self.0.requests.set(self.0.requests.get() + 1);
    }
}
struct Capture(Rc<Drops>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.captures.set(self.0.captures.get() + 1);
    }
}
type Node = Computation<u8, Operation, Opaque, Opaque>;
type Frame = CapturedBindings<Node, Row, Group>;
type Output<'a> = CapturedEffectResult<'a, Node, u64, Request, Capture, Error>;
const LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 1,
    max_branches: 2,
};
#[derive(Default)]
struct Environment {
    log: RefCell<Vec<&'static str>>,
    denied: Cell<bool>,
    drops: Rc<Drops>,
}
impl PureEvaluationEnvironment<u8, Operation> for Environment {
    type Result = u64;
    type Error = Error;
    fn field(&self, value: &u64, _: &u8) -> Result<ScalarValue, Error> {
        self.log.borrow_mut().push("field");
        Ok(ScalarValue::Integer(*value))
    }
    fn member(&self, _: &u64, _: &str, _: &Operation) -> Result<u64, Error> {
        Err(Error("private member".into()))
    }
}
impl<'a> EffectEvaluationEnvironment<'a, u8, Operation, Opaque, Opaque> for Environment {
    type Request = Request;
    type Capture = Capture;
    fn preflight_effect(&self, _: &'a Node) -> Result<(), Error> {
        self.log.borrow_mut().push("cold");
        if self.denied.get() {
            Err(Error("private policy".into()))
        } else {
            Ok(())
        }
    }
    fn prepare_effect(
        &self,
        _: &'a Node,
        _: &mut ScopeFrame<'_, 'a, PureValue<u64>>,
        _: &mut Fuel,
    ) -> Result<Request, CalculationFailure<Error>> {
        self.log.borrow_mut().push("prepare");
        Ok(Request(self.drops.clone()))
    }
    fn capture(
        &self,
        _: &'a str,
        _: &'a Node,
        _: &ScopeFrame<'_, 'a, PureValue<u64>>,
        _: &mut Fuel,
    ) -> Result<Capture, Error> {
        self.log.borrow_mut().push("capture");
        Ok(Capture(self.drops.clone()))
    }
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn field(name: &str) -> Node {
    Node::Field {
        value: Box::new(local(name)),
        field: 1,
    }
}
fn add(left: Node, right: Node) -> Node {
    Node::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn call() -> Node {
    Node::Call {
        operation: Operation,
        arguments: vec![],
    }
}
fn row(number: u64) -> Row {
    Row {
        number,
        reads: Rc::new(Cell::new(0)),
    }
}
fn frame(body: Node) -> Frame {
    Frame {
        name: "reply".into(),
        locals: vec![ScalarBinding {
            name: "n".into(),
            value: ScalarValue::Integer(1),
        }],
        results: vec![ProjectedBinding {
            name: "r".into(),
            result: row(2),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "g".into(),
            group: Group(row(3)),
        }],
        body,
    }
}
fn debit(fuel: &mut Fuel) -> Result<(), CalculationFailure<Error>> {
    fuel.charge(1)
        .map_err(|_| Error("private restore fuel".into()).into())
}
fn run<'a>(
    frame: &'a Frame,
    env: &Environment,
    prefix: &mut Vec<(&'a str, PureValue<u64>)>,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
    fail_group: bool,
) -> Output<'a> {
    resume_captured_effects_in_scope(
        frame,
        &mut ScopeFrame::new(prefix),
        env,
        fuel,
        limits,
        CapturedEffectHooks {
            scalar: |value: &ScalarValue, fuel: &mut Fuel| {
                env.log.borrow_mut().push("scalar");
                debit(fuel)?;
                Ok(PureValue::Scalar(value.clone()))
            },
            result: |value: &Row, fuel: &mut Fuel| {
                env.log.borrow_mut().push("result");
                debit(fuel)?;
                value.reads.set(value.reads.get() + 1);
                Ok(PureValue::Result(value.number))
            },
            group: |value: &Group, fuel: &mut Fuel| {
                env.log.borrow_mut().push("group");
                debit(fuel)?;
                if fail_group {
                    return Err(Error("private group".into()).into());
                }
                value.0.reads.set(value.0.reads.get() + 1);
                Ok(PureValue::Result(value.0.number))
            },
            pending: |fuel: &mut Fuel| {
                env.log.borrow_mut().push("pending");
                debit(fuel)?;
                Ok(PureValue::Result(4))
            },
        },
    )
}

#[test]
fn mixed_move_only_saved_schema_enters_actual_body_in_wire_order_with_original_meter() {
    let frame = frame(add(
        add(local("n"), field("r")),
        add(field("g"), add(field("reply"), field("parent"))),
    ));
    let env = Environment::default();
    let mut prefix = vec![("parent", PureValue::Result(77))];
    let mut fuel = Fuel::new(100);
    let outcome = run(&frame, &env, &mut prefix, &mut fuel, LIMITS, false).unwrap();
    assert!(matches!(
        outcome,
        ResumableEffectOutcome::Value(PureValue::Scalar(ScalarValue::Integer(87)))
    ));
    assert_eq!(
        &env.log.borrow()[..4],
        ["scalar", "result", "group", "pending"]
    );
    assert_eq!(
        env.log.borrow().iter().filter(|x| **x == "field").count(),
        4
    );
    assert_eq!(frame.results[0].result.reads.get(), 1);
    assert_eq!(frame.groups[0].group.0.reads.get(), 1);
    assert!(fuel.remaining() < 96);
    assert_eq!(prefix.len(), 1);
    assert!(matches!(prefix[0].1, PureValue::Result(77)));
}

#[test]
fn duplicate_checks_follow_native_restore_debits_and_stop_later_hooks() {
    let frame = frame(call());
    let env = Environment::default();
    let mut prefix = vec![("n", PureValue::Result(77))];
    let mut fuel = Fuel::new(100);
    let error = run(&frame, &env, &mut prefix, &mut fuel, LIMITS, false).unwrap_err();
    assert!(matches!(
        error,
        CaptureScopeError::Duplicate {
            site: CaptureScopeSite::Scalar { index: 0 }
        }
    ));
    assert_eq!(*env.log.borrow(), ["scalar"]);
    assert_eq!(fuel.remaining(), 99);
    assert_eq!(prefix.len(), 1);
    assert_eq!(frame.results[0].result.reads.get(), 0);
}

#[test]
fn native_restore_error_moves_unchanged_and_never_formats_private_payloads() {
    let frame = frame(call());
    let env = Environment::default();
    let mut prefix = vec![("parent", PureValue::Result(77))];
    let mut fuel = Fuel::new(100);
    let error = run(&frame, &env, &mut prefix, &mut fuel, LIMITS, true).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("private group"));
    assert!(matches!(error, CaptureScopeError::Native {
        site: CaptureScopeSite::Group { index: 0 },
        error: CalculationFailure::External(EffectEvaluationFault::Native(Error(value)))
    } if value == "private group"));
    assert_eq!(*env.log.borrow(), ["scalar", "result", "group"]);
    assert_eq!(fuel.remaining(), 97);
    assert_eq!(prefix.len(), 1);
}

#[test]
fn cold_denial_preserves_restore_order_but_never_prepares_selected_or_cold_effects() {
    let frame = frame(Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(Node::Literal {
            value: ScalarValue::Integer(9),
        }),
        otherwise: Box::new(call()),
    });
    let env = Environment::default();
    env.denied.set(true);
    let mut prefix = vec![];
    let mut fuel = Fuel::new(100);
    let error = run(&frame, &env, &mut prefix, &mut fuel, LIMITS, false).unwrap_err();
    assert!(matches!(error, CaptureScopeError::Native {
        site: CaptureScopeSite::Body,
        error: CalculationFailure::External(EffectEvaluationFault::Native(Error(value)))
    } if value == "private policy"));
    assert_eq!(
        *env.log.borrow(),
        ["scalar", "result", "group", "pending", "cold"]
    );
    assert_eq!(fuel.remaining(), 96);
    assert!(prefix.is_empty());
}

#[test]
fn physical_body_refusal_never_reaches_cold_native_policy_or_execution() {
    let frame = frame(call());
    let env = Environment::default();
    let mut prefix = vec![];
    let mut fuel = Fuel::new(100);
    let mut limits = LIMITS;
    limits.pure.max_nodes = 0;
    let error = run(&frame, &env, &mut prefix, &mut fuel, limits, false).unwrap_err();
    assert!(matches!(
        error,
        CaptureScopeError::Native {
            site: CaptureScopeSite::Body,
            ..
        }
    ));
    assert_eq!(*env.log.borrow(), ["scalar", "result", "group", "pending"]);
    assert_eq!(fuel.remaining(), 96);
    assert!(prefix.is_empty());
}

#[test]
fn successor_retains_exact_borrowed_saved_body_and_move_only_native_outputs() {
    let frame = frame(Node::Bind {
        name: "next".into(),
        value: Box::new(call()),
        body: Box::new(local("next")),
    });
    let env = Environment::default();
    let mut prefix = vec![];
    let mut fuel = Fuel::new(100);
    let outcome = run(&frame, &env, &mut prefix, &mut fuel, LIMITS, false).unwrap();
    let ResumableEffectOutcome::Suspended {
        request,
        continuation,
    } = outcome
    else {
        panic!()
    };
    let Node::Bind { body, .. } = &frame.body else {
        panic!()
    };
    assert_eq!(continuation.name(), "next");
    assert!(std::ptr::eq(continuation.body(), body.as_ref()));
    assert_eq!(
        *env.log.borrow(),
        [
            "scalar", "result", "group", "pending", "cold", "prepare", "capture"
        ]
    );
    assert_eq!(fuel.remaining(), 94);
    assert!(prefix.is_empty());
    assert_eq!(env.drops.requests.get(), 0);
    assert_eq!(env.drops.captures.get(), 0);
    drop((request, continuation));
    assert_eq!(env.drops.requests.get(), 1);
    assert_eq!(env.drops.captures.get(), 1);
}

#[test]
fn hook_scalar_failure_stays_typed_but_native_lookalike_is_never_recoverable() {
    for native in [false, true] {
        let frame: Frame = CapturedBindings {
            name: "reply".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body: local("reply"),
        };
        let env = Environment::default();
        let mut prefix = vec![];
        let mut fuel = Fuel::new(100);
        let error = resume_captured_effects_in_scope(
            &frame,
            &mut ScopeFrame::new(&mut prefix),
            &env,
            &mut fuel,
            LIMITS,
            CapturedEffectHooks {
                scalar: |_: &ScalarValue, _: &mut Fuel| Ok(PureValue::Scalar(ScalarValue::None)),
                result: |_: &Row, _: &mut Fuel| Ok(PureValue::Result(0)),
                group: |_: &Group, _: &mut Fuel| Ok(PureValue::Result(0)),
                pending: |_: &mut Fuel| {
                    if native {
                        Err(Error("LSV1401 private".into()).into())
                    } else {
                        Err(CalculationFailure::Scalar(ScalarError::IntegerArithmetic))
                    }
                },
            },
        )
        .unwrap_err();
        let CaptureScopeError::Native {
            site: CaptureScopeSite::Pending,
            error,
        } = error
        else {
            panic!()
        };
        assert_eq!(error.is_recoverable(), !native);
        assert_eq!(fuel.remaining(), 100);
        assert!(prefix.is_empty());
    }
}

struct PanicOnDrop;
impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("native cleanup");
    }
}
#[test]
fn unused_hook_cleanup_unwind_releases_unpublished_successor_without_retry_or_refund() {
    let frame: Frame = CapturedBindings {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: Node::Bind {
            name: "next".into(),
            value: Box::new(call()),
            body: Box::new(local("next")),
        },
    };
    let env = Environment::default();
    let mut prefix = vec![("parent", PureValue::Result(77))];
    let mut fuel = Fuel::new(100);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let guard = PanicOnDrop;
        let _ = resume_captured_effects_in_scope(
            &frame,
            &mut ScopeFrame::new(&mut prefix),
            &env,
            &mut fuel,
            LIMITS,
            CapturedEffectHooks {
                scalar: move |_: &ScalarValue, _: &mut Fuel| {
                    let _ = &guard;
                    Ok(PureValue::Scalar(ScalarValue::None))
                },
                result: |_: &Row, _: &mut Fuel| Ok(PureValue::Result(0)),
                group: |_: &Group, _: &mut Fuel| Ok(PureValue::Result(0)),
                pending: |_: &mut Fuel| Ok(PureValue::Result(4)),
            },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(*env.log.borrow(), ["cold", "prepare", "capture"]);
    assert_eq!(fuel.remaining(), 98);
    assert_eq!(prefix.len(), 1);
    assert_eq!(env.drops.requests.get(), 1);
    assert_eq!(env.drops.captures.get(), 1);
}
