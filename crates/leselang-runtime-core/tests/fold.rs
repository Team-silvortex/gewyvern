use leselang_runtime_core::{
    FoldCursor, FoldError, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarType, ScalarValue,
    StringListValue,
};

fn list(items: &[&str]) -> StringListValue {
    StringListValue(items.iter().map(|item| (*item).to_owned()).collect())
}

fn constructor_error(items: StringListValue, limit: u64) -> FoldError {
    FoldCursor::new(items, ScalarType::Integer, limit).unwrap_err()
}

fn scalar_values() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(7),
        ScalarValue::Boolean(true),
        ScalarValue::String("private-state".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("private-state".into()))),
        ScalarValue::StringList(list(&["private-state"])),
    ]
}

#[test]
fn constructor_limits_are_closed_full_width_and_never_truncate() {
    assert_eq!(MAX_STRING_LIST_ITEMS, 64);
    for limit in [0, 1, 2, 17, 64] {
        let cursor = FoldCursor::new(list(&[]), ScalarType::Integer, limit).unwrap();
        assert_eq!(cursor.completed_iterations(), 0);
    }
    for limit in [65, 1024, u64::MAX - 1, u64::MAX] {
        assert_eq!(constructor_error(list(&[]), limit), FoldError::InvalidLimit);
    }
    for count in [1, 2, 17, 64] {
        let items = StringListValue(vec![String::new(); count]);
        assert_eq!(
            constructor_error(items.clone(), count as u64 - 1),
            FoldError::IterationLimit
        );
        assert!(FoldCursor::new(items, ScalarType::Integer, count as u64).is_ok());
    }
    assert_eq!(
        constructor_error(StringListValue(vec![String::new(); 65]), u64::MAX),
        FoldError::InvalidLimit
    );
    assert_eq!(
        constructor_error(StringListValue(vec![String::new(); 65]), 0),
        FoldError::UnboundedItems
    );
}

#[test]
fn direct_unbounded_collections_cannot_enter_traversal() {
    for items in [
        StringListValue(vec![String::new(); 65]),
        StringListValue(vec!["x".repeat(4097)]),
        StringListValue(vec!["x".repeat(2049), "x".repeat(2048)]),
        StringListValue(vec!["\u{1f600}".repeat(1025)]),
    ] {
        assert_eq!(constructor_error(items, 64), FoldError::UnboundedItems);
    }
    for items in [
        StringListValue(vec!["x".repeat(4096)]),
        StringListValue(vec!["x".repeat(64); 64]),
        StringListValue(vec!["\u{1f600}".repeat(1024)]),
    ] {
        assert!(FoldCursor::new(items, ScalarType::Integer, 64).is_ok());
    }
}

#[test]
fn source_order_duplicates_empty_and_unicode_move_the_original_text_buffers() {
    let items = list(&["last", "", "\u{754c}\u{1f600}", "last"]);
    let pointers: Vec<_> = items.0.iter().map(|item| item.as_ptr()).collect();
    let mut cursor = FoldCursor::new(items, ScalarType::Integer, 4).unwrap();
    for (index, expected) in ["last", "", "\u{754c}\u{1f600}", "last"]
        .into_iter()
        .enumerate()
    {
        let item = cursor.next_item().unwrap().unwrap();
        assert_eq!(item, expected);
        assert_eq!(item.as_ptr(), pointers[index]);
        assert_eq!(cursor.completed_iterations(), index as u64);
        cursor
            .advance(&ScalarValue::Integer(index as u64 + 1))
            .unwrap();
    }
    assert_eq!(cursor.next_item(), Ok(None));
    assert_eq!(cursor.completed_iterations(), 4);
}

#[test]
fn phase_errors_do_not_consume_items_or_advance_counts() {
    let mut cursor = FoldCursor::new(list(&["a", "b"]), ScalarType::Integer, 2).unwrap();
    assert_eq!(
        cursor.advance(&ScalarValue::Integer(0)),
        Err(FoldError::InvalidPhase)
    );
    assert_eq!(cursor.next_item(), Ok(Some("a".into())));
    let before = format!("{cursor:?}");
    assert_eq!(cursor.next_item(), Err(FoldError::InvalidPhase));
    assert_eq!(format!("{cursor:?}"), before);
    assert_eq!(cursor.completed_iterations(), 0);
    cursor.advance(&ScalarValue::Integer(1)).unwrap();
    assert_eq!(
        cursor.advance(&ScalarValue::Integer(2)),
        Err(FoldError::InvalidPhase)
    );
    assert_eq!(cursor.completed_iterations(), 1);
    assert_eq!(cursor.next_item(), Ok(Some("b".into())));
    cursor.advance(&ScalarValue::Integer(2)).unwrap();
    assert_eq!(cursor.next_item(), Ok(None));
    assert_eq!(cursor.next_item(), Err(FoldError::InvalidPhase));
    assert_eq!(
        cursor.advance(&ScalarValue::Integer(3)),
        Err(FoldError::InvalidPhase)
    );
    assert_eq!(cursor.completed_iterations(), 2);
}

