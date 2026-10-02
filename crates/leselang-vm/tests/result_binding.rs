use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DebuggerCancelResult, EffectError, EffectErrorClass, EffectOperation, EffectRequest,
    EffectResult, PresentationResult, RetentionPolicy, RetryDisposition, RetryPolicy, ScalarValue,
    Step, Value, Vm, decode_continuation, encode_continuation,
};
use leserpent_domain::QueryResult;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-result-binding-{}-{}",
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

fn navigate(destination: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: "a".into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: destination.into(),
    })
}

fn list(revision: u64) -> EffectResult {
    EffectResult::Query(QueryResult::RuntimeList {
        revision: Revision(revision),
        runtimes: vec![],
    })
}

fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

const NAVIGATE: &str = r#"bind(prefix: concat(left: "arrived", right: ":"), body:
    bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body:
        concat(left: prefix, right: field(value: r, name: "focused_node_id"))))"#;

#[test]
fn durable_locals_resume_from_validated_host_results_without_restarting_the_budget() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let request = effect(start(&mut vm, NAVIGATE));
    assert_eq!(request.continuation.schema_version, 2);
    let binding = request.continuation.result_binding.as_ref().unwrap();
    assert_eq!(binding.name, "r");
    assert_eq!(binding.locals.len(), 1);
    assert_eq!(
        binding.locals[0].value,
        ScalarValue::String("arrived:".into())
    );
    let encoded = encode_continuation(&request.continuation).unwrap();
    assert_eq!(decode_continuation(&encoded).unwrap(), request.continuation);
    assert_eq!(
        serde_json::from_slice::<EffectRequest>(&serde_json::to_vec(&request).unwrap()).unwrap(),
        request
    );
    let mut replaced = request.continuation.clone();
    replaced.result_binding.as_mut().unwrap().body =
        leselang_hir::computation::Computation::Literal {
            value: ScalarValue::Boolean(false),
        };
    assert!(
        matches!(vm.resume(&replaced, navigate("changed")), Step::Fault(fault) if fault.code == "LSV2005")
    );
    assert_eq!(vm.pending_count(), 1);
    drop(vm);

    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.pending_continuations(),
        std::slice::from_ref(&request.continuation)
    );
    let terminal = vm.resume(&request.continuation, navigate("actual-target"));
    assert_eq!(
        terminal,
        done(ScalarValue::String("arrived:actual-target".into()))
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        vm.resume(&request.continuation, navigate("different-target")),
        terminal
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume(&request.continuation, navigate("changed-again")),
        terminal
    );
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
    let newer = effect(start(&mut vm, "runtime.list()"));
    assert!(matches!(
        vm.resume(&newer.continuation, list(2)),
        Step::Done(_)
    ));
    let compacted = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 10,
        })
        .unwrap();
    assert_eq!(compacted.removed_records, 1);
}

