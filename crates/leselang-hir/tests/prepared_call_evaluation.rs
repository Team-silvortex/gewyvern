use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    call_evaluation::{
        CallEvaluationHost, CallEvaluationLimits, PreparedCall, prepare_call_in_scope,
    },
    call_typing::{CallTypeHost, CallTypeLimits},
    ir::{Computation, ComputedArgument, ComputedBranch, GroupKind},
    native_group_preparation::{NativeGroupPreparationError, prepare_native_group_members},
    parallel_evaluation::{ParallelPoll, PreparedParallelMember},
    prepared_call_evaluation::*,
    prepared_parallel::PreparedParallelSession,
    prepared_sequence::PreparedSequenceSession,
    prepared_typing::{PreparedCallTypeError, infer_prepared_call_type},
    pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits, PureValue},
    pure_typing::{PureType, PureTypeEnvironment, TypeInferenceLimits},
    sequence_evaluation::{SequenceEvaluationLimits, SequencePoll},
};
use leselang_runtime_core::*;

#[derive(Default)]
struct Model {
    comparisons: Cell<usize>,
    clones: Cell<usize>,
    effects: Cell<usize>,
    events: RefCell<Vec<&'static str>>,
    project_failure: Cell<bool>,
    unwind: Cell<Option<&'static str>>,
    late_domain_unwind: Cell<bool>,
    live: Cell<bool>,
}
struct Operation(&'static str, Rc<Model>);
impl PartialEq for Operation {
    fn eq(&self, other: &Self) -> bool {
        self.1.comparisons.set(self.1.comparisons.get() + 1);
        self.0 == other.0
    }
}
struct Field(&'static str);
struct Native;
struct Private(String);
struct View(Rc<Model>);
impl Clone for View {
    fn clone(&self) -> Self {
        self.0.clones.set(self.0.clones.get() + 1);
        Self(self.0.clone())
    }
}
struct Environment(Rc<Model>);
impl PureEvaluationEnvironment<Field, Operation> for Environment {
    type Result = View;
    type Error = Private;
    fn field(&self, view: &View, field: &Field) -> Result<ScalarValue, Private> {
        assert!(Rc::ptr_eq(&view.0, &self.0));
        self.0.events.borrow_mut().push(field.0);
        assert_ne!(self.0.unwind.get(), Some("projection"));
        if field.0 == "late" && self.0.late_domain_unwind.get() {
            self.0.unwind.set(Some("domain"));
        }
        if self.0.project_failure.get() {
            Err(Private("private adapter details".into()))
        } else {
            Ok(ScalarValue::Integer(40))
        }
    }
    fn member(&self, _: &View, _: &str, _: &Operation) -> Result<View, Private> {
        Err(Private("no member".into()))
    }
}
struct Types;
impl PureTypeEnvironment<Field, Operation> for Types {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        Some(ScalarType::Integer)
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}
struct Domain(&'static str, Rc<Model>);
impl ScalarArgumentDomain for Domain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        self.1.events.borrow_mut().push(self.0);
        assert_ne!(self.1.unwind.get(), Some("domain"));
        matches!(value, ScalarValue::Integer(n) if *n <= 100)
    }
}
struct Declaration(Rc<Model>);
impl HostResultDomain<u64> for Declaration {
    type Error = Private;
    fn matches_type(&self, _: &u64) -> bool {
        true
    }
    fn validate_value(&self, n: &u64) -> Result<(), Private> {
        if *n <= 100 && self.0.live.get() {
            Ok(())
        } else {
            Err(Private("bad reply".into()))
        }
    }
}
impl ReplyAuthority<(u8, usize)> for Model {
    type Error = Private;
    fn authorize(&self, key: &(u8, usize)) -> Result<(), Private> {
        if self.live.get() && key.0 == 7 {
            Ok(())
        } else {
            Err(Private("revoked".into()))
        }
    }
}
type Node = Computation<Field, Operation, Native, usize>;
type Schema<'a> = OperationSchema<'a, Operation, &'a str, Domain, Declaration, u8>;
type Host<'a, 's> = CallEvaluationHost<'a, 's, Operation, Domain, Declaration, u8, Environment>;
type Call<'s> = PreparedCall<'s, Operation, Domain, Declaration, u8>;
const LIMITS: CallEvaluationLimits = CallEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 4,
};
const GROUP: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
fn fixture(run: impl FnOnce(&Host<'_, '_>, &[Schema<'_>], &Rc<Model>)) {
    let model = Rc::new(Model::default());
    model.live.set(true);
    let environment = Environment(model.clone());
    let parameters = [
        NamedParameter::required("n", Domain("n-domain", model.clone())),
        NamedParameter::optional("note", Domain("note-domain", model.clone())),
        NamedParameter::required("tag", Domain("tag-domain", model.clone())),
    ];
    let schemas = ["counter.bump", "counter.other"].map(|name| Schema {
        key: Operation(name, model.clone()),
        parameters: &parameters,
        result: Declaration(model.clone()),
        required_capability: 3,
    });
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    let host = Host {
        catalog: &catalog,
        version: 7,
        granted: &[3],
        environment: &environment,
    };
    model.comparisons.set(0);
    run(&host, &schemas, &model);
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn integer(n: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(n),
    }
}
fn boolean(value: bool) -> Node {
    Node::Literal {
        value: ScalarValue::Boolean(value),
    }
}
fn argument(name: &str, value: Node) -> ComputedArgument<Node> {
    ComputedArgument {
        name: name.into(),
        value,
    }
}
fn call(model: &Rc<Model>, operation: &'static str, n: Node) -> Node {
    Node::Call {
        operation: Operation(operation, model.clone()),
        arguments: vec![argument("tag", integer(7)), argument("n", n)],
    }
}
fn choose(when: Node, then: Node, otherwise: Node) -> Node {
    Node::Choose {
        when: Box::new(when),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn field(name: &'static str) -> Node {
    Node::Field {
        value: Box::new(local("reply")),
        field: Field(name),
    }
}
fn flow(model: &Rc<Model>, selected: bool) -> Node {
    bind(
        "amount",
        field("n"),
        choose(
            boolean(selected),
            call(
                model,
                "counter.bump",
                Node::Binary {
                    operator: BinaryOperator::Add,
                    left: Box::new(local("amount")),
                    right: Box::new(local("seed")),
                },
            ),
            call(model, "counter.bump", integer(2)),
        ),
    )
}
fn bindings(model: &Rc<Model>) -> Vec<(&'static str, PureValue<View>)> {
    vec![
        ("reply", PureValue::Result(View(model.clone()))),
        ("seed", PureValue::Scalar(ScalarValue::Integer(1))),
    ]
}

#[test]
fn uniform_original_schema_prepares_one_selected_call_in_live_scope_without_capture_or_prefix_copy()
{
    fixture(|host, schemas, model| {
        for selected in [true, false] {
            let node = flow(model, selected);
            let inferred = infer_prepared_call_type(
                &node,
                &[
                    ("reply", PureType::Result(())),
                    ("seed", PureType::Scalar(ScalarType::Integer)),
                ],
                &Types,
                &CallTypeHost {
                    catalog: host.catalog,
                    version: 7,
                    granted: &[3],
                    environment: &Types,
                },
                CallTypeLimits {
                    pure: TypeInferenceLimits {
                        max_nodes: 128,
                        max_depth: 16,
                        max_bindings: 8,
                    },
                    max_arguments: 4,
                },
            )
            .unwrap();
            assert!(std::ptr::eq(inferred, &schemas[0].result));
            model.events.borrow_mut().clear();
            model.clones.set(0);
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(50);
            let prepared =
                prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, LIMITS).unwrap();
            assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
            assert_eq!(
                prepared.arguments()[0].value,
                ScalarValue::Integer(if selected { 41 } else { 2 })
            );
            assert_eq!(
                (
                    prepared.arguments()[0].parameter_index,
                    prepared.arguments()[0].argument_index
                ),
                (0, 1)
            );
            assert_eq!(
                (
                    prepared.arguments()[1].parameter_index,
                    prepared.arguments()[1].argument_index
                ),
                (2, 0)
            );
            assert_eq!(*model.events.borrow(), ["n", "n-domain", "tag-domain"]);
            assert_eq!(model.clones.get(), 1);
            assert_eq!(model.effects.get(), 0);
            assert_eq!(scope.len(), 2);
            assert!(scope.get("amount").is_none());
            assert_eq!(fuel.remaining(), if selected { 40 } else { 42 });
        }
    });
}

#[test]
fn direct_call_cost_and_native_argument_failure_match_the_released_entry() {
    fixture(|host, schemas, model| {
        for n in [41, 101] {
            let node = call(model, "counter.bump", integer(n));
            let mut prefix = Vec::new();
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut atomic_fuel = Fuel::new(20);
            let atomic =
                prepare_atomic_call_in_scope(&node, &mut scope, host, &mut atomic_fuel, LIMITS);
            let mut old_fuel = Fuel::new(20);
            let old = prepare_call_in_scope(&node, &mut scope, host, &mut old_fuel, LIMITS);
            assert_eq!(atomic_fuel.remaining(), old_fuel.remaining());
            assert_eq!(atomic_fuel.remaining(), 17);
            if n <= 100 {
                assert!(std::ptr::eq(
                    atomic.unwrap().schema(),
                    old.unwrap().schema()
                ));
            } else {
                assert!(matches!(
                    atomic,
                    Err(PreparedCallEvaluationError::Execution(
                        CalculationFailure::External(
                            leselang_hir::effect_evaluation::EffectEvaluationFault::Native(
                                PreparedCallNativeFault::Arguments(_)
                            )
                        )
                    ))
                ));
                assert!(old.is_err());
            }
            assert!(std::ptr::eq(
                &schemas[0],
                host.catalog.authorize(&schemas[0].key, 7, &[3]).unwrap()
            ));
        }
    });
}

#[test]
fn cold_physical_flow_limits_and_prefix_reject_before_native_comparisons_or_execution() {
    fixture(|host, _, model| {
        for case in 0..9 {
            let good = call(model, "counter.bump", integer(1));
            let bad = match case {
                0 => Node::Host {
                    effect: Box::new(Native),
                },
                1 => Node::Group {
                    group_kind: GroupKind::Sequence,
                    branches: vec![],
                },
                2 => integer(0),
                3 => bind(
                    "bad",
                    call(model, "counter.bump", integer(1)),
                    call(model, "counter.bump", integer(2)),
                ),
                4 => call(
                    model,
                    "counter.bump",
                    Node::Host {
                        effect: Box::new(Native),
                    },
                ),
                _ => call(model, "counter.bump", integer(2)),
            };
            let node = choose(boolean(true), good, bad);
            let mut limits = LIMITS;
            match case {
                5 => limits.pure.max_nodes = 1,
                6 => limits.pure.max_depth = 0,
                7 => limits.max_arguments = 65,
                _ => {}
            }
            let mut prefix = bindings(model);
            if case == 8 {
                prefix.push(("reply", PureValue::Scalar(ScalarValue::None)));
            }
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(50);
            model.comparisons.set(0);
            assert!(
                prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, limits).is_err()
            );
            assert_eq!(fuel.remaining(), 50);
            assert_eq!(model.comparisons.get(), 0);
            assert_eq!(model.clones.get(), 0);
            assert!(model.events.borrow().is_empty());
        }
    });
}

#[test]
fn cold_unknown_wrong_version_missing_grant_named_shape_and_equal_result_different_operation_are_denied()
 {
    fixture(|host, _, model| {
        for case in 0..6 {
            let mut bad = call(
                model,
                match case {
                    0 => "unknown",
                    1 => "counter.other",
                    _ => "counter.bump",
                },
                integer(2),
            );
            if let Node::Call { arguments, .. } = &mut bad {
                if case == 2 {
                    arguments.pop();
                }
                if case == 3 {
                    arguments[1].name = "unknown".into();
                }
            }
            let node = bind(
                "amount",
                field("n"),
                choose(
                    boolean(true),
                    call(model, "counter.bump", local("amount")),
                    bad,
                ),
            );
            let other = Host {
                version: if case == 4 { 8 } else { 7 },
                granted: if case == 5 { &[] } else { &[3] },
                ..*host
            };
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(50);
            let failure =
                prepare_atomic_call_in_scope(&node, &mut scope, &other, &mut fuel, LIMITS)
                    .unwrap_err();
            if case == 1 {
                assert!(matches!(
                    failure,
                    PreparedCallEvaluationError::Preflight(
                        PreparedCallTypeError::InconsistentOperation
                    )
                ));
            }
            assert_eq!(fuel.remaining(), 50);
            assert_eq!(scope.len(), 2);
            assert_eq!(model.clones.get(), 0);
            assert!(model.events.borrow().is_empty());
        }
    });
}

#[test]
fn pure_preparation_reuses_bounded_loop_recovery_and_lazy_cold_values() {
    fixture(|host, _, model| {
        let value = Node::Loop {
            name: "i".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Node::Binary {
                operator: BinaryOperator::Lt,
                left: Box::new(local("i")),
                right: Box::new(integer(2)),
            }),
            next: Box::new(Node::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(local("i")),
                right: Box::new(integer(1)),
            }),
            limit: 2,
        };
        let node = bind(
            "amount",
            value,
            choose(
                boolean(true),
                call(model, "counter.bump", local("amount")),
                call(model, "counter.bump", field("cold")),
            ),
        );
        let mut prefix = bindings(model);
        let mut scope = ScopeFrame::new(&mut prefix);
        let mut fuel = Fuel::new(100);
        let prepared =
            prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, LIMITS).unwrap();
        assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(2));
        assert_eq!(scope.len(), 2);
        assert_eq!(model.clones.get(), 0);
        assert_eq!(*model.events.borrow(), ["n-domain", "tag-domain"]);
        drop(scope);
        let node = bind(
            "amount",
            Node::Recover {
                value: Box::new(Node::Binary {
                    operator: BinaryOperator::Div,
                    left: Box::new(integer(1)),
                    right: Box::new(integer(0)),
                }),
                fallback: Box::new(integer(8)),
            },
            call(model, "counter.bump", local("amount")),
        );
        let mut scope = ScopeFrame::new(&mut prefix);
        let prepared =
            prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, LIMITS).unwrap();
        assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(8));
        assert_eq!(scope.len(), 2);
    });
}

