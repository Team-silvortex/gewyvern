use leselang_command::reference_group_start::ReferenceGroupStartHost;
use leselang_command::reference_start::{ReferenceStartHost, StartContext, start};
use leselang_command::{EffectRequest, ExecutionOrder, ResultBinding, Step, Value};
use leselang_hir::{Effect, HirBranch, HirProgram, Type, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, ScalarValue};
use leselang_syntax::parse;
use std::{cell::RefCell, rc::Rc};

struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail: Option<&'static str>,
    unwind: bool,
    next: u64,
    group_pointer: Option<*const HirBranch>,
}

fn refused() -> Step {
    Step::Fault(Fault {
        code: "NATIVE".into(),
        message: "original native refusal".into(),
    })
}

impl Host {
    fn observe(&mut self, name: &'static str) -> Result<(), Fault> {
        self.trace.borrow_mut().push(name);
        if self.fail == Some(name) {
            assert!(!self.unwind, "native hook unwound");
            let Step::Fault(error) = refused() else {
                unreachable!()
            };
            return Err(error);
        }
        Ok(())
    }
}
impl ReferenceGroupStartHost for Host {
    fn check_admission(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, 1);
        self.observe("admit")
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.observe("allocate")?;
        let next = self.next;
        self.next += 1;
        Ok(next)
    }
}
impl ReferenceStartHost for Host {
    fn start_group(
        &mut self,
        branches: &[HirBranch],
        order: ExecutionOrder,
        context: StartContext,
        binding: Option<Box<ResultBinding>>,
    ) -> Step {
        if let Err(error) = self.observe("group") {
            return Step::Fault(error);
        }
        if let Some(pointer) = self.group_pointer {
            assert_eq!(branches.as_ptr(), pointer);
        }
        assert_eq!(order, ExecutionOrder::Parallel);
        assert_eq!(context.expected_revision, Some(Revision(7)));
        assert_eq!(context.deadline_at_ms, Some(31_100));
        assert_eq!(context.fuel_remaining, 100);
        assert!(binding.is_none());
        // The real owner is deliberately absent: a route observation is not a commit.
        refused()
    }
    fn commit_atomic(&mut self, request: EffectRequest) -> Step {
        if let Err(error) = self.observe("commit") {
            return Step::Fault(error);
        }
        Step::Effect(Box::new(request))
    }
}

fn host() -> Host {
    Host {
        trace: Rc::default(),
        fail: None,
        unwind: false,
        next: 7,
        group_pointer: None,
    }
}
fn context() -> StartContext {
    StartContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_100),
        fuel_remaining: 100,
    }
}
fn program(expression: &str) -> HirProgram {
    lower(&parse(&format!("fn main() = {expression}"))).unwrap()
}

#[test]
fn root_start_pure_execution_and_competing_refusals_never_visit_native_hooks() {
    for defect in 0..4 {
        let mut host = host();
        let mut context = context();
        let mut program = program("choose(when: true, then: 7, otherwise: div(left: 1, right: 0))");
        if defect > 0 {
            context.principal.id.clear();
        }
        if defect > 1 {
            program
                .function
                .required_capabilities
                .push("unknown".into());
        }
        if defect > 2 {
            context.fuel_remaining = 0;
        }
        let result = start(&program, context, &mut host as &mut dyn ReferenceStartHost);
        if defect == 0 {
            assert_eq!(
                result,
                Step::Done(Value::Scalar {
                    value: ScalarValue::Integer(7)
                })
            );
        } else {
            let Step::Fault(error) = result else { panic!() };
            assert_eq!(
                error.code,
                match defect {
                    1 => "LSV1402",
                    2 => "LSH2002",
                    _ => "LSV1001",
                }
            );
        }
        assert!(host.trace.borrow().is_empty());
        assert_eq!(host.next, 7);
    }
}

#[test]
fn root_start_preserves_atomic_authority_ownership_and_borrowed_group_dispatch() {
    for expression in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id"))"#,
    ] {
        let mut host = host();
        let context = context();
        let pointer = context.principal.id.as_ptr();
        let program = program(expression);
        let before = serde_json::to_vec(&program).unwrap();
        let Step::Effect(request) = start(&program, context, &mut host) else {
            panic!()
        };
        assert_eq!(*host.trace.borrow(), ["admit", "allocate", "commit"]);
        assert_eq!(request.operation.authority().0.id.as_ptr(), pointer);
        assert_eq!(request.effect_id, "effect-7");
        assert_eq!(request.continuation.deadline_at_ms, Some(31_100));
        assert_eq!(request.budget, request.continuation.resource_budget());
        assert_eq!(
            request.continuation.result_binding.is_some(),
            expression.starts_with("bind(")
        );
        assert!(request.continuation.fuel_remaining < 100);
        assert_eq!(serde_json::to_vec(&program).unwrap(), before);
    }
    let program = program(r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#);
    let Effect::All { branches } = &program.function.effect else {
        panic!()
    };
    let mut host = host();
    host.group_pointer = Some(branches.as_ptr());
    assert_eq!(start(&program, context(), &mut host), refused());
    assert_eq!(*host.trace.borrow(), ["group"]);
    assert_eq!(host.next, 7);
}

#[test]
fn root_start_hook_refusal_and_unwind_preserve_once_only_order_and_consumed_ids() {
    for failure in ["admit", "allocate", "commit", "group"] {
        for unwind in [false, true] {
            let mut host = host();
            host.fail = Some(failure);
            host.unwind = unwind;
            let program = program(if failure == "group" {
                r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#
            } else {
                r#"ui.focus(node_id: "a")"#
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                start(&program, context(), &mut host)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), refused());
            }
            let expected: &[&str] = match failure {
                "admit" => &["admit"],
                "allocate" => &["admit", "allocate"],
                "commit" => &["admit", "allocate", "commit"],
                _ => &["group"],
            };
            assert_eq!(host.trace.borrow().as_slice(), expected);
            assert_eq!(host.next, if failure == "commit" { 8 } else { 7 });
        }
    }
}

#[test]
fn root_start_authorization_precedes_allocation_but_late_shape_failure_does_not_refund() {
    for computed in [false, true] {
        let mut program = program(if computed {
            r#"bind(r: ui.focus(node_id: "a"), body: 1)"#
        } else {
            r#"ui.focus(node_id: "a")"#
        });
        program.function.result_type = Type::Structured;
        let mut host = host();
        assert!(matches!(
            start(&program, context(), &mut host),
            Step::Fault(_)
        ));
        assert_eq!(host.next, if computed { 7 } else { 8 });
        let expected: &[&str] = if computed {
            &[]
        } else {
            &["admit", "allocate"]
        };
        assert_eq!(host.trace.borrow().as_slice(), expected);
    }
}
