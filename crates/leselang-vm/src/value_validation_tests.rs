use super::*;

// Frozen pre-migration traversal; leaf rules remain independently covered.
pub(super) fn legacy_validate_value(value: &Value, depth: usize) -> Result<usize, Fault> {
    if depth > MAX_STRUCTURED_VALUE_DEPTH {
        return Err(Fault {
            code: "LSV2403".into(),
            message: "structured value nesting exceeds runtime bounds".into(),
        });
    }
    match value {
        Value::Structured { fields } if (1..=MAX_MERGE_BRANCHES).contains(&fields.len()) => {
            let mut names = BTreeSet::new();
            let mut output_items = 0usize;
            for field in fields {
                if !valid_merge_branch_name(&field.name) || !names.insert(field.name.as_str()) {
                    return Err(Fault {
                        code: "LSV2403".into(),
                        message: "structured value fields are invalid".into(),
                    });
                }
                output_items = output_items.saturating_add(legacy_validate_value(
                    &field.value,
                    depth.saturating_add(1),
                )?);
            }
            Ok(output_items)
        }
        _ => value
            .bounded_leaf_items(DEFAULT_MAX_OUTPUT_ITEMS)
            .ok_or_else(|| Fault {
                code: "LSV2403".into(),
                message: "structured value exceeds runtime bounds".into(),
            }),
    }
}

fn field(name: &str, value: Value) -> StructuredField {
    StructuredField {
        name: name.into(),
        value: Box::new(value),
    }
}

#[test]
fn value_traversal_matches_frozen_counts_fault_precedence_and_unchanged_bytes() {
    let leaf = |valid| Value::UiFocus {
        node_id: if valid { "a" } else { "bad node" }.into(),
    };
    let mut cases = vec![
        leaf(true),
        leaf(false),
        Value::Structured { fields: vec![] },
        Value::Scalar {
            value: ScalarValue::StringList(leselang_hir::computation::StringListValue(vec![])),
        },
        Value::RuntimeList {
            revision: Revision(1),
            runtimes: vec![],
        },
        Value::RuntimeHistory {
            revision: Revision(1),
            entries: vec![],
        },
    ];
    for name in ["a", "bad name", ""] {
        for first_valid in [false, true] {
            for second_valid in [false, true] {
                cases.push(Value::Structured {
                    fields: vec![
                        field("a", leaf(first_valid)),
                        field(name, leaf(second_valid)),
                    ],
                });
            }
        }
    }
    for count in [1, MAX_MERGE_BRANCHES, MAX_MERGE_BRANCHES + 1] {
        cases.push(Value::Structured {
            fields: (0..count)
                .map(|index| field(&format!("f{index}"), leaf(true)))
                .collect(),
        });
    }
    let mut nested = leaf(true);
    for _ in 0..=MAX_STRUCTURED_VALUE_DEPTH {
        nested = Value::Structured {
            fields: vec![field("a", nested)],
        };
        cases.push(nested.clone());
    }
    cases.push(Value::Structured {
        fields: vec![
            field(
                "a",
                Value::Structured {
                    fields: vec![field("a", leaf(true))],
                },
            ),
            field(
                "b",
                Value::Structured {
                    fields: vec![field("a", leaf(true))],
                },
            ),
            field(
                "c",
                Value::RuntimeList {
                    revision: Revision(1),
                    runtimes: vec![],
                },
            ),
            field(
                "d",
                Value::Scalar {
                    value: ScalarValue::StringList(leselang_hir::computation::StringListValue(
                        vec!["a".into(), "b".into()],
                    )),
                },
            ),
        ],
    });
    assert_eq!(cases.len(), 39);
    let mut comparisons = 0;
    for (index, value) in cases.iter().enumerate() {
        let before = serde_json::to_vec(value).unwrap();
        for depth in (0..=MAX_STRUCTURED_VALUE_DEPTH + 1).chain([usize::MAX]) {
            assert_eq!(
                validate_value(value, depth),
                legacy_validate_value(value, depth),
                "case {index}, depth {depth}"
            );
            assert_eq!(
                leselang_command::reference_value::validate_value(value, depth),
                legacy_validate_value(value, depth),
                "adapter case {index}, depth {depth}"
            );
            comparisons += 2;
        }
        let outcome = BranchOutcome::Value(value.clone());
        assert_eq!(
            validate_branch_outcome(&outcome),
            legacy_validate_value(value, 2)
        );
        assert_eq!(
            leselang_command::reference_value::validate_branch_outcome(&outcome),
            legacy_validate_value(value, 2)
        );
        assert_eq!(serde_json::to_vec(value).unwrap(), before);
    }
    assert_eq!(comparisons, 1_482);
}