#[test]
fn execution_failures_and_exhaustion_preserve_prefix_without_refunding_or_dispatch() {
    fixture(|host, _, model| {
        for case in 0..5 {
            let node = if case == 1 {
                bind("seed", integer(9), call(model, "counter.bump", integer(1)))
            } else if case == 2 {
                choose(
                    integer(1),
                    call(model, "counter.bump", integer(1)),
                    call(model, "counter.bump", integer(2)),
                )
            } else {
                flow(model, true)
            };
            let mut limits = LIMITS;
            if case == 3 {
                limits.pure.max_bindings = 2;
            }
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let start = if case == 0 {
                0
            } else if case == 4 {
                6
            } else {
                50
            };
            let mut fuel = Fuel::new(start);
            assert!(
                prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, limits).is_err()
            );
            assert_eq!(scope.len(), 2);
            assert!(matches!(
                scope.get("seed"),
                Some(PureValue::Scalar(ScalarValue::Integer(1)))
            ));
            assert_eq!(model.effects.get(), 0);
            if start > 0 {
                assert!(fuel.remaining() < start);
            }
            if case == 4 {
                assert_eq!(fuel.remaining(), 0);
            }
        }
    });
}

#[test]
fn native_projection_errors_remain_move_only_private_and_cannot_be_scalar_recovered() {
    fixture(|host, _, model| {
        model.project_failure.set(true);
        let node = bind(
            "amount",
            Node::Recover {
                value: Box::new(field("n")),
                fallback: Box::new(integer(9)),
            },
            call(model, "counter.bump", local("amount")),
        );
        let mut prefix = bindings(model);
        let mut scope = ScopeFrame::new(&mut prefix);
        let mut fuel = Fuel::new(50);
        let failure =
            prepare_atomic_call_in_scope(&node, &mut scope, host, &mut fuel, LIMITS).unwrap_err();
        assert!(!format!("{failure:?}").contains("private adapter details"));
        let PreparedCallEvaluationError::Execution(CalculationFailure::External(
            leselang_hir::effect_evaluation::EffectEvaluationFault::Pure(
                leselang_hir::pure_evaluation::PureEvaluationFault::Native(
                    PreparedCallNativeFault::Projection(Private(secret)),
                ),
            ),
        )) = failure
        else {
            panic!()
        };
        assert_eq!(secret, "private adapter details");
        assert_eq!(scope.len(), 2);
        assert_eq!(fuel.remaining(), 46);
        assert_eq!(*model.events.borrow(), ["n"]);
    });
}

