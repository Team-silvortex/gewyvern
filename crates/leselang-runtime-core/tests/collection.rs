use leselang_runtime_core::{
    MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, ScalarError, StringListBuilder, StringListValue,
};

#[test]
fn empty_default_and_explicit_preallocation_are_bounded_data_not_grants() {
    for builder in [StringListBuilder::new(), StringListBuilder::default()] {
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.remaining_bytes(), 4096);
        let value = builder.finish();
        assert!(value.0.is_empty());
        assert_eq!(value.0.capacity(), 0);
    }
    for capacity in [0, 1, 2, 17, 64] {
        let builder = StringListBuilder::with_capacity(capacity).unwrap();
        assert!(builder.is_empty());
        let value = builder.finish();
        assert!(value.0.capacity() >= capacity);
        assert!(value.is_bounded());
    }
    for capacity in [65, 1024, usize::MAX - 1, usize::MAX] {
        assert_eq!(
            StringListBuilder::with_capacity(capacity).unwrap_err(),
            ScalarError::StringListLimit
        );
    }
}

#[test]
fn owned_pushes_move_original_buffers_and_preserve_order_empty_and_duplicates() {
    let items = ["last", "", "\u{754c}\u{1f600}", "last"].map(str::to_owned);
    let pointers = items.each_ref().map(|item| item.as_ptr());
    let mut builder = StringListBuilder::with_capacity(items.len()).unwrap();
    for item in items {
        builder.try_push(item).unwrap();
    }
    assert_eq!(builder.len(), 4);
    assert_eq!(builder.remaining_bytes(), 4096 - 8 - 7);
    let value = builder.finish();
    assert_eq!(value.0, ["last", "", "\u{754c}\u{1f600}", "last"]);
    for (item, pointer) in value.0.iter().zip(pointers) {
        assert_eq!(item.as_ptr(), pointer);
    }
    assert!(value.is_bounded());
}

#[test]
fn borrowed_pushes_check_utf8_bytes_then_own_an_independent_copy() {
    let mut input = "\u{754c}\u{1f600}".to_owned();
    let mut builder = StringListBuilder::new();
    builder.try_push_text(&input).unwrap();
    assert_eq!(builder.remaining_bytes(), 4096 - 7);
    input.clear();
    builder.try_push_text("").unwrap();
    assert_eq!(builder.finish().0, ["\u{754c}\u{1f600}", ""]);
    let mut builder = StringListBuilder::new();
    assert_eq!(
        builder.try_push_text(&"\u{1f600}".repeat(1025)),
        Err(ScalarError::StringListLimit)
    );
    assert!(builder.is_empty());
    assert_eq!(builder.remaining_bytes(), 4096);
    builder.try_push_text(&"\u{1f600}".repeat(1024)).unwrap();
    assert_eq!(builder.remaining_bytes(), 0);
}

#[test]
fn byte_overflow_rejects_input_without_changing_prefix_or_poisoning_data_builder() {
    let text = "x".repeat(4096);
    let mut builder = StringListBuilder::new();
    builder.try_push_text(&text).unwrap();
    let before = format!("{builder:?}");
    assert_eq!(
        builder.try_push("x".into()),
        Err(ScalarError::StringListLimit)
    );
    assert_eq!(
        builder.try_push_text("x"),
        Err(ScalarError::StringListLimit)
    );
    assert_eq!(format!("{builder:?}"), before);
    assert_eq!(builder.len(), 1);
    assert_eq!(builder.remaining_bytes(), 0);
    for _ in 1..64 {
        builder.try_push(String::new()).unwrap();
    }
    assert_eq!(builder.try_push_text(""), Err(ScalarError::StringListLimit));
    let value = builder.finish();
    assert_eq!(value.0.len(), 64);
    assert_eq!(value.0[0], text);
    assert!(value.0[1..].iter().all(String::is_empty));
    assert!(value.is_bounded());
}

#[test]
fn count_overflow_is_explicit_and_does_not_append_even_an_empty_entry() {
    let mut builder = StringListBuilder::with_capacity(64).unwrap();
    for _ in 0..64 {
        builder.try_push_text("").unwrap();
    }
    let before = format!("{builder:?}");
    for rejected in [String::new(), "private-token".into(), "x".repeat(4097)] {
        assert_eq!(
            builder.try_push(rejected),
            Err(ScalarError::StringListLimit)
        );
        assert_eq!(format!("{builder:?}"), before);
    }
    assert_eq!(builder.remaining_bytes(), 4096);
    assert_eq!(builder.finish().0, vec![String::new(); 64]);
}

