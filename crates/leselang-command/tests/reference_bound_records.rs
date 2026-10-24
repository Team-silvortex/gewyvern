use leselang_command::reference_bound_receipt::validate_bound_group_receipt;
use leselang_command::reference_bound_records::{
    ReferenceBoundRecordProofs, StoredBoundRecord, inspect_bound_group_records,
};
use leselang_command::reference_merge::{invalid_group_binding as invalid, validate_merge_plan};
use leselang_command::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use leselang_command::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use leselang_command::reference_value::validate_value;
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectOperation, EffectRequest, ExecutionOrder,
    GroupResultBinding, MAX_MERGE_BRANCHES, MergePlan, PresentationEnvelope, PresentationOperation,
    ResultBinding, Step, StructuredField, Value,
};
use leselang_hir::{Effect, HirBranch, Type, computation::Computation, host_call::HostOperation};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{Fault, OrderedOutcomeSummary, OutcomeOrder, ScalarValue};
use std::{cell::RefCell, rc::Rc};

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Default)]
struct Log {
    events: Vec<String>,
    panic_at: Option<String>,
    calls: usize,
    fail_at: usize,
}
type Trace = Rc<RefCell<Log>>;

fn hit(trace: &Trace, event: String) {
    let panic = {
        let mut log = trace.borrow_mut();
        log.events.push(event.clone());
        log.panic_at.as_ref() == Some(&event)
    };
    assert!(!panic, "original owner unwind");
}

struct Bytes {
    data: Vec<u8>,
    name: String,
    trace: Trace,
}
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        hit(&self.trace, format!("read:{}", self.name));
        &self.data
    }
}
impl Drop for Bytes {
    fn drop(&mut self) {
        self.trace
            .borrow_mut()
            .events
            .push(format!("drop:{}", self.name));
    }
}

fn rows(
    raw: &[StoredBoundRecord],
    read_error: Option<usize>,
    trace: &Trace,
) -> impl Iterator<Item = Result<StoredBoundRecord<Bytes>, Fault>> + use<> {
    let trace = trace.clone();
    let owned = raw
        .iter()
        .enumerate()
        .map(|(index, (name, request, step))| {
            (
                name.clone(),
                request.as_ref().map(|data| Bytes {
                    data: data.clone(),
                    name: format!("q{index}"),
                    trace: trace.clone(),
                }),
                step.as_ref().map(|data| Bytes {
                    data: data.clone(),
                    name: format!("s{index}"),
                    trace: trace.clone(),
                }),
            )
        })
        .collect::<Vec<_>>();
    owned.into_iter().enumerate().map(move |(index, row)| {
        hit(&trace, format!("row:{index}"));
        if read_error == Some(index) {
            Err(fault("row-native", "original typed row read"))
        } else {
            Ok(row)
        }
    })
}

