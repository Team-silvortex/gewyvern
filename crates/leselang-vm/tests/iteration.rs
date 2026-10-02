use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, ScalarValue, Step, Value, Vm, decode_continuation,
    encode_continuation,
};
use leserpent_domain::QueryResult;

fn start(vm: &mut Vm, expression: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {expression}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["runtime.read", "ui.presentation"]),
        None,
    )
}

fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}

fn list(revision: u64) -> EffectResult {
    EffectResult::Query(QueryResult::RuntimeList {
        revision: Revision(revision),
        runtimes: vec![],
    })
}

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-iteration-{}-{}",
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

const THREE: &str =
    "loop(n: 0, while: lt(left: n, right: 3), next: add(left: n, right: 1), limit: 3)";
const RESULT_LOOP: &str = r#"bind(r: runtime.list(), body:
    loop(n: 1, while: lt(left: n, right: field(value: r, name: "revision")),
        next: mul(left: n, right: 2), limit: 6))"#;

#[test]
fn scalar_loops_return_the_final_state_without_allocating_host_work() {
    for (source, expected) in [
        (THREE, ScalarValue::Integer(3)),
        (
            "loop(ready: true, while: ready, next: false, limit: 1)",
            ScalarValue::Boolean(false),
        ),
        (
            "loop(unit: none, while: false, next: unit, limit: 0)",
            ScalarValue::None,
        ),
        (
            "loop(s: \"\", while: lt(left: len(value: s), right: 3), next: concat(left: s, right: \"x\"), limit: 3)",
            ScalarValue::String("xxx".into()),
        ),
        (
            "bind(goal: 5, body: loop(n: 0, while: lt(left: n, right: goal), next: add(left: n, right: 1), limit: 8))",
            ScalarValue::Integer(5),
        ),
        (
            "loop(n: 0, while: lt(left: n, right: 1024), next: add(left: n, right: 1), limit: 1024)",
            ScalarValue::Integer(1024),
        ),
    ] {
        let mut vm = Vm::new(10_000);
        assert_eq!(start(&mut vm, source), done(expected), "{source}");
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        assert!(vm.claim_effect(1, 10).unwrap().is_none());
    }
}

