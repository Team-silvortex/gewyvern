use std::cell::Cell;
use std::collections::HashSet;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_declarations::*;
use leselang_hir::helper_dependencies::{
    HelperDependencyError, HelperDependencyLimits, helper_dependency_order,
};
use leselang_hir::helper_source::{HelperParameterError, helper_parameters};
use leselang_runtime_core::StructureError;
use leselang_syntax::{Expression, Function, NamedArgument, Span, SyntaxTree, parse};

const LIMITS: HelperDeclarationLimits = HelperDeclarationLimits {
    max_functions: 32,
    max_parameters: 8,
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
fn declarations(tree: &SyntaxTree) -> Vec<&Function> {
    tree.function.iter().chain(&tree.helpers).collect()
}
fn accept<'source>(
    declarations: &[&'source Function],
    entry: &str,
    limits: HelperDeclarationLimits,
) -> Result<HelperDeclarations<'source>, HelperDeclarationError<Infallible>> {
    accept_helper_declarations(declarations, entry, limits, |_| Ok(false))
}
fn call(callee: &str, children: Vec<Expression>) -> Expression {
    Expression::Call {
        callee: callee.into(),
        arguments: children
            .into_iter()
            .enumerate()
            .map(|(index, value)| NamedArgument {
                name: format!("arg{index}"),
                value,
                span: Span { start: 9, end: 13 },
            })
            .collect(),
        span: Span { start: 3, end: 7 },
    }
}
fn integer() -> Expression {
    Expression::Integer {
        value: 0,
        span: Span { start: 20, end: 21 },
    }
}

