//! Product boundary parity against recorded pre-extraction observations.
use super::*;

#[test]
fn public_loop_stages_match_frozen_product_diagnostics_counters_and_scope_for_1152_cases() {
    let forms = [
        "loop(state: 0, while: lt(left: state, right: 2), next: add(left: state, right: 1), limit: 2)",
        "loop(next: state, limit: 1, while: false, state: 0)",
        "loop(state: 0, while: false, next: missing, limit: 0)",
        "loop(state: 0, while: 1, next: state, limit: 0)",
        "loop(state: none, while: false, next: none, limit: 0)",
        "loop(state: \"old\", while: false, next: \"new\", limit: 1)",
        "loop(state: true, while: state, next: false, limit: 1)",
        "loop(state: strings(a: \"a\"), while: false, next: state, limit: 1)",
        "loop(state: optional_string(value: none), while: false, next: state, limit: 1)",
        "loop(state: 0, while: false, next: bind(state: 1, body: state), limit: 1)",
        "loop(state: 0, while: false, next: loop(k: state, while: false, next: k, limit: 1), limit: 1)",
        "loop(state: 0, while: false, next: loop(state: 0, while: false, next: state, limit: 1), limit: 1)",
        "loop(while: false, next: 0, limit: 1)",
        "loop(state: 0, next: state, limit: 1)",
        "loop(state: 0, while: false, while: true, next: state, limit: 1)",
        "loop(state: 0, while: false, next: state, limit: missing)",
        "loop(state: 0, while: false, next: state, limit: 1025)",
        "loop(state: 0, while: false, next: state, limit: true)",
        "loop(state: missing, while: false, next: state, limit: 1)",
        "loop(state: 0, while: missing, next: state, limit: 1)",
        "loop(state: refresh(), while: false, next: state, limit: 1)",
        "loop(state: 0, while: false, next: true, limit: 1)",
        "loop(state: 0, while: refresh(), next: state, limit: 1)",
        "loop(state: state, while: false, next: state, limit: 1)",
    ];
    let mut baseline = migration_baseline::Baseline::load(
        "loop",
        include_str!("../tests/fixtures/migration-boundaries/loop.json"),
        &forms,
        1152,
    );
    let mut cases = 0;
    for (form, text) in forms.into_iter().enumerate() {
        let parsed = parse(&format!("fn main() = {text}"));
        assert!(
            parsed.diagnostics.is_empty(),
            "{text}: {:?}",
            parsed.diagnostics
        );
        let Expression::Call {
            arguments, span, ..
        } = parsed.function.unwrap().body
        else {
            panic!()
        };
        for prefix in 0..4 {
            for starting in [
                0,
                MAX_COMPUTATION_NODES - 2,
                MAX_COMPUTATION_NODES,
                MAX_COMPUTATION_NODES + 1,
            ] {
                for depth in [0, MAX_EFFECT_NESTING_DEPTH - 1, MAX_EFFECT_NESTING_DEPTH] {
                    let run = || {
                        let mut bindings = match prefix {
                            0 => vec![],
                            1 => vec![("state", Type::Scalar(ScalarType::Integer).into())],
                            2 => vec![("prefix", Type::Scalar(ScalarType::Boolean).into())],
                            _ => vec![("reply", Type::Structured.into())],
                        };
                        let mut scope = ScopeFrame::new(&mut bindings);
                        let mut visited = starting;
                        let mut functions = functions::FunctionTemplates::default();
                        let result = lower_loop(
                            &arguments,
                            span,
                            &mut scope,
                            &mut visited,
                            depth,
                            &mut functions,
                        );
                        let remaining = scope
                            .bindings()
                            .iter()
                            .map(|(name, local)| (*name, local.ty))
                            .collect::<Vec<_>>();
                        (result, visited, remaining)
                    };
                    baseline.assert_case(form, prefix, starting, depth, &run());
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 1152);
    baseline.finish();
}
