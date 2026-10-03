use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, HirProgram, lower};
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
            "leselang-functions-{}-{}",
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
fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
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

#[test]
fn helpers_compute_typed_results_without_allocating_effects() {
    for (source, expected) in [
        (
            "fn bump(n: integer) = add(left: n, right: 1)\nfn main() = bump(n: 7)",
            ScalarValue::Integer(8),
        ),
        (
            "fn flip(n: boolean) = not(value: n)\nfn main() = flip(n: false)",
            ScalarValue::Boolean(true),
        ),
        (
            "fn identity(text: string) = text\nfn main() = identity(text: \"hello\")",
            ScalarValue::String("hello".into()),
        ),
        (
            "fn nil(value: none) = value\nfn main() = nil(value: none)",
            ScalarValue::None,
        ),
        (
            "fn constant() = 42\nfn main() = constant()",
            ScalarValue::Integer(42),
        ),
        (
            "fn bump(n: integer) = add(left: n, right: 1)\nfn twice(n: integer) = bump(n: bump(n: n))\nfn main() = twice(n: 7)",
            ScalarValue::Integer(9),
        ),
        (
            "fn grow(n: integer) = loop(size: n, while: lt(left: size, right: 8), next: mul(left: size, right: 2), limit: 4)\nfn main() = grow(n: 1)",
            ScalarValue::Integer(8),
        ),
        (
            "fn number(text: string) = recover(value: parse_integer(value: text), fallback: 3)\nfn main() = number(text: \"bad\")",
            ScalarValue::Integer(3),
        ),
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            serde_json::from_slice::<HirProgram>(&serde_json::to_vec(&program).unwrap()).unwrap(),
            program
        );
        let mut vm = Vm::default();
        assert_eq!(start(&mut vm, source), done(expected), "{source}");
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
    }
}