#[test]
fn native_unwind_drops_temporary_bindings_without_capture_retry_or_fuel_reset() {
    fixture(|host, _, model| {
        for phase in ["projection", "domain"] {
            model.unwind.set(Some(phase));
            let node = bind("outer", integer(9), flow(model, true));
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(50);
            assert!(
                catch_unwind(AssertUnwindSafe(|| prepare_atomic_call_in_scope(
                    &node, &mut scope, host, &mut fuel, LIMITS
                )))
                .is_err()
            );
            assert_eq!(scope.len(), 2);
            assert!(scope.get("outer").is_none());
            assert!(scope.get("amount").is_none());
            assert!(fuel.remaining() < 50);
            assert_eq!(model.effects.get(), 0);
        }
    });
}

struct Input {
    n: u64,
    buffer: Box<u8>,
}
impl Borrow<u64> for Input {
    fn borrow(&self) -> &u64 {
        &self.n
    }
}
fn invoke(request: Call<'_>, model: &Model) -> Input {
    assert!(model.live.get());
    let ScalarValue::Integer(n) = request.arguments()[0].value else {
        panic!()
    };
    model.effects.set(model.effects.get() + 1);
    Input {
        n,
        buffer: Box::new(7),
    }
}

#[test]
fn prepared_bind_choose_members_reach_both_original_reply_protocols_without_a_second_meter() {
    fixture(|host, schemas, model| {
        for kind in [GroupKind::Sequence, GroupKind::Parallel] {
            model.effects.set(0);
            let group = Node::Group {
                group_kind: kind,
                branches: vec![
                    ComputedBranch {
                        name: "left".into(),
                        value: flow(model, true),
                        result_type: 0,
                    },
                    ComputedBranch {
                        name: "right".into(),
                        value: flow(model, false),
                        result_type: 0,
                    },
                ],
            };
            let Node::Group { branches, .. } = &group else {
                panic!()
            };
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            let prepared = prepare_native_group_members(
                kind,
                branches,
                64,
                &mut fuel,
                |branch| Ok::<_, ()>(branch.name.as_str()),
                |_| Ok(()),
                |_, branch, fuel| {
                    prepare_atomic_call_in_scope(&branch.value, &mut scope, host, fuel, LIMITS)
                        .map_err(|_| ())
                },
            )
            .unwrap();
            assert_eq!(fuel.remaining(), 79);
            assert_eq!(model.effects.get(), 0);
            let admit = |_: usize, branch: &ComputedBranch<Node, usize>, request: &Call<'_>| {
                assert!(std::ptr::eq(request.schema(), &schemas[branch.result_type]));
                Ok::<_, ()>(())
            };
            let correlate =
                |index, branch: &ComputedBranch<Node, usize>, request, fuel: &mut Fuel| {
                    fuel.charge(1).map_err(|_| ())?;
                    Ok::<_, ()>(PreparedParallelMember {
                        identity: (7, index),
                        declaration: &schemas[branch.result_type].result,
                        request,
                    })
                };
            if kind == GroupKind::Sequence {
                let mut session = PreparedSequenceSession::start(
                    &group, prepared, &mut fuel, GROUP, admit, correlate,
                )
                .unwrap();
                for index in 0..2 {
                    let SequencePoll::Request {
                        index: actual,
                        request,
                    } = session
                        .poll(|_, _, key, domain, request| {
                            model.authorize(key).map_err(|_| ())?;
                            assert!(std::ptr::eq(domain, &request.schema().result));
                            Ok::<_, ()>(())
                        })
                        .unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(actual, index);
                    let input = invoke(request, model);
                    let pointer = (&*input.buffer as *const u8) as usize;
                    let row = session
                        .try_accept(&(7, index), input, model.as_ref())
                        .unwrap();
                    let (actual, original, _, domain, input) = row.into_parts();
                    assert_eq!(actual, index);
                    assert!(std::ptr::eq(original, &branches[index]));
                    assert!(std::ptr::eq(domain, &schemas[0].result));
                    assert_eq!(input.n, [41, 2][index]);
                    assert_eq!((&*input.buffer as *const u8) as usize, pointer);
                }
            } else {
                let (mut session, requests) = PreparedParallelSession::start(
                    &group, prepared, &mut fuel, GROUP, admit, correlate,
                )
                .unwrap();
                for request in requests.into_iter().rev() {
                    let index = request.index;
                    let input = invoke(request.request, model);
                    session
                        .try_accept(index, &(7, index), input, model.as_ref())
                        .unwrap();
                }
                let ParallelPoll::Results(rows) = session.poll().unwrap() else {
                    panic!()
                };
                for (index, row) in rows.into_iter().enumerate() {
                    let (actual, original, _, domain, input) = row.into_parts();
                    assert_eq!(actual, index);
                    assert!(std::ptr::eq(original, &branches[index]));
                    assert!(std::ptr::eq(domain, &schemas[0].result));
                    assert_eq!(input.n, [41, 2][index]);
                }
            }
            assert_eq!(fuel.remaining(), 74);
            assert_eq!(scope.len(), 2);
            assert_eq!(model.effects.get(), 2);
        }
    });
}

