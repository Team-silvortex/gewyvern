use super::*;
use leselang_hir::{host_call::HostOperation, lower};
use leselang_syntax::parse;
use serde_json::json;

fn build(
    effect: &Effect,
    result_type: Type,
    principal: &str,
    grants: &[&str],
    fuel: u64,
) -> Result<EffectRequest, Fault> {
    Vm::build_effect_request(
        effect,
        result_type,
        9,
        Principal {
            id: principal.into(),
        },
        CapabilitySet::new(grants.iter().copied()),
        Some(Revision(7)),
        30_000,
        Some(31_100),
        fuel,
    )
}

#[test]
fn presentation_reply_projection_keeps_legacy_restore_types_and_mismatch_faults() {
    let cases = [
        (
            r#"ui.focus(node_id: "a")"#,
            json!({"kind":"focus","node_id":"a"}),
        ),
        (
            r#"ui.wait_hidden(node_id: "a")"#,
            json!({"kind":"wait_hidden","node_id":"a","timeout_ms":2000}),
        ),
        (
            r#"ui.navigate_focus(node_id: "a", direction: "last")"#,
            json!({"kind":"navigate_focus","node_id":"a","direction":"last","focused_node_id":"b"}),
        ),
        (
            r#"ui.wait_form_field_placeholder(node_id: "a", field: "name", expected: none)"#,
            json!({"kind":"wait_form_field_placeholder","node_id":"a","field":"name","expected":null,"timeout_ms":2000}),
        ),
        (
            r#"ui.assert_child_count(node_id: "a", count: "3")"#,
            json!({"kind":"assert_child_count","node_id":"a","count":3}),
        ),
        (
            r#"ui.set_form_value(node_id: "a", field: "name", value: "ready")"#,
            json!({"kind":"set_form_value","node_id":"a","field":"name","value":"ready"}),
        ),
    ];
    let mismatch = fault("LSV2103", "effect result does not match pending effect");
    for (source, reply_wire) in cases {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let request = build(
            &program.function.effect,
            program.function.result_type,
            "operator",
            &["ui.presentation"],
            21,
        )
        .unwrap();
        let reply =
            || EffectResult::Presentation(serde_json::from_value(reply_wire.clone()).unwrap());
        let expected = atomic_step_from_effect_result(
            &request.continuation,
            Some(&request.operation),
            reply(),
        );
        assert!(matches!(expected, Step::Done(_)), "{source}");
        let mut value_wire = reply_wire.clone();
        value_wire["kind"] = json!(format!("ui_{}", reply_wire["kind"].as_str().unwrap()));
        value_wire.as_object_mut().unwrap().remove("timeout_ms");
        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            json!({"kind":"done","payload":value_wire})
        );
        assert_eq!(
            atomic_step_from_effect_result(&request.continuation, None, reply()),
            expected
        );
        let mut legacy = Vm::default();
        legacy.restore(request.continuation.clone()).unwrap();
        assert_eq!(
            legacy.resume_at(&request.continuation, 1101, reply()),
            expected
        );
        let mut wrong_type = request.continuation.clone();
        wrong_type.result_type = Type::RuntimeList;
        assert_eq!(
            atomic_step_from_effect_result(&wrong_type, Some(&request.operation), reply()),
            mismatch
        );
        let mut wrong_envelope = request.operation.clone();
        let EffectOperation::Presentation(envelope) = &mut wrong_envelope else {
            panic!()
        };
        envelope.operation = PresentationOperation::Focus {
            node_id: "other".into(),
        };
        assert_eq!(
            atomic_step_from_effect_result(&request.continuation, Some(&wrong_envelope), reply()),
            mismatch
        );
        let mut wrong_reply = reply_wire.clone();
        wrong_reply["node_id"] = json!("other");
        assert_eq!(
            atomic_step_from_effect_result(
                &request.continuation,
                None,
                EffectResult::Presentation(serde_json::from_value(wrong_reply).unwrap())
            ),
            mismatch
        );
        for field in ["timeout_ms", "focused_node_id", "expected"] {
            if reply_wire.get(field).is_some() {
                let mut wrong_reply = reply_wire.clone();
                wrong_reply[field] = match field {
                    "timeout_ms" => json!(1999),
                    "focused_node_id" => json!("a"),
                    "expected" => json!(""),
                    _ => unreachable!(),
                };
                assert_eq!(
                    atomic_step_from_effect_result(
                        &request.continuation,
                        None,
                        EffectResult::Presentation(serde_json::from_value(wrong_reply).unwrap())
                    ),
                    mismatch
                );
            }
        }
        assert_eq!(request.continuation.fuel_remaining, 20);
    }
}

