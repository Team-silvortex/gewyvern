use leselang_hir::computation::{Computation, GroupLocalType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::ScalarType;
use leselang_syntax::{Expression, Span, parse};

fn body(source: &str) -> (Computation, Type, Vec<String>) {
    let program = lower(&parse(source)).unwrap();
    let canonical = canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&lower(&parse(&canonical)).unwrap()).unwrap()
    );
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected a native computation")
    };
    (
        *expression,
        program.function.result_type,
        program.function.required_capabilities,
    )
}

fn member<'node>(node: &'node Computation, group: &str, name: &str) -> &'node HostOperation {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        if let Computation::Member {
            group: bound,
            name: selected,
            operation,
        } = node
            && bound == group
            && selected == name
        {
            return operation;
        }
        pending.extend(node.children());
    }
    panic!("expected exact native member")
}

#[test]
fn reference_member_source_preserves_exact_native_slot_without_a_synthetic_local() {
    let (node, ty, capabilities) = body(
        "fn main() = bind(rows: seq(a: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"count\"))",
    );
    assert_eq!(ty, Type::Scalar(ScalarType::Integer));
    assert_eq!(capabilities, ["runtime.read"]);
    assert_eq!(*member(&node, "rows", "a"), HostOperation::RuntimeList);
    let Computation::Bind { body, .. } = &node else {
        panic!()
    };
    let Computation::Field { value, .. } = body.as_ref() else {
        panic!()
    };
    assert!(matches!(value.as_ref(), Computation::Member { .. }));
    assert_eq!(value.children().count(), 0);
}

#[test]
fn selected_group_and_helper_exports_keep_their_original_closed_member_observations() {
    for source in [
        "fn main() = bind(rows: choose(when: true, then: seq(a: runtime.list()), otherwise: seq(a: runtime.list())), body: field(value: member(value: rows, name: \"a\"), name: \"count\"))",
        "fn rows() = seq(a: runtime.list())\nfn main() = bind(batch: rows(), body: field(value: member(value: batch, name: \"a\"), name: \"count\"))",
        "fn echo(n: integer) = n\nfn main() = bind(batch: seq(a: runtime.list()), body: echo(n: field(value: member(value: batch, name: \"a\"), name: \"count\")))",
    ] {
        let (_, ty, capabilities) = body(source);
        assert_eq!(ty, Type::Scalar(ScalarType::Integer));
        assert_eq!(capabilities, ["runtime.read"]);
    }
}

#[test]
fn same_named_member_in_a_different_group_does_not_authorize_a_foreign_native_operation() {
    let (node, _, _) = body(
        "fn main() = bind(rows: seq(a: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"revision\"))",
    );
    let Computation::Bind { mut body, .. } = node else {
        panic!()
    };
    let groups = [
        GroupLocalType {
            name: "rows".into(),
            members: vec![("a".into(), HostOperation::RuntimeList)],
        },
        GroupLocalType {
            name: "other".into(),
            members: vec![("a".into(), HostOperation::RuntimeInspect)],
        },
    ];
    assert_eq!(
        body.validate_in_group_scope(&[], &groups).unwrap(),
        Type::Scalar(ScalarType::Integer)
    );
    let Computation::Field { value, .. } = body.as_mut() else {
        panic!()
    };
    let Computation::Member { group, .. } = value.as_mut() else {
        panic!()
    };
    *group = "other".into();
    assert!(body.validate_in_group_scope(&[], &groups).is_err());
}

#[test]
fn same_result_field_type_does_not_allow_substitution_of_another_native_operation() {
    let (node, _, _) = body(
        "fn main() = bind(rows: seq(a: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"revision\"))",
    );
    let Computation::Bind { mut body, .. } = node else {
        panic!()
    };
    let groups = [GroupLocalType {
        name: "rows".into(),
        members: vec![("a".into(), HostOperation::RuntimeList)],
    }];
    let Computation::Field { value, .. } = body.as_mut() else {
        panic!()
    };
    let Computation::Member { operation, .. } = value.as_mut() else {
        panic!()
    };
    *operation = HostOperation::RuntimeInspect;
    assert!(body.validate_in_group_scope(&[], &groups).is_err());
}

