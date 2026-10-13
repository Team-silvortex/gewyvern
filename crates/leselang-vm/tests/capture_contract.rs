use leselang_hir::{
    Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, Type, computation::Computation,
    host_call::HostOperation, lower, result_field::ResultField,
};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::CapturedBindings;
use leselang_syntax::parse;
use leselang_vm::{
    ContinuationImage, ContinuationToken, EffectRequest, EffectResult, PresentationResult,
    ProjectedBinding, ProjectedField, ProjectedGroup, ProjectedGroupBinding, ProjectedResult,
    ResultBinding, ScalarBinding, ScalarValue, Step, Value, Vm, decode_continuation,
    encode_continuation,
};

type SharedFrame = CapturedBindings<Computation, ProjectedResult, ProjectedGroup>;

#[test]
fn product_result_binding_alias_keeps_exact_core_vm_types_and_original_buffers() {
    let mut captured: leselang_command::ResultBinding = serde_json::from_str(
        r#"{"name":"reply","locals":[],"body":{"kind":"literal","value":{"kind":"none"}}}"#,
    )
    .unwrap();
    captured.results.push(ProjectedBinding {
        name: "saved".into(),
        result: projection("node"),
    });
    let wire = serde_json::to_vec(&captured).unwrap();
    let name = captured.name.as_ptr();
    let results = captured.results.as_ptr();
    let fields = captured.results[0].result.fields.as_ptr();
    let vm: ResultBinding = captured;
    let core: SharedFrame = vm;
    let adapter: leselang_command::ResultBinding = core;
    assert_eq!(adapter.name.as_ptr(), name);
    assert_eq!(adapter.results.as_ptr(), results);
    assert_eq!(adapter.results[0].result.fields.as_ptr(), fields);
    assert_eq!(serde_json::to_vec(&adapter).unwrap(), wire);
}

#[test]
fn product_group_aliases_keep_exact_core_and_vm_types_with_original_buffers() {
    let result = leselang_command::ProjectedResult::capture(
        HostOperation::UiFocus,
        &Value::UiFocus {
            node_id: "node-a".into(),
        },
    )
    .unwrap();
    let binding = leselang_command::ProjectedBinding {
        name: "member-a".into(),
        result,
    };
    let name = binding.name.as_ptr();
    let fields = binding.result.fields.as_ptr();
    let binding: ProjectedBinding = binding;
    let binding: leselang_runtime_core::ProjectedBinding<ProjectedResult> = binding;
    let group = leselang_command::ProjectedGroup {
        members: vec![binding],
    };
    let members = group.members.as_ptr();
    let wire = serde_json::to_vec(&group).unwrap();
    let group: ProjectedGroup = group;
    let group: leselang_runtime_core::ProjectedGroup<ProjectedResult> = group;
    assert_eq!(group.members.as_ptr(), members);
    assert_eq!(group.members[0].name.as_ptr(), name);
    assert_eq!(group.members[0].result.fields.as_ptr(), fields);
    assert_eq!(serde_json::to_vec(&group).unwrap(), wire);
    let saved = leselang_command::ProjectedGroupBinding {
        name: "saved-group".into(),
        group,
    };
    let saved: ProjectedGroupBinding = saved;
    let saved: leselang_runtime_core::ProjectedGroupBinding<ProjectedGroup> = saved;
    assert_eq!(saved.group.members.as_ptr(), members);
    let restored: ProjectedGroup = serde_json::from_slice(&wire).unwrap();
    assert_eq!(restored, saved.group);
}