#[test]
fn custom_entry_retains_original_declarations_source_order_and_borrowed_buffers() {
    let tree = tree("fn z(n: integer) = n\nfn launch() = z(n: 1)\nfn a() = \"private literal\"");
    let accepted = {
        let input = declarations(&tree);
        accept(&input, "launch", LIMITS).unwrap()
    };
    assert!(std::ptr::eq(accepted.entry(), &tree.helpers[0]));
    assert_eq!(
        accepted
            .helpers()
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert!(std::ptr::eq(
        accepted.helpers()[0],
        tree.function.as_ref().unwrap()
    ));
    assert!(std::ptr::eq(
        &accepted.helpers()[1].body,
        &tree.helpers[1].body
    ));
    let reservation = accepted.reserved_names().find(|name| *name == "n").unwrap();
    let helper = tree.function.as_ref().unwrap();
    assert_eq!(reservation.as_ptr(), helper.parameters[0].name.as_ptr());
    let Expression::String { value, .. } = &accepted.helpers()[1].body else {
        panic!()
    };
    let Expression::String {
        value: original, ..
    } = &tree.helpers[1].body
    else {
        panic!()
    };
    assert_eq!(value.as_ptr(), original.as_ptr());
}

#[test]
fn entry_and_declaration_names_are_exact_without_implicit_main_or_case_folding() {
    let tree = tree("fn main() = 0\nfn MAIN() = 1\nfn launch() = 2");
    let input = declarations(&tree);
    for (name, index) in [("main", 0), ("MAIN", 1), ("launch", 2)] {
        assert!(std::ptr::eq(
            accept(&input, name, LIMITS).unwrap().entry(),
            input[index]
        ));
    }
    assert!(matches!(
        accept(&input, "Launch", LIMITS),
        Err(HelperDeclarationError::MissingEntry { .. })
    ));
}

#[test]
fn zero_function_parameter_node_and_depth_limits_have_explicit_meaning() {
    let zero = HelperDeclarationLimits {
        max_functions: 0,
        max_parameters: 0,
        max_source_nodes: 0,
        max_source_depth: 0,
    };
    assert!(matches!(
        accept(&[], "launch", zero),
        Err(HelperDeclarationError::MissingEntry {
            span: Span { start: 0, end: 0 }
        })
    ));
    let entry = function("fn launch() = 0");
    assert!(matches!(
        accept(&[&entry], "launch", zero),
        Err(HelperDeclarationError::FunctionLimit)
    ));
    let leaf = HelperDeclarationLimits {
        max_functions: 1,
        max_source_nodes: 1,
        ..zero
    };
    assert!(
        accept(&[&entry], "launch", leaf)
            .unwrap()
            .helpers()
            .is_empty()
    );
    assert!(matches!(
        accept(
            &[&entry],
            "launch",
            HelperDeclarationLimits {
                max_source_nodes: 0,
                ..leaf
            }
        ),
        Err(HelperDeclarationError::Source {
            error: StructureError::NodeLimit,
            ..
        })
    ));
}

#[test]
fn invalid_ceilings_count_and_entry_reject_before_any_native_policy() {
    let entry = function("fn launch() = 0");
    for limits in [
        HelperDeclarationLimits {
            max_functions: 33,
            ..LIMITS
        },
        HelperDeclarationLimits {
            max_parameters: 9,
            ..LIMITS
        },
        HelperDeclarationLimits {
            max_source_nodes: 16_385,
            ..LIMITS
        },
        HelperDeclarationLimits {
            max_source_depth: 65,
            ..LIMITS
        },
    ] {
        assert!(matches!(
            accept_helper_declarations(
                &[&entry],
                "launch",
                limits,
                |_| -> Result<bool, Infallible> { panic!("invalid limits called policy") }
            ),
            Err(HelperDeclarationError::InvalidLimits)
        ));
    }
    assert!(matches!(
        accept_helper_declarations(
            &[&entry],
            "launch",
            HelperDeclarationLimits {
                max_functions: 0,
                ..LIMITS
            },
            |_| -> Result<bool, Infallible> { panic!("invalid count called policy") }
        ),
        Err(HelperDeclarationError::FunctionLimit)
    ));
    for name in ["", "bad name", "1entry", "入口"] {
        assert!(matches!(
            accept_helper_declarations(&[&entry], name, LIMITS, |_| -> Result<bool, Infallible> {
                panic!("invalid entry called policy")
            }),
            Err(HelperDeclarationError::InvalidEntry)
        ));
    }
}

#[test]
fn exact_thirty_two_declarations_fit_and_thirty_three_reject_before_policy() {
    let mut functions = (0..31)
        .map(|index| function(&format!("fn h{index}() = 0")))
        .collect::<Vec<_>>();
    functions.push(function("fn launch() = 0"));
    let limits = HelperDeclarationLimits {
        max_source_nodes: 1,
        max_source_depth: 0,
        ..LIMITS
    };
    let calls = Cell::new(0);
    let accepted = accept_helper_declarations(
        &functions.iter().collect::<Vec<_>>(),
        "launch",
        limits,
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(false)
        },
    )
    .unwrap();
    assert_eq!(accepted.helpers().len(), 31);
    assert_eq!(calls.get(), 32);
    drop(accepted);
    functions.push(function("fn extra() = 0"));
    assert!(matches!(
        accept_helper_declarations(
            &functions.iter().collect::<Vec<_>>(),
            "launch",
            limits,
            |_| -> Result<bool, Infallible> { panic!("overflow called policy") }
        ),
        Err(HelperDeclarationError::FunctionLimit)
    ));
}

#[test]
fn invalid_function_names_reject_at_original_index_before_policy() {
    let entry = function("fn launch() = 0");
    for name in ["bad name".to_owned(), "a".repeat(65), "é".to_owned()] {
        let mut bad = function("fn work() = 0");
        bad.name = name;
        let mut calls = Vec::new();
        let error = accept_helper_declarations(&[&entry, &bad], "launch", LIMITS, |name| {
            calls.push(name.to_owned());
            Ok::<_, Infallible>(false)
        })
        .unwrap_err();
        assert!(
            matches!(error, HelperDeclarationError::InvalidName { declaration_index: 1, span } if span == bad.span)
        );
        assert_eq!(calls, ["launch"]);
    }
}

#[test]
fn reserved_policy_is_explicit_once_per_valid_header_and_precedes_duplicate_check() {
    let work = function("fn work() = 0");
    let entry = function("fn launch() = 0");
    let mut calls = Vec::new();
    let error = accept_helper_declarations(&[&entry, &work, &work], "launch", LIMITS, |name| {
        calls.push(name.to_owned());
        Ok::<_, Infallible>(name == "work" && calls.len() == 3)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        HelperDeclarationError::ReservedName {
            declaration_index: 2,
            ..
        }
    ));
    assert_eq!(calls, ["launch", "work", "work"]);
    assert!(matches!(accept(&[&work, &work, &entry], "launch", LIMITS),
        Err(HelperDeclarationError::DuplicateName { declaration_index: 1, span }) if span == work.span));
    assert!(matches!(
        accept_helper_declarations(&[&entry], "launch", LIMITS, |_| Ok::<_, Infallible>(true)),
        Err(HelperDeclarationError::ReservedName {
            declaration_index: 0,
            ..
        })
    ));
}

