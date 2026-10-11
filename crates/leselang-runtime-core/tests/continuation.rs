use std::{cell::Cell, rc::Rc};

use leselang_runtime_core::{
    ContinuationFrame, ResourceBudget, ResourceBudgetError, ResourceBudgetLimits,
};
use serde::{Deserialize, Serialize};

const LIMITS: ResourceBudgetLimits = ResourceBudgetLimits {
    max_fuel: 10,
    max_deadline_ms: 20,
    max_deadline_at_ms: 30,
    max_output_items: 40,
};
fn budget() -> ResourceBudget {
    ResourceBudget {
        fuel_remaining: 10,
        deadline_ms: 20,
        deadline_at_ms: Some(30),
        max_output_items: 40,
    }
}
type Frame = ContinuationFrame<String, u64, String, Vec<u8>, String>;
fn frame() -> Frame {
    Frame {
        schema_version: 1,
        token: "local-1".into(),
        program_counter: 1,
        expected_revision: Some(7),
        result_type: "native".into(),
        pending_effect: vec![2],
        fuel_remaining: 10,
        deadline_ms: 20,
        deadline_at_ms: None,
        max_output_items: 40,
        result_binding: None,
        group_result: None,
    }
}

#[test]
fn legacy_frame_field_order_omission_and_optional_defaults_are_byte_exact() {
    let frame = frame();
    let bytes = serde_json::to_vec(&frame).unwrap();
    assert_eq!(bytes, br#"{"schema_version":1,"token":"local-1","program_counter":1,"expected_revision":7,"result_type":"native","pending_effect":[2],"fuel_remaining":10,"deadline_ms":20,"max_output_items":40}"#);
    assert_eq!(serde_json::from_slice::<Frame>(&bytes).unwrap(), frame);
    let mut populated = frame;
    populated.deadline_at_ms = Some(30);
    populated.result_binding = Some(Box::new("capture".into()));
    populated.group_result = Some("local-group".into());
    assert_eq!(serde_json::to_vec(&populated).unwrap(), br#"{"schema_version":1,"token":"local-1","program_counter":1,"expected_revision":7,"result_type":"native","pending_effect":[2],"fuel_remaining":10,"deadline_ms":20,"deadline_at_ms":30,"max_output_items":40,"result_binding":"capture","group_result":"local-group"}"#);
}

#[test]
fn missing_required_fields_and_unknown_frame_or_budget_fields_are_rejected() {
    let mut value = serde_json::to_value(frame()).unwrap();
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<Frame>(value).is_err());
    for field in [
        "schema_version",
        "token",
        "program_counter",
        "result_type",
        "pending_effect",
        "fuel_remaining",
        "deadline_ms",
        "max_output_items",
    ] {
        let mut value = serde_json::to_value(frame()).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<Frame>(value).is_err(),
            "missing {field}"
        );
    }
    let mut value = serde_json::to_value(budget()).unwrap();
    value["unknown"] = true.into();
    assert!(serde_json::from_value::<ResourceBudget>(value).is_err());
    assert_eq!(
        serde_json::to_string(&ResourceBudget {
            deadline_at_ms: None,
            ..budget()
        })
        .unwrap(),
        r#"{"fuel_remaining":10,"deadline_ms":20,"max_output_items":40}"#
    );
}

