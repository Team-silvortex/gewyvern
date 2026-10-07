use leselang_hir::computation::{Computation, ScalarValue, UnaryOperator};
use leselang_hir::ir::GroupKind;
use leselang_hir::native_graph::{NativeGraphLimits, NativeGraphShape, inspect_native_graph};
use leselang_hir::{
    CanonicalSourceError, Effect, HirBranch, Type, authorize, canonical_source, lower,
};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::ScalarType;
use leselang_syntax::parse;

fn focus() -> Effect {
    Effect::UiFocus {
        node_id: "target".into(),
    }
}
fn branch(name: &str, effect: Effect, result_type: Type) -> HirBranch {
    HirBranch {
        name: name.into(),
        effect,
        result_type,
    }
}
fn all() -> Effect {
    Effect::All {
        branches: vec![
            branch("focus", focus(), Type::UiFocus),
            branch(
                "read",
                Effect::RuntimeList {
                    filter: Default::default(),
                },
                Type::RuntimeList,
            ),
        ],
    }
}
fn structural_nodes(effect: &Effect) -> usize {
    inspect_native_graph(
        effect,
        NativeGraphLimits {
            max_nodes: 16 * 1024,
            max_depth: 63,
            max_members: 64,
        },
        |effect| -> Result<_, ()> {
            Ok(match effect {
                Effect::All { branches } => NativeGraphShape::Group {
                    kind: GroupKind::Parallel,
                    members: branches,
                },
                Effect::Sequence { steps } => NativeGraphShape::Group {
                    kind: GroupKind::Sequence,
                    members: steps,
                },
                _ => NativeGraphShape::Leaf,
            })
        },
        |branch: &HirBranch| Ok(&branch.effect),
        |_| Ok(()),
    )
    .unwrap()
    .nodes
}

#[test]
fn existing_nested_native_parallel_profiles_keep_canonical_wire_order_and_capability_checks() {
    let effect = Effect::All {
        branches: vec![
            branch("left", all(), Type::Structured),
            branch("right", all(), Type::Structured),
        ],
    };
    assert_eq!(structural_nodes(&effect), 7);
    let wire = serde_json::to_vec(&effect).unwrap();
    let source = canonical_source(&effect).unwrap();
    let program = lower(&parse(&source)).unwrap();
    assert_eq!(serde_json::to_vec(&program.function.effect).unwrap(), wire);
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["ui.presentation", "runtime.read"]),
    )
    .unwrap();
    assert!(source.find("left:").unwrap() < source.find("right:").unwrap());
}

#[test]
fn physical_native_graph_counts_do_not_certify_names_types_or_private_payload_domains() {
    let mut wrong_type = all();
    let Effect::All { branches } = &mut wrong_type else {
        panic!()
    };
    branches[0].result_type = Type::RuntimeList;
    let mut duplicate = all();
    let Effect::All { branches } = &mut duplicate else {
        panic!()
    };
    branches[1].name = branches[0].name.clone();
    let mut invalid_payload = all();
    let Effect::All { branches } = &mut invalid_payload else {
        panic!()
    };
    branches[0].effect = Effect::UiFocus { node_id: "".into() };
    for effect in [wrong_type, duplicate, invalid_payload] {
        assert_eq!(structural_nodes(&effect), 3);
        assert!(canonical_source(&effect).is_err());
    }
}

#[test]
fn shared_structure_inspection_does_not_enable_nested_exports_or_forbidden_sequence_profiles() {
    let effect = Effect::Sequence {
        steps: vec![branch("parallel", all(), Type::Structured)],
    };
    assert_eq!(structural_nodes(&effect), 4);
    assert!(
        matches!(canonical_source(&effect), Err(CanonicalSourceError::InvalidEffect(errors)) if errors.iter().any(|error| error.code == "LSH1301"))
    );
    let program = lower(&parse(
        r#"fn main() = bind(g: all(left: all(focus: ui.focus(node_id: "a"), read: runtime.list()), right: ui.focus(node_id: "b")), body: field(value: member(value: g, name: "left"), name: "count"))"#,
    ));
    assert!(program.is_err());
}

#[test]
fn compute_leaf_structure_stays_separate_from_its_semantic_type_and_native_graph_budget() {
    let value = Effect::Compute {
        expression: Box::new(Computation::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(Computation::Literal {
                value: ScalarValue::Integer(7),
            }),
        }),
    };
    assert_eq!(structural_nodes(&value), 1);
    assert!(canonical_source(&value).is_err());
    let pure = Effect::Compute {
        expression: Box::new(Computation::Literal {
            value: ScalarValue::Integer(7),
        }),
    };
    let program = lower(&parse(&canonical_source(&pure).unwrap())).unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    assert_eq!(program.function.effect, pure);
}

#[test]
fn graph_arity_and_depth_keep_legacy_diagnostics_before_source_formatting() {
    for effect in [
        Effect::All { branches: vec![] },
        Effect::Sequence { steps: vec![] },
        Effect::All {
            branches: vec![branch("one", focus(), Type::UiFocus)],
        },
    ] {
        assert!(
            matches!(canonical_source(&effect), Err(CanonicalSourceError::InvalidEffect(errors)) if errors.len() == 1 && errors[0].code == "LSH1201" && errors[0].span.is_none())
        );
    }
    let mut effect = focus();
    for _ in 0..leselang_hir::MAX_EFFECT_NESTING_DEPTH {
        effect = Effect::Sequence {
            steps: vec![branch("child", effect, Type::Structured)],
        };
    }
    assert!(
        matches!(canonical_source(&effect), Err(CanonicalSourceError::InvalidEffect(errors)) if errors.len() == 1 && errors[0].code == "LSH1204" && errors[0].span.is_none())
    );
}