#[test]
fn all_scalar_accumulators_require_same_type_without_retaining_their_values() {
    for expected in scalar_values() {
        let mut cursor = FoldCursor::new(list(&["entry"]), expected.scalar_type(), 1).unwrap();
        cursor.next_item().unwrap().unwrap();
        for wrong in scalar_values() {
            if wrong.scalar_type() != expected.scalar_type() {
                let before = format!("{cursor:?}");
                assert_eq!(cursor.advance(&wrong), Err(FoldError::StateTypeMismatch));
                assert_eq!(format!("{cursor:?}"), before);
            }
        }
        cursor.advance(&expected).unwrap();
        assert_eq!(cursor.completed_iterations(), 1);
        assert!(!format!("{cursor:?}").contains("private-state"));
        assert_eq!(cursor.next_item(), Ok(None));
    }
}

#[test]
fn oversized_next_states_do_not_make_progress_or_unlock_the_next_item() {
    for next in [
        ScalarValue::String("x".repeat(4097)),
        ScalarValue::OptionalString(OptionalStringValue(Some("x".repeat(4097)))),
        ScalarValue::StringList(StringListValue(vec![String::new(); 65])),
        ScalarValue::StringList(StringListValue(vec!["x".repeat(2049), "x".repeat(2048)])),
    ] {
        let mut cursor =
            FoldCursor::new(list(&["first", "second"]), next.scalar_type(), 2).unwrap();
        assert_eq!(cursor.next_item(), Ok(Some("first".into())));
        let before = format!("{cursor:?}");
        assert_eq!(cursor.advance(&next), Err(FoldError::UnboundedState));
        assert_eq!(format!("{cursor:?}"), before);
        assert_eq!(cursor.next_item(), Err(FoldError::InvalidPhase));
        let valid = scalar_values()
            .into_iter()
            .find(|value| value.scalar_type() == next.scalar_type())
            .unwrap();
        cursor.advance(&valid).unwrap();
        assert_eq!(cursor.next_item(), Ok(Some("second".into())));
        assert_eq!(cursor.completed_iterations(), 1);
    }
}

#[test]
fn empty_and_maximum_collections_have_exact_non_rearmable_completion() {
    for count in [0, 1, 2, 17, 64] {
        let mut cursor = FoldCursor::new(
            StringListValue(vec![String::new(); count]),
            ScalarType::None,
            count as u64,
        )
        .unwrap();
        for index in 0..count {
            assert_eq!(cursor.next_item(), Ok(Some(String::new())));
            cursor.advance(&ScalarValue::None).unwrap();
            assert_eq!(cursor.completed_iterations(), index as u64 + 1);
        }
        assert_eq!(cursor.next_item(), Ok(None));
        assert_eq!(cursor.next_item(), Err(FoldError::InvalidPhase));
        assert_eq!(
            cursor.advance(&ScalarValue::None),
            Err(FoldError::InvalidPhase)
        );
        assert_eq!(cursor.completed_iterations(), count as u64);
    }
}

#[test]
fn nested_cursors_and_send_handoff_do_not_share_progress() {
    let mut outer = FoldCursor::new(list(&["a", "b"]), ScalarType::Integer, 2).unwrap();
    while let Some(item) = outer.next_item().unwrap() {
        let mut inner = FoldCursor::new(list(&[&item]), ScalarType::Integer, 1).unwrap();
        let worker = std::thread::spawn(move || {
            assert!(inner.next_item().unwrap().is_some());
            inner.advance(&ScalarValue::Integer(1)).unwrap();
            assert_eq!(inner.next_item(), Ok(None));
            inner.completed_iterations()
        });
        assert_eq!(worker.join().unwrap(), 1);
        outer
            .advance(&ScalarValue::Integer(outer.completed_iterations() + 1))
            .unwrap();
    }
    assert_eq!(outer.completed_iterations(), 2);
}

#[test]
fn debug_and_fixed_errors_never_echo_items_or_accumulator_payloads() {
    let mut cursor = FoldCursor::new(
        list(&["private-token", "private-token"]),
        ScalarType::String,
        2,
    )
    .unwrap();
    assert!(!format!("{cursor:?}").contains("private-token"));
    cursor.next_item().unwrap().unwrap();
    assert!(!format!("{cursor:?}").contains("private-token"));
    cursor
        .advance(&ScalarValue::String("private-state".into()))
        .unwrap();
    let debug = format!("{cursor:?}");
    assert!(!debug.contains("private-token"));
    assert!(!debug.contains("private-state"));
    for (error, message) in [
        (FoldError::InvalidLimit, "fold limit exceeds 64 entries"),
        (
            FoldError::UnboundedItems,
            "fold items exceed 64 entries or 4096 bytes",
        ),
        (FoldError::IterationLimit, "fold iteration limit exhausted"),
        (
            FoldError::InvalidPhase,
            "fold operation violates item/advance order",
        ),
        (
            FoldError::StateTypeMismatch,
            "fold next must preserve the initial state's scalar type",
        ),
        (
            FoldError::UnboundedState,
            "fold next state exceeds its text or list bounds",
        ),
    ] {
        assert_eq!(error.to_string(), message);
        assert!(std::error::Error::source(&error).is_none());
    }
}
