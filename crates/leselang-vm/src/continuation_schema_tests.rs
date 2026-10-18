use super::*;

// Frozen pre-migration schema selection and frame admission, including numeric wire tags.
fn frozen_binding_schema(binding: &ResultBinding) -> u32 {
    if !binding.groups.is_empty() {
        if binding.body.is_pure() {
            8
        } else if binding.body.is_result_chain() {
            10
        } else {
            11
        }
    } else if !binding.body.is_pure() && !binding.body.is_result_chain() {
        5
    } else if !binding.results.is_empty()
        || (!binding.body.is_pure() && !binding.body.is_atomic_tail())
    {
        4
    } else if binding.body.is_pure() {
        2
    } else {
        3
    }
}

#[allow(
    clippy::manual_range_patterns,
    reason = "Frozen reference keeps each released version explicit."
)]
fn frozen_validate_image(image: &ContinuationImage) -> Result<(), Fault> {
    if let Some(binding) = &image.result_binding {
        binding.validate_structure()?;
    }
    let expected_version = if let Some(owner) = &image.group_result {
        let sequence = owner
            .as_str()
            .strip_prefix("merge-")
            .and_then(|value| value.parse::<u64>().ok());
        if image.result_binding.as_ref().is_some_and(|binding| {
            !matches!(image.schema_version, 8 | 9 | 10 | 11)
                || binding.groups.is_empty()
                || !(binding.body.is_pure()
                    || (image.schema_version == 10 && binding.body.is_result_chain())
                    || (image.schema_version == 11 && binding.body.is_result_flow()))
        }) || sequence.is_none_or(|value| {
            value == 0 || value > MAX_EFFECT_SEQUENCE || owner.as_str() != format!("merge-{value}")
        }) {
            return Err(group_binding::invalid());
        }
        match image.schema_version {
            11 => 11,
            10 => 10,
            9 => 9,
            8 => 8,
            7 => 7,
            _ => 6,
        }
    } else if let Some(binding) = &image.result_binding {
        if !binding.groups.is_empty() {
            return Err(group_binding::invalid());
        }
        frozen_binding_schema(binding)
    } else {
        1
    };
    if image.schema_version != expected_version {
        return Err(Fault {
            code: "LSV2001".to_string(),
            message: format!(
                "unsupported continuation version {}, expected {}",
                image.schema_version, expected_version
            ),
        });
    }
    if !valid_continuation_token(&image.token) {
        return Err(Fault {
            code: "LSV2006".to_string(),
            message: "invalid continuation token".to_string(),
        });
    }
    if image.program_counter != 1 {
        return Err(Fault {
            code: "LSV2014".to_string(),
            message: "continuation program counter is unsupported".to_string(),
        });
    }
    let expected_type = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .map(|operation| operation.result_type())
        .ok_or_else(|| Fault {
            code: "LSV2015".to_string(),
            message: "structured control flow cannot be restored as a single continuation"
                .to_string(),
        })?;
    if image.result_type != expected_type {
        return Err(Fault {
            code: "LSV2016".to_string(),
            message: "continuation result type does not match its pending effect".to_string(),
        });
    }
    if let Some(binding) = &image.result_binding {
        binding.validate(&image.pending_effect)?;
    }
    image
        .resource_budget()
        .validate(ResourceBudgetLimits {
            max_fuel: MAX_EXECUTION_FUEL,
            max_deadline_ms: MAX_EFFECT_DEADLINE_MS,
            max_deadline_at_ms: i64::MAX as u64,
            max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        })
        .map_err(|error| {
            let (code, message) = match error {
                ResourceBudgetError::Fuel => ("LSV2007", "continuation fuel exceeds runtime limit"),
                ResourceBudgetError::Deadline => {
                    ("LSV2008", "continuation deadline exceeds runtime limit")
                }
                ResourceBudgetError::AbsoluteDeadline => {
                    ("LSV2011", "continuation absolute deadline is out of range")
                }
                ResourceBudgetError::Output => (
                    "LSV2009",
                    "continuation output limit exceeds runtime maximum",
                ),
            };
            Fault {
                code: code.to_string(),
                message: message.to_string(),
            }
        })?;
    Ok(())
}