#[test]
fn result_fields_support_calculation_aliases_lazy_choices_and_unit_outputs() {
    for (source, result, expected) in [
        (
            r#"bind(r: runtime.list(), body: add(left: field(value: r, name: "revision"), right: field(value: r, name: "count")))"#,
            list(17),
            ScalarValue::Integer(17),
        ),
        (
            r#"bind(r: runtime.list(), body: bind(alias: r, body: choose(when: eq(left: field(value: alias, name: "count"), right: 0), then: "empty", otherwise: "populated")))"#,
            list(1),
            ScalarValue::String("empty".into()),
        ),
        (
            r#"bind(r: runtime.list(), body: choose(when: true, then: 3, otherwise: div(left: 1, right: 0)))"#,
            list(1),
            ScalarValue::Integer(3),
        ),
        (
            r#"bind(r: runtime.list(), body: none)"#,
            list(1),
            ScalarValue::None,
        ),
        (
            r#"bind(role: none, body: bind(r: runtime.list(role: role), body: eq(left: role, right: none)))"#,
            list(1),
            ScalarValue::Boolean(true),
        ),
        (
            r#"bind(node: concat(left: "a", right: ""), body: bind(r: ui.focus(node_id: node), body: eq(left: field(value: r, name: "node_id"), right: node)))"#,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "a".into(),
            }),
            ScalarValue::Boolean(true),
        ),
        (
            r#"bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body: ne(left: field(value: r, name: "node_id"), right: field(value: r, name: "focused_node_id")))"#,
            navigate("b"),
            ScalarValue::Boolean(true),
        ),
        (
            r#"bind(r: ui.assert_child_count(node_id: "a", count: "3"), body: field(value: r, name: "count"))"#,
            EffectResult::Presentation(PresentationResult::AssertChildCount {
                node_id: "a".into(),
                count: 3,
            }),
            ScalarValue::Integer(3),
        ),
        (
            r#"bind(r: ui.assert_form_field_max_length(node_id: "a", field: "name", max_length: "128"), body: field(value: r, name: "max_length"))"#,
            EffectResult::Presentation(PresentationResult::AssertFormFieldMaxLength {
                node_id: "a".into(),
                field: "name".into(),
                max_length: 128,
            }),
            ScalarValue::Integer(128),
        ),
    ] {
        let mut vm = Vm::default();
        let request = effect(start(&mut vm, source));
        assert_eq!(
            vm.resume(&request.continuation, result),
            done(expected),
            "{source}"
        );
    }
    let mut vm = Vm::default();
    assert_eq!(
        start(
            &mut vm,
            r#"choose(when: false, then: bind(r: runtime.list(), body: div(left: 1, right: 0)), otherwise: 5)"#
        ),
        done(ScalarValue::Integer(5))
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(
        effect(start(&mut vm, "runtime.list()")).effect_id,
        "effect-1"
    );
}

#[test]
fn captured_mutations_still_require_confirmed_correlated_dispatch_acknowledgement() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let program = lower(&parse(
        r#"fn main() = bind(r: debugger.cancel(session_id: "session-a"), body: true)"#,
    ))
    .unwrap();
    let request = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["debugger.control"]),
        Some(Revision(7)),
    ));
    let EffectOperation::Command(command) = &request.operation else {
        panic!("expected command");
    };
    assert_eq!(
        command.confirmation,
        leselang_host_contract::Confirmation::Confirmed
    );
    let result = EffectResult::DebuggerCancel(DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: "session-a".into(),
        observed_at_ms: 1_001,
    });
    assert!(
        matches!(vm.resume(&request.continuation, result.clone()), Step::Fault(fault) if fault.code == "LSV2110")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let lease = vm.claim_effect(1_000, 50).unwrap().unwrap();
    assert_eq!(
        vm.acknowledge_effect(&lease, 1_001, result.clone()),
        done(ScalarValue::Boolean(true))
    );
    assert_eq!(
        vm.acknowledge_effect(&lease, 1_002, result),
        done(ScalarValue::Boolean(true))
    );
}

#[test]
fn captured_queries_cannot_ignore_revision_or_output_limits() {
    let program = lower(&parse("fn main() = bind(r: runtime.list(), body: 1)")).unwrap();
    let mut vm = Vm::default();
    let request = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["runtime.read"]),
        Some(Revision(7)),
    ));
    assert!(
        matches!(vm.resume(&request.continuation, list(8)), Step::Fault(fault) if fault.code == "LSV2101")
    );
    let mut image = effect(start(&mut vm, "bind(r: runtime.list(), body: 1)")).continuation;
    image.max_output_items = 0;
    let mut target = Vm::default();
    target.restore(image.clone()).unwrap();
    assert!(
        matches!(target.resume(&image, list(1)), Step::Fault(fault) if fault.code == "LSV2102")
    );
}

