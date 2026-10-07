use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::control_source::*;
use leselang_hir::ir::Computation;
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarType, ScalarValue, StructureError};
use leselang_syntax::{Expression, parse};

struct Token {
    buffer: Box<[u8]>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Observation {
    ty: ScalarType,
    token: Token,
    panic_drop: bool,
}
impl Drop for Observation {
    fn drop(&mut self) {
        assert!(!self.panic_drop, "private native cleanup");
    }
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
const LIMITS: ControlSourceLimits = ControlSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_bindings: 8,
};
fn source(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn label(expression: &Expression) -> &str {
    match expression {
        Expression::Call { callee, .. } => callee,
        _ => "scalar",
    }
}
struct Adapter {
    events: Vec<String>,
    original: Vec<*const Expression>,
    produced: Vec<*const u8>,
    native_drops: Rc<Cell<usize>>,
    type_drops: Rc<Cell<usize>>,
    seen_types: Vec<(*const u8, *const u8)>,
    fail: Option<ControlSourcePhase>,
    unwind: Option<ControlSourcePhase>,
    false_boolean: bool,
    false_same: bool,
    cleanup_panic: bool,
    expanded: bool,
    admitted: usize,
}
impl Adapter {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            original: Vec::new(),
            produced: Vec::new(),
            native_drops: Rc::new(Cell::new(0)),
            type_drops: Rc::new(Cell::new(0)),
            seen_types: Vec::new(),
            fail: None,
            unwind: None,
            false_boolean: false,
            false_same: false,
            cleanup_panic: false,
            expanded: false,
            admitted: 0,
        }
    }
    fn step(&mut self, phase: ControlSourcePhase) -> Result<(), PrivateError> {
        assert_ne!(self.unwind, Some(phase), "private native unwind");
        if self.fail == Some(phase) {
            Err(PrivateError("private credential"))
        } else {
            Ok(())
        }
    }
    fn observed(&self, ty: ScalarType, panic_drop: bool) -> Observation {
        Observation {
            ty,
            token: Token {
                buffer: vec![7; 33].into_boxed_slice(),
                drops: self.type_drops.clone(),
            },
            panic_drop,
        }
    }
}
impl<'source> ControlSourceAdapter<'source, Token, Token, Token, Token> for Adapter {
    type Type = Observation;
    type Error = PrivateError;
    fn lower_leaf(
        &mut self,
        expression: &'source Expression,
        scope: ControlSourceScope<'_, 'source, Observation>,
    ) -> Result<(Node, Observation), PrivateError> {
        self.events.push(label(expression).into());
        self.original.push(expression);
        self.step(ControlSourcePhase::Leaf)?;
        assert!(!format!("{scope:?}").contains("private"));
        if label(expression) == "native.take" {
            let token = Token {
                buffer: vec![41; 55].into_boxed_slice(),
                drops: self.native_drops.clone(),
            };
            self.produced.push(token.buffer.as_ptr());
            let value = Node::Host {
                effect: Box::new(token),
            };
            let value = if self.expanded {
                Node::Bind {
                    name: "expanded".into(),
                    value: Box::new(Node::Literal {
                        value: ScalarValue::Integer(1),
                    }),
                    body: Box::new(value),
                }
            } else {
                value
            };
            return Ok((
                value,
                self.observed(ScalarType::Integer, self.cleanup_panic),
            ));
        }
        if label(expression) == "native.badcondition" {
            return Ok((
                Node::Host {
                    effect: Box::new(Token {
                        buffer: vec![1].into_boxed_slice(),
                        drops: self.native_drops.clone(),
                    }),
                },
                self.observed(ScalarType::Boolean, false),
            ));
        }
        if label(expression) == "native.wrongliteral" {
            return Ok((
                Node::Literal {
                    value: ScalarValue::Integer(1),
                },
                self.observed(ScalarType::Boolean, false),
            ));
        }
        if label(expression) == "native.observe" {
            let ty = scope.get("ticket").ok_or(PrivateError("missing ticket"))?;
            let same = scope
                .bindings()
                .iter()
                .find(|(name, _)| *name == "ticket")
                .unwrap()
                .1;
            self.seen_types
                .push((ty.token.buffer.as_ptr(), same.token.buffer.as_ptr()));
            return Ok((
                Node::Local {
                    name: "ticket".into(),
                },
                self.observed(ty.ty, false),
            ));
        }
        let prefix = scope
            .bindings()
            .iter()
            .map(|(name, ty)| (*name, ty.ty))
            .collect::<Vec<_>>();
        let (value, ty) = lower_scalar_source_with_scope(
            expression,
            ScalarSourceLimits {
                source: LIMITS.source,
                max_bindings: LIMITS.max_bindings,
            },
            &prefix,
            |_, _| -> Result<(Node, Option<ScalarType>), PrivateError> {
                Err(PrivateError("unknown native scalar"))
            },
        )
        .map_err(|_| PrivateError("scalar lexical/type rejected"))?;
        Ok((value, self.observed(ty, false)))
    }
    fn boolean(&mut self, _: &Node, ty: &Observation) -> Result<bool, PrivateError> {
        self.events.push("boolean".into());
        self.step(ControlSourcePhase::Condition)?;
        Ok(!self.false_boolean && ty.ty == ScalarType::Boolean)
    }
    fn same(&mut self, left: &Observation, right: &Observation) -> Result<bool, PrivateError> {
        self.events.push("same".into());
        self.step(ControlSourcePhase::Compare)?;
        Ok(!self.false_same && left.ty == right.ty)
    }
    fn admit(
        &mut self,
        _: &'source Expression,
        _: &Node,
        _: &Observation,
        scope: ControlSourceScope<'_, 'source, Observation>,
    ) -> Result<(), PrivateError> {
        self.events.push("admit".into());
        self.step(ControlSourcePhase::Admit)?;
        assert!(scope.is_empty() || scope.get("external").is_some());
        self.admitted += 1;
        Ok(())
    }
}