#[test]
fn duplicates_precede_bad_signature_and_body_without_name_normalization() {
    let original = function("fn work() = 0");
    let mut duplicate = function("fn work(n: integer) = 0");
    duplicate.parameters[0].type_name = "signed".into();
    duplicate.body = call("bad", vec![integer(); 256]);
    let entry = function("fn launch() = 0");
    assert!(
        matches!(accept(&[&original, &duplicate, &entry], "launch", LIMITS),
        Err(HelperDeclarationError::DuplicateName { declaration_index: 1, span }) if span == duplicate.span)
    );
    let upper = function("fn Work() = 0");
    assert_eq!(
        accept(&[&original, &upper, &entry], "launch", LIMITS)
            .unwrap()
            .helpers()
            .len(),
        2
    );
}

#[test]
fn all_six_scalar_tokens_and_unused_parameters_are_admitted_with_exact_bounds() {
    let helper = function(
        "fn work(i: integer, b: boolean, s: string, n: none, o: optional_string, l: string_list, x: integer, y: integer) = 0",
    );
    let entry = function("fn launch() = 0");
    assert!(accept(&[&helper, &entry], "launch", LIMITS).is_ok());
    assert!(
        matches!(accept(&[&helper, &entry], "launch", HelperDeclarationLimits { max_parameters: 7, ..LIMITS }),
        Err(HelperDeclarationError::Parameters { declaration_index: 0, span, error: HelperParameterError::ParameterLimit }) if span == helper.span)
    );
}

#[test]
fn signature_errors_keep_original_parameter_spans_and_precede_physical_body_errors() {
    let entry = function("fn launch() = 0");
    for (mutation, expected) in [
        (0, HelperParameterError::InvalidName { index: 1 }),
        (1, HelperParameterError::DuplicateName { index: 1 }),
        (2, HelperParameterError::UnknownType { index: 1 }),
    ] {
        let mut bad = function("fn work(a: integer, b: boolean) = 0");
        match mutation {
            0 => bad.parameters[1].name = "bad name".into(),
            1 => bad.parameters[1].name = "a".into(),
            _ => bad.parameters[1].type_name = "Bool".into(),
        }
        bad.body = call("native", vec![integer(); 256]);
        let error = accept(&[&entry, &bad], "launch", LIMITS).unwrap_err();
        assert!(
            matches!(error, HelperDeclarationError::Parameters { declaration_index: 1, span, error }
            if span == bad.parameters[1].span && error == expected)
        );
    }
}

#[test]
fn earlier_declaration_signature_and_source_errors_precede_later_name_errors() {
    let mut first = function("fn first(n: integer) = 0");
    let mut later = function("fn later() = 0");
    later.name = "bad name".into();
    first.parameters[0].type_name = "unknown".into();
    let calls = Cell::new(0);
    let error = accept_helper_declarations(&[&first, &later], "launch", LIMITS, |_| {
        calls.set(calls.get() + 1);
        Ok::<_, Infallible>(false)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        HelperDeclarationError::Parameters {
            declaration_index: 0,
            ..
        }
    ));
    assert_eq!(calls.get(), 1);
    first.parameters.clear();
    first.body = call("cold", vec![integer(); 256]);
    assert!(matches!(
        accept(&[&first, &later], "launch", LIMITS),
        Err(HelperDeclarationError::Source {
            declaration_index: 0,
            ..
        })
    ));
}

#[test]
fn missing_and_parameterized_entry_checks_follow_the_complete_cold_forest() {
    let helper = function("fn unused() = 0");
    assert!(
        matches!(accept(&[&helper], "launch", LIMITS), Err(HelperDeclarationError::MissingEntry { span }) if span == helper.span)
    );
    let entry = function("fn launch(n: integer) = 0");
    assert!(matches!(accept(&[&entry, &helper], "launch", LIMITS),
        Err(HelperDeclarationError::EntryParameters { declaration_index: 0, span }) if span == entry.span));
    let mut bad = helper.clone();
    bad.body = call("cold", vec![integer(); 256]);
    for input in [vec![&bad], vec![&entry, &bad]] {
        assert!(matches!(
            accept(&input, "launch", LIMITS),
            Err(HelperDeclarationError::Source { .. })
        ));
    }
}

