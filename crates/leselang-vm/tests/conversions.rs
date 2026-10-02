use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, PresentationResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::Connection;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-conversion-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn start(vm: &mut Vm, expression: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {expression}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        None,
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn conversion(name: &str, text: &str) -> String {
    let text = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("{name}(value: \"{text}\")")
}

#[test]
fn scalar_conversions_are_explicit_locale_independent_and_lossless() {
    use ScalarValue::{Boolean, Integer, String};
    for (expression, expected) in [
        ("to_string(value: 0)".into(), String("0".into())),
        (
            "to_string(value: 18446744073709551615)".into(),
            String(u64::MAX.to_string()),
        ),
        ("to_string(value: true)".into(), String("true".into())),
        ("to_string(value: false)".into(), String("false".into())),
        (conversion("to_string", "0007"), String("0007".into())),
        (
            conversion("to_string", "\u{754c}\u{1f642}\n\"\\"),
            String("\u{754c}\u{1f642}\n\"\\".into()),
        ),
        (conversion("parse_integer", "0"), Integer(0)),
        (conversion("parse_integer", "0007"), Integer(7)),
        (
            conversion("parse_integer", "18446744073709551615"),
            Integer(u64::MAX),
        ),
        (conversion("parse_boolean", "true"), Boolean(true)),
        (conversion("parse_boolean", "false"), Boolean(false)),
        (
            "to_string(value: parse_integer(value: \"00042\"))".into(),
            String("42".into()),
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        assert_eq!(
            serde_json::from_slice::<leselang_hir::HirProgram>(
                &serde_json::to_vec(&program).unwrap()
            )
            .unwrap(),
            program
        );
        let mut vm = Vm::default();
        assert_eq!(start(&mut vm, &expression), done(expected), "{expression}");
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
    }
    let legacy = leselang_hir::computation::Computation::Unary {
        operator: leselang_hir::computation::UnaryOperator::Not,
        value: Box::new(leselang_hir::computation::Computation::Literal {
            value: ScalarValue::Boolean(false),
        }),
    };
    assert_eq!(
        serde_json::to_value(&legacy).unwrap(),
        serde_json::json!({
            "kind": "unary", "operator": "not", "value": {"kind":"literal", "value":{"kind":"boolean","value":false}}
        })
    );
}

#[test]
fn parse_errors_reject_coercions_overflow_and_secret_echoes() {
    for (operation, cases) in [
        (
            "parse_integer",
            vec![
                "",
                "+1",
                "-0",
                "-1",
                "1.0",
                "1e2",
                "1_000",
                "0x2A",
                " 1",
                "1 ",
                "1\n",
                "1\0",
                "\u{ff11}",
                "\u{661}",
                "18446744073709551616",
                "secret-value-42",
            ],
        ),
        (
            "parse_boolean",
            vec![
                "",
                "TRUE",
                "False",
                "0",
                "1",
                " true",
                "false ",
                "true\n",
                "true\0",
                "none",
                "secret-value-42",
            ],
        ),
    ] {
        for text in cases {
            let mut vm = Vm::default();
            let step = start(&mut vm, &conversion(operation, text));
            assert!(
                matches!(&step, Step::Fault(f) if f.code == "LSV1408"),
                "{operation} {text:?}: {step:?}"
            );
            assert!(
                !serde_json::to_string(&step)
                    .unwrap()
                    .contains("secret-value-42")
            );
            assert_eq!(
                effect(start(&mut vm, "ui.focus(node_id: \"a\")")).effect_id,
                "effect-1"
            );
        }
    }
}

#[test]
fn text_size_and_shared_fuel_bound_every_conversion() {
    let zeros = "0".repeat(4096);
    assert_eq!(
        start(&mut Vm::new(130), &conversion("parse_integer", &zeros)),
        done(ScalarValue::Integer(0))
    );
    assert!(
        matches!(start(&mut Vm::new(129), &conversion("parse_integer", &zeros)), Step::Fault(f) if f.code == "LSV1001")
    );
    assert_eq!(
        start(&mut Vm::new(3), "to_string(value: 42)"),
        done(ScalarValue::String("42".into()))
    );
    assert!(
        matches!(start(&mut Vm::new(2), "to_string(value: 42)"), Step::Fault(f) if f.code == "LSV1001")
    );
    assert!(
        matches!(start(&mut Vm::default(), &conversion("parse_integer", &"9".repeat(4096))), Step::Fault(f) if f.code == "LSV1408")
    );
    assert_eq!(
        start(
            &mut Vm::default(),
            &conversion("to_string", &"x".repeat(4096))
        ),
        done(ScalarValue::String("x".repeat(4096)))
    );
    assert!(
        lower(&parse(&format!(
            "fn main() = {}",
            conversion("parse_integer", &"0".repeat(4097))
        )))
        .is_err()
    );
}

#[test]
fn parsing_respects_lazy_branches_and_bounded_loop_state() {
    for (source, expected) in [
        (
            r#"choose(when: parse_boolean(value: "true"), then: "ok", otherwise: to_string(value: parse_integer(value: "secret")))"#,
            ScalarValue::String("ok".into()),
        ),
        (
            r#"and(left: false, right: parse_boolean(value: "invalid"))"#,
            ScalarValue::Boolean(false),
        ),
        (
            r#"or(left: true, right: parse_boolean(value: "invalid"))"#,
            ScalarValue::Boolean(true),
        ),
        (
            r#"to_string(value: loop(n: 0, while: lt(left: n, right: parse_integer(value: "3")), next: add(left: n, right: 1), limit: 3))"#,
            ScalarValue::String("3".into()),
        ),
    ] {
        assert_eq!(start(&mut Vm::default(), source), done(expected));
    }
}

#[test]
fn conversions_cannot_skip_host_validation_or_partially_admit_groups() {
    for (source, code) in [
        (
            r#"ui.set_form_value(node_id: "form", field: "replicas", value: to_string(value: parse_integer(value: "invalid")))"#,
            "LSV1408",
        ),
        (
            r#"ui.focus(node_id: to_string(value: "bad node"))"#,
            "LSV1404",
        ),
        (
            r#"ui.set_form_value(node_id: "form", field: "replicas", value: to_string(value: "bad\nvalue"))"#,
            "LSV1404",
        ),
        (
            r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: to_string(value: parse_integer(value: "bad"))))"#,
            "LSV1408",
        ),
        (
            r#"all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: to_string(value: parse_integer(value: "bad"))))"#,
            "LSV1408",
        ),
        (
            r#"repeat(times: 2, body: ui.focus(node_id: to_string(value: parse_boolean(value: "bad"))))"#,
            "LSV1408",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        assert!(
            matches!(start(&mut vm, source), Step::Fault(f) if f.code == code),
            "{source}"
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            Connection::open(&path.0)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            effect(start(&mut vm, "ui.focus(node_id: \"a\")")).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn numeric_result_fields_feed_form_text_through_durable_chains() {
    let path = JournalPath::new();
    let source = r#"bind(counted: ui.assert_child_count(node_id: "rows", count: "7"), body:
        bind(n: add(left: field(value: counted, name: "count"), right: parse_integer(value: "1")), body:
            bind(written: ui.set_form_value(node_id: "form", field: "replicas", value: to_string(value: n)), body:
                parse_boolean(value: to_string(value: eq(left: n, right: 8))))))"#;
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 4);
    assert_eq!(
        decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap(),
        first.continuation
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(
        &first.continuation,
        EffectResult::Presentation(PresentationResult::AssertChildCount {
            node_id: "rows".into(),
            count: 7,
        }),
    ));
    assert!(
        matches!(&second.continuation.pending_effect, Effect::UiSetFormValue { node_id, field, value } if node_id == "form" && field == "replicas" && value == "8")
    );
    assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
    assert_eq!(second.continuation.schema_version, 4);
    assert_eq!(
        decode_continuation(&encode_continuation(&second.continuation).unwrap()).unwrap(),
        second.continuation
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume(&first.continuation, focus("changed"))),
        second
    );
    let result = EffectResult::Presentation(PresentationResult::SetFormValue {
        node_id: "form".into(),
        field: "replicas".into(),
        value: "8".into(),
    });
    assert_eq!(
        vm.resume(&second.continuation, result),
        done(ScalarValue::Boolean(true))
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&first.continuation, focus("changed")),
        done(ScalarValue::Boolean(true))
    );
}

