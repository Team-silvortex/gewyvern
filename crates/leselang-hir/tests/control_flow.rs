use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn sequence_and_repeat_lower_in_order_and_canonicalize_without_new_authority() {
    let program = lower(&parse(r#"fn main() = seq(
        inventory: runtime.list(),
        focus: repeat(times: 2, body: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")))
    )"#)).unwrap();
    let Effect::Sequence { steps } = &program.function.effect else {
        panic!("expected sequence")
    };
    assert_eq!(
        steps
            .iter()
            .map(|step| step.name.as_str())
            .collect::<Vec<_>>(),
        [
            "inventory",
            "focus__iteration_1__first",
            "focus__iteration_1__second",
            "focus__iteration_2__first",
            "focus__iteration_2__second"
        ]
    );
    assert_eq!(program.function.result_type, Type::Structured);
    assert_eq!(steps[0].result_type, Type::RuntimeList);
    assert_eq!(steps[1].result_type, Type::UiFocus);
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
    assert!(
        authorize(
            &program,
            &CapabilitySet::new(["runtime.read", "ui.presentation"])
        )
        .is_ok()
    );
    let source = canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        lower(&parse(&source)).unwrap().function.effect,
        program.function.effect
    );
}

#[test]
fn repeat_is_not_an_unbounded_or_implicitly_coerced_loop() {
    for source in [
        "fn main() = seq()",
        "fn main() = seq(a: runtime.list(), a: runtime.list())",
        "fn main() = repeat(times: 0, body: runtime.list())",
        "fn main() = repeat(times: 65, body: runtime.list())",
        "fn main() = repeat(times: 18446744073709551615, body: runtime.list())",
        "fn main() = repeat(times: \"2\", body: runtime.list())",
        "fn main() = repeat(times: 2, times: 2)",
        "fn main() = repeat(times: 2, body: none)",
        "fn main() = repeat(times: 2, body: runtime.list(), extra: none)",
        "fn main() = repeat(times: 9, body: repeat(times: 8, body: runtime.list()))",
        "fn main() = seq(a: all(b: runtime.list(), c: runtime.list()))",
        "fn main() = seq(a__b: runtime.list(), a: seq(b: runtime.list()))",
        "fn main() = runtime.list(role: 2)",
    ] {
        assert!(lower(&parse(source)).is_err(), "accepted {source}");
    }
    for count in [1, 64] {
        let program = lower(&parse(&format!(
            "fn main() = repeat(times: {count}, body: runtime.list())"
        )))
        .unwrap();
        assert!(
            matches!(program.function.effect, Effect::Sequence { steps } if steps.len() == count)
        );
    }
}

#[test]
fn flattened_names_and_forged_control_flow_metadata_are_checked() {
    let source = format!(
        "fn main() = seq({}: repeat(times: 2, body: runtime.list()))",
        "a".repeat(64)
    );
    assert!(lower(&parse(&source)).is_err());
    let mut program = lower(&parse(
        "fn main() = seq(a: runtime.list(), b: runtime.refresh(runtime_id: \"a\"))",
    ))
    .unwrap();
    program.function.required_capabilities = vec!["runtime.read".into()];
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
}
