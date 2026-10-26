use leselang_command::{
    EffectOperation, LoweringContext, LoweringError, PresentationEnvelope, PresentationOperation,
    PresentationResult, lower_effect,
};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Confirmation, Principal};
use leselang_syntax::parse;
use leserpent_domain::{CommandId, IdempotencyKey};
use leserpent_hir::{
    Effect, UiFocusNavigationDirection, UiFormInputKind, UiFormRequirementState, UiSelectionState,
    UiSemanticActionKind, UiSemanticNodeKind, lower,
};
use serde_json::{Value, json};

mod request_correspondence {
    use super::*;
    use leselang_command::{
        EffectRequestView, OperationRequestError, RequestCorrespondenceError as Error,
        ResultBinding, validate_preflighted_effect_request,
    };
    use leselang_host_contract::Revision;
    use leselang_runtime_core::{ContinuationFrame, ResourceBudget};
    use leserpent_hir::{Type, computation::Computation, host_call::HostOperation};
    use std::{cell::Cell, rc::Rc};

    struct Request<Token> {
        effect_id: String,
        required: String,
        operation: EffectOperation,
        frame: ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>,
        budget: ResourceBudget,
    }

    impl<Token> Request<Token> {
        fn new(effect: Effect, token: Token) -> Self {
            let required = HostOperation::for_effect(&effect)
                .unwrap()
                .required_capability();
            let (required, operation) = leselang_command::lower_effect_operation(
                &effect,
                Principal::new("operator").unwrap(),
                CapabilitySet::new([required, "ui.presentation"]),
                Some(Revision(7)),
                || {
                    Some((
                        CommandId::new("leselang-command-43").unwrap(),
                        IdempotencyKey::new("leselang-effect-43").unwrap(),
                    ))
                },
            )
            .unwrap();
            let frame = ContinuationFrame {
                schema_version: 1,
                token,
                program_counter: 1,
                expected_revision: Some(Revision(7)),
                result_type: HostOperation::for_effect(&effect).unwrap().result_type(),
                pending_effect: effect,
                fuel_remaining: 19,
                deadline_ms: 2_000,
                deadline_at_ms: Some(3_000),
                max_output_items: 8,
                result_binding: None,
                group_result: None,
            };
            let budget = frame.resource_budget();
            Self {
                effect_id: "effect-43".into(),
                required,
                operation,
                frame,
                budget,
            }
        }

        fn view(&self) -> EffectRequestView<'_, Token> {
            EffectRequestView {
                effect_id: &self.effect_id,
                required_capability: &self.required,
                operation: &self.operation,
                continuation: &self.frame,
                budget: &self.budget,
            }
        }

        fn with_cold_body(&mut self) {
            self.frame.result_binding = Some(Box::new(ResultBinding {
                name: "reply".into(),
                locals: vec![],
                results: vec![],
                groups: vec![],
                body: Computation::Host {
                    effect: Box::new(Effect::UiFocus {
                        node_id: "private-next".into(),
                    }),
                },
            }));
        }
    }

    fn focus() -> Effect {
        Effect::UiFocus {
            node_id: "private-widget".into(),
        }
    }

    fn admission_limits() -> leselang_command::ContinuationAdmissionLimits {
        use leselang_hir_core::projection_access::ProjectionAccessLimits;
        use leselang_runtime_core::{
            CaptureValidationLimits, GroupCaptureLimits, ResourceBudgetLimits,
        };
        leselang_command::ContinuationAdmissionLimits {
            binding: leselang_command::ResultBindingLimits {
                captures: CaptureValidationLimits {
                    max_saved_bindings: 16,
                    max_name_bytes: 64,
                },
                groups: GroupCaptureLimits {
                    min_members: 1,
                    max_members: 4,
                },
                accesses: ProjectionAccessLimits {
                    max_nodes: 256,
                    max_depth: 32,
                },
            },
            budget: ResourceBudgetLimits {
                max_fuel: 100,
                max_deadline_ms: 2000,
                max_deadline_at_ms: 3000,
                max_output_items: 8,
            },
            max_group_sequence: 43,
        }
    }

    #[test]
    fn checked_request_admission_covers_seventy_operations_without_copying_original_frames() {
        let cases = request_admission_cases();
        assert_eq!(cases.len(), 70);
        for (effect, _) in cases {
            let request = Request::new(effect, 43_u64);
            let before =
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap();
            let calls = std::cell::RefCell::new(Vec::new());
            assert_eq!(
                leselang_command::validate_effect_request(
                    request.view(),
                    admission_limits(),
                    |_| panic!("no group owner"),
                    |token| {
                        assert!(std::ptr::eq(token, &request.frame.token));
                        calls.borrow_mut().push("token");
                        true
                    },
                    |_| panic!("no group capture"),
                    |token| {
                        assert!(std::ptr::eq(token, &request.frame.token));
                        calls.borrow_mut().push("sequence");
                        Some(*token)
                    },
                ),
                Ok(())
            );
            assert_eq!(*calls.borrow(), ["token", "sequence"]);
            assert_eq!(
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap(),
                before
            );
        }
    }

    #[test]
    fn checked_request_admission_keeps_frame_before_cold_authority_and_canonical_identity() {
        use leselang_command::continuation_schema::ContinuationSchemaError;
        use leselang_command::{
            ContinuationAdmissionError as Frame, RequestAdmissionError as Admission,
        };
        for stage in 0..9 {
            let mut request = Request::new(focus(), 43_u64);
            request.with_cold_body();
            request.frame.schema_version = 3;
            let expected = match stage {
                0 => {
                    request.frame.schema_version = 0;
                    request.frame.program_counter = 0;
                    *request_authority_mut(&mut request.operation).1 = CapabilitySet::default();
                    Err(Admission::Frame(Frame::Schema(
                        ContinuationSchemaError::Version {
                            actual: 0,
                            expected: 3,
                        },
                    )))
                }
                1 => {
                    request.frame.program_counter = 0;
                    Err(Admission::Frame(Frame::Token))
                }
                2 => {
                    request.frame.program_counter = 0;
                    *request_authority_mut(&mut request.operation).1 = CapabilitySet::default();
                    Err(Admission::Frame(Frame::ProgramCounter))
                }
                3 => {
                    request.frame.fuel_remaining = 101;
                    *request_authority_mut(&mut request.operation).1 = CapabilitySet::default();
                    Err(Admission::Frame(Frame::Budget(
                        leselang_runtime_core::ResourceBudgetError::Fuel,
                    )))
                }
                4 => {
                    *request_authority_mut(&mut request.operation).1 = CapabilitySet::default();
                    request.effect_id = "private/wrong-effect".into();
                    Err(Admission::Correspondence(Error::SuccessorAuthority))
                }
                5 => Err(Admission::Correspondence(Error::ContinuationIdentity)),
                6 => {
                    request.effect_id = "private/wrong-effect".into();
                    request.budget.fuel_remaining += 1;
                    Err(Admission::Correspondence(Error::EffectIdentity))
                }
                7 => {
                    request.budget.fuel_remaining += 1;
                    Err(Admission::Correspondence(Error::Budget))
                }
                _ => Ok(()),
            };
            let before =
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap();
            let token_calls = Cell::new(0);
            let sequence_calls = Cell::new(0);
            let result = leselang_command::validate_effect_request(
                request.view(),
                admission_limits(),
                |_| panic!("no group owner"),
                |_| {
                    token_calls.set(token_calls.get() + 1);
                    stage != 1
                },
                |_| true,
                |_| {
                    sequence_calls.set(sequence_calls.get() + 1);
                    (stage != 5).then_some(43)
                },
            );
            assert_eq!(result, expected, "stage {stage}");
            assert_eq!(token_calls.get(), usize::from(stage != 0));
            assert_eq!(sequence_calls.get(), usize::from(stage >= 5));
            if let Err(error) = result {
                assert!(!format!("{error:?}: {error}").contains("private"));
                assert!(std::error::Error::source(&error).is_none());
            }
            assert_eq!(
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap(),
                before
            );
        }
    }

    #[test]
    fn checked_request_admission_borrows_opaque_local_owners_in_original_hook_order() {
        let drops = Rc::new(Cell::new(0));
        let mut request = Request::new(focus(), LocalToken(Rc::clone(&drops)));
        request.frame.group_result = Some(LocalToken(Rc::clone(&drops)));
        request.frame.schema_version = 6;
        let events = std::cell::RefCell::new(Vec::new());
        assert_eq!(
            leselang_command::validate_effect_request(
                request.view(),
                admission_limits(),
                |owner| {
                    assert!(std::ptr::eq(
                        owner,
                        request.frame.group_result.as_ref().unwrap()
                    ));
                    events.borrow_mut().push("owner");
                    "merge-1"
                },
                |token| {
                    assert!(std::ptr::eq(token, &request.frame.token));
                    events.borrow_mut().push("token");
                    true
                },
                |_| panic!("no group capture"),
                |token| {
                    assert!(std::ptr::eq(token, &request.frame.token));
                    events.borrow_mut().push("sequence");
                    Some(43)
                },
            ),
            Ok(())
        );
        assert_eq!(*events.borrow(), ["owner", "token", "sequence"]);
        assert_eq!(drops.get(), 0);
        drop(request);
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn checked_request_admission_propagates_each_native_unwind_and_drops_unused_hooks() {
        for stage in 0..3 {
            let token_drops = Rc::new(Cell::new(0));
            let hook_drops = Rc::new(Cell::new(0));
            let owner_capture = LocalToken(Rc::clone(&hook_drops));
            let token_capture = LocalToken(Rc::clone(&hook_drops));
            let sequence_capture = LocalToken(Rc::clone(&hook_drops));
            let mut request = Request::new(focus(), LocalToken(Rc::clone(&token_drops)));
            request.frame.group_result = Some(LocalToken(Rc::clone(&token_drops)));
            request.frame.schema_version = 6;
            let calls = std::cell::RefCell::new(Vec::new());
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                leselang_command::validate_effect_request(
                    request.view(),
                    admission_limits(),
                    |_| {
                        let _capture = owner_capture;
                        calls.borrow_mut().push(0);
                        assert_ne!(stage, 0, "trusted owner hook unwind");
                        "merge-1"
                    },
                    |_| {
                        let _capture = token_capture;
                        calls.borrow_mut().push(1);
                        assert_ne!(stage, 1, "trusted token hook unwind");
                        true
                    },
                    |_| true,
                    |_| {
                        let _capture = sequence_capture;
                        calls.borrow_mut().push(2);
                        panic!("trusted sequence hook unwind")
                    },
                )
            }));
            assert!(outcome.is_err());
            assert_eq!(*calls.borrow(), (0..=stage).collect::<Vec<_>>());
            assert_eq!(hook_drops.get(), 3);
            assert_eq!(token_drops.get(), 0);
            assert_eq!(request.frame.fuel_remaining, 19);
            drop(request);
            assert_eq!(token_drops.get(), 2);
        }
    }

    #[test]
    fn checked_request_admission_rechecks_mutable_data_without_promoting_live_grants() {
        use leselang_command::{ContinuationAdmissionError, RequestAdmissionError};
        let mut request = Request::new(focus(), 43_u64);
        let check = |request: &Request<u64>| {
            leselang_command::validate_effect_request(
                request.view(),
                admission_limits(),
                |_| "",
                |_| true,
                |_| true,
                |_| Some(43),
            )
        };
        assert_eq!(check(&request), Ok(()));
        request.frame.program_counter = 0;
        assert_eq!(
            check(&request),
            Err(RequestAdmissionError::Frame(
                ContinuationAdmissionError::ProgramCounter
            ))
        );
        // The low-level gate remains explicitly preflighted, not a frame validator.
        assert_eq!(
            validate_preflighted_effect_request(request.view(), |_| Some(43)),
            Ok(())
        );
        request.frame.program_counter = 1;
        request.frame.fuel_remaining = 0;
        request.frame.max_output_items = 0;
        request.budget = request.frame.resource_budget();
        assert_eq!(check(&request), Ok(()));
        // A consistent exhausted frame is not a debit, refill or execution grant.
        assert_eq!(request.frame.fuel_remaining, 0);
        assert_eq!(request.frame.max_output_items, 0);
    }

    #[test]
    fn full_request_correspondence_borrows_seventy_original_frames_without_wire_changes() {
        for (effect, _) in request_admission_cases() {
            let request = Request::new(effect, 43_u64);
            let before =
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap();
            let calls = Cell::new(0);
            assert_eq!(
                validate_preflighted_effect_request(request.view(), |token| {
                    assert!(std::ptr::eq(token, &request.frame.token));
                    calls.set(calls.get() + 1);
                    Some(*token)
                }),
                Ok(())
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap(),
                before
            );
        }
    }

    #[test]
    fn full_request_correspondence_keeps_cold_identity_operation_and_budget_precedence() {
        for stage in 0..12 {
            let mut request = Request::new(focus(), 43_u64);
            request.with_cold_body();
            request.budget.fuel_remaining += 1;
            let expected = match stage {
                0 => {
                    request_authority_mut(&mut request.operation)
                        .1
                        .clone_from(&CapabilitySet::default());
                    request.effect_id = "wrong-effect".into();
                    Error::SuccessorAuthority
                }
                1 => {
                    request.effect_id = "wrong-effect".into();
                    Error::ContinuationIdentity
                }
                2 => {
                    request.effect_id = "wrong-effect".into();
                    request.required = "private/wrong-capability".into();
                    Error::EffectIdentity
                }
                3 => {
                    request.required = "private/wrong-capability".into();
                    request_authority_mut(&mut request.operation).0.id = "private/invalid".into();
                    Error::Operation(OperationRequestError::Capability)
                }
                4 => {
                    request_authority_mut(&mut request.operation).0.id = "private/invalid".into();
                    Error::Operation(OperationRequestError::Envelope)
                }
                5 => {
                    *request_authority_mut(&mut request.operation).2 = u32::MAX;
                    Error::Operation(OperationRequestError::Envelope)
                }
                6 => {
                    request.frame.pending_effect = Effect::UiFocus {
                        node_id: "different".into(),
                    };
                    Error::Operation(OperationRequestError::Mismatch)
                }
                7 => Error::Budget,
                8 => {
                    request.budget = request.frame.resource_budget();
                    request.budget.deadline_ms += 1;
                    Error::Budget
                }
                9 => {
                    request.budget = request.frame.resource_budget();
                    request.budget.deadline_at_ms = None;
                    Error::Budget
                }
                10 => {
                    request.budget = request.frame.resource_budget();
                    request.budget.max_output_items += 1;
                    Error::Budget
                }
                _ => {
                    request.frame.result_binding = None;
                    request.operation = request_admission_cases().remove(0).1;
                    request_authority_mut(&mut request.operation).0.id = "private/invalid".into();
                    Error::Operation(OperationRequestError::Mismatch)
                }
            };
            let before =
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap();
            let calls = Cell::new(0);
            let error = validate_preflighted_effect_request(request.view(), |_| {
                calls.set(calls.get() + 1);
                (stage != 1).then_some(43)
            })
            .unwrap_err();
            assert_eq!(error, expected, "stage {stage}");
            assert_eq!(calls.get(), usize::from(stage != 0));
            assert!(!format!("{error:?}: {error}").contains("private"));
            assert!(std::error::Error::source(&error).is_none());
            assert_eq!(
                serde_json::to_vec(&(&request.frame, &request.operation, &request.budget)).unwrap(),
                before
            );
        }
    }

    struct LocalToken(Rc<Cell<usize>>);
    impl Drop for LocalToken {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn full_request_correspondence_accepts_opaque_local_tokens_and_drops_skipped_hooks() {
        for denied in [false, true] {
            let token_drops = Rc::new(Cell::new(0));
            let hook_drops = Rc::new(Cell::new(0));
            let capture = LocalToken(Rc::clone(&hook_drops));
            let mut request = Request::new(focus(), LocalToken(Rc::clone(&token_drops)));
            request.with_cold_body();
            if denied {
                *request_authority_mut(&mut request.operation).1 = CapabilitySet::default();
            }
            let calls = Cell::new(0);
            let result = validate_preflighted_effect_request(request.view(), |token| {
                let _capture = capture;
                calls.set(calls.get() + 1);
                assert!(std::ptr::eq(token, &request.frame.token));
                Some(43)
            });
            assert_eq!(
                result,
                if denied {
                    Err(Error::SuccessorAuthority)
                } else {
                    Ok(())
                }
            );
            assert_eq!(calls.get(), usize::from(!denied));
            assert_eq!(hook_drops.get(), 1);
            assert_eq!(token_drops.get(), 0);
            drop(request);
            assert_eq!(token_drops.get(), 1);
        }
    }

    #[test]
    fn full_request_correspondence_never_retries_native_identity_unwind() {
        let request = Request::new(focus(), 43_u64);
        let calls = Cell::new(0);
        let drops = Rc::new(Cell::new(0));
        let capture = LocalToken(Rc::clone(&drops));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validate_preflighted_effect_request(request.view(), |_| {
                let _capture = capture;
                calls.set(calls.get() + 1);
                panic!("trusted native hook unwind")
            })
        }));
        assert!(outcome.is_err());
        assert_eq!(calls.get(), 1);
        assert_eq!(drops.get(), 1);
        assert_eq!(request.frame.fuel_remaining, 19);
    }

    #[test]
    fn full_request_correspondence_does_not_admit_frames_or_authenticate_equal_budgets() {
        let mut request = Request::new(focus(), 43_u64);
        request.frame.schema_version = u32::MAX;
        request.frame.program_counter = u32::MAX;
        request.frame.result_type = Type::Structured;
        request.frame.fuel_remaining = u64::MAX;
        request.frame.deadline_ms = 0;
        request.budget = request.frame.resource_budget();
        assert_eq!(
            validate_preflighted_effect_request(request.view(), |_| Some(43)),
            Ok(())
        );
        assert_eq!(
            validate_preflighted_effect_request(request.view(), |_| None),
            Err(Error::ContinuationIdentity)
        );
    }

    #[test]
    fn inherited_request_context_borrows_different_token_types_and_leaves_fuel_external() {
        for defect in 0..9 {
            let original = Request::new(focus(), 43_u64);
            let drops = Rc::new(Cell::new(0));
            let mut saved = Request::new(focus(), LocalToken(Rc::clone(&drops)));
            match defect {
                1 => saved.frame.fuel_remaining = 0,
                2 => saved.frame.expected_revision = None,
                3 => request_authority_mut(&mut saved.operation).0.id = "different".into(),
                4 => *request_authority_mut(&mut saved.operation).1 = CapabilitySet::default(),
                5 => saved.frame.deadline_ms += 1,
                6 => saved.frame.deadline_at_ms = None,
                7 => saved.frame.max_output_items += 1,
                8 => saved.budget.fuel_remaining = u64::MAX,
                _ => {}
            }
            assert_eq!(
                original.view().has_same_inherited_context(&saved.view()),
                matches!(defect, 0 | 1 | 8)
            );
            assert_eq!(drops.get(), 0);
            drop(saved);
            assert_eq!(drops.get(), 1);
        }
    }
}

