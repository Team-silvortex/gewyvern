use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::repeat_source::{
    RepeatSourceAdapter, RepeatSourceError, RepeatSourceLimits, RepeatSourcePhase,
    lower_flat_repeat_source,
};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::{OptionalStringValue, ScalarValue, ScopeFrame, StructureError};
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
type Branch = ComputedBranch<Node, Token>;
const LIMITS: RepeatSourceLimits = RepeatSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    expanded: SourceCostLimits {
        max_nodes: 256,
        max_depth: 32,
    },
    max_repetitions: 64,
};
fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn source(count: usize) -> Expression {
    expression(&format!(
        "repeat(times: {count}, body: device.open(value: 7))"
    ))
}
fn token(drops: &Rc<Cell<usize>>) -> Token {
    Token {
        bytes: vec![7; 33],
        drops: drops.clone(),
    }
}
fn call(drops: &Rc<Cell<usize>>) -> Node {
    Node::Call {
        operation: token(drops),
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value: Node::Literal {
                value: ScalarValue::Integer(7),
            },
        }],
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Lower,
    Cost,
    Factory(usize),
    Admit(usize),
}
struct Adapter<'source> {
    input: &'source Expression,
    original: Option<Node>,
    copies: Vec<Option<Node>>,
    events: Vec<Event>,
    drops: Rc<Cell<usize>>,
    scope: Vec<(&'source str, ())>,
    fail: Option<Event>,
    unwind: Option<Event>,
    cost_calls: usize,
    costs: Vec<SourceCostExtra>,
    buffers: Vec<*const ComputedArgument<Node>>,
    first_operation: *const u8,
    first_result: *const u8,
    reject_identity: bool,
    wrong_result: Option<usize>,
    late_cost_unwind: Option<bool>,
}
impl<'source> Adapter<'source> {
    fn new(input: &'source Expression) -> Self {
        let drops = Rc::new(Cell::new(0));
        let original = call(&drops);
        Self {
            input,
            original: Some(original),
            copies: Vec::new(),
            events: Vec::new(),
            drops,
            scope: Vec::new(),
            fail: None,
            unwind: None,
            cost_calls: 0,
            costs: Vec::new(),
            buffers: Vec::new(),
            first_operation: std::ptr::null(),
            first_result: std::ptr::null(),
            reject_identity: false,
            wrong_result: None,
            late_cost_unwind: None,
        }
    }
    fn step(&mut self, event: Event) -> Result<(), PrivateError> {
        self.events.push(event);
        let mut outer = ScopeFrame::new(&mut self.scope);
        let mut scope = outer.nested();
        scope.push("scratch", ()).unwrap();
        if self.unwind == Some(event) {
            panic!("private native repeat unwind");
        }
        if self.fail == Some(event) {
            return Err(PrivateError("private repeat secret"));
        }
        Ok(())
    }
    fn check_source(&self, body: &NamedArgument) {
        let Expression::Call { arguments, .. } = self.input else {
            panic!()
        };
        assert!(std::ptr::eq(
            body,
            arguments
                .iter()
                .find(|argument| argument.name == "body")
                .unwrap()
        ));
    }
    fn buffer(&mut self, value: &Node) {
        if let Node::Call { arguments, .. } = value {
            self.buffers.push(arguments.as_ptr());
        }
    }
}
impl<'source> RepeatSourceAdapter<'source, Token, Token, Token, Token> for Adapter<'source> {
    type Error = PrivateError;
    fn lower_body(&mut self, body: &'source NamedArgument) -> Result<(Node, Token), PrivateError> {
        self.check_source(body);
        self.step(Event::Lower)?;
        let value = self.original.take().unwrap();
        self.buffer(&value);
        if let Node::Call { operation, .. } = &value {
            self.first_operation = operation.bytes.as_ptr();
        }
        let result = token(&self.drops);
        self.first_result = result.bytes.as_ptr();
        Ok((value, result))
    }
    fn host_cost(&mut self, _: &Token) -> Result<SourceCostExtra, PrivateError> {
        self.step(Event::Cost)?;
        let cost = self
            .costs
            .get(self.cost_calls)
            .copied()
            .unwrap_or(SourceCostExtra { nodes: 1, depth: 1 });
        self.cost_calls += 1;
        Ok(cost)
    }
    fn materialize(
        &mut self,
        iteration: usize,
        body: &'source NamedArgument,
        original: &Branch,
    ) -> Result<(Node, Token), PrivateError> {
        self.check_source(body);
        assert_eq!(original.result_type.bytes.as_ptr(), self.first_result);
        self.step(Event::Factory(iteration))?;
        if iteration == 3 {
            match self.late_cost_unwind {
                Some(true) => self.unwind = Some(Event::Cost),
                Some(false) => self.fail = Some(Event::Cost),
                None => {}
            }
        }
        let value = self
            .copies
            .get_mut(iteration - 2)
            .and_then(Option::take)
            .unwrap_or_else(|| call(&self.drops));
        self.buffer(&value);
        let mut result = token(&self.drops);
        if self.wrong_result == Some(iteration) {
            result.bytes[0] = 8;
        }
        Ok((value, result))
    }
    fn admit(
        &mut self,
        iteration: usize,
        body: &'source NamedArgument,
        original: &Branch,
        candidate: &Branch,
    ) -> Result<(), PrivateError> {
        self.check_source(body);
        assert_eq!(candidate.name, format!("iteration_{iteration}"));
        assert_eq!(original.result_type.bytes.as_ptr(), self.first_result);
        self.step(Event::Admit(iteration))?;
        if iteration == 1 {
            assert!(std::ptr::eq(original, candidate));
        }
        if self.reject_identity {
            let Node::Call { operation, .. } = &candidate.value else {
                return Err(PrivateError("not a native call"));
            };
            if operation.bytes != original.result_type.bytes
                || candidate.result_type.bytes != original.result_type.bytes
            {
                return Err(PrivateError("different native operation"));
            }
        }
        Ok(())
    }
}
fn expected(count: usize) -> Vec<Event> {
    let mut events = vec![Event::Lower];
    events.extend((2..=count).map(Event::Factory));
    events.extend((1..=count).map(Event::Admit));
    events
}

