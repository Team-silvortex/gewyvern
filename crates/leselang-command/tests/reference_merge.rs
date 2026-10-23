use leselang_command::reference_merge::{
    group_binding_admission_fault, group_binding_admission_limits, merge_plan_admission_fault,
    merge_plan_admission_limits, validate_group_binding, validate_merge_plan,
    validate_plan_encoding,
};
use leselang_command::{
    ExecutionOrder, GroupBindingError, GroupResultBinding, MAX_CONTINUATION_BYTES,
    MAX_MERGE_BRANCH_NAME_BYTES, MAX_MERGE_BRANCHES, MergePlan, MergePlanAdmissionError,
    MergePlanShapeError, ResultBinding,
};
use leselang_hir::computation::Computation;
use leselang_hir::{Effect, HirBranch, Type};
use leselang_runtime_core::{Fault, ScalarValue};

fn plan() -> MergePlan {
    MergePlan {
        branches: vec!["a".into(), "b".into()],
        order: ExecutionOrder::BoundParallel,
        result_binding: Some(Box::new(GroupResultBinding {
            pending: Effect::All {
                branches: ["a", "b"]
                    .into_iter()
                    .map(|name| HirBranch {
                        name: name.into(),
                        result_type: Type::UiFocus,
                        effect: Effect::UiFocus {
                            node_id: name.into(),
                        },
                    })
                    .collect(),
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
            fuel_remaining: 100,
            successor_sequence: None,
            additional_successor_sequences: vec![],
        })),
    }
}

fn node_id(plan: &mut MergePlan) -> &mut String {
    let Effect::All { branches } = &mut plan.result_binding.as_mut().unwrap().pending else {
        panic!()
    };
    let Effect::UiFocus { node_id } = &mut branches[0].effect else {
        panic!()
    };
    node_id
}

fn refused(plan: &MergePlan, code: &str) {
    let before = serde_json::to_vec(plan).unwrap();
    let fault = validate_merge_plan(plan).unwrap_err();
    assert_eq!(fault.code, code);
    assert!(!fault.message.contains("private-payload"));
    assert_eq!(serde_json::to_vec(plan).unwrap(), before);
}

#[test]
fn released_merge_limits_and_bound_admission_need_no_execution_owner() {
    let limits = merge_plan_admission_limits();
    assert_eq!(limits.shape.max_branches, MAX_MERGE_BRANCHES);
    assert_eq!(limits.shape.max_name_bytes, MAX_MERGE_BRANCH_NAME_BYTES);
    assert_eq!(limits.group, group_binding_admission_limits());
    assert_eq!(limits.group.plan.max_fuel, 1_000_000);
    assert_eq!(
        limits.group.plan.max_effect_sequence,
        leselang_command::continuation_identity::MAX_EFFECT_SEQUENCE
    );
    assert_eq!(limits.group.plan.max_branches, MAX_MERGE_BRANCHES);
    assert_eq!(
        limits.group.binding,
        leselang_command::reference_request::result_binding_limits()
    );
    let plan = plan();
    let bytes = serde_json::to_vec(&plan).unwrap();
    assert_eq!(validate_merge_plan(&plan), Ok(()));
    assert!(matches!(
        validate_group_binding(plan.result_binding.as_ref().unwrap(), &plan),
        Ok(Type::Scalar(_))
    ));
    assert_eq!(serde_json::to_vec(&plan).unwrap(), bytes);
    assert_eq!(
        validate_merge_plan(&MergePlan::new(["a", "b"]).unwrap()),
        Ok(())
    );
}

#[test]
fn shape_and_binding_fault_order_remains_released_and_redacted() {
    let mut plan = plan();
    *node_id(&mut plan) = "private-payload".repeat(MAX_CONTINUATION_BYTES);
    plan.result_binding.as_mut().unwrap().binding.body = Computation::Local {
        name: "missing".into(),
    };
    refused(&plan, "LSV3002");
    plan.result_binding.as_mut().unwrap().fuel_remaining = 1_000_001;
    refused(&plan, "LSV1409");
    plan.result_binding.as_mut().unwrap().binding.name = "private-payload".repeat(10);
    refused(&plan, "LSV1405");
    plan.branches.clear();
    refused(&plan, "LSV2401");
    plan.order = ExecutionOrder::ResultChain;
    refused(&plan, "LSV1409");
    plan.result_binding = None;
    refused(&plan, "LSV1407");
    plan.order = ExecutionOrder::Parallel;
    refused(&plan, "LSV2401");
    // Decoding remains data construction, not plan admission.
    assert!(serde_json::from_str::<MergePlan>(r#"{"branches":[]}"#).is_ok());
}

#[test]
fn group_plan_size_limit_is_inclusive_and_structure_precedes_encoding() {
    let mut plan = plan();
    let overhead = serde_json::to_vec(&plan).unwrap().len();
    *node_id(&mut plan) = "x".repeat(MAX_CONTINUATION_BYTES - overhead + 1);
    assert_eq!(
        serde_json::to_vec(&plan).unwrap().len(),
        MAX_CONTINUATION_BYTES
    );
    assert_eq!(validate_plan_encoding(&plan), Ok(()));
    // Encoding fits, but a giant node ID still fails the later semantic gate.
    refused(&plan, "LSV1405");
    node_id(&mut plan).push('x');
    assert_eq!(
        validate_plan_encoding(&plan).unwrap_err(),
        Fault {
            code: "LSV3002".into(),
            message: format!("group-result plan exceeds {MAX_CONTINUATION_BYTES} bytes"),
        }
    );
    refused(&plan, "LSV3002");
    let binding = &mut plan.result_binding.as_mut().unwrap().binding;
    for _ in 0..(leselang_hir::MAX_EFFECT_NESTING_DEPTH + 1) {
        binding.body = Computation::Bind {
            name: "nested".into(),
            value: Box::new(Computation::Literal {
                value: ScalarValue::None,
            }),
            body: Box::new(std::mem::replace(
                &mut binding.body,
                Computation::Literal {
                    value: ScalarValue::None,
                },
            )),
        };
    }
    refused(&plan, "LSV1405");
}

#[test]
fn diagnostic_mapping_keeps_native_errors_and_exact_product_vocabulary() {
    let native = Fault {
        code: "NATIVE".into(),
        message: "native-owned".into(),
    };
    assert_eq!(
        group_binding_admission_fault(GroupBindingError::Encoding(native.clone())),
        native
    );
    assert_eq!(
        merge_plan_admission_fault(MergePlanAdmissionError::Binding(
            GroupBindingError::Encoding(native.clone())
        )),
        native
    );
    for (error, code, message) in [
        (
            MergePlanAdmissionError::Shape(MergePlanShapeError::GroupBinding),
            "LSV1409",
            "invalid group-result binding or missing complete group journal",
        ),
        (
            MergePlanAdmissionError::Shape(MergePlanShapeError::Successor),
            "LSV1407",
            "invalid result-driven successor or missing original effect authority",
        ),
        (
            MergePlanAdmissionError::Shape(MergePlanShapeError::Branches),
            "LSV2401",
            "structured merge plan is invalid or exceeds runtime bounds",
        ),
    ] {
        assert_eq!(
            merge_plan_admission_fault(error),
            Fault {
                code: code.into(),
                message: message.into()
            }
        );
    }
    assert_eq!(
        group_binding_admission_fault(GroupBindingError::Binding),
        leselang_command::reference_request::continuation_admission_fault(
            leselang_command::ContinuationAdmissionError::Binding
        )
    );
}