#[test]
fn recursive_bind_choose_borrows_original_scope_metadata_and_moves_native_buffers() {
    let input = source(
        "bind(body: choose(otherwise: native.observe(), then: native.observe(), when: true), ticket: native.take())",
    );
    let mut adapter = Adapter::new();
    let (value, ty) = lower_control_source(&input, &[], LIMITS, &mut adapter).unwrap();
    assert_eq!(
        adapter.events,
        [
            "native.take",
            "scalar",
            "boolean",
            "native.observe",
            "native.observe",
            "same",
            "admit"
        ]
    );
    assert_eq!(adapter.admitted, 1);
    assert_eq!(adapter.seen_types.len(), 2);
    for (left, right) in &adapter.seen_types {
        assert_eq!(left, right);
    }
    assert_eq!(adapter.seen_types[0].0, adapter.seen_types[1].0);
    let Expression::Call { arguments, .. } = &input else {
        panic!()
    };
    assert_eq!(adapter.original[0], &arguments[1].value as *const _);
    let Node::Bind {
        value: original,
        body,
        ..
    } = &value
    else {
        panic!()
    };
    let Node::Host { effect } = original.as_ref() else {
        panic!()
    };
    assert_eq!(effect.buffer.as_ptr(), adapter.produced[0]);
    assert!(matches!(body.as_ref(), Node::Choose { .. }));
    assert_eq!(adapter.native_drops.get(), 0);
    assert_eq!(adapter.type_drops.get(), 3);
    assert_eq!(ty.ty, ScalarType::Integer);
    drop((value, ty));
    assert_eq!(adapter.native_drops.get(), 1);
    assert_eq!(adapter.type_drops.get(), 4);
}

#[test]
fn whole_cold_signatures_and_lexical_errors_stop_every_native_hook() {
    for text in [
        "bind(x: native.take(), body: bind(x: 1, body: x))",
        "choose(when: true, then: native.take(), otherwise: choose(when: true, then: 1))",
        "choose(when: true, then: native.take(), otherwise: add(left: 1))",
        "choose(when: true, then: native.take(), otherwise: field(value: item, name: 1))",
        "choose(when: true, then: native.take(), otherwise: member(value: native.take(), name: \"ready-step\"))",
        "choose(when: true, then: native.take(), otherwise: loop(x: 0, while: false, next: x, limit: 1025))",
        "choose(when: true, then: native.take(), otherwise: fold(x: 0, items: strings(), item: \"x\", next: 1, limit: 1))",
    ] {
        let mut adapter = Adapter::new();
        assert!(lower_control_source(&source(text), &[], LIMITS, &mut adapter).is_err());
        assert!(adapter.events.is_empty(), "{text}");
    }
}