#[test]
fn product_capture_reexports_are_exact_adapter_types_without_conversion_or_buffer_copies() {
    let source = Value::UiSetFormValue {
        node_id: "node-a".into(),
        field: "name".into(),
        value: "ready".into(),
    };
    for version in 1..=5 {
        let captured = leselang_command::ProjectedResult::capture_version(
            HostOperation::UiSetFormValue,
            &source,
            version,
        )
        .unwrap();
        let bytes = serde_json::to_vec(&captured).unwrap();
        let fields = captured.fields.as_ptr();
        let node = captured.fields[0].value.text().unwrap().as_ptr();
        let reference: ProjectedResult = captured;
        assert_eq!(reference.fields.as_ptr(), fields);
        assert_eq!(reference.fields[0].value.text().unwrap().as_ptr(), node);
        assert_eq!(serde_json::to_vec(&reference).unwrap(), bytes);
        let returned: leselang_command::ProjectedResult = reference;
        let field: leselang_command::ProjectedField = returned.fields.into_iter().next().unwrap();
        let field: ProjectedField = field;
        let shared: leselang_runtime_core::ScalarProjectionField<ResultField> = field;
        assert_eq!(shared.value.text().unwrap().as_ptr(), node);
        let restored: ProjectedResult = serde_json::from_slice(&bytes).unwrap();
        assert!(
            restored
                .matches_capture(HostOperation::UiSetFormValue, &source)
                .unwrap()
        );
    }
}
type SavedFrame = leselang_runtime_core::ContinuationFrame<
    ContinuationToken,
    Revision,
    Type,
    Effect,
    ResultBinding,
>;

// Frozen pre-extraction serde layout, independently of the shared specialization.
#[derive(serde::Serialize)]
struct LegacyImage<'a> {
    schema_version: u32,
    token: &'a ContinuationToken,
    program_counter: u32,
    expected_revision: Option<Revision>,
    result_type: &'a Type,
    pending_effect: &'a Effect,
    fuel_remaining: u64,
    deadline_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline_at_ms: Option<u64>,
    max_output_items: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_binding: Option<&'a ResultBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_result: Option<&'a ContinuationToken>,
}

#[test]
fn real_saved_frame_and_budget_are_exact_shared_types_with_legacy_wire_order() {
    let mut vm = Vm::new(300);
    let request = start(
        &mut vm,
        "fn main() = bind(a: ui.focus(node_id: \"a\"), body: field(value: a, name: \"node_id\"))",
    );
    let saved: SavedFrame = request.continuation;
    let returned: ContinuationImage = saved;
    let budget: leselang_runtime_core::ResourceBudget = request.budget;
    assert_eq!(budget, returned.resource_budget());
    for binding in [None, returned.result_binding.clone()] {
        let mut frame = returned.clone();
        frame.result_binding = binding;
        for deadline in [None, Some(42)] {
            frame.deadline_at_ms = deadline;
            let legacy = LegacyImage {
                schema_version: frame.schema_version,
                token: &frame.token,
                program_counter: frame.program_counter,
                expected_revision: frame.expected_revision,
                result_type: &frame.result_type,
                pending_effect: &frame.pending_effect,
                fuel_remaining: frame.fuel_remaining,
                deadline_ms: frame.deadline_ms,
                deadline_at_ms: frame.deadline_at_ms,
                max_output_items: frame.max_output_items,
                result_binding: frame.result_binding.as_deref(),
                group_result: frame.group_result.as_ref(),
            };
            assert_eq!(
                serde_json::to_vec(&frame).unwrap(),
                serde_json::to_vec(&legacy).unwrap()
            );
        }
    }
}

#[test]
fn shared_budget_keeps_reference_error_codes_and_rejection_precedence() {
    let mut vm = Vm::new(300);
    let mut frame = start(&mut vm, "fn main() = ui.focus(node_id: \"a\")").continuation;
    frame.fuel_remaining = leselang_vm::MAX_EXECUTION_FUEL + 1;
    frame.deadline_ms = 0;
    frame.deadline_at_ms = Some(0);
    frame.max_output_items = leselang_vm::DEFAULT_MAX_OUTPUT_ITEMS + 1;
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV2007");
    frame.fuel_remaining = 0;
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV2008");
    frame.deadline_ms = 1;
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV2011");
    frame.deadline_at_ms = None;
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV2009");
    frame.max_output_items = 0;
    assert!(encode_continuation(&frame).is_ok());
}

fn start(vm: &mut Vm, source: &str) -> EffectRequest {
    let Step::Effect(request) = vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ) else {
        panic!("effect expected")
    };
    *request
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn projection(node: &str) -> ProjectedResult {
    ProjectedResult {
        projection_version: 1,
        operation: HostOperation::UiFocus,
        fields: vec![ProjectedField {
            field: ResultField::NodeId,
            value: ScalarValue::String(node.into()),
        }],
    }
}