#[test]
fn original_body_result_buffers_and_once_factory_admission_order_are_preserved() {
    for count in [1, 3, 64] {
        let input = source(count);
        let mut adapter = Adapter::new(&input);
        let output = lower_flat_repeat_source(&input, LIMITS, &mut adapter).unwrap();
        assert_eq!(adapter.events, expected(count));
        let Node::Group {
            group_kind,
            branches,
        } = &output
        else {
            panic!()
        };
        assert_eq!(*group_kind, GroupKind::Sequence);
        assert_eq!(branches.len(), count);
        assert_eq!(branches[0].result_type.bytes.as_ptr(), adapter.first_result);
        for (index, branch) in branches.iter().enumerate() {
            assert_eq!(branch.name, format!("iteration_{}", index + 1));
            let Node::Call {
                operation,
                arguments,
            } = &branch.value
            else {
                panic!()
            };
            assert_eq!(arguments.as_ptr(), adapter.buffers[index]);
            if index == 0 {
                assert_eq!(operation.bytes.as_ptr(), adapter.first_operation);
            }
        }
        assert_eq!(adapter.drops.get(), 0);
        drop(output);
        assert_eq!(adapter.drops.get(), count * 2);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn root_signatures_literal_counts_and_all_cold_repeat_headers_precede_lowering() {
    for text in [
        "1",
        "seq(a: device.open())",
        "repeat()",
        "repeat(times: 2)",
        "repeat(times: 2, times: 2)",
        "repeat(times: 2, body: device.open(), extra: 1)",
        "repeat(times: 0, body: device.open())",
        "repeat(times: 65, body: device.open())",
        "repeat(times: 18446744073709551615, body: device.open())",
        "repeat(times: true, body: device.open())",
        "repeat(times: add(left: 1, right: 1), body: device.open())",
        "repeat(times: 2, body: choose(when: false, then: device.open(), otherwise: repeat(times: 0, body: device.open())))",
        "repeat(times: 2, body: device.open(value: repeat(times: 1, body: 1, extra: 2)))",
    ] {
        let input = expression(text);
        let mut adapter = Adapter::new(&input);
        assert!(
            lower_flat_repeat_source(&input, LIMITS, &mut adapter).is_err(),
            "{text}"
        );
        assert!(adapter.events.is_empty(), "{text}");
    }
}

#[test]
fn direct_nested_group_and_repeat_bodies_are_not_implicitly_flattened() {
    for body in [
        "seq(a: device.open())",
        "all(a: device.open(), b: device.close())",
        "repeat(times: 1, body: device.open())",
    ] {
        let input = expression(&format!("repeat(times: 2, body: {body})"));
        let mut adapter = Adapter::new(&input);
        assert!(matches!(
            lower_flat_repeat_source(&input, LIMITS, &mut adapter),
            Err(RepeatSourceError::NestedBody { .. })
        ));
        assert!(adapter.events.is_empty());
    }
}

#[test]
fn every_limit_ceiling_and_zero_minimum_capacity_precedes_native_callbacks() {
    let input = source(3);
    let mut cases = Vec::new();
    for field in 0..8 {
        let mut limits = LIMITS;
        match field {
            0 => limits.source.max_source_nodes = 16_385,
            1 => limits.source.max_source_depth = 65,
            2 => limits.source.max_lowered_nodes = 16_385,
            3 => limits.source.max_lowered_depth = 65,
            4 => limits.source.max_arguments = 65,
            5 => limits.expanded.max_nodes = 16_385,
            6 => limits.expanded.max_depth = 65,
            _ => limits.max_repetitions = 65,
        }
        cases.push(limits);
    }
    for field in 0..8 {
        let mut limits = LIMITS;
        match field {
            0 => limits.source.max_source_nodes = 0,
            1 => limits.source.max_source_depth = 0,
            2 => limits.source.max_lowered_nodes = 3,
            3 => limits.source.max_lowered_depth = 0,
            4 => limits.source.max_arguments = 1,
            5 => limits.expanded.max_nodes = 3,
            6 => limits.expanded.max_depth = 0,
            _ => limits.max_repetitions = 0,
        }
        cases.push(limits);
    }
    for limits in cases {
        let mut adapter = Adapter::new(&input);
        assert!(lower_flat_repeat_source(&input, limits, &mut adapter).is_err());
        assert!(adapter.events.is_empty(), "{limits:?}");
    }
}

#[test]
fn whole_cold_source_nodes_depth_names_and_text_precede_lowering() {
    let input = source(2);
    for limits in [
        RepeatSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 3,
                ..LIMITS.source
            },
            ..LIMITS
        },
        RepeatSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter::new(&input);
        assert!(lower_flat_repeat_source(&input, limits, &mut adapter).is_err());
        assert!(adapter.events.is_empty());
    }
    for text in [
        format!("repeat(times: 2, body: device.open({}: 1))", "a".repeat(65)),
        format!(
            "repeat(times: 2, body: device.open(value: \"{}\"))",
            "x".repeat(4097)
        ),
    ] {
        let input = expression(&text);
        let mut adapter = Adapter::new(&input);
        assert!(lower_flat_repeat_source(&input, LIMITS, &mut adapter).is_err());
        assert!(adapter.events.is_empty());
    }
}

