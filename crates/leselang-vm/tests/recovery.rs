use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectRequest, EffectResult, PresentationResult,
    RetryDisposition, RetryPolicy, ScalarValue, Step, Value, Vm, decode_continuation,
    encode_continuation,
};
use rusqlite::Connection;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-recovery-{}-{}",
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
const CAPTURE: &str = r#"bind(r: ui.focus(node_id: "bad"), body:
    recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: 7))"#;

#[test]
fn recovery_returns_success_unchanged_and_lazily_recovers_only_data_errors() {
    use ScalarValue::{Boolean, Integer, None, String};
    for (source, expected) in [
        (
            "recover(value: 0, fallback: div(left: 1, right: 0))",
            Integer(0),
        ),
        ("recover(value: false, fallback: true)", Boolean(false)),
        ("recover(value: none, fallback: none)", None),
        (
            r#"recover(value: "", fallback: "default")"#,
            String("".into()),
        ),
        (
            "recover(fallback: div(left: 1, right: 0), value: 2)",
            Integer(2),
        ),
        (
            "recover(value: div(left: 1, right: 0), fallback: 7)",
            Integer(7),
        ),
        (
            "recover(value: rem(left: 1, right: 0), fallback: 8)",
            Integer(8),
        ),
        (
            "recover(value: sub(left: 0, right: 1), fallback: 9)",
            Integer(9),
        ),
        (
            "recover(value: add(left: 18446744073709551615, right: 1), fallback: 10)",
            Integer(10),
        ),
        (
            "recover(value: mul(left: 18446744073709551615, right: 2), fallback: 11)",
            Integer(11),
        ),
        (
            r#"recover(value: parse_integer(value: "secret-value"), fallback: 12)"#,
            Integer(12),
        ),
        (
            r#"recover(value: parse_boolean(value: "False"), fallback: true)"#,
            Boolean(true),
        ),
        (
            r#"recover(value: to_string(value: div(left: 1, right: 0)), fallback: "unavailable")"#,
            String("unavailable".into()),
        ),
        (
            "recover(value: recover(value: div(left: 1, right: 0), fallback: div(left: 2, right: 0)), fallback: 13)",
            Integer(13),
        ),
    ] {
        let mut vm = Vm::default();
        assert_eq!(start(&mut vm, source), done(expected), "{source}");
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        assert_eq!(
            serde_json::from_slice::<leselang_hir::HirProgram>(
                &serde_json::to_vec(&program).unwrap()
            )
            .unwrap(),
            program
        );
    }
    let mut vm = Vm::default();
    let failed = start(
        &mut vm,
        r#"recover(value: div(left: 1, right: 0), fallback: parse_integer(value: "secret-value"))"#,
    );
    assert!(matches!(&failed, Step::Fault(fault) if fault.code == "LSV1408"));
    assert!(
        !serde_json::to_string(&failed)
            .unwrap()
            .contains("secret-value")
    );
    assert_eq!(
        effect(start(&mut vm, "ui.focus(node_id: \"a\")")).effect_id,
        "effect-1"
    );
}

#[test]
fn failed_computation_unwinds_locals_and_loop_state_before_fallback() {
    for (source, expected) in [
        (
            "bind(base: 9, body: recover(value: bind(tmp: 4, body: div(left: tmp, right: 0)), fallback: bind(tmp: 3, body: add(left: base, right: tmp))))",
            12,
        ),
        (
            "bind(base: 9, body: recover(value: loop(n: 0, while: true, next: div(left: 1, right: n), limit: 2), fallback: bind(n: 3, body: add(left: base, right: n))))",
            12,
        ),
        (
            "loop(n: 3, while: gt(left: n, right: 0), next: recover(value: sub(left: n, right: 2), fallback: 0), limit: 3)",
            0,
        ),
        (
            "recover(value: add(left: 1, right: div(left: 1, right: 0)), fallback: 7)",
            7,
        ),
    ] {
        assert_eq!(
            start(&mut Vm::default(), source),
            done(ScalarValue::Integer(expected)),
            "{source}"
        );
    }
}

