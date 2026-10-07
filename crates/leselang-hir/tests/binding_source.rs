use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::binding_source::{
    BindingSourceAdapter, BindingSourceError, BindingSourceForm, BindingSourceLimits,
    BindingSourcePhase, lower_binding_source,
};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::scalar_source::ScalarSourceError;
use leselang_hir::source_call::{SourceCallError, SourceCallLimits};
use leselang_runtime_core::{ScalarValue, ScopeFrame, StructureError};
use leselang_syntax::{Expression, NamedArgument, parse};

struct Token {
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
const LIMITS: BindingSourceLimits = BindingSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_bindings: 8,
};

fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn literal(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn choice(value: Node) -> Node {
    Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(value),
        otherwise: Box::new(literal(1)),
    }
}

struct Adapter<'source> {
    input: &'source Expression,
    events: Vec<BindingSourcePhase>,
    drops: Rc<Cell<usize>>,
    fail: Option<BindingSourcePhase>,
    unwind: Option<BindingSourcePhase>,
    value: Option<Node>,
    body: Option<Node>,
    rewrite: bool,
    scope: Vec<(&'source str, u32)>,
    metadata_buffer: *const u8,
    result_buffer: *const u8,
    value_buffer: *const u8,
}
impl<'source> Adapter<'source> {
    fn new(input: &'source Expression) -> Self {
        Self {
            input,
            events: Vec::new(),
            drops: Rc::new(Cell::new(0)),
            fail: None,
            unwind: None,
            value: None,
            body: None,
            rewrite: false,
            scope: Vec::new(),
            metadata_buffer: std::ptr::null(),
            result_buffer: std::ptr::null(),
            value_buffer: std::ptr::null(),
        }
    }
    fn token(&self) -> Token {
        Token {
            bytes: vec![7; 33],
            drops: self.drops.clone(),
        }
    }
    fn step(&mut self, phase: BindingSourcePhase) -> Result<(), PrivateError> {
        self.events.push(phase);
        if self.unwind == Some(phase) {
            panic!("private native unwind");
        }
        if self.fail == Some(phase) {
            return Err(PrivateError("secret credential"));
        }
        Ok(())
    }
}
impl<'source> BindingSourceAdapter<'source, Token, Token, Token, Token> for Adapter<'source> {
    type Value = Token;
    type Result = Token;
    type Error = PrivateError;

    fn lower_value(
        &mut self,
        source: &'source NamedArgument,
    ) -> Result<(Node, Token), PrivateError> {
        self.step(BindingSourcePhase::Value)?;
        assert!(
            self.scope.is_empty(),
            "initializer must not see its new local"
        );
        let Expression::Call { arguments, .. } = self.input else {
            panic!()
        };
        let original = arguments.iter().find(|item| item.name != "body").unwrap();
        assert!(std::ptr::eq(source, original));
        assert!(std::ptr::eq(&source.value, &original.value));
        let value = self.value.take().unwrap_or_else(|| Node::Host {
            effect: Box::new(self.token()),
        });
        if let Node::Host { effect } = &value {
            self.value_buffer = effect.bytes.as_ptr();
        }
        let metadata = self.token();
        self.metadata_buffer = metadata.bytes.as_ptr();
        Ok((value, metadata))
    }

    fn lower_body(
        &mut self,
        source: &BindingSourceForm<'source>,
        value: &Node,
        metadata: &mut Token,
    ) -> Result<(Node, Token), PrivateError> {
        self.events.push(BindingSourcePhase::Body);
        assert_eq!(metadata.bytes.as_ptr(), self.metadata_buffer);
        if let Node::Host { effect } = value {
            assert_eq!(effect.bytes.as_ptr(), self.value_buffer);
        }
        let Expression::Call {
            arguments, span, ..
        } = self.input
        else {
            panic!()
        };
        let original = arguments.iter().find(|item| item.name == "body").unwrap();
        assert!(std::ptr::eq(source.body(), &original.value));
        assert_eq!(source.span(), *span);
        assert!(!format!("{source:?}").contains("ticket"));
        let drops = self.drops.clone();
        let mut scope = ScopeFrame::new(&mut self.scope);
        let mut local = scope.nested();
        local.push(&source.binding().name, 17).unwrap();
        assert_eq!(local.get(&source.binding().name), Some(&17));
        if self.unwind == Some(BindingSourcePhase::Body) {
            panic!("private body unwind");
        }
        if self.fail == Some(BindingSourcePhase::Body) {
            return Err(PrivateError("secret credential"));
        }
        let body = self.body.take().unwrap_or_else(|| literal(41));
        let result = Token {
            bytes: vec![9; 35],
            drops,
        };
        self.result_buffer = result.bytes.as_ptr();
        Ok((body, result))
    }

    fn finish(
        &mut self,
        source: &BindingSourceForm<'source>,
        value: Node,
        metadata: Token,
        body: Node,
        result: Token,
    ) -> Result<(Node, Token), PrivateError> {
        self.step(BindingSourcePhase::Finish)?;
        assert!(self.scope.is_empty());
        assert_eq!(metadata.bytes.as_ptr(), self.metadata_buffer);
        assert_eq!(result.bytes.as_ptr(), self.result_buffer);
        let output = source.construct(value, body);
        Ok((if self.rewrite { choice(output) } else { output }, result))
    }
}

fn source() -> Expression {
    expression("bind(ticket: device.read(), body: ticket)")
}
fn no_hooks(input: &Expression, prefix: &[&str], limits: BindingSourceLimits) {
    let mut adapter = Adapter::new(input);
    assert!(lower_binding_source(input, prefix, limits, &mut adapter).is_err());
    assert!(adapter.events.is_empty());
    assert!(adapter.scope.is_empty());
    assert_eq!(adapter.drops.get(), 0);
}

#[test]
fn original_source_metadata_native_buffers_and_phase_order_are_preserved() {
    for text in [
        "bind(ticket: device.read(), body: ticket)",
        "bind(body: ticket, ticket: device.read())",
    ] {
        let input = expression(text);
        let mut adapter = Adapter::new(&input);
        let (output, result) = lower_binding_source(&input, &[], LIMITS, &mut adapter).unwrap();
        assert_eq!(
            adapter.events,
            [
                BindingSourcePhase::Value,
                BindingSourcePhase::Body,
                BindingSourcePhase::Finish
            ]
        );
        let Node::Bind { name, value, body } = &output else {
            panic!()
        };
        assert_eq!(name, "ticket");
        assert!(matches!(body.as_ref(), Node::Literal { .. }));
        let Node::Host { effect } = value.as_ref() else {
            panic!()
        };
        assert_eq!(effect.bytes.as_ptr(), adapter.value_buffer);
        assert_eq!(result.bytes.as_ptr(), adapter.result_buffer);
        assert_eq!(adapter.drops.get(), 1);
        drop((output, result));
        assert_eq!(adapter.drops.get(), 3);
    }
}