#[test]
fn complete_shifted_physical_reservation_precedes_every_factory() {
    let input = source(3);
    for limits in [
        RepeatSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 6,
                ..LIMITS.source
            },
            ..LIMITS
        },
        RepeatSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter::new(&input);
        assert!(matches!(
            lower_flat_repeat_source(&input, limits, &mut adapter),
            Err(RepeatSourceError::Output { iteration: 1, .. })
        ));
        assert_eq!(adapter.events, [Event::Lower]);
        assert_eq!(adapter.drops.get(), 2);
    }
    let mut adapter = Adapter::new(&input);
    let limits = RepeatSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 7,
            max_lowered_depth: 2,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(lower_flat_repeat_source(&input, limits, &mut adapter).is_ok());
}

#[test]
fn folded_constructor_and_opaque_source_costs_are_reserved_before_factories() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Call {
        operation: token(&adapter.drops),
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value: Node::Literal {
                value: ScalarValue::OptionalString(OptionalStringValue(None)),
            },
        }],
    });
    let limits = RepeatSourceLimits {
        expanded: SourceCostLimits {
            max_nodes: 9,
            ..LIMITS.expanded
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_flat_repeat_source(&input, limits, &mut adapter),
        Err(RepeatSourceError::Output {
            iteration: 1,
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!(adapter.events, [Event::Lower]);

    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Host {
        effect: Box::new(token(&adapter.drops)),
    });
    adapter.costs = vec![SourceCostExtra { nodes: 2, depth: 1 }];
    assert!(lower_flat_repeat_source(&input, limits, &mut adapter).is_err());
    assert_eq!(adapter.events, [Event::Lower, Event::Cost]);
}

#[test]
fn expanded_shifted_depth_and_full_opaque_cost_are_not_ast_size_or_fuel() {
    let input = source(2);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Host {
        effect: Box::new(token(&adapter.drops)),
    });
    adapter.costs = vec![SourceCostExtra { nodes: 1, depth: 4 }];
    let limits = RepeatSourceLimits {
        expanded: SourceCostLimits {
            max_depth: 4,
            ..LIMITS.expanded
        },
        ..LIMITS
    };
    assert!(matches!(
        lower_flat_repeat_source(&input, limits, &mut adapter),
        Err(RepeatSourceError::Output {
            iteration: 1,
            error: StructureError::DepthLimit
        })
    ));
    assert_eq!(adapter.events, [Event::Lower, Event::Cost]);
    for extra in [
        SourceCostExtra {
            nodes: usize::MAX,
            depth: 0,
        },
        SourceCostExtra {
            nodes: 0,
            depth: usize::MAX,
        },
    ] {
        let mut adapter = Adapter::new(&input);
        adapter.original = Some(Node::Host {
            effect: Box::new(token(&adapter.drops)),
        });
        adapter.costs = vec![extra];
        assert!(matches!(
            lower_flat_repeat_source(&input, LIMITS, &mut adapter),
            Err(RepeatSourceError::Cost { iteration: 1, .. })
        ));
        assert_eq!(adapter.events, [Event::Lower, Event::Cost]);
    }
}

#[test]
fn original_and_later_atomic_candidates_reject_without_admission_or_more_factories() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Literal {
        value: ScalarValue::Integer(7),
    });
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::Candidate { iteration: 1, .. })
    ));
    assert_eq!(adapter.events, [Event::Lower]);
    let mut adapter = Adapter::new(&input);
    adapter.copies = vec![Some(Node::Unary {
        operator: leselang_runtime_core::UnaryOperator::Len,
        value: Box::new(Node::Literal {
            value: ScalarValue::String("x".into()),
        }),
    })];
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::Candidate { iteration: 2, .. })
    ));
    assert_eq!(adapter.events, [Event::Lower, Event::Factory(2)]);
}

