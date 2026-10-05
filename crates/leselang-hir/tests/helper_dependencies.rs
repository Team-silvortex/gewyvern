use std::collections::{BTreeMap, BTreeSet};
use std::iter::FusedIterator;

use leselang_hir::helper_dependencies::*;
use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::{HelperParameterError, helper_parameters};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{Fuel, ScalarType, ScalarValue, ScopeFrame, StructureError};
use leselang_syntax::{Expression, Function, NamedArgument, Span, SyntaxTree, parse};

const LIMITS: HelperDependencyLimits = HelperDependencyLimits {
    max_helpers: 31,
    max_source_nodes: 256,
    max_source_depth: 32,
};
fn tree(source: &str) -> SyntaxTree {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree
}
fn function(source: &str) -> Function {
    tree(source).function.unwrap()
}
fn helpers<'a>(tree: &'a SyntaxTree, entry: &str) -> Vec<&'a Function> {
    tree.function
        .iter()
        .chain(&tree.helpers)
        .filter(|function| function.name != entry)
        .collect()
}
fn names<'a>(order: HelperDependencyOrder<'a>) -> Result<Vec<&'a str>, HelperDependencyError> {
    order
        .map(|function| function.map(|function| function.name.as_str()))
        .collect()
}
fn span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::Integer { span, .. }
        | Expression::String { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::None { span }
        | Expression::Reference { span, .. } => *span,
    }
}
fn call(callee: &str, children: Vec<Expression>) -> Expression {
    Expression::Call {
        callee: callee.into(),
        span: Span { start: 7, end: 11 },
        arguments: children
            .into_iter()
            .enumerate()
            .map(|(index, value)| NamedArgument {
                name: format!("arg{index}"),
                value,
                span: Span { start: 7, end: 11 },
            })
            .collect(),
    }
}

// Specification of the old ready-name policy, not another lowerer/interpreter.
fn reference_order<'a>(functions: &[&'a Function]) -> (Vec<&'a str>, usize) {
    let functions = functions
        .iter()
        .map(|function| (function.name.as_str(), *function))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies = BTreeMap::new();
    for (name, function) in &functions {
        let mut required = BTreeSet::new();
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            if let Expression::Call {
                callee, arguments, ..
            } = expression
            {
                if let Some((name, _)) = functions.get_key_value(callee.as_str()) {
                    required.insert(*name);
                }
                pending.extend(arguments.iter().map(|argument| &argument.value));
            }
        }
        dependencies.insert(*name, required);
    }
    let mut planned = BTreeSet::new();
    let mut output = Vec::new();
    while planned.len() < functions.len() {
        let Some(name) = dependencies.iter().find_map(|(name, required)| {
            (!planned.contains(name) && required.iter().all(|name| planned.contains(name)))
                .then_some(*name)
        }) else {
            break;
        };
        planned.insert(name);
        output.push(name);
    }
    let remaining = functions.len() - planned.len();
    (output, remaining)
}

#[test]
fn empty_graph_and_zero_policy_are_explicit_and_fused() {
    let limits = HelperDependencyLimits {
        max_helpers: 0,
        max_source_nodes: 0,
        max_source_depth: 0,
    };
    let mut order = helper_dependency_order(&[], "launch", limits).unwrap();
    fn fused(_: &impl FusedIterator) {}
    fused(&order);
    assert_eq!(order.size_hint(), (0, Some(0)));
    assert!(order.next().is_none());
    assert!(order.next().is_none());
}

#[test]
fn independent_helpers_use_exact_lexical_order_and_original_ast_borrows() {
    let functions = [
        function("fn z() = 0"),
        function("fn a() = 1"),
        function("fn A() = 2"),
    ];
    let mut order = {
        let temporary = functions.iter().collect::<Vec<_>>();
        helper_dependency_order(&temporary, "launch", LIMITS).unwrap()
    };
    for original in [&functions[2], &functions[1], &functions[0]] {
        let next = order.next().unwrap().unwrap();
        assert!(std::ptr::eq(next, original));
        assert!(std::ptr::eq(&next.body, &original.body));
        assert_eq!(next.name.as_ptr(), original.name.as_ptr());
    }
    assert!(order.next().is_none());
}

#[test]
fn each_newly_ready_name_is_selected_before_the_next_lexical_sibling() {
    let tree = tree("fn z() = 0\nfn a() = b()\nfn b() = 1\nfn launch() = a()");
    let functions = helpers(&tree, "launch");
    assert_eq!(
        names(helper_dependency_order(&functions, "launch", LIMITS).unwrap()).unwrap(),
        ["b", "a", "z"]
    );
}