#[test]
fn all_five_legacy_capture_paths_are_exact_shared_types_without_conversion() {
    let name = String::from("original buffer");
    let pointer = name.as_ptr();
    let scalar: ScalarBinding = leselang_runtime_core::ScalarBinding {
        name,
        value: ScalarValue::Integer(1),
    };
    let result: ProjectedBinding = leselang_runtime_core::ProjectedBinding {
        name: "first".into(),
        result: projection("a"),
    };
    let group: ProjectedGroup = leselang_runtime_core::ProjectedGroup {
        members: vec![result],
    };
    let group_binding: ProjectedGroupBinding = leselang_runtime_core::ProjectedGroupBinding {
        name: "batch".into(),
        group,
    };
    let reference: ResultBinding = CapturedBindings {
        name: "reply".into(),
        locals: vec![scalar],
        results: vec![],
        groups: vec![group_binding],
        body: Computation::Literal {
            value: ScalarValue::None,
        },
    };
    let shared: SharedFrame = reference;
    assert_eq!(shared.locals[0].name.as_ptr(), pointer);
    let returned: ResultBinding = shared;
    assert_eq!(returned.locals[0].name.as_ptr(), pointer);
    assert_eq!(returned.groups[0].group.members[0].result, projection("a"));
}

#[test]
fn legacy_empty_and_populated_capture_bytes_keep_field_order_and_projection_version_rules() {
    let empty = r#"{"name":"reply","locals":[],"body":{"kind":"literal","value":{"kind":"none"}}}"#;
    let frame: ResultBinding = serde_json::from_str(empty).unwrap();
    assert_eq!(serde_json::to_string(&frame).unwrap(), empty);
    let populated = r#"{"name":"reply","locals":[{"name":"n","value":{"kind":"integer","value":7}}],"results":[{"name":"first","result":{"operation":"ui.focus","fields":[{"field":"node_id","value":{"kind":"string","value":"a"}}]}}],"groups":[{"name":"batch","group":{"members":[{"name":"member","result":{"operation":"ui.focus","fields":[{"field":"node_id","value":{"kind":"string","value":"b"}}]}}]}}],"body":{"kind":"literal","value":{"kind":"none"}}}"#;
    let shared: SharedFrame = serde_json::from_str(populated).unwrap();
    let reference: ResultBinding = shared;
    assert_eq!(reference.results[0].result.projection_version, 1);
    assert_eq!(serde_json::to_string(&reference).unwrap(), populated);
}

#[test]
fn real_successor_capture_uses_shared_frames_and_reenters_once_with_original_fuel() {
    let mut vm = Vm::new(300);
    let first = start(
        &mut vm,
        r#"fn main() = bind(prefix: "target-", body:
        bind(a: ui.focus(node_id: "a"), body:
        bind(b: ui.focus(node_id: concat(left: prefix, right: field(value: a, name: "node_id"))),
        body: eq(left: field(value: b, name: "node_id"), right: concat(left: prefix, right: field(value: a, name: "node_id"))))))"#,
    );
    let Step::Effect(second) = vm.resume(&first.continuation, focus("a")) else {
        panic!()
    };
    let shared: &SharedFrame = second.continuation.result_binding.as_ref().unwrap();
    assert_eq!(shared.locals[0].value.text(), Some("target-"));
    assert_eq!(shared.results[0].result, projection("a"));
    assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
    let wire = encode_continuation(&second.continuation).unwrap();
    let restored = decode_continuation(&wire).unwrap();
    assert_eq!(encode_continuation(&restored).unwrap(), wire);
    let expected = Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    });
    assert_eq!(vm.resume(&restored, focus("target-a")), expected);
    assert_eq!(vm.resume(&first.continuation, focus("changed")), expected);
    assert_eq!(vm.resume(&restored, focus("changed")), expected);
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(1, 10).unwrap().is_none());
}