#[test]
fn fallback_uses_remaining_fuel_and_cannot_catch_resource_limits() {
    let source = "recover(value: div(left: 1, right: 0), fallback: 7)";
    assert_eq!(
        start(&mut Vm::new(5), source),
        done(ScalarValue::Integer(7))
    );
    for fuel in 0..5 {
        assert!(
            matches!(start(&mut Vm::new(fuel), source), Step::Fault(fault) if fault.code == "LSV1001")
        );
    }
    assert_eq!(
        start(
            &mut Vm::new(2),
            "recover(value: 7, fallback: div(left: 1, right: 0))"
        ),
        done(ScalarValue::Integer(7))
    );
    let source = r#"recover(value: parse_integer(value: "bad"), fallback: 7)"#;
    assert_eq!(
        start(&mut Vm::new(6), source),
        done(ScalarValue::Integer(7))
    );
    assert!(
        matches!(start(&mut Vm::new(5), source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let source =
        r#"recover(value: recover(value: parse_integer(value: "bad"), fallback: 7), fallback: 9)"#;
    assert!(
        matches!(start(&mut Vm::new(6), source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    for source in [
        "recover(value: loop(n: 0, while: true, next: n, limit: 0), fallback: 7)",
        "recover(value: div(left: 1, right: 0), fallback: loop(n: 0, while: true, next: n, limit: 0))",
    ] {
        assert!(
            matches!(start(&mut Vm::default(), source), Step::Fault(fault) if fault.code == "LSV1406")
        );
    }
    let source = format!(
        r#"recover(value: concat(left: "{}", right: "x"), fallback: "small")"#,
        "x".repeat(4096)
    );
    assert!(
        matches!(start(&mut Vm::new(10_000), &source), Step::Fault(fault) if fault.code == "LSV1403")
    );
}

#[test]
fn recovered_host_arguments_still_validate_and_group_failures_admit_no_work() {
    for (source, code) in [
        (
            r#"ui.focus(node_id: recover(value: "bad node", fallback: "a"))"#,
            "LSV1404",
        ),
        (
            r#"ui.focus(node_id: recover(value: to_string(value: div(left: 1, right: 0)), fallback: "bad node"))"#,
            "LSV1404",
        ),
        (
            r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: to_string(value: recover(value: div(left: 1, right: 0), fallback: parse_integer(value: "bad")))))"#,
            "LSV1408",
        ),
        (
            r#"all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: to_string(value: recover(value: div(left: 1, right: 0), fallback: parse_integer(value: "bad")))))"#,
            "LSV1408",
        ),
        (
            r#"repeat(times: 2, body: ui.focus(node_id: to_string(value: recover(value: div(left: 1, right: 0), fallback: parse_integer(value: "bad")))))"#,
            "LSV1408",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        assert!(
            matches!(start(&mut vm, source), Step::Fault(fault) if fault.code == code),
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
    let request = effect(start(
        &mut Vm::new(100),
        r#"ui.focus(node_id: to_string(value: recover(value: parse_integer(value: "bad"), fallback: 7)))"#,
    ));
    assert!(
        matches!(request.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "7")
    );
    assert!(request.continuation.result_binding.is_none());
    assert_eq!(request.continuation.schema_version, 1);
}

#[test]
fn pure_recovery_is_durable_and_replays_the_first_committed_fallback_or_fault() {
    for (source, expected) in [
        (CAPTURE, done(ScalarValue::Integer(7))),
        (
            r#"bind(r: ui.focus(node_id: "bad"), body: recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: div(left: 1, right: 0)))"#,
            Step::Fault(leselang_vm::Fault {
                code: "LSV1401".into(),
                message: "integer overflow, underflow, or division by zero".into(),
            }),
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, source));
        assert_eq!(first.continuation.schema_version, 2);
        assert_eq!(
            decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap(),
            first.continuation
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("bad")), expected);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("42")), expected);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn recovery_prepares_a_durable_successor_without_replacing_authority_or_replaying_work() {
    let path = JournalPath::new();
    let source = r#"bind(r: ui.focus(node_id: "bad"), body:
        ui.set_form_value(node_id: "form", field: "replicas", value: to_string(value:
            recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: 7))))"#;
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 3);
    assert!(Vm::default().restore(first.continuation.clone()).is_err());
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, focus("bad")));
    assert!(
        matches!(&second.continuation.pending_effect, Effect::UiSetFormValue { value, .. } if value == "7")
    );
    let (
        leselang_vm::EffectOperation::Presentation(first_authority),
        leselang_vm::EffectOperation::Presentation(next_authority),
    ) = (&first.operation, &second.operation)
    else {
        panic!("expected presentation authority")
    };
    assert_eq!(first_authority.principal, next_authority.principal);
    assert_eq!(first_authority.capabilities, next_authority.capabilities);
    assert_eq!(
        first.continuation.deadline_at_ms,
        second.continuation.deadline_at_ms
    );
    assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(effect(vm.resume(&first.continuation, focus("42"))), second);
    let terminal = vm.resume(
        &second.continuation,
        EffectResult::Presentation(PresentationResult::SetFormValue {
            node_id: "form".into(),
            field: "replicas".into(),
            value: "7".into(),
        }),
    );
    assert!(matches!(terminal, Step::Done(Value::UiSetFormValue { .. })));
    assert_eq!(vm.resume(&first.continuation, focus("different")), terminal);
}

