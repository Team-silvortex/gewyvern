use leselang_command::execution_outcome::ComputationOutcome as Outcome;
use leselang_command::reference_computation::{
    evaluate, resume, resume_group_outcome, resume_outcome,
};
use leselang_command::{ProjectedBinding, ProjectedResult, ResultBinding, StructuredField, Value};
use leselang_runtime_core::{Fuel, ScalarBinding, ScalarValue};
use leserpent_hir::computation::Computation;
use leserpent_hir::host_call::HostOperation;
use leserpent_hir::{Effect, HirBranch, Type};

#[test]
fn facade_borrows_original_effect_and_materializes_capture_without_a_vm_or_new_meter() {
    let expression = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "original".into(),
        }),
    };
    let Computation::Host { effect } = &expression else {
        panic!()
    };
    let mut fuel = Fuel::new(100);
    let Outcome::Host(actual) = evaluate(&expression, &mut fuel).unwrap() else {
        panic!()
    };
    assert!(matches!(actual, std::borrow::Cow::Borrowed(_)));
    assert!(std::ptr::eq(&*actual, &**effect));
    assert_eq!(fuel.remaining(), 99);

    let expression = Computation::Bind {
        name: "prefix".into(),
        value: Box::new(Computation::Literal {
            value: ScalarValue::String("saved".into()),
        }),
        body: Box::new(Computation::Bind {
            name: "reply".into(),
            value: Box::new(expression),
            body: Box::new(Computation::Local {
                name: "prefix".into(),
            }),
        }),
    };
    let Outcome::BoundHost { binding, .. } = evaluate(&expression, &mut fuel).unwrap() else {
        panic!()
    };
    assert!(fuel.remaining() < 100);
    assert_eq!(binding.locals[0].value, ScalarValue::String("saved".into()));
    let before = serde_json::to_vec(&binding).unwrap();
    let reply = Value::UiFocus {
        node_id: "original".into(),
    };
    let remaining = fuel.remaining();
    assert_eq!(
        resume(&binding, &reply, HostOperation::UiFocus, &mut fuel).unwrap(),
        ScalarValue::String("saved".into())
    );
    assert!(fuel.remaining() < remaining);
    assert_eq!(serde_json::to_vec(&binding).unwrap(), before);
}

#[test]
fn facade_keeps_scalar_only_refusal_saved_group_gate_and_zero_budget_failure() {
    let reply = Value::UiFocus {
        node_id: "node".into(),
    };
    let mut binding = ResultBinding {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "prefix".into(),
            value: ScalarValue::Integer(7),
        }],
        results: vec![],
        groups: vec![],
        body: Computation::Host {
            effect: Box::new(Effect::UiFocus {
                node_id: "next".into(),
            }),
        },
    };
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        resume_outcome(&binding, &reply, HostOperation::UiFocus, &mut fuel),
        Ok(Outcome::Host(_))
    ));
    let mut scalar_fuel = Fuel::new(100);
    assert_eq!(
        resume(&binding, &reply, HostOperation::UiFocus, &mut scalar_fuel)
            .unwrap_err()
            .code,
        "LSV1402"
    );
    assert_eq!(scalar_fuel.remaining(), fuel.remaining());

    binding.results.push(ProjectedBinding {
        name: "saved".into(),
        result: ProjectedResult::capture(HostOperation::UiFocus, &reply).unwrap(),
    });
    let grouped = Value::Structured {
        fields: vec![StructuredField {
            name: "member".into(),
            value: Box::new(reply),
        }],
    };
    let branches = [HirBranch {
        name: "member".into(),
        effect: Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
    }];
    let mut fuel = Fuel::new(100);
    let Err(fault) = resume_group_outcome(&binding, &grouped, &branches, &mut fuel) else {
        panic!()
    };
    assert_eq!(fault.code, "LSV1402");
    assert_eq!(fuel.remaining(), 99);
    let mut fuel = Fuel::new(0);
    let Err(fault) = evaluate(
        &Computation::Literal {
            value: ScalarValue::Integer(7),
        },
        &mut fuel,
    ) else {
        panic!()
    };
    assert_eq!(fault.code, "LSV1001");
    assert_eq!(fuel.remaining(), 0);
}