#[test]
fn presentation_reply_projection_moves_owned_strings_without_copying() {
    let effect = Effect::UiSetFormValue {
        node_id: "a".into(),
        field: "name".into(),
        value: "ready".into(),
    };
    let request = build(
        &effect,
        Type::UiSetFormValue,
        "operator",
        &["ui.presentation"],
        21,
    )
    .unwrap();
    let (node_id, field, value) = (
        String::from("a"),
        String::from("name"),
        String::from("ready"),
    );
    let pointers = (node_id.as_ptr(), field.as_ptr(), value.as_ptr());
    let result = atomic_step_from_effect_result(
        &request.continuation,
        Some(&request.operation),
        EffectResult::Presentation(PresentationResult::SetFormValue {
            node_id,
            field,
            value,
        }),
    );
    let Step::Done(Value::UiSetFormValue {
        node_id,
        field,
        value,
    }) = result
    else {
        panic!()
    };
    assert_eq!((node_id.as_ptr(), field.as_ptr(), value.as_ptr()), pointers);
}

#[test]
fn product_values_keep_nested_wire_and_legacy_unknown_field_policy() {
    let wire = r#"{"kind":"structured","fields":[{"name":"first","value":{"kind":"scalar","value":{"kind":"integer","value":7}}},{"name":"second","value":{"kind":"ui_wait_hidden","node_id":"a"}}]}"#;
    let value: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), wire);
    assert_eq!(group_binding::value_type(&value), Type::Structured);
    let mut extended = serde_json::to_value(&value).unwrap();
    extended["legacy_extra"] = json!(true);
    extended["fields"][0]["legacy_extra"] = json!(true);
    extended["fields"][1]["value"]["legacy_extra"] = json!(true);
    assert_eq!(serde_json::from_value::<Value>(extended).unwrap(), value);
    assert_eq!(validate_value(&value, 0).unwrap(), 2);
    let raw: Value =
        serde_json::from_value(json!({"kind":"ui_focus","node_id":"bad node"})).unwrap();
    assert_eq!(group_binding::value_type(&raw), Type::UiFocus);
    assert!(validate_value(&raw, 0).is_err());
    assert!(serde_json::from_value::<Value>(json!({"kind":"ui_focus","node_id":7})).is_err());
    assert!(serde_json::from_value::<Value>(json!({"kind":"unknown"})).is_err());
}