#[test]
fn cold_frontier_is_bounded_before_name_inventory_or_child_stack_growth() {
    let mut entry = function("fn launch() = 0");
    entry.body = call(
        "native",
        (0..256)
            .map(|_| Expression::Reference {
                name: "cold private".into(),
                span: Span { start: 40, end: 44 },
            })
            .collect(),
    );
    let before = serde_json::to_vec(&entry).unwrap();
    assert!(matches!(
        accept(&[&entry], "launch", LIMITS),
        Err(HelperDeclarationError::Source {
            declaration_index: 0,
            span: Span { start: 3, end: 7 },
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!(before, serde_json::to_vec(&entry).unwrap());
}

#[test]
fn physical_depth_walk_uses_original_rightmost_child_diagnostic_order() {
    let mut entry = function("fn launch() = 0");
    let leaf = |start| Expression::Reference {
        name: "cold".into(),
        span: Span {
            start,
            end: start + 1,
        },
    };
    entry.body = call(
        "choose",
        vec![call("left", vec![leaf(50)]), call("right", vec![leaf(70)])],
    );
    let error = accept(
        &[&entry],
        "launch",
        HelperDeclarationLimits {
            max_source_depth: 1,
            ..LIMITS
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        HelperDeclarationError::Source {
            span: Span { start: 70, end: 71 },
            error: StructureError::DepthLimit,
            ..
        }
    ));
}

#[test]
fn source_node_budgets_are_per_body_not_aggregate_expansion_or_fuel() {
    let entry = function("fn launch() = 0");
    let helper = function("fn work() = 0");
    let limits = HelperDeclarationLimits {
        max_source_nodes: 1,
        max_source_depth: 0,
        ..LIMITS
    };
    assert!(accept(&[&entry, &helper], "launch", limits).is_ok());
    let mut nested = helper.clone();
    nested.body = call("native", vec![integer(), integer()]);
    assert!(
        accept(
            &[&entry, &nested],
            "launch",
            HelperDeclarationLimits {
                max_source_nodes: 3,
                max_source_depth: 1,
                ..LIMITS
            }
        )
        .is_ok()
    );
}

#[test]
fn reservation_inventory_includes_cold_fold_labels_but_not_callees_or_literals() {
    let tree = tree(
        "fn work(n: integer) = choose(when: true, then: \"_lf4\", otherwise: fold(acc: 0, items: strings(), item: \"_lf1\", next: _lf2, limit: 0))\nfn launch() = work(n: _lf3)",
    );
    let accepted = accept(&declarations(&tree), "launch", LIMITS).unwrap();
    let names = accepted.reserved_names().collect::<HashSet<_>>();
    for name in [
        "n",
        "when",
        "then",
        "otherwise",
        "acc",
        "items",
        "item",
        "next",
        "limit",
        "_lf1",
        "_lf2",
        "_lf3",
    ] {
        assert!(names.contains(name), "missing reservation {name}");
    }
    for name in ["work", "launch", "choose", "fold", "strings", "_lf4"] {
        assert!(!names.contains(name));
    }
    let Expression::Call { arguments, .. } = &tree.function.as_ref().unwrap().body else {
        panic!()
    };
    let Expression::Call { arguments, .. } = &arguments[2].value else {
        panic!()
    };
    let Expression::String { value, .. } = &arguments[2].value else {
        panic!()
    };
    assert_eq!(
        accepted
            .reserved_names()
            .find(|name| *name == "_lf1")
            .unwrap()
            .as_ptr(),
        value.as_ptr()
    );
}

struct PrivateError {
    payload: &'static str,
    drops: Rc<Cell<usize>>,
}
impl Drop for PrivateError {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn native_policy_failure_and_unwind_return_no_partial_view_or_retry() {
    let tree = tree("fn private_helper() = \"private body\"\nfn launch() = 0");
    let input = declarations(&tree);
    let before = serde_json::to_vec(&tree).unwrap();
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let error = accept_helper_declarations(&input, "launch", LIMITS, |_| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(PrivateError {
                payload: "private native payload",
                drops: drops.clone(),
            })
        } else {
            Ok(false)
        }
    })
    .unwrap_err();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        format!("{error:?} {error}"),
        "native declaration name policy failed native declaration name policy failed"
    );
    assert!(std::error::Error::source(&error).is_none());
    assert!(
        matches!(&error, HelperDeclarationError::Policy { declaration_index: 1, error, .. } if error.payload == "private native payload")
    );
    drop(error);
    assert_eq!(drops.get(), 1);
    calls.set(0);
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        accept_helper_declarations(&input, "launch", LIMITS, |_| -> Result<bool, Infallible> {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                panic!("trusted policy unwind")
            }
            Ok(false)
        })
    }));
    assert!(unwind.is_err());
    assert_eq!(calls.get(), 2);
    assert_eq!(before, serde_json::to_vec(&tree).unwrap());
    assert!(accept(&input, "launch", LIMITS).is_ok());
}

