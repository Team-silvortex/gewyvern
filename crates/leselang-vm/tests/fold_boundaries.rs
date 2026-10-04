use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{
    BinaryOperator, FoldCursor, OptionalStringValue, ScalarType, ScalarValue, StringListValue,
    apply_binary,
};
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

fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

fn assert_fault(body: &str, fuel: u64, code: &str, message: &str) {
    let Step::Fault(fault) = start(body, fuel) else {
        panic!("expected fault: {body}")
    };
    assert_eq!(
        (fault.code.as_str(), fault.message.as_str()),
        (code, message)
    );
}

#[test]
fn literal_collection_folds_preserve_exact_copy_scan_and_iteration_fuel() {
    for count in [0, 1, 2, 17, 64] {
        for text in ["", "x"] {
            let items = (0..count)
                .map(|index| format!("i{index}: {}", serde_json::to_string(text).unwrap()))
                .collect::<Vec<_>>()
                .join(", ");
            let body = format!(
                "fold(total: 0, items: strings({items}), item: \"entry\", next: add(left: total, right: 1), limit: {count})"
            );
            let fuel = 3 + 6 * count + 3 * count * u64::from(!text.is_empty());
            assert_eq!(
                start(&body, fuel),
                done(ScalarValue::Integer(count)),
                "{body}"
            );
            assert_fault(&body, fuel - 1, "LSV1001", "computation fuel exhausted");
        }
    }
    for text in ["x".repeat(65), "\u{1f600}".repeat(1024)] {
        let body = format!(
            "fold(total: 0, items: strings(a: {}), item: \"entry\", next: total, limit: 1)",
            serde_json::to_string(&text).unwrap()
        );
        let fuel = 7 + 3 * text.len().div_ceil(64) as u64;
        assert_eq!(start(&body, fuel), done(ScalarValue::Integer(0)));
        assert_fault(&body, fuel - 1, "LSV1001", "computation fuel exhausted");
    }
}

#[test]
fn collection_then_initial_faults_precede_limit_but_limit_precedes_next() {
    for (body, code, message) in [
        (
            r#"fold(total: parse_integer(value: "bad"), items: strings(a: to_string(value: div(left: 1, right: 0))), item: "entry", next: total, limit: 0)"#,
            "LSV1401",
            "integer overflow, underflow, or division by zero",
        ),
        (
            r#"fold(total: parse_integer(value: "bad"), items: strings(a: "x"), item: "entry", next: total, limit: 0)"#,
            "LSV1408",
            "invalid integer text: expected ASCII decimal within u64",
        ),
        (
            r#"fold(total: 0, items: strings(a: "bad"), item: "entry", next: parse_integer(value: entry), limit: 0)"#,
            "LSV1406",
            "fold iteration limit exhausted",
        ),
        (
            r#"recover(value: fold(total: 0, items: strings(a: "bad"), item: "entry", next: parse_integer(value: entry), limit: 0), fallback: 9)"#,
            "LSV1406",
            "fold iteration limit exhausted",
        ),
    ] {
        assert_fault(body, 10_000, code, message);
    }
    let body = r#"fold(total: 0, items: strings(a: "bad"), item: "entry", next: parse_integer(value: entry), limit: 0)"#;
    assert_fault(body, 4, "LSV1001", "computation fuel exhausted");
    assert_fault(body, 5, "LSV1406", "fold iteration limit exhausted");
    assert_eq!(
        start(
            r#"fold(total: 7, items: strings(), item: "entry", next: div(left: total, right: 0), limit: 0)"#,
            3
        ),
        done(ScalarValue::Integer(7))
    );
}

#[test]
fn fold_error_cleanup_and_nested_folds_keep_local_scopes_isolated() {
    assert_eq!(
        start(
            r#"bind(base: 9, body: recover(value: fold(total: 0, items: strings(a: "bad"), item: "entry", next: parse_integer(value: entry), limit: 1), fallback: bind(total: 2, body: bind(entry: "x", body: add(left: base, right: add(left: total, right: len(value: entry)))))))"#,
            1000
        ),
        done(ScalarValue::Integer(12))
    );
    assert_eq!(
        start(
            r#"fold(total: 0, items: strings(a: "12", b: "345"), item: "row", next: add(left: total, right: fold(count: 0, items: split(left: row, right: ""), item: "entry", next: add(left: count, right: len(value: entry)), limit: 64)), limit: 2)"#,
            1000
        ),
        done(ScalarValue::Integer(5))
    );
}

#[test]
fn folds_preserve_every_scalar_accumulator_and_source_item_order() {
    for (initial, expected) in [
        ("7", ScalarValue::Integer(7)),
        ("true", ScalarValue::Boolean(true)),
        (r#""seed""#, ScalarValue::String("seed".into())),
        ("none", ScalarValue::None),
        (
            "optional_string(value: none)",
            ScalarValue::OptionalString(OptionalStringValue(None)),
        ),
        (
            r#"strings(a: "seed")"#,
            ScalarValue::StringList(StringListValue(vec!["seed".into()])),
        ),
    ] {
        let body = format!(
            "fold(acc: {initial}, items: strings(a: \"\", b: \"x\"), item: \"entry\", next: acc, limit: 2)"
        );
        assert_eq!(start(&body, 1000), done(expected));
    }
    assert_eq!(
        start(
            r#"fold(acc: strings(), items: strings(z: "last", a: "", b: "first", c: "last"), item: "entry", next: append(left: acc, right: entry), limit: 4)"#,
            1000
        ),
        done(ScalarValue::StringList(StringListValue(vec![
            "last".into(),
            "".into(),
            "first".into(),
            "last".into()
        ])))
    );
}

#[test]
fn empty_or_unselected_folds_still_preflight_next_types_and_purity() {
    for body in [
        r#"fold(total: 0, items: strings(), item: "entry", next: false, limit: 0)"#,
        r#"fold(total: 0, items: strings(), item: "entry", next: bind(r: runtime.list(), body: 0), limit: 0)"#,
        r#"choose(when: false, then: fold(total: 0, items: strings(), item: "entry", next: false, limit: 0), otherwise: 7)"#,
        r#"fold(total: 0, items: strings(), item: "entry", next: total, limit: 65)"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
}

#[test]
fn unrelated_headless_traversal_matches_reference_collection_order_and_progress() {
    for items in [
        vec![],
        vec!["last".into(), "".into(), "\u{754c}".into(), "last".into()],
        vec![String::new(); 64],
        vec!["\u{1f600}".repeat(1024)],
    ] {
        let source = items
            .iter()
            .enumerate()
            .map(|(index, text)| format!("i{index}: {}", serde_json::to_string(text).unwrap()))
            .collect::<Vec<_>>()
            .join(", ");
        let count = items.len() as u64;
        let mut cursor =
            FoldCursor::new(StringListValue(items), ScalarType::StringList, count).unwrap();
        let mut state = ScalarValue::StringList(StringListValue(vec![]));
        while let Some(item) = cursor.next_item().unwrap() {
            state = apply_binary(BinaryOperator::Append, state, ScalarValue::String(item)).unwrap();
            cursor.advance(&state).unwrap();
        }
        assert_eq!(cursor.completed_iterations(), count);
        assert_eq!(
            start(
                &format!(
                    "fold(acc: strings(), items: strings({source}), item: \"entry\", next: append(left: acc, right: entry), limit: {count})"
                ),
                10_000
            ),
            done(state)
        );
    }
}