struct Proofs<'a> {
    plan: &'a MergePlan,
    trace: Trace,
}
impl Proofs<'_> {
    fn record(&mut self, event: String) -> Result<(), Fault> {
        hit(&self.trace, event);
        let mut log = self.trace.borrow_mut();
        log.calls += 1;
        if log.calls == log.fail_at {
            Err(fault(
                "proof-native",
                &format!("original proof {}", log.calls),
            ))
        } else {
            Ok(())
        }
    }
    fn binding(&self, binding: &GroupResultBinding) {
        assert!(std::ptr::eq(
            binding,
            self.plan.result_binding.as_deref().unwrap()
        ));
    }
}
impl ReferenceBoundRecordProofs for Proofs<'_> {
    fn validate_capture(
        &mut self,
        binding: &GroupResultBinding,
        request: &EffectRequest,
        fields: &[StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault> {
        self.binding(binding);
        self.record(format!(
            "capture:{}:{}:{operations:?}",
            request.effect_id,
            serde_json::to_string(fields).unwrap()
        ))
    }
    fn validate_pair(
        &mut self,
        previous: &EffectRequest,
        step: &Step,
        request: &EffectRequest,
    ) -> Result<(), Fault> {
        self.record(format!(
            "pair:{}:{}:{}",
            previous.effect_id,
            serde_json::to_string(step).unwrap(),
            request.effect_id
        ))
    }
    fn validate_member(
        &mut self,
        binding: &GroupResultBinding,
        name: &str,
        value: &Value,
    ) -> Result<(), Fault> {
        self.binding(binding);
        self.record(format!(
            "member:{name}:{}",
            serde_json::to_string(value).unwrap()
        ))
    }
    fn validate_graph(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        assert!(std::ptr::eq(plan, self.plan));
        self.record(format!(
            "graph:{}:{}",
            token.as_str(),
            serde_json::to_string(graph).unwrap()
        ))
    }
    fn validate_reservations(&mut self, plan: &MergePlan) -> Result<(), Fault> {
        assert!(std::ptr::eq(plan, self.plan));
        self.record("reservations".into())
    }
}

fn original_rows(plan: &MergePlan) -> Vec<StoredBoundRecord> {
    plan.branches
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let mut request = request();
            let sequence = index + 7;
            request.effect_id = format!("effect-{sequence}");
            request.continuation.token =
                ContinuationToken::from_wire(format!("continuation-{sequence}"));
            let step = Step::Done(Value::UiFocus {
                node_id: name.clone(),
            });
            (
                name.clone(),
                Some(serde_json::to_vec(&request).unwrap()),
                Some(serde_json::to_vec(&step).unwrap()),
            )
        })
        .collect()
}