#[test]
fn smaller_larger_and_changed_depth_instances_cannot_spend_a_reserved_shape() {
    let input = source(3);
    for mode in 0..3 {
        let mut adapter = Adapter::new(&input);
        let value = match mode {
            0 => Node::Host {
                effect: Box::new(token(&adapter.drops)),
            },
            1 => Node::Call {
                operation: token(&adapter.drops),
                arguments: vec![ComputedArgument {
                    name: "value".into(),
                    value: Node::Unary {
                        operator: leselang_runtime_core::UnaryOperator::Len,
                        value: Box::new(Node::Literal {
                            value: ScalarValue::String("x".into()),
                        }),
                    },
                }],
            },
            _ => Node::Bind {
                name: "x".into(),
                value: Box::new(Node::Literal {
                    value: ScalarValue::Integer(1),
                }),
                body: Box::new(call(&adapter.drops)),
            },
        };
        adapter.copies = vec![Some(value)];
        assert!(matches!(
            lower_flat_repeat_source(&input, LIMITS, &mut adapter),
            Err(RepeatSourceError::ShapeChanged { iteration: 2, .. })
        ));
        assert_eq!(adapter.events, [Event::Lower, Event::Factory(2)]);
    }
}

#[test]
fn same_physical_shape_cannot_change_folded_or_native_source_cost() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    adapter.copies = vec![Some(Node::Call {
        operation: token(&adapter.drops),
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value: Node::Literal {
                value: ScalarValue::OptionalString(OptionalStringValue(None)),
            },
        }],
    })];
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::CostChanged { iteration: 2, .. })
    ));
    assert_eq!(adapter.events, [Event::Lower, Event::Factory(2)]);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Host {
        effect: Box::new(token(&adapter.drops)),
    });
    adapter.copies = vec![Some(Node::Host {
        effect: Box::new(token(&adapter.drops)),
    })];
    adapter.costs = vec![
        SourceCostExtra { nodes: 1, depth: 1 },
        SourceCostExtra { nodes: 2, depth: 1 },
    ];
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::CostChanged { iteration: 2, .. })
    ));
    assert_eq!(
        adapter.events,
        [Event::Lower, Event::Cost, Event::Factory(2), Event::Cost]
    );
}

#[test]
fn equal_node_count_with_changed_depth_cannot_reuse_the_reserved_shape() {
    let input = source(2);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(Node::Call {
            operation: token(&adapter.drops),
            arguments: Vec::new(),
        }),
        otherwise: Box::new(Node::Call {
            operation: token(&adapter.drops),
            arguments: Vec::new(),
        }),
    });
    adapter.copies = vec![Some(Node::Bind {
        name: "x".into(),
        value: Box::new(Node::Literal {
            value: ScalarValue::Integer(1),
        }),
        body: Box::new(call(&adapter.drops)),
    })];
    let error = lower_flat_repeat_source(&input, LIMITS, &mut adapter)
        .err()
        .unwrap();
    assert!(
        matches!(error, RepeatSourceError::ShapeChanged { iteration: 2, expected, actual }
        if expected.nodes == actual.nodes && expected.depth != actual.depth)
    );
    assert_eq!(adapter.events, [Event::Lower, Event::Factory(2)]);
}

#[test]
fn matching_shape_cost_and_result_tags_never_replace_exact_native_identity() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    let mut different = call(&adapter.drops);
    let Node::Call { operation, .. } = &mut different else {
        panic!()
    };
    operation.bytes[0] = 8;
    adapter.copies = vec![Some(different)];
    adapter.reject_identity = true;
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::Native {
            iteration: 2,
            phase: RepeatSourcePhase::Admit,
            ..
        })
    ));
    assert_eq!(
        adapter.events,
        [
            Event::Lower,
            Event::Factory(2),
            Event::Factory(3),
            Event::Admit(1),
            Event::Admit(2)
        ]
    );
    assert_eq!(adapter.drops.get(), 6);
}

