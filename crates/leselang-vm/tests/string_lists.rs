use leselang_hir::computation::{Computation, OptionalStringValue, StringListValue};
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationOperation, ScalarValue, Step, Value,
    Vm, decode_continuation, encode_continuation,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-string-lists-{}-{}",
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
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        100,
    )
}
fn calculate(body: &str, fuel: u64) -> Step {
    start(&mut Vm::new(fuel), &format!("fn main() = {body}"))
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("{other:?}"),
    }
}
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}
fn list(values: &[&str]) -> ScalarValue {
    ScalarValue::StringList(StringListValue(
        values.iter().map(|value| (*value).to_owned()).collect(),
    ))
}
fn optional(value: Option<&str>) -> ScalarValue {
    ScalarValue::OptionalString(OptionalStringValue(value.map(str::to_owned)))
}
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn fault(step: Step, code: &str) {
    assert!(
        matches!(step, Step::Fault(ref fault) if fault.code == code),
        "{step:?}"
    );
}
fn flow() -> &'static str {
    r#"fn main() = bind(first: ui.assert_text(node_id: "status", expected: "ready-a,skip,ready-b"), body:
        bind(parts: split(left: field(value: first, name: "expected"), right: ","), body:
            bind(selected: fold(output: strings(), items: parts, item: "part", next: choose(when: starts_with(left: part, right: "ready-"), then: append(left: output, right: part), otherwise: output), limit: 64), body:
                bind(last: ui.set_form_value(node_id: "form", field: "selected", value: join(left: selected, right: ";")), body: selected))))"#
}