#[test]
fn condition_precedes_next_and_limit_is_not_silent_truncation() {
    for source in [
        "loop(n: 7, while: false, next: div(left: 1, right: 0), limit: 0)",
        "loop(n: 7, while: false, next: div(left: 1, right: 0), limit: 1024)",
        "choose(when: true, then: 7, otherwise: loop(n: 0, while: true, next: n, limit: 0))",
    ] {
        assert_eq!(
            start(&mut Vm::default(), source),
            done(ScalarValue::Integer(7))
        );
    }
    for (source, code) in [
        (
            "loop(n: 0, while: true, next: div(left: 1, right: 0), limit: 0)",
            "LSV1406",
        ),
        ("loop(n: 0, while: true, next: n, limit: 2)", "LSV1406"),
        (
            "loop(n: div(left: 1, right: 0), while: false, next: n, limit: 0)",
            "LSV1401",
        ),
        (
            "loop(n: 0, while: eq(left: div(left: 1, right: 0), right: 0), next: n, limit: 0)",
            "LSV1401",
        ),
        (
            "loop(n: 0, while: true, next: div(left: 1, right: 0), limit: 1)",
            "LSV1401",
        ),
    ] {
        let mut vm = Vm::default();
        assert!(
            matches!(start(&mut vm, source), Step::Fault(fault) if fault.code == code),
            "{source}"
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        assert_eq!(
            effect(start(&mut vm, "runtime.list()")).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn loop_fuel_is_shared_across_conditions_steps_nested_loops_and_host_preparation() {
    assert_eq!(
        start(&mut Vm::new(23), THREE),
        done(ScalarValue::Integer(3))
    );
    assert!(
        matches!(start(&mut Vm::new(22), THREE), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let nested = "loop(n: 0, while: lt(left: n, right: 3), next: add(left: n, right: loop(m: 0, while: lt(left: m, right: 1), next: add(left: m, right: 1), limit: 1)), limit: 3)";
    assert_eq!(
        start(&mut Vm::new(53), nested),
        done(ScalarValue::Integer(3))
    );
    assert!(
        matches!(start(&mut Vm::new(52), nested), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let source = format!("bind(count: {THREE}, body: runtime.list())");
    let request = effect(start(&mut Vm::new(30), &source));
    assert_eq!(request.budget.fuel_remaining, 4);
    assert!(matches!(
        request.continuation.pending_effect,
        Effect::RuntimeList { .. }
    ));
    assert!(request.continuation.result_binding.is_none());
}

#[test]
fn loop_state_does_not_leak_or_mutate_enclosing_bindings() {
    for (source, expected) in [
        (format!("add(left: {THREE}, right: {THREE})"), 6),
        (
            "bind(seed: 9, body: add(left: loop(n: seed, while: gt(left: n, right: 0), next: sub(left: n, right: 1), limit: 9), right: seed))".into(),
            9,
        ),
        (format!("bind(n: {THREE}, body: add(left: n, right: 1))"), 4),
    ] {
        assert_eq!(
            start(&mut Vm::default(), &source),
            done(ScalarValue::Integer(expected))
        );
    }
}

#[test]
fn loop_string_copies_and_outputs_remain_bounded() {
    let source = format!(
        "loop(s: \"{}\", while: true, next: concat(left: s, right: \"x\"), limit: 2)",
        "x".repeat(4096)
    );
    assert!(
        matches!(start(&mut Vm::new(10_000), &source), Step::Fault(fault) if fault.code == "LSV1403")
    );
    let source = format!(
        "loop(s: \"{}\", while: false, next: s, limit: 0)",
        "x".repeat(64)
    );
    assert!(
        matches!(start(&mut Vm::new(3), &source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    assert_eq!(
        start(&mut Vm::new(4), &source),
        done(ScalarValue::String("x".repeat(64)))
    );
}

#[test]
fn loops_can_prepare_atomic_arguments_but_group_preparation_stays_all_or_nothing() {
    let source = r#"ui.focus(node_id: loop(node: "a", while: lt(left: len(value: node), right: 3), next: concat(left: node, right: "x"), limit: 2))"#;
    let request = effect(start(&mut Vm::default(), source));
    assert!(
        matches!(request.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "axx")
    );
    let source = r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: loop(node: "b", while: true, next: node, limit: 0)))"#;
    let mut vm = Vm::default();
    assert!(matches!(start(&mut vm, source), Step::Fault(fault) if fault.code == "LSV1406"));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 0);
    assert_eq!(
        effect(start(&mut vm, "runtime.list()")).effect_id,
        "effect-1"
    );
}

#[test]
fn result_driven_pure_loops_resume_after_restart_and_replay_the_first_committed_outcome() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let request = effect(start(&mut vm, RESULT_LOOP));
    let encoded = encode_continuation(&request.continuation).unwrap();
    assert_eq!(decode_continuation(&encoded).unwrap(), request.continuation);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let lease = vm.claim_effect(1, 10).unwrap().unwrap();
    let terminal = vm.acknowledge_effect(&lease, 2, list(17));
    assert_eq!(terminal, done(ScalarValue::Integer(32)));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.acknowledge_effect(&lease, 3, list(63)), terminal);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&request.continuation, list(1)), terminal);
    assert!(vm.claim_effect(4, 10).unwrap().is_none());
}

#[test]
fn loop_limit_and_remaining_fuel_faults_are_durable_after_result_reentry() {
    for (fuel, revision, code) in [(100, 65, "LSV1406"), (10, 17, "LSV1001")] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let request = effect(start(&mut vm, RESULT_LOOP));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 10_000).unwrap();
        let terminal = vm.resume(&request.continuation, list(revision));
        assert!(matches!(&terminal, Step::Fault(fault) if fault.code == code));
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 10_000).unwrap();
        assert_eq!(vm.resume(&request.continuation, list(1)), terminal);
        assert!(vm.claim_effect(1, 10).unwrap().is_none());
    }
}

#[test]
fn cancellation_and_deadline_prevent_result_loop_evaluation() {
    let program = lower(&parse(&format!("fn main() = {RESULT_LOOP}"))).unwrap();
    for cancel in [false, true] {
        let mut vm = Vm::new(100);
        let request = effect(vm.start_timed(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["runtime.read"]),
            None,
            100,
            10,
        ));
        let terminal = if cancel {
            vm.cancel_effect(&request.continuation, 105)
        } else {
            vm.resume_at(&request.continuation, 110, list(65))
        };
        assert!(matches!(terminal, Step::Cancelled(_)));
        assert_eq!(vm.resume_at(&request.continuation, 111, list(65)), terminal);
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn saved_loops_reject_forged_bounds_types_effects_and_unknown_fields() {
    let request = effect(start(&mut Vm::default(), RESULT_LOOP));
    let encoded = encode_continuation(&request.continuation).unwrap();
    for (field, replacement) in [
        ("limit", serde_json::json!(1025)),
        ("limit", serde_json::json!(u64::MAX)),
        (
            "next",
            serde_json::json!({"kind": "literal", "value": {"kind": "boolean", "value": false}}),
        ),
        (
            "next",
            serde_json::json!({"kind": "local", "name": "missing"}),
        ),
        ("name", serde_json::json!("while")),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut image: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        image["result_binding"]["body"][field] = replacement;
        assert!(
            decode_continuation(&serde_json::to_vec(&image).unwrap()).is_err(),
            "{field}"
        );
    }
    let mut image = request.continuation;
    let leselang_hir::computation::Computation::Loop { next, .. } =
        &mut image.result_binding.as_mut().unwrap().body
    else {
        unreachable!()
    };
    **next = leselang_hir::computation::Computation::Host {
        effect: Box::new(Effect::RuntimeList {
            filter: Default::default(),
        }),
    };
    assert_eq!(encode_continuation(&image).unwrap_err().code, "LSV1405");
    assert!(Vm::default().restore(image).is_err());
}
