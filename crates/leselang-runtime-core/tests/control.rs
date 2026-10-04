use leselang_runtime_core::{
    BinaryOperator, BinarySelection, LoopBudget, LoopError, LoopStep, MAX_LOOP_ITERATIONS,
    OptionalStringValue, ScalarError, ScalarType, ScalarValue, StringListValue, apply_binary,
    select_binary_left,
};

fn samples() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(1),
        ScalarValue::Boolean(true),
        ScalarValue::String("x".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("x".into()))),
        ScalarValue::StringList(StringListValue(vec!["x".into()])),
    ]
}

#[test]
fn boolean_selection_retains_exact_truth_tables_without_coercion() {
    for operator in [BinaryOperator::And, BinaryOperator::Or] {
        for left in [false, true] {
            for right in [false, true] {
                let selection = select_binary_left(operator, ScalarValue::Boolean(left)).unwrap();
                let actual = match selection {
                    BinarySelection::Complete(value) => value,
                    BinarySelection::NeedsRight(left) => {
                        apply_binary(operator, left, ScalarValue::Boolean(right)).unwrap()
                    }
                };
                assert_eq!(
                    actual,
                    ScalarValue::Boolean(if operator == BinaryOperator::And {
                        left && right
                    } else {
                        left || right
                    })
                );
            }
            let selection = select_binary_left(operator, ScalarValue::Boolean(left)).unwrap();
            assert_eq!(
                matches!(selection, BinarySelection::Complete(_)),
                (operator == BinaryOperator::And && !left)
                    || (operator == BinaryOperator::Or && left)
            );
        }
    }
    for value in samples() {
        for operator in [
            BinaryOperator::And,
            BinaryOperator::Or,
            BinaryOperator::ValueOr,
        ] {
            let accepts = match operator {
                BinaryOperator::ValueOr => value.scalar_type() == ScalarType::OptionalString,
                _ => value.scalar_type() == ScalarType::Boolean,
            };
            if !accepts {
                assert_eq!(
                    select_binary_left(operator, value.clone()),
                    Err(ScalarError::TypeMismatch)
                );
            }
        }
    }
}

#[test]
fn present_optional_text_is_bounded_and_moved_without_a_second_copy() {
    for original in [String::new(), "chosen".into(), "\u{1f600}".repeat(1024)] {
        let pointer = original.as_ptr();
        let length = original.len();
        let selection = select_binary_left(
            BinaryOperator::ValueOr,
            ScalarValue::OptionalString(OptionalStringValue(Some(original))),
        )
        .unwrap();
        let BinarySelection::Complete(ScalarValue::String(chosen)) = selection else {
            panic!("expected present text")
        };
        assert_eq!(chosen.as_ptr(), pointer);
        assert_eq!(chosen.len(), length);
        assert!(ScalarValue::String(chosen).is_bounded());
    }
    assert_eq!(
        select_binary_left(
            BinaryOperator::ValueOr,
            ScalarValue::OptionalString(OptionalStringValue(Some("x".repeat(4097))))
        ),
        Err(ScalarError::UnboundedOperand)
    );
    let absent = ScalarValue::OptionalString(OptionalStringValue(None));
    assert_eq!(
        select_binary_left(BinaryOperator::ValueOr, absent.clone()),
        Ok(BinarySelection::NeedsRight(absent))
    );
}