#[test]
fn stateful_reentry_keeps_saved_budget_and_once_only_terminal_replay_across_memory_restore() {
    let program = lower(&parse(r#"fn main() = bind(prefix: "p-", body:
        bind(first: ui.focus(node_id: "a"), body:
        bind(second: ui.focus(node_id: field(value: first, name: "node_id")), body:
        concat(left: prefix, right: concat(left: field(value: first, name: "node_id"), right: field(value: second, name: "node_id"))))))"#)).unwrap();
    let mut completed = 0;
    let mut exhausted = 0;
    for grant in 0..=96 {
        let mut original = Vm::new(grant);
        let first = original.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
        );
        let Step::Effect(first) = first else {
            assert!(matches!(first, Step::Fault(f) if f.code == "LSV1001"));
            assert_eq!(original.pending_count(), 0);
            continue;
        };
        let mut restored = Vm::new(1);
        restored.restore_request(*first.clone()).unwrap();
        let next = original.resume(&first.continuation, focus("a"));
        assert_eq!(restored.resume(&first.continuation, focus("a")), next);
        let terminal = if let Step::Effect(second) = next {
            assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
            let wire = encode_continuation(&second.continuation).unwrap();
            let decoded = decode_continuation(&wire).unwrap();
            assert_eq!(encode_continuation(&decoded).unwrap(), wire);
            let mut fresh = Vm::new(1);
            fresh.restore_request(*second.clone()).unwrap();
            let terminal = original.resume(&decoded, focus("a"));
            assert_eq!(restored.resume(&decoded, focus("a")), terminal);
            assert_eq!(fresh.resume(&decoded, focus("a")), terminal);
            assert_eq!(fresh.resume(&decoded, focus("changed")), terminal);
            assert_eq!(fresh.pending_count(), 0);
            assert!(fresh.claim_effect(1, 10).unwrap().is_none());
            terminal
        } else {
            next
        };
        match &terminal {
            Step::Done(Value::Scalar {
                value: ScalarValue::String(value),
            }) => {
                assert_eq!(value, "p-aa");
                completed += 1;
            }
            Step::Fault(f) => {
                assert_eq!(f.code, "LSV1001");
                exhausted += 1;
            }
            other => panic!("unexpected terminal for grant {grant}: {other:?}"),
        }
        assert_eq!(
            original.resume(&first.continuation, focus("changed")),
            terminal
        );
        assert_eq!(
            restored.resume(&first.continuation, focus("changed")),
            terminal
        );
        assert_eq!(original.pending_count(), 0);
        assert_eq!(restored.pending_count(), 0);
        assert!(original.claim_effect(1, 10).unwrap().is_none());
    }
    assert!(completed > 0);
    assert!(exhausted > 0);
}

#[test]
fn shared_data_never_bypasses_product_frame_validation_or_live_original_image_fences() {
    let mut vm = Vm::new(300);
    let first = start(
        &mut vm,
        r#"fn main() = bind(prefix: "target-", body:
        bind(r: ui.focus(node_id: "a"), body: concat(left: prefix, right: field(value: r, name: "node_id"))))"#,
    );
    let before = encode_continuation(&first.continuation).unwrap();
    let mut unbounded = first.clone();
    let shared: &mut SharedFrame = unbounded.continuation.result_binding.as_mut().unwrap();
    shared.locals[0].value = ScalarValue::String("x".repeat(4097));
    let wire = serde_json::to_vec(&unbounded.continuation).unwrap();
    assert!(matches!(decode_continuation(&wire), Err(f) if f.code == "LSV1405"));
    let mut fresh = Vm::new(300);
    assert!(matches!(fresh.restore_request(unbounded), Err(f) if f.code == "LSV1405"));
    assert_eq!(fresh.pending_count(), 0);
    let mut forged = first.continuation.clone();
    let shared: &mut SharedFrame = forged.result_binding.as_mut().unwrap();
    shared.locals[0].value = ScalarValue::String("changed-".into());
    assert!(matches!(vm.resume(&forged, focus("a")), Step::Fault(f) if f.code == "LSV2005"));
    assert_eq!(encode_continuation(&first.continuation).unwrap(), before);
    assert_eq!(vm.pending_count(), 1);
    assert_eq!(
        vm.resume(&first.continuation, focus("a")),
        Step::Done(Value::Scalar {
            value: ScalarValue::String("target-a".into()),
        })
    );
}

