use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use leselang_command::continuation_identity::{
    MAX_EFFECT_SEQUENCE, canonical_continuation_sequence, canonical_merge_sequence,
    valid_continuation_token,
};
use leselang_command::reference_group_batch::{
    AdmittedMergeGroup, ReferenceGroupBatchProofs, StoredMergeGroup, execution_order_label,
    inspect_group_batch, merge_reserved_sequence,
};
use leselang_command::reference_merge::{invalid_group_binding, validate_merge_plan};
use leselang_command::{
    ContinuationToken, ExecutionOrder, GroupResultBinding, MAX_CONTINUATION_BYTES, MergePlan,
    ResultBinding,
};
use leselang_hir::{Effect, HirBranch, Type, computation::Computation};
use leselang_runtime_core::{Fault, ScalarValue};

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

fn group_plan_fixture(parallel: bool, kind: usize) -> (GroupResultBinding, MergePlan) {
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let (body, order) = match (parallel, kind) {
        (p, 0) => (
            pure.clone(),
            if p {
                ExecutionOrder::BoundParallel
            } else {
                ExecutionOrder::BoundSequential
            },
        ),
        (p, 1) => (
            host.clone(),
            if p {
                ExecutionOrder::ParallelGroupTail
            } else {
                ExecutionOrder::GroupTail
            },
        ),
        (p, 2) => (
            capture.clone(),
            if p {
                ExecutionOrder::ParallelGroupCapture
            } else {
                ExecutionOrder::GroupCapture
            },
        ),
        (p, 3) => (
            Computation::Bind {
                name: "first".into(),
                value: Box::new(host),
                body: Box::new(capture.clone()),
            },
            if p {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            },
        ),
        (p, 4) => (
            Computation::Choose {
                when: Box::new(Computation::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                then: Box::new(capture),
                otherwise: Box::new(pure),
            },
            if p {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            },
        ),
        _ => unreachable!(),
    };
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            result_type: Type::UiFocus,
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
        })
        .collect();
    let binding = GroupResultBinding {
        pending: if parallel {
            Effect::All { branches }
        } else {
            Effect::Sequence { steps: branches }
        },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body,
        }),
        fuel_remaining: 100,
        successor_sequence: (kind != 0).then_some(100),
        additional_successor_sequences: if kind == 3 { vec![101] } else { vec![] },
    };
    let plan = MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: Some(Box::new(binding.clone())),
    };
    (binding, plan)
}

fn plans() -> Vec<MergePlan> {
    let mut plans = vec![
        MergePlan::new(["a", "b"]).unwrap(),
        MergePlan {
            branches: vec!["a".into(), "b".into()],
            order: ExecutionOrder::Sequential,
            result_binding: None,
        },
        MergePlan {
            branches: vec!["result".into(), "successor".into()],
            order: ExecutionOrder::ResultChain,
            result_binding: None,
        },
        MergePlan {
            branches: vec!["step_1".into(), "step_2".into()],
            order: ExecutionOrder::Dataflow,
            result_binding: None,
        },
    ];
    for parallel in [false, true] {
        for kind in 0..5 {
            plans.push(group_plan_fixture(parallel, kind).1);
        }
    }
    for plan in &plans {
        validate_merge_plan(plan).unwrap();
    }
    plans
}

#[derive(Default)]
struct Proofs {
    trace: Vec<String>,
    seen: Vec<(String, MergePlan, String, Option<Vec<u8>>)>,
    fail: Option<String>,
    collision: Option<String>,
}
impl Proofs {
    fn hit(&mut self, phase: &str, token: &str) -> Result<(), Fault> {
        let call = format!("{phase}:{token}");
        self.trace.push(call.clone());
        if self.fail.as_ref() == Some(&call) {
            Err(fault("HOST", &call))
        } else {
            Ok(())
        }
    }
    fn observe(
        &mut self,
        token: ContinuationToken,
        plan: MergePlan,
        state: String,
        terminal: Option<Vec<u8>>,
    ) -> Result<(), Fault> {
        self.hit("inspect", token.as_str())?;
        self.seen
            .push((token.as_str().into(), plan, state, terminal));
        Ok(())
    }
}
impl ReferenceGroupBatchProofs<Vec<u8>> for Proofs {
    fn token_conflicts(&mut self, token: &ContinuationToken) -> Result<bool, Fault> {
        self.hit("collision", token.as_str())?;
        Ok(self.collision.as_deref() == Some(token.as_str()))
    }
    fn inspect_group(&mut self, group: AdmittedMergeGroup) -> Result<(), Fault> {
        self.observe(group.token, group.plan, group.state, group.terminal)
    }
}