#[test]
fn inclusive_budget_limits_preserve_original_rejection_precedence_without_mutation() {
    assert_eq!(budget().validate(LIMITS), Ok(()));
    for (candidate, expected) in [
        (
            ResourceBudget {
                fuel_remaining: 11,
                deadline_ms: 0,
                deadline_at_ms: Some(0),
                max_output_items: 41,
            },
            ResourceBudgetError::Fuel,
        ),
        (
            ResourceBudget {
                deadline_ms: 0,
                deadline_at_ms: Some(0),
                max_output_items: 41,
                ..budget()
            },
            ResourceBudgetError::Deadline,
        ),
        (
            ResourceBudget {
                deadline_ms: 21,
                ..budget()
            },
            ResourceBudgetError::Deadline,
        ),
        (
            ResourceBudget {
                deadline_at_ms: Some(0),
                max_output_items: 41,
                ..budget()
            },
            ResourceBudgetError::AbsoluteDeadline,
        ),
        (
            ResourceBudget {
                deadline_at_ms: Some(31),
                ..budget()
            },
            ResourceBudgetError::AbsoluteDeadline,
        ),
        (
            ResourceBudget {
                max_output_items: 41,
                ..budget()
            },
            ResourceBudgetError::Output,
        ),
    ] {
        let original = serde_json::to_vec(&candidate).unwrap();
        assert_eq!(candidate.validate(LIMITS), Err(expected));
        assert_eq!(serde_json::to_vec(&candidate).unwrap(), original);
    }
}

#[test]
fn zero_ceilings_and_full_width_budget_values_are_never_repaired_or_saturated() {
    let zero = ResourceBudgetLimits {
        max_fuel: 0,
        max_deadline_ms: 1,
        max_deadline_at_ms: 0,
        max_output_items: 0,
    };
    let budget = ResourceBudget {
        fuel_remaining: 0,
        deadline_ms: 1,
        deadline_at_ms: None,
        max_output_items: 0,
    };
    assert!(budget.validate(zero).is_ok());
    assert_eq!(
        budget.validate(ResourceBudgetLimits {
            max_deadline_ms: 0,
            ..zero
        }),
        Err(ResourceBudgetError::Deadline)
    );
    assert_eq!(
        ResourceBudget {
            deadline_at_ms: Some(1),
            ..budget
        }
        .validate(zero),
        Err(ResourceBudgetError::AbsoluteDeadline)
    );
    let all = ResourceBudget {
        fuel_remaining: u64::MAX,
        deadline_ms: u64::MAX,
        deadline_at_ms: Some(u64::MAX),
        max_output_items: usize::MAX,
    };
    assert!(
        all.validate(ResourceBudgetLimits {
            max_fuel: u64::MAX,
            max_deadline_ms: u64::MAX,
            max_deadline_at_ms: u64::MAX,
            max_output_items: usize::MAX
        })
        .is_ok()
    );
}

struct Native {
    drops: Rc<Cell<usize>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
#[test]
fn opaque_gui_local_native_slots_move_without_clone_debug_serde_or_default() {
    let drops = Rc::new(Cell::new(0));
    let native = || Native {
        drops: drops.clone(),
    };
    let frame = ContinuationFrame {
        schema_version: 1,
        token: native(),
        program_counter: 1,
        expected_revision: Some(native()),
        result_type: native(),
        pending_effect: native(),
        fuel_remaining: 0,
        deadline_ms: 1,
        deadline_at_ms: None,
        max_output_items: 0,
        result_binding: Some(Box::new(native())),
        group_result: Some(native()),
    };
    let observed = frame.resource_budget();
    assert_eq!(observed.fuel_remaining, 0);
    assert_eq!(drops.get(), 0);
    assert!(
        observed
            .validate(ResourceBudgetLimits {
                max_fuel: 0,
                max_deadline_ms: 1,
                max_deadline_at_ms: 0,
                max_output_items: 0
            })
            .is_ok()
    );
    assert_eq!(drops.get(), 0);
    drop(frame);
    assert_eq!(drops.get(), 6);
}

#[derive(Deserialize, Serialize)]
struct NoDefault(u8);
#[test]
fn optional_native_slots_decode_without_accidentally_requiring_default() {
    type NativeFrame = ContinuationFrame<NoDefault, NoDefault, NoDefault, NoDefault, NoDefault>;
    let frame: NativeFrame = serde_json::from_str(r#"{"schema_version":1,"token":1,"program_counter":1,"result_type":2,"pending_effect":3,"fuel_remaining":0,"deadline_ms":1,"max_output_items":0}"#).unwrap();
    assert!(frame.expected_revision.is_none());
    assert!(frame.result_binding.is_none());
    assert!(frame.group_result.is_none());
}