#[test]
fn debug_redacts_entry_names_reservations_literals_and_rejection_payloads() {
    let tree = tree(
        "fn private_helper(n: integer) = \"private literal\"\nfn private_entry() = private_helper(n: 0)",
    );
    let accepted = accept(&declarations(&tree), "private_entry", LIMITS).unwrap();
    let debug = format!("{accepted:?}");
    assert!(debug.contains("helpers: 1"));
    for private in ["private_helper", "private_entry", "private literal"] {
        assert!(!debug.contains(private));
    }
    let error = accept(&declarations(&tree), "missing", LIMITS).unwrap_err();
    assert_eq!(
        format!("{error:?}"),
        "designated entry declaration is missing"
    );
}

#[test]
fn admission_does_not_certify_native_calls_entry_cycles_literals_or_cold_types() {
    let entry_call = tree("fn unused() = launch()\nfn launch() = 0");
    let accepted = accept(&declarations(&entry_call), "launch", LIMITS).unwrap();
    assert!(matches!(
        helper_dependency_order(
            accepted.helpers(),
            "launch",
            HelperDependencyLimits {
                max_helpers: 31,
                max_source_nodes: 256,
                max_source_depth: 32,
            }
        ),
        Err(HelperDependencyError::EntryCall { .. })
    ));
    let mut entry = function("fn launch() = 0");
    entry.body = call(
        &"native".repeat(100),
        vec![Expression::String {
            value: "private".repeat(5000),
            span: Span { start: 8, end: 9 },
        }],
    );
    assert!(accept(&[&entry], "launch", LIMITS).is_ok());
    entry.body = call("native", vec![integer(); 72]);
    assert!(accept(&[&entry], "launch", LIMITS).is_ok());
    let cold = tree("fn unused() = choose(when: true, then: 0, otherwise: unbound)\nfn main() = 0");
    assert!(accept(&declarations(&cold), "main", LIMITS).is_ok());
    assert!(leselang_hir::lower(&cold).is_err());
}