fn request_admission_cases() -> Vec<(Effect, EffectOperation)> {
    let mut result = control_plane_cases()
        .into_iter()
        .map(|(effect, operation)| {
            let operation = match operation {
                leselang_command::PlannedOperation::Query(query) => EffectOperation::Query(query),
                leselang_command::PlannedOperation::Command(command) => {
                    EffectOperation::Command(command)
                }
            };
            (effect, operation)
        })
        .collect::<Vec<_>>();
    result.extend(cases().into_iter().map(|mut case| {
        if case.operation_wire.get("timeout_ms").is_some() {
            case.operation_wire["timeout_ms"] = json!(2000);
        }
        (
            effect_from_wire(&case.operation_wire),
            EffectOperation::Presentation(PresentationEnvelope {
                schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
                principal: Principal::new("operator").unwrap(),
                capabilities: CapabilitySet::new(["ui.presentation"]),
                operation: serde_json::from_value(case.operation_wire).unwrap(),
            }),
        )
    }));
    assert_eq!(result.len(), 70);
    result
}

#[test]
fn request_construction_covers_seventy_operations_and_moves_original_gui_authority() {
    use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
    for (effect, operation) in request_admission_cases() {
        let before = serde_json::to_vec(&effect).unwrap();
        let principal = operation.authority().0.clone();
        let principal_buffer = principal.id.as_ptr();
        let request = build_effect_request(ReservedEffectInput {
            effect: &effect,
            result_type: leserpent_hir::host_call::HostOperation::for_effect(&effect)
                .unwrap()
                .result_type(),
            sequence: 9,
            principal,
            capabilities: operation.authority().1.clone(),
            expected_revision: request_context(&effect).expected_revision,
            deadline_ms: 30_000,
            deadline_at_ms: Some(31_100),
            fuel_remaining: 21,
        })
        .unwrap();
        assert_eq!(request.effect_id, "effect-9");
        assert_eq!(request.continuation.token.as_str(), "continuation-9");
        assert_eq!(request.continuation.fuel_remaining, 20);
        assert_eq!(request.continuation.deadline_at_ms, Some(31_100));
        assert_eq!(request.budget, request.continuation.resource_budget());
        assert_eq!(request.operation.authority(), operation.authority());
        if matches!(request.operation, EffectOperation::Presentation(_)) {
            assert_eq!(
                request.operation.authority().0.id.as_ptr(),
                principal_buffer
            );
        }
        assert_eq!(serde_json::to_vec(&effect).unwrap(), before);
        leselang_command::reference_request::validate_effect_request(&request).unwrap();
        let bytes = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<leselang_command::EffectRequest>(&bytes).unwrap(),
            request
        );
    }
}

#[test]
fn request_construction_debits_before_shape_checks_without_repairing_public_inputs() {
    use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
    for (effect, operation) in request_admission_cases() {
        let input = |fuel| ReservedEffectInput {
            effect: &effect,
            result_type: leserpent_hir::Type::Structured,
            sequence: 0,
            principal: operation.authority().0.clone(),
            capabilities: operation.authority().1.clone(),
            expected_revision: None,
            deadline_ms: 0,
            deadline_at_ms: Some(u64::MAX),
            fuel_remaining: fuel,
        };
        assert_eq!(build_effect_request(input(0)).unwrap_err().code, "LSV1001");
        assert_ne!(build_effect_request(input(2)).unwrap_err().code, "LSV1001");
    }
}

#[test]
fn request_construction_preserves_the_existing_group_input_type() {
    use leselang_command::reference_group_successor::ReservedEffectInput;
    let effect = Effect::UiFocus {
        node_id: "a".into(),
    };
    let input = ReservedEffectInput {
        effect: &effect,
        result_type: leserpent_hir::Type::UiFocus,
        sequence: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 30_000,
        deadline_at_ms: None,
        fuel_remaining: 2,
    };
    let request = leselang_command::reference_request_build::build_effect_request(input).unwrap();
    assert_eq!(request.continuation.fuel_remaining, 1);
}

#[test]
fn operation_lowering_composes_seventy_wire_shapes_with_lazy_identity_and_original_gui_owner() {
    use std::cell::Cell;
    struct Capture<'a>(&'a Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for (effect, expected) in request_admission_cases() {
        let calls = Cell::new(0);
        let drops = Cell::new(0);
        let capture = Capture(&drops);
        let principal = expected.authority().0.clone();
        let principal_buffer = principal.id.as_ptr();
        let capabilities = expected.authority().1.clone();
        let before = serde_json::to_vec(&effect).unwrap();
        let (required, operation) = leselang_command::lower_effect_operation(
            &effect,
            principal,
            capabilities,
            request_context(&effect).expected_revision,
            || {
                let _capture = capture;
                calls.set(calls.get() + 1);
                Some((
                    CommandId::new("command-a").unwrap(),
                    IdempotencyKey::new("effect-a").unwrap(),
                ))
            },
        )
        .unwrap();
        let gui = matches!(operation, EffectOperation::Presentation(_));
        assert_eq!(calls.get(), usize::from(!gui));
        assert_eq!(drops.get(), 1);
        if gui {
            assert_eq!(operation.authority().0.id.as_ptr(), principal_buffer);
        }
        assert_eq!(required, request_context(&effect).required_capability);
        assert_eq!(operation, expected);
        assert_eq!(
            serde_json::to_vec(&operation).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        assert_eq!(serde_json::to_vec(&effect).unwrap(), before);
        assert_eq!(operation.validate_request(request_context(&effect)), Ok(()));
    }
}

#[test]
fn operation_lowering_identity_refusal_precedes_control_plane_capability_checks() {
    use leselang_command::{OperationLoweringError, lower_effect_operation};
    for (effect, expected) in request_admission_cases() {
        if matches!(expected, EffectOperation::Presentation(_)) {
            continue;
        }
        let calls = std::cell::Cell::new(0);
        let error = lower_effect_operation(
            &effect,
            Principal {
                id: "private/invalid".into(),
            },
            CapabilitySet::new([] as [&str; 0]),
            None,
            || {
                calls.set(calls.get() + 1);
                None
            },
        )
        .unwrap_err();
        assert_eq!(error, OperationLoweringError::Identity);
        assert_eq!(calls.get(), 1);
        assert!(!format!("{error:?}: {error}").contains("private/invalid"));
        assert!(std::error::Error::source(&error).is_none());
        let error = lower_effect_operation(
            &effect,
            Principal {
                id: "private/invalid".into(),
            },
            CapabilitySet::new([] as [&str; 0]),
            None,
            || {
                Some((
                    CommandId::new("command-a").unwrap(),
                    IdempotencyKey::new("effect-a").unwrap(),
                ))
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            OperationLoweringError::Lowering(LoweringError::MissingCapability {
                capability: leserpent_hir::host_call::HostOperation::for_effect(&effect)
                    .unwrap()
                    .required_capability(),
            })
        );
    }
}

#[test]
fn operation_lowering_rejects_structured_effects_after_exactly_one_identity_attempt() {
    use leselang_command::{OperationLoweringError, lower_effect_operation};
    let compute = lower(&parse("fn main() = add(left: 1, right: 2)"))
        .unwrap()
        .function
        .effect;
    for effect in [
        Effect::All { branches: vec![] },
        Effect::Sequence { steps: vec![] },
        compute,
    ] {
        for grant_identity in [false, true] {
            let calls = std::cell::Cell::new(0);
            let outcome = lower_effect_operation(
                &effect,
                Principal::new("operator").unwrap(),
                CapabilitySet::default(),
                None,
                || {
                    calls.set(calls.get() + 1);
                    grant_identity.then(|| {
                        (
                            CommandId::new("command-a").unwrap(),
                            IdempotencyKey::new("effect-a").unwrap(),
                        )
                    })
                },
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(
                outcome,
                Err(if grant_identity {
                    OperationLoweringError::Lowering(
                        LoweringError::StructuredEffectRequiresExpansion,
                    )
                } else {
                    OperationLoweringError::Identity
                })
            );
        }
    }
}

#[test]
fn operation_lowering_gui_construction_is_not_authorization_or_request_admission() {
    for (effect, expected) in request_admission_cases() {
        if !matches!(expected, EffectOperation::Presentation(_)) {
            continue;
        }
        let (_, operation) = leselang_command::lower_effect_operation(
            &effect,
            Principal {
                id: "private/invalid".into(),
            },
            CapabilitySet::default(),
            None,
            || panic!("GUI must not request command identity"),
        )
        .unwrap();
        assert_eq!(operation.authority().0.id, "private/invalid");
        assert_eq!(
            operation.validate_request(request_context(&effect)),
            Err(leselang_command::OperationRequestError::Capability)
        );
    }
}

#[test]
fn operation_lowering_identity_factory_unwind_does_not_retry_or_retain_owned_capture() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    struct Capture<'a>(&'a Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let calls = Cell::new(0);
    let drops = Cell::new(0);
    let capture = Capture(&drops);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        leselang_command::lower_effect_operation(
            &Effect::RuntimeList {
                filter: Default::default(),
            },
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["runtime.read"]),
            None,
            || {
                let _capture = capture;
                calls.set(calls.get() + 1);
                panic!("trusted identity factory");
            },
        )
    }));
    assert!(outcome.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);
}

fn request_context(effect: &Effect) -> leselang_command::OperationRequestContext<'_> {
    leselang_command::OperationRequestContext {
        effect,
        required_capability: leserpent_hir::host_call::HostOperation::for_effect(effect)
            .unwrap()
            .required_capability(),
        expected_command_id: "command-a",
        expected_idempotency_key: "effect-a",
        expected_revision: Some(leselang_host_contract::Revision(7)),
    }
}

fn request_authority_mut(
    operation: &mut EffectOperation,
) -> (&mut Principal, &mut CapabilitySet, &mut u32) {
    match operation {
        EffectOperation::Query(e) => (&mut e.principal, &mut e.capabilities, &mut e.schema_version),
        EffectOperation::Command(e) => {
            (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
        }
        EffectOperation::Presentation(e) => {
            (&mut e.principal, &mut e.capabilities, &mut e.schema_version)
        }
    }
}

#[test]
fn operation_request_admission_covers_seventy_operations_without_vm_or_wire_changes() {
    for (effect, operation) in request_admission_cases() {
        let before = serde_json::to_vec(&operation).unwrap();
        let authority = operation.authority();
        let original = match &operation {
            EffectOperation::Query(e) => (&e.principal, &e.capabilities),
            EffectOperation::Command(e) => (&e.principal, &e.capabilities),
            EffectOperation::Presentation(e) => (&e.principal, &e.capabilities),
        };
        assert!(std::ptr::eq(authority.0, original.0));
        assert!(std::ptr::eq(authority.1, original.1));
        assert_eq!(operation.validate_request(request_context(&effect)), Ok(()));
        assert_eq!(serde_json::to_vec(&operation).unwrap(), before);
        let decoded: EffectOperation = serde_json::from_slice(&before).unwrap();
        assert_eq!(decoded.validate_request(request_context(&effect)), Ok(()));
        assert_eq!(decoded, operation);
    }
}

#[test]
fn operation_request_category_precedes_authority_and_authority_precedes_payload() {
    use leselang_command::OperationRequestError;
    let cases = request_admission_cases();
    for (effect, expected) in &cases {
        for (_, other) in &cases {
            let same_category = std::mem::discriminant(expected) == std::mem::discriminant(other);
            let mut torn = other.clone();
            let (principal, capabilities, schema) = request_authority_mut(&mut torn);
            principal.id = "private/invalid".into();
            *capabilities = CapabilitySet::new([] as [&str; 0]);
            *schema = u32::MAX;
            assert_eq!(
                torn.validate_request(request_context(effect)),
                Err(if same_category {
                    OperationRequestError::Capability
                } else {
                    OperationRequestError::Mismatch
                })
            );
            let (_, capabilities, _) = request_authority_mut(&mut torn);
            *capabilities = CapabilitySet::new([request_context(effect).required_capability]);
            assert_eq!(
                torn.validate_request(request_context(effect)),
                Err(if same_category {
                    OperationRequestError::Envelope
                } else {
                    OperationRequestError::Mismatch
                })
            );
        }
    }
}

#[test]
fn operation_request_identity_limits_and_diagnostics_remain_closed_and_redacted() {
    use leselang_command::OperationRequestError;
    for (effect, operation) in request_admission_cases() {
        for (id, valid) in [
            ("operator._-:09".into(), true),
            ("x".repeat(128), true),
            ("".into(), false),
            ("x".repeat(129), false),
            ("private/secret".into(), false),
            ("\u{00e9}".into(), false),
        ] {
            let mut torn = operation.clone();
            request_authority_mut(&mut torn).0.id = id;
            assert_eq!(
                torn.validate_request(request_context(&effect)),
                if valid {
                    Ok(())
                } else {
                    Err(OperationRequestError::Envelope)
                }
            );
        }
        for schema in [0, leserpent_domain::DOMAIN_SCHEMA_VERSION + 1, u32::MAX] {
            let mut torn = operation.clone();
            *request_authority_mut(&mut torn).2 = schema;
            assert_eq!(
                torn.validate_request(request_context(&effect)),
                Err(OperationRequestError::Envelope)
            );
        }
        let mut context = request_context(&effect);
        context.required_capability = "private/secret";
        let error = operation.validate_request(context).unwrap_err();
        assert_eq!(error, OperationRequestError::Capability);
        assert!(!format!("{error:?}: {error}").contains("private/secret"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn operation_request_uses_supplied_command_expectations_without_inventing_identity() {
    use leselang_command::OperationRequestError;
    for (effect, operation) in request_admission_cases() {
        for field in 0..3 {
            let mut context = request_context(&effect);
            match field {
                0 => context.expected_command_id = "another-command",
                1 => context.expected_idempotency_key = "another-key",
                _ => context.expected_revision = None,
            }
            assert_eq!(
                operation.validate_request(context),
                if matches!(operation, EffectOperation::Command(_)) {
                    Err(OperationRequestError::Mismatch)
                } else {
                    Ok(())
                }
            );
        }
    }
}

#[test]
fn operation_request_correspondence_is_not_source_or_live_authority_admission() {
    let filter = leselang_host_contract::RuntimeListFilter {
        environment: Some(String::new()),
        cluster: None,
        role: None,
    };
    let effect = Effect::RuntimeList {
        filter: filter.clone(),
    };
    assert!(leserpent_hir::canonical_source(&effect).is_err());
    let operation = EffectOperation::Query(leserpent_domain::QueryEnvelope {
        schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
        principal: Principal::new("declared-not-authenticated").unwrap(),
        capabilities: CapabilitySet::new(["runtime.read"]),
        query: leserpent_domain::Query::RuntimeList { filter },
    });
    assert_eq!(operation.validate_request(request_context(&effect)), Ok(()));
}

struct Case {
    operation: PresentationOperation,
    reply: PresentationResult,
    operation_wire: Value,
    reply_wire: Value,
}

// A closed wire matrix: adding a protocol variant must update both exhaustive matches.
macro_rules! protocol_cases {
    ($($variant:ident => $kind:literal {
        $($field:ident: ($native:expr, $wire:expr)),* $(,)?
    } $(reply { $($extra:ident: ($extra_native:expr, $extra_wire:expr)),* })?),* $(,)?) => {
        fn cases() -> Vec<Case> {
            vec![$(Case {
                operation: PresentationOperation::$variant { $($field: $native),* },
                reply: PresentationResult::$variant { $($field: $native,)* $($($extra: $extra_native),*)? },
                operation_wire: json!({"kind": $kind, $(stringify!($field): $wire),*}),
                reply_wire: json!({"kind": $kind, $(stringify!($field): $wire,)* $($(stringify!($extra): $extra_wire),*)?}),
            }),*]
        }
        fn operation_name(operation: &PresentationOperation) -> &'static str {
            match operation { $(PresentationOperation::$variant { .. } => $kind),* }
        }
        fn reply_name(reply: &PresentationResult) -> &'static str {
            match reply { $(PresentationResult::$variant { .. } => $kind),* }
        }
    };
}

protocol_cases! {
    Activate => "activate" {
        node_id: ("node-a".into(), "node-a")
    },
    Focus => "focus" {
        node_id: ("node-a".into(), "node-a")
    },
    NavigateFocus => "navigate_focus" {
        node_id: ("node-a".into(), "node-a"),
        direction: (UiFocusNavigationDirection::Next, "next")
    } reply { focused_node_id: ("child-a".into(), "child-a") },
    ScrollIntoView => "scroll_into_view" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertVisible => "assert_visible" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertHidden => "assert_hidden" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitHidden => "wait_hidden" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertRealized => "assert_realized" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitRealized => "wait_realized" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitVisible => "wait_visible" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitEnabled => "wait_enabled" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitDisabled => "wait_disabled" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    OpenWindow => "open_window" {
        node_id: ("node-a".into(), "node-a")
    },
    CloseWindow => "close_window" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertWindowOpen => "assert_window_open" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitWindowOpen => "wait_window_open" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertWindowClosed => "assert_window_closed" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitWindowClosed => "wait_window_closed" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitFocused => "wait_focused" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertFocused => "assert_focused" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitUnfocused => "wait_unfocused" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertUnfocused => "assert_unfocused" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertEnabled => "assert_enabled" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertDisabled => "assert_disabled" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertChildCount => "assert_child_count" {
        node_id: ("node-a".into(), "node-a"),
        count: (3, 3)
    },
    WaitChildCount => "wait_child_count" {
        node_id: ("node-a".into(), "node-a"),
        count: (3, 3),
        timeout_ms: (5000, 5000)
    },
    SetSelection => "set_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected")
    },
    AssertSelection => "assert_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected")
    },
    WaitSelection => "wait_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected"),
        timeout_ms: (5000, 5000)
    },
    AssertText => "assert_text" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitText => "wait_text" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAutomationId => "assert_automation_id" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAutomationId => "wait_automation_id" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertNodeKind => "assert_node_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticNodeKind::RuntimeCard, "runtime_card")
    },
    WaitNodeKind => "wait_node_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticNodeKind::RuntimeCard, "runtime_card"),
        timeout_ms: (5000, 5000)
    },
    AssertActionKind => "assert_action_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticActionKind::RuntimeInspect, "runtime_inspect")
    },
    WaitActionKind => "wait_action_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticActionKind::RuntimeInspect, "runtime_inspect"),
        timeout_ms: (5000, 5000)
    },
    AssertActionLabel => "assert_action_label" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitActionLabel => "wait_action_label" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertActionAvailable => "assert_action_available" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitActionAvailable => "wait_action_available" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertActionUnavailableReason => "assert_action_unavailable_reason" {
        node_id: ("node-a".into(), "node-a"),
        expected: (Some("ready".into()), "ready")
    },
    WaitActionUnavailableReason => "wait_action_unavailable_reason" {
        node_id: ("node-a".into(), "node-a"),
        expected: (Some("ready".into()), "ready"),
        timeout_ms: (5000, 5000)
    },
    SubmitForm => "submit_form" {
        node_id: ("node-a".into(), "node-a")
    },
    CancelForm => "cancel_form" {
        node_id: ("node-a".into(), "node-a")
    },
    SetFormValue => "set_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        value: ("ready".into(), "ready")
    },
    AssertFormValue => "assert_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready")
    },
    WaitFormValue => "wait_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertFormField => "assert_form_field" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready")
    },
    WaitFormField => "wait_form_field" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldInputKind => "assert_form_field_input_kind" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        input_kind: (UiFormInputKind::TrimmedText, "trimmed_text")
    },
    WaitFormFieldInputKind => "wait_form_field_input_kind" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        input_kind: (UiFormInputKind::TrimmedText, "trimmed_text"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldRequired => "assert_form_field_required" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        state: (UiFormRequirementState::Required, "required")
    },
    WaitFormFieldRequired => "wait_form_field_required" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        state: (UiFormRequirementState::Required, "required"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldMaxLength => "assert_form_field_max_length" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        max_length: (3, 3)
    },
    WaitFormFieldMaxLength => "wait_form_field_max_length" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        max_length: (3, 3),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldPlaceholder => "assert_form_field_placeholder" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: (Some("ready".into()), "ready")
    },
    WaitFormFieldPlaceholder => "wait_form_field_placeholder" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: (Some("ready".into()), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAccessibleName => "assert_accessible_name" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAccessibleName => "wait_accessible_name" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAccessibleDescription => "assert_accessible_description" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAccessibleDescription => "wait_accessible_description" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
}

