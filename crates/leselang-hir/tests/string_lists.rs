use leselang_hir::computation::{Computation, ScalarType, ScalarValue, StringListValue};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

#[test]
fn collections_and_folds_round_trip_with_explicit_types_and_source_order() {
    for (body, ty) in [
        ("strings()", ScalarType::StringList),
        (
            r#"strings(z: "last", a: "first", empty: "")"#,
            ScalarType::StringList,
        ),
        (
            r#"strings(a: to_string(value: 1), b: "two")"#,
            ScalarType::StringList,
        ),
        (r#"split(left: "a,b", right: ",")"#, ScalarType::StringList),
        (
            r#"append(left: strings(), right: "a")"#,
            ScalarType::StringList,
        ),
        (
            r#"join(left: strings(a: "a", b: "b"), right: ",")"#,
            ScalarType::String,
        ),
        (
            r#"item_at(left: strings(a: ""), right: 0)"#,
            ScalarType::OptionalString,
        ),
        (r#"len(value: strings())"#, ScalarType::Integer),
        (
            r#"eq(left: strings(), right: strings())"#,
            ScalarType::Boolean,
        ),
        (
            r#"fold(total: 0, items: strings(a: "2", b: "3"), item: "part", next: add(left: total, right: parse_integer(value: part)), limit: 2)"#,
            ScalarType::Integer,
        ),
        (
            r#"fold(result: strings(), items: strings(a: "a"), item: "part", next: append(left: result, right: part), limit: 1)"#,
            ScalarType::StringList,
        ),
    ] {
        let syntax = parse(&format!("fn main() = {body}"));
        let program = lower(&syntax).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        authorize(&program, &CapabilitySet::default()).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
        let formatted = format(&syntax).unwrap();
        assert_eq!(format(&parse(&formatted)).unwrap(), formatted);
    }
    let program = lower(&parse(r#"fn main() = strings(z: "last", a: "first")"#)).unwrap();
    assert!(
        matches!(program.function.effect, Effect::Compute { ref expression }
        if matches!(expression.as_ref(), Computation::Literal { value: ScalarValue::StringList(value) }
        if value.0 == ["last", "first"]))
    );
}

#[test]
fn collection_types_and_fold_lexical_scope_are_closed_even_in_cold_code() {
    for body in [
        r#"strings(a: 1)"#,
        r#"strings(a: none)"#,
        r#"strings(a: optional_string(value: "x"))"#,
        r#"strings(a: strings())"#,
        r#"strings(a: "x", a: "y")"#,
        r#"split(left: strings(), right: ",")"#,
        r#"append(left: "x", right: "y")"#,
        r#"join(left: strings(), right: 1)"#,
        r#"item_at(left: strings(), right: "0")"#,
        r#"to_string(value: strings())"#,
        r#"fold(total: 0, items: "a", item: "part", next: total, limit: 1)"#,
        r#"fold(total: 0, items: strings(), item: "part", next: part, limit: 1)"#,
        r#"fold(total: 0, items: strings(), item: "total", next: total, limit: 1)"#,
        r#"fold(total: part, items: strings(), item: "part", next: total, limit: 1)"#,
        r#"fold(total: 0, items: strings(a: part), item: "part", next: total, limit: 1)"#,
        r#"fold(total: 0, items: strings(), item: "part", next: total, limit: 65)"#,
        r#"fold(total: 0, items: strings(), item: "part", next: total, limit: add(left: 1, right: 1))"#,
        r#"fold(total: 0, items: strings(), item: "bad-name", next: total, limit: 0)"#,
        r#"bind(part: "x", body: fold(total: 0, items: strings(), item: "part", next: total, limit: 0))"#,
        r#"bind(total: 0, body: fold(total: 0, items: strings(), item: "part", next: total, limit: 0))"#,
        r#"bind(result: fold(total: 0, items: strings(), item: "part", next: total, limit: 0), body: part)"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
    assert!(
        lower(&parse(
            r#"fn main() = choose(when: true, then: strings(), otherwise: strings(a: false))"#
        ))
        .is_err()
    );
}

#[test]
fn no_collection_operand_or_empty_fold_can_hide_an_effect() {
    for body in [
        r#"strings(a: bind(r: ui.focus(node_id: "a"), body: "x"))"#,
        r#"fold(total: 0, items: strings(), item: "part", next: bind(r: ui.focus(node_id: "a"), body: total), limit: 0)"#,
        r#"fold(total: bind(r: ui.focus(node_id: "a"), body: 0), items: strings(), item: "part", next: total, limit: 0)"#,
        r#"fold(total: 0, items: bind(r: ui.focus(node_id: "a"), body: strings()), item: "part", next: total, limit: 0)"#,
        r#"ui.focus(node_id: strings(a: "a"))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
    for name in ["strings", "fold", "split", "join", "append", "item_at"] {
        assert!(lower(&parse(&format!("fn {name}() = 0\nfn main() = 0"))).is_err());
    }
}

#[test]
fn literal_collection_payloads_retain_bounds_before_indexing_or_shortening() {
    for values in [
        vec![String::new(); 65],
        vec!["x".repeat(4097)],
        vec!["x".repeat(2049); 2],
    ] {
        let effect = Effect::Compute {
            expression: Box::new(Computation::Literal {
                value: ScalarValue::StringList(StringListValue(values)),
            }),
        };
        assert!(canonical_source(&effect).is_err());
    }
    let items = (0..64)
        .map(|index| format!("i{index}: \"{}\"", "x".repeat(64)))
        .collect::<Vec<_>>()
        .join(", ");
    let program = lower(&parse(&format!("fn main() = strings({items})"))).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
    assert!(
        lower(&parse(&format!(
            "fn main() = strings({items}, extra: \"\")"
        )))
        .is_err()
    );
    assert!(
        lower(&parse(&format!(
            "fn main() = strings(a: \"{}\", b: \"x\")",
            "x".repeat(4096)
        )))
        .is_err()
    );
}

#[test]
fn pure_collection_helpers_expand_hygienically_and_preserve_host_authority() {
    let source = r#"fn count(parts: string_list) = fold(total: 0, items: parts, item: "entry", next: add(left: total, right: len(value: entry)), limit: 64)
        fn main() = bind(entry: "outer", body: bind(total: 9, body:
            bind(status: ui.assert_text(node_id: "status", expected: "a,b"), body:
                ui.set_form_value(node_id: "form", field: "size", value: to_string(value: count(parts: split(left: field(value: status, name: "expected"), right: ",")))))))"#;
    let program = lower(&parse(source)).unwrap();
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(authorize(&program, &CapabilitySet::default()).is_err());
    authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
}