#[test]
fn product_leaf_validation_keeps_counts_and_saved_value_compatibility() {
    use leselang_hir::computation::StringListValue;
    for (value, items) in [
        (
            Value::Scalar {
                value: ScalarValue::Integer(0),
            },
            1,
        ),
        (
            Value::Scalar {
                value: ScalarValue::StringList(StringListValue(vec![])),
            },
            1,
        ),
        (
            Value::Scalar {
                value: ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
            },
            2,
        ),
        (
            Value::RuntimeList {
                revision: Revision(7),
                runtimes: vec![],
            },
            0,
        ),
        (
            Value::RuntimeHistory {
                revision: Revision(7),
                entries: vec![],
            },
            0,
        ),
        (
            Value::UiNavigateFocus {
                node_id: "a".into(),
                focused_node_id: "a".into(),
                direction: leselang_hir::UiFocusNavigationDirection::Next,
            },
            1,
        ),
        (
            Value::UiAssertFormFieldMaxLength {
                node_id: "a".into(),
                field: "name".into(),
                max_length: usize::MAX,
            },
            1,
        ),
    ] {
        let before = serde_json::to_vec(&value).unwrap();
        assert_eq!(validate_value(&value, 0), Ok(items));
        assert_eq!(
            validate_value(&value, MAX_STRUCTURED_VALUE_DEPTH),
            Ok(items)
        );
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
}

#[test]
fn product_leaf_validation_keeps_recursive_fault_order_and_messages() {
    let leaf = |valid| Value::UiFocus {
        node_id: if valid { "a" } else { "bad node" }.into(),
    };
    let field = |name: &str, value| StructuredField {
        name: name.into(),
        value: Box::new(value),
    };
    let bounds = "structured value exceeds runtime bounds";
    let names = "structured value fields are invalid";
    let depth = "structured value nesting exceeds runtime bounds";
    for (value, start_depth, message) in [
        (leaf(false), 0, bounds),
        (leaf(false), MAX_STRUCTURED_VALUE_DEPTH + 1, depth),
        (Value::Structured { fields: vec![] }, 0, bounds),
        (
            Value::Structured {
                fields: vec![field("a", leaf(false)), field("a", leaf(true))],
            },
            0,
            bounds,
        ),
        (
            Value::Structured {
                fields: vec![field("a", leaf(true)), field("a", leaf(false))],
            },
            0,
            names,
        ),
        (
            Value::Structured {
                fields: vec![field("bad name", leaf(false))],
            },
            MAX_STRUCTURED_VALUE_DEPTH,
            names,
        ),
        (
            Value::Structured {
                fields: vec![field("a", leaf(false))],
            },
            MAX_STRUCTURED_VALUE_DEPTH,
            depth,
        ),
        (
            Value::Structured {
                fields: (0..=MAX_MERGE_BRANCHES)
                    .map(|i| field(&format!("f{i}"), leaf(true)))
                    .collect(),
            },
            0,
            bounds,
        ),
    ] {
        let before = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            validate_value(&value, start_depth),
            Err(Fault {
                code: "LSV2403".into(),
                message: message.into()
            })
        );
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
    let zero = Value::Structured {
        fields: vec![field(
            "empty",
            Value::RuntimeList {
                revision: Revision(7),
                runtimes: vec![],
            },
        )],
    };
    assert_eq!(validate_value(&zero, 0), Ok(0));
    let mut nested = leaf(true);
    for _ in 0..MAX_STRUCTURED_VALUE_DEPTH {
        nested = Value::Structured {
            fields: vec![field("a", nested)],
        };
    }
    assert_eq!(validate_value(&nested, 0), Ok(1));
    assert_eq!(validate_value(&nested, 1).unwrap_err().message, depth);
}

#[test]
fn presentation_request_mapping_keeps_frozen_bytes_and_single_budget_debit() {
    let cases = [
        (
            r#"ui.focus(node_id: "a")"#,
            r#"{"kind":"focus","node_id":"a"}"#,
        ),
        (
            r#"ui.wait_hidden(node_id: "a")"#,
            r#"{"kind":"wait_hidden","node_id":"a","timeout_ms":2000}"#,
        ),
        (
            r#"ui.navigate_focus(node_id: "a", direction: "last")"#,
            r#"{"kind":"navigate_focus","node_id":"a","direction":"last"}"#,
        ),
        (
            r#"ui.wait_form_field_placeholder(node_id: "a", field: "name", expected: none)"#,
            r#"{"kind":"wait_form_field_placeholder","node_id":"a","field":"name","expected":null,"timeout_ms":2000}"#,
        ),
        (
            r#"ui.assert_child_count(node_id: "a", count: "3")"#,
            r#"{"kind":"assert_child_count","node_id":"a","count":3}"#,
        ),
        (
            r#"ui.set_form_value(node_id: "a", field: "name", value: "ready")"#,
            r#"{"kind":"set_form_value","node_id":"a","field":"name","value":"ready"}"#,
        ),
    ];
    for (source, operation_wire) in cases {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let effect = &program.function.effect;
        let request = build(
            effect,
            program.function.result_type,
            "operator",
            &["ui.presentation"],
            21,
        )
        .unwrap();
        let expected_operation = format!(
            "{{\"kind\":\"presentation\",\"payload\":{{\"schema_version\":1,\"principal\":{{\"id\":\"operator\"}},\"capabilities\":[\"ui.presentation\"],\"operation\":{operation_wire}}}}}"
        );
        assert_eq!(
            serde_json::to_string(&request.operation).unwrap(),
            expected_operation,
            "{source}"
        );
        let operation: serde_json::Value = serde_json::from_str(&expected_operation).unwrap();
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "effect_id": "effect-9",
                "required_capability": "ui.presentation",
                "operation": operation,
                "continuation": {
                    "schema_version": 1, "token": "continuation-9", "program_counter": 1,
                    "expected_revision": 7, "result_type": program.function.result_type,
                    "pending_effect": effect, "fuel_remaining": 20, "deadline_ms": 30000,
                    "deadline_at_ms": 31100, "max_output_items": 10000
                },
                "budget": {"fuel_remaining": 20, "deadline_ms": 30000,
                    "deadline_at_ms": 31100, "max_output_items": 10000}
            })
        );
        let roundtrip: EffectRequest =
            serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
        assert_eq!(roundtrip, request);
        assert_eq!(
            build(
                effect,
                program.function.result_type,
                "operator",
                &["ui.presentation"],
                1
            )
            .unwrap()
            .budget
            .fuel_remaining,
            0
        );
    }
}

