use super::*;

// Frozen construction boundary before moving product lowering out of the VM.
#[allow(clippy::too_many_arguments)]
fn frozen_build_effect_request(
    effect: &Effect,
    result_type: Type,
    sequence: u64,
    principal: Principal,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
    deadline_ms: u64,
    deadline_at_ms: Option<u64>,
    fuel_remaining: u64,
) -> Result<EffectRequest, Fault> {
    use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
    let mut fuel = Fuel::new(fuel_remaining);
    fuel.charge(1).map_err(|_| Fault {
        code: "LSV1001".into(),
        message: "execution fuel exhausted before host operation".into(),
    })?;
    let image = ContinuationImage {
        schema_version: CONTINUATION_SCHEMA_VERSION,
        token: ContinuationToken(format!("continuation-{sequence}")),
        program_counter: 1,
        expected_revision,
        result_type,
        pending_effect: effect.clone(),
        fuel_remaining: fuel.remaining(),
        deadline_ms,
        deadline_at_ms,
        max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        result_binding: None,
        group_result: None,
    };
    let (required_capability, operation) = match PresentationOperation::from_effect(effect) {
        Some(operation) => (
            CAPABILITY_UI_PRESENTATION.to_string(),
            EffectOperation::Presentation(PresentationEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal,
                capabilities,
                operation,
            }),
        ),
        None => {
            let command_id = CommandId::new(format!("leselang-command-{sequence}"))
                .map_err(|_| generated_effect_identity_fault())?;
            let idempotency_key = IdempotencyKey::new(format!("leselang-effect-{sequence}"))
                .map_err(|_| generated_effect_identity_fault())?;
            let plan = lower_effect(
                effect,
                &LoweringContext {
                    principal,
                    capabilities,
                    expected_revision,
                    command_id,
                    idempotency_key,
                    origin: CommandOrigin::Leselang,
                    confirmation: if matches!(
                        effect,
                        Effect::RuntimeDeploy { .. } | Effect::DebuggerCancel { .. }
                    ) {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: false,
                },
            )
            .map_err(|error| Fault {
                code: "LSV1002".to_string(),
                message: format!("structured branch lowering failed: {error:?}"),
            })?;
            let operation = match plan.operation {
                PlannedOperation::Query(query) => EffectOperation::Query(query),
                PlannedOperation::Command(command) => EffectOperation::Command(command),
            };
            (plan.required_capability, operation)
        }
    };
    let request = EffectRequest {
        effect_id: format!("effect-{sequence}"),
        required_capability,
        operation,
        continuation: image.clone(),
        budget: ResourceBudget {
            fuel_remaining: image.fuel_remaining,
            deadline_ms: image.deadline_ms,
            deadline_at_ms: image.deadline_at_ms,
            max_output_items: image.max_output_items,
        },
    };
    validate_effect_request(&request)?;
    Ok(request)
}

#[test]
fn operation_lowering_preserves_frozen_build_wire_fuel_and_refusal_order() {
    for request in requests() {
        for scenario in 0..16 {
            let mut sequence = 9;
            let mut principal = Principal::new("operator").unwrap();
            let mut capabilities = request.operation.authority().1.clone();
            let mut result_type = request.continuation.result_type;
            let mut fuel = 21;
            let mut deadline = 30_000;
            let mut deadline_at = Some(31_100);
            let mut revision = Some(Revision(7));
            match scenario {
                1 => fuel = 0,
                2 => fuel = 1,
                3 => fuel = MAX_EXECUTION_FUEL + 2,
                4 => sequence = 0,
                5 => sequence = MAX_EFFECT_SEQUENCE,
                6 => sequence = MAX_EFFECT_SEQUENCE + 1,
                7 => sequence = u64::MAX,
                8 => capabilities = CapabilitySet::new([] as [&str; 0]),
                9 => principal.id = "private/invalid".into(),
                10 => result_type = Type::Structured,
                11 => deadline = 0,
                12 => deadline = MAX_EFFECT_DEADLINE_MS + 1,
                13 => deadline_at = Some(i64::MAX as u64 + 1),
                14 => revision = Some(Revision(u64::MAX)),
                15 => {
                    fuel = 0;
                    sequence = 0;
                    principal.id.clear();
                    capabilities = CapabilitySet::new([] as [&str; 0]);
                    result_type = Type::Structured;
                    deadline = 0;
                }
                _ => {}
            }
            let effect = &request.continuation.pending_effect;
            let before = serde_json::to_vec(effect).unwrap();
            let expected = frozen_build_effect_request(
                effect,
                result_type,
                sequence,
                principal.clone(),
                capabilities.clone(),
                revision,
                deadline,
                deadline_at,
                fuel,
            );
            let actual = Vm::build_effect_request(
                effect,
                result_type,
                sequence,
                principal,
                capabilities,
                revision,
                deadline,
                deadline_at,
                fuel,
            );
            assert_eq!(actual, expected, "scenario {scenario}");
            assert_eq!(
                serde_json::to_vec(&actual).unwrap(),
                serde_json::to_vec(&expected).unwrap()
            );
            assert_eq!(serde_json::to_vec(effect).unwrap(), before);
        }
    }
}
use crate::continuation_contract_tests::requests;