#[test]
fn prefix_names_and_active_quota_are_checked_before_native_work() {
    let mut adapter = Adapter::new();
    let external = adapter.observed(ScalarType::Integer, false);
    for prefix in [
        vec![("bad-name", &external)],
        vec![("external", &external), ("external", &external)],
    ] {
        assert!(
            lower_control_source(&source("native.take()"), &prefix, LIMITS, &mut adapter).is_err()
        );
        assert!(adapter.events.is_empty());
    }
    let limits = ControlSourceLimits {
        max_bindings: 1,
        ..LIMITS
    };
    assert!(
        lower_control_source(
            &source("bind(x: native.take(), body: x)"),
            &[("external", &external)],
            limits,
            &mut adapter
        )
        .is_err()
    );
    assert!(adapter.events.is_empty());
    let zero = ControlSourceLimits {
        max_bindings: 0,
        ..LIMITS
    };
    assert!(lower_control_source(&source("bind(x: 1, body: x)"), &[], zero, &mut adapter).is_err());
    assert!(adapter.events.is_empty());
}

#[test]
fn initializer_sees_parent_and_sibling_scopes_do_not_leak() {
    let input = source(
        "choose(when: true, then: bind(ticket: 1, body: native.observe()), otherwise: bind(ticket: 2, body: native.observe()))",
    );
    let mut adapter = Adapter::new();
    let (value, ty) = lower_control_source(
        &input,
        &[],
        ControlSourceLimits {
            max_bindings: 1,
            ..LIMITS
        },
        &mut adapter,
    )
    .unwrap();
    assert_eq!(adapter.seen_types.len(), 2);
    drop((value, ty));
    let mut adapter = Adapter::new();
    assert!(
        lower_control_source(
            &source("bind(ticket: native.observe(), body: ticket)"),
            &[],
            LIMITS,
            &mut adapter
        )
        .is_err()
    );
    assert_eq!(adapter.events, ["native.observe"]);
}

#[test]
fn cold_physical_limits_and_constructor_minimums_precede_callbacks() {
    for limits in [
        ControlSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        ControlSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        ControlSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 3,
                ..LIMITS.source
            },
            ..LIMITS
        },
        ControlSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        ControlSourceLimits {
            max_bindings: 1025,
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter::new();
        assert!(
            lower_control_source(
                &source("choose(when: true, then: native.take(), otherwise: native.take())"),
                &[],
                limits,
                &mut adapter
            )
            .is_err()
        );
        assert!(adapter.events.is_empty());
    }
}