#[test]
fn accepted_borrowed_headers_compose_with_shared_templates_hygiene_and_exact_fuel() {
    use leselang_hir::helper_bindings::{
        HelperBinding, HelperBindingLimits, bind_helper_arguments,
    };
    use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
    use leselang_hir::helper_templates::{HelperTemplate, HelperTemplateLimits};
    use leselang_hir::ir::Computation;
    use leselang_hir::pure_evaluation::*;
    use leselang_hir::pure_typing::*;
    use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
    use leselang_hir::source_call::SourceCallLimits;
    use leselang_runtime_core::{Fuel, ScalarType, ScalarValue, ScopeFrame};

    struct Native;
    type Node = Computation<Native, Native, Native, Native>;
    struct Host;
    impl PureTypeEnvironment<Native, Native> for Host {
        type Result = ();
        fn field_type(&self, _: &(), _: &Native) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &(), _: &str, _: &Native) -> Option<()> {
            None
        }
    }
    impl PureEvaluationEnvironment<Native, Native> for Host {
        type Result = ();
        type Error = Infallible;
        fn field(&self, _: &(), _: &Native) -> Result<ScalarValue, Infallible> {
            panic!("no native field")
        }
        fn member(&self, _: &(), _: &str, _: &Native) -> Result<(), Infallible> {
            panic!("no native member")
        }
    }
    let tree = tree("fn work(n: integer) = add(left: n, right: 1)\nfn launch() = work(n: 41)");
    let accepted = accept(&declarations(&tree), "launch", LIMITS).unwrap();
    let mut order = helper_dependency_order(
        accepted.helpers(),
        "launch",
        HelperDependencyLimits {
            max_helpers: 31,
            max_source_nodes: 256,
            max_source_depth: 32,
        },
    )
    .unwrap();
    let helper = order.next().unwrap().unwrap();
    assert!(std::ptr::eq(helper, tree.function.as_ref().unwrap()));
    assert!(order.next().is_none());
    let signature = helper_parameters(helper, 8).unwrap();
    let source = ScalarSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 256,
            max_source_depth: 32,
            max_lowered_nodes: 256,
            max_lowered_depth: 32,
            max_arguments: 64,
        },
        max_bindings: 8,
    };
    let make_body = || {
        lower_scalar_source_with_scope(
            &helper.body,
            source,
            &[(signature[0].name, signature[0].domain)],
            |_, _| -> Result<(Node, Option<ScalarType>), Infallible> { panic!("no native source") },
        )
        .unwrap()
        .0
    };
    let limits = HelperTemplateLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
        max_parameters: 8,
    };
    let template = HelperTemplate::new(
        vec![(signature[0].name.into(), signature[0].domain)],
        make_body(),
        Native,
        limits,
    )
    .unwrap();
    let body = template
        .materialize(limits, |_| Ok::<_, Infallible>(make_body()))
        .unwrap();
    let body = hygienic_helper_body(
        body,
        &[("n", "_p")],
        &[],
        HelperHygieneLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
            max_reserved_names: 0,
        },
        || -> Result<String, Infallible> { panic!("no local declarations") },
    )
    .unwrap();
    let literal = || Node::Literal {
        value: ScalarValue::Integer(41),
    };
    let renamed = bind_helper_arguments(
        body,
        vec![HelperBinding {
            name: "_p".into(),
            value: literal(),
        }],
        HelperBindingLimits {
            max_nodes: 256,
            max_depth: 32,
            max_parameters: 8,
        },
    )
    .unwrap();
    let original = Node::Bind {
        name: "n".into(),
        value: Box::new(literal()),
        body: Box::new(make_body()),
    };
    assert_eq!(
        infer_pure_type(
            &renamed,
            &[],
            &Host,
            TypeInferenceLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8
            }
        ),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let mut before = Fuel::new(100);
    let mut after = Fuel::new(100);
    for (node, fuel) in [(&original, &mut before), (&renamed, &mut after)] {
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let value = evaluate_pure_in_scope(
            node,
            &mut scope,
            &Host,
            fuel,
            PureEvaluationLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8,
            },
        )
        .unwrap();
        assert!(matches!(value, PureValue::Scalar(ScalarValue::Integer(42))));
        assert!(scope.is_empty());
    }
    assert_eq!(before.remaining(), after.remaining());
}

#[test]
fn reference_declaration_diagnostics_wire_names_and_cold_authority_are_unchanged() {
    let source = "fn work(a: string, b: string) = ui.focus(node_id: concat(left: a, right: b))\nfn main() = work(b: \"b\", a: \"a\")";
    let program = leselang_hir::lower(&tree(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let expected = leselang_syntax::format(&tree("fn main() = bind(_lf0: \"a\", body: bind(_lf1: \"b\", body: ui.focus(node_id: concat(left: _lf0, right: _lf1))))")).unwrap();
    assert_eq!(canonical, expected);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&tree(&canonical)).unwrap()).unwrap()
    );
    for (source, message) in [
        (
            "fn work(n: signed) = n\nfn main() = 0",
            "expected integer, boolean, string, none, optional_string or string_list parameter type",
        ),
        (
            "fn work() = 0\nfn other() = 0",
            "a multi-function program requires exactly one main",
        ),
        (
            "fn work() = 0\nfn main(n: integer) = n",
            "main cannot have parameters",
        ),
    ] {
        let errors = leselang_hir::lower(&tree(source)).unwrap_err();
        assert_eq!(errors[0].code, "LSH1501");
        assert_eq!(errors[0].message, message);
        assert!(errors[0].span.is_some());
    }
    let mut tree = tree("fn first() = 0\nfn second() = 0\nfn main() = 0");
    tree.helpers[1].name = "first".into();
    let errors = leselang_hir::lower(&tree).unwrap_err();
    assert_eq!(
        errors[0].message,
        "function names must be bounded, unique and not builtins"
    );
    assert_eq!(errors[0].span, Some(tree.helpers[1].span));
}