// Frozen product decisions with a local reservation set, not either batch runner.
fn frozen(groups: Vec<StoredMergeGroup>, proofs: &mut Proofs) -> Result<u64, Fault> {
    if groups.len() > 10_000 {
        return Err(fault("LSV4006", "journal group count exceeds limit"));
    }
    let mut reservations = BTreeSet::new();
    let mut highest = 0;
    for (token, bytes, state, terminal, order) in groups {
        let token = ContinuationToken::from_wire(token);
        if !valid_continuation_token(&token) {
            return Err(fault("LSV4007", "merge group token is invalid"));
        }
        if proofs.token_conflicts(&token)? {
            return Err(fault(
                "LSV4007",
                "merge group token conflicts with an effect token",
            ));
        }
        if bytes.len() > MAX_CONTINUATION_BYTES {
            return Err(fault(
                "LSV4006",
                &format!("journal entry exceeds {MAX_CONTINUATION_BYTES} bytes"),
            ));
        }
        let plan: MergePlan = serde_json::from_slice(&bytes)
            .map_err(|e| fault("LSV4007", &format!("invalid journal JSON: {e}")))?;
        validate_merge_plan(&plan).map_err(|_| fault("LSV4007", "merge group plan is invalid"))?;
        let owner = canonical_merge_sequence(&token)
            .or_else(|| canonical_continuation_sequence(&token))
            .unwrap_or(0);
        highest = highest.max(owner);
        if let Some(binding) = &plan.result_binding {
            for sequence in binding.reserved_sequences() {
                highest = highest.max(sequence);
                if sequence == 0
                    || sequence > MAX_EFFECT_SEQUENCE
                    || reservations.len() >= 640_000
                    || !reservations.insert(sequence)
                {
                    return Err(invalid_group_binding());
                }
            }
        }
        if frozen_label(plan.order) != order {
            return Err(fault(
                "LSV4007",
                "execution order conflicts with group plan",
            ));
        }
        proofs.observe(token, plan, state, terminal)?;
    }
    Ok(highest)
}

fn frozen_label(order: ExecutionOrder) -> &'static str {
    use ExecutionOrder::*;
    match order {
        Parallel
        | BoundParallel
        | ParallelGroupTail
        | ParallelGroupCapture
        | ParallelGroupDataflow
        | ParallelGroupConditional => "parallel",
        Sequential | BoundSequential | ResultChain | Dataflow | GroupTail | GroupCapture
        | GroupDataflow | GroupConditional => "sequential",
    }
}

fn rows(plan: &MergePlan) -> Vec<StoredMergeGroup> {
    ["merge-10", "continuation-22"]
        .into_iter()
        .enumerate()
        .map(|(index, token)| {
            let mut plan = plan.clone();
            if index == 1
                && let Some(binding) = &mut plan.result_binding
            {
                if let Some(sequence) = &mut binding.successor_sequence {
                    *sequence += 1000;
                }
                for sequence in &mut binding.additional_successor_sequences {
                    *sequence += 1000;
                }
            }
            (
                token.into(),
                serde_json::to_vec(&plan).unwrap(),
                "original-state".into(),
                Some(b"original-receipt".to_vec()),
                frozen_label(plan.order).into(),
            )
        })
        .collect()
}

