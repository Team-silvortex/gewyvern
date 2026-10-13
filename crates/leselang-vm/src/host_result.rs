use std::convert::Infallible;

use leselang_command::{ResultValueDomain, ResultValueError, ResultValueHost};
use leselang_runtime_core::{HostResultError, PendingReply, ReplyAcceptanceError, ReplyAuthority};

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

pub(super) fn validate_bound_value(image: &ContinuationImage, value: &Value) -> Result<(), Fault> {
    accept_bound_value(image, value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Step, Vm};
    use leselang_hir::{Type, computation::ScalarValue};
    use leselang_host_contract::{CapabilitySet, Principal};

    fn legacy_bound_value(image: &ContinuationImage, reply: &Value) -> Result<(), Fault> {
        if crate::group_binding::value_type(reply) != image.result_type {
            return Err(Fault {
                code: "LSV2103".into(),
                message: "effect result does not match pending effect".into(),
            });
        }
        let items = crate::validate_value(reply, 0)?;
        if items > image.max_output_items || image.max_output_items == 0 {
            return Err(Fault {
                code: "LSV2102".into(),
                message: "bound result exceeds the output item limit".into(),
            });
        }
        crate::validate_json_size_capped(reply, MAX_JOURNAL_ENTRY_BYTES, "bound host result")
    }

    fn structure(values: Vec<Value>) -> Value {
        Value::Structured {
            fields: values
                .into_iter()
                .enumerate()
                .map(|(index, value)| crate::StructuredField {
                    name: format!("field{index}"),
                    value: Box::new(value),
                })
                .collect(),
        }
    }

    #[test]
    fn bound_value_domain_matches_frozen_fault_order_bytes_and_original_owners() {
        let mut image = image();
        let valid = Value::UiFocus {
            node_id: "a".into(),
        };
        let mut deep = valid.clone();
        for _ in 0..=crate::MAX_STRUCTURED_VALUE_DEPTH {
            deep = structure(vec![deep]);
        }
        let values = [
            valid.clone(),
            Value::UiFocus {
                node_id: "bad id".into(),
            },
            Value::UiActivate {
                node_id: "x".repeat(4097),
            },
            Value::RuntimeList {
                revision: leselang_host_contract::Revision(7),
                runtimes: vec![],
            },
            Value::Scalar {
                value: ScalarValue::String("x".repeat(4096)),
            },
            structure(vec![]),
            structure(vec![valid.clone(), valid.clone()]),
            deep,
        ];
        for value in &values {
            let bytes = serde_json::to_vec(value).unwrap();
            for result_type in [
                value.result_type(),
                Type::UiFocus,
                Type::Structured,
                Type::RuntimeList,
            ] {
                for max_output_items in [0, 1, 2, crate::DEFAULT_MAX_OUTPUT_ITEMS] {
                    image.result_type = result_type;
                    image.max_output_items = max_output_items;
                    let saved = serde_json::to_vec(&image).unwrap();
                    let expected = legacy_bound_value(&image, value);
                    let actual = accept_bound_value(&image, value);
                    assert_eq!(
                        actual
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| (*error).clone()),
                        expected
                    );
                    if let Ok((frame, received)) = actual {
                        assert!(std::ptr::eq(frame, &image));
                        assert!(std::ptr::eq(received, value));
                    }
                    assert_eq!(serde_json::to_vec(&image).unwrap(), saved);
                    assert_eq!(serde_json::to_vec(value).unwrap(), bytes);
                }
            }
        }
        let oversized = structure(
            (0..64)
                .map(|_| {
                    structure(
                        (0..34)
                            .map(|_| Value::Scalar {
                                value: ScalarValue::String("x".repeat(4096)),
                            })
                            .collect(),
                    )
                })
                .collect(),
        );
        image.result_type = Type::Structured;
        image.max_output_items = crate::DEFAULT_MAX_OUTPUT_ITEMS;
        assert!(crate::validate_value(&oversized, 0).is_ok());
        let expected = legacy_bound_value(&image, &oversized).unwrap_err();
        assert_eq!(expected.code, "LSV3002");
        assert_eq!(validate_bound_value(&image, &oversized), Err(expected));
    }

    fn image() -> ContinuationImage {
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id"))"#,
        )).unwrap();
        let Step::Effect(request) = Vm::new(1000).start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
        ) else {
            panic!("expected effect")
        };
        request.continuation
    }

    #[test]
    fn wrong_result_kind_precedes_native_bounds_without_mutating_the_image() {
        let mut image = image();
        image.max_output_items = 0;
        let original = serde_json::to_vec(&image).unwrap();
        let reply = Value::UiActivate {
            node_id: "x".repeat(4097),
        };
        let error = validate_bound_value(&image, &reply).unwrap_err();
        assert_eq!(error.code, "LSV2103");
        assert_eq!(error.message, "effect result does not match pending effect");
        assert_eq!(serde_json::to_vec(&image).unwrap(), original);
    }

    #[test]
    fn valid_received_value_keeps_buffer_ownership_and_saved_fuel() {
        let image = image();
        let node_id = String::from("received-node");
        let pointer = node_id.as_ptr();
        let reply = Value::UiFocus { node_id };
        let fuel = image.fuel_remaining;
        let (accepted_image, accepted_reply) = accept_bound_value(&image, &reply).unwrap();
        assert!(std::ptr::eq(accepted_image, &image));
        assert!(std::ptr::eq(accepted_reply, &reply));
        let Value::UiFocus { node_id } = reply else {
            panic!()
        };
        assert_eq!(node_id.as_ptr(), pointer);
        assert_eq!(image.fuel_remaining, fuel);
    }

    #[test]
    fn native_validation_and_output_limit_faults_keep_their_exact_mapping() {
        let mut image = image();
        let reply = Value::UiFocus {
            node_id: "bad id".into(),
        };
        let expected = crate::validate_value(&reply, 0).unwrap_err();
        assert_eq!(validate_bound_value(&image, &reply), Err(expected));
        let reply = Value::UiFocus {
            node_id: "a".into(),
        };
        image.max_output_items = 0;
        let error = validate_bound_value(&image, &reply).unwrap_err();
        assert_eq!(error.code, "LSV2102");
        assert_eq!(error.message, "bound result exceeds the output item limit");
    }
}
