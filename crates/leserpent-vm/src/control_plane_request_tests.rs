use super::*;
use leselang_host_contract::RuntimeId;
use leselang_syntax::parse;
use leserpent_domain::{Command, Query};
use leserpent_hir::{host_call::HostOperation, lower};
use serde_json::json;

fn requests() -> Vec<EffectRequest> {
    [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
    ]
    .into_iter()
    .map(|source| {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let capability = HostOperation::for_effect(&program.function.effect)
            .unwrap()
            .required_capability();
        Vm::build_effect_request(
            &program.function.effect,
            program.function.result_type,
            9,
            Principal::new("operator").unwrap(),
            CapabilitySet::new([capability]),
            Some(Revision(7)),
            30_000,
            Some(31_100),
            21,
        )
        .unwrap()
    })
    .collect()
}

fn assert_rejected(request: EffectRequest, code: &str, message: &str) {
    let before = serde_json::to_vec(&request).unwrap();
    let expected = Fault {
        code: code.into(),
        message: message.into(),
    };
    assert_eq!(validate_effect_request(&request), Err(expected.clone()));
    assert_eq!(serde_json::to_vec(&request).unwrap(), before);
    let decode = serde_json::from_slice::<EffectRequest>(&before).unwrap_err();
    assert!(decode.to_string().contains(message), "{decode}");
    let mut vm = Vm::default();
    assert_eq!(vm.restore_request(request), Err(expected));
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn control_plane_admission_keeps_image_identity_authority_payload_and_budget_order() {
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
    for request in requests() {
        for (stage, (code, message)) in failures.iter().enumerate() {
            let mut torn = request.clone();
            if stage < 1 {
                torn.continuation.program_counter = 2;
            }
            if stage < 2 {
                torn.effect_id = "effect-10".into();
            }
            match &mut torn.operation {
                EffectOperation::Query(envelope) => {
                    if stage < 3 {
                        envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
                    }
                    if stage < 4 {
                        envelope.schema_version = 9;
                    }
                    if stage < 5 {
                        envelope.query = match envelope.query {
                            Query::RuntimeList { .. } => Query::RuntimeInspect {
                                runtime_id: RuntimeId::new("other").unwrap(),
                            },
                            _ => Query::RuntimeList {
                                filter: Default::default(),
                            },
                        };
                    }
                }
                EffectOperation::Command(envelope) => {
                    if stage < 3 {
                        envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
                    }
                    if stage < 4 {
                        envelope.schema_version = 9;
                    }
                    if stage < 5 {
                        envelope.command = match envelope.command {
                            Command::DebuggerCancel { .. } => Command::RuntimeRefresh {
                                runtime_id: RuntimeId::new("other").unwrap(),
                            },
                            _ => Command::DebuggerCancel {
                                session_id: "other".into(),
                            },
                        };
                    }
                }
                _ => panic!(),
            }
            torn.budget.fuel_remaining -= 1;
            assert_rejected(torn, code, message);
        }
        let wire = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<EffectRequest>(&wire).unwrap(),
            request
        );
        let mut vm = Vm::default();
        vm.restore_request(request.clone()).unwrap();
        vm.restore_request(request).unwrap();
        assert_eq!(vm.pending_count(), 1);
    }
}

#[test]
fn control_plane_admission_preserves_query_command_category_faults() {
    let cases = requests();
    for request in &cases {
        for other in &cases {
            let same_category = matches!(
                (&request.operation, &other.operation),
                (EffectOperation::Query(_), EffectOperation::Query(_))
                    | (EffectOperation::Command(_), EffectOperation::Command(_))
            );
            let mut torn = request.clone();
            torn.operation = other.operation.clone();
            match &mut torn.operation {
                EffectOperation::Query(envelope) => {
                    envelope.schema_version = 9;
                    envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
                }
                EffectOperation::Command(envelope) => {
                    envelope.schema_version = 9;
                    envelope.capabilities = CapabilitySet::new([] as [&str; 0]);
                }
                _ => panic!(),
            }
            let (code, message) = if same_category {
                ("LSV2011", "effect request capability is inconsistent")
            } else {
                ("LSV2013", "effect request does not match its continuation")
            };
            assert_rejected(torn, code, message);
        }
    }
}

#[test]
fn control_plane_admission_preserves_command_identity_confirmation_and_log_policy() {
    for request in requests() {
        let mutations = match &request.operation {
            EffectOperation::Command(command) => vec![
                ("command_id", json!("different-command")),
                ("idempotency_key", json!("different-key")),
                ("expected_revision", json!(null)),
                ("expected_revision", json!(8)),
                ("origin", json!("cli")),
                ("dry_run", json!(true)),
                (
                    "confirmation",
                    json!(if command.confirmation == Confirmation::Confirmed {
                        "not_required"
                    } else {
                        "confirmed"
                    }),
                ),
            ],
            EffectOperation::Query(query) if matches!(query.query, Query::RuntimeLogs { .. }) => {
                vec![
                    ("after_sequence", json!(0)),
                    ("limit", json!(255)),
                    ("limit", json!(257)),
                ]
            }
            _ => continue,
        };
        for (field, value) in mutations {
            let mut torn = request.clone();
            let mut operation = serde_json::to_value(&torn.operation).unwrap();
            if matches!(request.operation, EffectOperation::Query(_)) {
                operation["payload"]["query"][field] = value;
            } else {
                operation["payload"][field] = value;
            }
            torn.operation = serde_json::from_value(operation).unwrap();
            assert_rejected(
                torn,
                "LSV2013",
                "effect request does not match its continuation",
            );
        }
    }
}