#[test]
fn all_input_permutations_preserve_ready_order_with_chains_and_shared_dependencies() {
    fn check(functions: &mut [&Function], start: usize) -> usize {
        if start == functions.len() {
            let expected = reference_order(functions);
            assert_eq!(expected.1, 0);
            assert_eq!(
                names(helper_dependency_order(functions, "launch", LIMITS).unwrap()).unwrap(),
                expected.0
            );
            return 1;
        }
        let mut checked = 0;
        for index in start..functions.len() {
            functions.swap(start, index);
            checked += check(functions, start + 1);
            functions.swap(start, index);
        }
        checked
    }
    let tree = tree(
        "fn a() = c()\nfn b() = c()\nfn c() = native.wrap(value: d())\nfn d() = 1\nfn z() = 2\nfn launch() = a()",
    );
    assert_eq!(check(&mut helpers(&tree, "launch"), 0), 120);
}

#[test]
fn dense_maximum_graph_uses_safe_bits_and_matches_the_old_order() {
    let functions = (0..MAX_DEPENDENCY_HELPERS)
        .map(|index| {
            let mut function = function(&format!("fn f{index:02}() = 0"));
            function.body = call(
                "native.wrap",
                (0..index)
                    .map(|dependency| call(&format!("f{dependency:02}"), vec![]))
                    .collect(),
            );
            function
        })
        .collect::<Vec<_>>();
    let mut refs = functions.iter().rev().collect::<Vec<_>>();
    let expected = reference_order(&refs);
    let mut order = helper_dependency_order(&refs, "launch", LIMITS).unwrap();
    assert_eq!(order.size_hint(), (1, Some(31)));
    let mut output = Vec::new();
    while let Some(next) = order.next() {
        output.push(next.unwrap().name.as_str());
        assert!(order.size_hint().1.unwrap() <= 31 - output.len());
    }
    assert_eq!(output, expected.0);
    refs.rotate_left(9);
    assert_eq!(
        names(helper_dependency_order(&refs, "launch", LIMITS).unwrap()).unwrap(),
        output
    );
    assert_eq!(order.size_hint(), (0, Some(0)));
}

#[test]
fn direct_mutual_and_cold_unused_cycles_are_not_reachability_pruned() {
    for body in [
        "work()",
        "choose(when: true, then: 0, otherwise: work())",
        "recover(value: 0, fallback: work())",
        "loop(acc: 0, while: false, next: work(), limit: 0)",
        "fold(acc: 0, items: strings(), item: \"entry\", next: work(), limit: 0)",
        "native.dialog(value: work())",
    ] {
        let tree = tree(&format!("fn work() = {body}\nfn launch() = 0"));
        let functions = helpers(&tree, "launch");
        let mut order = helper_dependency_order(&functions, "launch", LIMITS).unwrap();
        assert_eq!(
            order.next(),
            Some(Err(HelperDependencyError::Cycle { remaining: 1 }))
        );
        assert!(order.next().is_none());
        assert!(order.next().is_none());
    }
    let tree = tree("fn a() = b()\nfn b() = a()\nfn launch() = 0");
    assert_eq!(
        names(helper_dependency_order(&helpers(&tree, "launch"), "launch", LIMITS).unwrap()),
        Err(HelperDependencyError::Cycle { remaining: 2 })
    );
}

#[test]
fn ready_helpers_precede_a_single_terminal_cycle_error_including_blocked_dependents() {
    let tree =
        tree("fn good() = 0\nfn a() = b()\nfn b() = a()\nfn dependent() = a()\nfn launch() = 0");
    let functions = helpers(&tree, "launch");
    let mut order = helper_dependency_order(&functions, "launch", LIMITS).unwrap();
    assert_eq!(order.next().unwrap().unwrap().name, "good");
    assert_eq!(order.size_hint(), (1, Some(3)));
    assert_eq!(
        order.next(),
        Some(Err(HelperDependencyError::Cycle { remaining: 3 }))
    );
    assert_eq!(order.size_hint(), (0, Some(0)));
    assert!(order.next().is_none());
    assert_eq!(reference_order(&functions), (vec!["good"], 3));
}

