use super::*;
use crate::{
    Fault, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding,
    ScalarBinding, StructuredField, Value,
};
use leselang_hir::computation::{
    BinaryOperator, Computation, ComputedArgument, ComputedBranch, GroupKind, ScalarValue,
    UnaryOperator,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, HirBranch, Type};
use leselang_runtime_core::Fuel;
use serde_json::json;

#[path = "computation_contract_tests/frozen.rs"]
mod frozen;

fn literal(value: ScalarValue) -> Computation {
    Computation::Literal { value }
}
fn integer(value: u64) -> Computation {
    literal(ScalarValue::Integer(value))
}
fn local(name: &str) -> Computation {
    Computation::Local { name: name.into() }
}
fn field(value: Computation) -> Computation {
    Computation::Field {
        value: Box::new(value),
        field: ResultField::NodeId,
    }
}
fn focus() -> Effect {
    Effect::UiFocus {
        node_id: "original".into(),
    }
}
fn call(value: Computation) -> Computation {
    Computation::Call {
        operation: HostOperation::UiFocus,
        arguments: vec![ComputedArgument {
            name: "node_id".into(),
            value,
        }],
    }
}
fn parse() -> Computation {
    Computation::Unary {
        operator: UnaryOperator::ParseInteger,
        value: Box::new(literal(ScalarValue::String("private-invalid".into()))),
    }
}
fn expressions() -> Vec<Computation> {
    let host = Computation::Host {
        effect: Box::new(focus()),
    };
    let mut cases = vec![
        integer(7),
        Computation::Strings {
            items: vec![literal(ScalarValue::String("a".into()))],
        },
        local("pending"),
        field(local("pending")),
        Computation::Member {
            group: "pending".into(),
            name: "member".into(),
            operation: HostOperation::UiFocus,
        },
        Computation::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(integer(u64::MAX)),
            right: Box::new(integer(1)),
        },
        parse(),
        Computation::Unary {
            operator: UnaryOperator::ParseBoolean,
            value: Box::new(literal(ScalarValue::String("private-invalid".into()))),
        },
        Computation::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(integer(1)),
        },
        Computation::Bind {
            name: "n".into(),
            value: Box::new(integer(3)),
            body: Box::new(local("n")),
        },
        Computation::Bind {
            name: "next".into(),
            value: Box::new(host.clone()),
            body: Box::new(field(local("next"))),
        },
        Computation::Loop {
            name: "n".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(literal(ScalarValue::Boolean(false))),
            next: Box::new(integer(1)),
            limit: 2,
        },
        Computation::Loop {
            name: "n".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(literal(ScalarValue::Boolean(true))),
            next: Box::new(integer(1)),
            limit: 2,
        },
        Computation::Fold {
            name: "n".into(),
            item: "item".into(),
            items: Box::new(Computation::Strings {
                items: vec![literal(ScalarValue::String("a".into()))],
            }),
            initial: Box::new(integer(0)),
            next: Box::new(integer(1)),
            limit: 2,
        },
        Computation::Fold {
            name: "n".into(),
            item: "item".into(),
            items: Box::new(Computation::Strings {
                items: vec![
                    literal(ScalarValue::String("a".into())),
                    literal(ScalarValue::String("b".into())),
                ],
            }),
            initial: Box::new(integer(0)),
            next: Box::new(integer(1)),
            limit: 1,
        },
        Computation::Choose {
            when: Box::new(literal(ScalarValue::Boolean(true))),
            then: Box::new(integer(7)),
            otherwise: Box::new(integer(8)),
        },
        Computation::Recover {
            value: Box::new(parse()),
            fallback: Box::new(integer(7)),
        },
        Computation::Recover {
            value: Box::new(host.clone()),
            fallback: Box::new(integer(7)),
        },
        host,
        call(literal(ScalarValue::String("node".into()))),
        Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![ComputedArgument {
                name: "unknown".into(),
                value: integer(0),
            }],
        },
        call(integer(3)),
    ];
    for group_kind in [GroupKind::Sequence, GroupKind::Parallel] {
        cases.push(Computation::Group {
            group_kind,
            branches: vec![ComputedBranch {
                name: "member".into(),
                value: call(literal(ScalarValue::String("node".into()))),
                result_type: Type::UiFocus,
            }],
        });
    }
    assert_eq!(cases.len(), 24);
    cases
}