#[test]
fn presentation_request_refusals_keep_fuel_shape_type_and_authority_precedence() {
    let effect = Effect::UiFocus {
        node_id: "a".into(),
    };
    for (ty, principal, grants, fuel, code, message) in [
        (
            Type::RuntimeList,
            "bad principal",
            &[][..],
            0,
            "LSV1001",
            "execution fuel exhausted before host operation",
        ),
        (
            Type::RuntimeList,
            "bad principal",
            &[][..],
            21,
            "LSV2016",
            "continuation result type does not match its pending effect",
        ),
        (
            Type::UiFocus,
            "bad principal",
            &[][..],
            21,
            "LSV2011",
            "effect request capability is inconsistent",
        ),
        (
            Type::UiFocus,
            "bad principal",
            &["ui.presentation"][..],
            21,
            "LSV2012",
            "effect request envelope is invalid",
        ),
    ] {
        assert_eq!(
            build(&effect, ty, principal, grants, fuel).unwrap_err(),
            Fault {
                code: code.into(),
                message: message.into()
            }
        );
    }
    let invalid = Effect::UiFocus {
        node_id: "bad node".into(),
    };
    let failure = build(&invalid, Type::RuntimeList, "bad principal", &[], 21).unwrap_err();
    assert_eq!(failure.code, "LSV2016");
    assert_eq!(
        failure.message,
        "continuation result type does not match its pending effect"
    );
    let failure = build(
        &invalid,
        Type::UiFocus,
        "operator",
        &["ui.presentation"],
        21,
    )
    .unwrap_err();
    assert_eq!(failure.code, "LSV2013");
    assert_eq!(
        failure.message,
        "effect request does not match its continuation"
    );
}

#[test]
fn presentation_admission_keeps_image_identity_authority_payload_and_budget_order() {
    let request = build(
        &Effect::UiFocus {
            node_id: "a".into(),
        },
        Type::UiFocus,
        "operator",
        &["ui.presentation"],
        21,
    )
    .unwrap();
    let failures = [
        ("LSV2014", "continuation program counter is unsupported"),
        (
            "LSV2010",
            "effect identifier does not match continuation token",
        ),
        ("LSV2011", "effect request capability is inconsistent"),
        ("LSV2012", "effect request envelope is invalid"),
        ("LSV2013", "effect request does not match its continuation"),
        ("LSV2013", "effect request does not match its continuation"),
    ];
    for (stage, (code, message)) in failures.into_iter().enumerate() {
        let mut torn = request.clone();
        let EffectOperation::Presentation(envelope) = &mut torn.operation else {
            panic!()
        };
        envelope.operation = PresentationOperation::WaitHidden {
            node_id: "bad node".into(),
            timeout_ms: 1,
        };
        if stage < 1 {
            torn.continuation.program_counter = 2;
        }
        if stage < 2 {
            torn.effect_id = "effect-10".into();
        }
        if stage < 3 {
            envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
        }
        if stage < 4 {
            envelope.schema_version = 9;
            envelope.principal.id = "bad principal".into();
        }
        if stage == 5 {
            envelope.operation = PresentationOperation::Focus {
                node_id: "a".into(),
            };
        }
        torn.budget.fuel_remaining -= 1;
        let before = serde_json::to_vec(&torn).unwrap();
        let expected = Fault {
            code: code.into(),
            message: message.into(),
        };
        assert_eq!(validate_effect_request(&torn), Err(expected.clone()));
        assert_eq!(serde_json::to_vec(&torn).unwrap(), before);
        let decode = serde_json::from_slice::<EffectRequest>(&before).unwrap_err();
        // Wire decoding rejects a malformed Principal before VM admission begins.
        let decode_message = if stage < 4 {
            "data did not match any variant of untagged enum EffectRequestWire"
        } else {
            message
        };
        assert!(
            decode.to_string().contains(decode_message),
            "stage {stage}: {decode}"
        );
        let mut vm = Vm::default();
        assert_eq!(vm.restore_request(torn), Err(expected));
        assert_eq!(vm.pending_count(), 0);
    }
    let mut vm = Vm::default();
    vm.restore_request(request.clone()).unwrap();
    vm.restore_request(request).unwrap();
    assert_eq!(vm.pending_count(), 1);
}

