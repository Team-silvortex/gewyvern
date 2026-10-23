use leselang_command::reference_completion::{sequence_stop, validate_terminal_step};
use leselang_command::reference_group_recovery::{
    RecoveredMergeBranch, ReferenceGroupProofs, inspect_group_record, inspect_pending_sequence,
};
use leselang_command::{ContinuationToken, ExecutionOrder, MergePlan, MergeWait, Step, Value};
use leselang_runtime_core::Fault;

fn fault(message: impl Into<String>) -> Fault {
    Fault {
        code: "LSV4007".into(),
        message: message.into(),
    }
}
fn terminal() -> Step {
    Step::Done(Value::UiFocus {
        node_id: "original".into(),
    })
}
fn plan(order: ExecutionOrder) -> MergePlan {
    MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: None,
    }
}
fn rows() -> Vec<RecoveredMergeBranch> {
    ["a", "b"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            (
                name.into(),
                index as i64,
                "pending".into(),
                None,
                format!("continuation-{}", index + 1),
            )
        })
        .collect()
}

#[derive(Default)]
struct Proofs {
    trace: Vec<String>,
    fail: Option<&'static str>,
    panic_at: Option<&'static str>,
    receipt_ptr: usize,
    local: std::rc::Rc<()>,
}
impl Proofs {
    fn hit(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.push(stage.into());
        assert_ne!(self.panic_at, Some(stage), "native proof unwind");
        if self.fail == Some(stage) {
            Err(Fault {
                code: "NATIVE".into(),
                message: stage.into(),
            })
        } else {
            Ok(())
        }
    }
}
impl ReferenceGroupProofs for Proofs {
    fn validate_chain(&mut self) -> Result<(), Fault> {
        self.hit("chain")
    }
    fn validate_blocked_dispatches(&mut self, first_pending: usize) -> Result<(), Fault> {
        self.hit("blocked")?;
        self.trace.push(format!("frontier:{first_pending}"));
        Ok(())
    }
    fn validate_unbound_receipt(&mut self, terminal: &Step) -> Result<(), Fault> {
        self.hit("receipt")?;
        if let Step::Done(Value::UiFocus { node_id }) = terminal {
            self.receipt_ptr = node_id.as_ptr() as usize;
        }
        Ok(())
    }
    fn validate_records(&mut self, terminal: Option<&Step>) -> Result<(), Fault> {
        self.hit("records")?;
        if let Some(Step::Done(Value::UiFocus { node_id })) = terminal {
            assert_eq!(self.receipt_ptr, node_id.as_ptr() as usize);
        }
        self.trace.push(format!("terminal:{}", terminal.is_some()));
        Ok(())
    }
}