#[test]
fn all_presentation_operations_and_replies_preserve_the_closed_wire_contract() {
    let cases = cases();
    assert_eq!(cases.len(), 62);
    for case in cases {
        assert_eq!(
            serde_json::to_value(&case.operation).unwrap(),
            case.operation_wire
        );
        assert_eq!(serde_json::to_value(&case.reply).unwrap(), case.reply_wire);
        assert_eq!(operation_name(&case.operation), reply_name(&case.reply));
        let operation: PresentationOperation = serde_json::from_value(case.operation_wire).unwrap();
        let reply: PresentationResult = serde_json::from_value(case.reply_wire).unwrap();
        assert_eq!(operation, case.operation);
        assert_eq!(reply, case.reply);
        assert_eq!(
            serde_json::to_vec(&operation).unwrap(),
            serde_json::to_vec(&case.operation).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&reply).unwrap(),
            serde_json::to_vec(&case.reply).unwrap()
        );
    }
}

#[test]
fn all_presentation_operations_and_replies_reject_unknown_fields_and_wrong_field_types() {
    for case in cases() {
        for (original, operation) in [(case.operation_wire, true), (case.reply_wire, false)] {
            let rejected = |wire: Value| {
                if operation {
                    assert!(serde_json::from_value::<PresentationOperation>(wire).is_err());
                } else {
                    assert!(serde_json::from_value::<PresentationResult>(wire).is_err());
                }
            };
            let mut extra = original.clone();
            extra["unexpected"] = json!("not-allowed");
            rejected(extra);
            for key in original.as_object().unwrap().keys() {
                let mut wrong = original.clone();
                wrong[key] = json!(false);
                rejected(wrong);
            }
            let mut missing = original.clone();
            missing.as_object_mut().unwrap().remove("node_id");
            rejected(missing);
        }
    }
}

#[test]
fn optional_text_and_focus_navigation_keep_their_distinct_reply_shapes() {
    for kind in [
        "assert_action_unavailable_reason",
        "wait_action_unavailable_reason",
        "assert_form_field_placeholder",
        "wait_form_field_placeholder",
    ] {
        let mut case = cases()
            .into_iter()
            .find(|case| operation_name(&case.operation) == kind)
            .unwrap();
        for wire in [&mut case.operation_wire, &mut case.reply_wire] {
            wire["expected"] = Value::Null;
        }
        let operation: PresentationOperation =
            serde_json::from_value(case.operation_wire.clone()).unwrap();
        let reply: PresentationResult = serde_json::from_value(case.reply_wire.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(operation).unwrap(),
            case.operation_wire
        );
        assert_eq!(serde_json::to_value(reply).unwrap(), case.reply_wire);
    }
    let case = cases()
        .into_iter()
        .find(|case| operation_name(&case.operation) == "navigate_focus")
        .unwrap();
    assert!(serde_json::from_value::<PresentationResult>(case.operation_wire).is_err());
    assert!(serde_json::from_value::<PresentationOperation>(case.reply_wire).is_err());
}

fn effect_from_wire(operation: &Value) -> Effect {
    let mut effect = operation.clone();
    effect["kind"] = json!(format!("ui_{}", operation["kind"].as_str().unwrap()));
    effect.as_object_mut().unwrap().remove("timeout_ms");
    serde_json::from_value(effect).unwrap()
}

#[test]
fn all_presentation_effects_map_without_a_vm_and_keep_the_fixed_wait_policy() {
    let context = LoweringContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        command_id: CommandId::new("command-a").unwrap(),
        idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
        origin: CommandOrigin::Leselang,
        confirmation: Confirmation::NotRequired,
        dry_run: false,
    };
    for case in cases() {
        let mut expected = case.operation_wire;
        for (field, value) in [
            ("node_id", "node-a"),
            ("field", "field-a"),
            ("expected", "expected-a"),
            ("value", "value-a"),
        ] {
            if expected.get(field).is_some() {
                expected[field] = json!(value);
            }
        }
        let effect = effect_from_wire(&expected);
        let before = serde_json::to_vec(&effect).unwrap();
        if expected.get("timeout_ms").is_some() {
            expected["timeout_ms"] = json!(2000);
        }
        let mapped = PresentationOperation::from_effect(&effect).unwrap();
        assert_eq!(serde_json::to_value(&mapped).unwrap(), expected);
        assert_eq!(serde_json::to_vec(&effect).unwrap(), before);
        assert_eq!(PresentationOperation::from_effect(&effect), Some(mapped));
        // Moving the mapper does not admit GUI operations to the daemon command path.
        assert_eq!(
            lower_effect(&effect, &context),
            Err(LoweringError::FrontendLocalEffect)
        );
    }
}

#[test]
fn presentation_mapping_preserves_absent_optional_text_without_default_substitution() {
    for case in cases() {
        if !matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        ) {
            continue;
        }
        for expected in [Value::Null, json!(""), json!("ready")] {
            let mut wire = case.operation_wire.clone();
            wire["expected"] = expected;
            let effect = effect_from_wire(&wire);
            if wire.get("timeout_ms").is_some() {
                wire["timeout_ms"] = json!(2000);
            }
            assert_eq!(
                serde_json::to_value(PresentationOperation::from_effect(&effect).unwrap()).unwrap(),
                wire
            );
        }
    }
}

#[test]
fn presentation_mapping_does_not_expand_computation_groups_or_control_plane_effects() {
    let sources = [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
        r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        r#"all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        "add(left: 1, right: 2)",
    ];
    for source in sources {
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        assert_eq!(
            PresentationOperation::from_effect(&effect),
            None,
            "{source}"
        );
        for case in reply_cases() {
            assert!(!case.operation.matches_effect(&effect), "{source}");
        }
    }
}

#[test]
fn presentation_mapping_is_data_conversion_not_a_validation_certificate() {
    let effect = Effect::UiFocus {
        node_id: "bad node".into(),
    };
    assert_eq!(
        PresentationOperation::from_effect(&effect),
        Some(PresentationOperation::Focus {
            node_id: "bad node".into(),
        })
    );
    assert!(!leserpent_hir::validate_ui_node_id("bad node"));
}

fn reply_cases() -> Vec<Case> {
    cases()
        .into_iter()
        .map(|mut case| {
            for wire in [&mut case.operation_wire, &mut case.reply_wire] {
                if wire.get("timeout_ms").is_some() {
                    wire["timeout_ms"] = json!(2000);
                }
                for (field, value) in [
                    ("field", "field-a"),
                    ("expected", "expected-a"),
                    ("value", "value-a"),
                ] {
                    if wire.get(field).is_some() {
                        wire[field] = json!(value);
                    }
                }
            }
            case.operation = serde_json::from_value(case.operation_wire.clone()).unwrap();
            case.reply = serde_json::from_value(case.reply_wire.clone()).unwrap();
            case
        })
        .collect()
}

fn envelope(operation: PresentationOperation) -> EffectOperation {
    EffectOperation::Presentation(PresentationEnvelope {
        schema_version: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        operation,
    })
}

#[test]
fn all_presentation_replies_project_to_the_released_value_wire_and_type() {
    for case in reply_cases() {
        let mut expected = case.reply_wire;
        let kind = format!("ui_{}", expected["kind"].as_str().unwrap());
        expected["kind"] = json!(kind);
        expected.as_object_mut().unwrap().remove("timeout_ms");
        let value = leselang_command::Value::from(case.reply);
        assert_eq!(
            serde_json::to_value(value.result_type()).unwrap(),
            json!(kind)
        );
        assert_eq!(serde_json::to_value(&value).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<leselang_command::Value>(expected).unwrap(),
            value
        );
    }
}

#[test]
fn presentation_value_projection_preserves_optional_text_and_owned_buffers() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        )
    }) {
        for expected in [Value::Null, json!(""), json!("present")] {
            let mut wire = case.reply_wire.clone();
            wire["expected"] = expected.clone();
            let reply: PresentationResult = serde_json::from_value(wire).unwrap();
            let value = leselang_command::Value::from(reply);
            assert_eq!(serde_json::to_value(&value).unwrap()["expected"], expected);
        }
    }
    let (node_id, field, value) = (
        String::from("node-a"),
        String::from("field-a"),
        String::from("value-a"),
    );
    let pointers = (node_id.as_ptr(), field.as_ptr(), value.as_ptr());
    let projected = leselang_command::Value::from(PresentationResult::SetFormValue {
        node_id,
        field,
        value,
    });
    let leselang_command::Value::UiSetFormValue {
        node_id,
        field,
        value,
    } = projected
    else {
        panic!()
    };
    assert_eq!((node_id.as_ptr(), field.as_ptr(), value.as_ptr()), pointers);
}

#[test]
fn presentation_value_conversion_is_not_validation_or_admission() {
    let reply = PresentationResult::WaitHidden {
        node_id: "bad node".into(),
        timeout_ms: 1,
    };
    assert!(!reply.matches_effect(
        &Effect::UiWaitHidden {
            node_id: "bad node".into()
        },
        None
    ));
    let value = leselang_command::Value::from(reply);
    assert_eq!(value.result_type(), leserpent_hir::Type::UiWaitHidden);
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"kind":"ui_wait_hidden","node_id":"bad node"})
    );
}

#[test]
fn product_value_types_include_scalars_domain_results_and_ordered_structures() {
    use leselang_command::{DebuggerCancelResult, StructuredField, Value as ProductValue};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{CommandResult, CommandStatus, RefreshStatus, RuntimeProjection};
    use leserpent_hir::{
        Type,
        computation::{OptionalStringValue, ScalarType, ScalarValue, StringListValue},
    };

    let runtime = RuntimeProjection {
        id: RuntimeId::new("runtime-a").unwrap(),
        name: "runtime-a".into(),
        endpoint: "http://127.0.0.1:8080".into(),
        sidecar_endpoint: None,
        registered_at_unix_ms: None,
        updated_at_unix_ms: None,
        revision: Revision(7),
        refresh_count: 0,
        refresh_status: RefreshStatus::NeverRequested,
        tags: Default::default(),
        status: Default::default(),
        sidecar_status: None,
        capabilities: Default::default(),
        capabilities_observed_for_revision: None,
    };
    let command = CommandResult {
        command_id: CommandId::new("command-a").unwrap(),
        status: CommandStatus::Applied,
        runtime: runtime.clone(),
        events: vec![],
    };
    for count in [0, 1, 2] {
        for value in [
            ProductValue::RuntimeList {
                revision: Revision(7),
                runtimes: vec![runtime.clone(); count],
            },
            ProductValue::RuntimeHistory {
                revision: Revision(7),
                entries: vec![command.clone(); count],
            },
        ] {
            assert_eq!(value.bounded_leaf_items(1), (count <= 1).then_some(count));
            assert_eq!(value.bounded_leaf_items(0), (count == 0).then_some(0));
            assert_eq!(value.bounded_leaf_items(2), Some(count));
        }
    }
    let cases = [
        (
            ProductValue::RuntimeList {
                revision: Revision(7),
                runtimes: vec![runtime.clone()],
            },
            Type::RuntimeList,
        ),
        (
            ProductValue::RuntimeInspect {
                revision: Revision(7),
                runtime: Box::new(runtime),
            },
            Type::RuntimeInspect,
        ),
        (
            ProductValue::RuntimeHistory {
                revision: Revision(7),
                entries: vec![command.clone()],
            },
            Type::RuntimeHistory,
        ),
        (
            ProductValue::RuntimeLogs {
                revision: Revision(7),
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                runtime_name: "runtime-a".into(),
                entries: vec![],
            },
            Type::RuntimeLogs,
        ),
        (
            ProductValue::RuntimeRefresh {
                result: Box::new(command.clone()),
            },
            Type::RuntimeRefresh,
        ),
        (
            ProductValue::RuntimeCapabilitiesRefresh {
                result: Box::new(command.clone()),
            },
            Type::RuntimeCapabilitiesRefresh,
        ),
        (
            ProductValue::RuntimeDeploy {
                result: Box::new(command),
            },
            Type::RuntimeDeploy,
        ),
        (
            ProductValue::DebuggerCancel {
                result: DebuggerCancelResult {
                    command_id: CommandId::new("command-a").unwrap(),
                    session_id: "session-a".into(),
                    observed_at_ms: 9,
                },
            },
            Type::DebuggerCancel,
        ),
        (
            ProductValue::Structured {
                fields: vec![
                    StructuredField {
                        name: "z".into(),
                        value: Box::new(ProductValue::UiFocus {
                            node_id: "a".into(),
                        }),
                    },
                    StructuredField {
                        name: "a".into(),
                        value: Box::new(ProductValue::Scalar {
                            value: ScalarValue::Integer(7),
                        }),
                    },
                ],
            },
            Type::Structured,
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(value.result_type(), expected);
        let count = match expected {
            Type::Structured => None,
            Type::RuntimeLogs => Some(0),
            _ => Some(1),
        };
        assert_eq!(value.bounded_leaf_items(1), count);
        let wire = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            serde_json::from_slice::<ProductValue>(&wire).unwrap(),
            value
        );
    }
    for (value, expected) in [
        (ScalarValue::Integer(u64::MAX), ScalarType::Integer),
        (ScalarValue::Boolean(false), ScalarType::Boolean),
        (ScalarValue::String("text".into()), ScalarType::String),
        (ScalarValue::None, ScalarType::None),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarType::OptionalString,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["z".into(), "a".into()])),
            ScalarType::StringList,
        ),
    ] {
        assert_eq!(
            ProductValue::Scalar { value }.result_type(),
            Type::Scalar(expected)
        );
    }
    let wire = r#"{"kind":"structured","fields":[{"name":"z","value":{"kind":"ui_focus","node_id":"a"}},{"name":"a","value":{"kind":"scalar","value":{"kind":"integer","value":7}}}]}"#;
    let value: ProductValue = serde_json::from_str(wire).unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), wire);
}