#[test]
fn shared_structure_refusals_keep_lsv1405_and_never_consume_original_pending_authority() {
    let mut vm = Vm::new(300);
    let first = start(
        &mut vm,
        r#"fn main() = bind(prefix: "target-", body:
        bind(r: ui.focus(node_id: "a"), body: concat(left: prefix, right: field(value: r, name: "node_id"))))"#,
    );
    let original = encode_continuation(&first.continuation).unwrap();
    let original_fuel = first.budget.fuel_remaining;
    let mut variants = vec![];
    let mut too_many = first.clone();
    let frame = too_many.continuation.result_binding.as_mut().unwrap();
    frame.locals = vec![
        ScalarBinding {
            name: "x".into(),
            value: ScalarValue::None
        };
        MAX_EFFECT_NESTING_DEPTH + 1
    ];
    variants.push(too_many);
    for position in 0..4 {
        let mut oversized = first.clone();
        let frame = oversized.continuation.result_binding.as_mut().unwrap();
        let name = "x".repeat(MAX_BRANCH_NAME_BYTES + 1);
        match position {
            0 => frame.name = name,
            1 => frame.locals[0].name = name,
            2 => frame.results.push(ProjectedBinding {
                name,
                result: projection("a"),
            }),
            _ => frame.groups.push(ProjectedGroupBinding {
                name,
                group: ProjectedGroup { members: vec![] },
            }),
        }
        variants.push(oversized);
    }
    for names in [
        vec![],
        vec!["a"; leselang_vm::MAX_MERGE_BRANCHES],
        vec!["duplicate", "duplicate"],
        vec!["invalid name"],
    ] {
        let mut invalid = first.clone();
        let frame = invalid.continuation.result_binding.as_mut().unwrap();
        frame.groups.push(ProjectedGroupBinding {
            name: "batch".into(),
            group: ProjectedGroup {
                members: names
                    .into_iter()
                    .map(|name| ProjectedBinding {
                        name: name.into(),
                        result: projection("a"),
                    })
                    .collect(),
            },
        });
        variants.push(invalid);
    }
    let mut native_schema = first.clone();
    let mut invalid_result = projection("a");
    invalid_result.projection_version = u32::MAX;
    native_schema
        .continuation
        .result_binding
        .as_mut()
        .unwrap()
        .results
        .push(ProjectedBinding {
            name: "saved".into(),
            result: invalid_result,
        });
    variants.push(native_schema);
    for request in variants {
        let wire = serde_json::to_vec(&request.continuation).unwrap();
        assert!(matches!(decode_continuation(&wire), Err(f) if f.code == "LSV1405"));
        let mut restored = Vm::new(1);
        assert!(matches!(restored.restore_request(request), Err(f) if f.code == "LSV1405"));
        assert_eq!(restored.pending_count(), 0);
        assert_eq!(vm.pending_count(), 1);
        assert_eq!(first.budget.fuel_remaining, original_fuel);
        assert_eq!(encode_continuation(&first.continuation).unwrap(), original);
    }
    assert_eq!(
        vm.resume(&first.continuation, focus("a")),
        Step::Done(Value::Scalar {
            value: ScalarValue::String("target-a".into()),
        })
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn real_shared_scope_keeps_all_capture_names_distinct_and_group_successor_authority_unchanged() {
    let mut vm = Vm::new(1000);
    let first = start(
        &mut vm,
        r#"fn main() = bind(prefix: "prefix-", body:
        bind(g: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")), body:
        bind(saved: ui.focus(node_id: "saved"), body:
        bind(current: ui.focus(node_id: "c"), body:
        concat(left: prefix, right: concat(left: field(value: saved, name: "node_id"),
            right: field(value: member(value: g, name: "first"), name: "node_id")))))))"#,
    );
    let Step::Effect(second) = vm.resume(&first.continuation, focus("a")) else {
        panic!()
    };
    let Step::Effect(saved) = vm.resume(&second.continuation, focus("b")) else {
        panic!()
    };
    let Step::Effect(current) = vm.resume(&saved.continuation, focus("saved")) else {
        panic!()
    };
    let capture = current.continuation.result_binding.as_ref().unwrap();
    assert_eq!(capture.locals[0].name, "prefix");
    assert_eq!(capture.results[0].name, "saved");
    assert_eq!(capture.groups[0].name, "g");
    assert_eq!(capture.name, "current");
    let original = encode_continuation(&current.continuation).unwrap();
    let restored = decode_continuation(&original).unwrap();
    assert_eq!(encode_continuation(&restored).unwrap(), original);
    for position in 0..8 {
        let mut forged = (*current).clone();
        let capture = forged.continuation.result_binding.as_mut().unwrap();
        match position {
            0 => capture.locals[0].name = "saved".into(),
            1 => capture.results[0].name = "prefix".into(),
            2 => capture.groups[0].name = "prefix".into(),
            3 => capture.groups[0].name = "saved".into(),
            4 => capture.name = "g".into(),
            5 => capture.name = "saved".into(),
            6 => capture.results.push(capture.results[0].clone()),
            _ => capture.groups.push(capture.groups[0].clone()),
        }
        assert!(matches!(encode_continuation(&forged.continuation), Err(f) if f.code == "LSV1405"));
        let wire = serde_json::to_vec(&forged.continuation).unwrap();
        assert!(matches!(decode_continuation(&wire), Err(f) if f.code == "LSV1405"));
        let mut fresh = Vm::new(1);
        assert!(matches!(fresh.restore_request(forged), Err(f) if f.code == "LSV1405"));
        assert_eq!(fresh.pending_count(), 0);
        assert_eq!(vm.pending_count(), 1);
        assert_eq!(
            encode_continuation(&current.continuation).unwrap(),
            original
        );
    }
    let expected = Step::Done(Value::Scalar {
        value: ScalarValue::String("prefix-saveda".into()),
    });
    assert_eq!(vm.resume(&restored, focus("c")), expected);
    assert_eq!(vm.resume(&saved.continuation, focus("changed")), expected);
    assert_eq!(vm.resume(&second.continuation, focus("changed")), expected);
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(1, 10).unwrap().is_none());
}