#[test]
fn presentation_admission_does_not_apply_gui_authority_to_non_gui_effects() {
    let gui = build(
        &Effect::UiFocus {
            node_id: "a".into(),
        },
        Type::UiFocus,
        "operator",
        &["ui.presentation"],
        21,
    )
    .unwrap();
    let expected = Fault {
        code: "LSV2013".into(),
        message: "effect request does not match its continuation".into(),
    };
    let sources = [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
    ];
    for source in sources {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let capability = HostOperation::for_effect(&program.function.effect)
            .unwrap()
            .required_capability();
        let mut non_gui = build(
            &program.function.effect,
            program.function.result_type,
            "operator",
            &[capability],
            21,
        )
        .unwrap();
        let mut wrong_gui = gui.clone();
        wrong_gui.operation = non_gui.operation.clone();
        match &mut wrong_gui.operation {
            EffectOperation::Query(envelope) => {
                envelope.schema_version = 9;
                envelope.principal.id.clear();
                envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
            }
            EffectOperation::Command(envelope) => {
                envelope.schema_version = 9;
                envelope.principal.id.clear();
                envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
            }
            _ => panic!(),
        }
        assert_eq!(
            validate_effect_request(&wrong_gui),
            Err(expected.clone()),
            "{source}"
        );
        non_gui.operation = gui.operation.clone();
        let EffectOperation::Presentation(envelope) = &mut non_gui.operation else {
            panic!()
        };
        envelope.schema_version = 9;
        envelope.principal.id.clear();
        envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
        assert_eq!(
            validate_effect_request(&non_gui),
            Err(expected.clone()),
            "{source}"
        );
        let mut vm = Vm::default();
        assert_eq!(vm.restore_request(non_gui), Err(expected.clone()));
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn presentation_mapping_cannot_bypass_request_validation_or_reply_correspondence() {
    let effect = Effect::UiFocus {
        node_id: "a".into(),
    };
    let request = build(&effect, Type::UiFocus, "operator", &["ui.presentation"], 21).unwrap();
    let mut torn = request.clone();
    let EffectOperation::Presentation(envelope) = &mut torn.operation else {
        panic!()
    };
    envelope.operation = PresentationOperation::Focus {
        node_id: "b".into(),
    };
    assert!(validate_effect_request(&torn).is_err());
    let mut vm = Vm::new(1000);
    vm.restore_request(request.clone()).unwrap();
    let result = vm.resume_at(
        &request.continuation,
        1101,
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: "b".into(),
        }),
    );
    assert!(matches!(&result, Step::Fault(fault) if fault.code == "LSV2103"));
    assert_eq!(
        vm.resume_at(
            &request.continuation,
            1102,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "a".into()
            })
        ),
        result
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn presentation_mapping_does_not_change_query_command_or_confirmation_routes() {
    for (source, capability, confirmation) in [
        (r#"runtime.list()"#, "runtime.read", None),
        (
            r#"runtime.refresh(runtime_id: "runtime-a")"#,
            "runtime.refresh",
            Some(Confirmation::NotRequired),
        ),
        (
            r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
            "runtime.deploy",
            Some(Confirmation::Confirmed),
        ),
        (
            r#"debugger.cancel(session_id: "session-a")"#,
            "debugger.control",
            Some(Confirmation::Confirmed),
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let effect = &program.function.effect;
        let operation = HostOperation::for_effect(effect).unwrap();
        assert_eq!(operation.required_capability(), capability);
        let request = build(
            effect,
            program.function.result_type,
            "operator",
            &[capability],
            21,
        )
        .unwrap();
        if let Some(confirmation) = confirmation {
            let EffectOperation::Command(command) = request.operation else {
                panic!()
            };
            assert_eq!(command.command_id.as_str(), "leselang-command-9");
            assert_eq!(command.idempotency_key.as_str(), "leselang-effect-9");
            assert_eq!(command.expected_revision, Some(Revision(7)));
            assert_eq!(command.origin, CommandOrigin::Leselang);
            assert_eq!(command.confirmation, confirmation);
            assert!(!command.dry_run);
        } else {
            assert!(matches!(request.operation, EffectOperation::Query(_)));
        }
        let missing = build(effect, program.function.result_type, "operator", &[], 21).unwrap_err();
        assert_eq!(missing.code, "LSV1002");
        assert_eq!(
            missing.message,
            format!(
                "structured branch lowering failed: MissingCapability {{ capability: {capability:?} }}"
            )
        );
    }
}
