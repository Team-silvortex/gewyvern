use leselang_hir::{computation, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{OptionalStringValue, ScalarValue, StringListValue};
use leselang_syntax::parse;
use leselang_vm::{Step, Value, Vm};

#[test]
fn core_hir_and_vm_share_one_scalar_type_and_legacy_result_wire() {
    for (shared, wire) in [
        (
            ScalarValue::Integer(u64::MAX),
            r#"{"kind":"scalar","value":{"kind":"integer","value":18446744073709551615}}"#,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            r#"{"kind":"scalar","value":{"kind":"optional_string","value":null}}"#,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
            r#"{"kind":"scalar","value":{"kind":"optional_string","value":""}}"#,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["z".into(), "".into(), "z".into()])),
            r#"{"kind":"scalar","value":{"kind":"string_list","value":["z","","z"]}}"#,
        ),
    ] {
        let hir: computation::ScalarValue = shared;
        let old_path: leselang_vm::ScalarValue = hir;
        let value = Value::Scalar { value: old_path };
        assert_eq!(serde_json::to_string(&value).unwrap(), wire);
        assert_eq!(serde_json::from_str::<Value>(wire).unwrap(), value);
    }
}

#[test]
fn reference_evaluation_returns_the_shared_data_without_new_host_requirements() {
    let mut vm = Vm::default();
    for (body, expected) in [
        ("18446744073709551615", ScalarValue::Integer(u64::MAX)),
        ("true", ScalarValue::Boolean(true)),
        (
            r#""literal data""#,
            ScalarValue::String("literal data".into()),
        ),
        ("none", ScalarValue::None),
        (
            "optional_string(value: none)",
            ScalarValue::OptionalString(OptionalStringValue(None)),
        ),
        (
            r#"optional_string(value: "")"#,
            ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
        ),
        (
            r#"strings(z: "last", a: "first")"#,
            ScalarValue::StringList(StringListValue(vec!["last".into(), "first".into()])),
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {body}"))).unwrap();
        assert!(program.function.required_capabilities.is_empty());
        assert_eq!(
            vm.start(
                &program,
                Principal::new("operator").unwrap(),
                CapabilitySet::default(),
                None,
            ),
            Step::Done(Value::Scalar { value: expected })
        );
        assert_eq!(vm.pending_count(), 0);
    }
}
