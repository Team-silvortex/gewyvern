use super::*;
use crate::group_binding::{ReferenceGroupResultBinding, invalid};
use crate::result_binding::{ReferenceProjectedGroup, ReferenceResultBinding};
use crate::{GroupResultBinding, ResultBinding, ScalarValue, Value, Vm};
use leselang_hir::{Effect, Type, host_call::HostOperation};

type Snapshot = (ContinuationToken, MergePlan, Vec<(String, EffectRequest)>);

fn frozen_graph(
    owner: &ContinuationToken,
    plan: &MergePlan,
    rows: &[(&str, &EffectRequest)],
) -> Result<(), Fault> {
    plan.validate_shape(leselang_command::MergePlanShapeLimits {
        max_branches: crate::MAX_MERGE_BRANCHES,
        max_name_bytes: crate::MAX_MERGE_BRANCH_NAME_BYTES,
    })
    .map_err(|error| match error {
        leselang_command::MergePlanShapeError::GroupBinding => invalid(),
        leselang_command::MergePlanShapeError::Successor => crate::successor::invalid(),
        leselang_command::MergePlanShapeError::Branches => Fault {
            code: "LSV2401".into(),
            message: "structured merge plan is invalid or exceeds runtime bounds".into(),
        },
    })?;
    if let Some(binding) = &plan.result_binding {
        binding.validate(plan)?;
    }
    if !valid_continuation_token(owner) {
        return Err(journal_fault("LSV4032", "merge group token is invalid"));
    }
    let mismatch = || {
        journal_fault(
            "LSV4032",
            "merge branch requests do not match the declared plan",
        )
    };
    if rows.len() != plan.branches.len() {
        return Err(mismatch());
    }
    let mut identities = std::collections::HashSet::new();
    for (position, (name, request)) in rows.iter().enumerate() {
        crate::operation_request_tests::frozen_validate_effect_request(request)?;
        if *name != plan.branches[position]
            || request.continuation.token == *owner
            || !identities.insert(request.continuation.token.as_str())
        {
            return Err(mismatch());
        }
    }
    if let Some(binding) = &plan.result_binding {
        frozen_validate_requests(binding, owner, plan, rows)?;
    } else if rows
        .iter()
        .any(|(_, request)| request.continuation.group_result.is_some())
    {
        return Err(invalid());
    }
    Ok(())
}