#[test]
fn all_presentation_payload_and_leaf_bounds_preserve_identifiers_text_and_counts() {
    use leserpent_hir::{
        MAX_UI_CHILD_COUNT, MAX_UI_EXPECTED_TEXT_BYTES, MAX_UI_FORM_FIELD_KEY_BYTES,
        MAX_UI_FORM_FIELD_MAX_LENGTH, MAX_UI_NODE_ID_BYTES,
    };

    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for case in cases {
        let value = leselang_command::Value::from(case.reply);
        let wire = serde_json::to_value(&value).unwrap();
        let check = |field: &str, candidate: Value, expected| {
            if case.operation_wire.get(field).is_some() {
                let mut operation_wire = case.operation_wire.clone();
                operation_wire[field] = candidate.clone();
                let operation: PresentationOperation =
                    serde_json::from_value(operation_wire.clone()).unwrap();
                let effect = effect_from_wire(&operation_wire);
                assert_eq!(
                    operation.matches_effect(&effect),
                    expected == Some(1),
                    "{operation_wire}"
                );
                assert_eq!(serde_json::to_value(&operation).unwrap(), operation_wire);
            }
            let mut changed = wire.clone();
            changed[field] = candidate;
            let value: leselang_command::Value = serde_json::from_value(changed.clone()).unwrap();
            // Query ceiling zero is not the separate per-execution output budget.
            assert_eq!(value.bounded_leaf_items(0), expected, "{changed}");
            assert_eq!(serde_json::to_value(&value).unwrap(), changed);
        };
        assert_eq!(value.bounded_leaf_items(0), Some(1), "{wire}");
        for (field, max) in [
            ("node_id", MAX_UI_NODE_ID_BYTES),
            ("focused_node_id", MAX_UI_NODE_ID_BYTES),
            ("field", MAX_UI_FORM_FIELD_KEY_BYTES),
        ] {
            if wire.get(field).is_none() {
                continue;
            }
            check(field, json!("a".repeat(max)), Some(1));
            for invalid in [
                "".into(),
                "bad id".into(),
                "a\n".into(),
                "a".repeat(max + 1),
            ] {
                check(field, json!(invalid), None);
            }
            check(field, json!("a.b_c-d"), Some(1));
            check(field, json!("a:b"), (field != "field").then_some(1));
        }
        for field in ["expected", "value"] {
            if wire.get(field).is_none() {
                continue;
            }
            let name = operation_name(&case.operation);
            let automation_id = name.ends_with("automation_id");
            let max = if automation_id {
                MAX_UI_NODE_ID_BYTES
            } else if name.ends_with("form_value") {
                MAX_UI_FORM_FIELD_MAX_LENGTH
            } else {
                MAX_UI_EXPECTED_TEXT_BYTES
            };
            check(field, json!("a".repeat(max)), Some(1));
            check(field, json!("a".repeat(max + 1)), None);
            check(field, json!(""), (!automation_id).then_some(1));
            for invalid in ["\n", "\0", "\u{85}"] {
                check(field, json!(invalid), None);
            }
            if !automation_id {
                check(field, json!("\u{e9}".repeat(max / 2)), Some(1));
                check(field, json!("\u{e9}".repeat(max / 2 + 1)), None);
            }
            if name.ends_with("unavailable_reason") || name.ends_with("placeholder") {
                check(field, Value::Null, Some(1));
            }
        }
        if wire.get("count").is_some() {
            check("count", json!(MAX_UI_CHILD_COUNT), Some(1));
            check("count", json!(MAX_UI_CHILD_COUNT + 1), None);
        }
        if wire.get("max_length").is_some() {
            check("max_length", json!(usize::MAX), Some(1));
        }
        if wire.get("focused_node_id").is_some() {
            check("focused_node_id", wire["node_id"].clone(), Some(1));
        }
    }
}

#[test]
fn product_scalar_leaf_bounds_preserve_empty_lists_and_byte_budgets() {
    use leselang_command::Value as ProductValue;
    use leserpent_hir::computation::{
        MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarValue,
        StringListValue,
    };

    let check = |value, expected| {
        let value = ProductValue::Scalar { value };
        let before = serde_json::to_vec(&value).unwrap();
        assert_eq!(value.bounded_leaf_items(0), expected);
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    };
    for value in [
        ScalarValue::None,
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
    ] {
        check(value, Some(1));
    }
    check(
        ScalarValue::OptionalString(OptionalStringValue(None)),
        Some(1),
    );
    for bytes in [0, MAX_SCALAR_STRING_BYTES, MAX_SCALAR_STRING_BYTES + 1] {
        let expected = (bytes <= MAX_SCALAR_STRING_BYTES).then_some(1);
        check(ScalarValue::String("a".repeat(bytes)), expected);
        check(
            ScalarValue::OptionalString(OptionalStringValue(Some("a".repeat(bytes)))),
            expected,
        );
    }
    for count in [0, 1, MAX_STRING_LIST_ITEMS, MAX_STRING_LIST_ITEMS + 1] {
        check(
            ScalarValue::StringList(StringListValue(vec![String::new(); count])),
            (count <= MAX_STRING_LIST_ITEMS).then_some(count.max(1)),
        );
    }
    let part = "\u{e9}".repeat(MAX_SCALAR_STRING_BYTES / 4);
    check(
        ScalarValue::StringList(StringListValue(vec![part.clone(), part.clone()])),
        Some(2),
    );
    check(
        ScalarValue::StringList(StringListValue(vec![part.clone(), format!("{part}a")])),
        None,
    );
    // Scalar text is not presentation text: control characters retain their old semantics.
    check(ScalarValue::String("\n\0".into()), Some(1));
    assert_eq!(
        ProductValue::Structured { fields: vec![] }.bounded_leaf_items(1),
        None
    );
}

#[test]
fn product_log_and_debugger_leaf_bounds_keep_their_distinct_limits() {
    use leselang_command::{DebuggerCancelResult, Value as ProductValue};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{MAX_RUNTIME_LOG_QUERY_ENTRIES, RuntimeLogLevel, RuntimeLogRecord};

    let max = usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES);
    for count in [0, 1, max, max + 1] {
        let value = ProductValue::RuntimeLogs {
            revision: Revision(7),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: String::new(),
            entries: vec![
                RuntimeLogRecord {
                    sequence: 0,
                    level: RuntimeLogLevel::Info,
                    message: String::new()
                };
                count
            ],
        };
        for query_ceiling in [0, 1, usize::MAX] {
            assert_eq!(
                value.bounded_leaf_items(query_ceiling),
                (count <= max).then_some(count)
            );
        }
    }
    for (session_id, observed_at_ms, expected) in [
        ("session-a", 0, Some(1)),
        ("session-a", i64::MAX as u64, Some(1)),
        ("session-a", i64::MAX as u64 + 1, None),
        ("", 0, None),
        ("bad session", 0, None),
    ] {
        let value = ProductValue::DebuggerCancel {
            result: DebuggerCancelResult {
                command_id: CommandId::new("command-a").unwrap(),
                session_id: session_id.into(),
                observed_at_ms,
            },
        };
        assert_eq!(value.bounded_leaf_items(0), expected);
    }
}

#[test]
fn control_plane_requests_match_only_their_original_effect_and_retain_bytes() {
    let cases = control_plane_cases();
    assert_eq!(cases.len(), 8);
    for (index, (effect, operation)) in cases.iter().enumerate() {
        let before = serde_json::to_vec(&(effect, operation)).unwrap();
        for (other_index, (_, other)) in cases.iter().enumerate() {
            assert_eq!(control_plane_matches(other, effect), index == other_index);
        }
        assert_eq!(serde_json::to_vec(&(effect, operation)).unwrap(), before);
        for gui in reply_cases() {
            assert!(!control_plane_matches(
                operation,
                &effect_from_wire(&gui.operation_wire)
            ));
        }
        for source in [
            r#"seq(first: runtime.list(), second: runtime.list())"#,
            r#"all(first: runtime.list(), second: runtime.list())"#,
            "add(left: 1, right: 2)",
        ] {
            let effect = lower(&parse(&format!("fn main() = {source}")))
                .unwrap()
                .function
                .effect;
            assert!(!control_plane_matches(operation, &effect));
        }
    }
}

fn control_plane_reply_cases() -> Vec<(Effect, EffectOperation, leselang_command::EffectResult)> {
    use leselang_command::{DebuggerCancelResult, EffectResult, PlannedOperation};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{
        CommandResult, CommandStatus, QueryResult, RefreshStatus, RuntimeProjection,
    };
    let runtime = RuntimeProjection {
        id: RuntimeId::new("runtime-a").unwrap(),
        name: "runtime-a".into(),
        endpoint: "http://127.0.0.1:8080".into(),
        sidecar_endpoint: None,
        registered_at_unix_ms: None,
        updated_at_unix_ms: None,
        revision: Revision(7),
        refresh_count: 0,
        refresh_status: RefreshStatus::NeverRequested,
        tags: Default::default(),
        status: Default::default(),
        sidecar_status: None,
        capabilities: Default::default(),
        capabilities_observed_for_revision: None,
    };
    let command = CommandResult {
        command_id: CommandId::new("command-a").unwrap(),
        status: CommandStatus::Applied,
        runtime: runtime.clone(),
        events: vec![],
    };
    control_plane_cases()
        .into_iter()
        .enumerate()
        .map(|(index, (effect, operation))| {
            let operation = match operation {
                PlannedOperation::Query(query) => EffectOperation::Query(query),
                PlannedOperation::Command(command) => EffectOperation::Command(command),
            };
            let reply = match index {
                0 => EffectResult::Query(QueryResult::RuntimeList {
                    revision: Revision(7),
                    runtimes: vec![runtime.clone()],
                }),
                1 => EffectResult::Query(QueryResult::RuntimeInspect {
                    revision: Revision(7),
                    runtime: runtime.clone(),
                }),
                2 => EffectResult::Query(QueryResult::RuntimeHistory {
                    revision: Revision(7),
                    entries: vec![command.clone()],
                }),
                3 => EffectResult::Query(QueryResult::RuntimeLogs {
                    revision: Revision(7),
                    runtime_id: runtime.id.clone(),
                    runtime_name: "runtime-a".into(),
                    entries: vec![],
                }),
                4..=6 => EffectResult::Command(Box::new(command.clone())),
                7 => EffectResult::DebuggerCancel(DebuggerCancelResult {
                    command_id: command.command_id.clone(),
                    session_id: "session-a".into(),
                    observed_at_ms: 11,
                }),
                _ => unreachable!(),
            };
            (effect, operation, reply)
        })
        .collect()
}

#[test]
fn composed_effect_values_cover_seventy_operations_types_and_legacy_context() {
    use leselang_command::{
        EffectResult, EffectValueContext, EffectValueError, Value as ProductValue,
    };
    use leselang_host_contract::Revision;
    use leserpent_hir::{Type, host_call::HostOperation};

    let mut cases = control_plane_reply_cases();
    cases.extend(reply_cases().into_iter().map(|case| {
        (
            effect_from_wire(&case.operation_wire),
            envelope(case.operation),
            EffectResult::Presentation(case.reply),
        )
    }));
    assert_eq!(cases.len(), 70);
    let mut types = cases
        .iter()
        .map(|(effect, _, _)| HostOperation::for_effect(effect).unwrap().result_type())
        .collect::<Vec<_>>();
    types.push(Type::Structured);
    for (index, (effect, operation, reply)) in cases.iter().enumerate() {
        let ty = HostOperation::for_effect(effect).unwrap().result_type();
        let expected = match reply {
            EffectResult::Presentation(reply) => ProductValue::from(reply.clone()),
            reply => reply.clone().into_control_plane_value(ty).unwrap(),
        };
        let before = serde_json::to_vec(&(effect, operation)).unwrap();
        for saved in [None, Some(operation)] {
            for declared in &types {
                let context = EffectValueContext {
                    effect,
                    result_type: *declared,
                    operation: saved,
                    expected_revision: Some(Revision(7)),
                    max_output_items: 1000,
                };
                let actual = reply.clone().into_effect_value(context);
                if *declared == ty && !(saved.is_none() && (4..8).contains(&index)) {
                    assert_eq!(actual, Ok(expected.clone()));
                    let restored: EffectResult =
                        serde_json::from_slice(&serde_json::to_vec(reply).unwrap()).unwrap();
                    assert_eq!(restored.into_effect_value(context), actual);
                } else {
                    assert_eq!(actual, Err(EffectValueError::Mismatch));
                }
            }
        }
        assert_eq!(serde_json::to_vec(&(effect, operation)).unwrap(), before);
    }
}

