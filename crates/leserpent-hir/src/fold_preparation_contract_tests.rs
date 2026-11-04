//! Product boundary parity against recorded pre-extraction observations.
use super::*;

#[test]
fn public_fold_stages_match_frozen_product_diagnostics_counters_and_scope_for_1920_cases() {
    let forms = [
        "fold(total: 0, items: strings(a: \"a\"), item: \"entry\", next: add(left: total, right: 1), limit: 1)",
        "fold(next: total, limit: 0, total: 0, item: \"entry\", items: strings())",
        "fold(total: 0, items: strings(), item: \"entry\", next: missing, limit: 0)",
        "fold(total: 0, items: strings(), item: \"entry\", next: true, limit: 0)",
        "fold(total: none, items: strings(), item: \"entry\", next: none, limit: 0)",
        "fold(total: \"\", items: strings(a: \"a\"), item: \"entry\", next: entry, limit: 1)",
        "fold(total: true, items: strings(), item: \"entry\", next: false, limit: 1)",
        "fold(total: strings(a: \"a\"), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: optional_string(value: none), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(items: strings(), item: \"entry\", next: 0, limit: 0)",
        "fold(total: 0, item: \"entry\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), items: strings(), item: \"entry\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: 1, next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"total\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"bad name\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: missing)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: 65)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: true)",
        "fold(total: missing, items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: 1, item: \"entry\", next: total, limit: 1)",
        "fold(total: refresh(), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: refresh(), limit: 1)",
        "fold(total: 0, items: entry, item: \"entry\", next: total, limit: 1)",
        "fold(total: total, items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: bind(entry: 1, body: entry), limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: fold(total: 0, items: strings(), item: \"other\", next: total, limit: 1), limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: fold(n: total, items: strings(), item: \"other\", next: n, limit: 1), limit: 1)",
        "fold(total: 0, items: missing, item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: refresh(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: 64)",
        "fold(total: 0, items: strings(), item: \"entry\", next: snapshot, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: loop(n: total, while: false, next: n, limit: 0), limit: 1)",
    ];
    let mut baseline = migration_baseline::Baseline::load(
        "fold",
        include_str!("../tests/fixtures/migration-boundaries/fold.json"),
        &forms,
        1920,
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
        for prefix in 0..5 {
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
                            1 => vec![("total", Type::Scalar(ScalarType::Integer).into())],
                            2 => vec![("entry", Type::Scalar(ScalarType::String).into())],
                            3 => vec![("prefix", Type::Scalar(ScalarType::Boolean).into())],
                            _ => vec![("snapshot", Type::Structured.into())],
                        };
                        let mut scope = ScopeFrame::new(&mut bindings);
                        let mut visited = starting;
                        let mut functions = functions::FunctionTemplates::default();
                        let result = lower_fold(
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
    assert_eq!(cases, 1920);
    baseline.finish();
}
