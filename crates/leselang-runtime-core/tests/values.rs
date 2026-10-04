use leselang_runtime_core::{
    MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarType, ScalarValue,
    StringListValue,
};
use serde::Deserialize;
use serde::de::{DeserializeSeed, SeqAccess, value};
use serde_json::json;

#[test]
fn every_legacy_scalar_wire_is_byte_exact_and_round_trips() {
    for (value, wire) in [
        (ScalarValue::Integer(0), r#"{"kind":"integer","value":0}"#),
        (
            ScalarValue::Integer(u64::MAX),
            r#"{"kind":"integer","value":18446744073709551615}"#,
        ),
        (
            ScalarValue::Boolean(true),
            r#"{"kind":"boolean","value":true}"#,
        ),
        (
            ScalarValue::Boolean(false),
            r#"{"kind":"boolean","value":false}"#,
        ),
        (
            ScalarValue::String("".into()),
            r#"{"kind":"string","value":""}"#,
        ),
        (ScalarValue::None, r#"{"kind":"none"}"#),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            r#"{"kind":"optional_string","value":null}"#,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
            r#"{"kind":"optional_string","value":""}"#,
        ),
        (
            ScalarValue::StringList(StringListValue(Vec::new())),
            r#"{"kind":"string_list","value":[]}"#,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["z".into(), "".into(), "z".into()])),
            r#"{"kind":"string_list","value":["z","","z"]}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), wire);
        assert_eq!(serde_json::from_str::<ScalarValue>(wire).unwrap(), value);
        assert_eq!(
            serde_json::from_value::<ScalarValue>(serde_json::from_str(wire).unwrap()).unwrap(),
            value
        );
    }
}

#[test]
fn type_tags_and_nonallocating_queries_have_no_host_semantics() {
    for (value, ty, tag, text) in [
        (
            ScalarValue::Integer(7),
            ScalarType::Integer,
            "integer",
            None,
        ),
        (
            ScalarValue::Boolean(false),
            ScalarType::Boolean,
            "boolean",
            None,
        ),
        (
            ScalarValue::String("ui.focus(node_id: secret)".into()),
            ScalarType::String,
            "string",
            Some("ui.focus(node_id: secret)"),
        ),
        (ScalarValue::None, ScalarType::None, "none", None),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarType::OptionalString,
            "optional_string",
            None,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
            ScalarType::OptionalString,
            "optional_string",
            Some(""),
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["runtime.deploy".into()])),
            ScalarType::StringList,
            "string_list",
            None,
        ),
    ] {
        assert_eq!(value.scalar_type(), ty);
        assert_eq!(value.text(), text);
        assert!(value.is_bounded());
        assert_eq!(serde_json::to_value(ty).unwrap(), json!(tag));
        assert_eq!(
            serde_json::from_value::<ScalarType>(json!(tag)).unwrap(),
            ty
        );
        match &value {
            ScalarValue::String(text) => assert_eq!(value.text().unwrap().as_ptr(), text.as_ptr()),
            ScalarValue::OptionalString(OptionalStringValue(Some(text))) => {
                assert_eq!(value.text().unwrap().as_ptr(), text.as_ptr());
            }
            _ => {}
        }
    }
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ScalarValue>();
    send_sync::<ScalarType>();
}

#[test]
fn absent_empty_and_none_never_coerce_or_synthesize_missing_payloads() {
    let absent = ScalarValue::OptionalString(OptionalStringValue(None));
    let empty = ScalarValue::OptionalString(OptionalStringValue(Some(String::new())));
    assert_ne!(absent, empty);
    assert_ne!(absent, ScalarValue::None);
    for wire in [
        r#"{"kind":"optional_string"}"#,
        r#"{"kind":"string_list"}"#,
        r#"{"kind":"string"}"#,
        r#"{"kind":"optional_string","value":false}"#,
        r#"{"kind":"optional_string","value":0}"#,
        r#"{"kind":"optional_string","value":[]}"#,
    ] {
        assert!(serde_json::from_str::<ScalarValue>(wire).is_err(), "{wire}");
        assert!(
            serde_json::from_value::<ScalarValue>(serde_json::from_str(wire).unwrap()).is_err(),
            "{wire}"
        );
    }
}