#[test]
fn scalar_record_missing_and_expired_helper_scopes_do_not_supply_member_exports() {
    for source in [
        "fn main() = bind(row: 1, body: member(value: row, name: \"a\"))",
        "fn main() = bind(row: runtime.list(), body: member(value: row, name: \"a\"))",
        "fn main() = member(value: row, name: \"a\")",
        "fn count() = bind(rows: seq(a: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"count\"))\nfn main() = member(value: rows, name: \"a\")",
        "fn main() = bind(rows: seq(a: runtime.list()), body: member(value: rows, name: \"other\"))",
    ] {
        let errors = lower(&parse(source)).unwrap_err();
        assert!(!errors.is_empty());
        assert!(
            errors.iter().all(|error| error.code == "LSH1412"),
            "{source}: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .all(|error| error.message == "member is not exported by this bound group")
        );
    }
}

#[test]
fn member_header_diagnostics_keep_legacy_root_spans_and_never_lower_metadata_as_values() {
    for (source, code, message) in [
        (
            "member(value: runtime.list(), name: \"a\")",
            "LSH1412",
            "member requires a bound group reference and a literal step name",
        ),
        (
            "member(value: missing, name: missing)",
            "LSH1412",
            "member requires a bound group reference and a literal step name",
        ),
        (
            "member(value: missing, name: 1)",
            "LSH1412",
            "member requires a bound group reference and a literal step name",
        ),
        (
            "member(value: missing)",
            "LSH1401",
            "expected exactly the named arguments: value, name",
        ),
        (
            "member(value: missing, name: \"a\", name: \"b\")",
            "LSH1401",
            "expected exactly the named arguments: value, name",
        ),
        (
            "member(value: missing, name: \"bad name\")",
            "LSH1412",
            "member is not exported by this bound group",
        ),
    ] {
        let tree = parse(&format!("fn main() = {source}"));
        let Expression::Call { span, .. } = &tree.function.as_ref().unwrap().body else {
            panic!()
        };
        let errors = lower(&tree).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        assert_eq!(errors[0].span, Some(*span));
    }
}

#[test]
fn full_width_group_last_member_and_source_metadata_limits_preserve_wire_and_authority() {
    for width in [1, 32, 64] {
        let members = (0..width)
            .map(|index| format!("b{index}: runtime.list()"))
            .collect::<Vec<_>>()
            .join(", ");
        let last = width - 1;
        let source = format!(
            "fn main() = bind(rows: seq({members}), body: field(value: member(value: rows, name: \"b{last}\"), name: \"count\"))"
        );
        let (node, ty, _) = body(&source);
        assert_eq!(ty, Type::Scalar(ScalarType::Integer));
        assert_eq!(
            *member(&node, "rows", &format!("b{last}")),
            HostOperation::RuntimeList
        );
        let program = lower(&parse(&source)).unwrap();
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(&program, &CapabilitySet::new(["runtime.read"])).unwrap();
    }
}

#[test]
fn field_child_error_precedence_is_unchanged_when_member_lowering_delegates_to_shared_core() {
    let source = "fn main() = field(value: member(value: missing, name: \"a\"), name: 1)";
    let tree = parse(source);
    let Expression::Call { arguments, .. } = &tree.function.as_ref().unwrap().body else {
        panic!()
    };
    let Expression::Call { span, .. } = &arguments[0].value else {
        panic!()
    };
    let errors = lower(&tree).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "LSH1412");
    assert_eq!(
        errors[0].message,
        "member is not exported by this bound group"
    );
    assert_eq!(
        errors[0].span,
        Some(Span {
            start: span.start,
            end: span.end
        })
    );
}