fn bindings() -> Vec<Option<ResultBinding>> {
    use leselang_hir::{computation::Computation, host_call::HostOperation};
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let chain = Computation::Bind {
        name: "next_reply".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let flow = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(chain.clone()),
        otherwise: Box::new(pure.clone()),
    };
    let saved = ProjectedBinding {
        name: "saved".into(),
        result: ProjectedResult::capture(
            HostOperation::UiFocus,
            &Value::UiFocus {
                node_id: "saved".into(),
            },
        )
        .unwrap(),
    };
    let mut bindings = vec![None];
    for body in [pure, host, chain, flow] {
        for results in [vec![], vec![saved.clone()]] {
            for groups in [
                vec![],
                vec![ProjectedGroupBinding {
                    name: "saved_group".into(),
                    group: ProjectedGroup {
                        members: vec![saved.clone()],
                    },
                }],
            ] {
                bindings.push(Some(ResultBinding {
                    name: "reply".into(),
                    locals: vec![],
                    results: results.clone(),
                    groups,
                    body: body.clone(),
                }));
            }
        }
    }
    bindings
}

#[test]
fn continuation_schema_preserves_frozen_selection_and_frame_refusal_order() {
    let base = crate::continuation_contract_tests::requests()
        .remove(9)
        .continuation;
    let owners = [
        None,
        Some("merge-1".to_owned()),
        Some("merge-0".to_owned()),
        Some("merge-01".to_owned()),
        Some("merge-+1".to_owned()),
        Some("merge--1".to_owned()),
        Some("merge- 1".to_owned()),
        Some("foreign-1".to_owned()),
        Some("merge-18446744073709551616".to_owned()),
        Some(format!("merge-{MAX_EFFECT_SEQUENCE}")),
        Some(format!("merge-{}", MAX_EFFECT_SEQUENCE + 1)),
        Some(String::new()),
    ];
    let mut comparisons = 0;
    let mut accepted = 0;
    for binding in bindings() {
        if let Some(binding) = &binding {
            assert_eq!(binding.schema_version(), frozen_binding_schema(binding));
        }
        for owner in &owners {
            for version in (0..=12).chain([u32::MAX]) {
                for defect in 0..8 {
                    let mut image = base.clone();
                    image.result_binding = binding.clone().map(Box::new);
                    image.group_result = owner.clone().map(ContinuationToken);
                    image.schema_version = version;
                    match defect {
                        0 => {}
                        1 => image.token = ContinuationToken("invalid".into()),
                        2 => image.program_counter = 0,
                        3 => image.result_type = Type::RuntimeList,
                        4 => image.fuel_remaining = MAX_EXECUTION_FUEL + 1,
                        5 => image.deadline_ms = 0,
                        6 => {
                            image.token = ContinuationToken("invalid".into());
                            image.result_type = Type::Structured;
                            image.fuel_remaining = MAX_EXECUTION_FUEL + 1;
                        }
                        7 => {
                            if let Some(binding) = &mut image.result_binding {
                                binding.name = "not a local".into();
                            }
                            image.schema_version = u32::MAX;
                        }
                        _ => unreachable!(),
                    }
                    let before = serde_json::to_vec(&image).unwrap();
                    let expected = frozen_validate_image(&image);
                    accepted += usize::from(expected.is_ok());
                    assert_eq!(
                        validate_image(&image),
                        expected,
                        "version={version}, owner={owner:?}, defect={defect}, binding={binding:?}"
                    );
                    assert_eq!(serde_json::to_vec(&image).unwrap(), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 22_848);
    assert!(accepted > 0);
}

#[test]
fn continuation_structure_precedes_schema_even_for_oversized_body() {
    use leselang_hir::computation::Computation;
    let mut image = crate::continuation_contract_tests::requests()
        .remove(9)
        .continuation;
    let mut binding = bindings().remove(1).unwrap();
    for _ in 0..=leselang_hir::MAX_EFFECT_NESTING_DEPTH {
        binding.body = Computation::Bind {
            name: "nested".into(),
            value: Box::new(Computation::Literal {
                value: ScalarValue::None,
            }),
            body: Box::new(binding.body),
        };
    }
    image.result_binding = Some(Box::new(binding));
    image.schema_version = u32::MAX;
    image.group_result = Some(ContinuationToken("invalid".into()));
    assert_eq!(validate_image(&image), Err(result_binding::invalid()));
    assert_eq!(validate_image(&image), frozen_validate_image(&image));
}