// Frozen decisions and codec never call the moved adapter or shared phase runners.
fn decode(bytes: &[u8]) -> Result<Step, Fault> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(Fault {
            code: "LSV4006".into(),
            message: "journal entry exceeds 8388608 bytes".into(),
        });
    }
    serde_json::from_slice(bytes).map_err(|e| fault(format!("invalid journal JSON: {e}")))
}
fn frozen_frontier(branches: &[RecoveredMergeBranch], proofs: &mut Proofs) -> Result<(), Fault> {
    if branches.len() > 64 {
        return Err(fault("merge branches do not match their declared plan"));
    }
    let first = branches
        .iter()
        .position(|b| b.2 == "pending")
        .unwrap_or(branches.len());
    if branches[first..].iter().any(|b| b.2 != "pending") {
        return Err(fault("sequence completion is out of order"));
    }
    let steps = branches[..first]
        .iter()
        .map(|b| {
            decode(
                b.3.as_deref()
                    .ok_or_else(|| fault("completed sequence step has no result"))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sequence_stop(steps.iter())?.is_some() {
        return Err(fault("failed sequence still has pending successors"));
    }
    proofs.validate_blocked_dispatches(first)
}
fn frozen(
    proofs: &mut Proofs,
    owner: &ContinuationToken,
    plan: &MergePlan,
    rows: &[RecoveredMergeBranch],
    state: &str,
    encoded: Option<&[u8]>,
) -> Result<(), Fault> {
    let mismatch = || fault("merge branches do not match their declared plan");
    if rows.len() != plan.branches.len() || rows.len() > 64 {
        return Err(mismatch());
    }
    let mut ids = std::collections::HashSet::new();
    for (index, row) in rows.iter().enumerate() {
        if usize::try_from(row.1).ok() != Some(index)
            || row.0 != plan.branches[index]
            || row.4 == owner.as_str()
            || !ids.insert(&row.4)
        {
            return Err(mismatch());
        }
    }
    let completed = rows.iter().all(|row| row.2 == "completed");
    if matches!(
        plan.order,
        ExecutionOrder::ResultChain | ExecutionOrder::Dataflow
    ) {
        proofs.validate_chain()?;
    }
    if plan.order.is_sequential() && state == "pending" {
        frozen_frontier(rows, proofs)?;
    }
    let receipt = match (state, completed, encoded) {
        ("pending", false, None) => None,
        ("completed", true, Some(raw)) => {
            let step = decode(raw)?;
            validate_terminal_step(&step)?;
            proofs.validate_unbound_receipt(&step)?;
            Some(step)
        }
        _ => return Err(fault("merge group state conflicts with its branch graph")),
    };
    proofs.validate_records(receipt.as_ref())
}

#[test]
fn group_policy_matches_frozen_graph_chain_frontier_receipt_and_record_precedence() {
    use ExecutionOrder::*;
    let orders = [
        Parallel,
        Sequential,
        ResultChain,
        Dataflow,
        BoundParallel,
        BoundSequential,
        GroupTail,
        GroupCapture,
        ParallelGroupTail,
        ParallelGroupCapture,
        GroupDataflow,
        ParallelGroupDataflow,
        GroupConditional,
        ParallelGroupConditional,
    ];
    let owner = ContinuationToken::from_wire("group-owner".into());
    let valid = serde_json::to_vec(&terminal()).unwrap();
    let nonterminal = serde_json::to_vec(&Step::Waiting(MergeWait {
        merge_token: owner.clone(),
        completed_branches: 0,
        total_branches: 2,
    }))
    .unwrap();
    let invalid = serde_json::to_vec(&Step::Fault(Fault {
        code: String::new(),
        message: "invalid".into(),
    }))
    .unwrap();
    let stop = serde_json::to_vec(&Step::Fault(fault("original stop"))).unwrap();
    let mut cases = 0;
    // Exercise order classification, not plan admission; real bound plans are
    // covered by the unchanged SQLite fixtures before this inspection boundary.
    for order in orders {
        let plan = plan(order);
        for defect in 0..12 {
            let mut branches = rows();
            match defect {
                0 => {}
                1 => {
                    branches[0].2 = "completed".into();
                    branches[0].3 = Some(valid.clone());
                }
                2 => {
                    for b in &mut branches {
                        b.2 = "completed".into();
                        b.3 = Some(valid.clone());
                    }
                }
                3 => {
                    branches[1].2 = "completed".into();
                    branches[1].3 = Some(valid.clone());
                }
                4 => branches[0].1 = -1,
                5 => branches[0].0 = "wrong".into(),
                6 => branches[1].4 = branches[0].4.clone(),
                7 => branches[0].4 = owner.as_str().into(),
                8 => {
                    branches.pop();
                }
                9..=11 => {
                    branches[0].2 = "completed".into();
                    branches[0].3 = match defect {
                        9 => None,
                        10 => Some(b"{".to_vec()),
                        _ => Some(stop.clone()),
                    };
                }
                _ => unreachable!(),
            }
            for state in ["pending", "completed", "invalid"] {
                for encoded in [
                    None,
                    Some(valid.as_slice()),
                    Some(b"{".as_slice()),
                    Some(nonterminal.as_slice()),
                    Some(invalid.as_slice()),
                ] {
                    for fail in [
                        None,
                        Some("chain"),
                        Some("blocked"),
                        Some("receipt"),
                        Some("records"),
                    ] {
                        let mut expected = Proofs {
                            fail,
                            ..Default::default()
                        };
                        let result =
                            frozen(&mut expected, &owner, &plan, &branches, state, encoded);
                        let mut actual = Proofs {
                            fail,
                            ..Default::default()
                        };
                        let dynamic: &mut dyn ReferenceGroupProofs = &mut actual;
                        assert_eq!(
                            inspect_group_record(dynamic, &owner, &plan, &branches, state, encoded),
                            result,
                            "{order:?}, {defect}, {state}, {fail:?}"
                        );
                        assert_eq!(actual.trace, expected.trace);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 12600);
}

#[test]
fn pending_prefix_decodes_all_receipts_before_stop_checks_and_native_query() {
    let valid = serde_json::to_vec(&terminal()).unwrap();
    let stop = serde_json::to_vec(&Step::Fault(fault("stop"))).unwrap();
    let bad_terminal = serde_json::to_vec(&Step::Waiting(MergeWait {
        merge_token: ContinuationToken::from_wire("group".into()),
        completed_branches: 0,
        total_branches: 1,
    }))
    .unwrap();
    let mut cases = 0;
    for mask in 0..27 {
        for payload in 0..6 {
            for fail in [None, Some("blocked")] {
                let branches = (0..3)
                    .map(|index| {
                        (
                            format!("b{index}"),
                            index as i64,
                            ["pending", "completed", "legacy"]
                                [(mask / 3_usize.pow(index as u32)) % 3]
                                .into(),
                            match payload {
                                0 => Some(valid.clone()),
                                1 => None,
                                2 => Some(b"{".to_vec()),
                                3 => Some(stop.clone()),
                                4 => Some(bad_terminal.clone()),
                                _ => Some(if index == 0 {
                                    stop.clone()
                                } else {
                                    b"{".to_vec()
                                }),
                            },
                            format!("continuation-{index}"),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut expected = Proofs {
                    fail,
                    ..Default::default()
                };
                let result = frozen_frontier(&branches, &mut expected);
                let mut actual = Proofs {
                    fail,
                    ..Default::default()
                };
                assert_eq!(
                    inspect_pending_sequence(&branches, |first| actual
                        .validate_blocked_dispatches(first)),
                    result
                );
                assert_eq!(actual.trace, expected.trace);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 324);
    for count in [0, 64, 65] {
        let branches = vec![rows().remove(0); count];
        let mut observed = None;
        let result = inspect_pending_sequence(&branches, |first| {
            observed = Some(first);
            Ok(())
        });
        assert_eq!(result.is_ok(), count <= 64);
        assert_eq!(observed, (count <= 64).then_some(0));
    }
}

#[test]
fn borrowed_receipt_bounds_and_original_proof_payload_are_preserved() {
    let owner = ContinuationToken::from_wire("owner".into());
    let mut rows = rows();
    for row in &mut rows {
        row.2 = "completed".into();
    }
    let plan = plan(ExecutionOrder::Parallel);
    let mut bytes = serde_json::to_vec(&terminal()).unwrap();
    bytes.resize(8 * 1024 * 1024, b' ');
    let mut proofs = Proofs::default();
    let local = proofs.local.clone();
    assert_eq!(
        inspect_group_record(
            &mut proofs,
            &owner,
            &plan,
            &rows,
            "completed",
            Some(bytes.as_slice())
        ),
        Ok(())
    );
    assert_eq!(proofs.trace, ["receipt", "records", "terminal:true"]);
    assert_eq!(std::rc::Rc::strong_count(&local), 2);
    bytes.push(b' ');
    proofs.trace.clear();
    assert_eq!(
        inspect_group_record(
            &mut proofs,
            &owner,
            &plan,
            &rows,
            "completed",
            Some(bytes.as_slice())
        )
        .unwrap_err()
        .code,
        "LSV4006"
    );
    assert!(proofs.trace.is_empty());
}

#[test]
fn native_proof_and_move_only_receipt_unwinds_stop_without_reentry() {
    use std::{cell::RefCell, rc::Rc};
    struct Encoded {
        bytes: Vec<u8>,
        trace: Rc<RefCell<Vec<&'static str>>>,
        panic_at: Option<&'static str>,
    }
    impl AsRef<[u8]> for Encoded {
        fn as_ref(&self) -> &[u8] {
            self.trace.borrow_mut().push("decode");
            assert_ne!(self.panic_at, Some("decode"));
            &self.bytes
        }
    }
    impl Drop for Encoded {
        fn drop(&mut self) {
            self.trace.borrow_mut().push("drop");
            assert_ne!(self.panic_at, Some("drop"));
        }
    }
    let owner = ContinuationToken::from_wire("owner".into());
    let plan = plan(ExecutionOrder::ResultChain);
    let mut cases = 0;
    for completed in [false, true] {
        let stages: &[&str] = if completed {
            &["chain", "decode", "receipt", "drop", "records"]
        } else {
            &["chain", "blocked", "records"]
        };
        for stage in stages {
            let mut branches = rows();
            if completed {
                for row in &mut branches {
                    row.2 = "completed".into();
                }
            }
            let trace = Rc::new(RefCell::new(vec![]));
            let mut proofs = Proofs {
                panic_at: Some(stage),
                ..Default::default()
            };
            let encoded = completed.then(|| Encoded {
                bytes: serde_json::to_vec(&terminal()).unwrap(),
                trace: trace.clone(),
                panic_at: Some(stage),
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                inspect_group_record(
                    &mut proofs,
                    &owner,
                    &plan,
                    &branches,
                    if completed { "completed" } else { "pending" },
                    encoded,
                )
            }));
            assert!(result.is_err(), "{stage}");
            let expected: &[&str] = match *stage {
                "chain" => &["chain"],
                "blocked" => &["chain", "blocked"],
                "decode" => &["chain"],
                "receipt" | "drop" => &["chain", "receipt"],
                "records" if completed => &["chain", "receipt", "records"],
                _ => &["chain", "blocked", "frontier:0", "records"],
            };
            assert_eq!(proofs.trace, expected);
            if completed {
                assert_eq!(
                    *trace.borrow(),
                    if *stage == "chain" {
                        vec!["drop"]
                    } else {
                        vec!["decode", "drop"]
                    }
                );
            } else {
                assert!(trace.borrow().is_empty());
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 8);
}
