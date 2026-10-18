use super::*;

// Frozen complete pair gate before moving correspondence into the adapter.
fn frozen(parent: &EffectRequest, result: &Step, next: &EffectRequest) -> Result<(), Fault> {
    crate::operation_request_tests::frozen_validate_effect_request(parent)?;
    crate::operation_request_tests::frozen_validate_effect_request(next)?;
    let first = &parent.continuation;
    let second = &next.continuation;
    let Step::Done(value) = result else {
        return Err(invalid());
    };
    if matches!(value, Value::Scalar { .. }) {
        return Err(invalid());
    }
    if value.result_type() != first.result_type {
        return Err(Fault {
            code: "LSV2103".into(),
            message: "effect result does not match pending effect".into(),
        });
    }
    let count = crate::validate_value(value, 0)?;
    if count > first.max_output_items || first.max_output_items == 0 {
        return Err(Fault {
            code: "LSV2102".into(),
            message: "bound result exceeds the output item limit".into(),
        });
    }
    crate::validate_json_size_capped(value, MAX_JOURNAL_ENTRY_BYTES, "bound host result")?;
    let schema_valid = leselang_command::continuation_schema::successor_schema_matches(
        first.schema_version,
        first.group_result.as_ref().map(ContinuationToken::as_str),
        second.schema_version,
        second.group_result.as_ref().map(ContinuationToken::as_str),
    );
    let same_context = authority(&parent.operation) == authority(&next.operation)
        && first.expected_revision == second.expected_revision
        && first.deadline_ms == second.deadline_ms
        && first.deadline_at_ms == second.deadline_at_ms
        && first.max_output_items == second.max_output_items;
    if !schema_valid
        || first
            .result_binding
            .as_ref()
            .is_none_or(|binding| binding.body.is_pure())
        || !same_context
        || first.fuel_remaining <= second.fuel_remaining
        || canonical_continuation_sequence(&first.token)
            >= canonical_continuation_sequence(&second.token)
    {
        return Err(invalid());
    }
    let prior = first.result_binding.as_ref().ok_or_else(invalid)?;
    let next_type = match &second.result_binding {
        Some(binding) => binding.validate(&second.pending_effect)?,
        None => second.result_type,
    };
    if prior.validate(&first.pending_effect)? != next_type {
        return Err(invalid());
    }
    if let Some(binding) = &second.result_binding {
        validate_extension(prior, binding, &first.pending_effect, value)?;
    }
    Ok(())
}