#[test]
fn all_four_native_slots_boxes_and_buffers_need_no_clone_eq_debug_serde_or_send() {
    let input = source(1);
    let mut adapter = Adapter::new(&input);
    let field = token(&adapter.drops);
    let field_pointer = field.bytes.as_ptr();
    let value = Box::new(Node::Local {
        name: "received".into(),
    });
    let pointer = value.as_ref() as *const Node;
    adapter.original = Some(Node::Call {
        operation: token(&adapter.drops),
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value: Node::Field { value, field },
        }],
    });
    let output = lower_flat_repeat_source(&input, LIMITS, &mut adapter).unwrap();
    let Node::Group { branches, .. } = &output else {
        panic!()
    };
    let Node::Call { arguments, .. } = &branches[0].value else {
        panic!()
    };
    let Node::Field { value, field } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(value.as_ref() as *const Node, pointer);
    assert_eq!(field.bytes.as_ptr(), field_pointer);
    let input = source(1);
    let mut adapter = Adapter::new(&input);
    adapter.original = Some(Node::Host {
        effect: Box::new(token(&adapter.drops)),
    });
    assert!(lower_flat_repeat_source(&input, LIMITS, &mut adapter).is_ok());
}

#[test]
fn native_failures_preserve_phase_iteration_payload_and_restore_frames_without_retry() {
    for (event, phase, iteration, consumed) in [
        (Event::Lower, RepeatSourcePhase::Lower, 1, 0),
        (Event::Factory(2), RepeatSourcePhase::Materialize, 2, 2),
        (Event::Factory(3), RepeatSourcePhase::Materialize, 3, 4),
        (Event::Admit(1), RepeatSourcePhase::Admit, 1, 6),
        (Event::Admit(2), RepeatSourcePhase::Admit, 2, 6),
        (Event::Admit(3), RepeatSourcePhase::Admit, 3, 6),
    ] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.fail = Some(event);
        let error = lower_flat_repeat_source(&input, LIMITS, &mut adapter)
            .err()
            .unwrap();
        assert_eq!(adapter.events.last(), Some(&event));
        assert!(!format!("{error:?} {error}").contains("private repeat secret"));
        assert!(std::error::Error::source(&error).is_none());
        assert!(
            matches!(error, RepeatSourceError::Native { iteration: at, phase: stage, error: PrivateError("private repeat secret"), .. } if at == iteration && stage == phase)
        );
        assert_eq!(adapter.drops.get(), consumed);
        assert!(adapter.scope.is_empty());
        assert!(lower_flat_repeat_source(&input, LIMITS, &mut Adapter::new(&input)).is_ok());
    }
}

#[test]
fn native_unwind_drops_all_consumed_instances_and_restores_scopes_once() {
    for (event, consumed) in [
        (Event::Lower, 0),
        (Event::Factory(2), 2),
        (Event::Factory(3), 4),
        (Event::Admit(1), 6),
        (Event::Admit(3), 6),
    ] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.unwind = Some(event);
        assert!(
            catch_unwind(AssertUnwindSafe(|| lower_flat_repeat_source(
                &input,
                LIMITS,
                &mut adapter
            )))
            .is_err()
        );
        assert_eq!(adapter.events.last(), Some(&event));
        assert_eq!(adapter.drops.get(), consumed);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn native_cost_failure_and_unwind_release_original_or_partial_instances_without_admission() {
    for unwind in [false, true] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.original = Some(Node::Host {
            effect: Box::new(token(&adapter.drops)),
        });
        let before = adapter.drops.get();
        if unwind {
            adapter.unwind = Some(Event::Cost);
        } else {
            adapter.fail = Some(Event::Cost);
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            lower_flat_repeat_source(&input, LIMITS, &mut adapter)
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = outcome.unwrap().err().unwrap();
            assert!(matches!(
                error,
                RepeatSourceError::Cost {
                    iteration: 1,
                    error: SourceCostError::Observation {
                        host_index: 0,
                        error: PrivateError("private repeat secret")
                    }
                }
            ));
        }
        assert_eq!(adapter.events, [Event::Lower, Event::Cost]);
        assert_eq!(adapter.drops.get() - before, 2);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn late_native_cost_failure_and_unwind_drop_every_instance_without_admission() {
    for unwind in [false, true] {
        let input = source(3);
        let mut adapter = Adapter::new(&input);
        adapter.original = Some(Node::Host {
            effect: Box::new(token(&adapter.drops)),
        });
        adapter.copies = vec![
            Some(Node::Host {
                effect: Box::new(token(&adapter.drops)),
            }),
            Some(Node::Host {
                effect: Box::new(token(&adapter.drops)),
            }),
        ];
        adapter.late_cost_unwind = Some(unwind);
        let before = adapter.drops.get();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            lower_flat_repeat_source(&input, LIMITS, &mut adapter)
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(RepeatSourceError::Cost {
                    iteration: 3,
                    error: SourceCostError::Observation {
                        host_index: 0,
                        error: PrivateError("private repeat secret")
                    }
                })
            ));
        }
        assert_eq!(
            adapter.events,
            [
                Event::Lower,
                Event::Cost,
                Event::Factory(2),
                Event::Cost,
                Event::Factory(3),
                Event::Cost
            ]
        );
        assert_eq!(adapter.drops.get() - before, 6);
        assert!(adapter.scope.is_empty());
    }
}