#[test]
fn post_result_faults_are_durable_and_never_turn_an_invalid_reply_into_success() {
    for (source, fuel, result, code) in [
        (
            r#"bind(r: runtime.list(), body: add(left: field(value: r, name: "revision"), right: 1))"#,
            100,
            list(u64::MAX),
            "LSV1401",
        ),
        (
            r#"bind(r: runtime.list(), body: add(left: field(value: r, name: "revision"), right: 1))"#,
            3,
            list(1),
            "LSV1001",
        ),
        (
            r#"bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body: 1)"#,
            100,
            navigate("bad target"),
            "LSV2103",
        ),
        (
            r#"bind(r: runtime.list(), body: 1)"#,
            100,
            navigate("b"),
            "LSV2103",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let request = effect(start(&mut vm, source));
        let terminal = vm.resume(&request.continuation, result);
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == code),
            "{terminal:?}"
        );
        drop(vm);
        let mut recovered = Vm::open_journal(&path.0, 1_000).unwrap();
        assert_eq!(recovered.resume(&request.continuation, list(2)), terminal);
        assert!(recovered.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn retry_lease_deadline_and_cancellation_still_fence_result_completion() {
    let path = JournalPath::new();
    let program = lower(&parse(&format!("fn main() = {NAVIGATE}"))).unwrap();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let request = effect(vm.start_timed(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        100,
        1000,
    ));
    let lease = vm.claim_effect(101, 50).unwrap().unwrap();
    assert!(matches!(
        vm.report_effect_error(
            &lease,
            102,
            EffectError {
                class: EffectErrorClass::Transient,
                code: "busy".into(),
                message: "retry".into()
            },
            &RetryPolicy::default()
        )
        .unwrap(),
        RetryDisposition::Scheduled(_)
    ));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let retry = vm.claim_effect(352, 100).unwrap().unwrap();
    assert_eq!(retry.request, request);
    assert!(matches!(
        vm.acknowledge_effect(&lease, 353, navigate("stale")),
        Step::Fault(_)
    ));
    let terminal = vm.acknowledge_effect(&retry, 354, navigate("b"));
    assert_eq!(terminal, done(ScalarValue::String("arrived:b".into())));
    assert_eq!(vm.acknowledge_effect(&retry, 355, navigate("c")), terminal);

    for cancel_at in [105, 110] {
        let mut vm = Vm::new(100);
        let failing = lower(&parse(
            "fn main() = bind(r: runtime.list(), body: div(left: 1, right: 0))",
        ))
        .unwrap();
        let request = effect(vm.start_timed(
            &failing,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["runtime.read"]),
            None,
            100,
            10,
        ));
        let terminal = vm.cancel_effect(&request.continuation, cancel_at);
        assert!(matches!(terminal, Step::Cancelled(_)));
        assert_eq!(vm.resume_at(&request.continuation, 111, list(1)), terminal);
    }
}

#[test]
fn competing_result_transforms_keep_the_first_durable_output_and_rollback_is_retryable() {
    let path = JournalPath::new();
    let mut first = Vm::open_journal(&path.0, 100).unwrap();
    let request = effect(start(&mut first, NAVIGATE));
    let mut second = Vm::open_journal(&path.0, 1).unwrap();
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_result BEFORE UPDATE OF terminal_step ON vm_effects BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(matches!(
        first.resume(&request.continuation, navigate("not-committed")),
        Step::Fault(_)
    ));
    assert_eq!(first.pending_count(), 1);
    connection
        .execute_batch("DROP TRIGGER reject_result;")
        .unwrap();
    let winner = second.resume(&request.continuation, navigate("winner"));
    assert_eq!(winner, done(ScalarValue::String("arrived:winner".into())));
    assert_eq!(
        first.resume(&request.continuation, navigate("loser")),
        winner
    );
    drop(first);
    drop(second);
    let mut restarted = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        restarted.resume(&request.continuation, navigate("later")),
        winner
    );
}

