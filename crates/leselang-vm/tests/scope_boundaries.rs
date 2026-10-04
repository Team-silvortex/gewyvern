use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::ScalarValue;
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, Step, Value, Vm, decode_continuation, encode_continuation,
};
use leserpent_domain::QueryResult;

fn start(vm: &mut Vm, body: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {body}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["runtime.read"]),
        None,
    )
}

fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

#[test]
fn nested_names_and_aliases_preserve_exact_expression_and_copy_fuel() {
    let source = "bind(base: 1, body: bind(tmp: 2, body: add(left: base, right: tmp)))";
    assert_eq!(
        start(&mut Vm::new(7), source),
        done(ScalarValue::Integer(3))
    );
    assert!(
        matches!(start(&mut Vm::new(6), source), Step::Fault(fault) if fault.code == "LSV1001")
    );
    for text in [
        String::new(),
        "x".into(),
        "x".repeat(65),
        "\u{1f600}".repeat(512),
    ] {
        let source = format!(
            "bind(prefix: {}, body: bind(alias: prefix, body: concat(left: prefix, right: alias)))",
            serde_json::to_string(&text).unwrap()
        );
        let fuel = 7 + 6 * text.len().div_ceil(64) as u64;
        assert_eq!(
            start(&mut Vm::new(fuel), &source),
            done(ScalarValue::String(text.repeat(2)))
        );
        assert!(
            matches!(start(&mut Vm::new(fuel - 1), &source), Step::Fault(fault) if fault.code == "LSV1001")
        );
    }
}

#[test]
fn failed_inner_frames_release_all_names_before_recovery_without_losing_outer_values() {
    for source in [
        "bind(base: 9, body: recover(value: bind(tmp: 4, body: bind(inner: 2, body: div(left: tmp, right: 0))), fallback: bind(tmp: 1, body: bind(inner: 2, body: add(left: base, right: add(left: tmp, right: inner))))))",
        "bind(base: 9, body: recover(value: loop(tmp: 0, while: true, next: bind(inner: 2, body: div(left: inner, right: tmp)), limit: 1), fallback: bind(tmp: 1, body: bind(inner: 2, body: add(left: base, right: add(left: tmp, right: inner))))))",
        r#"bind(base: 9, body: recover(value: fold(tmp: 0, items: strings(a: "bad"), item: "inner", next: parse_integer(value: inner), limit: 1), fallback: bind(tmp: 1, body: bind(inner: 2, body: add(left: base, right: add(left: tmp, right: inner))))))"#,
    ] {
        assert_eq!(
            start(&mut Vm::new(1000), source),
            done(ScalarValue::Integer(12)),
            "{source}"
        );
    }
}

#[test]
fn only_surviving_lexical_names_are_owned_by_the_host_suspension_image() {
    let mut vm = Vm::new(1000);
    let request: EffectRequest = {
        let source = "bind(base: 9, body: bind(recovered: recover(value: bind(tmp: 4, body: div(left: tmp, right: 0)), fallback: bind(tmp: 3, body: tmp)), body: bind(r: runtime.list(), body: add(left: base, right: recovered))))";
        let Step::Effect(request) = start(&mut vm, source) else {
            panic!("expected effect")
        };
        *request
    };
    let binding = request.continuation.result_binding.as_ref().unwrap();
    assert_eq!(binding.name, "r");
    assert_eq!(
        binding
            .locals
            .iter()
            .map(|local| local.name.as_str())
            .collect::<Vec<_>>(),
        ["base", "recovered"]
    );
    assert_eq!(
        binding
            .locals
            .iter()
            .map(|local| local.value.clone())
            .collect::<Vec<_>>(),
        [ScalarValue::Integer(9), ScalarValue::Integer(3)]
    );
    let wire = encode_continuation(&request.continuation).unwrap();
    let restored = decode_continuation(&wire).unwrap();
    assert_eq!(restored, request.continuation);
    assert_eq!(
        vm.resume(
            &restored,
            EffectResult::Query(QueryResult::RuntimeList {
                revision: Revision(7),
                runtimes: vec![],
            })
        ),
        done(ScalarValue::Integer(12))
    );
}

#[test]
fn inactive_name_reuse_is_valid_but_active_shadowing_and_cold_aliases_are_rejected() {
    assert_eq!(
        start(
            &mut Vm::new(100),
            "add(left: bind(tmp: 1, body: tmp), right: bind(tmp: 2, body: tmp))"
        ),
        done(ScalarValue::Integer(3))
    );
    for source in [
        "bind(tmp: 1, body: bind(tmp: 2, body: tmp))",
        "bind(tmp: 1, body: loop(tmp: 0, while: false, next: tmp, limit: 0))",
        r#"bind(tmp: "x", body: fold(total: 0, items: strings(), item: "tmp", next: total, limit: 0))"#,
        "choose(when: true, then: 7, otherwise: bind(tmp: 1, body: missing))",
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
}