#[test]
fn batch_policy_preserves_frozen_fault_trace_rows_and_maximum_across_all_orders() {
    let mut comparisons = 0;
    for plan in plans() {
        for position in 0..2 {
            for defect in 0..12 {
                for fail in [
                    None,
                    Some("collision:merge-10"),
                    Some("inspect:merge-10"),
                    Some("collision:continuation-22"),
                    Some("inspect:continuation-22"),
                ] {
                    let mut rows = rows(&plan);
                    let mut collision = None;
                    match defect {
                        0 => {}
                        1 => rows[position].0.clear(),
                        2 => rows[position].0 = "private token".into(),
                        3 => rows[position].1 = b"{".to_vec(),
                        4 => {
                            let mut p = plan.clone();
                            p.branches.clear();
                            rows[position].1 = serde_json::to_vec(&p).unwrap();
                        }
                        5 => rows[position].4 = "wrong".into(),
                        6 => rows[position].2 = "untrusted-state".into(),
                        7 => rows[position].3 = Some(b"{".to_vec()),
                        8 => {
                            // Both groups now reserve the same original successor.
                            rows[1].1 = rows[0].1.clone();
                            rows[position].4 = "wrong".into();
                        }
                        9 => {
                            let mut p = plan.clone();
                            if let Some(binding) = &mut p.result_binding {
                                binding.successor_sequence = Some(0);
                            }
                            rows[position].1 = serde_json::to_vec(&p).unwrap();
                        }
                        10 => collision = Some(rows[position].0.clone()),
                        11 => rows[position].1 = b"[]".to_vec(),
                        _ => unreachable!(),
                    }
                    let mut expected = Proofs {
                        fail: fail.map(str::to_string),
                        collision: collision.clone(),
                        ..Proofs::default()
                    };
                    let mut actual = Proofs {
                        fail: fail.map(str::to_string),
                        collision,
                        ..Proofs::default()
                    };
                    let result = frozen(rows.clone(), &mut expected);
                    assert_eq!(
                        inspect_group_batch(rows, &mut actual),
                        result,
                        "{:?}/{position}/{defect}/{fail:?}",
                        plan.order
                    );
                    assert_eq!(actual.trace, expected.trace);
                    assert_eq!(actual.seen, expected.seen);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 1_680);
}

#[test]
fn batch_limits_are_inclusive_and_size_decode_follows_original_collision_proof() {
    let plan = MergePlan::new(["a", "b"]).unwrap();
    let mut proofs = Proofs::default();
    assert_eq!(
        inspect_group_batch::<Vec<u8>, _>(vec![], &mut proofs),
        Ok(0)
    );
    for count in [10_000, 10_001] {
        proofs.trace.clear();
        proofs.seen.clear();
        let rows = (1..=count)
            .map(|i| {
                (
                    format!("merge-{i}"),
                    serde_json::to_vec(&plan).unwrap(),
                    "pending".into(),
                    None,
                    "parallel".into(),
                )
            })
            .collect();
        let actual = inspect_group_batch(rows, &mut proofs);
        if count == 10_000 {
            assert_eq!(actual, Ok(10_000));
            assert_eq!(proofs.seen.len(), count);
        } else {
            assert_eq!(actual.unwrap_err().code, "LSV4006");
            assert!(proofs.trace.is_empty());
        }
    }
    for extra in [0, 1] {
        let mut rows = rows(&plan);
        rows[0].1.resize(MAX_CONTINUATION_BYTES + extra, b' ');
        proofs = Proofs::default();
        let result = inspect_group_batch(rows.clone(), &mut proofs);
        if extra == 0 {
            assert_eq!(result, Ok(22));
        } else {
            assert_eq!(result.unwrap_err().code, "LSV4006");
            assert_eq!(proofs.trace, ["collision:merge-10"]);
        }
        proofs = Proofs {
            collision: Some("merge-10".into()),
            ..Proofs::default()
        };
        assert_eq!(
            inspect_group_batch(rows, &mut proofs).unwrap_err().message,
            "merge group token conflicts with an effect token"
        );
    }
}

#[test]
fn global_reservations_reject_duplicates_before_order_and_never_leak_to_the_next_pass() {
    let plan = group_plan_fixture(false, 3).1;
    let mut clean = rows(&plan);
    let mut duplicate = clean.clone();
    duplicate[1].1 = duplicate[0].1.clone();
    duplicate[1].4 = "wrong".into();
    let mut proofs = Proofs::default();
    assert_eq!(
        inspect_group_batch(duplicate, &mut proofs),
        Err(invalid_group_binding())
    );
    assert_eq!(
        proofs.trace,
        [
            "collision:merge-10",
            "inspect:merge-10",
            "collision:continuation-22"
        ]
    );
    proofs.trace.clear();
    clean.reverse();
    assert_eq!(inspect_group_batch(clean, &mut proofs), Ok(1101));
    assert_eq!(
        proofs.trace,
        [
            "collision:continuation-22",
            "inspect:continuation-22",
            "collision:merge-10",
            "inspect:merge-10"
        ]
    );
    for (token, expected) in [
        ("opaque-owner", 0),
        ("merge-001", 0),
        ("merge-0", 0),
        ("continuation-17", 17),
        ("merge-29", 29),
    ] {
        assert_eq!(
            merge_reserved_sequence(
                &ContinuationToken::from_wire(token.into()),
                &MergePlan::new(["a", "b"]).unwrap()
            ),
            expected
        );
    }
    for plan in plans() {
        assert_eq!(execution_order_label(plan.order), frozen_label(plan.order));
    }
}

struct TrackedBytes {
    id: String,
    bytes: Vec<u8>,
    trace: Rc<RefCell<Vec<String>>>,
    panic_read: bool,
}
impl AsRef<[u8]> for TrackedBytes {
    fn as_ref(&self) -> &[u8] {
        self.trace.borrow_mut().push(format!("read:{}", self.id));
        assert!(!self.panic_read, "native byte owner unwind");
        &self.bytes
    }
}
impl Drop for TrackedBytes {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(format!("drop:{}", self.id));
    }
}
struct LocalProofs {
    trace: Rc<RefCell<Vec<String>>>,
    panic_at: Option<String>,
    pointers: Vec<usize>,
}
impl LocalProofs {
    fn hit(&self, call: String) {
        self.trace.borrow_mut().push(call.clone());
        assert_ne!(self.panic_at.as_ref(), Some(&call), "native proof unwind");
    }
}
impl ReferenceGroupBatchProofs<TrackedBytes> for LocalProofs {
    fn token_conflicts(&mut self, token: &ContinuationToken) -> Result<bool, Fault> {
        self.hit(format!("collision:{}", token.as_str()));
        Ok(false)
    }
    fn inspect_group(&mut self, group: AdmittedMergeGroup<TrackedBytes>) -> Result<(), Fault> {
        self.hit(format!("inspect:{}", group.token.as_str()));
        let terminal = group.terminal.unwrap();
        assert!(self.pointers.contains(&(terminal.bytes.as_ptr() as usize)));
        assert_eq!(terminal.bytes, b"original-receipt");
        assert_eq!(group.state, "original-state");
        Ok(())
    }
}

#[test]
fn borrowed_local_proofs_and_move_only_bytes_preserve_ownership_and_unwind_without_retry() {
    let failures = [
        None,
        Some("collision:merge-10"),
        Some("inspect:merge-10"),
        Some("collision:continuation-22"),
        Some("inspect:continuation-22"),
        Some("read:plan-0"),
        Some("read:plan-1"),
    ];
    for failure in failures {
        let trace = Rc::new(RefCell::new(vec![]));
        let mut pointers = vec![];
        let mut tracked = vec![];
        for (i, (token, bytes, state, terminal, order)) in
            rows(&MergePlan::new(["a", "b"]).unwrap())
                .into_iter()
                .enumerate()
        {
            let terminal = terminal.unwrap();
            pointers.push(terminal.as_ptr() as usize);
            tracked.push((
                token,
                TrackedBytes {
                    id: format!("plan-{i}"),
                    bytes,
                    trace: trace.clone(),
                    panic_read: failure == Some(format!("read:plan-{i}").as_str()),
                },
                state,
                Some(TrackedBytes {
                    id: format!("terminal-{i}"),
                    bytes: terminal,
                    trace: trace.clone(),
                    panic_read: true,
                }),
                order,
            ));
        }
        let mut owner = LocalProofs {
            trace: trace.clone(),
            panic_at: failure.map(str::to_string),
            pointers,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inspect_group_batch(
                tracked,
                &mut owner as &mut dyn ReferenceGroupBatchProofs<TrackedBytes>,
            )
        }));
        if failure.is_none() {
            assert_eq!(result.unwrap(), Ok(22));
        } else {
            assert!(result.is_err());
        }
        let trace = trace.borrow();
        for id in ["plan-0", "terminal-0", "plan-1", "terminal-1"] {
            assert_eq!(
                trace
                    .iter()
                    .filter(|call| **call == format!("drop:{id}"))
                    .count(),
                1
            );
        }
        if let Some(failure) = failure {
            let position = trace.iter().position(|call| call == failure).unwrap();
            assert!(
                trace[position + 1..]
                    .iter()
                    .all(|call| call.starts_with("drop:"))
            );
        }
        assert!(!trace.iter().any(|call| call.starts_with("read:terminal")));
    }
}