#[test]
fn snapshots_are_bounded_and_forged_environments_are_not_restored() {
    let mut body = format!("\"{}\"", "x".repeat(4096));
    for _ in 0..4 {
        body = format!("concat(left: {body}, right: {body})");
    }
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10_000).unwrap();
    assert!(
        matches!(start(&mut vm, &format!("bind(r: runtime.list(), body: {body})")), Step::Fault(fault) if fault.code == "LSV3002")
    );
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(1, 10).unwrap().is_none());
    let valid = effect(start(&mut vm, NAVIGATE)).continuation;
    let mut deep = leselang_hir::computation::Computation::Literal {
        value: ScalarValue::Boolean(true),
    };
    for _ in 0..64 {
        deep = leselang_hir::computation::Computation::Unary {
            operator: leselang_hir::computation::UnaryOperator::Not,
            value: Box::new(deep),
        };
    }
    let mut invalid = valid.clone();
    invalid.result_binding.as_mut().unwrap().body = deep;
    assert_eq!(encode_continuation(&invalid).unwrap_err().code, "LSV1405");
    let mut invalid = valid.clone();
    let binding = invalid.result_binding.as_mut().unwrap();
    binding.locals.push(binding.locals[0].clone());
    assert_eq!(Vm::default().restore(invalid).unwrap_err().code, "LSV1405");
    let mut invalid = valid.clone();
    invalid.result_binding.as_mut().unwrap().locals[0].value =
        ScalarValue::String("x".repeat(4097));
    assert_eq!(Vm::default().restore(invalid).unwrap_err().code, "LSV1405");
    let mut invalid = valid.clone();
    invalid.result_binding.as_mut().unwrap().locals.clear();
    assert_eq!(Vm::default().restore(invalid).unwrap_err().code, "LSV1405");
    let connection = rusqlite::Connection::open(&path.0).unwrap();
    let mut corrupt = valid.clone();
    corrupt.result_binding.as_mut().unwrap().name = "prefix".into();
    connection
        .execute(
            "UPDATE vm_effects SET image = ?1 WHERE token = ?2",
            rusqlite::params![serde_json::to_vec(&corrupt).unwrap(), valid.token.as_str()],
        )
        .unwrap();
    drop(vm);
    assert!(Vm::open_journal(&path.0, 100).is_err());
}

#[test]
fn result_binding_wire_is_strict_and_legacy_atomic_continuations_remain_unchanged() {
    let mut vm = Vm::default();
    let image = effect(start(&mut vm, NAVIGATE)).continuation;
    let encoded = serde_json::to_value(&image).unwrap();
    for mutate in [
        |json: &mut serde_json::Value| {
            json["schema_version"] = 1.into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"] = serde_json::Value::Null;
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["extra"] = true.into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["name"] = "prefix".into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["locals"][0]["extra"] = true.into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["right"]["field"] = "unknown".into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"]["right"]["field"] = "revision".into();
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"] = serde_json::json!({"kind":"local", "name":"missing"});
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"] = serde_json::json!({"kind":"local", "name":"r"});
        },
        |json: &mut serde_json::Value| {
            json["result_binding"]["body"] =
                serde_json::json!({"kind":"host", "effect":{"kind":"runtime_list", "filter":{}}});
        },
    ] {
        let mut invalid = encoded.clone();
        mutate(&mut invalid);
        assert!(
            decode_continuation(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{invalid}"
        );
    }
    let mut target = Vm::new(1);
    target
        .restore(decode_continuation(&serde_json::to_vec(&encoded).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        target.resume(&image, navigate("restored")),
        done(ScalarValue::String("arrived:restored".into()))
    );
    let atomic = effect(start(&mut vm, "runtime.list()")).continuation;
    assert_eq!(atomic.schema_version, 1);
    assert!(atomic.result_binding.is_none());
    let json = serde_json::to_value(&atomic).unwrap();
    assert!(json.get("result_binding").is_none());
    assert_eq!(
        decode_continuation(&serde_json::to_vec(&json).unwrap()).unwrap(),
        atomic
    );
}
