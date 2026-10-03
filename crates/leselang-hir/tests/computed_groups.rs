use leselang_hir::computation::{Computation, GroupKind};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

fn compile(expression: &str) -> leselang_hir::HirProgram {
    lower(&parse(&format!("fn main() = {expression}"))).unwrap()
}

#[test]
fn computed_groups_preserve_scope_flattening_order_and_canonical_roundtrip() {
    let source = r#"bind(node: concat(left: "runtime-", right: "a"), body: seq(
        first: ui.focus(node_id: "static"),
        replay: repeat(times: 2, body: seq(
            focus: ui.focus(node_id: node),
            verify: ui.assert_text(node_id: node, expected: concat(left: "Rea", right: "dy"))
        ))))"#;
    let program = compile(source);
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!("expected binding")
    };
    let Computation::Group {
        group_kind,
        branches,
    } = body.as_ref()
    else {
        panic!("expected group")
    };
    assert_eq!(*group_kind, GroupKind::Sequence);
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>(),
        [
            "first",
            "replay__iteration_1__focus",
            "replay__iteration_1__verify",
            "replay__iteration_2__focus",
            "replay__iteration_2__verify",
        ]
    );
    assert_eq!(program.function.result_type, Type::Structured);
    assert_eq!(branches[0].result_type, Type::UiFocus);
    assert_eq!(branches[2].result_type, Type::UiAssertText);
    let canonical = canonical_source(&program.function.effect).unwrap();
    assert!(!canonical.contains("repeat("));
    assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();

    for source in [
        r#"seq(a: ui.focus(node_id: concat(left: "a", right: "b")))"#,
        r#"bind(node: "a", body: seq(a: ui.focus(node_id: node)))"#,
        r#"repeat(times: 2, body: ui.focus(node_id: concat(left: "a", right: "b")))"#,
        r#"all(true: ui.focus(node_id: concat(left: "a", right: "b")), false: ui.focus(node_id: "c"))"#,
        r#"bind(empty: none, body: all(first: runtime.list(role: empty), second: runtime.list()))"#,
        r#"seq(a: ui.focus(node_id: bind(label: "a", body: label)), b: ui.focus(node_id: bind(label: "b", body: label)))"#,
        r#"bind(node: "a", body: seq(first: seq(focus: ui.focus(node_id: node)), second: repeat(times: 2, body: ui.focus(node_id: node))))"#,
    ] {
        let program = compile(source);
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    }
}

#[test]
fn computed_groups_preflight_all_authority_including_cold_branches() {
    let mut program = compile(
        r#"bind(node: "a", body: choose(when: true,
        then: seq(read: runtime.inspect(runtime_id: node)),
        otherwise: seq(write: runtime.refresh(runtime_id: node))))"#,
    );
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "runtime.refresh"]
    );
    assert_eq!(
        authorize(&program, &CapabilitySet::new(["runtime.read"]))
            .unwrap_err()
            .code,
        "LSH2001"
    );
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "runtime.refresh"]),
    )
    .unwrap();
    program.function.required_capabilities.pop();
    assert_eq!(
        authorize(
            &program,
            &CapabilitySet::new(["runtime.read", "runtime.refresh"])
        )
        .unwrap_err()
        .code,
        "LSH2002"
    );
}