#[test]
fn mixed_snapshot_keeps_group_restore_fence_saved_fuel_wire_and_once_only_successors() {
    let program = lower(&parse(
        r#"fn main() = bind(prefix: "prefix-", body:
        bind(g: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")), body:
        bind(saved: ui.focus(node_id: "saved"), body:
        bind(current: ui.focus(node_id: "c"), body:
        concat(left: prefix, right: concat(left: field(value: saved, name: "node_id"),
            right: field(value: member(value: g, name: "first"), name: "node_id")))))))"#,
    ))
    .unwrap();
    let mut completed = 0;
    let mut exhausted = 0;
    let mut mixed = 0;
    for grant in 0..=192 {
        let mut original = Vm::new(grant);
        let mut step = original.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
        );
        let mut receipts = vec![];
        for node in ["a", "b", "saved", "c"] {
            let Step::Effect(request) = &step else {
                break;
            };
            let wire = encode_continuation(&request.continuation).unwrap();
            assert_eq!(
                encode_continuation(&decode_continuation(&wire).unwrap()).unwrap(),
                wire
            );
            if let Some(capture) = &request.continuation.result_binding
                && !capture.locals.is_empty()
                && !capture.results.is_empty()
                && !capture.groups.is_empty()
            {
                mixed += 1;
            }
            let mut fresh = Vm::new(1);
            let restored = fresh.restore_request(*request.clone());
            let group_owned = request.continuation.group_result.is_some();
            if group_owned {
                assert!(matches!(restored, Err(f) if f.code == "LSV1409"));
                assert_eq!(fresh.pending_count(), 0);
                assert!(fresh.claim_effect(1, 10).unwrap().is_none());
            } else {
                restored.unwrap();
            }
            receipts.push(request.continuation.clone());
            let next = original.resume(&request.continuation, focus(node));
            if !group_owned {
                assert_eq!(
                    fresh.resume(&request.continuation, focus(node)),
                    next,
                    "grant {grant} node {node}"
                );
                assert_eq!(fresh.resume(&request.continuation, focus("changed")), next);
                if !matches!(next, Step::Effect(_)) {
                    assert_eq!(fresh.pending_count(), 0);
                    assert!(fresh.claim_effect(1, 10).unwrap().is_none());
                }
            }
            step = next;
        }
        match &step {
            Step::Done(Value::Scalar {
                value: ScalarValue::String(value),
            }) => {
                assert_eq!(value, "prefix-saveda");
                completed += 1;
            }
            Step::Fault(f) => {
                assert_eq!(f.code, "LSV1001");
                exhausted += 1;
            }
            other => panic!("unexpected terminal for grant {grant}: {other:?}"),
        }
        for receipt in receipts {
            assert_eq!(original.resume(&receipt, focus("changed")), step);
        }
        assert_eq!(original.pending_count(), 0);
        assert!(original.claim_effect(1, 10).unwrap().is_none());
    }
    assert!(completed > 0 && exhausted > 0 && mixed > 0);
}
