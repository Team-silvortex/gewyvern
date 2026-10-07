use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_expansion::HelperExpansionCost;
use leselang_hir::helper_join::*;
use leselang_hir::helper_returns::{HelperReturnError, HelperReturnLimits};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::*;
use leselang_hir::source_cost::*;
use leselang_runtime_core::*;

type Data = Computation<u8, u32, &'static str, ()>;
const LIMITS: HelperJoinLimits = HelperJoinLimits {
    output: HelperReturnLimits {
        max_nodes: 128,
        max_depth: 16,
    },
    source: SourceCostLimits {
        max_nodes: 128,
        max_depth: 16,
    },
};
const ZERO: SourceCostExtra = SourceCostExtra { nodes: 0, depth: 0 };
fn integer(value: u64) -> Data {
    Data::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Data {
    Data::Local { name: name.into() }
}
fn host(name: &'static str) -> Data {
    Data::Host {
        effect: Box::new(name),
    }
}
fn bind(name: &str, value: Data, body: Data) -> Data {
    Data::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn choose(then: Data, otherwise: Data) -> Data {
    Data::Choose {
        when: Box::new(Data::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn list(count: usize) -> Data {
    Data::Literal {
        value: ScalarValue::StringList(StringListValue(vec!["x".into(); count])),
    }
}
fn plan(value: Data, continuation: Data) -> HelperJoin<Data> {
    HelperJoin::new("answer".into(), value, continuation, LIMITS, |_| {
        Ok::<_, Infallible>(ZERO)
    })
    .unwrap()
}
fn denied<Node, Error>(result: Result<Node, HelperJoinError<Error>>) -> HelperJoinError<Error> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected helper join denial"),
    }
}

#[test]
fn exact_leaf_cost_is_admitted_before_once_whole_output_admission() {
    let calls = Cell::new(0);
    let join = HelperJoin::new(
        "answer".into(),
        integer(41),
        local("answer"),
        HelperJoinLimits {
            output: HelperReturnLimits {
                max_nodes: 3,
                max_depth: 1,
            },
            source: SourceCostLimits {
                max_nodes: 3,
                max_depth: 1,
            },
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(ZERO)
        },
    )
    .unwrap();
    assert_eq!(join.name(), "answer");
    assert_eq!(join.shape().returns, 1);
    assert_eq!(
        join.source_cost(),
        HelperExpansionCost { nodes: 3, depth: 1 }
    );
    let copies = Cell::new(0);
    let admissions = Cell::new(0);
    let output = join
        .connect(
            |body, index| {
                assert_eq!(index, 0);
                assert_eq!(copies.replace(1), 0);
                Ok::<_, Infallible>(body.clone())
            },
            |output, cost| {
                assert_eq!(copies.get(), 1);
                assert_eq!(admissions.replace(1), 0);
                assert_eq!(output, &bind("answer", integer(41), local("answer")));
                assert_eq!(cost, HelperExpansionCost { nodes: 3, depth: 1 });
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(output, bind("answer", integer(41), local("answer")));
    assert_eq!((calls.get(), copies.get(), admissions.get()), (0, 1, 1));
}

#[test]
fn zero_invalid_physical_name_capture_and_boundary_limits_precede_native_queries() {
    let cases = [
        (
            "answer",
            host("input"),
            local("answer"),
            HelperJoinLimits {
                output: HelperReturnLimits {
                    max_nodes: 0,
                    ..LIMITS.output
                },
                ..LIMITS
            },
        ),
        (
            "answer",
            host("input"),
            local("answer"),
            HelperJoinLimits {
                source: SourceCostLimits {
                    max_nodes: 0,
                    ..LIMITS.source
                },
                ..LIMITS
            },
        ),
        (
            "answer",
            host("input"),
            local("answer"),
            HelperJoinLimits {
                output: HelperReturnLimits {
                    max_depth: 0,
                    ..LIMITS.output
                },
                ..LIMITS
            },
        ),
        (
            "answer",
            host("input"),
            local("answer"),
            HelperJoinLimits {
                source: SourceCostLimits {
                    max_nodes: 16_385,
                    ..LIMITS.source
                },
                ..LIMITS
            },
        ),
        (
            "answer",
            host("input"),
            local("answer"),
            HelperJoinLimits {
                output: HelperReturnLimits {
                    max_depth: 65,
                    ..LIMITS.output
                },
                ..LIMITS
            },
        ),
        ("invalid name", host("input"), integer(0), LIMITS),
        (
            "answer",
            bind("visible", integer(1), host("input")),
            local("visible"),
            LIMITS,
        ),
        (
            "answer",
            Data::Recover {
                value: Box::new(host("cold")),
                fallback: Box::new(integer(0)),
            },
            integer(0),
            LIMITS,
        ),
    ];
    for (name, value, continuation, limits) in cases {
        let calls = Cell::new(0);
        assert!(
            HelperJoin::new(name.into(), value, continuation, limits, |_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>(ZERO)
            })
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn both_cold_folded_inputs_are_checked_before_observing_the_first_native_value() {
    for (value, continuation) in [
        (host("value"), list(8)),
        (bind("x", list(8), host("value")), integer(0)),
    ] {
        let calls = Cell::new(0);
        let error = denied(HelperJoin::new(
            "answer".into(),
            value,
            continuation,
            HelperJoinLimits {
                source: SourceCostLimits {
                    max_nodes: 8,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            |_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>(ZERO)
            },
        ));
        assert!(matches!(
            error,
            HelperJoinError::Language(SourceCostError::Structure(StructureError::NodeLimit))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn combined_cold_continuation_weights_are_bounded_before_native_observation() {
    let calls = Cell::new(0);
    let error = denied(HelperJoin::new(
        "answer".into(),
        choose(host("left"), host("right")),
        list(2),
        HelperJoinLimits {
            source: SourceCostLimits {
                max_nodes: 11,
                ..LIMITS.source
            },
            ..LIMITS
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(ZERO)
        },
    ));
    assert!(matches!(
        error,
        HelperJoinError::Bounds(StructureError::NodeLimit)
    ));
    assert_eq!(calls.get(), 0);
    let error = denied(HelperJoin::new(
        "answer".into(),
        host("value"),
        list(1),
        HelperJoinLimits {
            source: SourceCostLimits {
                max_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>(ZERO)
        },
    ));
    assert!(matches!(
        error,
        HelperJoinError::Bounds(StructureError::DepthLimit)
    ));
    assert_eq!(calls.get(), 0);
}

fn native_cost(name: &str) -> SourceCostExtra {
    match name {
        "right" => SourceCostExtra { nodes: 2, depth: 2 },
        "left" => SourceCostExtra { nodes: 3, depth: 1 },
        "continuation" => SourceCostExtra { nodes: 4, depth: 1 },
        _ => ZERO,
    }
}

#[test]
fn original_hosts_are_observed_once_and_copies_keep_left_to_right_order() {
    let events = RefCell::new(vec![]);
    let join = HelperJoin::new(
        "answer".into(),
        choose(host("left"), host("right")),
        host("continuation"),
        LIMITS,
        |name| {
            events.borrow_mut().push(*name);
            Ok::<_, Infallible>(native_cost(name))
        },
    )
    .unwrap();
    assert_eq!(*events.borrow(), ["right", "left", "continuation"]);
    assert_eq!(
        join.source_cost(),
        HelperExpansionCost {
            nodes: 21,
            depth: 4
        }
    );
    let copies = RefCell::new(vec![]);
    let output = join
        .connect(
            |body, index| {
                copies.borrow_mut().push(index);
                Ok::<_, Infallible>(body.clone())
            },
            |output, expected| {
                assert_eq!(*copies.borrow(), [0, 1]);
                assert_eq!(
                    measure_source_cost(output, LIMITS.source, |name| Ok::<_, Infallible>(
                        native_cost(name)
                    ))
                    .unwrap(),
                    expected
                );
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(*events.borrow(), ["right", "left", "continuation"]);
    assert_eq!(
        output,
        choose(
            bind("answer", host("left"), host("continuation")),
            bind("answer", host("right"), host("continuation"))
        )
    );
}

#[test]
fn native_extra_overflow_and_shifted_return_depth_are_checked_before_copy() {
    for (extra, limits, expected_depth) in [
        (
            SourceCostExtra {
                nodes: usize::MAX,
                depth: 0,
            },
            LIMITS,
            false,
        ),
        (
            SourceCostExtra {
                nodes: 0,
                depth: usize::MAX,
            },
            LIMITS,
            true,
        ),
        (
            SourceCostExtra { nodes: 0, depth: 2 },
            HelperJoinLimits {
                source: SourceCostLimits {
                    max_depth: 2,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            true,
        ),
        (
            SourceCostExtra { nodes: 2, depth: 0 },
            HelperJoinLimits {
                source: SourceCostLimits {
                    max_nodes: 4,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            false,
        ),
    ] {
        let calls = Cell::new(0);
        let error = denied(HelperJoin::new(
            "answer".into(),
            host("input"),
            integer(0),
            limits,
            |_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>(extra)
            },
        ));
        assert_eq!(calls.get(), 1);
        assert!(
            matches!(error, HelperJoinError::Bounds(StructureError::DepthLimit)) == expected_depth
        );
        assert!(matches!(error, HelperJoinError::Bounds(_)));
    }
}

#[test]
fn zero_sized_native_slots_do_not_merge_original_occurrence_costs() {
    type Zst = Computation<(), (), (), ()>;
    let leaf = || Zst::Host {
        effect: Box::new(()),
    };
    let value = Zst::Bind {
        name: "prefix".into(),
        value: Box::new(leaf()),
        body: Box::new(Zst::Choose {
            when: Box::new(Zst::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(leaf()),
            otherwise: Box::new(Zst::Bind {
                name: "branch".into(),
                value: Box::new(Zst::Literal {
                    value: ScalarValue::Integer(0),
                }),
                body: Box::new(leaf()),
            }),
        }),
    };
    let calls = Cell::new(0);
    let addresses = RefCell::new(vec![]);
    let join = HelperJoin::new("answer".into(), value, leaf(), LIMITS, |native| {
        addresses.borrow_mut().push(std::ptr::from_ref(native));
        let index = calls.get();
        calls.set(index + 1);
        Ok::<_, Infallible>(SourceCostExtra {
            nodes: [2, 3, 5, 7][index],
            depth: if index == 0 { 4 } else { 0 },
        })
    })
    .unwrap();
    assert_eq!(calls.get(), 4);
    assert!(addresses.borrow().windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(
        join.source_cost(),
        HelperExpansionCost {
            nodes: 36,
            depth: 8
        }
    );
    join.connect(
        |body, _| Ok::<_, Infallible>(body.clone()),
        |_, cost| {
            assert_eq!(
                cost,
                HelperExpansionCost {
                    nodes: 36,
                    depth: 8
                }
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 4);
}

#[test]
fn same_physical_copy_with_changed_folded_node_weights_is_rejected_before_admission() {
    let admissions = Cell::new(0);
    let error = denied(plan(host("value"), list(1)).connect(
        |_, _| Ok::<_, Infallible>(list(2)),
        |_, _| {
            admissions.set(admissions.get() + 1);
            Ok(())
        },
    ));
    assert!(matches!(error, HelperJoinError::ChangedSource));
    assert_eq!(admissions.get(), 0);
}

#[test]
fn same_physical_copy_with_changed_folded_depth_is_rejected_before_admission() {
    let original = Data::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(list(1)),
        right: Box::new(Data::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(integer(0)),
        }),
    };
    let copy = Data::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(integer(0)),
        right: Box::new(Data::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(list(1)),
        }),
    };
    let error = denied(plan(host("value"), original).connect(
        |_, _| Ok::<_, Infallible>(copy.clone()),
        |_, _| panic!("changed source must not reach admission"),
    ));
    assert!(matches!(error, HelperJoinError::ChangedSource));
}

#[test]
fn equal_cost_does_not_certify_cold_native_semantics_or_live_authority() {
    let join = plan(choose(host("left"), host("right")), host("continuation"));
    let copies = Cell::new(0);
    let admissions = Cell::new(0);
    let error = denied(join.connect(
        |_, index| {
            copies.set(copies.get() + 1);
            Ok::<_, &'static str>(host(if index == 0 {
                "continuation"
            } else {
                "foreign_cold_grant"
            }))
        },
        |output, expected| {
            admissions.set(admissions.get() + 1);
            assert_eq!(copies.get(), 2);
            assert_eq!(expected, HelperExpansionCost { nodes: 8, depth: 2 });
            let Data::Choose { otherwise, .. } = output else {
                panic!()
            };
            let Data::Bind { body, .. } = otherwise.as_ref() else {
                panic!()
            };
            assert!(
                matches!(body.as_ref(), Data::Host { effect } if **effect == "foreign_cold_grant")
            );
            Err("revoked or foreign grant")
        },
    ));
    assert!(matches!(
        error,
        HelperJoinError::Admission("revoked or foreign grant")
    ));
    assert_eq!((copies.get(), admissions.get()), (2, 1));
}

struct Slot {
    label: &'static str,
    bytes: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
}
impl Slot {
    fn new(label: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            label,
            bytes: vec![9; 8],
            drops: Rc::clone(drops),
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.label);
    }
}
struct PrivateError(&'static str);
type Native = Computation<Slot, Slot, Slot, Slot>;
fn native_host(label: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native::Host {
        effect: Box::new(Slot::new(label, drops)),
    }
}
fn native_choose(drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native::Choose {
        when: Box::new(Native::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(native_host("left", drops)),
        otherwise: Box::new(native_host("right", drops)),
    }
}

#[test]
fn move_only_all_four_slots_original_boxes_vectors_and_buffers_move_unchanged() {
    let drops = Rc::new(RefCell::new(vec![]));
    let original = Native::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "read".into(),
            result_type: Slot::new("result", &drops),
            value: Native::Call {
                operation: Slot::new("operation", &drops),
                arguments: vec![ComputedArgument {
                    name: "count".into(),
                    value: Native::Field {
                        field: Slot::new("field", &drops),
                        value: Box::new(native_host("effect", &drops)),
                    },
                }],
            },
        }],
    };
    let Native::Group { branches, .. } = &original else {
        panic!()
    };
    let branches_ptr = branches.as_ptr();
    let result_ptr = branches[0].result_type.bytes.as_ptr();
    let Native::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    let operation_ptr = operation.bytes.as_ptr();
    let arguments_ptr = arguments.as_ptr();
    let Native::Field { field, value } = &arguments[0].value else {
        panic!()
    };
    let field_ptr = field.bytes.as_ptr();
    let box_ptr = value.as_ref() as *const Native;
    let Native::Host { effect } = value.as_ref() else {
        panic!()
    };
    let effect_ptr = std::ptr::from_ref(effect.as_ref());
    let bytes_ptr = effect.bytes.as_ptr();
    let queries = Cell::new(0);
    let join = HelperJoin::new(
        "answer".into(),
        original,
        native_host("template", &drops),
        LIMITS,
        |effect| {
            queries.set(queries.get() + 1);
            if effect.label == "effect" {
                assert!(std::ptr::eq(effect, effect_ptr));
                assert_eq!(effect.bytes.as_ptr(), bytes_ptr);
            }
            Ok::<_, PrivateError>(ZERO)
        },
    )
    .unwrap();
    let text = format!("{join:?}");
    assert!(!text.contains("template") && !text.contains("answer"));
    let output = join
        .connect(
            |body, index| {
                assert_eq!(index, 0);
                let Native::Host { effect } = body else {
                    panic!()
                };
                assert_eq!(effect.label, "template");
                Ok::<_, PrivateError>(native_host("copy", &drops))
            },
            |output, cost| {
                assert_eq!(cost, HelperExpansionCost { nodes: 6, depth: 4 });
                let Native::Bind { value, .. } = output else {
                    panic!()
                };
                let Native::Group { branches, .. } = value.as_ref() else {
                    panic!()
                };
                assert_eq!(branches.as_ptr(), branches_ptr);
                assert_eq!(branches[0].result_type.bytes.as_ptr(), result_ptr);
                let Native::Call {
                    operation,
                    arguments,
                } = &branches[0].value
                else {
                    panic!()
                };
                assert_eq!(operation.bytes.as_ptr(), operation_ptr);
                assert_eq!(arguments.as_ptr(), arguments_ptr);
                let Native::Field { field, value } = &arguments[0].value else {
                    panic!()
                };
                assert_eq!(field.bytes.as_ptr(), field_ptr);
                assert_eq!(value.as_ref() as *const Native, box_ptr);
                let Native::Host { effect } = value.as_ref() else {
                    panic!()
                };
                assert!(std::ptr::eq(effect.as_ref(), effect_ptr));
                assert_eq!(effect.bytes.as_ptr(), bytes_ptr);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(queries.get(), 2);
    assert_eq!(*drops.borrow(), ["template"]);
    drop(output);
    let mut actual = drops.borrow().clone();
    actual.sort();
    assert_eq!(
        actual,
        ["copy", "effect", "field", "operation", "result", "template"]
    );
}

#[test]
fn native_observation_error_keeps_private_payload_and_drops_owned_inputs_once() {
    let drops = Rc::new(RefCell::new(vec![]));
    let calls = Cell::new(0);
    let error = denied(HelperJoin::new(
        "answer".into(),
        native_choose(&drops),
        native_host("template", &drops),
        LIMITS,
        |_| {
            calls.set(calls.get() + 1);
            Err(PrivateError("private observer payload"))
        },
    ));
    assert_eq!(calls.get(), 1);
    assert!(!format!("{error:?} {error}").contains("private observer payload"));
    assert!(std::error::Error::source(&error).is_none());
    let HelperJoinError::NativeCost(error) = error else {
        panic!()
    };
    assert_eq!(error.0, "private observer payload");
    let mut actual = drops.borrow().clone();
    actual.sort();
    assert_eq!(actual, ["left", "right", "template"]);
}

#[test]
fn native_observation_unwind_drops_inputs_without_retry_or_materialization() {
    let drops = Rc::new(RefCell::new(vec![]));
    let calls = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = HelperJoin::new(
            "answer".into(),
            native_choose(&drops),
            native_host("template", &drops),
            LIMITS,
            |_| -> Result<SourceCostExtra, PrivateError> {
                calls.set(calls.get() + 1);
                panic!("observer unwind")
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    let mut actual = drops.borrow().clone();
    actual.sort();
    assert_eq!(actual, ["left", "right", "template"]);
}

#[test]
fn copy_error_and_unwind_release_partial_output_without_admission_or_refund() {
    for unwind in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let join = HelperJoin::new(
            "answer".into(),
            native_choose(&drops),
            native_host("template", &drops),
            LIMITS,
            |_| Ok::<_, PrivateError>(ZERO),
        )
        .unwrap();
        let reserved = Cell::new(join.source_cost().nodes);
        let copies = Cell::new(0);
        let admissions = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            join.connect(
                |_, index| {
                    copies.set(copies.get() + 1);
                    if index == 1 {
                        if unwind {
                            panic!("copy unwind")
                        }
                        return Err(PrivateError("private copy payload"));
                    }
                    Ok(native_host("copy", &drops))
                },
                |_, _| {
                    admissions.set(admissions.get() + 1);
                    Ok(())
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let error = denied(result.unwrap());
            assert!(!format!("{error:?}").contains("private copy payload"));
            assert!(matches!(
                error,
                HelperJoinError::Copy(HelperReturnError::Factory { index: 1, .. })
            ));
        }
        assert_eq!((copies.get(), admissions.get(), reserved.get()), (2, 0, 8));
        let mut actual = drops.borrow().clone();
        actual.sort();
        assert_eq!(actual, ["copy", "left", "right", "template"]);
    }
}

#[test]
fn admission_error_and_unwind_never_publish_or_retry_the_complete_output() {
    for unwind in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let join = HelperJoin::new(
            "answer".into(),
            native_host("value", &drops),
            native_host("template", &drops),
            LIMITS,
            |_| Ok::<_, PrivateError>(ZERO),
        )
        .unwrap();
        let admissions = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            join.connect(
                |_, _| Ok(native_host("copy", &drops)),
                |_, _| {
                    admissions.set(admissions.get() + 1);
                    if unwind {
                        panic!("admission unwind")
                    }
                    Err(PrivateError("private admission payload"))
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let error = denied(result.unwrap());
            assert!(!format!("{error:?}").contains("private admission payload"));
            let HelperJoinError::Admission(error) = error else {
                panic!()
            };
            assert_eq!(error.0, "private admission payload");
        }
        assert_eq!(admissions.get(), 1);
        let mut actual = drops.borrow().clone();
        actual.sort();
        assert_eq!(actual, ["copy", "template", "value"]);
    }
}

struct Device;
impl PureTypeEnvironment<u8, u32> for Device {
    type Result = ();
    fn field_type(&self, _: &(), _: &u8) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<u8, u32> for Device {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn unrelated_scalar_host_keeps_full_typing_result_fuel_and_caller_prefix() {
    let continuation = Data::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(local("answer")),
        right: Box::new(integer(1)),
    };
    let original = bind("answer", integer(41), continuation.clone());
    let admissions = Cell::new(0);
    let output = plan(integer(41), continuation)
        .connect(
            |body, _| Ok::<_, ()>(body.clone()),
            |output, _| {
                admissions.set(admissions.get() + 1);
                assert_eq!(
                    infer_pure_type(
                        output,
                        &[],
                        &Device,
                        TypeInferenceLimits {
                            max_nodes: 128,
                            max_depth: 16,
                            max_bindings: 8
                        }
                    )
                    .unwrap(),
                    PureType::Scalar(ScalarType::Integer)
                );
                Ok(())
            },
        )
        .unwrap();
    let run = |node: &Data| {
        let mut fuel = Fuel::new(100);
        let mut bindings = vec![(
            "caller",
            PureValue::Scalar(ScalarValue::String("prefix".into())),
        )];
        let result = evaluate_pure_in_scope(
            node,
            &mut ScopeFrame::new(&mut bindings),
            &Device,
            &mut fuel,
            PureEvaluationLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 8,
            },
        )
        .unwrap();
        assert!(matches!(
            result,
            PureValue::Scalar(ScalarValue::Integer(42))
        ));
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, "caller");
        fuel.remaining()
    };
    assert_eq!(run(&original), run(&output));
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&output).unwrap()
    );
    assert_eq!(admissions.get(), 1);
}

#[test]
fn bounded_return_corpus_costs_match_complete_post_copy_observation() {
    let mut values = vec![
        integer(0),
        list(2),
        host("left"),
        Data::Literal {
            value: ScalarValue::OptionalString(OptionalStringValue(None)),
        },
    ];
    for depth in 0..3 {
        for (index, value) in values.clone().into_iter().enumerate() {
            values.push(bind(
                &format!("v{depth}_{index}"),
                integer(0),
                value.clone(),
            ));
            values.push(choose(value, host("right")));
        }
    }
    for value in values {
        let join = HelperJoin::new(
            "answer".into(),
            value,
            bind("caller", list(2), host("continuation")),
            LIMITS,
            |name| Ok::<_, Infallible>(native_cost(name)),
        )
        .unwrap();
        let expected = join.source_cost();
        let output = join
            .connect(
                |body, _| Ok::<_, Infallible>(body.clone()),
                |output, cost| {
                    assert_eq!(
                        cost,
                        measure_source_cost(output, LIMITS.source, |name| Ok::<_, Infallible>(
                            native_cost(name)
                        ))
                        .unwrap()
                    );
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            expected,
            measure_source_cost(&output, LIMITS.source, |name| Ok::<_, Infallible>(
                native_cost(name)
            ))
            .unwrap()
        );
    }
}
