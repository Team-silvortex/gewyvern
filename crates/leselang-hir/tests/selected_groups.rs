use leselang_hir::computation::ScalarType;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn conditional_group_exports_roundtrip_for_inline_and_function_returns() {
    for kind in ["seq", "all"] {
        for helper in [false, true] {
            let groups = format!(
                r#"choose(when: alternate, then: {kind}(first: ui.focus(node_id: "a"), second: ui.assert_text(node_id: "a", expected: "ready")), otherwise: {kind}(first: ui.focus(node_id: "b"), second: ui.assert_text(node_id: "b", expected: concat(left: "re", right: "ady"))))"#
            );
            let source = if helper {
                format!(
                    r#"fn rows(alternate: boolean) = {groups}
                    fn main() = bind(group: rows(alternate: false), body: field(value: member(value: group, name: "second"), name: "expected"))"#
                )
            } else {
                format!(
                    r#"fn main() = bind(alternate: true, body: bind(group: {groups}, body: field(value: member(value: group, name: "second"), name: "expected")))"#
                )
            };
            let program = lower(&parse(&source)).unwrap();
            assert_eq!(
                program.function.result_type,
                Type::Scalar(ScalarType::String)
            );
            assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
            let canonical = canonical_source(&program.function.effect).unwrap();
            assert!(!canonical.contains("rows("));
            assert_eq!(lower(&parse(&canonical)).unwrap(), program);
            assert!(authorize(&program, &CapabilitySet::new([] as [&str; 0])).is_err());
            authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
        }
    }
}

#[test]
fn choosing_between_prepared_group_helper_calls_preserves_the_named_interface() {
    let source = r#"fn rows(node: string) = seq(first: ui.focus(node_id: concat(left: "node-", right: node)), second: ui.assert_text(node_id: node, expected: "ready"))
        fn main() = bind(group: choose(when: true, then: rows(node: "a"), otherwise: rows(node: "b")), body: field(value: member(value: group, name: "first"), name: "node_id"))"#;
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::String)
    );
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
}

#[test]
fn nested_choices_pure_preparation_and_group_aliases_preserve_closed_members() {
    let source = r#"fn rows(node: string, alternate: boolean) = bind(target: concat(left: node, right: "-row"), body:
        choose(when: alternate, then: seq(first: ui.focus(node_id: target)), otherwise:
            choose(when: false, then: seq(first: ui.focus(node_id: "cold")), otherwise: seq(first: ui.focus(node_id: node)))))
        fn main() = bind(group: rows(node: "a", alternate: false), body: bind(alias: group, body:
            bind(written: ui.set_form_value(node_id: "form", field: "target", value: field(value: member(value: alias, name: "first"), name: "node_id")), body: field(value: written, name: "value"))))"#;
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::String)
    );
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
}

#[test]
fn cold_group_exports_cannot_union_names_reorder_members_or_change_operations_and_modes() {
    for otherwise in [
        r#"seq(other: ui.focus(node_id: "b"))"#,
        r#"seq(first: ui.activate(node_id: "b"))"#,
        r#"seq(first: ui.focus(node_id: "b"), second: ui.focus(node_id: "b"))"#,
        r#"all(first: ui.focus(node_id: "b"), second: ui.focus(node_id: "b"))"#,
    ] {
        for helper in [false, true] {
            let choice = format!(
                r#"choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: {otherwise})"#
            );
            let source = if helper {
                format!(
                    r#"fn rows() = {choice}
                    fn main() = bind(group: rows(), body: field(value: member(value: group, name: "first"), name: "node_id"))"#
                )
            } else {
                format!(r#"fn main() = bind(group: {choice}, body: true)"#)
            };
            assert!(lower(&parse(&source)).is_err(), "{source}");
        }
    }
    let source = r#"fn main() = bind(group: choose(when: true,
        then: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")),
        otherwise: seq(second: ui.focus(node_id: "b"), first: ui.focus(node_id: "a"))), body: true)"#;
    assert!(lower(&parse(source)).is_err());
    assert!(
        lower(&parse(&source.replace(
            "seq(second: ui.focus(node_id: \"b\"), first: ui.focus(node_id: \"a\"))",
            "all(first: ui.focus(node_id: \"a\"), second: ui.focus(node_id: \"b\"))",
        )))
        .is_err()
    );
}

#[test]
fn cold_domains_guards_and_result_dependent_group_nesting_remain_rejected() {
    for source in [
        r#"fn main() = bind(group: choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "bad node"))), body: true)"#,
        r#"fn main() = bind(group: choose(when: ui.assert_visible(node_id: "a"), then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b"))), body: true)"#,
        r#"fn main() = bind(result: ui.focus(node_id: "a"), body: bind(group: choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b"))), body: true))"#,
        r#"fn rows() = choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b")))
            fn main() = seq(first: rows())"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn selected_group_and_longest_successor_path_share_the_existing_slot_limit() {
    for count in [63, 64] {
        let members = (0..count)
            .map(|index| format!(r#"step{index}: ui.focus(node_id: "a")"#))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            r#"fn main() = bind(group: choose(when: true, then: seq({members}), otherwise: seq({members})), body: ui.focus(node_id: field(value: member(value: group, name: "step0"), name: "node_id")))"#
        );
        let lowered = lower(&parse(&source));
        if count == 63 {
            let program = lowered.unwrap();
            assert_eq!(
                lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
                program
            );
        } else {
            assert!(
                lowered
                    .unwrap_err()
                    .iter()
                    .any(|error| error.code == "LSH1412")
            );
        }
    }
}

#[test]
fn forged_cold_group_signature_fails_canonical_and_authority_revalidation() {
    let source = r#"fn main() = bind(group: choose(when: true, then: seq(first: ui.focus(node_id: "a")), otherwise: seq(first: ui.focus(node_id: "b"))), body: field(value: member(value: group, name: "first"), name: "node_id"))"#;
    let mut program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = &mut program.function.effect else {
        panic!()
    };
    let leselang_hir::computation::Computation::Bind { value, .. } = expression.as_mut() else {
        panic!()
    };
    let leselang_hir::computation::Computation::Choose { otherwise, .. } = value.as_mut() else {
        panic!()
    };
    let leselang_hir::computation::Computation::Host { effect } = otherwise.as_mut() else {
        panic!()
    };
    let Effect::Sequence { steps } = effect.as_mut() else {
        panic!()
    };
    steps[0].name = "different".into();
    assert!(canonical_source(&program.function.effect).is_err());
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
}