#[test]
fn request_correspondence_preserves_cold_authority_identity_and_budget_fault_order() {
    use leselang_hir::computation::Computation;
    for original in requests() {
        for cold in [false, true] {
            for defect in 0..12 {
                let mut request = original.clone();
                let host = Computation::Host {
                    effect: Box::new(Effect::UiFocus {
                        node_id: "cold-next".into(),
                    }),
                };
                let body = if cold {
                    Computation::Choose {
                        when: Box::new(Computation::Literal {
                            value: ScalarValue::Boolean(false),
                        }),
                        then: Box::new(Computation::Bind {
                            name: "next".into(),
                            value: Box::new(host),
                            body: Box::new(Computation::Literal {
                                value: ScalarValue::None,
                            }),
                        }),
                        otherwise: Box::new(Computation::Literal {
                            value: ScalarValue::None,
                        }),
                    }
                } else {
                    host
                };
                let binding = ResultBinding {
                    name: "reply".into(),
                    locals: vec![],
                    results: vec![],
                    groups: vec![],
                    body,
                };
                request.continuation.schema_version = binding.schema_version();
                request.continuation.result_binding = Some(Box::new(binding));
                validate_image(&request.continuation).unwrap();
                let (principal, capabilities, schema) = match &mut request.operation {
                    EffectOperation::Query(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                    EffectOperation::Command(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                    EffectOperation::Presentation(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                };
                *capabilities = CapabilitySet::new([
                    request.required_capability.as_str(),
                    CAPABILITY_UI_PRESENTATION,
                ]);
                match defect {
                    1 => *capabilities = CapabilitySet::default(),
                    2 => request.effect_id = "wrong-effect".into(),
                    3 => request.required_capability = "wrong-capability".into(),
                    4 => principal.id = "private/invalid".into(),
                    5 => *schema = u32::MAX,
                    6 => request.budget.fuel_remaining += 1,
                    7 => request.budget.deadline_ms += 1,
                    8 => request.budget.deadline_at_ms = Some(u64::MAX),
                    9 => request.budget.max_output_items += 1,
                    10 | 11 => {
                        *capabilities = CapabilitySet::default();
                        principal.id = "private/invalid".into();
                        request.effect_id = "wrong-effect".into();
                        request.budget.fuel_remaining += 1;
                        if defect == 11 {
                            request.continuation.program_counter = 2;
                        }
                    }
                    _ => {}
                }
                let before = serde_json::to_vec(&request).unwrap();
                let expected = frozen_validate_effect_request(&request);
                if defect == 0 {
                    assert_eq!(expected, Ok(()));
                } else {
                    assert!(expected.is_err());
                }
                assert_eq!(validate_effect_request(&request), expected);
                let decoded = serde_json::from_slice::<EffectRequest>(&before);
                assert_eq!(decoded.is_ok(), expected.is_ok());
                assert_eq!(serde_json::to_vec(&request).unwrap(), before);
            }
        }
    }
}

// Frozen before moving request policy out of the VM. Image checks remain shared;
// the operation-category, authority and payload ordering below is intentionally fixed.
fn frozen_validate_effect_request(request: &EffectRequest) -> Result<(), Fault> {
    use leselang_command::{command_matches_effect, query_matches_effect};
    validate_image(&request.continuation)?;
    if let Some(binding) = &request.continuation.result_binding {
        let (_, capabilities) = successor::authority(&request.operation);
        if binding
            .body
            .required_capabilities()
            .iter()
            .any(|capability| !capabilities.contains(capability))
        {
            return Err(successor::invalid());
        }
    }
    let sequence =
        canonical_continuation_sequence(&request.continuation.token).ok_or_else(|| Fault {
            code: "LSV2010".to_string(),
            message: "effect request has a non-canonical continuation token".to_string(),
        })?;
    let expected_effect_id = format!("effect-{sequence}");
    let expected_command_id = format!("leselang-command-{sequence}");
    let expected_idempotency_key = format!("leselang-effect-{sequence}");
    if request.effect_id != expected_effect_id {
        return Err(Fault {
            code: "LSV2010".to_string(),
            message: "effect identifier does not match continuation token".to_string(),
        });
    }
    let operation_valid = match (&request.continuation.pending_effect, &request.operation) {
        (
            effect @ (Effect::RuntimeList { .. }
            | Effect::RuntimeInspect { .. }
            | Effect::RuntimeHistory { .. }
            | Effect::RuntimeLogs { .. }),
            EffectOperation::Query(query),
        ) => {
            frozen_validate_effect_identity(
                query.schema_version,
                &query.principal,
                &query.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_READ,
            )?;
            query_matches_effect(&query.query, effect)
        }
        (
            effect @ (Effect::RuntimeRefresh { .. } | Effect::RuntimeCapabilitiesRefresh { .. }),
            EffectOperation::Command(command),
        ) => {
            frozen_validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_REFRESH,
            )?;
            command_matches_effect(
                command,
                effect,
                &expected_command_id,
                &expected_idempotency_key,
                request.continuation.expected_revision,
            )
        }
        (effect @ Effect::RuntimeDeploy { .. }, EffectOperation::Command(command)) => {
            frozen_validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_RUNTIME_DEPLOY,
            )?;
            command_matches_effect(
                command,
                effect,
                &expected_command_id,
                &expected_idempotency_key,
                request.continuation.expected_revision,
            )
        }
        (effect @ Effect::DebuggerCancel { .. }, EffectOperation::Command(command)) => {
            frozen_validate_effect_identity(
                command.schema_version,
                &command.principal,
                &command.capabilities,
                &request.required_capability,
                CAPABILITY_DEBUGGER_CONTROL,
            )?;
            command_matches_effect(
                command,
                effect,
                &expected_command_id,
                &expected_idempotency_key,
                request.continuation.expected_revision,
            )
        }
        (effect, EffectOperation::Presentation(presentation))
            if leselang_hir::host_call::HostOperation::for_effect(effect).is_some_and(
                |operation| operation.required_capability() == CAPABILITY_UI_PRESENTATION,
            ) =>
        {
            frozen_validate_effect_identity(
                presentation.schema_version,
                &presentation.principal,
                &presentation.capabilities,
                &request.required_capability,
                CAPABILITY_UI_PRESENTATION,
            )?;
            presentation.operation.matches_effect(effect)
        }
        _ => false,
    };
    if !operation_valid || request.budget != request.continuation.resource_budget() {
        return Err(Fault {
            code: "LSV2013".to_string(),
            message: "effect request does not match its continuation".to_string(),
        });
    }
    Ok(())
}

fn frozen_validate_effect_identity(
    schema_version: u32,
    principal: &Principal,
    capabilities: &CapabilitySet,
    actual_capability: &str,
    expected_capability: &str,
) -> Result<(), Fault> {
    if actual_capability != expected_capability || !capabilities.contains(actual_capability) {
        return Err(Fault {
            code: "LSV2011".to_string(),
            message: "effect request capability is inconsistent".to_string(),
        });
    }
    if schema_version != DOMAIN_SCHEMA_VERSION
        || principal.id.is_empty()
        || principal.id.len() > 128
        || !principal
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(Fault {
            code: "LSV2012".to_string(),
            message: "effect request envelope is invalid".to_string(),
        });
    }
    Ok(())
}