#[test]
fn adoption_reuses_vector_capacity_and_all_existing_text_buffers() {
    let mut entries = Vec::with_capacity(256);
    entries.extend(["last", "", "\u{754c}"].map(str::to_owned));
    let vector_pointer = entries.as_ptr();
    let vector_capacity = entries.capacity();
    let text_pointers: Vec<_> = entries.iter().map(|item| item.as_ptr()).collect();
    let mut builder = StringListBuilder::try_from(StringListValue(entries)).unwrap();
    assert_eq!(builder.len(), 3);
    assert_eq!(builder.remaining_bytes(), 4096 - 7);
    builder.try_push("next".into()).unwrap();
    let value = builder.finish();
    assert_eq!(value.0.as_ptr(), vector_pointer);
    assert_eq!(value.0.capacity(), vector_capacity);
    for (item, pointer) in value.0.iter().zip(text_pointers) {
        assert_eq!(item.as_ptr(), pointer);
    }
    assert_eq!(value.0, ["last", "", "\u{754c}", "next"]);
    assert!(value.is_bounded());
}

#[test]
fn adoption_rejects_unchecked_count_or_aggregate_bytes_without_partial_data() {
    for value in [
        StringListValue(vec![String::new(); 65]),
        StringListValue(vec!["x".repeat(4097)]),
        StringListValue(vec!["x".repeat(2049), "x".repeat(2048)]),
    ] {
        assert_eq!(
            StringListBuilder::try_from(value).unwrap_err(),
            ScalarError::UnboundedOperand
        );
    }
    for value in [
        StringListValue(vec![String::new(); 64]),
        StringListValue(vec!["x".repeat(64); 64]),
        StringListValue(vec!["x".repeat(4096), String::new()]),
    ] {
        let expected = value.clone();
        assert_eq!(
            StringListBuilder::try_from(value).unwrap().finish(),
            expected
        );
    }
}

#[test]
fn incremental_boundary_matrix_matches_a_separate_prefix_model() {
    for text in [
        String::new(),
        "x".into(),
        "x".repeat(64),
        "x".repeat(65),
        "\u{754c}".repeat(1365),
        "\u{1f600}".repeat(1024),
        "x".repeat(4097),
    ] {
        for borrowed in [false, true] {
            let mut builder = StringListBuilder::new();
            let mut count = 0;
            let mut bytes = 0;
            for _ in 0..66 {
                let allowed =
                    count < MAX_STRING_LIST_ITEMS && bytes + text.len() <= MAX_SCALAR_STRING_BYTES;
                let result = if borrowed {
                    builder.try_push_text(&text)
                } else {
                    builder.try_push(text.clone())
                };
                assert_eq!(
                    result,
                    if allowed {
                        Ok(())
                    } else {
                        Err(ScalarError::StringListLimit)
                    }
                );
                if allowed {
                    count += 1;
                    bytes += text.len();
                }
                assert_eq!(builder.len(), count);
                assert_eq!(builder.remaining_bytes(), MAX_SCALAR_STRING_BYTES - bytes);
            }
            let value = builder.finish();
            assert_eq!(value.0, vec![text.clone(); count]);
            assert!(value.is_bounded());
        }
    }
}

#[test]
fn builder_debug_is_metadata_only_and_finished_data_is_not_a_validation_certificate() {
    let mut builder = StringListBuilder::new();
    builder.try_push_text("private-token").unwrap();
    let debug = format!("{builder:?}");
    assert!(!debug.contains("private-token"));
    assert!(debug.contains("items: 1"));
    assert!(debug.contains("bytes: 13"));
    let mut value = builder.finish();
    assert!(value.is_bounded());
    value.0.push("x".repeat(4096));
    assert!(!value.is_bounded());
    assert!(StringListBuilder::try_from(value).is_err());
}

#[test]
fn independent_builders_and_worker_handoff_have_no_shared_prefix_or_counter() {
    let mut first = StringListBuilder::new();
    first.try_push_text("first").unwrap();
    let worker = std::thread::spawn(move || {
        first.try_push_text("worker").unwrap();
        first.finish()
    });
    let mut second = StringListBuilder::default();
    second.try_push_text("second").unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(worker.join().unwrap().0, ["first", "worker"]);
    assert_eq!(second.finish().0, ["second"]);
}

#[test]
fn list_decoder_keeps_byte_and_type_checks_before_the_extra_entry_count_error() {
    let prefix = vec!["x".repeat(64); 64];
    for (extra, expected) in [
        (serde_json::json!(""), "string list exceeds 64 entries"),
        (serde_json::json!("x"), "string list exceeds 4096 bytes"),
        (serde_json::json!(null), "invalid type: null"),
    ] {
        let mut payload: Vec<_> = prefix.iter().map(|text| serde_json::json!(text)).collect();
        payload.push(extra);
        let wire = serde_json::to_string(&payload).unwrap();
        for error in [
            serde_json::from_str::<StringListValue>(&wire).unwrap_err(),
            serde_json::from_value::<StringListValue>(serde_json::json!(payload)).unwrap_err(),
        ] {
            assert!(error.to_string().contains(expected), "{error}");
            assert!(!error.to_string().contains(&"x".repeat(64)));
        }
    }
}
