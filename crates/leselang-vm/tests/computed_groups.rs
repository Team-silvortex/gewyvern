use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectRequest, EffectResult, PresentationResult,
    RetryDisposition, RetryPolicy, Step, Value, Vm, decode_continuation, encode_continuation,
};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-computed-group-{}-{}",
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
        other => panic!("expected effect, got {other:?}"),
    }
}

fn result(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}

fn node(request: &EffectRequest) -> &str {
    let Effect::UiFocus { node_id } = &request.continuation.pending_effect else {
        panic!("expected focus")
    };
    node_id
}

#[test]
fn computed_sequence_and_repeat_recover_resolved_arguments_without_recomputing() {
    let path = JournalPath::new();
    let source = r#"bind(prefix: "runtime-", body: seq(
        first: ui.focus(node_id: concat(left: prefix, right: "a")),
        again: repeat(times: 2, body: ui.focus(node_id: concat(left: prefix, right: "b")))))"#;
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let first = effect(start(&mut vm, source));
    assert_eq!(node(&first), "runtime-a");
    assert_eq!(first.budget.fuel_remaining, 68);
    assert_eq!(vm.pending_count(), 3);
    for image in vm.pending_continuations() {
        assert!(matches!(image.pending_effect, Effect::UiFocus { .. }));
        assert_eq!(
            decode_continuation(&encode_continuation(&image).unwrap()).unwrap(),
            image
        );
    }
    let first_lease = vm.claim_effect(1, 100).unwrap().unwrap();
    assert!(vm.claim_effect(2, 100).unwrap().is_none());
    assert!(matches!(
        vm.report_effect_error(
            &first_lease,
            2,
            EffectError {
                class: EffectErrorClass::Transient,
                code: "busy".into(),
                message: "retry later".into()
            },
            &RetryPolicy::default()
        )
        .unwrap(),
        RetryDisposition::Scheduled(_)
    ));
    drop(vm);

    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert!(vm.claim_effect(251, 100).unwrap().is_none());
    let redelivery = vm.claim_effect(252, 100).unwrap().unwrap();
    assert_eq!(redelivery.request, first);
    let second = effect(vm.acknowledge_effect(&redelivery, 253, result("runtime-a")));
    assert_eq!(node(&second), "runtime-b");
    assert_eq!(second.budget.fuel_remaining, 67);
    let third = effect(vm.resume(&second.continuation, result("runtime-b")));
    assert_eq!(node(&third), "runtime-b");
    assert_eq!(third.budget.fuel_remaining, 66);
    assert_ne!(first.effect_id, second.effect_id);
    assert_ne!(second.effect_id, third.effect_id);
    let done = vm.resume(&third.continuation, result("runtime-b"));
    let Step::Done(Value::Structured { fields }) = &done else {
        panic!("expected merge result: {done:?}")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "again__iteration_1", "again__iteration_2"]
    );
    drop(vm);
    let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        recovered.resume(&first.continuation, result("runtime-a")),
        done
    );
    assert!(recovered.claim_effect(254, 100).unwrap().is_none());
}

#[test]
fn later_argument_failures_leave_the_entire_group_unallocated_and_unjournaled() {
    for (source, fuel, code) in [
        (
            r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: concat(left: "bad", right: " node")))"#,
            100,
            "LSV1404",
        ),
        (
            r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: concat(left: "bad", right: " node")))"#,
            100,
            "LSV1404",
        ),
        (
            r#"repeat(times: 2, body: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: concat(left: "bad", right: " node"))))"#,
            100,
            "LSV1404",
        ),
        (
            r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: bind(n: div(left: 1, right: 0), body: "b")))"#,
            100,
            "LSV1401",
        ),
        (
            r#"seq(a: ui.focus(node_id: concat(left: "a", right: "")), b: ui.focus(node_id: "b"))"#,
            8,
            "LSV1001",
        ),
        (
            r#"seq(a: ui.focus(node_id: concat(left: "a", right: "")), b: ui.focus(node_id: "b"))"#,
            10,
            "LSV1001",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let step = start(&mut vm, source);
        assert!(
            matches!(&step, Step::Fault(fault) if fault.code == code),
            "{source}: {step:?}"
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        assert!(vm.claim_effect(1, 10).unwrap().is_none());
        let connection = rusqlite::Connection::open(&path.0).unwrap();
        for table in [
            "vm_effects",
            "vm_dispatches",
            "vm_merge_groups",
            "vm_merge_branches",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0);
        }
        assert_eq!(
            effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn computed_parallel_groups_prepare_once_then_keep_existing_parallel_merge_semantics() {
    let path = JournalPath::new();
    let source = r#"bind(node: "a", body: all(first: ui.focus(node_id: node), second: ui.focus(node_id: node)))"#;
    let mut vm = Vm::open_journal(&path.0, 20).unwrap();
    let Step::Effects(batch) = start(&mut vm, source) else {
        panic!("expected parallel group")
    };
    assert_eq!(
        batch
            .branches
            .iter()
            .map(|branch| branch.branch.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        batch
            .branches
            .iter()
            .all(|branch| branch.request.budget.fuel_remaining == 7)
    );
    let first = vm.claim_effect(1, 100).unwrap().unwrap();
    let second = vm.claim_effect(2, 100).unwrap().unwrap();
    assert_ne!(first.request.effect_id, second.request.effect_id);
    assert!(matches!(
        vm.acknowledge_effect(&second, 3, result("a")),
        Step::Waiting(_)
    ));
    drop(vm);
    let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
    let done = recovered.acknowledge_effect(&first, 4, result("a"));
    assert!(matches!(&done, Step::Done(Value::Structured { fields }) if fields.len() == 2));
    assert_eq!(
        recovered.merge_result(&batch.merge_token).unwrap(),
        Some(done)
    );
    assert_eq!(recovered.pending_count(), 0);
}

#[test]
fn preparation_stays_lazy_at_outer_decisions_but_cannot_skip_authority_checks() {
    let mut vm = Vm::new(20);
    let source = r#"choose(when: true,
        then: seq(a: ui.focus(node_id: concat(left: "a", right: ""))),
        otherwise: seq(b: ui.focus(node_id: bind(n: div(left: 1, right: 0), body: "b"))))"#;
    let selected = effect(start(&mut vm, source));
    assert_eq!(node(&selected), "a");
    let program = lower(&parse(
        r#"fn main() = bind(node: "a", body: choose(when: true,
        then: seq(a: ui.focus(node_id: node)),
        otherwise: seq(b: runtime.refresh(runtime_id: node))))"#,
    ))
    .unwrap();
    let mut vm = Vm::new(20);
    assert!(
        matches!(vm.start(&program, Principal::new("operator").unwrap(), CapabilitySet::new(["ui.presentation"]), None), Step::Fault(fault) if fault.code == "LSH2001")
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
        "effect-1"
    );
}

