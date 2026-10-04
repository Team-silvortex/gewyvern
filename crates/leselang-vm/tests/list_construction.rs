use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{ScalarValue, StringListValue};
use leselang_syntax::parse;
use leselang_vm::{Step, Value, Vm};

fn start(body: &str, fuel: u64) -> Step {
    let program = lower(&parse(&format!("fn main() = {body}"))).unwrap();
    assert!(program.function.required_capabilities.is_empty());
    let mut vm = Vm::new(fuel);
    let step = vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::default(),
        None,
    );
    assert_eq!(vm.pending_count(), 0);
    step
}

fn done(items: Vec<String>) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::StringList(StringListValue(items)),
    })
}

fn fault(body: &str, fuel: u64, code: &str, message: &str) {
    let Step::Fault(fault) = start(body, fuel) else {
        panic!("expected fault: {body}")
    };
    assert_eq!(
        (fault.code.as_str(), fault.message.as_str()),
        (code, message)
    );
}

#[test]
fn computed_constructor_exact_fuel_includes_each_entry_and_final_materialization() {
    for count in [0, 1, 2, 17, 64] {
        for text in ["", "x"] {
            let entries = (0..count)
                .map(|index| format!("i{index}: seed"))
                .collect::<Vec<_>>()
                .join(", ");
            let body = format!(
                "bind(seed: {}, body: strings({entries}))",
                serde_json::to_string(text).unwrap()
            );
            let text_cost = u64::from(!text.is_empty());
            let fuel = 3 + text_cost + 2 * count as u64 * (1 + text_cost);
            assert_eq!(start(&body, fuel), done(vec![text.into(); count]));
            fault(&body, fuel - 1, "LSV1001", "computation fuel exhausted");
        }
    }
    for text in ["x".repeat(65), "\u{1f600}".repeat(1024)] {
        let body = format!(
            "bind(seed: {}, body: strings(a: seed))",
            serde_json::to_string(&text).unwrap()
        );
        let fuel = 5 + 3 * text.len().div_ceil(64) as u64;
        assert_eq!(start(&body, fuel), done(vec![text]));
        fault(&body, fuel - 1, "LSV1001", "computation fuel exhausted");
    }
}

#[test]
fn overflowing_prefix_stops_before_later_calculation_and_cannot_be_recovered() {
    let body = format!(
        "bind(seed: {}, body: strings(z: seed, a: \"x\", later: to_string(value: div(left: 1, right: 0))))",
        serde_json::to_string(&"x".repeat(4096)).unwrap()
    );
    fault(&body, 133, "LSV1001", "computation fuel exhausted");
    fault(
        &body,
        134,
        "LSV1403",
        "computed string list exceeds 64 entries or 4096 bytes",
    );
    fault(
        &format!("recover(value: {body}, fallback: strings())"),
        1000,
        "LSV1403",
        "computed string list exceeds 64 entries or 4096 bytes",
    );
    fault(
        r#"strings(z: to_string(value: parse_integer(value: "bad")), a: to_string(value: div(left: 1, right: 0)))"#,
        1000,
        "LSV1408",
        "invalid integer text: expected ASCII decimal within u64",
    );
}

#[test]
fn computed_entries_keep_source_order_empty_duplicates_and_unicode() {
    assert_eq!(
        start(
            &format!(
                r#"bind(seed: {}, body: strings(z: concat(left: seed, right: "z"), a: "", b: seed, c: seed))"#,
                serde_json::to_string("\u{754c}").unwrap()
            ),
            1000
        ),
        done(vec![
            "\u{754c}z".into(),
            "".into(),
            "\u{754c}".into(),
            "\u{754c}".into()
        ])
    );
    let expected = StringListValue(vec![
        "\u{754c}z".into(),
        "".into(),
        "\u{754c}".into(),
        "\u{754c}".into(),
    ]);
    assert_eq!(
        serde_json::to_string(&expected).unwrap(),
        "[\"\u{754c}z\",\"\",\"\u{754c}\",\"\u{754c}\"]"
    );
}

#[test]
fn cold_constructor_entries_still_require_string_type_and_pure_expressions() {
    for body in [
        "strings(a: none)",
        r#"strings(a: bind(r: runtime.list(), body: "x"))"#,
        r#"choose(when: false, then: strings(a: true), otherwise: strings())"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
}