#[test]
fn a_deferred_left_value_is_not_a_signature_or_boundedness_certificate() {
    let original = "x".repeat(4097);
    let pointer = original.as_ptr();
    let BinarySelection::NeedsRight(left) =
        select_binary_left(BinaryOperator::Eq, ScalarValue::String(original)).unwrap()
    else {
        panic!("equality must not short circuit")
    };
    assert_eq!(left.text().unwrap().as_ptr(), pointer);
    assert_eq!(
        apply_binary(BinaryOperator::Eq, left, ScalarValue::String("".into())),
        Err(ScalarError::UnboundedOperand)
    );
    for operator in [
        BinaryOperator::Add,
        BinaryOperator::Sub,
        BinaryOperator::Mul,
        BinaryOperator::Div,
        BinaryOperator::Rem,
        BinaryOperator::Eq,
        BinaryOperator::Ne,
        BinaryOperator::Lt,
        BinaryOperator::Le,
        BinaryOperator::Gt,
        BinaryOperator::Ge,
        BinaryOperator::Concat,
        BinaryOperator::Contains,
        BinaryOperator::StartsWith,
        BinaryOperator::EndsWith,
        BinaryOperator::CharAt,
        BinaryOperator::Split,
        BinaryOperator::Join,
        BinaryOperator::Append,
        BinaryOperator::ItemAt,
    ] {
        for value in samples() {
            assert_eq!(
                select_binary_left(operator, value.clone()),
                Ok(BinarySelection::NeedsRight(value))
            );
        }
    }
    // The eager API still validates a supplied right value, even on a lazy operator.
    assert_eq!(
        apply_binary(
            BinaryOperator::And,
            ScalarValue::Boolean(false),
            ScalarValue::Integer(0)
        ),
        Err(ScalarError::TypeMismatch)
    );
}

#[test]
fn loop_limit_preflight_accepts_zero_and_maximum_but_not_unbounded_limits() {
    assert_eq!(MAX_LOOP_ITERATIONS, 1024);
    for limit in [0, 1, MAX_LOOP_ITERATIONS] {
        assert_eq!(
            LoopBudget::new(ScalarType::Integer, limit)
                .unwrap()
                .completed_iterations(),
            0
        );
    }
    for limit in [MAX_LOOP_ITERATIONS + 1, u64::MAX - 1, u64::MAX] {
        assert!(matches!(
            LoopBudget::new(ScalarType::Integer, limit),
            Err(LoopError::InvalidLimit)
        ));
    }
}

#[test]
fn a_zero_limit_still_permits_condition_first_exit_but_never_an_advance() {
    let mut done = LoopBudget::new(ScalarType::Integer, 0).unwrap();
    assert_eq!(done.check_condition(false), Ok(LoopStep::Done));
    assert_eq!(done.check_condition(true), Err(LoopError::InvalidPhase));
    assert_eq!(
        done.advance(&ScalarValue::Integer(0)),
        Err(LoopError::InvalidPhase)
    );
    assert_eq!(done.completed_iterations(), 0);
    let mut exhausted = LoopBudget::new(ScalarType::Integer, 0).unwrap();
    assert_eq!(
        exhausted.check_condition(true),
        Err(LoopError::IterationLimit)
    );
    assert_eq!(
        exhausted.check_condition(false),
        Err(LoopError::InvalidPhase)
    );
    assert_eq!(
        exhausted.advance(&ScalarValue::Integer(0)),
        Err(LoopError::InvalidPhase)
    );
    assert_eq!(exhausted.completed_iterations(), 0);
}

#[test]
fn reaching_the_limit_requires_one_last_condition_without_truncating_state() {
    for limit in [1, 2, 17, MAX_LOOP_ITERATIONS] {
        for final_condition in [false, true] {
            let mut budget = LoopBudget::new(ScalarType::Integer, limit).unwrap();
            for iteration in 0..limit {
                assert_eq!(budget.check_condition(true), Ok(LoopStep::Continue));
                assert_eq!(budget.completed_iterations(), iteration);
                budget
                    .advance(&ScalarValue::Integer(iteration + 1))
                    .unwrap();
            }
            assert_eq!(budget.completed_iterations(), limit);
            assert_eq!(
                budget.check_condition(final_condition),
                if final_condition {
                    Err(LoopError::IterationLimit)
                } else {
                    Ok(LoopStep::Done)
                }
            );
            assert_eq!(budget.check_condition(false), Err(LoopError::InvalidPhase));
            assert_eq!(
                budget.advance(&ScalarValue::Integer(0)),
                Err(LoopError::InvalidPhase)
            );
            assert_eq!(budget.completed_iterations(), limit);
        }
    }
}