#[test]
fn aggregate_leaf_expansion_retains_outer_future_roots_before_sibling_hooks() {
    let mut adapter = Adapter::new();
    adapter.expanded = true;
    let limits = ControlSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 5,
            ..LIMITS.source
        },
        ..LIMITS
    };
    let result = lower_control_source(
        &source("choose(when: true, then: native.take(), otherwise: native.take())"),
        &[],
        limits,
        &mut adapter,
    );
    assert!(matches!(
        result,
        Err(ControlSourceError::Output {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    assert_eq!(adapter.events, ["scalar", "boolean", "native.take"]);
    assert_eq!(adapter.native_drops.get(), 1);
    assert_eq!(adapter.admitted, 0);
}

#[test]
fn shifted_native_leaf_depth_is_not_reset_at_each_control_boundary() {
    let mut adapter = Adapter::new();
    adapter.expanded = true;
    let limits = ControlSourceLimits {
        source: SourceCallLimits {
            max_lowered_depth: 1,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_control_source(
            &source("bind(x: native.take(), body: x)"),
            &[],
            limits,
            &mut adapter
        ),
        Err(ControlSourceError::Output {
            error: StructureError::DepthLimit,
            ..
        })
    ));
    assert_eq!(adapter.events, ["native.take"]);
    assert_eq!(adapter.native_drops.get(), 1);
}

#[test]
fn independent_condition_purity_and_literal_kind_cannot_be_overridden() {
    for condition in ["native.badcondition()", "native.wrongliteral()"] {
        let mut adapter = Adapter::new();
        assert!(
            lower_control_source(
                &source(&format!("choose(when: {condition}, then: 1, otherwise: 2)")),
                &[],
                LIMITS,
                &mut adapter
            )
            .is_err()
        );
        assert_eq!(adapter.events, [condition.trim_end_matches("()")]);
        assert_eq!(adapter.admitted, 0);
    }
    let mut adapter = Adapter::new();
    adapter.false_boolean = true;
    assert!(matches!(
        lower_control_source(
            &source("choose(when: true, then: 1, otherwise: 2)"),
            &[],
            LIMITS,
            &mut adapter
        ),
        Err(ControlSourceError::Condition { .. })
    ));
    assert_eq!(adapter.events, ["scalar", "boolean"]);
}

#[test]
fn closed_native_branch_comparison_happens_after_both_cold_outputs() {
    let mut adapter = Adapter::new();
    adapter.false_same = true;
    assert!(matches!(
        lower_control_source(
            &source("choose(when: true, then: native.take(), otherwise: native.take())"),
            &[],
            LIMITS,
            &mut adapter
        ),
        Err(ControlSourceError::BranchTypes { .. })
    ));
    assert_eq!(
        adapter.events,
        ["scalar", "boolean", "native.take", "native.take", "same"]
    );
    assert_eq!(adapter.native_drops.get(), 2);
    assert_eq!(adapter.type_drops.get(), 3);
}

#[test]
fn every_native_failure_or_unwind_releases_owned_parts_without_admission_or_retry() {
    let input = source("choose(when: true, then: native.take(), otherwise: native.take())");
    for phase in [
        ControlSourcePhase::Leaf,
        ControlSourcePhase::Condition,
        ControlSourcePhase::Compare,
        ControlSourcePhase::Admit,
    ] {
        for panic in [false, true] {
            let mut adapter = Adapter::new();
            if panic {
                adapter.unwind = Some(phase);
            } else {
                adapter.fail = Some(phase);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                lower_control_source(&input, &[], LIMITS, &mut adapter)
            }));
            if panic {
                assert!(result.is_err());
            } else {
                let Err(ControlSourceError::Native {
                    phase: actual,
                    error: PrivateError(payload),
                    ..
                }) = result.unwrap()
                else {
                    panic!()
                };
                assert_eq!(actual, phase);
                assert_eq!(payload, "private credential");
            }
            assert_eq!(adapter.admitted, 0);
            let produced = adapter.produced.len();
            assert_eq!(adapter.native_drops.get(), produced);
            assert!(
                adapter
                    .events
                    .iter()
                    .filter(|event| event.as_str() == "admit")
                    .count()
                    <= 1
            );
        }
    }
}

#[test]
fn native_observation_cleanup_unwind_cannot_publish_a_complete_tree() {
    let mut adapter = Adapter::new();
    adapter.cleanup_panic = true;
    let input = source("bind(ticket: native.take(), body: ticket)");
    assert!(
        catch_unwind(AssertUnwindSafe(|| lower_control_source(
            &input,
            &[],
            LIMITS,
            &mut adapter
        )))
        .is_err()
    );
    assert_eq!(adapter.admitted, 0);
    assert_eq!(adapter.native_drops.get(), 1);
    assert_eq!(adapter.type_drops.get(), 2);
}

#[test]
fn initial_prefix_borrows_the_original_native_type_without_copying_or_consuming_it() {
    let mut adapter = Adapter::new();
    let external = adapter.observed(ScalarType::Integer, false);
    let ptr = external.token.buffer.as_ptr();
    let (value, ty) = lower_control_source(
        &source("add(left: external, right: 1)"),
        &[("external", &external)],
        LIMITS,
        &mut adapter,
    )
    .unwrap();
    assert_eq!(adapter.admitted, 1);
    assert_eq!(external.token.buffer.as_ptr(), ptr);
    assert_eq!(adapter.type_drops.get(), 0);
    drop((value, ty));
    assert_eq!(adapter.type_drops.get(), 1);
    drop(external);
    assert_eq!(adapter.type_drops.get(), 2);
}

#[test]
fn machine_matchable_native_errors_never_format_private_payloads() {
    let mut adapter = Adapter::new();
    adapter.fail = Some(ControlSourcePhase::Admit);
    let error = lower_control_source(&source("native.take()"), &[], LIMITS, &mut adapter)
        .err()
        .unwrap();
    assert!(!format!("{error:?} {error}").contains("credential"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(adapter.native_drops.get(), 1);
}
