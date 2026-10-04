use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{LoopBudget, LoopStep, ScalarType, ScalarValue};
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

#[test]
fn shared_loop_decisions_preserve_exact_reference_fuel_thresholds() {
    assert_eq!(
        leselang_hir::computation::MAX_LOOP_ITERATIONS,
        leselang_runtime_core::MAX_LOOP_ITERATIONS
    );
    for limit in [0, 1, 2, 17, 64, 1024] {
        let body = format!(
            "loop(n: 0, while: lt(left: n, right: {limit}), next: add(left: n, right: 1), limit: {limit})"
        );
        let mut budget = LoopBudget::new(ScalarType::Integer, limit).unwrap();
        let mut current = 0;
        while budget.check_condition(current < limit).unwrap() == LoopStep::Continue {
            current += 1;
            budget.advance(&ScalarValue::Integer(current)).unwrap();
        }
        assert_eq!(budget.completed_iterations(), limit);
        let fuel = 5 + 6 * limit;
        assert_eq!(
            start(&body, fuel),
            done(ScalarValue::Integer(current)),
            "{body}"
        );
        assert!(
            matches!(start(&body, fuel - 1), Step::Fault(fault) if fault.code == "LSV1001"),
            "{body}"
        );
    }
}

#[test]
fn condition_faults_and_fuel_precede_the_loop_limit_and_limit_faults_are_not_recoverable() {
    assert_eq!(
        start(
            "loop(n: 7, while: false, next: div(left: 1, right: 0), limit: 0)",
            3
        ),
        done(ScalarValue::Integer(7))
    );
    for (body, fuel, code, message) in [
        (
            "loop(n: 0, while: true, next: div(left: 1, right: 0), limit: 0)",
            3,
            "LSV1406",
            "loop iteration limit exhausted",
        ),
        (
            "loop(n: 0, while: eq(left: div(left: 1, right: 0), right: 0), next: n, limit: 0)",
            100,
            "LSV1401",
            "integer overflow, underflow, or division by zero",
        ),
        (
            "recover(value: loop(n: 0, while: true, next: n, limit: 2), fallback: 7)",
            8,
            "LSV1406",
            "loop iteration limit exhausted",
        ),
    ] {
        let Step::Fault(fault) = start(body, fuel) else {
            panic!("expected loop fault: {body}")
        };
        assert_eq!(
            (fault.code.as_str(), fault.message.as_str()),
            (code, message)
        );
    }
    assert!(
        matches!(start("loop(n: 0, while: true, next: div(left: 1, right: 0), limit: 0)", 2), Step::Fault(fault) if fault.code == "LSV1001")
    );
    assert_eq!(
        start(
            "bind(base: 9, body: recover(value: loop(n: 0, while: true, next: div(left: 1, right: n), limit: 2), fallback: bind(n: 3, body: add(left: base, right: n))))",
            100
        ),
        done(ScalarValue::Integer(12))
    );
}

#[test]
fn short_circuit_materialization_keeps_exact_empty_and_maximum_text_costs() {
    for (value, fuel) in [
        ("".into(), 2),
        ("x".into(), 4),
        ("\u{1f600}".repeat(1024), 130),
    ] {
        let body = format!(
            "value_or(left: optional_string(value: {}), right: to_string(value: div(left: 1, right: 0)))",
            serde_json::to_string(&value).unwrap()
        );
        assert_eq!(start(&body, fuel), done(ScalarValue::String(value)));
        assert!(matches!(start(&body, fuel - 1), Step::Fault(fault) if fault.code == "LSV1001"));
    }
}

#[test]
fn left_only_selection_never_replaces_cold_path_type_and_purity_preflight() {
    for body in [
        "and(left: false, right: 1)",
        "or(left: true, right: none)",
        "value_or(left: optional_string(value: \"chosen\"), right: false)",
        "and(left: false, right: bind(r: runtime.list(), body: true))",
        "loop(n: 0, while: false, next: false, limit: 0)",
        "loop(n: 0, while: false, next: bind(r: runtime.list(), body: 0), limit: 0)",
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
}
