use leselang_command::reference_group_start::{
    GroupStartContext, ReferenceGroupStartHost, prepare_group_start,
};
use leselang_command::{ExecutionOrder, ResultBinding};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, HirBranch, Type};
use std::{cell::RefCell, rc::Rc};

#[path = "support/group_start_fixture.rs"]
mod fixture;

struct Host {
    trace: Rc<RefCell<Vec<String>>>,
    fail_at: Option<usize>,
    unwind: bool,
    next: u64,
}

impl Host {
    fn observe(&mut self, event: String) -> Result<(), Fault> {
        self.trace.borrow_mut().push(event);
        if self.fail_at == Some(self.trace.borrow().len() - 1) {
            assert!(!self.unwind, "original reservation owner unwound");
            return Err(Fault {
                code: "NATIVE".into(),
                message: "original refusal".into(),
            });
        }
        Ok(())
    }
}

impl ReferenceGroupStartHost for Host {
    fn check_admission(&mut self, additional: usize) -> Result<(), Fault> {
        self.observe(format!("admit:{additional}"))
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.observe(format!("allocate:{}", self.next))?;
        let next = self.next;
        self.next += 1;
        Ok(next)
    }
}

fn branches() -> Vec<HirBranch> {
    ["a", "b"]
        .map(|name| HirBranch {
            name: name.into(),
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
            result_type: Type::UiFocus,
        })
        .into()
}

fn context() -> GroupStartContext {
    GroupStartContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_100),
        fuel_remaining: 100,
    }
}

fn host() -> Host {
    Host {
        trace: Rc::default(),
        fail_at: None,
        unwind: false,
        next: 10,
    }
}

#[test]
fn group_start_preserves_twelve_shapes_ordered_reservations_and_original_binding_owner() {
    let mut orders = std::collections::BTreeSet::new();
    for sequential in [false, true] {
        for kind in 0..6 {
            let mut host = host();
            let branches = branches();
            let before = serde_json::to_vec(&branches).unwrap();
            let saved = fixture::binding(kind);
            let pointer = saved.as_deref().map(|b| b as *const ResultBinding);
            let result = prepare_group_start(
                &branches,
                if sequential {
                    ExecutionOrder::Sequential
                } else {
                    ExecutionOrder::Parallel
                },
                context(),
                saved,
                &mut host as &mut dyn ReferenceGroupStartHost,
            )
            .unwrap();
            assert_eq!(result.merge_token.as_str(), "merge-10");
            assert_eq!(result.branches.len(), 2);
            let expected_reservations = match kind {
                0 | 1 => 3,
                4 => 5,
                _ => 4,
            };
            assert_eq!(host.next, 10 + expected_reservations);
            let mut trace = vec!["admit:2".to_string()];
            trace.extend((10..host.next).map(|n| format!("allocate:{n}")));
            assert_eq!(*host.trace.borrow(), trace);
            for (i, branch) in result.branches.iter().enumerate() {
                assert_eq!(branch.request.effect_id, format!("effect-{}", 11 + i));
                let fuel = 99 - if sequential { i as u64 } else { 0 };
                assert_eq!(branch.request.continuation.fuel_remaining, fuel);
                assert_eq!(branch.request.budget.fuel_remaining, fuel);
                assert_eq!(branch.request.continuation.deadline_at_ms, Some(31_100));
                assert_eq!(branch.request.continuation.group_result.is_some(), kind > 0);
            }
            assert_eq!(
                result
                    .plan
                    .result_binding
                    .as_ref()
                    .map(|b| &*b.binding as *const ResultBinding),
                pointer
            );
            if let Some(group) = &result.plan.result_binding {
                assert_eq!(group.fuel_remaining, 98);
                assert_eq!(
                    group.reserved_sequences().collect::<Vec<_>>(),
                    (13..host.next).collect::<Vec<_>>()
                );
            }
            assert_eq!(serde_json::to_vec(&branches).unwrap(), before);
            orders.insert(serde_json::to_string(&result.plan.order).unwrap());
        }
    }
    assert_eq!(orders.len(), 12);
}

#[test]
fn group_start_native_refusal_or_unwind_stops_at_every_original_hook_without_refund() {
    for fail_at in 0..6 {
        for unwind in [false, true] {
            let mut host = host();
            host.fail_at = Some(fail_at);
            host.unwind = unwind;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepare_group_start(
                    &branches(),
                    ExecutionOrder::Parallel,
                    context(),
                    fixture::binding(4),
                    &mut host,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap().unwrap_err(),
                    Fault {
                        code: "NATIVE".into(),
                        message: "original refusal".into()
                    }
                );
            }
            assert_eq!(host.trace.borrow().len(), fail_at + 1);
            assert_eq!(host.next, 10 + fail_at.saturating_sub(1) as u64);
        }
    }
}

#[test]
fn group_start_preflight_and_failed_build_never_publish_or_repair_partial_work() {
    for defect in 0..7 {
        let mut host = host();
        let mut branches = branches();
        let mut context = context();
        let mut saved = fixture::binding(4);
        match defect {
            0 => branches[0].effect = Effect::All { branches: vec![] },
            1 => context.fuel_remaining = 1,
            2 => branches[1].name = "a".into(),
            3 => saved.as_mut().unwrap().name = "bad name".into(),
            4 => branches[1].result_type = Type::Structured,
            5 => context.principal.id.clear(),
            6 => {
                saved = None;
                branches[1].result_type = Type::Structured;
            }
            _ => unreachable!(),
        }
        let before = serde_json::to_vec(&branches).unwrap();
        assert!(
            prepare_group_start(
                &branches,
                ExecutionOrder::Parallel,
                context,
                saved,
                &mut host
            )
            .is_err()
        );
        let calls = match defect {
            0..=4 => 0,
            5 => 3,
            6 => 4,
            _ => unreachable!(),
        };
        assert_eq!(host.trace.borrow().len(), calls, "defect={defect}");
        assert_eq!(
            host.next,
            match defect {
                5 => 12,
                6 => 13,
                _ => 10,
            }
        );
        assert_eq!(serde_json::to_vec(&branches).unwrap(), before);
    }
}