#[test]
fn reference_flat_and_legacy_nested_repeat_keep_labels_wire_authority_and_diagnostics() {
    for body in [
        "repeat(times: 3, body: ui.focus(node_id: concat(left: \"a\", right: \"b\")))",
        "bind(node: \"a\", body: repeat(body: ui.focus(node_id: node), times: 2))",
        "bind(node: \"a\", body: repeat(times: 2, body: seq(a: ui.focus(node_id: node), b: runtime.list())))",
        "bind(node: \"a\", body: repeat(times: 2, body: repeat(times: 2, body: ui.focus(node_id: node))))",
    ] {
        let program = leselang_hir::lower(&parse(&format!("fn main() = {body}"))).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&leselang_hir::lower(&parse(&canonical)).unwrap()).unwrap()
        );
        leselang_hir::authorize(
            &program,
            &leselang_host_contract::CapabilitySet::new(["ui.presentation", "runtime.read"]),
        )
        .unwrap();
        assert!(
            leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
                .is_err()
        );
    }
    for (body, code) in [
        (
            "repeat(times: 0, body: ui.focus(node_id: concat(left: \"a\", right: \"b\")))",
            "LSH1301",
        ),
        (
            "repeat(times: 2, body: ui.focus(node_id: missing))",
            "LSH1403",
        ),
    ] {
        let errors = leselang_hir::lower(&parse(&format!("fn main() = {body}"))).unwrap_err();
        assert_eq!(errors[0].code, code);
        assert!(errors[0].span.is_some());
    }
}

#[test]
fn same_sized_native_result_observations_do_not_become_declarations() {
    let input = source(3);
    let mut adapter = Adapter::new(&input);
    adapter.wrong_result = Some(2);
    adapter.reject_identity = true;
    assert!(matches!(
        lower_flat_repeat_source(&input, LIMITS, &mut adapter),
        Err(RepeatSourceError::Native {
            iteration: 2,
            phase: RepeatSourcePhase::Admit,
            ..
        })
    ));
    assert_eq!(
        adapter.events,
        [
            Event::Lower,
            Event::Factory(2),
            Event::Factory(3),
            Event::Admit(1),
            Event::Admit(2)
        ]
    );
    assert_eq!(adapter.drops.get(), 6);
}