#[test]
fn ordering_errors_never_consume_or_reset_an_iteration() {
    let mut budget = LoopBudget::new(ScalarType::Integer, 2).unwrap();
    assert_eq!(
        budget.advance(&ScalarValue::Integer(1)),
        Err(LoopError::InvalidPhase)
    );
    assert_eq!(budget.check_condition(true), Ok(LoopStep::Continue));
    assert_eq!(budget.check_condition(false), Err(LoopError::InvalidPhase));
    assert_eq!(budget.completed_iterations(), 0);
    budget.advance(&ScalarValue::Integer(1)).unwrap();
    assert_eq!(
        budget.advance(&ScalarValue::Integer(2)),
        Err(LoopError::InvalidPhase)
    );
    assert_eq!(budget.completed_iterations(), 1);
    assert_eq!(budget.check_condition(false), Ok(LoopStep::Done));
}

#[test]
fn every_state_type_is_preserved_without_storing_state_payloads() {
    for state in samples() {
        let mut budget = LoopBudget::new(state.scalar_type(), 1).unwrap();
        budget.check_condition(true).unwrap();
        for wrong in samples() {
            if wrong.scalar_type() != state.scalar_type() {
                assert_eq!(budget.advance(&wrong), Err(LoopError::StateTypeMismatch));
                assert_eq!(budget.completed_iterations(), 0);
            }
        }
        budget.advance(&state).unwrap();
        assert_eq!(budget.completed_iterations(), 1);
        assert_eq!(budget.check_condition(false), Ok(LoopStep::Done));
    }
    let mut budget = LoopBudget::new(ScalarType::String, 1).unwrap();
    budget.check_condition(true).unwrap();
    budget
        .advance(&ScalarValue::String("private-token".into()))
        .unwrap();
    assert!(!format!("{budget:?}").contains("private-token"));
}

#[test]
fn unbounded_next_states_leave_counter_and_pending_advance_unchanged() {
    for (invalid, valid) in [
        (
            ScalarValue::String("x".repeat(4097)),
            ScalarValue::String("".into()),
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("x".repeat(4097)))),
            ScalarValue::OptionalString(OptionalStringValue(None)),
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["".into(); 65])),
            ScalarValue::StringList(StringListValue(vec![])),
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["x".repeat(4096), "x".into()])),
            ScalarValue::StringList(StringListValue(vec![])),
        ),
    ] {
        let mut budget = LoopBudget::new(invalid.scalar_type(), 1).unwrap();
        assert_eq!(budget.check_condition(true), Ok(LoopStep::Continue));
        assert_eq!(budget.advance(&invalid), Err(LoopError::UnboundedState));
        assert_eq!(budget.completed_iterations(), 0);
        // Correcting native input explicitly is not replay or recovery of an expression.
        budget.advance(&valid).unwrap();
        assert_eq!(budget.completed_iterations(), 1);
    }
}

#[test]
fn nested_budgets_keep_independent_counters_and_can_move_between_workers() {
    let mut outer = LoopBudget::new(ScalarType::Integer, 2).unwrap();
    outer.check_condition(true).unwrap();
    let inner = LoopBudget::new(ScalarType::Boolean, 1).unwrap();
    let inner = std::thread::spawn(move || {
        let mut inner = inner;
        inner.check_condition(true).unwrap();
        inner.advance(&ScalarValue::Boolean(false)).unwrap();
        inner.check_condition(false).unwrap();
        inner
    })
    .join()
    .unwrap();
    assert_eq!(inner.completed_iterations(), 1);
    assert_eq!(outer.completed_iterations(), 0);
    outer.advance(&ScalarValue::Integer(1)).unwrap();
    assert_eq!(outer.completed_iterations(), 1);
}

#[test]
fn control_errors_are_fixed_payload_free_observations_not_retry_authority() {
    for (failure, message) in [
        (
            LoopError::InvalidLimit,
            "loop limit exceeds 1024 iterations",
        ),
        (
            LoopError::InvalidPhase,
            "loop operation violates condition/advance order",
        ),
        (
            LoopError::StateTypeMismatch,
            "loop next must preserve the initial state's scalar type",
        ),
        (
            LoopError::UnboundedState,
            "loop next state exceeds its text or list bounds",
        ),
        (LoopError::IterationLimit, "loop iteration limit exhausted"),
    ] {
        assert_eq!(failure.to_string(), message);
        assert!(!format!("{failure:?}").contains("private-token"));
        let error: &dyn std::error::Error = &failure;
        assert!(error.source().is_none());
    }
}