#[test]
fn invalid_result_conversion_is_terminal_redacted_and_replayed() {
    for tail in [
        r#"parse_integer(value: field(value: r, name: "node_id"))"#,
        r#"ui.focus(node_id: to_string(value: parse_integer(value: field(value: r, name: "node_id"))))"#,
    ] {
        let path = JournalPath::new();
        let source = format!(r#"bind(r: ui.focus(node_id: "secret-value-42"), body: {tail})"#);
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, &source));
        let terminal = vm.resume(&first.continuation, focus("secret-value-42"));
        assert!(matches!(&terminal, Step::Fault(f) if f.code == "LSV1408"));
        assert!(
            !serde_json::to_string(&terminal)
                .unwrap()
                .contains("secret-value-42")
        );
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("7")), terminal);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn conditional_conversion_resumes_with_original_authority_and_budget() {
    for (text, early) in [("true", true), ("false", false)] {
        let source = format!(
            r#"bind(r: ui.focus(node_id: "{text}"), body:
            choose(when: parse_boolean(value: field(value: r, name: "node_id")), then: to_string(value: 1),
                otherwise: bind(s: ui.focus(node_id: "2"), body: to_string(value: 2))))"#
        );
        let mut original = Vm::new(500);
        let first = effect(start(&mut original, &source));
        assert_eq!(first.continuation.schema_version, 5);
        assert!(Vm::default().restore(first.continuation.clone()).is_err());
        let mut vm = Vm::new(1);
        vm.restore_request(first.clone()).unwrap();
        let step = vm.resume(&first.continuation, focus(text));
        if early {
            assert_eq!(step, done(ScalarValue::String("1".into())));
        } else {
            let second = effect(step);
            assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
            assert_eq!(
                vm.resume(&second.continuation, focus("2")),
                done(ScalarValue::String("2".into()))
            );
        }
        let mut exhausted = first.clone();
        exhausted.continuation.fuel_remaining = 1;
        exhausted.budget.fuel_remaining = 1;
        let mut vm = Vm::new(1_000_000);
        vm.restore_request(exhausted.clone()).unwrap();
        assert!(
            matches!(vm.resume(&exhausted.continuation, focus(text)), Step::Fault(f) if f.code == "LSV1001")
        );
    }
}