#[test]
fn computed_sequences_preserve_deadlines_and_cancel_blocked_successors() {
    let source =
        r#"fn main() = bind(node: "a", body: repeat(times: 3, body: ui.focus(node_id: node)))"#;
    for expire in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100).unwrap();
        let first = effect(vm.start_timed(
            &lower(&parse(source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            20,
        ));
        assert!(
            vm.pending_continuations()
                .iter()
                .all(|image| image.deadline_at_ms == Some(120))
        );
        let future = vm
            .pending_continuations()
            .into_iter()
            .find(|image| image.token != first.continuation.token)
            .unwrap();
        assert!(
            matches!(vm.resume_at(&future, 101, result("a")), Step::Fault(fault) if fault.code == "LSV2410")
        );
        let terminal = if expire {
            vm.resume_at(&first.continuation, 120, result("a"))
        } else {
            vm.cancel_effect(&first.continuation, 101)
        };
        assert!(matches!(terminal, Step::Cancelled(_)));
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert!(vm.claim_effect(121, 10).unwrap().is_none());
        assert_eq!(
            vm.resume_at(&first.continuation, 121, result("a")),
            terminal
        );
    }
}

#[test]
fn computed_group_wire_is_strict_and_never_becomes_a_durable_pending_effect() {
    use leselang_hir::computation::Computation;
    let program = lower(&parse(
        r#"fn main() = seq(a: ui.focus(node_id: concat(left: "a", right: "")))"#,
    ))
    .unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computed group")
    };
    let wire = serde_json::to_value(expression.as_ref()).unwrap();
    assert_eq!(
        serde_json::from_value::<Computation>(wire.clone()).unwrap(),
        **expression
    );
    let mut extra = wire.clone();
    extra["branches"][0]["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Computation>(extra).is_err());
    let mut invalid_kind = wire;
    invalid_kind["group_kind"] = serde_json::json!("unbounded");
    assert!(serde_json::from_value::<Computation>(invalid_kind).is_err());
    let mut vm = Vm::new(20);
    let mut image = effect(start(
        &mut vm,
        r#"seq(a: ui.focus(node_id: concat(left: "a", right: "")))"#,
    ))
    .continuation;
    image.pending_effect = program.function.effect;
    assert!(encode_continuation(&image).is_err());
}

#[test]
fn computed_filters_cannot_create_groups_that_the_recovery_decoder_would_reject() {
    for field in ["environment", "cluster", "role"] {
        for value in ["x".repeat(129), "bad\tvalue".into()] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 100).unwrap();
            let literal = serde_json::to_string(&value).unwrap();
            let direct = format!("fn main() = runtime.list({field}: {literal})");
            assert!(
                lower(&parse(&direct))
                    .unwrap_err()
                    .iter()
                    .any(|error| error.code == "LSH1109")
            );
            let source = format!(
                "seq(first: ui.focus(node_id: \"a\"), later: runtime.list({field}: concat(left: {literal}, right: \"\")))"
            );
            assert!(
                matches!(start(&mut vm, &source), Step::Fault(fault) if fault.code == "LSV1404")
            );
            assert_eq!(vm.pending_count(), 0);
            assert_eq!(
                effect(start(&mut vm, r#"ui.focus(node_id: "a")"#)).effect_id,
                "effect-1"
            );
            drop(vm);
            let vm = Vm::open_journal(&path.0, 1).unwrap();
            assert_eq!(vm.pending_count(), 1);
        }
    }
}