mod counter {
    use super::*;
    use leselang_hir::call_evaluation::{
        CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
    };
    use leselang_hir::effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, EffectEvaluationOutcome,
        evaluate_effects_in_scope,
    };
    use leselang_hir::pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue,
    };
    use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
    use leselang_hir::source_call::{SourceCallHost, SourceSchema, lower_source_call};
    use leselang_runtime_core::*;

    struct Declaration;
    impl HostResultDomain<ScalarValue> for Declaration {
        type Error = PrivateError;
        fn matches_type(&self, value: &ScalarValue) -> bool {
            matches!(value, ScalarValue::Integer(_))
        }
        fn validate_value(&self, value: &ScalarValue) -> Result<(), PrivateError> {
            if matches!(value, ScalarValue::Integer(0..=100)) {
                Ok(())
            } else {
                Err(PrivateError("private result domain"))
            }
        }
    }
    type Schema<'a> = SourceSchema<'a, &'static str, ScalarTypeSet, Declaration, u8>;
    type Ir<'a> = Computation<(), &'static str, (), &'a Schema<'a>>;
    type IrBranch<'a> = ComputedBranch<Ir<'a>, &'a Schema<'a>>;
    const PURE: PureEvaluationLimits = PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    };
    struct Counter<'host, 'schema> {
        host: SourceCallHost<'host, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
        events: Vec<&'static str>,
        wrong_result: Option<usize>,
        changed_value: Option<usize>,
    }
    fn integer(node: &Ir<'_>) -> Result<u64, PrivateError> {
        if let Ir::Literal {
            value: ScalarValue::Integer(value),
        } = node
        {
            Ok(*value)
        } else {
            Err(PrivateError("not an integer"))
        }
    }
    impl<'source, 'schema>
        RepeatSourceAdapter<'source, (), &'static str, (), &'schema Schema<'schema>>
        for Counter<'_, 'schema>
    {
        type Error = PrivateError;
        fn lower_body(
            &mut self,
            body: &'source NamedArgument,
        ) -> Result<(Ir<'schema>, &'schema Schema<'schema>), PrivateError> {
            self.events.push("lower");
            let call = lower_source_call(&body.value, &self.host, LIMITS.source, |argument| {
                lower_scalar_source_with_scope(
                    &argument.value,
                    ScalarSourceLimits {
                        source: LIMITS.source,
                        max_bindings: 8,
                    },
                    &[],
                    |_, _| -> Result<(Ir<'schema>, Option<ScalarType>), PrivateError> {
                        Err(PrivateError("unknown alias"))
                    },
                )
                .map(|(value, ty)| (value, Some(ty)))
            })
            .map_err(|_| PrivateError("schema rejected"))?;
            let schema = call.schema();
            Ok((
                Ir::Call {
                    operation: schema.key,
                    arguments: call.into_arguments(),
                },
                schema,
            ))
        }
        fn host_cost(&mut self, _: &()) -> Result<SourceCostExtra, PrivateError> {
            Err(PrivateError("no opaque effects"))
        }
        fn materialize(
            &mut self,
            iteration: usize,
            _: &'source NamedArgument,
            original: &IrBranch<'schema>,
        ) -> Result<(Ir<'schema>, &'schema Schema<'schema>), PrivateError> {
            self.events.push("copy");
            let Ir::Call {
                operation,
                arguments,
            } = &original.value
            else {
                return Err(PrivateError("not a call"));
            };
            let mut copied = Vec::new();
            for argument in arguments {
                let value =
                    integer(&argument.value)? + u64::from(self.changed_value == Some(iteration));
                copied.push(ComputedArgument {
                    name: argument.name.to_owned(),
                    value: Ir::Literal {
                        value: ScalarValue::Integer(value),
                    },
                });
            }
            let declaration = if self.wrong_result == Some(iteration) {
                self.host
                    .catalog
                    .authorize("counter.other", self.host.version, self.host.granted)
                    .map_err(|_| PrivateError("missing alternate row"))?
            } else {
                original.result_type
            };
            Ok((
                Ir::Call {
                    operation,
                    arguments: copied,
                },
                declaration,
            ))
        }
        fn admit(
            &mut self,
            _: usize,
            _: &'source NamedArgument,
            original: &IrBranch<'schema>,
            candidate: &IrBranch<'schema>,
        ) -> Result<(), PrivateError> {
            self.events.push("admit");
            let (
                Ir::Call {
                    operation,
                    arguments,
                },
                Ir::Call {
                    operation: first,
                    arguments: before,
                },
            ) = (&candidate.value, &original.value)
            else {
                return Err(PrivateError("not a call"));
            };
            let row = self
                .host
                .catalog
                .authorize(operation, self.host.version, self.host.granted)
                .map_err(|_| PrivateError("native admission rejected"))?;
            if operation != first
                || !std::ptr::eq(row, candidate.result_type)
                || !std::ptr::eq(row, original.result_type)
                || arguments.len() != before.len()
            {
                return Err(PrivateError("wrong original declaration"));
            }
            for (candidate, original) in arguments.iter().zip(before) {
                if candidate.name != original.name
                    || integer(&candidate.value)? != integer(&original.value)?
                {
                    return Err(PrivateError("changed repeated value"));
                }
            }
            Ok(())
        }
    }

    struct Environment<'a> {
        catalog: &'a OperationCatalog<'a, &'static str, &'a str, ScalarTypeSet, Declaration, u8>,
        version: u32,
        grants: &'a [u8],
    }
    impl PureEvaluationEnvironment<(), &'static str> for Environment<'_> {
        type Result = ();
        type Error = PrivateError;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, PrivateError> {
            Err(PrivateError("no fields"))
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), PrivateError> {
            Err(PrivateError("no replies"))
        }
    }
    impl<'expression, 'schema>
        EffectEvaluationEnvironment<'expression, (), &'static str, (), &'schema Schema<'schema>>
        for Environment<'schema>
    {
        type Request = Vec<PreparedCall<'schema, &'static str, ScalarTypeSet, Declaration, u8>>;
        type Capture = ();
        fn preflight_effect(
            &self,
            expression: &'expression Ir<'schema>,
        ) -> Result<(), PrivateError> {
            if let Ir::Group { branches, .. } = expression {
                for branch in branches {
                    let Ir::Call { operation, .. } = &branch.value else {
                        return Err(PrivateError("not atomic"));
                    };
                    let row = self
                        .catalog
                        .authorize(operation, self.version, self.grants)
                        .map_err(|_| PrivateError("live policy rejected"))?;
                    if !std::ptr::eq(row, branch.result_type) {
                        return Err(PrivateError("forged result declaration"));
                    }
                }
            } else if let Ir::Call {
                operation,
                arguments,
            } = expression
            {
                let row = self
                    .catalog
                    .authorize(operation, self.version, self.grants)
                    .map_err(|_| PrivateError("live policy rejected"))?;
                row.bind_arguments(
                    &arguments
                        .iter()
                        .map(|argument| argument.name.as_str())
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| PrivateError("wrong names"))?;
            } else {
                return Err(PrivateError("not group/call"));
            }
            Ok(())
        }
        fn prepare_effect(
            &self,
            expression: &'expression Ir<'schema>,
            scope: &mut ScopeFrame<'_, 'expression, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Request, CalculationFailure<PrivateError>> {
            let Ir::Group { branches, .. } = expression else {
                return Err(PrivateError("not group").into());
            };
            let mut calls = Vec::new();
            for branch in branches {
                let Ir::Call {
                    operation,
                    arguments,
                } = &branch.value
                else {
                    return Err(PrivateError("not atomic").into());
                };
                let row = self
                    .catalog
                    .authorize(operation, self.version, self.grants)
                    .map_err(|_| PrivateError("live policy rejected"))?;
                fuel.charge(1)
                    .map_err(|_| PrivateError("native call root fuel"))?;
                calls.push(
                    evaluate_call_arguments_in_scope(
                        arguments,
                        row,
                        scope,
                        self,
                        fuel,
                        CallEvaluationLimits {
                            pure: PURE,
                            max_arguments: 64,
                        },
                    )
                    .map_err(|_| PrivateError("argument preparation rejected"))?,
                );
            }
            Ok(calls)
        }
        fn capture(
            &self,
            _: &'expression str,
            _: &'expression Ir<'schema>,
            _: &ScopeFrame<'_, 'expression, PureValue<()>>,
            _: &mut Fuel,
        ) -> Result<(), PrivateError> {
            Err(PrivateError("no suspension"))
        }
    }

    #[test]
    fn parsed_native_repeat_prepares_original_schema_values_and_exact_fuel_without_dispatch() {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [OperationSchema {
            key: "counter.read",
            parameters: &parameters,
            required_capability: 31,
            result: Declaration,
        }];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 1,
                max_parameters_per_operation: 1,
            },
        )
        .unwrap();
        let input = expression("repeat(times: 3, body: counter.read(value: 42))");
        let mut adapter = Counter {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31],
            },
            events: Vec::new(),
            wrong_result: None,
            changed_value: None,
        };
        let output = lower_flat_repeat_source(&input, LIMITS, &mut adapter).unwrap();
        assert_eq!(
            adapter.events,
            ["lower", "copy", "copy", "admit", "admit", "admit"]
        );
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31],
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        let outcome = evaluate_effects_in_scope(
            &output,
            &mut scope,
            &environment,
            &mut fuel,
            EffectEvaluationLimits {
                pure: PURE,
                max_arguments: 64,
                max_branches: 64,
            },
        )
        .unwrap();
        let EffectEvaluationOutcome::Request(calls) = outcome else {
            panic!()
        };
        assert_eq!(calls.len(), 3);
        for call in calls {
            assert!(std::ptr::eq(call.schema(), &schemas[0]));
            assert_eq!(call.arguments()[0].value, ScalarValue::Integer(42));
            call.schema()
                .check_result(&ScalarValue::Integer(42))
                .unwrap();
            assert!(
                call.schema()
                    .check_result(&ScalarValue::Boolean(true))
                    .is_err()
            );
            assert!(
                call.schema()
                    .check_result(&ScalarValue::Integer(101))
                    .is_err()
            );
        }
        assert_eq!(
            fuel.remaining(),
            93,
            "one group and three call/literal roots"
        );
        assert!(scope.is_empty());
    }

    #[test]
    fn wrong_copy_declarations_changed_values_and_stale_live_policy_cannot_return_partial_requests()
    {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.read",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration,
            },
            OperationSchema {
                key: "counter.other",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration,
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
        let input = expression("repeat(times: 3, body: counter.read(value: 42))");
        for (wrong_result, changed_value) in [(Some(2), None), (None, Some(2))] {
            let mut adapter = Counter {
                host: SourceCallHost {
                    catalog: &catalog,
                    version: 7,
                    granted: &[31, 32],
                },
                events: Vec::new(),
                wrong_result,
                changed_value,
            };
            assert!(matches!(
                lower_flat_repeat_source(&input, LIMITS, &mut adapter),
                Err(RepeatSourceError::Native {
                    iteration: 2,
                    phase: RepeatSourcePhase::Admit,
                    ..
                })
            ));
            assert_eq!(adapter.events, ["lower", "copy", "copy", "admit", "admit"]);
        }
        let mut adapter = Counter {
            host: SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31],
            },
            events: Vec::new(),
            wrong_result: None,
            changed_value: None,
        };
        let mut output = lower_flat_repeat_source(&input, LIMITS, &mut adapter).unwrap();
        for (version, grants) in [(8, &[31][..]), (7, &[][..])] {
            let environment = Environment {
                catalog: &catalog,
                version,
                grants,
            };
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            assert!(
                evaluate_effects_in_scope(
                    &output,
                    &mut scope,
                    &environment,
                    &mut fuel,
                    EffectEvaluationLimits {
                        pure: PURE,
                        max_arguments: 64,
                        max_branches: 64
                    }
                )
                .is_err()
            );
            assert_eq!(fuel.remaining(), 100);
        }
        let Ir::Group { branches, .. } = &mut output else {
            panic!()
        };
        branches[2].result_type = &schemas[1];
        let environment = Environment {
            catalog: &catalog,
            version: 7,
            grants: &[31, 32],
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let mut fuel = Fuel::new(100);
        assert!(
            evaluate_effects_in_scope(
                &output,
                &mut scope,
                &environment,
                &mut fuel,
                EffectEvaluationLimits {
                    pure: PURE,
                    max_arguments: 64,
                    max_branches: 64
                }
            )
            .is_err()
        );
        assert_eq!(fuel.remaining(), 100);
    }
}