#[test]
fn bound_rows_match_frozen_results_lazy_reads_and_original_capture_data() {
    let owner = ContinuationToken::from_wire("merge-1".into());
    let terminal = Step::Done(Value::Scalar {
        value: ScalarValue::None,
    });
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            let (binding, base) = group_plan_fixture(parallel, kind);
            for created in 0..=binding.reserved_sequences().count() {
                let mut plan = base.clone();
                for index in 0..created {
                    plan.branches
                        .push(binding.plan_successor_name_at(index, 64).unwrap());
                }
                validate_merge_plan(&plan).unwrap();
                for position in [0, plan.branches.len() - 1] {
                    for defect in 0..11 {
                        let mut raw = original_rows(&plan);
                        match defect {
                            0 => {}
                            1 => raw[position].1 = None,
                            2 => raw[position].1 = Some(b"{".to_vec()),
                            3 => raw[position].2 = None,
                            4 => raw[position].2 = Some(b"{".to_vec()),
                            5 => {
                                raw[position].2 = Some(
                                    serde_json::to_vec(&Step::Fault(fault(
                                        "LSV1001",
                                        "original stop",
                                    )))
                                    .unwrap(),
                                )
                            }
                            6 => {
                                raw[position].2 = Some(
                                    serde_json::to_vec(&Step::Done(Value::Scalar {
                                        value: ScalarValue::None,
                                    }))
                                    .unwrap(),
                                )
                            }
                            7 => {}
                            8 => raw[position].0 = "original-other-name".into(),
                            9 => {
                                let mut request = request();
                                request.continuation.pending_effect =
                                    Effect::Sequence { steps: vec![] };
                                raw[position].1 = Some(serde_json::to_vec(&request).unwrap());
                            }
                            10 => raw[plan.branches.len() - 2].2 = None,
                            _ => unreachable!(),
                        }
                        let original = raw.clone();
                        for fail_at in [0, 1, 2, 3, 5, 7] {
                            for terminal in [None, Some(&terminal)] {
                                let actual_trace = Rc::new(RefCell::new(Log {
                                    fail_at,
                                    ..Log::default()
                                }));
                                let expected_trace = Rc::new(RefCell::new(Log {
                                    fail_at,
                                    ..Log::default()
                                }));
                                let error = (defect == 7).then_some(position);
                                let actual = inspect_bound_group_records(
                                    &owner,
                                    &plan,
                                    rows(&raw, error, &actual_trace),
                                    terminal,
                                    &mut Proofs {
                                        plan: &plan,
                                        trace: actual_trace.clone(),
                                    },
                                );
                                let expected = frozen(
                                    &owner,
                                    &plan,
                                    rows(&raw, error, &expected_trace),
                                    terminal,
                                    &mut Proofs {
                                        plan: &plan,
                                        trace: expected_trace.clone(),
                                    },
                                );
                                assert_eq!(
                                    actual, expected,
                                    "kind={kind} created={created} position={position} defect={defect} fail_at={fail_at}"
                                );
                                assert_eq!(
                                    actual_trace.borrow().events,
                                    expected_trace.borrow().events
                                );
                                if defect == 0 && fail_at == 0 && terminal.is_none() {
                                    assert_eq!(actual, Ok(()));
                                }
                                assert_eq!(raw, original);
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 5280);
}

#[test]
fn local_move_only_rows_drop_once_on_every_read_proof_and_iterator_unwind() {
    let (binding, mut plan) = group_plan_fixture(false, 3);
    for index in 0..2 {
        plan.branches
            .push(binding.plan_successor_name_at(index, 64).unwrap());
    }
    let owner = ContinuationToken::from_wire("merge-1".into());
    let raw = original_rows(&plan);
    let trace = Rc::new(RefCell::new(Log::default()));
    inspect_bound_group_records(
        &owner,
        &plan,
        rows(&raw, None, &trace),
        None,
        &mut Proofs {
            plan: &plan,
            trace: trace.clone(),
        },
    )
    .unwrap();
    let events = trace
        .borrow()
        .events
        .iter()
        .filter(|event| !event.starts_with("drop:"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 21);
    for event in events {
        let trace = Rc::new(RefCell::new(Log {
            panic_at: Some(event.clone()),
            ..Log::default()
        }));
        let mut proofs = Proofs {
            plan: &plan,
            trace: trace.clone(),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let local: &mut dyn ReferenceBoundRecordProofs = &mut proofs;
            inspect_bound_group_records(&owner, &plan, rows(&raw, None, &trace), None, local)
        }));
        assert!(result.is_err(), "{event}");
        let log = trace.borrow();
        let stopped = log.events.iter().position(|value| value == &event).unwrap();
        assert!(
            log.events[stopped + 1..]
                .iter()
                .all(|value| value.starts_with("drop:"))
        );
        for index in 0..4 {
            for prefix in ["q", "s"] {
                assert_eq!(
                    log.events
                        .iter()
                        .filter(|value| **value == format!("drop:{prefix}{index}"))
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn unbound_stream_stays_cold_and_encoded_limits_precede_later_reads() {
    struct NeverRows;
    impl IntoIterator for NeverRows {
        type Item = Result<StoredBoundRecord<Vec<u8>>, Fault>;
        type IntoIter = std::iter::Empty<Self::Item>;
        fn into_iter(self) -> Self::IntoIter {
            panic!("unbound stream entered")
        }
    }
    let (_, mut plan) = group_plan_fixture(false, 0);
    let owner = ContinuationToken::from_wire("merge-1".into());
    plan.result_binding = None;
    let trace = Rc::new(RefCell::new(Log::default()));
    assert_eq!(
        inspect_bound_group_records(
            &owner,
            &plan,
            NeverRows,
            None,
            &mut Proofs {
                plan: &plan,
                trace: trace.clone()
            }
        ),
        Ok(())
    );
    assert!(trace.borrow().events.is_empty());
    let (_, plan) = group_plan_fixture(false, 0);
    for payload in [1, 2] {
        let mut raw = original_rows(&plan);
        let bytes = if payload == 1 {
            &mut raw[0].1
        } else {
            &mut raw[0].2
        };
        bytes
            .as_mut()
            .unwrap()
            .resize(MAX_JOURNAL_ENTRY_BYTES, b' ');
        let trace = Rc::new(RefCell::new(Log::default()));
        inspect_bound_group_records(
            &owner,
            &plan,
            rows(&raw, None, &trace),
            None,
            &mut Proofs {
                plan: &plan,
                trace: trace.clone(),
            },
        )
        .unwrap();
        let bytes = if payload == 1 {
            &mut raw[0].1
        } else {
            &mut raw[0].2
        };
        bytes.as_mut().unwrap().push(b' ');
        let trace = Rc::new(RefCell::new(Log::default()));
        assert_eq!(
            inspect_bound_group_records(
                &owner,
                &plan,
                rows(&raw, None, &trace),
                None,
                &mut Proofs {
                    plan: &plan,
                    trace: trace.clone()
                }
            )
            .unwrap_err()
            .code,
            "LSV4006"
        );
        assert!(!trace.borrow().events.iter().any(|event| event == "row:1"));
    }
}

fn request() -> EffectRequest {
    let continuation = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: None,
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(40_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    };
    EffectRequest {
        effect_id: "effect-7".into(),
        required_capability: "ui.presentation".into(),
        operation: EffectOperation::Presentation(PresentationEnvelope {
            schema_version: 1,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            operation: PresentationOperation::Focus {
                node_id: "a".into(),
            },
        }),
        budget: continuation.resource_budget(),
        continuation,
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

// Frozen pre-extraction loop. The only substitutions are storage/error plumbing and native proof hooks.
fn frozen<Bytes: AsRef<[u8]>>(
    token: &ContinuationToken,
    plan: &MergePlan,
    rows: impl IntoIterator<Item = Result<StoredBoundRecord<Bytes>, Fault>>,
    terminal: Option<&Step>,
    proofs: &mut dyn ReferenceBoundRecordProofs,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    let mut requests: Vec<(String, EffectRequest)> = Vec::with_capacity(plan.branches.len());
    let mut outcomes = OrderedOutcomeSummary::new(
        if plan.order.is_sequential() {
            OutcomeOrder::Sequential
        } else {
            OutcomeOrder::Parallel
        },
        MAX_MERGE_BRANCHES,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )
    .map_err(|_| invalid())?;
    let mut last_step = None;
    let mut prefix_fields = Vec::new();
    let mut prefix_operations = Vec::new();
    let mut previous_step = None;
    let mut successful_prefix = 0usize;
    for row in rows {
        let (name, request, step) = row?;
        let request: EffectRequest = decode_journal_value(
            request.ok_or_else(invalid)?.as_ref(),
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        if binding.captures_successor()
            && requests.len() >= binding.plan_branches().map_err(|_| invalid())?.len()
        {
            proofs.validate_capture(binding, &request, &prefix_fields, &prefix_operations)?;
            if requests.len() > binding.plan_branches().map_err(|_| invalid())?.len() {
                proofs.validate_pair(
                    &requests.last().ok_or_else(invalid)?.1,
                    previous_step.as_ref().ok_or_else(invalid)?,
                    &request,
                )?;
            }
        }
        previous_step = None;
        if let Some(bytes) = step {
            let step: Step = decode_journal_value(bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
            previous_step = Some(step.clone());
            if requests.len() + 1 == plan.branches.len() {
                last_step = Some(step.clone());
            }
            if let Step::Done(value) = step {
                proofs.validate_member(binding, &name, &value)?;
                if requests.len() < binding.plan_branches().map_err(|_| invalid())?.len() {
                    successful_prefix += 1;
                }
                outcomes
                    .observe_success(validate_value(&value, 0)?, |_, _| {
                        Step::Fault(fault("LSV2404", "sequential output exceeds runtime bounds"))
                    })
                    .map_err(|_| invalid())?;
                if binding.captures_successor() && binding.has_successor(plan) {
                    prefix_operations.push(
                        leselang_hir::host_call::HostOperation::for_effect(
                            &request.continuation.pending_effect,
                        )
                        .ok_or_else(invalid)?,
                    );
                    prefix_fields.push(StructuredField {
                        name: name.clone(),
                        value: Box::new(value),
                    });
                }
            } else {
                outcomes.observe_stop(step).map_err(|_| invalid())?;
            }
        }
        requests.push((name, request));
    }
    let graph = requests
        .iter()
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    proofs.validate_graph(token, plan, &graph)?;
    if binding.has_successor(plan)
        && successful_prefix != binding.plan_branches().map_err(|_| invalid())?.len()
    {
        return Err(invalid());
    }
    proofs.validate_reservations(plan)?;
    validate_bound_group_receipt(
        plan,
        requests
            .last()
            .and_then(|(_, request)| request.continuation.result_binding.as_deref()),
        last_step.as_ref(),
        outcomes,
        terminal,
    )
}