#[test]
fn composed_effect_values_preserve_query_revision_count_and_mismatch_precedence() {
    use leselang_command::{EffectResult, EffectValueContext, EffectValueError};
    use leselang_host_contract::Revision;
    use leserpent_domain::{QueryResult, RuntimeLogLevel, RuntimeLogRecord};
    use leserpent_hir::{Type, host_call::HostOperation};

    for (index, (effect, operation, reply)) in
        control_plane_reply_cases().into_iter().take(4).enumerate()
    {
        for expected in [None, Some(Revision(7))] {
            for revision in [Revision(7), Revision(8)] {
                for limit in [0, 1, 1000] {
                    for count in [0, 1, 256, 257] {
                        let mut reply = reply.clone();
                        match &mut reply {
                            EffectResult::Query(QueryResult::RuntimeList {
                                revision: actual,
                                runtimes,
                            }) => {
                                *actual = revision;
                                runtimes.resize(count, runtimes[0].clone());
                            }
                            EffectResult::Query(QueryResult::RuntimeInspect {
                                revision: actual,
                                ..
                            }) => *actual = revision,
                            EffectResult::Query(QueryResult::RuntimeHistory {
                                revision: actual,
                                entries,
                            }) => {
                                *actual = revision;
                                entries.resize(count, entries[0].clone());
                            }
                            EffectResult::Query(QueryResult::RuntimeLogs {
                                revision: actual,
                                entries,
                                ..
                            }) => {
                                *actual = revision;
                                entries.resize(
                                    count,
                                    RuntimeLogRecord {
                                        sequence: 1,
                                        level: RuntimeLogLevel::Info,
                                        message: "private-payload".into(),
                                    },
                                );
                            }
                            _ => panic!(),
                        }
                        let context = EffectValueContext {
                            effect: &effect,
                            result_type: HostOperation::for_effect(&effect).unwrap().result_type(),
                            operation: Some(&operation),
                            expected_revision: expected,
                            max_output_items: limit,
                        };
                        assert_eq!(
                            reply.clone().into_effect_value(EffectValueContext {
                                result_type: Type::Structured,
                                ..context
                            }),
                            Err(EffectValueError::Mismatch)
                        );
                        let actual = reply.into_effect_value(context);
                        let effective_limit = if index == 3 { limit.min(256) } else { limit };
                        if expected.is_some_and(|expected| expected != revision) {
                            assert_eq!(
                                actual,
                                Err(EffectValueError::RevisionConflict {
                                    expected: expected.unwrap(),
                                    actual: revision
                                })
                            );
                        } else if index != 1 && count > effective_limit {
                            assert_eq!(
                                actual,
                                Err(EffectValueError::OutputLimit {
                                    count,
                                    limit: effective_limit
                                })
                            );
                        } else {
                            assert_eq!(actual.unwrap().result_type(), context.result_type);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn composed_effect_values_move_payloads_without_granting_admission() {
    use leselang_command::{EffectResult, EffectValueContext, Value as ProductValue};
    use leselang_host_contract::Revision;
    use leserpent_domain::QueryResult;
    use leserpent_hir::{Type, host_call::HostOperation};

    for (effect, operation, reply) in control_plane_reply_cases() {
        let pointer = match &reply {
            EffectResult::Query(QueryResult::RuntimeList { runtimes, .. }) => {
                runtimes.as_ptr().cast::<()>()
            }
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => {
                runtime.name.as_ptr().cast::<()>()
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries.as_ptr().cast::<()>()
            }
            EffectResult::Query(QueryResult::RuntimeLogs { runtime_name, .. }) => {
                runtime_name.as_ptr().cast::<()>()
            }
            EffectResult::Command(result) => std::ptr::from_ref(result.as_ref()).cast::<()>(),
            EffectResult::DebuggerCancel(result) => result.session_id.as_ptr().cast::<()>(),
            _ => panic!(),
        };
        let value = reply
            .into_effect_value(EffectValueContext {
                effect: &effect,
                result_type: HostOperation::for_effect(&effect).unwrap().result_type(),
                operation: Some(&operation),
                expected_revision: Some(Revision(7)),
                max_output_items: 1000,
            })
            .unwrap();
        let after = match &value {
            ProductValue::RuntimeList { runtimes, .. } => runtimes.as_ptr().cast::<()>(),
            ProductValue::RuntimeInspect { runtime, .. } => runtime.name.as_ptr().cast::<()>(),
            ProductValue::RuntimeHistory { entries, .. } => entries.as_ptr().cast::<()>(),
            ProductValue::RuntimeLogs { runtime_name, .. } => runtime_name.as_ptr().cast::<()>(),
            ProductValue::RuntimeRefresh { result }
            | ProductValue::RuntimeCapabilitiesRefresh { result }
            | ProductValue::RuntimeDeploy { result } => {
                std::ptr::from_ref(result.as_ref()).cast::<()>()
            }
            ProductValue::DebuggerCancel { result } => result.session_id.as_ptr().cast::<()>(),
            _ => panic!(),
        };
        assert_eq!(pointer, after);
    }
    let effect = Effect::UiFocus {
        node_id: "private-node".into(),
    };
    let mut operation = envelope(PresentationOperation::Focus {
        node_id: "private-node".into(),
    });
    let EffectOperation::Presentation(saved) = &mut operation else {
        panic!()
    };
    saved.schema_version = 999;
    saved.principal.id = "invalid principal".into();
    saved.capabilities = CapabilitySet::new([] as [&str; 0]);
    let node_id = String::from("private-node");
    let pointer = node_id.as_ptr();
    let context = EffectValueContext {
        effect: &effect,
        result_type: Type::UiFocus,
        operation: Some(&operation),
        expected_revision: Some(Revision(99)),
        max_output_items: 0,
    };
    assert_eq!(format!("{context:?}"), "EffectValueContext");
    // Neither this conversion nor legacy GUI correspondence authenticates the envelope.
    let value = EffectResult::Presentation(PresentationResult::Focus { node_id })
        .into_effect_value(context)
        .unwrap();
    let ProductValue::UiFocus { node_id } = value else {
        panic!()
    };
    assert_eq!(pointer, node_id.as_ptr());
}

#[test]
fn control_plane_replies_match_original_ids_without_copying_or_consuming_payloads() {
    let cases = control_plane_reply_cases();
    for (index, (effect, operation, reply)) in cases.iter().enumerate() {
        let before = serde_json::to_vec(reply).unwrap();
        for (other_index, (_, _, other)) in cases.iter().enumerate() {
            // Command replies have no operation tag; this fixture shares the same saved ID.
            let expected = index == other_index
                || ((4..=6).contains(&index) && (4..=6).contains(&other_index));
            assert_eq!(
                other.matches_control_plane_effect(effect, Some(operation)),
                expected
            );
        }
        assert_eq!(reply.matches_control_plane_effect(effect, None), index < 4);
        let wrong_category = EffectOperation::Presentation(PresentationEnvelope {
            schema_version: 1,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::default(),
            operation: PresentationOperation::Focus {
                node_id: "node-a".into(),
            },
        });
        assert_eq!(
            reply.matches_control_plane_effect(effect, Some(&wrong_category)),
            index < 4
        );
        assert_eq!(serde_json::to_vec(reply).unwrap(), before);
        let decoded: leselang_command::EffectResult = serde_json::from_slice(&before).unwrap();
        assert!(decoded.matches_control_plane_effect(effect, Some(operation)));
    }
}

#[test]
fn control_plane_replies_reject_torn_ids_statuses_and_out_of_range_cancel_time() {
    use leselang_command::EffectResult;
    use leselang_host_contract::RuntimeId;
    use leserpent_domain::{CommandStatus, QueryResult};
    for (effect, operation, reply) in control_plane_reply_cases() {
        let mut torn = reply.clone();
        match &mut torn {
            EffectResult::Query(QueryResult::RuntimeList { .. }) => continue,
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => {
                runtime.id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries.push(entries[0].clone());
                entries[1].runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeLogs { runtime_id, .. }) => {
                *runtime_id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Command(result) => result.runtime.id = RuntimeId::new("other").unwrap(),
            EffectResult::DebuggerCancel(result) => result.session_id = "other".into(),
            EffectResult::Presentation(_) => unreachable!(),
        }
        assert!(!torn.matches_control_plane_effect(&effect, Some(&operation)));
        if let EffectResult::Command(result) = &reply {
            let mut changed = result.clone();
            changed.status = CommandStatus::Planned;
            assert!(
                !EffectResult::Command(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
            let mut changed = result.clone();
            changed.command_id = CommandId::new("other").unwrap();
            assert!(
                !EffectResult::Command(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
        if let EffectResult::DebuggerCancel(result) = &reply {
            for (timestamp, expected) in [
                (0, true),
                (i64::MAX as u64, true),
                (i64::MAX as u64 + 1, false),
                (u64::MAX, false),
            ] {
                let mut changed = result.clone();
                changed.observed_at_ms = timestamp;
                assert_eq!(
                    EffectResult::DebuggerCancel(changed)
                        .matches_control_plane_effect(&effect, Some(&operation)),
                    expected
                );
            }
            let mut changed = result.clone();
            changed.command_id = CommandId::new("other").unwrap();
            assert!(
                !EffectResult::DebuggerCancel(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
    }
}

#[test]
fn control_plane_reply_correspondence_is_not_authority_revision_bounds_or_domain_validation() {
    use leselang_command::EffectResult;
    use leselang_host_contract::Revision;
    use leserpent_domain::QueryResult;
    for (effect, mut operation, mut reply) in control_plane_reply_cases() {
        match &mut operation {
            EffectOperation::Command(command) => {
                command.schema_version = 99;
                command.principal.id.clear();
                command.capabilities = CapabilitySet::default();
                command.dry_run = true;
                command.confirmation = Confirmation::NotRequired;
                command.expected_revision = Some(Revision(999));
                assert!(reply.matches_control_plane_effect(&effect, Some(&operation)));
            }
            EffectOperation::Query(_) => {
                match &mut reply {
                    EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                        *revision = Revision(999);
                        runtimes[0].name.clear();
                        runtimes.resize(10_001, runtimes[0].clone());
                    }
                    EffectResult::Query(QueryResult::RuntimeInspect { revision, runtime }) => {
                        *revision = Revision(999);
                        runtime.name.clear();
                    }
                    EffectResult::Query(QueryResult::RuntimeHistory { revision, entries }) => {
                        *revision = Revision(999);
                        entries.clear();
                    }
                    EffectResult::Query(QueryResult::RuntimeLogs {
                        revision,
                        runtime_name,
                        entries,
                        ..
                    }) => {
                        *revision = Revision(999);
                        runtime_name.clear();
                        entries.resize(
                            257,
                            leserpent_domain::RuntimeLogRecord {
                                sequence: 0,
                                level: leserpent_domain::RuntimeLogLevel::Info,
                                message: String::new(),
                            },
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(reply.matches_control_plane_effect(&effect, None));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn control_plane_reply_matcher_rejects_all_gui_and_structured_operations() {
    for (effect, operation, reply) in control_plane_reply_cases() {
        for gui in reply_cases() {
            let gui_effect = effect_from_wire(&gui.operation_wire);
            assert!(!reply.matches_control_plane_effect(&gui_effect, Some(&operation)));
            assert!(
                !leselang_command::EffectResult::Presentation(gui.reply)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
        for non_atomic in [
            Effect::All { branches: vec![] },
            Effect::Sequence { steps: vec![] },
            lower(&parse("fn main() = 1")).unwrap().function.effect,
        ] {
            assert!(!reply.matches_control_plane_effect(&non_atomic, Some(&operation)));
        }
    }
}

fn control_plane_value_types() -> [(leserpent_hir::Type, &'static str); 8] {
    use leserpent_hir::Type;
    [
        (Type::RuntimeList, "runtime_list"),
        (Type::RuntimeInspect, "runtime_inspect"),
        (Type::RuntimeHistory, "runtime_history"),
        (Type::RuntimeLogs, "runtime_logs"),
        (Type::RuntimeRefresh, "runtime_refresh"),
        (
            Type::RuntimeCapabilitiesRefresh,
            "runtime_capabilities_refresh",
        ),
        (Type::RuntimeDeploy, "runtime_deploy"),
        (Type::DebuggerCancel, "debugger_cancel"),
    ]
}

fn capture_cases() -> Vec<(
    leserpent_hir::host_call::HostOperation,
    leselang_command::Value,
)> {
    use leserpent_hir::host_call::HostOperation;
    let mut values = reply_cases()
        .into_iter()
        .map(|case| {
            let operation = serde_json::from_value(json!(format!(
                "ui.{}",
                case.reply_wire["kind"].as_str().unwrap()
            )))
            .unwrap();
            (operation, leselang_command::Value::from(case.reply))
        })
        .collect::<Vec<_>>();
    for ((effect, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        values.push((
            HostOperation::for_effect(&effect).unwrap(),
            reply.into_control_plane_value(ty).unwrap(),
        ));
    }
    assert_eq!(values.len(), 70);
    values
}

#[test]
fn reference_group_binding_policy_covers_seventy_operations_and_five_saved_vocabularies() {
    use leselang_command::reference_group_binding::{validate_member, validate_saved_capture};
    use leselang_command::{
        ContinuationImage, ContinuationToken, EffectRequest, GroupResultBinding, ProjectedBinding,
        ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding, StructuredField,
        Value as ProductValue,
    };
    use leselang_runtime_core::ScalarValue;
    use leserpent_hir::{HirBranch, computation::Computation, host_call::HostOperation};
    let requests = request_admission_cases();
    let mut comparisons = 0;
    for (operation, raw) in capture_cases() {
        let (effect, envelope) = requests
            .iter()
            .find(|(effect, _)| HostOperation::for_effect(effect) == Some(operation))
            .unwrap();
        let branches = vec![HirBranch {
            name: "member".into(),
            result_type: operation.result_type(),
            effect: effect.clone(),
        }];
        let fields = vec![StructuredField {
            name: "member".into(),
            value: Box::new(raw.clone()),
        }];
        for version in 1..=5 {
            let captured = ProjectedResult::capture_version(operation, &raw, version).unwrap();
            let group = GroupResultBinding {
                pending: Effect::Sequence {
                    steps: branches.clone(),
                },
                binding: Box::new(ResultBinding {
                    name: "group".into(),
                    locals: vec![],
                    results: vec![],
                    groups: vec![],
                    body: Computation::Literal {
                        value: ScalarValue::None,
                    },
                }),
                fuel_remaining: 99,
                successor_sequence: None,
                additional_successor_sequences: vec![],
            };
            let continuation = ContinuationImage {
                schema_version: 5,
                token: ContinuationToken::from_wire("continuation-7".into()),
                program_counter: 1,
                expected_revision: None,
                result_type: operation.result_type(),
                pending_effect: effect.clone(),
                fuel_remaining: 99,
                deadline_ms: 30_000,
                deadline_at_ms: Some(40_000),
                max_output_items: 10_000,
                group_result: None,
                result_binding: Some(Box::new(ResultBinding {
                    name: "current".into(),
                    locals: vec![],
                    groups: vec![ProjectedGroupBinding {
                        name: "group".into(),
                        group: ProjectedGroup {
                            members: vec![ProjectedBinding {
                                name: "member".into(),
                                result: captured.clone(),
                            }],
                        },
                    }],
                    results: vec![ProjectedBinding {
                        name: "alias".into(),
                        result: captured,
                    }],
                    body: Computation::Literal {
                        value: ScalarValue::None,
                    },
                })),
            };
            let mut request = EffectRequest {
                effect_id: "effect-7".into(),
                required_capability: request_context(effect).required_capability.into(),
                operation: envelope.clone(),
                budget: continuation.resource_budget(),
                continuation,
            };
            let before = serde_json::to_vec(&(&group, &request, &fields)).unwrap();
            assert_eq!(validate_member(&group, "member", &raw), Ok(()));
            assert_eq!(
                validate_member(
                    &group,
                    "member",
                    &ProductValue::Scalar {
                        value: ScalarValue::None
                    }
                )
                .unwrap_err()
                .code,
                "LSV1409"
            );
            assert_eq!(
                validate_saved_capture(&group, &request, &fields, &[operation]),
                Ok(())
            );
            assert_eq!(
                serde_json::to_vec(&(&group, &request, &fields)).unwrap(),
                before
            );
            request
                .continuation
                .result_binding
                .as_mut()
                .unwrap()
                .results[0]
                .result
                .projection_version = 99;
            assert_eq!(
                validate_saved_capture(&group, &request, &fields, &[operation])
                    .unwrap_err()
                    .code,
                "LSV1405"
            );
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 350);
}

#[test]
fn adapter_result_view_lifecycle_covers_seventy_operations_and_five_saved_vocabularies() {
    use leselang_command::{
        ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding,
        ResultBindingLimits, ResultView, ResultViewError, StructuredField, Value as ProductValue,
        validate_group_capture_correspondence, validate_result_binding,
        validate_result_binding_extension,
    };
    use leselang_hir_core::projection_access::ProjectionAccessLimits;
    use leselang_runtime_core::{
        CaptureCorrespondenceLimits, CaptureValidationLimits, GroupCaptureLimits, ScalarValue,
    };
    use leserpent_hir::{
        HirBranch, computation::Computation, host_call::HostOperation, result_field::ResultField,
    };
    let mut cases = reply_cases()
        .into_iter()
        .map(|case| {
            (
                effect_from_wire(&case.operation_wire),
                ProductValue::from(case.reply),
            )
        })
        .collect::<Vec<_>>();
    for ((effect, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        cases.push((effect, reply.into_control_plane_value(ty).unwrap()));
    }
    assert_eq!(cases.len(), 70);
    let limits = ResultBindingLimits {
        captures: CaptureValidationLimits {
            max_saved_bindings: 3,
            max_name_bytes: 16,
        },
        groups: GroupCaptureLimits {
            min_members: 1,
            max_members: 1,
        },
        accesses: ProjectionAccessLimits {
            max_nodes: 100,
            max_depth: 16,
        },
    };
    for (effect, value) in cases {
        let operation = HostOperation::for_effect(&effect).unwrap();
        let branches = [HirBranch {
            name: "member".into(),
            effect: effect.clone(),
            result_type: operation.result_type(),
        }];
        let raw_group = ProductValue::Structured {
            fields: vec![StructuredField {
                name: "member".into(),
                value: Box::new(value.clone()),
            }],
        };
        let raw = ResultView::Raw {
            value: &value,
            operation,
        };
        let group_view = ResultView::Group {
            value: &raw_group,
            branches: &branches,
        };
        let current = raw.capture_result().unwrap();
        assert_eq!(
            group_view
                .select_member("member", operation)
                .unwrap()
                .capture_result()
                .unwrap(),
            current
        );
        assert_eq!(
            group_view
                .capture_group(limits.groups, |_| true)
                .unwrap()
                .members[0]
                .result,
            current
        );
        for version in 1..=5 {
            let legacy = ProjectedResult::capture_version(operation, &value, version).unwrap();
            let prior = ResultBinding {
                name: "current".into(),
                locals: vec![],
                results: vec![ProjectedBinding {
                    name: "archived".into(),
                    result: legacy.clone(),
                }],
                groups: vec![ProjectedGroupBinding {
                    name: "batch".into(),
                    group: ProjectedGroup {
                        members: vec![ProjectedBinding {
                            name: "member".into(),
                            result: legacy.clone(),
                        }],
                    },
                }],
                body: Computation::Literal {
                    value: ScalarValue::None,
                },
            };
            let mut saved = prior.clone();
            saved.name = "next".into();
            saved.results.push(ProjectedBinding {
                name: "current".into(),
                result: raw.capture_result().unwrap(),
            });
            let wire = serde_json::to_vec(&saved).unwrap();
            let restored: ResultBinding = serde_json::from_slice(&wire).unwrap();
            let saved_view = ResultView::Projected(&restored.results[0].result);
            let saved_group = ResultView::ProjectedGroup(&restored.groups[0].group);
            let member = saved_group.select_member("member", operation).unwrap();
            let ResultView::Projected(borrowed) = member else {
                panic!()
            };
            assert!(std::ptr::eq(
                borrowed,
                &restored.groups[0].group.members[0].result
            ));
            assert_eq!(saved_view.capture_result().unwrap(), legacy);
            assert_eq!(
                saved_group
                    .capture_group(limits.groups, |_| panic!("saved copy revalidated"))
                    .unwrap(),
                prior.groups[0].group
            );
            for field in ResultField::ALL {
                let expected = legacy
                    .fields
                    .iter()
                    .find(|stored| stored.field == field)
                    .map(|stored| stored.value.clone())
                    .ok_or(ResultViewError::InvalidProjection);
                assert_eq!(saved_view.project_scalar(field), expected);
                assert_eq!(member.project_scalar(field), expected);
                assert_eq!(
                    raw.project_scalar(field),
                    value
                        .project_scalar(field)
                        .ok_or(ResultViewError::InvalidProjection)
                );
            }
            assert!(validate_result_binding(&prior, &effect, limits, |_| true).is_ok());
            assert!(validate_result_binding(&restored, &effect, limits, |_| true).is_ok());
            assert_eq!(
                validate_result_binding_extension(&prior, &restored, &effect, &value, 3),
                Ok(())
            );
            let ProductValue::Structured { fields } = &raw_group else {
                panic!()
            };
            assert_eq!(
                validate_group_capture_correspondence(
                    &restored,
                    &branches,
                    fields,
                    &[operation],
                    CaptureCorrespondenceLimits {
                        max_saved_bindings: 3,
                        max_original_rows: 1
                    },
                    limits.groups,
                    |_| true,
                ),
                Ok(())
            );
            assert_eq!(serde_json::to_vec(&restored).unwrap(), wire);
        }
    }
}

#[test]
fn capture_replay_comparisons_preserve_seventy_operations_and_five_legacy_versions() {
    use leselang_command::{
        CaptureReplayError, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding,
        ProjectedResult, ResultBinding, StructuredField, Value as ProductValue,
        validate_group_capture_correspondence, validate_result_binding_extension,
    };
    use leselang_runtime_core::{CaptureCorrespondenceLimits, GroupCaptureLimits, ScalarValue};
    use leserpent_hir::{HirBranch, computation::Computation, host_call::HostOperation};
    let mut cases = reply_cases()
        .into_iter()
        .map(|case| {
            (
                effect_from_wire(&case.operation_wire),
                ProductValue::from(case.reply),
            )
        })
        .collect::<Vec<_>>();
    for ((effect, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        cases.push((effect, reply.into_control_plane_value(ty).unwrap()));
    }
    assert_eq!(cases.len(), 70);
    for (effect, raw) in cases {
        let operation = HostOperation::for_effect(&effect).unwrap();
        let branches = [HirBranch {
            name: "member".into(),
            result_type: operation.result_type(),
            effect: effect.clone(),
        }];
        let fields = [StructuredField {
            name: "member".into(),
            value: Box::new(raw.clone()),
        }];
        for version in 1..=5 {
            let captured = ProjectedResult::capture_version(operation, &raw, version).unwrap();
            let prior = ResultBinding {
                name: "current".into(),
                locals: vec![],
                results: vec![],
                groups: vec![ProjectedGroupBinding {
                    name: "group".into(),
                    group: ProjectedGroup {
                        members: vec![ProjectedBinding {
                            name: "member".into(),
                            result: captured.clone(),
                        }],
                    },
                }],
                body: Computation::Literal {
                    value: ScalarValue::None,
                },
            };
            let mut saved = prior.clone();
            saved.name = "next".into();
            saved.results = vec![
                ProjectedBinding {
                    name: "current".into(),
                    result: captured.clone(),
                },
                ProjectedBinding {
                    name: "alias".into(),
                    result: captured.clone(),
                },
            ];
            let before = serde_json::to_vec(&(&prior, &saved)).unwrap();
            assert_eq!(
                validate_result_binding_extension(&prior, &saved, &effect, &raw, 3),
                Ok(())
            );
            assert_eq!(
                validate_group_capture_correspondence(
                    &saved,
                    &branches,
                    &fields,
                    &[operation],
                    CaptureCorrespondenceLimits {
                        max_saved_bindings: 3,
                        max_original_rows: 1
                    },
                    GroupCaptureLimits {
                        min_members: 1,
                        max_members: 1
                    },
                    |_| true
                ),
                Ok(())
            );
            // A projection with no exported fields cannot certify raw result identity.
            let expected = if captured.fields.is_empty() {
                Ok(())
            } else {
                Err(CaptureReplayError::InvalidProjection)
            };
            assert_eq!(
                validate_result_binding_extension(
                    &prior,
                    &saved,
                    &effect,
                    &ProductValue::Scalar {
                        value: ScalarValue::None
                    },
                    3
                ),
                expected
            );
            assert_eq!(serde_json::to_vec(&(&prior, &saved)).unwrap(), before);
        }
    }
}

#[test]
fn complete_binding_validation_covers_seventy_operations_and_five_closed_vocabularies() {
    use leselang_command::{
        ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding,
        ResultBindingLimits, validate_result_binding,
    };
    use leselang_hir_core::projection_access::ProjectionAccessLimits;
    use leselang_runtime_core::{CaptureValidationLimits, GroupCaptureLimits};
    use leserpent_hir::{Effect, Type, computation::Computation, result_field::ResultField};
    let limits = ResultBindingLimits {
        captures: CaptureValidationLimits {
            max_saved_bindings: 2,
            max_name_bytes: 16,
        },
        groups: GroupCaptureLimits {
            min_members: 1,
            max_members: 1,
        },
        accesses: ProjectionAccessLimits {
            max_nodes: 100,
            max_depth: 16,
        },
    };
    let pending = Effect::UiFocus {
        node_id: "pending".into(),
    };
    for (operation, value) in capture_cases() {
        for version in 1..=5 {
            let saved = ProjectedResult::capture_version(operation, &value, version).unwrap();
            for field in ResultField::ALL {
                let expected = saved
                    .fields
                    .iter()
                    .find(|entry| entry.field == field)
                    .map(|entry| Type::Scalar(entry.value.scalar_type()));
                for receiver in [
                    Computation::Local {
                        name: "saved".into(),
                    },
                    Computation::Member {
                        group: "group".into(),
                        name: "member".into(),
                        operation,
                    },
                ] {
                    let captured = ResultBinding {
                        name: "reply".into(),
                        locals: vec![],
                        results: vec![ProjectedBinding {
                            name: "saved".into(),
                            result: saved.clone(),
                        }],
                        groups: vec![ProjectedGroupBinding {
                            name: "group".into(),
                            group: ProjectedGroup {
                                members: vec![ProjectedBinding {
                                    name: "member".into(),
                                    result: saved.clone(),
                                }],
                            },
                        }],
                        body: Computation::Field {
                            value: Box::new(receiver),
                            field,
                        },
                    };
                    let before = serde_json::to_vec(&captured).unwrap();
                    assert_eq!(
                        validate_result_binding(&captured, &pending, limits, |_| true).ok(),
                        expected,
                        "{operation:?} v{version} {field:?}"
                    );
                    assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
                }
            }
        }
    }
}

#[test]
fn captured_access_masks_cover_seventy_operations_five_versions_and_twelve_fields() {
    use leselang_command::{
        ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult,
        validate_captured_projection_accesses,
    };
    use leselang_hir_core::projection_access::ProjectionAccessLimits;
    use leselang_runtime_core::{CapturedBindings, ScalarValue};
    use leserpent_hir::{computation::Computation, result_field::ResultField};

    let limits = ProjectionAccessLimits {
        max_nodes: 100,
        max_depth: 16,
    };
    let pending = leserpent_hir::Effect::UiFocus {
        node_id: "pending".into(),
    };
    for (operation, value) in capture_cases() {
        for version in 1..=5 {
            let saved = ProjectedResult::capture_version(operation, &value, version).unwrap();
            let bytes = serde_json::to_vec(&saved).unwrap();
            let local = || Computation::Local {
                name: "saved".into(),
            };
            let call = || Computation::Call {
                operation,
                arguments: vec![],
            };
            let mut frame = CapturedBindings {
                name: "pending".into(),
                locals: vec![],
                results: vec![ProjectedBinding {
                    name: "saved".into(),
                    result: saved.clone(),
                }],
                groups: vec![ProjectedGroupBinding {
                    name: "group".into(),
                    group: ProjectedGroup {
                        members: vec![ProjectedBinding {
                            name: "member".into(),
                            result: saved.clone(),
                        }],
                    },
                }],
                body: local(),
            };
            for field in ResultField::ALL {
                let allowed = saved.fields.iter().any(|stored| stored.field == field);
                for receiver in [
                    local(),
                    Computation::Bind {
                        name: "alias".into(),
                        value: Box::new(local()),
                        body: Box::new(Computation::Local {
                            name: "alias".into(),
                        }),
                    },
                    Computation::Member {
                        group: "group".into(),
                        name: "member".into(),
                        operation,
                    },
                    Computation::Bind {
                        name: "alias".into(),
                        value: Box::new(Computation::Local {
                            name: "group".into(),
                        }),
                        body: Box::new(Computation::Member {
                            group: "alias".into(),
                            name: "member".into(),
                            operation,
                        }),
                    },
                    Computation::Choose {
                        when: Box::new(Computation::Literal {
                            value: ScalarValue::Boolean(true),
                        }),
                        then: Box::new(call()),
                        otherwise: Box::new(local()),
                    },
                    Computation::Choose {
                        when: Box::new(Computation::Literal {
                            value: ScalarValue::Boolean(false),
                        }),
                        then: Box::new(local()),
                        otherwise: Box::new(call()),
                    },
                ] {
                    frame.body = Computation::Field {
                        value: Box::new(receiver),
                        field,
                    };
                    assert_eq!(
                        validate_captured_projection_accesses(&frame, &pending, limits).is_ok(),
                        allowed,
                        "{operation:?} v{version} {field:?}"
                    );
                }
                // Live calls still use the current catalog, not the saved vocabulary.
                frame.body = Computation::Field {
                    value: Box::new(call()),
                    field,
                };
                assert_eq!(
                    validate_captured_projection_accesses(&frame, &pending, limits).is_ok(),
                    field.result_type(operation.result_type()).is_some()
                );
            }
            assert_eq!(serde_json::to_vec(&frame.results[0].result).unwrap(), bytes);
            assert_eq!(
                serde_json::to_vec(&frame.groups[0].group.members[0].result).unwrap(),
                bytes
            );
        }
    }
}

#[test]
fn group_captures_cover_seventy_operations_with_mixed_versions_and_exact_member_order() {
    use leselang_command::{
        ProjectedGroup, ProjectedGroupExt, StructuredField, Value as ProductValue,
    };
    use leselang_runtime_core::GroupCaptureLimits;
    use leserpent_hir::{HirBranch, host_call::HostOperation, result_field::ResultField};
    let mut cases = reply_cases()
        .into_iter()
        .map(|case| {
            (
                effect_from_wire(&case.operation_wire),
                ProductValue::from(case.reply),
            )
        })
        .collect::<Vec<_>>();
    for ((effect, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        cases.push((effect, reply.into_control_plane_value(ty).unwrap()));
    }
    assert_eq!(cases.len(), 70);
    let branches = cases
        .iter()
        .enumerate()
        .map(|(i, (effect, _))| HirBranch {
            name: format!("member-{i}"),
            result_type: HostOperation::for_effect(effect).unwrap().result_type(),
            effect: effect.clone(),
        })
        .collect::<Vec<_>>();
    let fields = cases
        .into_iter()
        .enumerate()
        .map(|(i, (_, value))| StructuredField {
            name: format!("member-{i}"),
            value: Box::new(value),
        })
        .collect::<Vec<_>>();
    let raw = ProductValue::Structured { fields };
    // The adapter accepts caller policy, not the reference VM's 63-member ceiling.
    let limits = GroupCaptureLimits {
        min_members: 1,
        max_members: 70,
    };
    let original = ProjectedGroup::capture(&branches, &raw, limits, |_| true).unwrap();
    let ProductValue::Structured { fields } = &raw else {
        panic!()
    };
    let vocabularies: [&[ResultField]; 5] = [
        &ResultField::V1,
        &ResultField::V2,
        &ResultField::V3,
        &ResultField::V4,
        &ResultField::V5,
    ];
    for round in 0..5 {
        let mut group = original.clone();
        for (index, member) in group.members.iter_mut().enumerate() {
            let version = (index + round) % 5;
            member.result.projection_version = version as u32 + 1;
            member
                .result
                .fields
                .retain(|field| vocabularies[version].contains(&field.field));
        }
        assert!(
            group
                .matches_members(&branches, fields, limits, |_| true)
                .unwrap()
        );
        let wire = serde_json::to_vec(&group).unwrap();
        let expected = format!(
            "{{\"members\":[{}]}}",
            group
                .members
                .iter()
                .map(|member| {
                    format!(
                        "{{\"name\":{},\"result\":{}}}",
                        serde_json::to_string(&member.name).unwrap(),
                        serde_json::to_string(&member.result).unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(wire, expected.as_bytes());
        let restored: ProjectedGroup = serde_json::from_slice(&wire).unwrap();
        assert_eq!(restored, group);
        assert!(restored.matches_signature(&branches));
        assert!(
            restored
                .matches_members(&branches, fields, limits, |_| true)
                .unwrap()
        );
        group.members.reverse();
        assert!(group.validate(limits, |_| true).is_ok());
        assert!(
            !group
                .matches_members(&branches, fields, limits, |_| true)
                .unwrap()
        );
    }
    let mut wire = serde_json::to_value(&original).unwrap();
    wire["unknown"] = json!(true);
    assert!(serde_json::from_value::<ProjectedGroup>(wire).is_err());
    let mut wire = serde_json::to_value(&original).unwrap();
    wire["members"][0]["unknown"] = json!(true);
    assert!(serde_json::from_value::<ProjectedGroup>(wire).is_err());
}

#[test]
fn group_capture_policy_is_explicit_and_name_hooks_keep_original_validation_order() {
    use leselang_command::{
        InvalidProjectedResult, ProjectedBinding, ProjectedGroup, ProjectedGroupExt,
        ProjectedResult, StructuredField, Value as ProductValue,
    };
    use leselang_runtime_core::GroupCaptureLimits;
    use leserpent_hir::{HirBranch, Type, host_call::HostOperation};
    let value = ProductValue::UiFocus {
        node_id: "a".into(),
    };
    let member = ProjectedBinding {
        name: "a".into(),
        result: ProjectedResult::capture(HostOperation::UiFocus, &value).unwrap(),
    };
    let mut group = ProjectedGroup {
        members: vec![
            member.clone(),
            ProjectedBinding {
                name: "b".into(),
                ..member
            },
        ],
    };
    let limits = GroupCaptureLimits {
        min_members: 1,
        max_members: 2,
    };
    for policy in [
        GroupCaptureLimits {
            min_members: 0,
            max_members: 0,
        },
        GroupCaptureLimits {
            min_members: 3,
            max_members: 2,
        },
    ] {
        assert_eq!(
            group.validate(policy, |_| panic!("count must precede callbacks")),
            Err(InvalidProjectedResult)
        );
    }
    let empty = ProjectedGroup { members: vec![] };
    assert!(
        empty
            .validate(
                GroupCaptureLimits {
                    min_members: 0,
                    max_members: 0
                },
                |_| panic!()
            )
            .is_ok()
    );
    group.members[0].result.projection_version = 0;
    let before = group.clone();
    let mut visited = Vec::new();
    assert_eq!(
        group.validate(limits, |name| {
            visited.push(name.to_owned());
            true
        }),
        Err(InvalidProjectedResult)
    );
    assert_eq!(visited, ["a", "b"]); // Every name is checked before saved result data.
    assert_eq!(group, before);
    group.members[0].result.projection_version = 1;
    group.members[1].name = "a".into();
    let mut visited = Vec::new();
    assert_eq!(
        group.validate(limits, |name| {
            visited.push(name.to_owned());
            true
        }),
        Err(InvalidProjectedResult)
    );
    assert_eq!(visited, ["a", "a"]);
    let mut visited = Vec::new();
    assert_eq!(
        group.validate(limits, |name| {
            visited.push(name.to_owned());
            false
        }),
        Err(InvalidProjectedResult)
    );
    assert_eq!(visited, ["a"]);

    let branches = [HirBranch {
        name: "custom.name".into(),
        effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        result_type: Type::UiFocus,
    }];
    let raw = ProductValue::Structured {
        fields: vec![StructuredField {
            name: "custom.name".into(),
            value: Box::new(value),
        }],
    };
    assert!(ProjectedGroup::capture(&branches, &raw, limits, |name| name == "custom.name").is_ok());
    assert_eq!(
        ProjectedGroup::capture(&branches, &raw, limits, |_| false),
        Err(InvalidProjectedResult)
    );
    let wrong_shape = ProductValue::Scalar {
        value: leserpent_hir::computation::ScalarValue::None,
    };
    assert_eq!(
        ProjectedGroup::capture(&branches, &wrong_shape, limits, |_| panic!("shape first")),
        Err(InvalidProjectedResult)
    );
}

#[test]
fn product_captures_preserve_seventy_operations_five_versions_and_exact_wire_order() {
    use leselang_command::{ProjectedField, ProjectedResult};
    use leserpent_hir::{computation::ScalarValue, result_field::ResultField};

    #[derive(serde::Serialize)]
    struct FrozenFrame<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        projection_version: Option<u32>,
        operation: &'a str,
        fields: Vec<FrozenField<'a>>,
    }
    #[derive(serde::Serialize)]
    struct FrozenField<'a> {
        field: &'a str,
        value: ScalarValue,
    }
    let fields = [
        "revision",
        "count",
        "node_id",
        "focused_node_id",
        "max_length",
        "selected",
        "required",
        "expected",
        "field",
        "value",
        "kind",
        "optional_expected",
    ];
    let widths = [5, 7, 10, 11, 12];
    for (operation, value) in capture_cases() {
        let current = ProjectedResult::capture(operation, &value).unwrap();
        let expected_version = fields
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                let key: ResultField = serde_json::from_value(json!(name)).unwrap();
                key.result_type(operation.result_type())
                    .map(|_| widths.iter().position(|width| i < *width).unwrap() as u32 + 1)
            })
            .max()
            .unwrap_or(1);
        assert_eq!(current.projection_version, expected_version);
        for (index, width) in widths.into_iter().enumerate() {
            let version = index as u32 + 1;
            let captured = ProjectedResult::capture_version(operation, &value, version).unwrap();
            let expected = FrozenFrame {
                projection_version: (version != 1).then_some(version),
                operation: operation.name(),
                fields: fields[..width]
                    .iter()
                    .filter_map(|name| {
                        let key: ResultField = serde_json::from_value(json!(name)).unwrap();
                        key.result_type(operation.result_type())
                            .map(|_| FrozenField {
                                field: name,
                                value: value.project_scalar(key).unwrap(),
                            })
                    })
                    .collect(),
            };
            let wire = serde_json::to_vec(&captured).unwrap();
            assert_eq!(
                wire,
                serde_json::to_vec(&expected).unwrap(),
                "{operation:?}/{version}"
            );
            let restored: ProjectedResult = serde_json::from_slice(&wire).unwrap();
            assert_eq!(restored, captured);
            assert!(restored.matches_capture(operation, &value).unwrap());
            for name in fields {
                let key: ResultField = serde_json::from_value(json!(name)).unwrap();
                assert_eq!(
                    restored.field(key).is_ok(),
                    expected.fields.iter().any(|f| f.field == name)
                );
            }
            // The nested field remains the core's exact generic type.
            for field in captured.fields {
                let shared: leselang_runtime_core::ScalarProjectionField<ResultField> = field;
                let _: ProjectedField = shared;
            }
        }
    }
}

#[test]
fn product_capture_validation_rejects_mutations_without_repair_or_payload_diagnostics() {
    use leselang_command::{InvalidProjectedResult, ProjectedResult};
    use leserpent_hir::computation::ScalarValue;
    for (operation, value) in capture_cases() {
        for version in 1..=5 {
            let original = ProjectedResult::capture_version(operation, &value, version).unwrap();
            for mutation in 0..7 {
                let mut changed = original.clone();
                match mutation {
                    0 => changed.projection_version = 0,
                    1 => changed.projection_version = u32::MAX,
                    2 if !changed.fields.is_empty() => {
                        changed.fields.remove(0);
                    }
                    3 if !changed.fields.is_empty() => {
                        changed.fields.push(changed.fields[0].clone())
                    }
                    4 if !changed.fields.is_empty() => changed.fields[0].value = ScalarValue::None,
                    5 if changed.fields.len() > 1 => changed.fields.reverse(),
                    6 if !changed.fields.is_empty() => {
                        changed.fields[0].value = ScalarValue::String("private-payload".repeat(400))
                    }
                    _ => continue,
                }
                let before = changed.clone();
                assert_eq!(changed.validate(), Err(InvalidProjectedResult));
                assert_eq!(
                    changed.matches_capture(operation, &value),
                    Err(InvalidProjectedResult)
                );
                assert_eq!(changed, before);
            }
        }
    }
    assert_eq!(
        InvalidProjectedResult.to_string(),
        "invalid product result projection"
    );
    assert_eq!(
        format!("{InvalidProjectedResult:?}"),
        "InvalidProjectedResult"
    );
}

#[test]
fn product_capture_shape_never_certifies_raw_identity_or_trusted_restore_version() {
    use leselang_command::{ProjectedResult, Value as ProductValue};
    use leserpent_hir::{
        computation::ScalarValue, host_call::HostOperation, result_field::ResultField,
    };
    let raw = ProductValue::UiActivate {
        node_id: "node-a".into(),
    };
    let mut frame = ProjectedResult::capture(HostOperation::UiFocus, &raw).unwrap();
    assert!(frame.matches_capture(HostOperation::UiFocus, &raw).unwrap());
    assert!(
        !frame
            .matches_capture(HostOperation::UiActivate, &raw)
            .unwrap()
    );
    // Pure projection intentionally preserves the original field-only policy.
    // VM/host gates, not this data helper, must admit the actual raw result type.
    let empty = ProjectedResult::capture(HostOperation::RuntimeRefresh, &raw).unwrap();
    assert!(empty.fields.is_empty());
    assert!(
        empty
            .matches_capture(HostOperation::RuntimeRefresh, &raw)
            .unwrap()
    );
    frame.projection_version = 0;
    assert!(frame.validate().is_err());
    assert_eq!(
        frame.field(ResultField::NodeId),
        Ok(ScalarValue::String("node-a".into()))
    );
    assert!(
        frame
            .matches_capture(HostOperation::RuntimeRefresh, &raw)
            .is_err()
    );
    let decoded: ProjectedResult =
        serde_json::from_value(serde_json::to_value(&frame).unwrap()).unwrap();
    assert!(decoded.validate().is_err());
}

#[test]
fn structured_validation_reuses_all_seventy_seven_value_types_without_vm_policy() {
    use leselang_command::{
        ResultValueDomain, ResultValueError, ResultValueHost, StructuredField,
        Value as ProductValue, ValueBoundsError, ValueValidationLimits, validate_value_items,
    };
    use leselang_runtime_core::{HostResultError, validate_host_result};
    use leserpent_hir::computation::{OptionalStringValue, ScalarValue, StringListValue};

    struct EncodingVisits(std::cell::Cell<usize>);
    impl ResultValueHost for EncodingVisits {
        type Error = std::convert::Infallible;
        fn valid_field_name(&self, _: &str) -> bool {
            panic!("leaf has no fields")
        }
        fn validate_encoded_value(&self, _: &ProductValue) -> Result<(), Self::Error> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }

    let mut values: Vec<ProductValue> = reply_cases()
        .into_iter()
        .map(|case| ProductValue::from(case.reply))
        .collect();
    for ((_, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        values.push(reply.into_control_plane_value(ty).unwrap());
    }
    values.extend(
        [
            ScalarValue::Integer(u64::MAX),
            ScalarValue::Boolean(false),
            ScalarValue::String("scalar".into()),
            ScalarValue::None,
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarValue::StringList(StringListValue(vec![])),
        ]
        .into_iter()
        .map(|value| ProductValue::Scalar { value }),
    );
    values.push(ProductValue::Structured { fields: vec![] });
    assert_eq!(values.len(), 77);
    for value in values {
        let before = serde_json::to_vec(&value).unwrap();
        for max_query_items in [0, 1, 4] {
            let limits = ValueValidationLimits {
                max_depth: 2,
                max_fields: 1,
                max_query_items,
            };
            let expected = value
                .bounded_leaf_items(max_query_items)
                .ok_or(ValueBoundsError::Value);
            for result_type in [value.result_type(), leserpent_hir::Type::UiFocus] {
                for max_output_items in [0, 1, 4] {
                    let host = EncodingVisits(std::cell::Cell::new(0));
                    let domain = ResultValueDomain {
                        result_type,
                        value_limits: limits,
                        max_output_items,
                        host: &host,
                    };
                    let result = if result_type != value.result_type() {
                        Err(HostResultError::TypeMismatch)
                    } else {
                        expected
                            .map_err(ResultValueError::Bounds)
                            .and_then(|count| {
                                if count > max_output_items || max_output_items == 0 {
                                    Err(ResultValueError::OutputLimit {
                                        count,
                                        limit: max_output_items,
                                    })
                                } else {
                                    Ok(())
                                }
                            })
                            .map_err(HostResultError::InvalidValue)
                    };
                    assert_eq!(validate_host_result(&domain, &value), result);
                    assert_eq!(host.0.get(), usize::from(result.is_ok()));
                }
            }
            assert_eq!(
                validate_value_items(&value, 0, limits, |_| panic!("leaf has no fields")),
                expected
            );
            let nested = ProductValue::Structured {
                fields: vec![StructuredField {
                    name: "a".into(),
                    value: Box::new(value.clone()),
                }],
            };
            assert_eq!(
                validate_value_items(&nested, 1, limits, |name| name == "a"),
                expected
            );
            assert_eq!(
                validate_value_items(&nested, 2, limits, |name| name == "a"),
                Err(ValueBoundsError::Nesting)
            );
        }
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
}

#[test]
fn scalar_projection_covers_all_seventy_seven_types_and_twelve_explicit_fields() {
    use leselang_command::Value as ProductValue;
    use leserpent_hir::computation::{OptionalStringValue, ScalarValue, StringListValue};
    use leserpent_hir::result_field::ResultField;

    let mut values: Vec<ProductValue> = reply_cases()
        .into_iter()
        .map(|case| ProductValue::from(case.reply))
        .collect();
    for ((_, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        values.push(reply.into_control_plane_value(ty).unwrap());
    }
    values.extend(
        [
            ScalarValue::Integer(u64::MAX),
            ScalarValue::Boolean(false),
            ScalarValue::String("scalar".into()),
            ScalarValue::None,
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarValue::StringList(StringListValue(vec!["item".into()])),
        ]
        .into_iter()
        .map(|value| ProductValue::Scalar { value }),
    );
    values.push(ProductValue::Structured { fields: vec![] });
    assert_eq!(values.len(), 77);
    assert_eq!(ResultField::ALL.len(), 12);

    for value in values {
        let before = serde_json::to_value(&value).unwrap();
        for field in ResultField::ALL {
            // The HIR type catalog and released wire fields are independent of
            // the adapter's native enum match, including renamed kind/state fields.
            let expected = field.result_type(value.result_type()).map(|ty| {
                let scalar = match field {
                    ResultField::Revision => {
                        ScalarValue::Integer(before["revision"].as_u64().unwrap())
                    }
                    ResultField::Count => ScalarValue::Integer(
                        before
                            .get("runtimes")
                            .or_else(|| before.get("entries"))
                            .map_or_else(
                                || before["count"].as_u64().unwrap(),
                                |items| items.as_array().unwrap().len() as u64,
                            ),
                    ),
                    ResultField::MaxLength => {
                        ScalarValue::Integer(before["max_length"].as_u64().unwrap())
                    }
                    ResultField::Selected => ScalarValue::Boolean(before["state"] == "selected"),
                    ResultField::Required => ScalarValue::Boolean(before["state"] == "required"),
                    ResultField::Kind => ScalarValue::String(
                        before
                            .get("expected_kind")
                            .or_else(|| before.get("input_kind"))
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .into(),
                    ),
                    ResultField::OptionalExpected => ScalarValue::OptionalString(
                        OptionalStringValue(before["expected"].as_str().map(str::to_owned)),
                    ),
                    ResultField::NodeId
                    | ResultField::FocusedNodeId
                    | ResultField::Expected
                    | ResultField::Field
                    | ResultField::Value => {
                        ScalarValue::String(before[field.name()].as_str().unwrap().into())
                    }
                };
                assert_eq!(scalar.scalar_type(), ty);
                scalar
            });
            assert_eq!(
                value.project_scalar(field),
                expected,
                "{:?}/{field:?}",
                value.result_type()
            );
        }
        assert_eq!(serde_json::to_value(&value).unwrap(), before);
    }
}

#[test]
fn scalar_projection_keeps_optional_states_and_returns_independent_owned_strings() {
    use leselang_command::Value as ProductValue;
    use leserpent_hir::computation::{OptionalStringValue, ScalarValue};
    use leserpent_hir::result_field::ResultField;

    for expected in [None, Some(String::new()), Some("ready".into())] {
        for value in [
            ProductValue::UiAssertFormFieldPlaceholder {
                node_id: "a".into(),
                field: "name".into(),
                expected: expected.clone(),
            },
            ProductValue::UiWaitFormFieldPlaceholder {
                node_id: "a".into(),
                field: "name".into(),
                expected: expected.clone(),
            },
            ProductValue::UiAssertActionUnavailableReason {
                node_id: "a".into(),
                expected: expected.clone(),
            },
            ProductValue::UiWaitActionUnavailableReason {
                node_id: "a".into(),
                expected: expected.clone(),
            },
        ] {
            assert_eq!(
                value.project_scalar(ResultField::OptionalExpected),
                Some(ScalarValue::OptionalString(OptionalStringValue(
                    expected.clone()
                )))
            );
            assert_eq!(value.project_scalar(ResultField::Expected), None);
        }
    }
    let value = ProductValue::UiSetFormValue {
        node_id: "node-a".into(),
        field: "name".into(),
        value: "ready".into(),
    };
    for (field, text) in [
        (ResultField::NodeId, "node-a"),
        (ResultField::Field, "name"),
        (ResultField::Value, "ready"),
    ] {
        let Some(ScalarValue::String(mut owned)) = value.project_scalar(field) else {
            panic!("missing string");
        };
        let Some(ScalarValue::String(second)) = value.project_scalar(field) else {
            panic!("missing string");
        };
        assert_ne!(owned.as_ptr(), second.as_ptr());
        owned.clear();
        assert_eq!(second, text);
        assert_eq!(
            value.project_scalar(field),
            Some(ScalarValue::String(text.into()))
        );
    }
}

#[test]
fn scalar_projection_is_not_bounds_validation_or_execution_admission() {
    use leselang_command::Value as ProductValue;
    use leselang_host_contract::Revision;
    use leserpent_hir::computation::ScalarValue;
    use leserpent_hir::result_field::ResultField;

    for count in [0, 1, usize::MAX] {
        let value = ProductValue::UiAssertChildCount {
            node_id: "".into(),
            count,
        };
        assert_eq!(
            value.project_scalar(ResultField::Count),
            Some(ScalarValue::Integer(count as u64))
        );
    }
    let oversized = "x".repeat(leserpent_hir::computation::MAX_SCALAR_STRING_BYTES + 1);
    let value = ProductValue::UiAssertText {
        node_id: "".into(),
        expected: oversized.clone(),
    };
    assert_eq!(value.bounded_leaf_items(1), None);
    assert_eq!(
        value.project_scalar(ResultField::Expected),
        Some(ScalarValue::String(oversized))
    );
    let value = ProductValue::RuntimeList {
        revision: Revision(u64::MAX),
        runtimes: vec![],
    };
    assert_eq!(
        value.project_scalar(ResultField::Revision),
        Some(ScalarValue::Integer(u64::MAX))
    );
    assert_eq!(
        value.project_scalar(ResultField::Count),
        Some(ScalarValue::Integer(0))
    );
    for (state, expected) in [
        (UiSelectionState::Selected, true),
        (UiSelectionState::Unselected, false),
    ] {
        let value = ProductValue::UiSetSelection {
            node_id: "".into(),
            state,
        };
        assert_eq!(
            value.project_scalar(ResultField::Selected),
            Some(ScalarValue::Boolean(expected))
        );
    }
    for (state, expected) in [
        (UiFormRequirementState::Required, true),
        (UiFormRequirementState::Optional, false),
    ] {
        let value = ProductValue::UiAssertFormFieldRequired {
            node_id: "".into(),
            field: "".into(),
            state,
        };
        assert_eq!(
            value.project_scalar(ResultField::Required),
            Some(ScalarValue::Boolean(expected))
        );
    }
}

fn expected_control_plane_value_wire(reply: &leselang_command::EffectResult, tag: &str) -> Value {
    let payload = serde_json::to_value(reply).unwrap()["payload"].clone();
    if matches!(reply, leselang_command::EffectResult::Query(_)) {
        let mut expected = payload;
        expected["kind"] = json!(tag);
        expected
    } else {
        json!({"kind": tag, "result": payload})
    }
}

#[test]
fn control_plane_value_projection_keeps_closed_type_matrix_and_released_wire() {
    use leselang_command::{EffectResult, Value as ProductValue};
    use leserpent_hir::{Type, computation::ScalarType};
    let accepted = control_plane_value_types();
    let mut rejected = vec![
        Type::Structured,
        Type::Scalar(ScalarType::Integer),
        Type::Scalar(ScalarType::Boolean),
        Type::Scalar(ScalarType::String),
        Type::Scalar(ScalarType::None),
        Type::Scalar(ScalarType::OptionalString),
        Type::Scalar(ScalarType::StringList),
    ];
    for gui in reply_cases() {
        let ty = ProductValue::from(gui.reply.clone()).result_type();
        rejected.push(ty);
        for (target, _) in accepted {
            assert!(
                EffectResult::Presentation(gui.reply.clone())
                    .into_control_plane_value(target)
                    .is_none()
            );
        }
        assert!(
            EffectResult::Presentation(gui.reply)
                .into_control_plane_value(ty)
                .is_none()
        );
    }
    assert_eq!(accepted.len() + rejected.len(), 77);
    for (index, (_, _, reply)) in control_plane_reply_cases().into_iter().enumerate() {
        for (target_index, (ty, tag)) in accepted.iter().copied().enumerate() {
            let value = reply.clone().into_control_plane_value(ty);
            let matches = index == target_index
                || ((4..=6).contains(&index) && (4..=6).contains(&target_index));
            if matches {
                let value = value.unwrap();
                let expected = expected_control_plane_value_wire(&reply, tag);
                assert_eq!(serde_json::to_value(&value).unwrap(), expected);
                assert_eq!(value.result_type(), ty);
                assert_eq!(
                    serde_json::from_value::<ProductValue>(expected).unwrap(),
                    value
                );
            } else {
                assert!(value.is_none(), "wrong projection {index} as {ty:?}");
            }
        }
        for &ty in &rejected {
            assert!(reply.clone().into_control_plane_value(ty).is_none());
        }
    }
}

#[test]
fn control_plane_value_projection_moves_buffers_and_keeps_inspect_boxing() {
    use leselang_command::{EffectResult, Value as ProductValue};
    use leserpent_domain::{CommandResult, QueryResult};
    for (index, (_, _, mut reply)) in control_plane_reply_cases().into_iter().enumerate() {
        if let EffectResult::Query(QueryResult::RuntimeLogs { entries, .. }) = &mut reply {
            entries.push(leserpent_domain::RuntimeLogRecord {
                sequence: 1,
                level: leserpent_domain::RuntimeLogLevel::Info,
                message: "ready".into(),
            });
        }
        let before: Vec<usize> = match &reply {
            EffectResult::Query(QueryResult::RuntimeList { runtimes, .. }) => {
                vec![runtimes.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                vec![entries.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                runtime_name,
                entries,
                ..
            }) => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            EffectResult::Command(result) => vec![&**result as *const CommandResult as usize],
            EffectResult::DebuggerCancel(result) => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        let value = reply
            .into_control_plane_value(control_plane_value_types()[index].0)
            .unwrap();
        let after: Vec<usize> = match &value {
            ProductValue::RuntimeList { runtimes, .. } => vec![runtimes.as_ptr() as usize],
            ProductValue::RuntimeInspect { runtime, .. } => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            ProductValue::RuntimeHistory { entries, .. } => vec![entries.as_ptr() as usize],
            ProductValue::RuntimeLogs {
                runtime_name,
                entries,
                ..
            } => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            ProductValue::RuntimeRefresh { result }
            | ProductValue::RuntimeCapabilitiesRefresh { result }
            | ProductValue::RuntimeDeploy { result } => {
                vec![&**result as *const CommandResult as usize]
            }
            ProductValue::DebuggerCancel { result } => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        assert_eq!(after, before);
    }
}

#[test]
fn control_plane_value_projection_does_not_certify_correspondence_status_time_or_bounds() {
    use leselang_command::EffectResult;
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{CommandStatus, QueryResult};
    for (index, (_, _, mut reply)) in control_plane_reply_cases().into_iter().enumerate() {
        match &mut reply {
            EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                *revision = Revision(999);
                runtimes[0].name.clear();
                runtimes.resize(10_001, runtimes[0].clone());
            }
            EffectResult::Query(QueryResult::RuntimeInspect { revision, runtime }) => {
                *revision = Revision(999);
                runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries[0].runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                runtime_id,
                runtime_name,
                ..
            }) => {
                *runtime_id = RuntimeId::new("other").unwrap();
                runtime_name.clear();
            }
            EffectResult::Command(result) => {
                result.status = CommandStatus::Planned;
                result.command_id = CommandId::new("other").unwrap();
            }
            EffectResult::DebuggerCancel(result) => {
                result.observed_at_ms = u64::MAX;
                result.session_id.clear();
            }
            _ => unreachable!(),
        }
        let (ty, tag) = control_plane_value_types()[index];
        let expected = expected_control_plane_value_wire(&reply, tag);
        let value = reply.into_control_plane_value(ty).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
}

fn control_plane_cases() -> Vec<(Effect, leselang_command::PlannedOperation)> {
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
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        let context = LoweringContext {
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new([
                "runtime.read",
                "runtime.refresh",
                "runtime.deploy",
                "debugger.control",
            ]),
            expected_revision: Some(leselang_host_contract::Revision(7)),
            command_id: CommandId::new("command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
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
        };
        let operation = lower_effect(&effect, &context).unwrap().operation;
        (effect, operation)
    })
    .collect()
}

fn control_plane_matches(operation: &leselang_command::PlannedOperation, effect: &Effect) -> bool {
    match operation {
        leselang_command::PlannedOperation::Query(query) => {
            leselang_command::query_matches_effect(&query.query, effect)
        }
        leselang_command::PlannedOperation::Command(command) => {
            leselang_command::command_matches_effect(
                command,
                effect,
                "command-a",
                "effect-a",
                Some(leselang_host_contract::Revision(7)),
            )
        }
    }
}

#[test]
fn control_plane_requests_reject_torn_payloads_and_command_metadata() {
    use leselang_command::PlannedOperation;
    for (effect, operation) in control_plane_cases() {
        let wire = serde_json::to_value(&operation).unwrap();
        let payload = if matches!(operation, PlannedOperation::Query(_)) {
            "query"
        } else {
            "command"
        };
        for (field, _) in wire["payload"][payload].as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let mutations = match field.as_str() {
                "filter" => ["environment", "cluster", "role"]
                    .into_iter()
                    .map(|field| {
                        let mut filter = wire["payload"][payload]["filter"].clone();
                        filter[field] = json!("other");
                        filter
                    })
                    .collect(),
                "after_sequence" => vec![json!(0), json!(u64::MAX)],
                "limit" => vec![json!(0), json!(255), json!(257), json!(u16::MAX)],
                "runtime_id" | "pipeline_kind" | "target" | "session_id" => vec![json!("other")],
                _ => panic!("add a payload mutation for {field}"),
            };
            for value in mutations {
                let mut changed = wire.clone();
                changed["payload"][payload][field] = value;
                let changed: PlannedOperation = serde_json::from_value(changed).unwrap();
                assert!(
                    !control_plane_matches(&changed, &effect),
                    "{payload}.{field}"
                );
            }
        }
        let PlannedOperation::Command(command) = &operation else {
            continue;
        };
        let confirmation = if command.confirmation == Confirmation::Confirmed {
            "not_required"
        } else {
            "confirmed"
        };
        for (field, value) in [
            ("command_id", json!("other-command")),
            ("idempotency_key", json!("other-key")),
            ("expected_revision", Value::Null),
            ("expected_revision", json!(8)),
            ("origin", json!("gui")),
            ("origin", json!("cli")),
            ("origin", json!("model")),
            ("origin", json!("compatibility_adapter")),
            ("confirmation", json!(confirmation)),
            ("dry_run", json!(true)),
        ] {
            let mut changed = wire.clone();
            changed["payload"][field] = value;
            let changed: PlannedOperation = serde_json::from_value(changed).unwrap();
            assert!(!control_plane_matches(&changed, &effect), "{field}");
        }
        assert!(!leselang_command::command_matches_effect(
            command,
            &effect,
            "other",
            "effect-a",
            Some(leselang_host_contract::Revision(7))
        ));
        assert!(!leselang_command::command_matches_effect(
            command,
            &effect,
            "command-a",
            "other",
            Some(leselang_host_contract::Revision(7))
        ));
    }
}

#[test]
fn control_plane_correspondence_is_not_authority_canonicalization_or_identity_generation() {
    use leselang_command::{PlannedOperation, command_matches_effect, query_matches_effect};
    use leselang_host_contract::{Revision, RuntimeListFilter};
    use leserpent_domain::{Command, Query};

    for (effect, operation) in control_plane_cases() {
        let PlannedOperation::Command(mut command) = operation else {
            continue;
        };
        command.schema_version = 9;
        command.principal.id.clear();
        command.capabilities = CapabilitySet::new([] as [&str; 0]);
        command.command_id = CommandId::new("caller-command").unwrap();
        command.idempotency_key = IdempotencyKey::new("caller-key").unwrap();
        for revision in [None, Some(Revision(0)), Some(Revision(u64::MAX))] {
            command.expected_revision = revision;
            assert!(command_matches_effect(
                &command,
                &effect,
                "caller-command",
                "caller-key",
                revision
            ));
        }
        if let Effect::RuntimeDeploy { runtime_id, .. } = effect {
            let effect = Effect::RuntimeDeploy {
                runtime_id: runtime_id.clone(),
                pipeline_kind: String::new(),
                target: Some(String::new()),
            };
            command.command = Command::RuntimeDeploy {
                runtime_id,
                pipeline_kind: String::new(),
                target: Some(String::new()),
            };
            assert!(command_matches_effect(
                &command,
                &effect,
                "caller-command",
                "caller-key",
                command.expected_revision
            ));
            assert!(leserpent_hir::canonical_source(&effect).is_err());
        }
    }
    let filter = RuntimeListFilter {
        environment: Some(String::new()),
        cluster: None,
        role: None,
    };
    let effect = Effect::RuntimeList {
        filter: filter.clone(),
    };
    let query = Query::RuntimeList { filter };
    assert!(query_matches_effect(&query, &effect));
    assert!(!query_matches_effect(
        &query,
        &Effect::RuntimeList {
            filter: Default::default()
        }
    ));
    assert!(leserpent_hir::canonical_source(&effect).is_err());
}

#[test]
fn control_plane_correspondence_rejects_non_language_registration_commands() {
    use leselang_command::{PlannedOperation, command_matches_effect};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::Command;
    let runtime_id = RuntimeId::new("runtime-a").unwrap();
    let commands = [
        Command::RuntimeRegister {
            runtime_id: runtime_id.clone(),
            name: "name".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            sidecar_endpoint: None,
            tags: Default::default(),
        },
        Command::RuntimeRegistrationUpdate {
            runtime_id: runtime_id.clone(),
            name: "name".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            sidecar_endpoint: None,
            tags: Default::default(),
        },
        Command::RuntimeDiscoveryIntake {
            runtime_id,
            capabilities: None,
            status: None,
            sidecar_status: None,
        },
    ];
    for (effect, operation) in control_plane_cases() {
        let PlannedOperation::Command(mut envelope) = operation else {
            continue;
        };
        for command in &commands {
            envelope.command = command.clone();
            assert!(!command_matches_effect(
                &envelope,
                &effect,
                "command-a",
                "effect-a",
                Some(Revision(7))
            ));
        }
    }
}

#[test]
fn all_presentation_requests_match_only_their_own_effect_without_mutation() {
    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for (index, case) in cases.iter().enumerate() {
        let effect = effect_from_wire(&case.operation_wire);
        let before = serde_json::to_vec(&(&effect, &case.operation)).unwrap();
        assert_eq!(
            leserpent_hir::host_call::HostOperation::for_effect(&effect)
                .unwrap()
                .required_capability(),
            "ui.presentation"
        );
        for (other_index, other) in cases.iter().enumerate() {
            assert_eq!(
                other.operation.matches_effect(&effect),
                index == other_index
            );
        }
        assert_eq!(
            serde_json::to_vec(&(&effect, &case.operation)).unwrap(),
            before
        );
    }
}

#[test]
fn presentation_requests_reject_each_torn_field_and_preserve_optional_distinctions() {
    for case in reply_cases() {
        let effect = effect_from_wire(&case.operation_wire);
        for (field, value) in case.operation_wire.as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let mut changed = case.operation_wire.clone();
            changed[field] = different_field(field, value);
            let operation: PresentationOperation = serde_json::from_value(changed.clone()).unwrap();
            assert!(!operation.matches_effect(&effect), "{changed}");
            if field != "timeout_ms" {
                assert!(
                    !case.operation.matches_effect(&effect_from_wire(&changed)),
                    "{changed}"
                );
            }
        }
        if case.operation_wire.get("timeout_ms").is_some() {
            for timeout in [0, 1, 1999, 2000, 2001, u64::MAX] {
                let mut changed = case.operation_wire.clone();
                changed["timeout_ms"] = json!(timeout);
                let operation: PresentationOperation = serde_json::from_value(changed).unwrap();
                assert_eq!(operation.matches_effect(&effect), timeout == 2000);
            }
        }
        let name = operation_name(&case.operation);
        if name.ends_with("unavailable_reason") || name.ends_with("placeholder") {
            for expected in [Value::Null, json!(""), json!("present")] {
                let mut pending = case.operation_wire.clone();
                pending["expected"] = expected.clone();
                let effect = effect_from_wire(&pending);
                for actual in [Value::Null, json!(""), json!("present")] {
                    let mut operation = pending.clone();
                    operation["expected"] = actual.clone();
                    let operation: PresentationOperation =
                        serde_json::from_value(operation).unwrap();
                    assert_eq!(operation.matches_effect(&effect), actual == expected);
                }
            }
        }
    }
}

#[test]
fn all_presentation_replies_match_only_their_own_effect_and_saved_operation() {
    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for (index, case) in cases.iter().enumerate() {
        let effect = effect_from_wire(&case.operation_wire);
        let operation = envelope(case.operation.clone());
        let before = serde_json::to_vec(&(&effect, &operation, &case.reply)).unwrap();
        assert!(case.reply.matches_effect(&effect, None));
        assert!(case.reply.matches_effect(&effect, Some(&operation)));
        for (other_index, other) in cases.iter().enumerate() {
            let expected = index == other_index;
            assert_eq!(other.reply.matches_effect(&effect, None), expected);
            assert_eq!(
                other.reply.matches_effect(&effect, Some(&operation)),
                expected
            );
            assert_eq!(
                case.reply
                    .matches_effect(&effect, Some(&envelope(other.operation.clone()))),
                expected
            );
            assert_eq!(
                case.reply
                    .matches_effect(&effect_from_wire(&other.operation_wire), Some(&operation)),
                expected
            );
        }
        assert_eq!(
            serde_json::to_vec(&(&effect, &operation, &case.reply)).unwrap(),
            before
        );
    }
}

fn different_field(field: &str, value: &Value) -> Value {
    match field {
        "node_id" | "field" | "value" | "expected" => json!("different"),
        "timeout_ms" | "count" | "max_length" => json!(value.as_u64().unwrap() + 1),
        "direction" => json!("previous"),
        "state" if value == "selected" => json!("unselected"),
        "state" => json!("optional"),
        "expected_kind" if value == "runtime_card" => json!("column"),
        "expected_kind" => json!("runtime_refresh"),
        "input_kind" => json!("path_token"),
        _ => panic!("add a typed mutation for {field}"),
    }
}

#[test]
fn all_presentation_replies_reject_each_torn_payload_field() {
    for case in reply_cases() {
        let effect = effect_from_wire(&case.operation_wire);
        let operation = envelope(case.operation.clone());
        for (field, value) in case.operation_wire.as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let changed = different_field(field, value);
            assert_ne!(changed, *value);
            let mut reply_wire = case.reply_wire.clone();
            reply_wire[field] = changed.clone();
            let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
            assert!(
                !reply.matches_effect(&effect, None),
                "reply {} {field}",
                reply_name(&reply)
            );
            assert!(!reply.matches_effect(&effect, Some(&operation)));
            let mut operation_wire = case.operation_wire.clone();
            operation_wire[field] = changed;
            let torn = envelope(serde_json::from_value(operation_wire.clone()).unwrap());
            assert!(
                !case.reply.matches_effect(&effect, Some(&torn)),
                "operation {} {field}",
                reply_name(&case.reply)
            );
            // A reply and envelope agreeing with each other is not enough.
            assert!(!reply.matches_effect(&effect, Some(&torn)));
            if field != "timeout_ms" {
                let torn_effect = effect_from_wire(&operation_wire);
                assert!(!case.reply.matches_effect(&torn_effect, None));
                assert!(!case.reply.matches_effect(&torn_effect, Some(&operation)));
            }
        }
    }
}

#[test]
fn presentation_reply_optional_text_keeps_none_empty_and_present_distinct() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        )
    }) {
        let states = [Value::Null, json!(""), json!("ready")];
        for expected in &states {
            let mut operation_wire = case.operation_wire.clone();
            operation_wire["expected"] = expected.clone();
            let effect = effect_from_wire(&operation_wire);
            let operation = envelope(serde_json::from_value(operation_wire).unwrap());
            for actual in &states {
                let mut reply_wire = case.reply_wire.clone();
                reply_wire["expected"] = actual.clone();
                let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
                assert_eq!(reply.matches_effect(&effect, None), actual == expected);
                assert_eq!(
                    reply.matches_effect(&effect, Some(&operation)),
                    actual == expected
                );
            }
        }
    }
}

#[test]
fn presentation_reply_navigation_and_count_boundaries_remain_product_rules() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "navigate_focus" | "assert_child_count" | "wait_child_count"
        )
    }) {
        for (value, accepted) in if operation_name(&case.operation) == "navigate_focus" {
            vec![
                (json!("child-a"), true),
                (json!("node-a"), false),
                (json!(""), false),
                (json!("bad node"), false),
                (
                    json!("x".repeat(leserpent_hir::MAX_UI_NODE_ID_BYTES + 1)),
                    false,
                ),
            ]
        } else {
            vec![
                (json!(0), true),
                (json!(leserpent_hir::MAX_UI_CHILD_COUNT), true),
                (json!(leserpent_hir::MAX_UI_CHILD_COUNT + 1), false),
            ]
        } {
            let mut operation_wire = case.operation_wire.clone();
            let mut reply_wire = case.reply_wire.clone();
            if operation_name(&case.operation) == "navigate_focus" {
                reply_wire["focused_node_id"] = value;
            } else {
                operation_wire["count"] = value.clone();
                reply_wire["count"] = value;
            }
            let effect = effect_from_wire(&operation_wire);
            let operation = envelope(serde_json::from_value(operation_wire).unwrap());
            let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
            assert_eq!(reply.matches_effect(&effect, None), accepted);
            assert_eq!(reply.matches_effect(&effect, Some(&operation)), accepted);
        }
    }
}

#[test]
fn presentation_reply_correspondence_is_not_an_authority_certificate() {
    let case = reply_cases().remove(1);
    let effect = effect_from_wire(&case.operation_wire);
    let mut operation = envelope(case.operation);
    let EffectOperation::Presentation(saved) = &mut operation else {
        panic!()
    };
    saved.schema_version = 0;
    saved.principal.id = "bad principal".into();
    saved.capabilities = CapabilitySet::default();
    assert!(case.reply.matches_effect(&effect, Some(&operation)));
    let foreign = EffectOperation::Query(leserpent_domain::QueryEnvelope {
        schema_version: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["runtime.read"]),
        query: leserpent_domain::Query::RuntimeList {
            filter: Default::default(),
        },
    });
    assert!(!case.reply.matches_effect(&effect, Some(&foreign)));
    for source in [
        "runtime.list()",
        "add(left: 1, right: 2)",
        r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    ] {
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        assert!(!case.reply.matches_effect(&effect, None));
        assert!(!case.reply.matches_effect(&effect, Some(&operation)));
    }
}
