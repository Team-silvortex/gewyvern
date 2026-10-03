use leselang_hir::computation::{Computation, GroupKind, ScalarType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

fn compile(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    let canonical = canonical_source(&program.function.effect).unwrap();
    assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    assert_eq!(
        lower(&parse(&format(&parse(source)).unwrap())).unwrap(),
        program
    );
    program
}

#[test]
fn prepared_atomic_helpers_compose_with_groups_and_flattened_repetition() {
    let program = compile(
        r#"fn focus(node: string, alternate: boolean) = bind(label: concat(left: "node-", right: node), body:
        choose(when: alternate, then: ui.focus(node_id: "alternate"), otherwise: ui.focus(node_id: label)))
        fn wrapped(node: string) = focus(node: node, alternate: false)
        fn main() = seq(first: wrapped(node: "a"), again: repeat(times: 2, body: seq(focus: focus(node: "b", alternate: true), visible: ui.assert_visible(node_id: "alternate"))))"#,
    );
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    let Computation::Group {
        group_kind,
        branches,
    } = expression.as_ref()
    else {
        panic!()
    };
    assert_eq!(*group_kind, GroupKind::Sequence);
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>(),
        [
            "first",
            "again__iteration_1__focus",
            "again__iteration_1__visible",
            "again__iteration_2__focus",
            "again__iteration_2__visible"
        ]
    );
    assert_eq!(
        branches[0].value.prepared_atomic_operation(),
        Some(HostOperation::UiFocus)
    );
    assert_eq!(
        branches[2].value.prepared_atomic_operation(),
        Some(HostOperation::UiAssertVisible)
    );
    assert_eq!(program.function.result_type, Type::Structured);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn expanded_pure_preparation_and_selection_are_a_canonical_group_surface() {
    for expression in [
        r#"seq(a: bind(node: "a", body: ui.focus(node_id: node)))"#,
        r#"all(a: bind(node: "a", body: ui.focus(node_id: node)), b: bind(node: "b", body: ui.focus(node_id: node)))"#,
        r#"repeat(times: 2, body: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")))"#,
        r#"seq(a: bind(node: recover(value: to_string(value: div(left: 1, right: 0)), fallback: "a"), body: ui.focus(node_id: node)))"#,
    ] {
        compile(&format!("fn main() = {expression}"));
    }
    for count in [1, 64] {
        compile(&format!(
            r#"fn focus() = ui.focus(node_id: "a")
            fn main() = repeat(times: {count}, body: focus())"#
        ));
    }
}

#[test]
fn helper_group_members_keep_static_named_projection_signatures() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn check(node: string) = ui.assert_text(node_id: node, expected: concat(left: "re", right: "ady"))
            fn main() = bind(group: {kind}(first: check(node: "a"), second: check(node: "b")), body:
                eq(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))"#
        );
        let program = compile(&source);
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Boolean)
        );
    }
    compile(
        r#"fn focus(node: string) = ui.focus(node_id: node)
        fn main() = bind(group: repeat(times: 2, body: focus(node: "a")), body: field(value: member(value: group, name: "iteration_2"), name: "node_id"))"#,
    );
}

