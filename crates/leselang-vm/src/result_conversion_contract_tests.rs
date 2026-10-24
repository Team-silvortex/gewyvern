use super::*;

#[test]
fn conversion_matches_frozen_for_all_operations_bindings_contexts_and_competing_faults() {
    let mut comparisons = 0;
    for (request, original_reply) in crate::reply_contract_tests::cases() {
        for kind in 0..6 {
            for supplied_operation in [false, true] {
                for defect in 0..8 {
                    let mut image = request.continuation.clone();
                    image.result_binding = crate::group_start_fixture::binding(kind);
                    let operation = supplied_operation.then_some(&request.operation);
                    let mut reply = original_reply.clone();
                    match defect {
                        1 => image.fuel_remaining = 0,
                        2 => image.fuel_remaining = 1,
                        3 => image.max_output_items = 0,
                        4 => image.result_type = Type::Structured,
                        5 => image.expected_revision = Some(Revision(u64::MAX)),
                        6 => {
                            reply = EffectResult::Presentation(PresentationResult::Focus {
                                node_id: "wrong".into(),
                            })
                        }
                        7 => {
                            image.fuel_remaining = 0;
                            image.max_output_items = 0;
                            image.result_type = Type::Structured;
                        }
                        _ => {}
                    }
                    let before = serde_json::to_vec(&image).unwrap();
                    let atomic = atomic_step_from_effect_result(&image, operation, reply.clone());
                    assert_eq!(
                        atomic,
                        frozen::atomic_step_from_effect_result(&image, operation, reply.clone())
                    );
                    if let Step::Done(value) = &atomic {
                        assert_eq!(
                            validate_bound_value(&image, value),
                            frozen::validate_bound_value(&image, value)
                        );
                    }
                    let expected =
                        frozen::step_from_effect_result(&image, operation, reply.clone());
                    let actual = step_from_effect_result(&image, operation, reply);
                    assert_eq!(
                        actual, expected,
                        "kind={kind}, supplied={supplied_operation}, defect={defect}, type={:?}",
                        request.continuation.result_type
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(serde_json::to_vec(&image).unwrap(), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 6720);
}

// Frozen pre-extraction conversion/observation composition. Shared kernels remain shared.
pub(crate) mod frozen {
    use super::super::*;
    use std::convert::Infallible;

    use leselang_command::{ResultValueDomain, ResultValueError, ResultValueHost};
    use leselang_runtime_core::{
        HostResultError, PendingReply, ReplyAcceptanceError, ReplyAuthority,
    };

    use crate::{ContinuationImage, Fault, MAX_JOURNAL_ENTRY_BYTES, Value};

    struct BoundResultHost;

    impl ResultValueHost for BoundResultHost {
        type Error = Fault;

        fn valid_field_name(&self, name: &str) -> bool {
            crate::valid_merge_branch_name(name)
        }

        fn validate_encoded_value(&self, reply: &Value) -> Result<(), Fault> {
            crate::validate_json_size_capped(reply, MAX_JOURNAL_ENTRY_BYTES, "bound host result")
        }
    }

    struct ResultObservation;

    impl ReplyAuthority<&str> for ResultObservation {
        type Error = Infallible;

        fn authorize(&self, _: &&str) -> Result<(), Infallible> {
            // Durable identity/lease/revision checks precede raw result construction.
            // This observation-only checkpoint grants no new execution authority.
            Ok(())
        }
    }

    pub(super) fn accept_bound_value<'a>(
        image: &'a ContinuationImage,
        value: &'a Value,
    ) -> Result<(&'a ContinuationImage, &'a Value), Fault> {
        let declaration = ResultValueDomain {
            result_type: image.result_type,
            value_limits: crate::value_validation_limits(),
            max_output_items: image.max_output_items,
            host: &BoundResultHost,
        };
        let mut pending = PendingReply::new(image.token.as_str(), image, &declaration);
        let accepted = pending
            .try_accept(&image.token.as_str(), value, &ResultObservation)
            .map_err(|rejected| match rejected.error {
                ReplyAcceptanceError::Result(HostResultError::TypeMismatch) => Fault {
                    code: "LSV2103".into(),
                    message: "effect result does not match pending effect".into(),
                },
                ReplyAcceptanceError::Result(HostResultError::InvalidValue(error)) => match error {
                    ResultValueError::Bounds(error) => crate::value_bounds_fault(error),
                    ResultValueError::OutputLimit { .. } => Fault {
                        code: "LSV2102".into(),
                        message: "bound result exceeds the output item limit".into(),
                    },
                    ResultValueError::Encoding(error) => error,
                },
                ReplyAcceptanceError::Authority(error) => match error {},
                ReplyAcceptanceError::Closed(_) | ReplyAcceptanceError::IdentityMismatch => {
                    crate::result_binding::invalid()
                }
            })?;
        let (_, image, _, value) = accepted.into_parts();
        Ok((image, value))
    }

    pub(super) fn validate_bound_value(
        image: &ContinuationImage,
        value: &Value,
    ) -> Result<(), Fault> {
        accept_bound_value(image, value).map(|_| ())
    }

    pub(crate) fn step_from_effect_result(
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        let step = atomic_step_from_effect_result(image, operation, result);
        let Some(binding) = &image.result_binding else {
            return step;
        };
        let Step::Done(value) = step else {
            return step;
        };
        let (image, value) = match accept_bound_value(image, &value) {
            Ok(accepted) => accepted,
            Err(error) => return Step::Fault(error),
        };
        let mut fuel = Fuel::new(image.fuel_remaining);
        let Some(operation) =
            leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        else {
            return Step::Fault(result_binding::invalid());
        };
        match computation::resume(binding, value, operation, &mut fuel) {
            Ok(value) => Step::Done(Value::Scalar { value }),
            Err(error) => Step::Fault(error),
        }
    }

    pub(crate) fn atomic_step_from_effect_result(
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        match result.into_effect_value(leselang_command::EffectValueContext {
            effect: &image.pending_effect,
            result_type: image.result_type,
            operation,
            expected_revision: image.expected_revision,
            max_output_items: image.max_output_items,
        }) {
            Ok(value) => Step::Done(value),
            Err(leselang_command::EffectValueError::Mismatch) => {
                fault("LSV2103", "effect result does not match pending effect")
            }
            Err(leselang_command::EffectValueError::RevisionConflict { expected, actual }) => {
                fault(
                    "LSV2101",
                    format!(
                        "effect revision conflict: expected {}, actual {}",
                        expected.0, actual.0
                    ),
                )
            }
            Err(leselang_command::EffectValueError::OutputLimit { count, limit }) => fault(
                "LSV2102",
                format!("effect returned {count} items, limit is {limit}"),
            ),
        }
    }
}