#[test]
fn all_ceiling_violations_precede_native_hooks() {
    for limits in [
        BindingSourceLimits {
            max_bindings: 1_025,
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 16_385,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 16_385,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_arguments: 65,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        no_hooks(&source(), &[], limits);
    }
}

#[test]
fn zero_source_output_and_lexical_quotas_deny_before_hooks() {
    for limits in [
        BindingSourceLimits {
            max_bindings: 0,
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 2,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        no_hooks(&source(), &[], limits);
    }
}

#[test]
fn exact_three_node_one_depth_output_and_active_binding_limits_are_inclusive() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    let limits = BindingSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 3,
            max_lowered_depth: 1,
            ..LIMITS.source
        },
        max_bindings: 1,
    };
    assert!(lower_binding_source(&input, &[], limits, &mut adapter).is_ok());
    assert_eq!(adapter.drops.get(), 3);
    no_hooks(&input, &["ambient"], limits);
}

#[test]
fn invalid_duplicate_overlong_shadowed_and_overquota_prefixes_precede_hooks() {
    for prefix in [
        vec!["ambient", "ambient"],
        vec!["bad.name"],
        vec![""],
        vec!["ticket"],
        vec!["body"],
    ] {
        no_hooks(&source(), &prefix, LIMITS);
    }
    no_hooks(&source(), &[&"x".repeat(65)], LIMITS);
    no_hooks(
        &source(),
        &["a", "b"],
        BindingSourceLimits {
            max_bindings: 1,
            ..LIMITS
        },
    );
}

#[test]
fn wrong_root_missing_extra_duplicate_and_invalid_binding_operands_precede_hooks() {
    for text in [
        "device.read()",
        "1",
        "bind(ticket: 1)",
        "bind(body: 1)",
        "bind(ticket: 1, body: ticket, extra: 2)",
        "bind(body: 1, body: 2)",
    ] {
        no_hooks(&expression(text), &[], LIMITS);
    }
    let mut input = source();
    let Expression::Call { arguments, .. } = &mut input else {
        panic!()
    };
    arguments[0].name = "bad.name".into();
    no_hooks(&input, &[], LIMITS);
}

#[test]
fn all_cold_literal_call_name_and_argument_bounds_precede_hooks() {
    let at = leselang_syntax::Span { start: 0, end: 0 };
    for body in [
        Expression::String {
            value: "x".repeat(4097),
            span: at,
        },
        Expression::Call {
            callee: "bad..name".into(),
            arguments: vec![],
            span: at,
        },
        Expression::Call {
            callee: "device.read".into(),
            span: at,
            arguments: (0..65)
                .map(|index| NamedArgument {
                    name: format!("a{index}"),
                    value: Expression::Integer { value: 1, span: at },
                    span: at,
                })
                .collect(),
        },
    ] {
        let mut input = source();
        let Expression::Call { arguments, .. } = &mut input else {
            panic!()
        };
        arguments[1].value = body;
        no_hooks(&input, &[], LIMITS);
    }
}

#[test]
fn cold_binding_loop_and_fold_conflicts_are_checked_before_initialization() {
    for body in [
        "bind(ticket: 1, body: ticket)",
        "loop(ticket: 1, while: false, next: ticket, limit: 0)",
        "fold(total: 0, items: strings(a: \"x\"), item: \"ticket\", next: total, limit: 0)",
        "loop(next_state: 1, while: false, next: next_state, limit: 1025)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: 65)",
    ] {
        no_hooks(
            &expression(&format!(
                "bind(ticket: device.read(), body: choose(when: true, then: ticket, otherwise: {body}))"
            )),
            &[],
            LIMITS,
        );
    }
}

#[test]
fn sibling_local_scopes_do_not_leak_or_accumulate_binding_quota() {
    let input = expression(
        "bind(ticket: device.read(), body: choose(when: true, then: bind(next_state: 1, body: next_state), otherwise: bind(next_state: 2, body: next_state)))",
    );
    let mut adapter = Adapter::new(&input);
    assert!(
        lower_binding_source(
            &input,
            &[],
            BindingSourceLimits {
                max_bindings: 2,
                ..LIMITS
            },
            &mut adapter
        )
        .is_ok()
    );
    assert!(adapter.scope.is_empty());
}

#[test]
fn generated_value_budget_reserves_body_and_stops_body_hook() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    adapter.value = Some(choice(literal(1)));
    let limits = BindingSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 5,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_binding_source(&input, &[], limits, &mut adapter),
        Err(BindingSourceError::Output {
            phase: BindingSourcePhase::Value,
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!(adapter.events, [BindingSourcePhase::Value]);
    assert_eq!(adapter.drops.get(), 1);
}

#[test]
fn shifted_generated_value_depth_is_checked_before_body_hook() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    adapter.value = Some(choice(literal(1)));
    let limits = BindingSourceLimits {
        source: SourceCallLimits {
            max_lowered_depth: 1,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_binding_source(&input, &[], limits, &mut adapter),
        Err(BindingSourceError::Output {
            phase: BindingSourcePhase::Value,
            error: StructureError::DepthLimit
        })
    ));
    assert_eq!(adapter.events, [BindingSourcePhase::Value]);
}