#[test]
fn helper_preparation_scopes_do_not_leak_between_group_members() {
    compile(
        r#"fn focus(node: string) = bind(label: concat(left: node, right: "-inner"), body: ui.focus(node_id: label))
        fn main() = bind(_lf0: "outer", body: bind(label: "outer", body: seq(first: focus(node: label), second: focus(node: label))))"#,
    );
    assert!(lower(&parse(r#"fn main() = seq(first: bind(node: "a", body: ui.focus(node_id: node)), second: ui.focus(node_id: node))"#)).is_err());
}

#[test]
fn cold_and_unused_helpers_retain_type_capability_and_host_domain_preflight() {
    let mut program = compile(
        r#"fn unused() = runtime.refresh(runtime_id: "a")
        fn inspect(alternate: boolean) = choose(when: alternate, then: runtime.inspect(runtime_id: "b"), otherwise: runtime.inspect(runtime_id: "a"))
        fn main() = seq(focus: ui.focus(node_id: "a"), read: inspect(alternate: false))"#,
    );
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["ui.presentation", "runtime.read"]),
    )
    .unwrap();
    program.function.required_capabilities.pop();
    assert!(
        authorize(
            &program,
            &CapabilitySet::new(["ui.presentation", "runtime.read"])
        )
        .is_err()
    );
    for source in [
        r#"fn work() = choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "bad node"))
            fn main() = seq(first: work())"#,
        r#"fn work() = choose(when: true, then: ui.focus(node_id: "a"), otherwise: runtime.list())
            fn main() = seq(first: work())"#,
    ] {
        assert!(lower(&parse(source)).is_err());
    }
}

#[test]
fn group_members_never_capture_results_return_data_or_hide_multiple_effects() {
    for body in [
        r#"bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id"))"#,
        r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        r#"all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: bind(r: ui.focus(node_id: "b"), body: ui.focus(node_id: "c")))"#,
        r#""a""#,
    ] {
        for call in [
            "seq(first: work())",
            "all(first: work(), second: ui.focus(node_id: \"a\"))",
            "repeat(times: 2, body: work())",
        ] {
            let source = format!("fn work() = {body}\nfn main() = {call}");
            assert!(lower(&parse(&source)).is_err(), "{source}");
        }
    }
    for body in [
        r#"seq(a: bind(r: ui.focus(node_id: "a"), body: 1))"#,
        r#"seq(a: bind(node: "a", body: seq(b: ui.focus(node_id: node))))"#,
        r#"all(a: choose(when: true, then: seq(b: ui.focus(node_id: "a")), otherwise: seq(c: ui.focus(node_id: "b"))), d: ui.focus(node_id: "d"))"#,
    ] {
        assert!(lower(&parse(&format!("fn main() = {body}"))).is_err());
    }
}

#[test]
fn repeat_reserves_expanded_helper_nodes_including_literal_collection_entries() {
    let entries = (0..64)
        .map(|index| format!("i{index}: \"\""))
        .collect::<Vec<_>>()
        .join(", ");
    let definition = "fn focus(parts: string_list) = ui.focus(node_id: concat(left: \"a\", right: join(left: parts, right: \"\")))";
    compile(&format!(
        "{definition}\nfn main() = repeat(times: 13, body: focus(parts: strings({entries})))"
    ));
    let source = format!(
        "{definition}\nfn main() = repeat(times: 15, body: focus(parts: strings({entries})))"
    );
    assert!(
        lower(&parse(&source))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );
    let long_name = "a".repeat(64);
    assert!(
        lower(&parse(&format!(
            r#"fn focus(node: string) = ui.focus(node_id: node)
        fn main() = seq({long_name}: repeat(times: 2, body: focus(node: "a")))"#
        )))
        .is_err()
    );
}

#[test]
fn forged_prepared_members_cannot_bypass_result_type_or_atomic_flow_checks() {
    let baseline = compile(
        r#"fn main() = seq(a: bind(node: "a", body: ui.focus(node_id: node)), b: ui.focus(node_id: "b"))"#,
    );
    for replacement in [
        lower(&parse(
            r#"fn main() = bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        ))
        .unwrap()
        .function
        .effect,
        lower(&parse(r#"fn main() = seq(a: ui.focus(node_id: "a"))"#))
            .unwrap()
            .function
            .effect,
        Effect::UiAssertVisible {
            node_id: "b".into(),
        },
    ] {
        let mut program = baseline.clone();
        let Effect::Compute { expression } = &mut program.function.effect else {
            panic!()
        };
        let Computation::Group { branches, .. } = expression.as_mut() else {
            panic!()
        };
        branches[0].value = match replacement {
            Effect::Compute { expression } => *expression,
            effect => Computation::Host {
                effect: Box::new(effect),
            },
        };
        assert_eq!(
            authorize(&program, &CapabilitySet::new(["ui.presentation"]))
                .unwrap_err()
                .code,
            "LSH1405"
        );
    }
}

#[test]
fn prepared_signature_inspection_is_bounded_even_before_public_hir_validation() {
    let focus = || Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "a".into(),
        }),
    };
    let mut deep = focus();
    for index in 0..128 {
        deep = Computation::Bind {
            name: format!("n{index}"),
            value: Box::new(Computation::Literal {
                value: leselang_hir::computation::ScalarValue::Integer(0),
            }),
            body: Box::new(deep),
        };
    }
    assert_eq!(deep.prepared_atomic_operation(), None);
    assert!(deep.validate_structure().is_err());
    let mismatched = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: leselang_hir::computation::ScalarValue::Boolean(true),
        }),
        then: Box::new(focus()),
        otherwise: Box::new(Computation::Host {
            effect: Box::new(Effect::UiAssertVisible {
                node_id: "a".into(),
            }),
        }),
    };
    assert_eq!(mismatched.prepared_atomic_operation(), None);
    let mut deep_argument = Computation::Literal {
        value: leselang_hir::computation::ScalarValue::String("a".into()),
    };
    for index in 0..128 {
        deep_argument = Computation::Bind {
            name: format!("n{index}"),
            value: Box::new(Computation::Literal {
                value: leselang_hir::computation::ScalarValue::Integer(0),
            }),
            body: Box::new(deep_argument),
        };
    }
    let call = Computation::Call {
        operation: HostOperation::UiFocus,
        arguments: vec![leselang_hir::computation::ComputedArgument {
            name: "node_id".into(),
            value: deep_argument,
        }],
    };
    assert_eq!(call.prepared_atomic_operation(), None);
}