#[test]
fn closed_values_reject_unknown_fields_types_and_numeric_coercions() {
    for wire in [
        r#"{"kind":"host_handle","value":7}"#,
        r#"{"kind":"integer","value":-1}"#,
        r#"{"kind":"integer","value":1.0}"#,
        r#"{"kind":"integer","value":"1"}"#,
        r#"{"kind":"integer","value":18446744073709551616}"#,
        r#"{"kind":"boolean","value":1}"#,
        r#"{"kind":"string","value":null}"#,
        r#"{"kind":"string_list","value":["a"],"authority":"admin"}"#,
        r#"{"kind":"string","kind":"integer","value":1}"#,
        r#"{"kind":"integer","value":1,"value":2}"#,
    ] {
        assert!(serde_json::from_str::<ScalarValue>(wire).is_err(), "{wire}");
    }
    assert!(serde_json::from_str::<ScalarType>(r#""host_handle""#).is_err());
}

#[test]
fn plain_strings_preserve_the_legacy_decode_then_validate_boundary() {
    let wire = json!({"kind":"string", "value":"x".repeat(MAX_SCALAR_STRING_BYTES + 1)});
    let value: ScalarValue = serde_json::from_value(wire.clone()).unwrap();
    assert!(!value.is_bounded());
    assert_eq!(serde_json::to_value(&value).unwrap(), wire);
    assert_eq!(value.text().unwrap().len(), MAX_SCALAR_STRING_BYTES + 1);
    // Data may contain controls; host argument validators still own their domains.
    assert!(ScalarValue::String("\0\n".into()).is_bounded());
}

#[test]
fn public_construction_and_mutation_still_require_explicit_boundedness_checks() {
    let mut list = StringListValue(vec![String::new(); MAX_STRING_LIST_ITEMS]);
    assert!(list.is_bounded());
    list.0.push(String::new());
    assert!(!list.is_bounded());
    assert!(!ScalarValue::StringList(list).is_bounded());
    for value in [
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
        ScalarValue::StringList(StringListValue(vec!["x".repeat(2_049); 2])),
    ] {
        assert!(!value.is_bounded());
        assert!(!serde_json::to_string(&value).unwrap().is_empty());
    }
}

#[test]
fn optional_wire_bounds_count_utf8_bytes_and_keep_owned_and_borrowed_parity() {
    let at_limit = "\u{1f40d}".repeat(MAX_SCALAR_STRING_BYTES / 4);
    for (text, accepted) in [(at_limit.clone(), true), (format!("{at_limit}x"), false)] {
        let payload = json!(text);
        let borrowed: Result<OptionalStringValue, _> = serde_json::from_str(&payload.to_string());
        let owned: Result<OptionalStringValue, _> = serde_json::from_value(payload);
        assert_eq!(borrowed.is_ok(), accepted);
        assert_eq!(owned.is_ok(), accepted);
        if accepted {
            assert_eq!(borrowed.unwrap(), owned.unwrap());
        }
    }
}

#[test]
fn list_wire_bounds_share_count_and_utf8_aggregate_budget_without_truncation() {
    for (items, accepted) in [
        (vec![String::new(); MAX_STRING_LIST_ITEMS], true),
        (vec![String::new(); MAX_STRING_LIST_ITEMS + 1], false),
        (vec!["x".repeat(64); MAX_STRING_LIST_ITEMS], true),
        (vec!["\u{1f40d}".repeat(16); MAX_STRING_LIST_ITEMS], true),
        (vec!["x".repeat(MAX_SCALAR_STRING_BYTES)], true),
        (vec!["x".repeat(MAX_SCALAR_STRING_BYTES), "".into()], true),
        (vec!["x".repeat(MAX_SCALAR_STRING_BYTES), "x".into()], false),
        (vec!["x".repeat(2_049); 2], false),
        (vec!["x".repeat(MAX_SCALAR_STRING_BYTES + 1)], false),
    ] {
        let payload = json!(items);
        let borrowed: Result<StringListValue, _> = serde_json::from_str(&payload.to_string());
        let owned: Result<StringListValue, _> = serde_json::from_value(payload);
        assert_eq!(borrowed.is_ok(), accepted);
        assert_eq!(owned.is_ok(), accepted);
        if accepted {
            assert_eq!(borrowed.unwrap().0, items);
            assert_eq!(owned.unwrap().0, items);
        }
    }
}

#[test]
fn malformed_list_elements_never_coerce_into_text() {
    for payload in [
        json!(null),
        json!("a"),
        json!([null]),
        json!([1]),
        json!([true]),
        json!([["a"]]),
        json!([{"kind":"string", "value":"a"}]),
    ] {
        assert!(serde_json::from_str::<StringListValue>(&payload.to_string()).is_err());
        assert!(serde_json::from_value::<StringListValue>(payload).is_err());
    }
}

struct HostSequence<'a> {
    remaining: usize,
    calls: &'a mut usize,
}

impl<'de> SeqAccess<'de> for HostSequence<'_> {
    type Error = value::Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        *self.calls += 1;
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(value::BorrowedStrDeserializer::new(""))
            .map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(usize::MAX)
    }
}

#[test]
fn hostile_sequence_hints_cannot_reserve_memory_or_collect_unbounded_entries() {
    let mut calls = 0;
    let decoded = StringListValue::deserialize(value::SeqAccessDeserializer::new(HostSequence {
        remaining: MAX_STRING_LIST_ITEMS,
        calls: &mut calls,
    }))
    .unwrap();
    assert_eq!(decoded.0.len(), MAX_STRING_LIST_ITEMS);
    assert_eq!(calls, MAX_STRING_LIST_ITEMS + 1);

    calls = 0;
    let error = StringListValue::deserialize(value::SeqAccessDeserializer::new(HostSequence {
        remaining: usize::MAX,
        calls: &mut calls,
    }))
    .unwrap_err();
    assert_eq!(calls, MAX_STRING_LIST_ITEMS + 1);
    assert_eq!(error.to_string(), "string list exceeds 64 entries");
}