#[test]
fn references_argument_labels_and_literals_do_not_create_call_edges() {
    let first =
        tree("fn a() = b()\nfn b(a: integer) = bind(a_value: a, body: \"a\")\nfn launch() = 0");
    assert_eq!(
        names(helper_dependency_order(&helpers(&first, "launch"), "launch", LIMITS).unwrap())
            .unwrap(),
        ["b", "a"]
    );
    let tree =
        tree("fn a() = native.dialog(value: b())\nfn b() = \"native.dialog\"\nfn launch() = 0");
    assert_eq!(
        names(helper_dependency_order(&helpers(&tree, "launch"), "launch", LIMITS).unwrap())
            .unwrap(),
        ["b", "a"]
    );
}

#[test]
fn repeated_edges_do_not_consume_extra_graph_slots_but_physical_nodes_still_count() {
    let tree = tree(
        "fn a() = native.wrap(first: b(), second: b(), third: b())\nfn b() = 0\nfn launch() = 0",
    );
    let functions = helpers(&tree, "launch");
    let mut limits = LIMITS;
    limits.max_source_nodes = 4;
    assert_eq!(
        names(helper_dependency_order(&functions, "launch", limits).unwrap()).unwrap(),
        ["b", "a"]
    );
    limits.max_source_nodes = 3;
    assert_eq!(
        helper_dependency_order(&functions, "launch", limits).unwrap_err(),
        HelperDependencyError::Source {
            declaration_index: 0,
            span: span(&functions[0].body),
            error: StructureError::NodeLimit,
        }
    );
}

#[test]
fn explicit_entry_is_not_hardcoded_and_entry_calls_are_rejected_before_order_iteration() {
    for body in [
        "launch()",
        "choose(when: true, then: 0, otherwise: launch())",
        "native.form(value: launch())",
    ] {
        let source = format!("fn work() = {body}\nfn launch() = 0");
        let tree = tree(&source);
        let functions = helpers(&tree, "launch");
        let start = source.find("launch()").unwrap();
        assert_eq!(
            helper_dependency_order(&functions, "launch", LIMITS).unwrap_err(),
            HelperDependencyError::EntryCall {
                declaration_index: 0,
                span: Span {
                    start,
                    end: start + "launch()".len()
                },
            }
        );
    }
    let tree = tree("fn work() = main()\nfn launch() = 0");
    assert_eq!(
        names(helper_dependency_order(&helpers(&tree, "launch"), "launch", LIMITS).unwrap())
            .unwrap(),
        ["work"]
    );
}

#[test]
fn entry_call_priority_is_lexical_helper_then_rightmost_physical_child() {
    let source = "fn z() = launch()\nfn a() = native.wrap(first: launch(), second: native.wrap(value: launch()))\nfn launch() = 0";
    let tree = tree(source);
    let functions = helpers(&tree, "launch");
    let start = source.find("value: launch()").unwrap() + "value: ".len();
    assert_eq!(
        helper_dependency_order(&functions, "launch", LIMITS).unwrap_err(),
        HelperDependencyError::EntryCall {
            declaration_index: 1,
            span: Span {
                start,
                end: start + "launch()".len()
            },
        }
    );
}

#[test]
fn all_physical_preflight_precedes_entry_call_scanning_without_mutating_source() {
    let functions = [
        function("fn a() = launch()"),
        function("fn z() = native.wrap(value: 0)"),
    ];
    let mut limits = LIMITS;
    limits.max_source_nodes = 1;
    let refs = functions.iter().collect::<Vec<_>>();
    let before = serde_json::to_vec(&functions).unwrap();
    assert_eq!(
        helper_dependency_order(&refs, "launch", limits).unwrap_err(),
        HelperDependencyError::Source {
            declaration_index: 1,
            span: span(&functions[1].body),
            error: StructureError::NodeLimit,
        }
    );
    assert_eq!(serde_json::to_vec(&functions).unwrap(), before);
}