#[test]
fn operation_request_adapter_preserves_frozen_seventy_operation_admission() {
    fn compare(request: &EffectRequest) {
        let before = serde_json::to_vec(request).unwrap();
        let expected = frozen_validate_effect_request(request);
        assert_eq!(validate_effect_request(request), expected);
        assert_eq!(serde_json::to_vec(request).unwrap(), before);
        match expected {
            Ok(()) => assert_eq!(
                serde_json::from_slice::<EffectRequest>(&before).unwrap(),
                *request
            ),
            Err(_) => assert!(serde_json::from_slice::<EffectRequest>(&before).is_err()),
        }
    }
    let cases = requests();
    for request in &cases {
        compare(request);
        for other in &cases {
            let mut torn = request.clone();
            torn.operation = other.operation.clone();
            compare(&torn);
            for stage in 0..4 {
                let mut conflicting = torn.clone();
                let (principal, capabilities, schema) = match &mut conflicting.operation {
                    EffectOperation::Query(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                    EffectOperation::Command(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                    EffectOperation::Presentation(e) => {
                        (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
                    }
                };
                principal.id = "private/invalid".into();
                *capabilities = CapabilitySet::new([] as [&str; 0]);
                *schema = u32::MAX;
                if stage == 0 {
                    conflicting.continuation.program_counter = 2;
                }
                if stage <= 1 {
                    conflicting.effect_id = "wrong-effect".into();
                }
                if stage <= 2 {
                    conflicting.required_capability = "wrong-capability".into();
                }
                conflicting.budget.fuel_remaining = 0;
                compare(&conflicting);
            }
        }
    }
}