fn binding(body: Computation) -> ResultBinding {
    ResultBinding {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "prefix".into(),
            value: ScalarValue::String("original-prefix".into()),
        }],
        results: vec![],
        groups: vec![],
        body,
    }
}
fn replies() -> (Value, Value, Vec<HirBranch>) {
    let raw = Value::UiFocus {
        node_id: "original".into(),
    };
    let group = Value::Structured {
        fields: vec![StructuredField {
            name: "member".into(),
            value: Box::new(raw.clone()),
        }],
    };
    (
        raw,
        group,
        vec![HirBranch {
            name: "member".into(),
            effect: focus(),
            result_type: Type::UiFocus,
        }],
    )
}
fn observe(value: Result<Outcome<'_>, Fault>) -> Result<serde_json::Value, Fault> {
    value.map(|value| match value {
        Outcome::Scalar(value) => json!({"scalar":value}),
        Outcome::Result => json!({"result":true}),
        Outcome::Host(effect) => json!({"host":effect, "borrowed":matches!(effect, std::borrow::Cow::Borrowed(_))}),
        Outcome::BoundHost { effect, binding } => json!({"host":effect, "binding":binding, "borrowed":matches!(effect, std::borrow::Cow::Borrowed(_))}),
    })
}
fn compare(
    frame: &ResultBinding,
    raw: &Value,
    group: &Value,
    branches: &[HirBranch],
    grant: u64,
    entry: usize,
) {
    let mut actual = Fuel::new(grant);
    let mut expected = Fuel::new(grant);
    let (actual_result, expected_result) = match entry {
        0 => (
            observe(evaluate(&frame.body, &mut actual)),
            observe(frozen::evaluate(&frame.body, &mut expected)),
        ),
        1 => (
            observe(resume_outcome(
                frame,
                raw,
                HostOperation::UiFocus,
                &mut actual,
            )),
            observe(frozen::resume_outcome(
                frame,
                raw,
                HostOperation::UiFocus,
                &mut expected,
            )),
        ),
        2 => (
            observe(resume_group_outcome(frame, group, branches, &mut actual)),
            observe(frozen::resume_group_outcome(
                frame,
                group,
                branches,
                &mut expected,
            )),
        ),
        3 => (
            resume(frame, raw, HostOperation::UiFocus, &mut actual).map(|v| json!(v)),
            frozen::resume(frame, raw, HostOperation::UiFocus, &mut expected).map(|v| json!(v)),
        ),
        _ => (
            resume_group(frame, group, branches, &mut actual).map(|v| json!(v)),
            frozen::resume_group(frame, group, branches, &mut expected).map(|v| json!(v)),
        ),
    };
    assert_eq!(
        actual_result, expected_result,
        "entry={entry}, grant={grant}"
    );
    assert_eq!(
        actual.remaining(),
        expected.remaining(),
        "entry={entry}, grant={grant}"
    );
    assert_eq!(
        serde_json::to_vec(&actual_result).unwrap(),
        serde_json::to_vec(&expected_result).unwrap()
    );
}

#[test]
fn product_computation_migration_preserves_all_ir_nodes_five_entries_and_fuel() {
    let (raw, group, branches) = replies();
    let mut comparisons = 0;
    let bodies = expressions();
    let kinds: std::collections::BTreeSet<_> = bodies
        .iter()
        .map(|body| match body {
            Computation::Literal { .. } => "literal",
            Computation::Strings { .. } => "strings",
            Computation::Local { .. } => "local",
            Computation::Field { .. } => "field",
            Computation::Member { .. } => "member",
            Computation::Binary { .. } => "binary",
            Computation::Unary { .. } => "unary",
            Computation::Bind { .. } => "bind",
            Computation::Loop { .. } => "loop",
            Computation::Fold { .. } => "fold",
            Computation::Choose { .. } => "choose",
            Computation::Recover { .. } => "recover",
            Computation::Host { .. } => "host",
            Computation::Call { .. } => "call",
            Computation::Group { .. } => "group",
        })
        .collect();
    assert_eq!(kinds.len(), 15);
    for body in bodies {
        let frame = binding(body);
        let before = serde_json::to_vec(&(&frame, &raw, &group, &branches)).unwrap();
        for grant in 0..=96 {
            for entry in 0..5 {
                compare(&frame, &raw, &group, &branches, grant, entry);
                comparisons += 1;
            }
        }
        assert_eq!(
            serde_json::to_vec(&(&frame, &raw, &group, &branches)).unwrap(),
            before
        );
    }
    assert_eq!(comparisons, 11640);
}

#[test]
fn product_capture_migration_keeps_saved_rows_duplicate_order_and_projection_faults() {
    let (raw, group, branches) = replies();
    let projected = ProjectedResult::capture(HostOperation::UiFocus, &raw).unwrap();
    let mut comparisons = 0;
    for defect in 0..10 {
        let mut raw = raw.clone();
        let mut frame = binding(field(local("pending")));
        match defect {
            0 => {}
            1 => frame.locals.push(frame.locals[0].clone()),
            2 => frame.name = "prefix".into(),
            3..=6 => {
                frame.results.push(ProjectedBinding {
                    name: "saved".into(),
                    result: projected.clone(),
                });
                frame.body = field(local("saved"));
                match defect {
                    4 => frame.results[0].name = "prefix".into(),
                    5 => frame.results[0].result.projection_version = 0,
                    6 => frame.results[0].result.fields.clear(),
                    _ => {}
                }
            }
            7 | 8 => {
                frame.groups.push(ProjectedGroupBinding {
                    name: "saved".into(),
                    group: ProjectedGroup {
                        members: vec![ProjectedBinding {
                            name: "member".into(),
                            result: projected.clone(),
                        }],
                    },
                });
                if defect == 8 {
                    frame.groups[0].name = "prefix".into();
                }
                frame.body = field(Computation::Member {
                    group: "saved".into(),
                    name: "member".into(),
                    operation: HostOperation::UiFocus,
                });
            }
            _ => {
                raw = Value::Scalar {
                    value: ScalarValue::None,
                }
            }
        }
        let before = serde_json::to_vec(&(&frame, &raw)).unwrap();
        for grant in 0..=64 {
            for entry in 1..5 {
                compare(&frame, &raw, &group, &branches, grant, entry);
                comparisons += 1;
            }
        }
        assert_eq!(serde_json::to_vec(&(&frame, &raw)).unwrap(), before);
    }
    assert_eq!(comparisons, 2600);
}
