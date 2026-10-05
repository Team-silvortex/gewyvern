use std::convert::Infallible;

use leselang_runtime_core::{
    HostResultDomain, HostResultError, PendingReply, ReplyAcceptanceError, ReplyAuthority,
};

use crate::{ContinuationImage, Fault, MAX_JOURNAL_ENTRY_BYTES, Value};

struct BoundResultDomain<'a>(&'a ContinuationImage);

impl HostResultDomain<Value> for BoundResultDomain<'_> {
    type Error = Fault;

    fn matches_type(&self, reply: &Value) -> bool {
        crate::group_binding::value_type(reply) == self.0.result_type
    }

    fn validate_value(&self, reply: &Value) -> Result<(), Fault> {
        let items = crate::validate_value(reply, 0)?;
        if items > self.0.max_output_items || self.0.max_output_items == 0 {
            return Err(Fault {
                code: "LSV2102".into(),
                message: "bound result exceeds the output item limit".into(),
            });
        }
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
    let declaration = BoundResultDomain(image);
    let mut pending = PendingReply::new(image.token.as_str(), image, &declaration);
    let accepted = pending
        .try_accept(&image.token.as_str(), value, &ResultObservation)
        .map_err(|rejected| match rejected.error {
            ReplyAcceptanceError::Result(HostResultError::TypeMismatch) => Fault {
                code: "LSV2103".into(),
                message: "effect result does not match pending effect".into(),
            },
            ReplyAcceptanceError::Result(HostResultError::InvalidValue(error)) => error,
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
    use leselang_host_contract::{CapabilitySet, Principal};

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