const PREPARED_GROUP: PreparedGroupEvaluationLimits = PreparedGroupEvaluationLimits {
    call: LIMITS,
    max_branches: 64,
};
fn atomic_group(model: &Rc<Model>, kind: GroupKind) -> Node {
    Node::Group {
        group_kind: kind,
        branches: vec![
            ComputedBranch {
                name: "left".into(),
                value: flow(model, true),
                result_type: 0,
            },
            ComputedBranch {
                name: "right".into(),
                value: bind(
                    "amount",
                    field("n"),
                    choose(
                        boolean(false),
                        call(model, "counter.other", local("amount")),
                        call(model, "counter.other", integer(2)),
                    ),
                ),
                result_type: 1,
            },
        ],
    }
}
fn dimensions(node: &Node) -> (usize, usize) {
    let mut pending = vec![(node, 0)];
    let mut nodes = 0;
    let mut max_depth = 0;
    while let Some((node, depth)) = pending.pop() {
        nodes += 1;
        max_depth = max_depth.max(depth);
        pending.extend(node.children().map(|child| (child, depth + 1)));
    }
    (nodes, max_depth)
}

#[test]
fn complete_atomic_group_bridge_preserves_original_rows_scope_once_order_and_one_meter() {
    fixture(|host, schemas, model| {
        for kind in [GroupKind::Sequence, GroupKind::Parallel] {
            model.events.borrow_mut().clear();
            model.clones.set(0);
            let node = atomic_group(model, kind);
            let Node::Group { branches, .. } = &node else {
                panic!()
            };
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            let prepared = prepare_atomic_group_in_scope(
                &node,
                &mut scope,
                host,
                &mut fuel,
                PREPARED_GROUP,
                |index, branch, schema| {
                    assert!(std::ptr::eq(branch, &branches[index]));
                    assert!(std::ptr::eq(schema, &schemas[branch.result_type]));
                    model.events.borrow_mut().push(if index == 0 {
                        "admit-left"
                    } else {
                        "admit-right"
                    });
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(prepared.metadata().kind(), kind);
            assert_eq!(prepared.metadata().name(1), Some("right"));
            for (index, row) in prepared.members().iter().enumerate() {
                assert_eq!(row.index(), index);
                assert!(std::ptr::eq(row.original(), &branches[index]));
                assert!(std::ptr::eq(row.prepared().schema(), &schemas[index]));
                assert_eq!(
                    row.prepared().arguments()[0].value,
                    ScalarValue::Integer([41, 2][index])
                );
                assert_eq!(row.prepared().arguments()[1].parameter_index, 2);
                assert_eq!(row.prepared().arguments()[1].argument_index, 0);
            }
            assert_eq!(
                *model.events.borrow(),
                [
                    "admit-left",
                    "admit-right",
                    "n",
                    "n-domain",
                    "tag-domain",
                    "n",
                    "n-domain",
                    "tag-domain"
                ]
            );
            assert_eq!(fuel.remaining(), 79);
            assert_eq!(model.clones.get(), 2);
            assert_eq!(model.effects.get(), 0);
            assert_eq!(scope.len(), 2);
            assert!(scope.get("amount").is_none());
        }
    });
}

#[test]
fn aggregate_shifted_atomic_group_physical_and_scope_denials_precede_catalog_or_native_hooks() {
    for case in 0..11 {
        fixture(|host, _, model| {
            let mut node = atomic_group(model, GroupKind::Parallel);
            let (nodes, depth) = dimensions(&node);
            let mut limits = PREPARED_GROUP;
            match case {
                0 => limits.call.pure.max_nodes = nodes - 1,
                1 => limits.call.pure.max_depth = depth - 1,
                2 => limits.call.pure.max_nodes = 0,
                3 => limits.call.pure.max_bindings = 1,
                4 => limits.call.max_arguments = 65,
                _ => {
                    let Node::Group { branches, .. } = &mut node else {
                        panic!()
                    };
                    match case {
                        5 => branches[1].name = "left".into(),
                        6 => branches[1].name = "bad member".into(),
                        7 => {
                            branches.pop();
                        }
                        8 => branches[1].value = integer(0),
                        9 => branches[1].value = atomic_group(model, GroupKind::Sequence),
                        _ => {
                            branches[1].value = bind(
                                "effect",
                                call(model, "counter.bump", integer(1)),
                                call(model, "counter.bump", integer(2)),
                            )
                        }
                    }
                }
            }
            let admissions = Cell::new(0);
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            model.comparisons.set(0);
            assert!(
                prepare_atomic_group_in_scope(
                    &node,
                    &mut scope,
                    host,
                    &mut fuel,
                    limits,
                    |_, _, _| {
                        admissions.set(admissions.get() + 1);
                        Ok(())
                    }
                )
                .is_err()
            );
            assert_eq!(model.comparisons.get(), 0);
            assert_eq!(admissions.get(), 0);
            assert_eq!(model.clones.get(), 0);
            assert!(model.events.borrow().is_empty());
            assert_eq!(fuel.remaining(), 100);
            assert_eq!(scope.len(), 2);
        });
    }
}

#[test]
fn every_cold_atomic_group_catalog_signature_and_uniform_row_precedes_any_value_preparation() {
    for case in 0..6 {
        fixture(|host, _, model| {
            let mut node = atomic_group(model, GroupKind::Parallel);
            let Node::Group { branches, .. } = &mut node else {
                panic!()
            };
            match case {
                0 => branches[1].value = call(model, "counter.unknown", integer(2)),
                1 => {
                    branches[1].value = choose(
                        boolean(true),
                        call(model, "counter.bump", integer(1)),
                        call(model, "counter.other", integer(2)),
                    )
                }
                2 => {
                    branches[1].value = choose(
                        boolean(true),
                        call(model, "counter.other", integer(1)),
                        call(model, "counter.unknown", integer(2)),
                    )
                }
                3 => {
                    let mut call = call(model, "counter.other", integer(2));
                    let Node::Call { arguments, .. } = &mut call else {
                        panic!()
                    };
                    arguments[0].name = "n".into();
                    branches[1].value = call;
                }
                _ => {}
            }
            let other = Host {
                version: if case == 4 { 8 } else { 7 },
                granted: if case == 5 { &[] } else { &[3] },
                ..*host
            };
            let admissions = Cell::new(0);
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            let error = prepare_atomic_group_in_scope(
                &node,
                &mut scope,
                &other,
                &mut fuel,
                PREPARED_GROUP,
                |_, _, _| {
                    admissions.set(admissions.get() + 1);
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(
                matches!(error, PreparedGroupEvaluationError::Member { index: 1, .. }) || case >= 4
            );
            assert_eq!(admissions.get(), 0);
            assert_eq!(model.clones.get(), 0);
            assert!(model.events.borrow().is_empty());
            assert_eq!(model.effects.get(), 0);
            assert_eq!(fuel.remaining(), 100);
            assert_eq!(scope.len(), 2);
        });
    }
}

#[test]
fn inclusive_atomic_group_forest_and_sixty_four_member_limits_do_not_reset_per_member() {
    fixture(|host, schemas, model| {
        for kind in [GroupKind::Sequence, GroupKind::Parallel] {
            let node = Node::Group {
                group_kind: kind,
                branches: (0..64)
                    .map(|index| ComputedBranch {
                        name: format!("m{index}"),
                        value: call(model, "counter.bump", integer(1)),
                        result_type: 0,
                    })
                    .collect(),
            };
            let (nodes, depth) = dimensions(&node);
            assert_eq!((nodes, depth), (193, 2));
            let mut limits = PREPARED_GROUP;
            limits.call.pure.max_nodes = nodes;
            limits.call.pure.max_depth = depth;
            let mut prefix = Vec::new();
            let mut scope = ScopeFrame::new(&mut prefix);
            for denied in [false, true] {
                let mut current = limits;
                if denied {
                    current.call.pure.max_nodes -= 1;
                }
                let admissions = Cell::new(0);
                let mut fuel = Fuel::new(400);
                model.comparisons.set(0);
                let result = prepare_atomic_group_in_scope(
                    &node,
                    &mut scope,
                    host,
                    &mut fuel,
                    current,
                    |_, _, schema| {
                        assert!(std::ptr::eq(schema, &schemas[0]));
                        admissions.set(admissions.get() + 1);
                        Ok(())
                    },
                );
                if denied {
                    assert!(result.is_err());
                    assert_eq!(admissions.get(), 0);
                    assert_eq!(model.comparisons.get(), 0);
                    assert_eq!(fuel.remaining(), 400);
                } else {
                    let prepared = result.unwrap();
                    assert_eq!(prepared.members().len(), 64);
                    assert_eq!(prepared.metadata().name(63), Some("m63"));
                    assert_eq!(admissions.get(), 64);
                    assert_eq!(fuel.remaining(), 143);
                }
                assert_eq!(model.effects.get(), 0);
            }
        }
    });
}

#[test]
fn complete_atomic_group_native_admission_failure_or_unwind_precedes_fuel_and_projections() {
    for unwind in [false, true] {
        fixture(|host, _, model| {
            let node = atomic_group(model, GroupKind::Parallel);
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            let admissions = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                prepare_atomic_group_in_scope(
                    &node,
                    &mut scope,
                    host,
                    &mut fuel,
                    PREPARED_GROUP,
                    |index, _, _| {
                        admissions.set(admissions.get() + 1);
                        if index == 1 {
                            assert!(!unwind, "private native admission unwind");
                            return Err(Private("private late admission".into()));
                        }
                        Ok(())
                    },
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().unwrap_err();
                assert!(!format!("{error} {error:?}").contains("private"));
                assert!(std::error::Error::source(&error).is_none());
                let PreparedGroupEvaluationError::Preparation(
                    NativeGroupPreparationError::Admission(
                        leselang_hir::native_group::NativeGroupMemberError::Native {
                            member_index: 1,
                            error: PreparedGroupMemberError::Admission(Private(message)),
                            ..
                        },
                    ),
                ) = error
                else {
                    panic!()
                };
                assert_eq!(message, "private late admission");
            }
            assert_eq!(admissions.get(), 2);
            assert_eq!(model.clones.get(), 0);
            assert!(model.events.borrow().is_empty());
            assert_eq!(model.effects.get(), 0);
            assert_eq!(fuel.remaining(), 100);
            assert_eq!(scope.len(), 2);
        });
    }
}

#[test]
fn atomic_group_execution_exhaustion_returns_no_partial_batch_or_scope_leak_or_refund() {
    fixture(|host, _, model| {
        let node = atomic_group(model, GroupKind::Sequence);
        for start in 0..=21 {
            model.events.borrow_mut().clear();
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(start);
            let admissions = Cell::new(0);
            let result = prepare_atomic_group_in_scope(
                &node,
                &mut scope,
                host,
                &mut fuel,
                PREPARED_GROUP,
                |_, _, _| {
                    admissions.set(admissions.get() + 1);
                    Ok(())
                },
            );
            if start == 21 {
                assert_eq!(result.unwrap().members().len(), 2);
            } else {
                assert!(result.is_err());
            }
            assert_eq!(fuel.remaining(), 0);
            assert_eq!(admissions.get(), 2);
            assert_eq!(scope.len(), 2);
            assert!(scope.get("amount").is_none());
            assert_eq!(model.effects.get(), 0);
        }
    });
}

#[test]
fn late_atomic_group_domain_failure_or_unwind_cleans_prepared_prefix_and_temporary_views() {
    for unwind in [false, true] {
        fixture(|host, _, model| {
            let mut node = atomic_group(model, GroupKind::Parallel);
            let Node::Group { branches, .. } = &mut node else {
                panic!()
            };
            branches[1].value = call(
                model,
                "counter.other",
                if unwind { field("late") } else { integer(101) },
            );
            model.late_domain_unwind.set(unwind);
            let mut prefix = bindings(model);
            let mut scope = ScopeFrame::new(&mut prefix);
            let mut fuel = Fuel::new(100);
            let references = Rc::strong_count(model);
            let result = catch_unwind(AssertUnwindSafe(|| {
                prepare_atomic_group_in_scope(
                    &node,
                    &mut scope,
                    host,
                    &mut fuel,
                    PREPARED_GROUP,
                    |_, _, _| Ok(()),
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(Rc::strong_count(model), references);
            assert_eq!(fuel.remaining(), if unwind { 83 } else { 84 });
            assert_eq!(scope.len(), 2);
            assert!(scope.get("amount").is_none());
            assert_eq!(
                model
                    .events
                    .borrow()
                    .iter()
                    .filter(|event| **event == "n-domain")
                    .count(),
                2
            );
            assert_eq!(model.effects.get(), 0);
        });
    }
}