#[test]
fn helper_parameters_and_inner_locals_cannot_capture_or_shadow_caller_state() {
    for (source, expected) in [
        (
            "fn f(n: integer) = bind(tmp: add(left: n, right: 1), body: tmp)\nfn main() = bind(n: 100, body: bind(tmp: 200, body: add(left: f(n: 7), right: add(left: n, right: tmp))))",
            308,
        ),
        (
            "fn f(n: integer) = add(left: n, right: n)\nfn main() = bind(_lf0: 10, body: add(left: f(n: _lf0), right: f(n: 2)))",
            24,
        ),
        (
            "fn f(n: integer) = choose(when: true, then: bind(tmp: n, body: tmp), otherwise: bind(tmp: 9, body: tmp))\nfn main() = add(left: f(n: 7), right: f(n: 3))",
            10,
        ),
        (
            "fn f(n: integer) = bind(total: add(left: n, right: 1), body: total)\nfn main() = loop(total: 0, while: lt(left: total, right: 3), next: f(n: total), limit: 3)",
            3,
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
fn parameters_are_eager_once_in_signature_order_with_shared_fuel() {
    let source = "fn duplicate(n: integer) = add(left: n, right: n)\nfn main() = duplicate(n: add(left: 2, right: 3))";
    assert_eq!(
        start(&mut Vm::new(7), source),
        done(ScalarValue::Integer(10))
    );
    assert!(
        matches!(start(&mut Vm::new(6), source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let source = "fn unused(n: integer) = 1\nfn main() = unused(n: div(left: 1, right: 0))";
    assert!(
        matches!(start(&mut Vm::default(), source), Step::Fault(fault) if fault.code == "LSV1401")
    );
    let source = "fn first(a: integer, b: integer) = a\nfn main() = first(b: parse_integer(value: \"secret\"), a: div(left: 1, right: 0))";
    assert!(
        matches!(start(&mut Vm::default(), source), Step::Fault(fault) if fault.code == "LSV1401")
    );
    let source = "fn first(a: integer, b: integer) = a\nfn main() = choose(when: true, then: 1, otherwise: first(a: div(left: 1, right: 0), b: 2))";
    assert_eq!(
        start(&mut Vm::default(), source),
        done(ScalarValue::Integer(1))
    );
}

#[test]
fn helper_strings_preserve_existing_bounds_and_copy_fuel_costs() {
    let source = format!(
        "fn same(text: string) = text\nfn main() = same(text: \"{}\")",
        "x".repeat(4096)
    );
    assert_eq!(
        start(&mut Vm::default(), &source),
        done(ScalarValue::String("x".repeat(4096)))
    );
    assert!(
        matches!(start(&mut Vm::new(100), &source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let source = format!(
        "fn double(text: string) = concat(left: text, right: text)\nfn main() = double(text: \"{}\")",
        "x".repeat(2049)
    );
    assert!(
        matches!(start(&mut Vm::default(), &source), Step::Fault(fault) if fault.code == "LSV1403")
    );
}

#[test]
fn computed_group_failures_from_helpers_admit_no_partial_effect() {
    for body in [
        "seq(first: ui.focus(node_id: \"a\"), second: ui.focus(node_id: node(text: \"bad\")))",
        "all(first: ui.focus(node_id: \"a\"), second: ui.focus(node_id: node(text: \"bad\")))",
        "repeat(times: 2, body: ui.focus(node_id: node(text: \"bad\")))",
    ] {
        let source = format!(
            "fn node(text: string) = to_string(value: parse_integer(value: text))\nfn main() = {body}"
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        assert!(matches!(start(&mut vm, &source), Step::Fault(fault) if fault.code == "LSV1408"));
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
            effect(start(&mut vm, "fn main() = ui.focus(node_id: \"a\")")).effect_id,
            "effect-1"
        );
    }
    let source =
        "fn same(text: string) = text\nfn main() = ui.focus(node_id: same(text: \"bad node\"))";
    assert!(
        matches!(start(&mut Vm::default(), source), Step::Fault(fault) if fault.code == "LSV1404")
    );
}

#[test]
fn expanded_result_helpers_recover_without_source_and_replay_the_committed_scalar() {
    let path = JournalPath::new();
    let source = "fn arrived(text: string) = concat(left: \"arrived-\", right: text)\nfn main() = bind(r: ui.focus(node_id: \"a\"), body: arrived(text: field(value: r, name: \"node_id\")))";
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let request = effect(start(&mut vm, source));
    assert_eq!(request.continuation.schema_version, 2);
    assert_eq!(
        decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
        request.continuation
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let terminal = done(ScalarValue::String("arrived-a".into()));
    assert_eq!(vm.resume(&request.continuation, focus("a")), terminal);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&request.continuation, focus("b")), terminal);
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn helper_dataflow_successors_preserve_identity_authority_and_existing_schemas() {
    let path = JournalPath::new();
    let source = "fn next_node(text: string) = concat(left: text, right: \"-next\")\nfn matches(text: string) = eq(left: text, right: \"a-next\")\nfn main() = bind(r: ui.focus(node_id: \"a\"), body: bind(s: ui.focus(node_id: next_node(text: field(value: r, name: \"node_id\"))), body: matches(text: field(value: s, name: \"node_id\"))))";
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 4);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, focus("a")));
    assert!(
        matches!(&second.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "a-next")
    );
    assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
    assert_ne!(first.effect_id, second.effect_id);
    assert_eq!(
        vm.resume(&first.continuation, focus("different")),
        Step::Effect(Box::new(second.clone()))
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let terminal = done(ScalarValue::Boolean(true));
    assert_eq!(vm.resume(&second.continuation, focus("a-next")), terminal);
    assert_eq!(vm.resume(&first.continuation, focus("different")), terminal);
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
}

#[test]
fn restored_helpers_never_refill_fuel_or_bypass_cancellation_and_deadline() {
    let source = "fn expensive(text: string) = concat(left: text, right: text)\nfn main() = bind(r: ui.focus(node_id: \"a\"), body: expensive(text: field(value: r, name: \"node_id\")))";
    let mut original = Vm::new(500);
    let request = effect(start(&mut original, source));
    let mut exhausted = request.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(1_000_000);
    vm.restore_request(exhausted.clone()).unwrap();
    assert!(
        matches!(vm.resume(&exhausted.continuation, focus("a")), Step::Fault(fault) if fault.code == "LSV1001")
    );
    for cancel in [false, true] {
        let mut vm = Vm::new(500);
        let program = lower(&parse(source)).unwrap();
        let request = effect(vm.start_timed(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            10,
        ));
        let terminal = if cancel {
            vm.cancel_effect(&request.continuation, 105)
        } else {
            vm.resume_at(&request.continuation, 111, focus("a"))
        };
        assert!(!matches!(&terminal, Step::Done(_)));
        assert_eq!(
            vm.resume_at(&request.continuation, 112, focus("a")),
            terminal
        );
    }
}

#[test]
fn bound_group_results_can_feed_pure_helpers_without_changing_group_recovery() {
    let path = JournalPath::new();
    let source = "fn same(text: string) = text\nfn main() = bind(g: seq(first: ui.focus(node_id: \"a\"), second: ui.focus(node_id: \"b\")), body: same(text: field(value: member(value: g, name: \"second\"), name: \"node_id\")))";
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 6);
    let second = effect(vm.resume(&first.continuation, focus("a")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let terminal = done(ScalarValue::String("b".into()));
    assert_eq!(vm.resume(&second.continuation, focus("b")), terminal);
    assert_eq!(vm.resume(&first.continuation, focus("ignored")), terminal);
}