#[test]
fn zero_leaf_and_depth_bounds_are_per_helper_and_do_not_grant_expansion_budget() {
    let functions = [function("fn a() = 0"), function("fn b() = 1")];
    let refs = functions.iter().collect::<Vec<_>>();
    let mut limits = HelperDependencyLimits {
        max_helpers: 2,
        max_source_nodes: 1,
        max_source_depth: 0,
    };
    assert_eq!(
        names(helper_dependency_order(&refs, "launch", limits).unwrap()).unwrap(),
        ["a", "b"]
    );
    limits.max_source_nodes = 0;
    assert!(matches!(
        helper_dependency_order(&refs, "launch", limits),
        Err(HelperDependencyError::Source {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    limits.max_source_nodes = 8;
    let nested = function("fn c() = native.wrap(value: 0)");
    let Expression::Call { arguments, .. } = &nested.body else {
        panic!()
    };
    assert_eq!(
        helper_dependency_order(&[&nested], "launch", limits).unwrap_err(),
        HelperDependencyError::Source {
            declaration_index: 0,
            span: span(&arguments[0].value),
            error: StructureError::DepthLimit,
        }
    );
}

#[test]
fn excessive_frontiers_counts_and_safety_ceilings_fail_before_graph_allocation() {
    let mut wide = function("fn wide() = 0");
    wide.body = call("native.wrap", vec![wide.body.clone(); 257]);
    assert!(matches!(
        helper_dependency_order(&[&wide], "launch", LIMITS),
        Err(HelperDependencyError::Source {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    let leaf = function("fn leaf() = 0");
    assert_eq!(
        helper_dependency_order(&vec![&leaf; 32], "launch", LIMITS).unwrap_err(),
        HelperDependencyError::HelperLimit
    );
    for dimension in 0..3 {
        let mut limits = LIMITS;
        match dimension {
            0 => limits.max_helpers = 32,
            1 => limits.max_source_nodes = 16385,
            _ => limits.max_source_depth = 65,
        }
        assert_eq!(
            helper_dependency_order(&[], "launch", limits).unwrap_err(),
            HelperDependencyError::InvalidLimits
        );
    }
}

#[test]
fn invalid_duplicate_and_entry_names_report_original_submission_indices_and_spans() {
    let mut invalid = function("fn z() = 0");
    invalid.name = "bad-key".into();
    let a = function("fn a() = 0");
    assert_eq!(
        helper_dependency_order(&[&a, &invalid], "launch", LIMITS).unwrap_err(),
        HelperDependencyError::InvalidName {
            declaration_index: 1,
            span: invalid.span
        }
    );
    let duplicate = function("fn a() = 1");
    assert_eq!(
        helper_dependency_order(&[&a, &duplicate], "launch", LIMITS).unwrap_err(),
        HelperDependencyError::DuplicateName {
            declaration_index: 1,
            span: duplicate.span
        }
    );
    let entry = function("fn launch() = 0");
    assert_eq!(
        helper_dependency_order(&[&a, &entry], "launch", LIMITS).unwrap_err(),
        HelperDependencyError::EntryConflict {
            declaration_index: 1,
            span: entry.span
        }
    );
    for entry in ["", "body", "none", "bad-key"] {
        assert_eq!(
            helper_dependency_order(&[], entry, LIMITS).unwrap_err(),
            HelperDependencyError::InvalidEntry
        );
    }
}

#[test]
fn debug_and_error_formats_never_dump_source_names_bodies_or_parameter_payloads() {
    let function = function("fn secret_function(secret_parameter: integer) = \"secret_literal\"");
    let mut order = helper_dependency_order(&[&function], "private_entry", LIMITS).unwrap();
    for rendered in [
        format!("{order:?}"),
        format!(
            "{} {:?}",
            HelperDependencyError::Cycle { remaining: 1 },
            HelperDependencyError::Cycle { remaining: 1 }
        ),
    ] {
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("private_entry"));
    }
    assert!(std::ptr::eq(order.next().unwrap().unwrap(), &function));
}

#[test]
fn dependency_planning_is_not_signature_operand_or_native_operation_validation() {
    let function = function("fn add(n: native_receipt) = missing.operation(value: \"bad\")");
    assert_eq!(
        names(helper_dependency_order(&[&function], "launch", LIMITS).unwrap()).unwrap(),
        ["add"]
    );
    assert_eq!(
        helper_parameters(&function, 8).unwrap_err(),
        HelperParameterError::UnknownType { index: 0 }
    );
    let mut function = function.clone();
    function.body = Expression::String {
        value: "x".repeat(4097),
        span: function.span,
    };
    assert_eq!(
        names(helper_dependency_order(&[&function], "launch", LIMITS).unwrap()).unwrap(),
        ["add"]
    );
    let errors = leselang_hir::lower(&tree("fn add() = 0\nfn main() = 0")).unwrap_err();
    assert_eq!(errors[0].code, "LSH1501");
}

struct PureHost;
impl PureTypeEnvironment<(), ()> for PureHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<(), ()> for PureHost {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &()) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn parsed_planned_declaration_composes_with_signatures_hygiene_cold_inference_and_interpretation() {
    type Node = Computation<(), (), (), ()>;
    let function = function(
        "fn work(n: integer) = loop(acc: n, while: lt(left: acc, right: 4), next: add(left: acc, right: 1), limit: 3)",
    );
    let mut order = helper_dependency_order(&[&function], "launch", LIMITS).unwrap();
    let selected = order.next().unwrap().unwrap();
    let parameters = helper_parameters(selected, 8).unwrap();
    assert_eq!(
        parameters[0].name.as_ptr(),
        selected.parameters[0].name.as_ptr()
    );
    let source = || {
        lower_scalar_source_with_scope(
            &selected.body,
            ScalarSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 256,
                    max_source_depth: 32,
                    max_lowered_nodes: 256,
                    max_lowered_depth: 32,
                    max_arguments: 64,
                },
                max_bindings: 8,
            },
            &[(parameters[0].name, parameters[0].domain)],
            |_, _| -> Result<(Node, Option<ScalarType>), ()> { panic!("no native lowering") },
        )
        .unwrap()
        .0
    };
    let original = Node::Bind {
        name: "n".into(),
        value: Box::new(Node::Literal {
            value: ScalarValue::Integer(1),
        }),
        body: Box::new(source()),
    };
    let body = hygienic_helper_body(
        source(),
        &[("n", "_parameter")],
        &["launch"],
        HelperHygieneLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
            max_reserved_names: 8,
        },
        || Ok::<_, ()>("_state".into()),
    )
    .unwrap();
    let renamed = Node::Bind {
        name: "_parameter".into(),
        value: Box::new(Node::Literal {
            value: ScalarValue::Integer(1),
        }),
        body: Box::new(body),
    };
    let types = TypeInferenceLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    };
    assert_eq!(
        infer_pure_type(&renamed, &[], &PureHost, types),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let mut before = Fuel::new(1000);
    let mut after = Fuel::new(1000);
    for (node, fuel) in [(&original, &mut before), (&renamed, &mut after)] {
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let result = evaluate_pure_in_scope(
            node,
            &mut scope,
            &PureHost,
            fuel,
            PureEvaluationLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8,
            },
        )
        .unwrap();
        assert!(matches!(result, PureValue::Scalar(ScalarValue::Integer(4))));
        assert!(scope.is_empty());
    }
    assert_eq!(before.remaining(), after.remaining());
    assert!(order.next().is_none());
}

#[test]
fn reference_ready_helper_lowering_error_keeps_precedence_over_later_cycle_detection() {
    for name in ["a", "z"] {
        let good_shape = format!("fn {name}() = add(left: true, right: 1)\nfn main() = 0");
        let expected = leselang_hir::lower(&tree(&good_shape)).unwrap_err()[0].clone();
        let source =
            format!("fn {name}() = add(left: true, right: 1)\nfn cycle() = cycle()\nfn main() = 0");
        let error = leselang_hir::lower(&tree(&source)).unwrap_err()[0].clone();
        assert_eq!(error, expected);
        assert_ne!(error.code, "LSH1502");
    }
}

#[test]
fn reference_entry_rejection_and_lexical_expansion_preserve_diagnostics_wire_and_authority() {
    let source = "fn z() = main()\nfn a() = choose(when: true, then: main(), otherwise: main())\nfn main() = 0";
    let error = leselang_hir::lower(&tree(source)).unwrap_err().remove(0);
    let start = source.find("otherwise: main()").unwrap() + "otherwise: ".len();
    assert_eq!(error.code, "LSH1502");
    assert_eq!(error.message, "helpers cannot call main");
    assert_eq!(
        error.span,
        Some(Span {
            start,
            end: start + "main()".len()
        })
    );
    let source = "fn a(n: integer) = b(n: n)\nfn b(n: integer) = n\nfn z(n: integer) = n\nfn main() = ui.focus(node_id: to_string(value: a(n: 7)))";
    let program = leselang_hir::lower(&tree(source)).unwrap();
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let expected = leselang_syntax::format(&tree("fn main() = ui.focus(node_id: to_string(value: bind(_lf1: 7, body: bind(_lf2: _lf1, body: _lf2))))")).unwrap();
    assert_eq!(canonical, expected);
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&tree(&canonical)).unwrap()).unwrap()
    );
}