fn sources() -> Vec<String> {
    let field = r#"field(value: first, name: "node_id")"#;
    let capture = format!(
        r#"bind(next: ui.focus(node_id: {field}), body: field(value: next, name: "node_id"))"#
    );
    let chain = format!(
        r#"bind(next: ui.focus(node_id: {field}), body: bind(last: ui.focus(node_id: field(value: next, name: "node_id")), body: field(value: last, name: "node_id")))"#
    );
    let mut sources = [
        format!("ui.focus(node_id: {field})"),
        capture.clone(),
        chain.clone(),
        format!(r#"choose(when: eq(left: {field}, right: "a"), then: {chain}, otherwise: "skip")"#),
        format!(r#"choose(when: eq(left: {field}, right: "missing"), then: "skip", otherwise: {capture})"#),
    ].map(|body| format!(r#"fn main() = bind(first: ui.focus(node_id: "a"), body: {body})"#)).to_vec();
    for group in ["seq", "all"] {
        sources.push(format!(r#"fn main() = bind(g: {group}(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(first: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body: {capture}))"#));
    }
    sources
}

fn reply(request: &EffectRequest) -> (EffectResult, Step) {
    let Effect::UiFocus { node_id } = &request.continuation.pending_effect else {
        panic!("focus fixture expected")
    };
    (
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: node_id.clone(),
        }),
        Step::Done(Value::UiFocus {
            node_id: node_id.clone(),
        }),
    )
}

fn pairs() -> Vec<(EffectRequest, Step, EffectRequest)> {
    let mut pairs = Vec::new();
    for source in sources() {
        let mut vm = Vm::new(1000);
        let mut step = vm.start(
            &leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7)),
        );
        for _ in 0..10 {
            step = match step {
                Step::Effect(parent) => {
                    let (reply, result) = reply(&parent);
                    let next = vm.resume(&parent.continuation, reply);
                    if parent.continuation.result_binding.is_some()
                        && let Step::Effect(request) = &next
                    {
                        pairs.push((*parent, result, (**request).clone()));
                    }
                    next
                }
                Step::Effects(batch) => {
                    let mut next = None;
                    for branch in batch.branches {
                        next =
                            Some(vm.resume(&branch.request.continuation, reply(&branch.request).0));
                    }
                    next.unwrap()
                }
                Step::Done(_) => break,
                other => panic!("unexpected fixture state: {other:?}"),
            };
        }
        assert!(matches!(step, Step::Done(_)));
    }
    pairs
}

fn mutate(request: &mut EffectRequest, result: &mut Step, defect: usize) {
    let image = &mut request.continuation;
    match defect {
        0 => {}
        1 => image.schema_version = 0,
        2 => image.token = ContinuationToken("continuation-01".into()),
        3 => image.fuel_remaining = 0,
        4 => image.fuel_remaining = 1000,
        5 => image.deadline_ms += 1,
        6 => image.deadline_at_ms = Some(1),
        7 => image.max_output_items = 0,
        8 => image.expected_revision = None,
        9 => image.group_result = Some(ContinuationToken("merge-999".into())),
        10 => image.result_binding = None,
        11 => image.result_type = Type::Structured,
        12 => image.program_counter = 0,
        13 => request.required_capability = "wrong".into(),
        14 => request.effect_id = "effect-999".into(),
        15 | 16 => {
            let EffectOperation::Presentation(envelope) = &mut request.operation else {
                panic!()
            };
            if defect == 15 {
                envelope.principal = Principal::new("other").unwrap();
            } else {
                envelope.capabilities = CapabilitySet::new(["ui.presentation", "runtime.read"]);
            }
        }
        17 => *result = Step::Fault(invalid()),
        18 => {
            *result = Step::Done(Value::Scalar {
                value: ScalarValue::None,
            })
        }
        19 => {
            *result = Step::Done(Value::UiAssertVisible {
                node_id: "a".into(),
            })
        }
        20 => {
            *result = Step::Done(Value::UiFocus {
                node_id: "different".into(),
            })
        }
        21 => {
            *result = Step::Done(Value::UiFocus {
                node_id: "x".repeat(16384),
            })
        }
        22..=29 => {
            if let Some(binding) = &mut image.result_binding {
                match defect {
                    22 => binding.name = "bad name".into(),
                    23 => {
                        binding.body = leselang_hir::computation::Computation::Literal {
                            value: ScalarValue::None,
                        }
                    }
                    24 => binding.results.clear(),
                    25 => binding.groups.clear(),
                    26 => {
                        if let Some(result) = binding.results.first_mut() {
                            result.name = "renamed".into();
                        }
                    }
                    27 => {
                        if let Some(result) = binding.results.first_mut() {
                            result.result.projection_version = u32::MAX;
                        }
                    }
                    28 => {
                        if let Some(result) = binding.results.first_mut() {
                            result.result.fields[0].value = ScalarValue::String("forged".into());
                        }
                    }
                    29 => {
                        binding.body = leselang_hir::computation::Computation::Local {
                            name: "missing".into(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        30 => {
            image.schema_version = 0;
            *result = Step::Fault(invalid());
        }
        31 => request.budget.fuel_remaining += 1,
        _ => unreachable!(),
    }
    if defect != 31 {
        request.budget = image.resource_budget();
    }
}

#[test]
fn successor_pair_preserves_real_atomic_and_group_reentry_and_frozen_refusal_order() {
    let pairs = pairs();
    let mut comparisons = 0;
    for (parent, result, next) in &pairs {
        assert_eq!(frozen(parent, result, next), Ok(()));
        for side in [false, true] {
            for defect in 0..32 {
                let (mut parent, mut result, mut next) =
                    (parent.clone(), result.clone(), next.clone());
                mutate(
                    if side { &mut next } else { &mut parent },
                    &mut result,
                    defect,
                );
                let before = serde_json::to_vec(&(&parent, &result, &next)).unwrap();
                let expected = frozen(&parent, &result, &next);
                assert_eq!(
                    validate_pair(&parent, &result, &next),
                    expected,
                    "side {side}, defect {defect}"
                );
                assert_eq!(
                    serde_json::to_vec(&(&parent, &result, &next)).unwrap(),
                    before
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(pairs.len(), 9);
    assert_eq!(comparisons, 576);
}

#[test]
fn complete_successor_admission_keeps_parent_child_completion_and_value_fault_precedence() {
    let mut comparisons = 0;
    for (parent, result, next) in pairs() {
        for first_defect in [0, 1, 2, 5, 7, 11, 12, 13, 14, 31] {
            for second_defect in [0, 1, 2, 5, 7, 11, 12, 13, 14, 31] {
                for value_defect in [0, 17, 18, 19, 20, 21] {
                    let (mut first, mut done, mut second) =
                        (parent.clone(), result.clone(), next.clone());
                    mutate(&mut first, &mut done, first_defect);
                    mutate(&mut second, &mut done, second_defect);
                    // Result-only defects must not repair either request's torn budget.
                    mutate(&mut parent.clone(), &mut done, value_defect);
                    let before = serde_json::to_vec(&(&first, &done, &second)).unwrap();
                    assert_eq!(
                        validate_pair(&first, &done, &second),
                        frozen(&first, &done, &second),
                        "parent {first_defect}, child {second_defect}, value {value_defect}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&(&first, &done, &second)).unwrap(),
                        before
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 5_400);
}