#[test]
fn computed_groups_do_not_enable_result_binding_unbounded_counts_or_mixed_groups() {
    for source in [
        r#"bind(count: 2, body: repeat(times: count, body: ui.focus(node_id: "a")))"#,
        r#"repeat(times: add(left: 1, right: 1), body: ui.focus(node_id: concat(left: "a", right: "b")))"#,
        r#"seq(first: ui.focus(node_id: concat(left: "a", right: "b")), value: 1)"#,
        r#"seq(a: bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id")))"#,
        r#"seq(a: choose(when: true, then: ui.focus(node_id: "a"), otherwise: bind(r: ui.focus(node_id: "b"), body: ui.focus(node_id: "c"))))"#,
        r#"seq(a: ui.focus(node_id: bind(local: "a", body: local)), b: ui.focus(node_id: local))"#,
        r#"seq(a: ui.focus(node_id: concat(left: "a", right: "b")), b: all(c: runtime.list(), d: runtime.list()))"#,
        r#"all(a: ui.focus(node_id: concat(left: "a", right: "b")), b: seq(c: runtime.list()))"#,
        r#"all(a: all(b: ui.focus(node_id: concat(left: "a", right: "b")), c: runtime.list()), d: runtime.list())"#,
        r#"choose(when: true, then: seq(a: ui.focus(node_id: "a")), otherwise: seq(b: ui.focus(node_id: missing)))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.assert_child_count(node_id: "a", count: add(left: 1, right: 1)))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: runtime.list()))"#,
        r#"bind(result: ui.focus(node_id: "a"), body: seq(b: ui.focus(node_id: result)))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn expanded_computation_counts_and_names_are_bounded_before_runtime() {
    for depth in [14, 15] {
        let mut value = r#""a""#.to_string();
        for _ in 0..depth {
            value = format!("concat(left: {value}, right: \"\")");
        }
        assert_eq!(
            lower(&parse(&format!(
                "fn main() = seq(a: ui.focus(node_id: {value}))"
            )))
            .is_ok(),
            depth == 14
        );
    }
    let mut value = r#""a""#.to_string();
    for _ in 0..3 {
        value = format!("concat(left: {value}, right: {value})");
    }
    for (count, accepted) in [(63, true), (64, false)] {
        let result = lower(&parse(&format!(
            "fn main() = repeat(times: {count}, body: ui.focus(node_id: {value}))"
        )));
        assert_eq!(result.is_ok(), accepted);
        if let Err(errors) = result {
            assert!(errors.iter().any(|error| error.code == "LSH1405"));
        }
    }
    for count in [1, 64] {
        let program = compile(&format!(
            r#"repeat(times: {count}, body: ui.focus(node_id: concat(left: "a", right: "b")))"#
        ));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    }
    for source in [
        r#"seq(a: repeat(times: 32, body: ui.focus(node_id: concat(left: "a", right: "b"))), b: repeat(times: 33, body: ui.focus(node_id: "b")))"#.to_string(),
        r#"seq(a__iteration_1: ui.focus(node_id: concat(left: "a", right: "b")), a: repeat(times: 1, body: ui.focus(node_id: "b")))"#.into(),
        format!(r#"seq({}: repeat(times: 2, body: ui.focus(node_id: concat(left: "a", right: "b"))))"#, "a".repeat(64)),
    ] {
        assert!(lower(&parse(&format!("fn main() = {source}"))).is_err());
    }
}

#[test]
fn forged_computed_groups_cannot_hide_nested_effects_or_wrong_branch_types() {
    let baseline = compile(
        r#"seq(a: ui.focus(node_id: concat(left: "a", right: "b")), b: ui.focus(node_id: "b"))"#,
    );
    let Effect::Compute { expression: group } = &baseline.function.effect else {
        panic!("expected computation")
    };
    for case in 0..6 {
        let mut program = baseline.clone();
        let Effect::Compute { expression } = &mut program.function.effect else {
            panic!("expected computation")
        };
        let Computation::Group {
            group_kind,
            branches,
        } = expression.as_mut()
        else {
            panic!("expected group")
        };
        match case {
            0 => branches[0].result_type = Type::RuntimeDeploy,
            1 => branches[1].name = branches[0].name.clone(),
            2 => {
                *group_kind = GroupKind::Parallel;
                branches.truncate(1);
            }
            3 => *branches = vec![branches[0].clone(); 65],
            4 => {
                branches[0].value = Computation::Host {
                    effect: Box::new(baseline.function.effect.clone()),
                }
            }
            5 => branches[0].value = *group.clone(),
            _ => unreachable!(),
        }
        assert_eq!(
            authorize(&program, &CapabilitySet::new(["ui.presentation"]))
                .unwrap_err()
                .code,
            "LSH1405"
        );
    }
}