#[test]
fn recovered_locals_and_prior_projections_survive_multiple_suspensions() {
    let path = JournalPath::new();
    let source = r#"bind(r: ui.focus(node_id: "bad"), body:
        bind(n: recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: 7), body:
            bind(s: ui.focus(node_id: to_string(value: n)), body:
                add(left: n, right: recover(value: parse_integer(value: field(value: r, name: "node_id")),
                    fallback: parse_integer(value: field(value: s, name: "node_id")))))))"#;
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 4);
    let second = effect(vm.resume(&first.continuation, focus("bad")));
    assert_eq!(second.continuation.schema_version, 4);
    assert_eq!(
        decode_continuation(&encode_continuation(&second.continuation).unwrap()).unwrap(),
        second.continuation
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&second.continuation, focus("7")),
        done(ScalarValue::Integer(14))
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&first.continuation, focus("changed")),
        done(ScalarValue::Integer(14))
    );
}

#[test]
fn conditional_recovery_restarts_with_saved_fuel_and_the_original_exit_decision() {
    for (node, early) in [("true", true), ("bad", false)] {
        let path = JournalPath::new();
        let source = format!(
            r#"bind(r: ui.focus(node_id: "{node}"), body:
            choose(when: recover(value: parse_boolean(value: field(value: r, name: "node_id")), fallback: false),
                then: 1, otherwise: bind(s: ui.focus(node_id: "2"), body:
                    recover(value: parse_integer(value: field(value: s, name: "node_id")), fallback: 0))))"#
        );
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, &source));
        assert_eq!(first.continuation.schema_version, 5);
        let mut exhausted = first.clone();
        exhausted.continuation.fuel_remaining = 1;
        exhausted.budget.fuel_remaining = 1;
        let mut other = Vm::new(1_000_000);
        other.restore_request(exhausted.clone()).unwrap();
        assert!(
            matches!(other.resume(&exhausted.continuation, focus(node)), Step::Fault(fault) if fault.code == "LSV1001")
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let mut terminal = vm.resume(&first.continuation, focus(node));
        if !early {
            let next = effect(terminal);
            drop(vm);
            vm = Vm::open_journal(&path.0, 1).unwrap();
            terminal = vm.resume(&next.continuation, focus("2"));
        }
        assert_eq!(
            terminal,
            done(ScalarValue::Integer(if early { 1 } else { 2 }))
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume(&first.continuation, focus("changed")), terminal);
    }
}

#[test]
fn recovery_does_not_intercept_authority_cancellation_deadlines_or_host_failures() {
    let program = lower(&parse(&format!("fn main() = {CAPTURE}"))).unwrap();
    assert!(
        matches!(Vm::default().start(&program, Principal::new("operator").unwrap(), CapabilitySet::default(), None), Step::Fault(fault) if fault.code == "LSH2001")
    );
    for cancel in [true, false] {
        let mut vm = Vm::new(500);
        let first = effect(vm.start_timed(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            10,
        ));
        let terminal = if cancel {
            vm.cancel_effect(&first.continuation, 105)
        } else {
            vm.resume_at(&first.continuation, 111, focus("bad"))
        };
        assert!(matches!(terminal, Step::Cancelled(_)));
        assert_eq!(
            vm.resume_at(&first.continuation, 112, focus("bad")),
            terminal
        );
        assert_eq!(vm.pending_count(), 0);
    }
    let mut vm = Vm::default();
    let first = effect(start(&mut vm, CAPTURE));
    assert!(
        matches!(vm.resume(&first.continuation, focus("wrong")), Step::Fault(fault) if fault.code == "LSV2103")
    );
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(vm.start_timed(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        100,
        1000,
    ));
    let lease = vm.claim_effect(101, 50).unwrap().unwrap();
    let disposition = vm
        .report_effect_error(
            &lease,
            102,
            EffectError {
                class: EffectErrorClass::Permanent,
                code: "LSV1408".into(),
                message: "host failure with a language-like code".into(),
            },
            &RetryPolicy::default(),
        )
        .unwrap();
    let RetryDisposition::Terminal(terminal) = disposition else {
        panic!("host error must remain terminal")
    };
    assert!(matches!(&terminal, Step::Failed(failure) if failure.error.code == "LSV1408"));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 103, focus("bad")),
        terminal
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn recovery_continuations_reject_forged_fallbacks_and_unknown_shapes() {
    let mut vm = Vm::default();
    let first = effect(start(&mut vm, CAPTURE));
    let encoded = serde_json::to_value(&first.continuation).unwrap();
    for mutate in [
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["extra"] = true.into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["kind"] = "catch_all".into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["fallback"] =
                serde_json::json!({"kind":"literal", "value":{"kind":"boolean","value":false}});
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["fallback"] =
                serde_json::json!({"kind":"local", "name":"missing"});
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["fallback"] =
                serde_json::json!({"kind":"host", "effect":{"kind":"runtime_list", "filter":{}}});
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["fallback"] = serde_json::json!({"kind":"literal", "value":{"kind":"string","value":"x".repeat(4097)}});
        },
    ] {
        let mut invalid = encoded.clone();
        mutate(&mut invalid);
        assert!(decode_continuation(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let restored = decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap();
    let mut vm = Vm::new(1);
    vm.restore(restored.clone()).unwrap();
    assert_eq!(
        vm.resume(&restored, focus("bad")),
        done(ScalarValue::Integer(7))
    );
}