#[test]
fn generated_body_uses_same_aggregate_budget_before_finish_hook() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    adapter.body = Some(choice(literal(1)));
    let limits = BindingSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 5,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_binding_source(&input, &[], limits, &mut adapter),
        Err(BindingSourceError::Output {
            phase: BindingSourcePhase::Body,
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!(
        adapter.events,
        [BindingSourcePhase::Value, BindingSourcePhase::Body]
    );
    assert_eq!(adapter.drops.get(), 3);
    assert!(adapter.scope.is_empty());
}

#[test]
fn rewritten_finish_output_is_rechecked_without_partial_result() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    adapter.rewrite = true;
    let limits = BindingSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 3,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_binding_source(&input, &[], limits, &mut adapter),
        Err(BindingSourceError::Output {
            phase: BindingSourcePhase::Finish,
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!(adapter.events.len(), 3);
    assert_eq!(adapter.drops.get(), 3);
}

#[test]
fn shifted_body_and_rewritten_output_depth_are_checked_at_their_own_phases() {
    for phase in [BindingSourcePhase::Body, BindingSourcePhase::Finish] {
        let input = source();
        let mut adapter = Adapter::new(&input);
        if phase == BindingSourcePhase::Body {
            adapter.body = Some(choice(literal(1)));
        } else {
            adapter.rewrite = true;
        }
        let limits = BindingSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        };
        assert!(matches!(
            lower_binding_source(&input, &[], limits, &mut adapter),
            Err(BindingSourceError::Output {
                phase: actual, error: StructureError::DepthLimit,
            }) if actual == phase
        ));
        assert_eq!(
            adapter.events.len(),
            if phase == BindingSourcePhase::Body {
                2
            } else {
                3
            }
        );
        assert_eq!(adapter.drops.get(), 3);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn whole_cold_source_nodes_and_depth_fail_before_value_lowering() {
    let input = expression(
        "bind(ticket: device.read(), body: choose(when: true, then: ticket, otherwise: add(left: 1, right: 2)))",
    );
    for limits in [
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 3,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BindingSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 2,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        no_hooks(&input, &[], limits);
    }
}

#[test]
fn native_failure_keeps_original_error_phase_cleanup_and_no_retry() {
    for (phase, visits, drops) in [
        (BindingSourcePhase::Value, 1, 0),
        (BindingSourcePhase::Body, 2, 2),
        (BindingSourcePhase::Finish, 3, 3),
    ] {
        let input = source();
        let mut adapter = Adapter::new(&input);
        adapter.fail = Some(phase);
        let error = lower_binding_source(&input, &[], LIMITS, &mut adapter)
            .err()
            .unwrap();
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert!(std::error::Error::source(&error).is_none());
        assert!(
            matches!(error, BindingSourceError::Native { phase: actual, error: PrivateError("secret credential") } if actual == phase)
        );
        assert_eq!(adapter.events.len(), visits);
        assert_eq!(adapter.drops.get(), drops);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn native_unwind_releases_owned_parts_and_guarded_frames_once() {
    for (phase, visits, drops) in [
        (BindingSourcePhase::Value, 1, 0),
        (BindingSourcePhase::Body, 2, 2),
        (BindingSourcePhase::Finish, 3, 3),
    ] {
        let input = source();
        let mut adapter = Adapter::new(&input);
        adapter.unwind = Some(phase);
        assert!(
            catch_unwind(AssertUnwindSafe(|| lower_binding_source(
                &input,
                &[],
                LIMITS,
                &mut adapter
            )))
            .is_err()
        );
        assert_eq!(adapter.events.len(), visits);
        assert_eq!(adapter.drops.get(), drops);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn all_four_move_only_ir_slots_child_boxes_and_vectors_move_without_clone() {
    let input = source();
    let mut adapter = Adapter::new(&input);
    let field = adapter.token();
    let field_ptr = field.bytes.as_ptr();
    let operation = adapter.token();
    let operation_ptr = operation.bytes.as_ptr();
    let declaration = adapter.token();
    let declaration_ptr = declaration.bytes.as_ptr();
    let effect = Box::new(adapter.token());
    let effect_ptr = effect.bytes.as_ptr();
    let arguments = vec![ComputedArgument {
        name: "value".into(),
        value: Node::Field {
            value: Box::new(literal(1)),
            field,
        },
    }];
    let arguments_ptr = arguments.as_ptr();
    adapter.value = Some(Node::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![
            ComputedBranch {
                name: "first".into(),
                value: Node::Call {
                    operation,
                    arguments,
                },
                result_type: declaration,
            },
            ComputedBranch {
                name: "second".into(),
                value: Node::Host { effect },
                result_type: adapter.token(),
            },
        ],
    });
    let (output, result) = lower_binding_source(&input, &[], LIMITS, &mut adapter).unwrap();
    let Node::Bind { value, .. } = &output else {
        panic!()
    };
    let Node::Group { branches, .. } = value.as_ref() else {
        panic!()
    };
    assert_eq!(branches[0].result_type.bytes.as_ptr(), declaration_ptr);
    let Node::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    assert_eq!(operation.bytes.as_ptr(), operation_ptr);
    assert_eq!(arguments.as_ptr(), arguments_ptr);
    let Node::Field { field, .. } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_ptr);
    let Node::Host { effect } = &branches[1].value else {
        panic!()
    };
    assert_eq!(effect.bytes.as_ptr(), effect_ptr);
    drop((output, result));
    assert_eq!(adapter.drops.get(), 7);
}

#[test]
fn malformed_source_error_is_redacted_and_never_grants_a_native_default() {
    let input = expression("bind(body: 1)");
    let mut adapter = Adapter::new(&input);
    let error = lower_binding_source(&input, &[], LIMITS, &mut adapter)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        BindingSourceError::Source(ScalarSourceError::BindingShape { .. })
    ));
    assert!(!format!("{error:?}").contains("body: 1"));
    assert!(std::error::Error::source(&error).is_none());
    let bad_limits = BindingSourceLimits {
        max_bindings: 1_025,
        ..LIMITS
    };
    assert!(matches!(
        lower_binding_source(&input, &[], bad_limits, &mut adapter),
        Err(BindingSourceError::Source(ScalarSourceError::Source(
            SourceCallError::InvalidLimits
        )))
    ));
}

mod counter {
    use super::*;
    use leselang_hir::call_evaluation::{
        CallEvaluationHost, CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
        prepare_call_in_scope,
    };
    use leselang_hir::call_typing::{CallTypeHost, CallTypeLimits};
    use leselang_hir::control_source::*;
    use leselang_hir::effect_evaluation::*;
    use leselang_hir::effect_reentry::*;
    use leselang_hir::effect_session::*;
    use leselang_hir::flow_typing::{
        CallFlowEnvironment, CallFlowTypeLimits, infer_call_flow_type,
    };
    use leselang_hir::pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
        scalar_copy_cost,
    };
    use leselang_hir::pure_typing::{PureType, PureTypeEnvironment, TypeInferenceLimits};
    use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
    use leselang_hir::source_call::{SourceCallHost, SourceSchema, lower_source_call};
    use leselang_runtime_core::*;
    use std::borrow::Borrow;
    use std::cell::RefCell;

    struct ReplyDeclaration {
        maximum: u64,
        checks: Rc<Cell<usize>>,
    }
    impl HostResultDomain<ScalarValue> for ReplyDeclaration {
        type Error = PrivateError;
        fn matches_type(&self, reply: &ScalarValue) -> bool {
            matches!(reply, ScalarValue::Integer(_))
        }
        fn validate_value(&self, reply: &ScalarValue) -> Result<(), PrivateError> {
            self.checks.set(self.checks.get() + 1);
            match reply {
                ScalarValue::Integer(value) if *value <= self.maximum => Ok(()),
                _ => Err(PrivateError("private counter domain")),
            }
        }
    }
    type Counter = Computation<(), &'static str, (), ()>;
    type Schema<'schema> = SourceSchema<'schema, &'static str, ScalarTypeSet, ReplyDeclaration, u8>;
    struct Compiler<'host, 'schema> {
        host: SourceCallHost<'host, 'schema, &'static str, ScalarTypeSet, ReplyDeclaration, u8>,
        events: Vec<BindingSourcePhase>,
    }
    impl<'schema> Compiler<'_, 'schema> {
        fn call(
            &self,
            source: &Expression,
            prefix: &[(&str, ScalarType)],
        ) -> Result<(Counter, &'schema Schema<'schema>), PrivateError> {
            let lowered = lower_source_call(source, &self.host, LIMITS.source, |argument| {
                lower_scalar_source_with_scope(
                    &argument.value,
                    ScalarSourceLimits {
                        source: LIMITS.source,
                        max_bindings: LIMITS.max_bindings,
                    },
                    prefix,
                    |_, _| -> Result<(Counter, Option<ScalarType>), PrivateError> {
                        Err(PrivateError("unknown native scalar"))
                    },
                )
                .map(|(value, ty)| (value, Some(ty)))
            })
            .map_err(|_| PrivateError("native catalog or lexical operand rejected"))?;
            let schema = lowered.schema();
            Ok((
                Counter::Call {
                    operation: schema.key,
                    arguments: lowered.into_arguments(),
                },
                schema,
            ))
        }
    }
    impl<'source, 'schema> BindingSourceAdapter<'source, (), &'static str, (), ()>
        for Compiler<'_, 'schema>
    {
        type Value = &'schema Schema<'schema>;
        type Result = &'schema Schema<'schema>;
        type Error = PrivateError;
        fn lower_value(
            &mut self,
            source: &'source NamedArgument,
        ) -> Result<(Counter, Self::Value), PrivateError> {
            self.events.push(BindingSourcePhase::Value);
            self.call(&source.value, &[])
        }
        fn lower_body(
            &mut self,
            source: &BindingSourceForm<'source>,
            value: &Counter,
            metadata: &mut Self::Value,
        ) -> Result<(Counter, Self::Result), PrivateError> {
            self.events.push(BindingSourcePhase::Body);
            let Counter::Call { operation, .. } = value else {
                return Err(PrivateError("not atomic"));
            };
            let original = self
                .host
                .catalog
                .authorize(operation, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("live catalog rejected"))?;
            assert!(std::ptr::eq(original, *metadata));
            self.call(
                source.body(),
                &[(source.binding().name.as_str(), ScalarType::Integer)],
            )
        }
        fn finish(
            &mut self,
            source: &BindingSourceForm<'source>,
            value: Counter,
            metadata: Self::Value,
            body: Counter,
            result: Self::Result,
        ) -> Result<(Counter, Self::Result), PrivateError> {
            self.events.push(BindingSourcePhase::Finish);
            for (node, declaration) in [(&value, metadata), (&body, result)] {
                let Counter::Call { operation, .. } = node else {
                    return Err(PrivateError("not atomic"));
                };
                let original = self
                    .host
                    .catalog
                    .authorize(operation, self.host.version, self.host.granted)
                    .map_err(|_| PrivateError("live catalog rejected"))?;
                if !std::ptr::eq(original, declaration) {
                    return Err(PrivateError("wrong declaration"));
                }
            }
            Ok((source.construct(value, body), result))
        }
    }
    struct Environment;
    impl PureTypeEnvironment<(), &'static str> for Environment {
        type Result = ();
        fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &(), _: &str, _: &&'static str) -> Option<()> {
            None
        }
    }
    impl<'schema> CallFlowEnvironment<'schema, (), &'static str, ReplyDeclaration> for Environment {
        fn call_result_type(
            &self,
            operation: &&'static str,
            declaration: &'schema ReplyDeclaration,
        ) -> Option<PureType<()>> {
            (matches!(*operation, "counter.read" | "counter.write") && declaration.maximum >= 100)
                .then_some(PureType::Scalar(ScalarType::Integer))
        }
    }
    impl<'source> ControlSourceAdapter<'source, (), &'static str, (), ()> for Compiler<'_, '_> {
        type Type = ScalarType;
        type Error = PrivateError;
        fn lower_leaf(
            &mut self,
            source: &'source Expression,
            scope: ControlSourceScope<'_, 'source, ScalarType>,
        ) -> Result<(Counter, ScalarType), PrivateError> {
            let prefix = scope
                .bindings()
                .iter()
                .map(|(name, ty)| (*name, **ty))
                .collect::<Vec<_>>();
            if matches!(source, Expression::Call { callee, .. } if callee.starts_with("counter.")) {
                let (value, _) = self.call(source, &prefix)?;
                Ok((value, ScalarType::Integer))
            } else {
                lower_scalar_source_with_scope(
                    source,
                    ScalarSourceLimits {
                        source: LIMITS.source,
                        max_bindings: LIMITS.max_bindings,
                    },
                    &prefix,
                    |_, _| -> Result<(Counter, Option<ScalarType>), PrivateError> {
                        Err(PrivateError("unknown native scalar"))
                    },
                )
                .map_err(|_| PrivateError("native scalar source rejected"))
            }
        }
        fn boolean(&mut self, _: &Counter, ty: &ScalarType) -> Result<bool, PrivateError> {
            Ok(*ty == ScalarType::Boolean)
        }
        fn same(&mut self, left: &ScalarType, right: &ScalarType) -> Result<bool, PrivateError> {
            Ok(left == right)
        }
        fn admit(
            &mut self,
            _: &'source Expression,
            value: &Counter,
            ty: &ScalarType,
            scope: ControlSourceScope<'_, 'source, ScalarType>,
        ) -> Result<(), PrivateError> {
            let schemas = CallTypeHost {
                catalog: self.host.catalog,
                version: self.host.version,
                granted: self.host.granted,
                environment: &Environment,
            };
            let prefix = scope
                .bindings()
                .iter()
                .map(|(name, ty)| (*name, PureType::Scalar(**ty)))
                .collect::<Vec<_>>();
            let inferred = infer_call_flow_type(
                value,
                &prefix,
                &Environment,
                &schemas,
                CallFlowTypeLimits {
                    call: CallTypeLimits {
                        pure: TypeInferenceLimits {
                            max_nodes: 256,
                            max_depth: 32,
                            max_bindings: 8,
                        },
                        max_arguments: 64,
                    },
                    max_calls: 64,
                },
            )
            .map_err(|_| PrivateError("whole native cold flow typing rejected"))?;
            let mut pending = vec![value];
            while let Some(node) = pending.pop() {
                if let Counter::Bind { value, .. } = node {
                    let mut initial = vec![value.as_ref()];
                    while let Some(child) = initial.pop() {
                        if matches!(child, Counter::Bind { value, .. } if !value.is_pure()) {
                            return Err(PrivateError("unfinished nested binding capture"));
                        }
                        initial.extend(child.children());
                    }
                }
                pending.extend(node.children());
            }
            if inferred != PureType::Scalar(*ty) {
                return Err(PrivateError("inconsistent whole result observation"));
            }
            Ok(())
        }
    }
    impl PureEvaluationEnvironment<(), &'static str> for Environment {
        type Result = ();
        type Error = PrivateError;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, PrivateError> {
            Err(PrivateError("no fields"))
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), PrivateError> {
            Err(PrivateError("no members"))
        }
    }

    struct NativeReply(ScalarValue);
    impl Borrow<ScalarValue> for NativeReply {
        fn borrow(&self) -> &ScalarValue {
            &self.0
        }
    }
    struct Execution<'host, 'schema> {
        host: SourceCallHost<'host, 'schema, &'static str, ScalarTypeSet, ReplyDeclaration, u8>,
        events: RefCell<Vec<&'static str>>,
        actual: Cell<u64>,
    }
    impl PureEvaluationEnvironment<(), &'static str> for Execution<'_, '_> {
        type Result = ();
        type Error = PrivateError;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, PrivateError> {
            Err(PrivateError("no fields"))
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), PrivateError> {
            Err(PrivateError("no members"))
        }
    }
    const EXECUTION_LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
        },
        max_arguments: 64,
        max_branches: 64,
    };
    impl<'expression, 'schema> EffectEvaluationEnvironment<'expression, (), &'static str, (), ()>
        for Execution<'_, 'schema>
    {
        type Request = PreparedCall<'schema, &'static str, ScalarTypeSet, ReplyDeclaration, u8>;
        type Capture = RestoredEffectBindings<'expression, ()>;
        fn preflight_effect(&self, expression: &Counter) -> Result<(), PrivateError> {
            let Counter::Call {
                operation,
                arguments,
            } = expression
            else {
                return Err(PrivateError("unsupported effect"));
            };
            let schema = self
                .host
                .catalog
                .authorize(operation, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("cold catalog rejected"))?;
            let names = arguments
                .iter()
                .map(|arg| arg.name.as_str())
                .collect::<Vec<_>>();
            schema
                .bind_arguments(&names)
                .map_err(|_| PrivateError("cold arguments rejected"))?;
            Ok(())
        }
        fn prepare_effect(
            &self,
            expression: &'expression Counter,
            scope: &mut ScopeFrame<'_, 'expression, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Request, CalculationFailure<PrivateError>> {
            let Counter::Call {
                operation,
                arguments,
            } = expression
            else {
                return Err(PrivateError("unsupported effect").into());
            };
            self.events.borrow_mut().push(operation);
            let schema = self
                .host
                .catalog
                .authorize(operation, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("live catalog rejected"))?;
            evaluate_call_arguments_in_scope(
                arguments,
                schema,
                scope,
                self,
                fuel,
                CallEvaluationLimits {
                    pure: EXECUTION_LIMITS.pure,
                    max_arguments: 64,
                },
            )
            .map_err(|_| PrivateError("native argument evaluation failed").into())
        }
        fn capture(
            &self,
            _: &'expression str,
            _: &'expression Counter,
            scope: &ScopeFrame<'_, 'expression, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Capture, PrivateError> {
            let mut values = Vec::new();
            for (name, value) in scope.bindings() {
                let cost = match value {
                    PureValue::Scalar(value) => scalar_copy_cost(value),
                    _ => 0,
                };
                fuel.charge(1 + cost)
                    .map_err(|_| PrivateError("capture fuel exhausted"))?;
                values.push((*name, value.clone()));
            }
            Ok(values)
        }
    }
    impl ReplyAuthority<(u64, &'static str)> for Execution<'_, '_> {
        type Error = PrivateError;
        fn authorize(&self, identity: &(u64, &'static str)) -> Result<(), PrivateError> {
            if identity.0 != 17 {
                return Err(PrivateError("wrong execution owner"));
            }
            self.host
                .catalog
                .authorize(&identity.1, self.host.version, self.host.granted)
                .map(|_| ())
                .map_err(|_| PrivateError("live reply policy rejected"))
        }
    }
    impl<'expression>
        EffectReentryEnvironment<
            'expression,
            (),
            &'static str,
            (),
            (),
            (u64, &'static str),
            ReplyDeclaration,
            NativeReply,
        > for Execution<'_, '_>
    {
        fn restore_capture(
            &self,
            _: &(u64, &'static str),
            _: &ReplyDeclaration,
            capture: RestoredEffectBindings<'expression, ()>,
            _: &mut Fuel,
            _: EffectEvaluationLimits,
        ) -> Result<RestoredEffectBindings<'expression, ()>, CalculationFailure<PrivateError>>
        {
            Ok(capture)
        }
        fn bind_reply(
            &self,
            identity: &(u64, &'static str),
            declaration: &ReplyDeclaration,
            reply: NativeReply,
            _: &mut Fuel,
        ) -> Result<PureValue<()>, CalculationFailure<PrivateError>> {
            let original = self
                .host
                .catalog
                .authorize(&identity.1, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("live reentry policy rejected"))?;
            if !std::ptr::eq(declaration, &original.result) {
                return Err(PrivateError("changed result declaration").into());
            }
            Ok(PureValue::Scalar(reply.0))
        }
    }
    impl Execution<'_, '_> {
        fn invoke(
            &self,
            request: PreparedCall<'_, &'static str, ScalarTypeSet, ReplyDeclaration, u8>,
        ) -> NativeReply {
            self.host
                .catalog
                .authorize(&request.schema().key, self.host.version, self.host.granted)
                .unwrap();
            match request.schema().key {
                "counter.read" => NativeReply(ScalarValue::Integer(self.actual.get())),
                "counter.write" => {
                    let ScalarValue::Integer(value) = request.arguments()[0].value else {
                        panic!()
                    };
                    self.actual.set(value);
                    NativeReply(ScalarValue::Integer(self.actual.get()))
                }
                _ => panic!(),
            }
        }
    }

    impl<'expression, 'schema>
        EffectSessionEnvironment<'expression, 'schema, (), &'static str, (), ()>
        for Execution<'_, 'schema>
    {
        type Identity = (u64, &'static str);
        type Declaration = ReplyDeclaration;
        type Reply = NativeReply;
        type Dispatch = PreparedCall<'schema, &'static str, ScalarTypeSet, ReplyDeclaration, u8>;
        fn correlate_request(
            &self,
            request: Self::Dispatch,
            _: &mut Fuel,
        ) -> EffectCorrelation<
            'schema,
            Self::Identity,
            Self::Declaration,
            Self::Dispatch,
            PrivateError,
        > {
            let original = self
                .host
                .catalog
                .authorize(&request.schema().key, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("live native correlation policy"))?;
            if !std::ptr::eq(original, request.schema()) {
                return Err(PrivateError("changed native request schema").into());
            }
            Ok(CorrelatedEffectRequest {
                identity: (17, original.key),
                declaration: &original.result,
                dispatch: request,
            })
        }
    }

    #[test]
    fn parsed_binding_session_owns_reply_gated_reentry_final_output_and_cancellation() {
        let checks = Rc::new(Cell::new(0));
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &[],
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 200,
                    checks: checks.clone(),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let source = expression(
            "bind(ticket: counter.read(), body: counter.write(value: add(left: ticket, right: 1)))",
        );
        let mut compiler = Compiler {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: Vec::new(),
        };
        let (expression, declared) =
            lower_binding_source(&source, &[], LIMITS, &mut compiler).unwrap();
        let execution = Execution {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: RefCell::new(Vec::new()),
            actual: Cell::new(41),
        };
        let mut session =
            EffectSession::new(&expression, Vec::new(), Fuel::new(100), EXECUTION_LIMITS);
        let EffectSessionPoll::Request(read) = session.poll(&execution).unwrap() else {
            panic!()
        };
        assert!(std::ptr::eq(read.schema(), &schemas[0]));
        let reply = execution.invoke(read);
        let rejected = session
            .try_accept(&(18, "counter.read"), reply, &execution)
            .unwrap_err();
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        assert_eq!(*execution.events.borrow(), ["counter.read"]);
        assert!(matches!(
            session.poll(&execution).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        session
            .try_accept(&(17, "counter.read"), rejected.reply, &execution)
            .unwrap();
        assert_eq!(session.status(), EffectSessionStatus::Ready);
        assert_eq!(session.fuel_remaining(), 98);
        assert_eq!(*execution.events.borrow(), ["counter.read"]);
        let EffectSessionPoll::Request(write) = session.poll(&execution).unwrap() else {
            panic!()
        };
        assert!(std::ptr::eq(write.schema(), declared));
        assert_eq!(write.arguments()[0].value, ScalarValue::Integer(42));
        assert_eq!(session.fuel_remaining(), 94);
        let reply = execution.invoke(write);
        session
            .try_accept(&(17, "counter.write"), reply, &execution)
            .unwrap();
        assert_eq!(session.status(), EffectSessionStatus::Ready);
        assert!(matches!(
            session.poll(&execution).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
        ));
        assert_eq!(execution.actual.get(), 42);
        assert_eq!(session.fuel_remaining(), 94);
        assert!(matches!(
            session.poll(&execution).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
        assert_eq!(
            *execution.events.borrow(),
            ["counter.read", "counter.write"]
        );
        assert_eq!(checks.get(), 2);

        execution.events.borrow_mut().clear();
        execution.actual.set(41);
        let mut cancelled =
            EffectSession::new(&expression, Vec::new(), Fuel::new(100), EXECUTION_LIMITS);
        let EffectSessionPoll::Request(read) = cancelled.poll(&execution).unwrap() else {
            panic!()
        };
        let reply = execution.invoke(read);
        cancelled
            .try_accept(&(17, "counter.read"), reply, &execution)
            .unwrap();
        assert!(cancelled.cancel());
        assert!(matches!(
            cancelled.poll(&execution).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
        assert_eq!(execution.actual.get(), 41);
        assert_eq!(*execution.events.borrow(), ["counter.read"]);
        assert_eq!(cancelled.fuel_remaining(), 98);
    }

    #[test]
    fn parsed_recursive_control_source_runs_selected_native_calls_and_pure_tail() {
        let checks = Rc::new(Cell::new(0));
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &[],
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 200,
                    checks: checks.clone(),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let source = expression(
            "bind(ticket: counter.read(), body: choose(when: gt(left: ticket, right: 40), then: bind(next: counter.write(value: add(left: ticket, right: 1)), body: add(left: next, right: 1)), otherwise: bind(next: counter.write(value: 0), body: add(left: next, right: 1))))",
        );
        let mut compiler = Compiler {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: Vec::new(),
        };
        let (program, ty) = lower_control_source(
            &source,
            &[],
            ControlSourceLimits {
                source: LIMITS.source,
                max_bindings: LIMITS.max_bindings,
            },
            &mut compiler,
        )
        .unwrap();
        assert_eq!(ty, ScalarType::Integer);
        let execution = Execution {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: RefCell::new(Vec::new()),
            actual: Cell::new(41),
        };
        let mut session =
            EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION_LIMITS);
        let EffectSessionPoll::Request(read) = session.poll(&execution).unwrap() else {
            panic!()
        };
        assert!(std::ptr::eq(read.schema(), &schemas[0]));
        assert_eq!(session.fuel_remaining(), 98);
        let rejected = session
            .try_accept(&(18, "counter.read"), execution.invoke(read), &execution)
            .unwrap_err();
        session
            .try_accept(&(17, "counter.read"), rejected.reply, &execution)
            .unwrap();
        assert_eq!(execution.actual.get(), 41);
        assert!(matches!(session.status(), EffectSessionStatus::Ready));
        let EffectSessionPoll::Request(write) = session.poll(&execution).unwrap() else {
            panic!()
        };
        assert!(std::ptr::eq(write.schema(), &schemas[1]));
        assert_eq!(write.arguments()[0].value, ScalarValue::Integer(42));
        assert_eq!(session.fuel_remaining(), 88);
        session
            .try_accept(&(17, "counter.write"), execution.invoke(write), &execution)
            .unwrap();
        assert_eq!(execution.actual.get(), 42);
        assert!(matches!(
            session.poll(&execution).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(43)))
        ));
        assert_eq!(session.fuel_remaining(), 84);
        assert_eq!(
            *execution.events.borrow(),
            ["counter.read", "counter.write"]
        );
        assert_eq!(checks.get(), 2);
        assert!(matches!(
            session.poll(&execution).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));

        execution.actual.set(1);
        execution.events.borrow_mut().clear();
        let mut other = EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION_LIMITS);
        let EffectSessionPoll::Request(read) = other.poll(&execution).unwrap() else {
            panic!()
        };
        other
            .try_accept(&(17, "counter.read"), execution.invoke(read), &execution)
            .unwrap();
        let EffectSessionPoll::Request(write) = other.poll(&execution).unwrap() else {
            panic!()
        };
        assert_eq!(write.arguments()[0].value, ScalarValue::Integer(0));
        other
            .try_accept(&(17, "counter.write"), execution.invoke(write), &execution)
            .unwrap();
        assert!(matches!(
            other.poll(&execution).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(1)))
        ));
        assert_eq!(other.fuel_remaining(), 86);
        assert_eq!(
            *execution.events.borrow(),
            ["counter.read", "counter.write"]
        );

        execution.actual.set(41);
        execution.events.borrow_mut().clear();
        let mut cancelled =
            EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION_LIMITS);
        let EffectSessionPoll::Request(read) = cancelled.poll(&execution).unwrap() else {
            panic!()
        };
        cancelled
            .try_accept(&(17, "counter.read"), execution.invoke(read), &execution)
            .unwrap();
        assert!(cancelled.cancel());
        assert!(matches!(
            cancelled.poll(&execution).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
        assert_eq!(*execution.events.borrow(), ["counter.read"]);
        assert_eq!(execution.actual.get(), 41);
    }

    #[test]
    fn recursive_control_native_admission_rejects_cold_types_and_unfinished_capture() {
        let checks = Rc::new(Cell::new(0));
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &[],
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 200,
                    checks: checks.clone(),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let mut compiler = Compiler {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: Vec::new(),
        };
        for source in [
            "choose(when: true, then: counter.read(), otherwise: counter.write(value: false))",
            "bind(x: bind(y: counter.read(), body: add(left: y, right: 1)), body: x)",
            "choose(when: true, then: counter.read(), otherwise: false)",
        ] {
            assert!(
                lower_control_source(
                    &expression(source),
                    &[],
                    ControlSourceLimits {
                        source: LIMITS.source,
                        max_bindings: 8
                    },
                    &mut compiler
                )
                .is_err()
            );
        }
        compiler.host.version = 8;
        assert!(
            lower_control_source(
                &expression("counter.read()"),
                &[],
                ControlSourceLimits {
                    source: LIMITS.source,
                    max_bindings: 8
                },
                &mut compiler
            )
            .is_err()
        );
        compiler.host.version = 7;
        compiler.host.granted = &[31];
        assert!(
            lower_control_source(
                &expression(
                    "choose(when: true, then: counter.read(), otherwise: counter.write(value: 0))"
                ),
                &[],
                ControlSourceLimits {
                    source: LIMITS.source,
                    max_bindings: 8
                },
                &mut compiler
            )
            .is_err()
        );
        assert_eq!(
            checks.get(),
            0,
            "compilation must not accept or invoke native replies"
        );
    }

    #[test]
    fn parsed_binding_invokes_native_calls_accepts_original_domains_and_reenters_with_exact_fuel() {
        let checks = Rc::new(Cell::new(0));
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &[],
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let source = expression(
            "bind(ticket: counter.read(), body: counter.write(value: add(left: ticket, right: 1)))",
        );
        let mut compiler = Compiler {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: Vec::new(),
        };
        let (expression, declared) =
            lower_binding_source(&source, &[], LIMITS, &mut compiler).unwrap();
        let execution = Execution {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: RefCell::new(Vec::new()),
            actual: Cell::new(41),
        };
        let mut values = Vec::new();
        let mut fuel = Fuel::new(100);
        let outcome = evaluate_resumable_effects_in_scope(
            &expression,
            &mut ScopeFrame::new(&mut values),
            &execution,
            &mut fuel,
            EXECUTION_LIMITS,
        )
        .unwrap();
        let ResumableEffectOutcome::Suspended {
            request,
            continuation,
        } = outcome
        else {
            panic!()
        };
        assert!(std::ptr::eq(request.schema(), &schemas[0]));
        assert_eq!(fuel.remaining(), 98);
        let mut pending =
            PendingReply::new((17, "counter.read"), continuation, &request.schema().result);
        let reply = execution.invoke(request);
        let accepted = pending
            .try_accept(&(17, "counter.read"), reply, &execution)
            .unwrap();
        let ResumableEffectOutcome::Request(request) =
            resume_accepted_effects(accepted, &execution, &mut fuel, EXECUTION_LIMITS).unwrap()
        else {
            panic!()
        };
        assert!(std::ptr::eq(request.schema(), declared));
        assert_eq!(request.arguments()[0].value, ScalarValue::Integer(42));
        assert_eq!(fuel.remaining(), 94);
        let mut terminal = PendingReply::new((17, "counter.write"), (), &request.schema().result);
        let reply = execution.invoke(request);
        let (_, (), original, reply) = terminal
            .try_accept(&(17, "counter.write"), reply, &execution)
            .unwrap()
            .into_parts();
        assert!(std::ptr::eq(original, &schemas[1].result));
        assert_eq!(reply.0, ScalarValue::Integer(42));
        assert_eq!(execution.actual.get(), 42);
        assert_eq!(
            *execution.events.borrow(),
            ["counter.read", "counter.write"]
        );
        assert_eq!(checks.get(), 2);
        assert!(values.is_empty());
        assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
        assert_eq!(terminal.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    }

    #[test]
    fn parsed_native_result_binding_uses_original_schemas_received_values_and_exact_fuel() {
        let checks = Rc::new(Cell::new(0));
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &[],
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: checks.clone(),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let input = expression(
            "bind(ticket: counter.read(), body: counter.write(value: add(left: ticket, right: 1)))",
        );
        let mut compiler = Compiler {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
            events: Vec::new(),
        };
        let (output, declared) = lower_binding_source(&input, &[], LIMITS, &mut compiler).unwrap();
        assert!(std::ptr::eq(declared, &schemas[1]));
        assert_eq!(
            compiler.events,
            [
                BindingSourcePhase::Value,
                BindingSourcePhase::Body,
                BindingSourcePhase::Finish
            ]
        );
        let Counter::Bind { name, value, body } = &output else {
            panic!()
        };
        let pure = PureEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
        };
        let call_limits = CallEvaluationLimits {
            pure,
            max_arguments: 64,
        };
        let host = CallEvaluationHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
            environment: &Environment,
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        let read = prepare_call_in_scope(value, &mut scope, &host, &mut fuel, call_limits).unwrap();
        assert!(std::ptr::eq(read.schema(), &schemas[0]));
        assert_eq!(fuel.remaining(), 99);
        let reply = ScalarValue::Integer(41);
        read.schema().check_result(&reply).unwrap();
        assert_eq!(checks.get(), 1);
        {
            let mut local = scope.nested();
            local.push(name, PureValue::Scalar(reply)).unwrap();
            let Counter::Call { arguments, .. } = body.as_ref() else {
                panic!()
            };
            let mut direct_fuel = Fuel::new(100);
            let direct = evaluate_pure_in_scope(
                &arguments[0].value,
                &mut local,
                &Environment,
                &mut direct_fuel,
                pure,
            )
            .unwrap();
            let mut call_fuel = Fuel::new(100);
            let prepared =
                prepare_call_in_scope(body, &mut local, &host, &mut call_fuel, call_limits)
                    .unwrap();
            assert!(std::ptr::eq(prepared.schema(), declared));
            let PureValue::Scalar(direct) = direct else {
                panic!()
            };
            assert_eq!(direct, ScalarValue::Integer(42));
            assert_eq!(prepared.arguments()[0].value, direct);
            assert_eq!(direct_fuel.remaining() - call_fuel.remaining(), 1);
            let bad = prepare_call_in_scope(
                body,
                &mut local,
                &CallEvaluationHost {
                    catalog: &catalog,
                    version: 8,
                    granted: &[31, 32],
                    environment: &Environment,
                },
                &mut Fuel::new(100),
                call_limits,
            );
            assert!(
                bad.is_err(),
                "source observation cannot grant future version authority"
            );
        }
        assert!(scope.is_empty());
        assert!(matches!(
            declared.check_result(&ScalarValue::Boolean(true)),
            Err(HostResultError::TypeMismatch)
        ));
        assert_eq!(checks.get(), 1);
        assert!(matches!(
            declared.check_result(&ScalarValue::Integer(101)),
            Err(HostResultError::InvalidValue(PrivateError(
                "private counter domain"
            )))
        ));
        assert_eq!(checks.get(), 2);
    }

    #[test]
    fn native_versions_grants_and_undefined_initializer_names_stop_before_later_phases() {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &parameters,
                required_capability: 31,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: Rc::new(Cell::new(0)),
                },
            },
            OperationSchema {
                key: "counter.write",
                parameters: &parameters,
                required_capability: 32,
                result: ReplyDeclaration {
                    maximum: 100,
                    checks: Rc::new(Cell::new(0)),
                },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        for (version, grants, value, phase, count) in [
            (8, &[31, 32][..], "1", BindingSourcePhase::Value, 1),
            (7, &[][..], "1", BindingSourcePhase::Value, 1),
            (7, &[31][..], "1", BindingSourcePhase::Body, 2),
            (7, &[31, 32][..], "ticket", BindingSourcePhase::Value, 1),
        ] {
            let input = expression(&format!(
                "bind(ticket: counter.read(value: {value}), body: counter.write(value: ticket))"
            ));
            let mut compiler = Compiler {
                host: SourceCallHost {
                    catalog: &catalog,
                    version,
                    granted: grants,
                },
                events: Vec::new(),
            };
            assert!(
                matches!(lower_binding_source(&input, &[], LIMITS, &mut compiler), Err(BindingSourceError::Native { phase: actual, .. }) if actual == phase)
            );
            assert_eq!(compiler.events.len(), count);
        }
    }
}

#[test]
fn reference_pure_atomic_group_and_helper_bindings_keep_canonical_wire_and_authority() {
    for source in [
        "fn main() = bind(n: 41, body: add(left: n, right: 1))",
        "fn main() = bind(body: field(value: row, name: \"count\"), row: runtime.list())",
        "fn main() = bind(rows: seq(a: runtime.list(), b: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"count\"))",
        "fn rows(n: integer) = choose(when: eq(left: n, right: 0), then: runtime.list(), otherwise: runtime.list())\nfn main() = bind(row: rows(n: 0), body: field(value: row, name: \"count\"))",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let again = leselang_hir::lower(&parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        let grants = leselang_host_contract::CapabilitySet::new(["runtime.read"]);
        assert!(leselang_hir::authorize(&program, &grants).is_ok());
        if source.contains("runtime.list()") {
            assert!(
                leselang_hir::authorize(
                    &program,
                    &leselang_host_contract::CapabilitySet::default()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn reference_cold_type_shadow_group_export_and_capture_flow_fail_closed() {
    for source in [
        "fn main() = bind(row: runtime.list(), body: choose(when: true, then: 1, otherwise: add(left: row, right: 1)))",
        "fn main() = bind(row: runtime.list(), body: loop(row: 1, while: false, next: row, limit: 0))",
        "fn main() = bind(rows: seq(a: runtime.list()), body: member(value: rows, name: \"missing\"))",
        "fn main() = bind(row: runtime.list(), body: seq(a: runtime.list(), b: runtime.list()))",
    ] {
        assert!(leselang_hir::lower(&parse(source)).is_err(), "{source}");
    }
}
