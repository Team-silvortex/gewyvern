use leselang_hir::{
    MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, computation::Computation,
    host_call::HostOperation, lower, result_field::ResultField,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::CapturedBindings;
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, PresentationResult, ProjectedBinding, ProjectedField,
    ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding, ScalarBinding,
    ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};

type SharedFrame = CapturedBindings<Computation, ProjectedResult, ProjectedGroup>;

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
