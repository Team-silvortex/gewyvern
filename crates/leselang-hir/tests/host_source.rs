use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::host_source::{HostSourceError, HostSourceResult, lower_host_source};
use leselang_hir::ir::Computation;
use leselang_hir::source_call::{SourceCallError, SourceCallLimits};
use leselang_syntax::{Expression, parse};

// Native payloads and observations deliberately have no Clone/Debug/serde/Send.
struct NativeEffect {
    owner: Rc<()>,
    operation: u32,
    arguments: Vec<u8>,
    hidden_nodes: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for NativeEffect {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Observation {
    owner: Rc<()>,
    declaration: u32,
    drops: Rc<Cell<usize>>,
}
impl Drop for Observation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError(&'static str);
type Node = Computation<u8, u32, NativeEffect, u16>;
type Output = HostSourceResult<Node, Observation, PrivateError>;

const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 8,
    max_source_depth: 4,
    max_lowered_nodes: 1,
    max_lowered_depth: 0,
    max_arguments: 4,
};

fn expression(source: &str) -> Expression {
    parse(&format!("fn main() = {source}"))
        .function
        .unwrap()
        .body
}
fn prepared(owner: &Rc<()>, drops: &Rc<Cell<usize>>) -> (NativeEffect, Observation) {
    (
        NativeEffect {
            owner: owner.clone(),
            operation: 7,
            arguments: vec![3; 17],
            hidden_nodes: 1,
            drops: drops.clone(),
        },
        Observation {
            owner: owner.clone(),
            declaration: 7,
            drops: drops.clone(),
        },
    )
}
fn corroborate(
    effect: &NativeEffect,
    ty: &Observation,
    owner: &Rc<()>,
    live: bool,
) -> Result<(), PrivateError> {
    (live
        && Rc::ptr_eq(&effect.owner, owner)
        && Rc::ptr_eq(&ty.owner, owner)
        && effect.operation == ty.declaration
        && ty.declaration == 7
        && effect.hidden_nodes <= 4)
        .then_some(())
        .ok_or(PrivateError("private native policy"))
}

#[test]
fn original_ast_payload_buffer_and_move_only_type_pass_once_into_one_host_root() {
    let source = expression("device.read(name: \"original\")");
    let owner = Rc::new(());
    let drops = Rc::new(Cell::new(0));
    let (effect, ty) = prepared(&owner, &drops);
    let buffer = effect.arguments.as_ptr();
    let events = RefCell::new(Vec::new());
    let output: Output = lower_host_source(
        &source,
        LIMITS,
        |original| {
            events.borrow_mut().push("prepare");
            assert!(std::ptr::eq(original, &source));
            Ok((effect, ty))
        },
        |original, effect, ty| {
            events.borrow_mut().push("admit");
            assert!(std::ptr::eq(original, &source));
            assert_eq!(effect.arguments.as_ptr(), buffer);
            corroborate(effect, ty, &owner, true)
        },
    );
    let (node, ty) = output.unwrap();
    assert_eq!(*events.borrow(), ["prepare", "admit"]);
    assert_eq!(node.children().count(), 0);
    assert!(!node.is_pure());
    let Node::Host { effect } = &node else {
        panic!()
    };
    assert_eq!(effect.arguments.as_ptr(), buffer);
    assert!(Rc::ptr_eq(&ty.owner, &owner));
    assert_eq!(drops.get(), 0);
    drop((node, ty));
    assert_eq!(drops.get(), 2);
}

#[test]
fn every_limit_ceiling_precedes_preparation_and_native_admission() {
    let source = expression("device.read()");
    for limits in [
        SourceCallLimits {
            max_source_nodes: 16_385,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 65,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_nodes: 16_385,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_depth: 65,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 65,
            ..LIMITS
        },
    ] {
        let outcome: Output = lower_host_source(
            &source,
            limits,
            |_| panic!("invalid limits"),
            |_, _, _| panic!("invalid limits"),
        );
        assert!(matches!(
            outcome,
            Err(HostSourceError::Source(SourceCallError::InvalidLimits))
        ));
    }
}

#[test]
fn whole_cold_source_names_text_arguments_and_physical_limits_precede_native_work() {
    for (source, limits) in [
        (
            expression("device.read()"),
            SourceCallLimits {
                max_source_nodes: 0,
                ..LIMITS
            },
        ),
        (
            expression("device.read(value: device.hidden(a: 1, b: 2))"),
            SourceCallLimits {
                max_arguments: 1,
                ..LIMITS
            },
        ),
        (
            expression("device.read(value: device.hidden(a: 1))"),
            SourceCallLimits {
                max_source_depth: 1,
                ..LIMITS
            },
        ),
        (
            expression("device.read(value: device.hidden(a: 1))"),
            SourceCallLimits {
                max_source_nodes: 2,
                ..LIMITS
            },
        ),
        (
            expression(&format!("device.read(value: \"{}\")", "x".repeat(4097))),
            LIMITS,
        ),
    ] {
        let outcome: Output = lower_host_source(
            &source,
            limits,
            |_| panic!("invalid cold source"),
            |_, _, _| panic!("invalid cold source"),
        );
        assert!(matches!(outcome, Err(HostSourceError::Source(_))));
    }
    let mut source = expression("device.read(value: device.hidden(a: 1))");
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    arguments[0].name = "invalid name".into();
    let outcome: Output = lower_host_source(
        &source,
        LIMITS,
        |_| panic!("invalid cold name"),
        |_, _, _| panic!("invalid cold name"),
    );
    assert!(matches!(outcome, Err(HostSourceError::Source(_))));
}

#[test]
fn noncall_and_zero_generated_capacity_never_prepare_native_parts() {
    let outcome: Output = lower_host_source(
        &expression("1"),
        LIMITS,
        |_| panic!("not a call"),
        |_, _, _| panic!("not a call"),
    );
    assert!(matches!(outcome, Err(HostSourceError::NotCall { .. })));
    let outcome: Output = lower_host_source(
        &expression("device.read()"),
        SourceCallLimits {
            max_lowered_nodes: 0,
            ..LIMITS
        },
        |_| panic!("no root"),
        |_, _, _| panic!("no root"),
    );
    assert!(matches!(outcome, Err(HostSourceError::Output { .. })));
}

#[test]
fn source_metadata_is_not_hidden_effect_or_extra_language_node_accounting() {
    let source = expression("device.read(a: \"first\", b: \"second\")");
    let owner = Rc::new(());
    let drops = Rc::new(Cell::new(0));
    let output: Output = lower_host_source(
        &source,
        SourceCallLimits {
            max_source_nodes: 3,
            max_source_depth: 1,
            ..LIMITS
        },
        |_| Ok(prepared(&owner, &drops)),
        |_, effect, ty| corroborate(effect, ty, &owner, true),
    );
    let (node, ty) = output.unwrap();
    assert_eq!(node.children().count(), 0);
    drop((node, ty));
    assert_eq!(drops.get(), 2);
}

#[test]
fn preparation_failure_keeps_private_payload_and_stops_admission() {
    let output: Output = lower_host_source(
        &expression("device.read()"),
        LIMITS,
        |_| Err(PrivateError("secret preparation")),
        |_, _, _| panic!("preparation failed"),
    );
    let error = output.err().unwrap();
    assert!(!format!("{error:?}").contains("secret"));
    assert!(std::error::Error::source(&error).is_none());
    let HostSourceError::Preparation { error, .. } = error else {
        panic!()
    };
    assert_eq!(error.0, "secret preparation");
}

#[test]
fn admission_failure_releases_native_effect_and_observation_without_partial_handoff() {
    let owner = Rc::new(());
    let drops = Rc::new(Cell::new(0));
    let queries = Cell::new(0);
    let output: Output = lower_host_source(
        &expression("device.read()"),
        LIMITS,
        |_| Ok(prepared(&owner, &drops)),
        |_, _, _| {
            queries.set(queries.get() + 1);
            Err(PrivateError("secret admission"))
        },
    );
    let error = output.err().unwrap();
    assert!(!format!("{error}").contains("secret"));
    assert_eq!(queries.get(), 1);
    assert_eq!(drops.get(), 2);
    let HostSourceError::Admission { error, .. } = error else {
        panic!()
    };
    assert_eq!(error.0, "secret admission");
}

#[test]
fn native_preparation_or_admission_unwind_drops_owned_parts_without_retry() {
    for prepare_unwind in [false, true] {
        let owner = Rc::new(());
        let drops = Rc::new(Cell::new(0));
        let prepare_count = Cell::new(0);
        let admit_count = Cell::new(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let output: Output = lower_host_source(
                &expression("device.read()"),
                LIMITS,
                |_| {
                    prepare_count.set(prepare_count.get() + 1);
                    let parts = prepared(&owner, &drops);
                    if prepare_unwind {
                        panic!("native prepare unwind")
                    }
                    Ok(parts)
                },
                |_, _, _| {
                    admit_count.set(admit_count.get() + 1);
                    panic!("native admission unwind")
                },
            );
            output
        }));
        assert!(outcome.is_err());
        assert_eq!(prepare_count.get(), 1);
        assert_eq!(admit_count.get(), usize::from(!prepare_unwind));
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn one_language_root_never_certifies_foreign_result_or_unbounded_opaque_graphs() {
    for mismatch in [
        "effect-owner",
        "result-owner",
        "declaration",
        "hidden-graph",
        "foreign-operation-and-result",
        "revoked",
    ] {
        let owner = Rc::new(());
        let foreign = Rc::new(());
        let drops = Rc::new(Cell::new(0));
        let (mut effect, mut ty) = prepared(&owner, &drops);
        match mismatch {
            "effect-owner" => effect.owner = foreign,
            "result-owner" => ty.owner = foreign,
            "declaration" => ty.declaration = 8,
            "hidden-graph" => effect.hidden_nodes = 5,
            "foreign-operation-and-result" => {
                effect.operation = 8;
                ty.declaration = 8;
            }
            _ => {}
        }
        let output: Output = lower_host_source(
            &expression("device.read()"),
            LIMITS,
            |_| Ok((effect, ty)),
            |_, effect, ty| corroborate(effect, ty, &owner, mismatch != "revoked"),
        );
        assert!(
            matches!(output, Err(HostSourceError::Admission { .. })),
            "{mismatch}"
        );
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn unrelated_numeric_and_panel_hosts_use_the_same_entry_without_product_payloads() {
    struct NumericCommand(Vec<u64>);
    struct NumericDeclaration(Rc<()>);
    struct PanelCommand {
        text: String,
        schema: Rc<()>,
    }
    struct PanelDeclaration {
        schema: Rc<()>,
        result_name: &'static str,
    }
    let numeric_owner = Rc::new(());
    let numeric_source = expression("counter.read()");
    let numeric: HostSourceResult<
        Computation<(), u64, NumericCommand, ()>,
        NumericDeclaration,
        PrivateError,
    > = lower_host_source(
        &numeric_source,
        LIMITS,
        |_| {
            Ok((
                NumericCommand(vec![41]),
                NumericDeclaration(numeric_owner.clone()),
            ))
        },
        |source, command, declaration| {
            assert!(std::ptr::eq(source, &numeric_source));
            (command.0 == [41] && Rc::ptr_eq(&declaration.0, &numeric_owner))
                .then_some(())
                .ok_or(PrivateError("numeric row"))
        },
    );
    let (numeric_node, _) = numeric.unwrap();
    let panel_owner = Rc::new(());
    let panel_source = expression("panel.read(node: \"status\")");
    let text = String::from("Ready");
    let original_buffer = text.as_ptr();
    let panel: HostSourceResult<
        Computation<String, char, PanelCommand, ()>,
        PanelDeclaration,
        PrivateError,
    > = lower_host_source(
        &panel_source,
        LIMITS,
        |_| {
            Ok((
                PanelCommand {
                    text,
                    schema: panel_owner.clone(),
                },
                PanelDeclaration {
                    schema: panel_owner.clone(),
                    result_name: "caption",
                },
            ))
        },
        |source, command, declaration| {
            assert!(std::ptr::eq(source, &panel_source));
            assert_eq!(command.text.as_ptr(), original_buffer);
            (Rc::ptr_eq(&command.schema, &declaration.schema)
                && declaration.result_name == "caption")
                .then_some(())
                .ok_or(PrivateError("panel row"))
        },
    );
    let (panel_node, _) = panel.unwrap();
    assert!(matches!(numeric_node, Computation::Host { .. }));
    let Computation::Host { effect } = panel_node else {
        panic!()
    };
    assert_eq!(effect.text.as_ptr(), original_buffer);
}

#[test]
fn native_observations_after_handoff_are_not_saved_admission_certificates() {
    let owner = Rc::new(());
    let drops = Rc::new(Cell::new(0));
    let output: Output = lower_host_source(
        &expression("device.read()"),
        LIMITS,
        |_| Ok(prepared(&owner, &drops)),
        |_, effect, ty| corroborate(effect, ty, &owner, true),
    );
    let (mut node, ty) = output.unwrap();
    let Node::Host { effect } = &mut node else {
        panic!()
    };
    effect.hidden_nodes = 5;
    assert!(corroborate(effect, &ty, &owner, true).is_err());
    drop((node, ty));
    assert_eq!(drops.get(), 2);
}