#[test]
fn strings_split_append_join_and_index_preserve_order_unicode_and_empty_entries() {
    for (body, expected) in [
        (r#"strings()"#, list(&[])),
        (
            r#"strings(z: "last", a: "first")"#,
            list(&["last", "first"]),
        ),
        (
            r#"strings(z: to_string(value: 9), a: "")"#,
            list(&["9", ""]),
        ),
        (
            r#"split(left: ",a,,b,", right: ",")"#,
            list(&["", "a", "", "b", ""]),
        ),
        (r#"split(left: "", right: ",")"#, list(&[""])),
        (r#"split(left: "ab", right: "")"#, list(&["", "a", "b", ""])),
        (r#"split(left: "", right: "")"#, list(&["", ""])),
        (r#"append(left: strings(), right: "")"#, list(&[""])),
        (
            r#"join(left: strings(a: "", b: "a", c: ""), right: ",")"#,
            ScalarValue::String(",a,".into()),
        ),
        (
            r#"join(left: strings(), right: ",")"#,
            ScalarValue::String("".into()),
        ),
        (
            r#"item_at(left: strings(a: ""), right: 0)"#,
            optional(Some("")),
        ),
        (r#"item_at(left: strings(a: ""), right: 1)"#, optional(None)),
        (
            r#"item_at(left: strings(a: "x"), right: 18446744073709551615)"#,
            optional(None),
        ),
        (
            r#"len(value: split(left: ",", right: ","))"#,
            ScalarValue::Integer(2),
        ),
        (
            r#"eq(left: strings(a: "a", b: "b"), right: strings(z: "a", x: "b"))"#,
            ScalarValue::Boolean(true),
        ),
        (
            r#"ne(left: strings(a: "a", b: "b"), right: strings(a: "b", b: "a"))"#,
            ScalarValue::Boolean(true),
        ),
    ] {
        assert_eq!(calculate(body, 10000), done(expected), "{body}");
    }
    let text = "\u{754c}\u{1f642}e\u{301}";
    assert_eq!(
        calculate(&format!("split(left: {}, right: \"\")", quote(text)), 1000),
        done(list(&["", "\u{754c}", "\u{1f642}", "e", "\u{301}", ""]))
    );
}

#[test]
fn folds_compute_filter_and_nested_aggregates_with_hygienic_helpers() {
    assert_eq!(
        calculate(
            r#"fold(total: 0, items: split(left: "2,3,5", right: ","), item: "part", next: add(left: total, right: parse_integer(value: part)), limit: 3)"#,
            1000
        ),
        done(ScalarValue::Integer(10))
    );
    assert_eq!(
        calculate(
            r#"fold(text: "", items: strings(z: "b", a: "a"), item: "part", next: concat(left: text, right: part), limit: 2)"#,
            1000
        ),
        done(ScalarValue::String("ba".into()))
    );
    assert_eq!(
        calculate(
            r#"fold(total: 7, items: strings(), item: "part", next: div(left: total, right: 0), limit: 0)"#,
            100
        ),
        done(ScalarValue::Integer(7))
    );
    assert_eq!(
        calculate(
            r#"fold(total: 0, items: strings(a: "1:2", b: "3:4"), item: "row", next: add(left: total, right: fold(sum: 0, items: split(left: row, right: ":"), item: "part", next: add(left: sum, right: parse_integer(value: part)), limit: 2)), limit: 2)"#,
            10000
        ),
        done(ScalarValue::Integer(10))
    );
    let source = r#"fn size(parts: string_list) = fold(total: 0, items: parts, item: "entry", next: add(left: total, right: len(value: entry)), limit: 64)
        fn main() = bind(entry: "outer", body: bind(total: 99, body: add(left: size(parts: strings(a: "a", b: "bc")), right: size(parts: strings(a: entry)))))"#;
    assert_eq!(
        start(&mut Vm::new(1000), source),
        done(ScalarValue::Integer(8))
    );
}

#[test]
fn limits_are_preflighted_without_truncation_and_cannot_be_recovered() {
    fault(
        calculate(
            r#"fold(total: 0, items: strings(a: "bad", b: "bad"), item: "part", next: parse_integer(value: part), limit: 1)"#,
            1000,
        ),
        "LSV1406",
    );
    fault(
        calculate(
            r#"recover(value: fold(total: 0, items: strings(a: "bad"), item: "part", next: parse_integer(value: part), limit: 0), fallback: 9)"#,
            1000,
        ),
        "LSV1406",
    );
    assert_eq!(
        calculate(
            r#"bind(part: "outer", body: recover(value: fold(total: 0, items: strings(a: "bad"), item: "entry", next: parse_integer(value: entry), limit: 1), fallback: len(value: part)))"#,
            1000
        ),
        done(ScalarValue::Integer(5))
    );
    let cases = [
        format!("split(left: {}, right: \",\")", quote(&",".repeat(64))),
        format!("split(left: {}, right: \"\")", quote(&"x".repeat(63))),
        format!(
            "append(left: strings(a: {}), right: \"x\")",
            quote(&"x".repeat(4096))
        ),
        format!(
            "join(left: strings(a: {}, b: {}), right: \",\")",
            quote(&"x".repeat(2048)),
            quote(&"x".repeat(2048))
        ),
        format!(
            "bind(text: {}, body: strings(a: text, b: \"x\"))",
            quote(&"x".repeat(4096))
        ),
    ];
    for body in cases {
        fault(calculate(&body, 10000), "LSV1403");
        let fallback = if body.starts_with("join") {
            "\"fallback\""
        } else {
            "strings()"
        };
        fault(
            calculate(
                &format!("recover(value: {body}, fallback: {fallback})"),
                10000,
            ),
            "LSV1403",
        );
    }
    let maximum = format!("split(left: {}, right: \",\")", quote(&",".repeat(63)));
    assert_eq!(
        calculate(&format!("len(value: {maximum})"), 1000),
        done(ScalarValue::Integer(64))
    );
    fault(
        calculate(&format!("append(left: {maximum}, right: \"\")"), 1000),
        "LSV1403",
    );
}

#[test]
fn collection_copies_and_each_iteration_consume_shared_fuel() {
    assert_eq!(calculate(r#"strings(a: "x")"#, 3), done(list(&["x"])));
    fault(calculate(r#"strings(a: "x")"#, 2), "LSV1001");
    assert_eq!(
        calculate(r#"split(left: "x", right: ",")"#, 9),
        done(list(&["x"]))
    );
    fault(calculate(r#"split(left: "x", right: ",")"#, 8), "LSV1001");
    let body = r#"fold(total: 0, items: strings(a: "x"), item: "part", next: total, limit: 1)"#;
    assert_eq!(calculate(body, 10), done(ScalarValue::Integer(0)));
    fault(calculate(body, 9), "LSV1001");
    fault(
        calculate(&format!("recover(value: {body}, fallback: 0)"), 9),
        "LSV1001",
    );
    let items = (0..64)
        .map(|index| format!("i{index}: \"\""))
        .collect::<Vec<_>>()
        .join(", ");
    assert_eq!(
        calculate(&format!("strings({items})"), 65),
        done(list(&vec![""; 64]))
    );
    fault(calculate(&format!("strings({items})"), 64), "LSV1001");
}

#[test]
fn list_wire_is_closed_bounded_and_requires_an_explicit_array_payload() {
    let value = list(&["a", ""]);
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        serde_json::json!({"kind":"string_list","value":["a", ""]})
    );
    assert_eq!(
        serde_json::from_value::<ScalarValue>(serde_json::to_value(value.clone()).unwrap())
            .unwrap(),
        value
    );
    for payload in [
        serde_json::Value::Null,
        serde_json::json!(false),
        serde_json::json!([null]),
        serde_json::json!([1]),
        serde_json::json!([[]]),
        serde_json::json!({}),
        serde_json::json!(vec![""; 65]),
        serde_json::json!(["x".repeat(4097)]),
        serde_json::json!(["x".repeat(2049), "x".repeat(2048)]),
    ] {
        assert!(
            serde_json::from_value::<ScalarValue>(
                serde_json::json!({"kind":"string_list","value":payload})
            )
            .is_err()
        );
    }
    for wire in [
        serde_json::json!({"kind":"string_list"}),
        serde_json::json!({"kind":"string_list","value":[],"extra":0}),
    ] {
        assert!(serde_json::from_value::<ScalarValue>(wire).is_err());
    }
    assert!(
        serde_json::from_value::<ScalarValue>(
            serde_json::json!({"kind":"string_list","value":vec!["x".repeat(64);64]})
        )
        .is_ok()
    );
}

#[test]
fn collection_locals_survive_durable_reentry_and_replay_without_source_or_fuel_refill() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, flow()));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert!(
        matches!(&second.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == "ready-a;ready-b"))
    );
    assert_eq!(second.continuation.expected_revision, Some(Revision(7)));
    assert_eq!(second.continuation.deadline_at_ms, Some(200));
    assert!(second.continuation.fuel_remaining < first.continuation.fuel_remaining);
    assert_eq!(
        decode_continuation(&encode_continuation(&second.continuation).unwrap()).unwrap(),
        second.continuation
    );
    let saved = second.continuation.result_binding.as_ref().unwrap();
    assert!(
        saved
            .locals
            .iter()
            .any(|local| local.name == "selected" && local.value == list(&["ready-a", "ready-b"]))
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Effect(Box::new(second.clone()))
    );
    let terminal = done(list(&["ready-a", "ready-b"]));
    assert_eq!(
        vm.resume_at(&second.continuation, 103, receipt(&second)),
        terminal
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 104, receipt(&first)),
        terminal
    );
    assert_eq!(
        vm.resume_at(&second.continuation, 104, receipt(&first)),
        terminal
    );
}

#[test]
fn corrupted_saved_collections_fold_nodes_and_unknown_fields_are_rejected() {
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, flow()));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let wire = serde_json::to_value(&second.continuation).unwrap();
    for payload in [
        serde_json::json!(vec![""; 65]),
        serde_json::json!(["x".repeat(4097)]),
        serde_json::Value::Null,
        serde_json::json!([false]),
    ] {
        let mut changed = wire.clone();
        changed["result_binding"]["locals"][0]["value"]["value"] = payload;
        assert!(decode_continuation(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
    let body = lower(&parse(
        r#"fn main() = fold(total: 0, items: strings(), item: "part", next: total, limit: 0)"#,
    ))
    .unwrap()
    .function
    .effect;
    let leselang_hir::Effect::Compute { expression } = body else {
        panic!()
    };
    let mut node = serde_json::to_value(&expression).unwrap();
    node["extra"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Computation>(node).is_err());
    let first_wire = serde_json::to_value(&first.continuation).unwrap();
    for (field, value) in [
        ("limit", serde_json::json!(65)),
        ("item", serde_json::json!("output")),
        (
            "next",
            serde_json::json!({"kind":"literal","value":{"kind":"integer","value":0}}),
        ),
    ] {
        let mut changed = first_wire.clone();
        changed["result_binding"]["body"]["body"]["value"][field] = value;
        assert!(
            decode_continuation(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn collection_driven_host_arguments_do_not_bypass_domain_validation_or_group_preflight() {
    for kind in ["seq", "all"] {
        for value in [
            r#"join(left: strings(a: "a", b: "b"), right: "\n")"#.to_owned(),
            format!(
                "join(left: strings(a: {}), right: \"\")",
                quote(&"x".repeat(257))
            ),
        ] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let source = format!(
                r#"fn main() = {kind}(valid: ui.focus(node_id: "a"), invalid: ui.set_form_value(node_id: "form", field: "value", value: {value}))"#
            );
            fault(start(&mut vm, &source), "LSV1404");
            assert_eq!(vm.pending_count(), 0);
            assert_eq!(
                Connection::open(&path.0)
                    .unwrap()
                    .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}

#[test]
fn pure_fold_sql_failures_roll_back_selected_successors_and_list_terminals() {
    for finishing in [false, true] {
        let writes: &[(&str, &str)] = if finishing {
            &[("vm_effects", "UPDATE"), ("vm_dispatches", "UPDATE")]
        } else {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_effects", "INSERT"),
                ("vm_dispatches", "INSERT"),
            ]
        };
        for (table, operation) in writes {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let first = effect(start(&mut vm, flow()));
            let current = if finishing {
                effect(vm.resume_at(&first.continuation, 101, receipt(&first)))
            } else {
                first
            };
            let connection = Connection::open(&path.0).unwrap();
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
                .unwrap();
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume_at(&current.continuation, 102, receipt(&current)),
                Step::Fault(_)
            ));
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                count
            );
            assert_eq!(
                connection
                    .query_row(
                        "SELECT state FROM vm_effects WHERE token = ?1",
                        [current.continuation.token.as_str()],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                "pending"
            );
            drop(vm);
            connection
                .execute_batch("DROP TRIGGER reject_transition")
                .unwrap();
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let mut step = vm.resume_at(&current.continuation, 103, receipt(&current));
            if !finishing {
                let next = effect(step);
                step = vm.resume_at(&next.continuation, 104, receipt(&next));
            }
            assert_eq!(step, done(list(&["ready-a", "ready-b"])));
        }
    }
}

#[test]
fn group_result_folds_wait_for_every_member_before_selecting_a_tail() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn main() = bind(group: {kind}(one: ui.assert_text(node_id: "a", expected: "2,3"), two: ui.assert_text(node_id: "b", expected: "5")), body:
            bind(parts: append(left: split(left: field(value: member(value: group, name: "one"), name: "expected"), right: ","), right: field(value: member(value: group, name: "two"), name: "expected")), body:
                bind(last: ui.set_form_value(node_id: "form", field: "total", value: to_string(value: fold(total: 0, items: parts, item: "part", next: add(left: total, right: parse_integer(value: part)), limit: 64))), body: parts)))"#
        );
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let (first, tail) = match start(&mut vm, &source) {
            Step::Effect(first) => {
                let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                (
                    *first,
                    effect(vm.resume_at(&second.continuation, 102, receipt(&second))),
                )
            }
            Step::Effects(batch) => {
                assert_eq!(kind, "all");
                assert!(matches!(
                    vm.resume_at(
                        &batch.branches[1].request.continuation,
                        101,
                        receipt(&batch.branches[1].request)
                    ),
                    Step::Waiting(_)
                ));
                assert_eq!(
                    Connection::open(&path.0)
                        .unwrap()
                        .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row
                            .get::<_, i64>(0))
                        .unwrap(),
                    2
                );
                let first = batch.branches[0].request.clone();
                drop(vm);
                vm = Vm::open_journal(&path.0, 1).unwrap();
                let tail = effect(vm.resume_at(&first.continuation, 102, receipt(&first)));
                (first, tail)
            }
            other => panic!("{other:?}"),
        };
        assert!(
            matches!(&tail.operation, EffectOperation::Presentation(envelope) if matches!(&envelope.operation, PresentationOperation::SetFormValue { value, .. } if value == "10"))
        );
        assert_eq!(
            vm.resume_at(&tail.continuation, 103, receipt(&tail)),
            done(list(&["2", "3", "5"]))
        );
        assert_eq!(
            vm.resume_at(&first.continuation, 104, receipt(&first)),
            done(list(&["2", "3", "5"]))
        );
    }
}

#[test]
fn collection_reentry_preserves_cancellation_deadlines_and_remaining_fuel() {
    let first = effect(start(&mut Vm::new(10000), flow()));
    let mut exhausted = first.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut vm = Vm::new(10000);
    vm.restore_request(exhausted.clone()).unwrap();
    fault(
        vm.resume_at(&exhausted.continuation, 101, receipt(&first)),
        "LSV1001",
    );
    let mut vm = Vm::new(10000);
    vm.restore_request(first.clone()).unwrap();
    assert!(matches!(
        vm.resume_at(&first.continuation, 200, receipt(&first)),
        Step::Cancelled(_)
    ));
    let mut vm = Vm::new(10000);
    vm.restore_request(first.clone()).unwrap();
    assert!(matches!(
        vm.cancel_effect(&first.continuation, 101),
        Step::Cancelled(_)
    ));
    assert!(matches!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Cancelled(_)
    ));
}

#[test]
fn collection_operand_order_and_lazy_parent_boundaries_are_deterministic() {
    fault(
        calculate(
            r#"strings(first: to_string(value: div(left: 1, right: 0)), second: to_string(value: parse_integer(value: "secret")))"#,
            1000,
        ),
        "LSV1401",
    );
    fault(
        calculate(
            r#"join(right: to_string(value: parse_integer(value: "secret")), left: strings(first: to_string(value: div(left: 1, right: 0))))"#,
            1000,
        ),
        "LSV1401",
    );
    // The collection is evaluated before the initial state, regardless of written argument order.
    fault(
        calculate(
            r#"fold(total: parse_integer(value: "secret"), items: strings(first: to_string(value: div(left: 1, right: 0))), item: "part", next: total, limit: 1)"#,
            1000,
        ),
        "LSV1401",
    );
    assert_eq!(
        calculate(
            r#"choose(when: true, then: strings(), otherwise: strings(first: to_string(value: div(left: 1, right: 0))))"#,
            100
        ),
        done(list(&[]))
    );
    assert_eq!(
        calculate(
            r#"value_or(left: item_at(left: strings(a: ""), right: 0), right: to_string(value: div(left: 1, right: 0)))"#,
            100
        ),
        done(ScalarValue::String("".into()))
    );
    let step = calculate(
        r#"fold(total: 0, items: strings(first: "private-rejected-text"), item: "part", next: parse_integer(value: part), limit: 1)"#,
        1000,
    );
    assert!(!format!("{step:?}").contains("private-rejected-text"));
    fault(step, "LSV1408");
}