#[test]
fn complete_merge_graph_admission_preserves_frozen_plan_owner_request_and_correspondence_order() {
    let mut comparisons = 0;
    let mut states = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            for original in snapshots(parallel, kind) {
                states += 1;
                for plan_defect in [0, 25, 27, 28, 29, 32] {
                    for request_defect in [0, 3, 4, 7, 16, 17] {
                        for bad_owner in [false, true] {
                            let mut candidate = original.clone();
                            mutate(&mut candidate, 0, plan_defect);
                            mutate(&mut candidate, 0, request_defect);
                            if bad_owner {
                                candidate.0 = ContinuationToken::from_wire(String::new());
                            }
                            let bytes = serde_json::to_vec(&candidate).unwrap();
                            let rows = candidate
                                .2
                                .iter()
                                .map(|(name, request)| (name.as_str(), request))
                                .collect::<Vec<_>>();
                            assert_eq!(
                                validate_merge_graph_input(&candidate.0, &candidate.1, &rows),
                                frozen_graph(&candidate.0, &candidate.1, &rows),
                                "parallel {parallel}, kind {kind}, plan {plan_defect}, request {request_defect}, owner {bad_owner}"
                            );
                            assert_eq!(serde_json::to_vec(&candidate).unwrap(), bytes);
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(states, 40);
    assert_eq!(comparisons, 2_880);
}

#[test]
fn complete_merge_graph_admission_retains_unbound_group_identity_and_owner_refusals() {
    let mut comparisons = 0;
    for parallel in [false, true] {
        let mut original = snapshots(parallel, 0).remove(0);
        original.1.result_binding = None;
        original.1.order = if parallel {
            ExecutionOrder::Parallel
        } else {
            ExecutionOrder::Sequential
        };
        for (_, request) in &mut original.2 {
            request.continuation.group_result = None;
            request.continuation.schema_version = 1;
        }
        for defect in 0..7 {
            let mut candidate = original.clone();
            match defect {
                0 => {}
                1 => {
                    candidate.2.pop();
                }
                2 => candidate.2[0].0 = "wrong".into(),
                3 => candidate.0 = candidate.2[0].1.continuation.token.clone(),
                4 => candidate.2[1].1 = candidate.2[0].1.clone(),
                5 => {
                    candidate.2[0].1.continuation.group_result = Some(candidate.0.clone());
                    candidate.2[0].1.continuation.schema_version = 6;
                }
                6 => candidate.2[0].1.continuation.program_counter = 0,
                _ => unreachable!(),
            }
            let rows = candidate
                .2
                .iter()
                .map(|(name, request)| (name.as_str(), request))
                .collect::<Vec<_>>();
            let expected = frozen_graph(&candidate.0, &candidate.1, &rows);
            assert_eq!(expected.is_ok(), defect == 0);
            let bytes = serde_json::to_vec(&candidate).unwrap();
            assert_eq!(
                validate_merge_graph_input(&candidate.0, &candidate.1, &rows),
                expected
            );
            assert_eq!(serde_json::to_vec(&candidate).unwrap(), bytes);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 14);
}

fn snapshot(vm: &Vm) -> Snapshot {
    #[cfg(not(feature = "sqlite-journal"))]
    let Journal::Ephemeral(journal) = &vm.journal;
    #[cfg(feature = "sqlite-journal")]
    let journal = match &vm.journal {
        Journal::Ephemeral(journal) => journal,
        _ => panic!("expected memory fixture"),
    };
    let (owner, group) = journal.merge_groups.first_key_value().unwrap();
    (
        owner.clone(),
        group.plan.clone(),
        group
            .plan
            .branches
            .iter()
            .zip(&group.branch_tokens)
            .map(|(name, token)| (name.clone(), journal.dispatches[token].request.clone()))
            .collect(),
    )
}

fn source(parallel: bool, kind: usize) -> String {
    let group = if parallel { "all" } else { "seq" };
    let member = r#"field(value: member(value: g, name: "a"), name: "node_id")"#;
    let focus = format!("ui.focus(node_id: {member})");
    let capture = format!(r#"bind(next: {focus}, body: field(value: next, name: "node_id"))"#);
    let body = match kind {
        0 => member.to_string(),
        1 => focus,
        2 => capture,
        3 => format!(
            r#"bind(first: {focus}, body: bind(next: ui.focus(node_id: field(value: first, name: "node_id")), body: field(value: next, name: "node_id")))"#
        ),
        4 => format!(
            r#"choose(when: eq(left: {member}, right: "a"), then: {capture}, otherwise: "skip")"#
        ),
        _ => unreachable!(),
    };
    format!(
        r#"fn main() = bind(g: {group}(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: {body})"#
    )
}

fn reply(request: &EffectRequest) -> crate::EffectResult {
    let Effect::UiFocus { node_id } = &request.continuation.pending_effect else {
        panic!()
    };
    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
        node_id: node_id.clone(),
    })
}

fn snapshots(parallel: bool, kind: usize) -> Vec<Snapshot> {
    let program = leselang_hir::lower(&leselang_syntax::parse(&source(parallel, kind))).unwrap();
    let mut vm = Vm::new(1000);
    let mut step = vm.start(
        &program,
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        Some(crate::Revision(7)),
    );
    let mut snapshots = vec![];
    loop {
        snapshots.push(snapshot(&vm));
        step = match step {
            Step::Effect(request) => vm.resume(&request.continuation, reply(&request)),
            Step::Effects(batch) => {
                let mut result = None;
                for (index, branch) in batch.branches.iter().enumerate() {
                    let next = vm.resume(&branch.request.continuation, reply(&branch.request));
                    if index + 1 < batch.branches.len() {
                        assert!(matches!(next, Step::Waiting(_)));
                        snapshots.push(snapshot(&vm));
                    }
                    result = Some(next);
                }
                result.unwrap()
            }
            Step::Done(value) => {
                assert!(matches!(
                    value,
                    Value::Scalar { .. } | Value::UiFocus { .. }
                ));
                return snapshots;
            }
            other => panic!("unexpected group fixture step: {other:?}"),
        };
    }
}

fn empty_binding() -> Box<ResultBinding> {
    Box::new(ResultBinding {
        name: "invalid name".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: leselang_hir::computation::Computation::Literal {
            value: ScalarValue::None,
        },
    })
}

fn mutate(snapshot: &mut Snapshot, index: usize, defect: usize) {
    let (owner, plan, requests) = snapshot;
    match defect {
        1 => {
            requests.pop();
        }
        2 => requests.reverse(),
        25 => {
            plan.branches.pop();
        }
        26 => plan.result_binding.as_mut().unwrap().fuel_remaining += 1,
        27 | 35 => plan.result_binding.as_mut().unwrap().binding.name = "invalid name".into(),
        28 => plan.order = ExecutionOrder::Parallel,
        29 => plan
            .result_binding
            .as_mut()
            .unwrap()
            .binding
            .locals
            .push(crate::ScalarBinding {
                name: "invalid name".into(),
                value: ScalarValue::None,
            }),
        30 => requests.push(requests[0].clone()),
        31 => *owner = ContinuationToken::from_wire("merge-999".into()),
        32 => plan.result_binding.as_mut().unwrap().successor_sequence = None,
        _ => {}
    }
    let Some((name, request)) = requests.get_mut(index) else {
        return;
    };
    let image = &mut request.continuation;
    match defect {
        3 => *name = "wrong-name".into(),
        4 => {
            image.pending_effect = Effect::UiFocus {
                node_id: "wrong-node".into(),
            }
        }
        5 => image.result_type = Type::Structured,
        6 => image.group_result = Some(ContinuationToken::from_wire("merge-999".into())),
        7 => image.schema_version = 0,
        8 | 35 => image.fuel_remaining += 1,
        9 => image.deadline_ms += 1,
        10 => image.deadline_at_ms = Some(999),
        11 => image.max_output_items += 1,
        12 => image.expected_revision = Some(crate::Revision(8)),
        13 | 14 => {
            let crate::EffectOperation::Presentation(envelope) = &mut request.operation else {
                panic!()
            };
            if defect == 13 {
                envelope.principal.id = "different".into();
            } else {
                envelope.capabilities =
                    crate::CapabilitySet::new(["ui.presentation", "runtime.read"]);
            }
        }
        15 => image.token = ContinuationToken::from_wire("continuation-01".into()),
        16 => request.effect_id = "wrong-effect".into(),
        17 => request.budget.fuel_remaining += 1,
        18 => image.result_binding = None,
        19 => image.result_binding = Some(empty_binding()),
        20..=23 | 34 => {
            if let Some(binding) = &mut image.result_binding {
                match defect {
                    20 => binding.groups.clear(),
                    21 => binding.groups[0].name = "different-group".into(),
                    22 => binding.groups[0].group.members[0].name = "different-member".into(),
                    23 => {
                        binding.groups[0].group.members[0].result.operation =
                            HostOperation::UiAssertVisible
                    }
                    34 => {
                        binding.body = leselang_hir::computation::Computation::Local {
                            name: "missing".into(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        24 => {
            image.token = ContinuationToken::from_wire(format!(
                "continuation-{}",
                plan.result_binding
                    .as_ref()
                    .unwrap()
                    .successor_sequence
                    .unwrap_or(u64::MAX)
            ))
        }
        33 => image.fuel_remaining = 0,
        _ => {}
    }
}

#[test]
fn group_request_correspondence_preserves_real_journal_snapshots_and_corruption_order() {
    let mut snapshots_count = 0;
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            for original in snapshots(parallel, kind) {
                snapshots_count += 1;
                let names = original
                    .2
                    .iter()
                    .map(|(name, request)| (name.as_str(), request))
                    .collect::<Vec<_>>();
                assert_eq!(
                    frozen_validate_requests(
                        original.1.result_binding.as_ref().unwrap(),
                        &original.0,
                        &original.1,
                        &names
                    ),
                    Ok(())
                );
                for index in [0, original.2.len() - 1] {
                    for defect in 0..36 {
                        let mut state = original.clone();
                        mutate(&mut state, index, defect);
                        let before = serde_json::to_vec(&state).unwrap();
                        let (owner, plan, requests) = &state;
                        let binding = plan.result_binding.as_ref().unwrap();
                        let names = requests
                            .iter()
                            .map(|(name, request)| (name.as_str(), request))
                            .collect::<Vec<_>>();
                        let expected = frozen_validate_requests(binding, owner, plan, &names);
                        assert_eq!(
                            binding.validate_requests(owner, plan, &names),
                            expected,
                            "parallel {parallel}, kind {kind}, index {index}, defect {defect}"
                        );
                        assert_eq!(serde_json::to_vec(&state).unwrap(), before);
                        if defect == 35 {
                            assert_eq!(expected.unwrap_err().code, "LSV1405");
                        }
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(snapshots_count, 40);
    assert_eq!(comparisons, 2880);
}

// Frozen before moving complete group/request correspondence out of the VM.
fn frozen_validate_requests(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    plan: &MergePlan,
    requests: &[(&str, &EffectRequest)],
) -> Result<(), Fault> {
    let final_type = group.validate(plan)?;
    let branches = group.branches()?;
    let first = requests.first().ok_or_else(invalid)?.1;
    if requests.len() != plan.branches.len()
        || first
            .continuation
            .fuel_remaining
            .checked_sub((group.branches()?.len() - 1) as u64)
            != Some(group.fuel_remaining)
    {
        return Err(invalid());
    }
    for (index, (branch, (name, request))) in group.branches()?.iter().zip(requests).enumerate() {
        let image = &request.continuation;
        let expected_fuel =
            first
                .continuation
                .fuel_remaining
                .checked_sub(if plan.order.is_sequential() {
                    index as u64
                } else {
                    0
                });
        if *name != branch.name
            || image.pending_effect != branch.effect
            || image.result_type != branch.result_type
            || image.group_result.as_ref() != Some(owner)
            || image.result_binding.is_some()
            || image.schema_version != group.continuation_schema()
            || Some(image.fuel_remaining) != expected_fuel
            || !frozen_same_context(first, request)
        {
            return Err(invalid());
        }
    }
    let mut previous_fuel = group.fuel_remaining;
    for (index, (name, request)) in requests.iter().skip(branches.len()).enumerate() {
        let image = &request.continuation;
        let sequence = group.reserved_sequences().nth(index).ok_or_else(invalid)?;
        if *name != group.successor_name_at(index)?
            || image.token.as_str() != format!("continuation-{sequence}")
            || image.schema_version != group.continuation_schema()
            || image.group_result.as_ref() != Some(owner)
            || HostOperation::for_effect(&image.pending_effect).is_none()
            || if group.captures_successor() {
                image.result_binding.as_ref().is_none_or(|binding| {
                    !(binding.body.is_pure()
                        || (group.is_dataflow() && binding.body.is_result_chain())
                        || (group.is_conditional() && binding.body.is_result_flow()))
                        || binding.groups.is_empty()
                        || binding.validate(&image.pending_effect).ok() != Some(final_type)
                        || !frozen_capture_accepts_type_at(
                            &group.binding.body,
                            index,
                            image.result_type,
                        )
                        || !binding
                            .groups
                            .iter()
                            .any(|saved_group| saved_group.name == group.binding.name)
                        || binding
                            .groups
                            .iter()
                            .any(|group| !group.group.matches_signature(branches))
                })
            } else {
                image.result_binding.is_some() || image.result_type != final_type
            }
            || image.fuel_remaining >= previous_fuel
            || !frozen_same_context(first, request)
        {
            return Err(invalid());
        }
        previous_fuel = image.fuel_remaining;
    }
    if let Some(sequence) = group.successor_sequence
        && requests
            .iter()
            .take(group.branches()?.len())
            .any(|(_, request)| {
                crate::canonical_continuation_sequence(&request.continuation.token)
                    .is_none_or(|prefix| prefix >= sequence)
            })
    {
        return Err(invalid());
    }
    Ok(())
}

fn frozen_capture_accepts_type_at(
    expression: &leselang_hir::computation::Computation,
    position: usize,
    ty: Type,
) -> bool {
    use leselang_hir::computation::Computation;
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => {
            frozen_capture_accepts_type_at(body, position, ty)
        }
        Computation::Bind { body, .. } if position > 0 => {
            frozen_capture_accepts_type_at(body, position - 1, ty)
        }
        Computation::Bind { value, .. } => value
            .prepared_atomic_operation()
            .is_some_and(|operation| operation.result_type() == ty),
        Computation::Choose {
            then, otherwise, ..
        } => {
            frozen_capture_accepts_type_at(then, position, ty)
                || frozen_capture_accepts_type_at(otherwise, position, ty)
        }
        _ => false,
    }
}

fn frozen_same_context(original: &EffectRequest, saved: &EffectRequest) -> bool {
    leselang_runtime_core::validate_inherited_context(
        &original.continuation.resource_budget(),
        &saved.continuation.resource_budget(),
        || {
            Ok::<_, std::convert::Infallible>(
                crate::successor::authority(&original.operation)
                    == crate::successor::authority(&saved.operation),
            )
        },
        || Ok(original.continuation.expected_revision == saved.continuation.expected_revision),
    )
    .is_ok()
}
