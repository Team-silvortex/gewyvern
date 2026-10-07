use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits};
use leselang_hir::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError as Error, CallFlowTypeLimits, GroupFlowEnvironment,
    GroupFlowTypeLimits, GroupMemberType, GroupTypeError, HostFlowEnvironment, HostFlowTypeLimits,
    HostGroupFlowEnvironment, HostGroupFlowTypeLimits, infer_group_flow_type, infer_host_flow_type,
    infer_host_group_flow_type,
};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::prepared_typing::{
    PreparedCallSchemas, PreparedCallTypeError, SelectedPreparedCall,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::{
    NamedParameter, OperationCatalog, OperationCatalogError, OperationCatalogLimits,
    OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
};

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<&'static str>>,
    clones: Cell<usize>,
    panic_on: Cell<Option<&'static str>>,
    exported_operations: RefCell<Vec<usize>>,
    exported_names: RefCell<Vec<usize>>,
}
impl Trace {
    fn event(&self, name: &'static str) {
        self.events.borrow_mut().push(name);
        assert_ne!(self.panic_on.get(), Some(name), "private native work");
    }
}
#[derive(Clone, PartialEq)]
enum Shape<'e> {
    Receipt(u8),
    Group {
        mode: GroupKind,
        members: Rc<[(&'e str, &'e u32, u8)]>,
    },
}
struct Metadata<'e> {
    shape: Shape<'e>,
    trace: Rc<Trace>,
}
impl Clone for Metadata<'_> {
    fn clone(&self) -> Self {
        self.trace.clones.set(self.trace.clones.get() + 1);
        Self {
            shape: self.shape.clone(),
            trace: self.trace.clone(),
        }
    }
}
impl PartialEq for Metadata<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.shape == other.shape
    }
}
struct Native {
    operation: u32,
    declaration: u8,
    fake_operation: Option<u32>,
    graph_nodes: usize,
    live: bool,
    owner: Rc<Trace>,
}
type Ir = Computation<u8, u32, Native, u8>;
#[derive(Default)]
struct Environment<'e> {
    trace: Rc<Trace>,
    missing_operation: bool,
    missing_group: bool,
    life: PhantomData<&'e ()>,
}
impl<'e> Environment<'e> {
    fn receipt(&self, ty: u8) -> Metadata<'e> {
        Metadata {
            shape: Shape::Receipt(ty),
            trace: self.trace.clone(),
        }
    }
    fn host(&self, operation: u32) -> Ir {
        Ir::Host {
            effect: Box::new(Native {
                operation,
                declaration: if operation == 18 { 2 } else { 1 },
                fake_operation: None,
                graph_nodes: 4,
                live: true,
                owner: self.trace.clone(),
            }),
        }
    }
}
impl<'e> PureTypeEnvironment<u8, u32> for Environment<'e> {
    type Result = Metadata<'e>;
    fn field_type(&self, result: &Self::Result, field: &u8) -> Option<ScalarType> {
        self.trace.event("field");
        (matches!(result.shape, Shape::Receipt(1 | 2)) && *field == 7)
            .then_some(ScalarType::Integer)
    }
    fn member_result(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &u32,
    ) -> Option<Self::Result> {
        self.trace.event("member");
        let Shape::Group { members, .. } = &group.shape else {
            return None;
        };
        members
            .iter()
            .find(|(declared, key, _)| *declared == name && *key == operation)
            .map(|(_, _, ty)| self.receipt(*ty))
    }
}
impl<'e, 's> CallFlowEnvironment<'s, u8, u32, u8> for Environment<'e> {
    fn call_result_type(&self, _: &u32, declaration: &'s u8) -> Option<PureType<Self::Result>> {
        self.trace.event("call_type");
        Some(PureType::Result(self.receipt(*declaration)))
    }
}
impl<'e, 's> HostFlowEnvironment<'e, 's, u8, u32, u8, Native> for Environment<'e> {
    fn admit_host(&self, effect: &'e Native) -> bool {
        self.trace.event("host_admit");
        effect.live
            && effect.graph_nodes <= 4
            && Rc::ptr_eq(&effect.owner, &self.trace)
            && matches!(
                (effect.operation, effect.declaration),
                (17 | 19, 1) | (18, 2)
            )
    }
    fn host_result_type(&self, effect: &'e Native) -> Option<PureType<Self::Result>> {
        self.trace.event("host_type");
        Some(PureType::Result(self.receipt(effect.declaration)))
    }
}
impl<'e, 's> GroupFlowEnvironment<'e, 's, u8, u32, u8, u8> for Environment<'e> {
    fn group_branch_type_matches(
        &self,
        declaration: &u8,
        inferred: &PureType<Self::Result>,
    ) -> bool {
        self.trace.event("branch");
        matches!(inferred, PureType::Result(Metadata { shape: Shape::Receipt(ty), .. }) if declaration == ty)
    }
    fn group_result_type(
        &self,
        mode: GroupKind,
        members: &[GroupMemberType<'e, u32, Self::Result>],
    ) -> Option<Self::Result> {
        self.trace.event("group");
        if self.missing_group {
            return None;
        }
        let rows = members
            .iter()
            .map(|member| {
                self.trace
                    .exported_names
                    .borrow_mut()
                    .push(member.name.as_ptr() as usize);
                self.trace
                    .exported_operations
                    .borrow_mut()
                    .push(std::ptr::from_ref(member.operation) as usize);
                let PureType::Result(Metadata {
                    shape: Shape::Receipt(ty),
                    ..
                }) = &member.inferred
                else {
                    return None;
                };
                Some((member.name, member.operation, *ty))
            })
            .collect::<Option<Rc<[_]>>>()?;
        Some(Metadata {
            shape: Shape::Group {
                mode,
                members: rows,
            },
            trace: self.trace.clone(),
        })
    }
}
impl<'e, 's> HostGroupFlowEnvironment<'e, 's, u8, u32, u8, Native, u8> for Environment<'e> {
    fn host_group_operation(&self, effect: &'e Native) -> Option<&'e u32> {
        self.trace.event("host_key");
        if self.missing_operation {
            None
        } else {
            Some(effect.fake_operation.as_ref().unwrap_or(&effect.operation))
        }
    }
    fn admit_host_group_operation(
        &self,
        effect: &'e Native,
        operation: &'e u32,
        declaration: &'s u8,
    ) -> bool {
        self.trace.event("pair");
        std::ptr::eq(operation, &effect.operation) && *declaration == effect.declaration
    }
}
struct Schemas<'a, 's, 'e> {
    host: CallTypeHost<'a, 's, u32, ScalarTypeSet, u8, u8, Environment<'e>>,
}
impl<'s> PreparedCallSchemas<'s, u32> for Schemas<'_, 's, '_> {
    type Key = u32;
    type Domain = ScalarTypeSet;
    type Result = u8;
    type Capability = u8;
    fn select(
        &self,
        operation: &u32,
    ) -> Result<SelectedPreparedCall<'s, u32, Self>, OperationCatalogError> {
        self.host.environment.trace.event("schema");
        self.host.select(operation)
    }
}
const LIMITS: HostGroupFlowTypeLimits = HostGroupFlowTypeLimits {
    group: GroupFlowTypeLimits {
        flow: CallFlowTypeLimits {
            call: CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 128,
                    max_depth: 16,
                    max_bindings: 16,
                },
                max_arguments: 1,
            },
            max_calls: 16,
        },
        max_groups: 8,
        max_branches: 64,
    },
    max_hosts: 16,
};
fn with_schemas<'e, T>(
    env: &Environment<'e>,
    version: u32,
    grants: &[u8],
    f: impl FnOnce(&Schemas<'_, '_, 'e>) -> T,
) -> T {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 2,
            required_capability: 32,
        },
        OperationSchema {
            key: 19,
            parameters: &parameters,
            result: 1,
            required_capability: 31,
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 3,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    f(&Schemas {
        host: CallTypeHost {
            catalog: &catalog,
            version,
            granted: grants,
            environment: env,
        },
    })
}
fn check<'e>(
    expression: &'e Ir,
    prefix: &[(&'e str, PureType<Metadata<'e>>)],
    env: &Environment<'e>,
    limits: HostGroupFlowTypeLimits,
) -> Result<PureType<Metadata<'e>>, Error> {
    with_schemas(env, 9, &[31, 32], |schemas| {
        infer_host_group_flow_type(expression, prefix, env, schemas, limits)
    })
}
fn number() -> Ir {
    Ir::Literal {
        value: ScalarValue::Integer(7),
    }
}
fn boolean() -> Ir {
    Ir::Literal {
        value: ScalarValue::Boolean(false),
    }
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(value: Ir) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field: 7,
    }
}
fn member(name: &str, operation: u32) -> Ir {
    Ir::Member {
        group: "g".into(),
        name: name.into(),
        operation,
    }
}
fn call(operation: u32, value: Ir) -> Ir {
    Ir::Call {
        operation,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value,
        }],
    }
}
fn bind(name: &str, value: Ir, body: Ir) -> Ir {
    Ir::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn choose(then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(boolean()),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn branch(name: &str, value: Ir, result_type: u8) -> ComputedBranch<Ir, u8> {
    ComputedBranch {
        name: name.into(),
        value,
        result_type,
    }
}
fn group(mode: GroupKind, branches: Vec<ComputedBranch<Ir, u8>>) -> Ir {
    Ir::Group {
        group_kind: mode,
        branches,
    }
}

#[test]
fn mixed_sequence_parallel_exports_borrow_original_names_and_host_call_operations() {
    for mode in [GroupKind::Sequence, GroupKind::Parallel] {
        let env = Environment::default();
        let value = group(
            mode,
            vec![
                branch("native", env.host(17), 1),
                branch("computed", call(18, number()), 2),
            ],
        );
        let Ir::Group { branches, .. } = &value else {
            unreachable!()
        };
        let Ir::Host { effect } = &branches[0].value else {
            unreachable!()
        };
        let Ir::Call { operation, .. } = &branches[1].value else {
            unreachable!()
        };
        let operations = [
            std::ptr::from_ref(&effect.operation) as usize,
            std::ptr::from_ref(operation) as usize,
        ];
        let names = branches
            .iter()
            .map(|b| b.name.as_ptr() as usize)
            .collect::<Vec<_>>();
        let expression = bind("g", value, field(member("native", 17)));
        assert!(matches!(
            check(&expression, &[], &env, LIMITS),
            Ok(PureType::Scalar(ScalarType::Integer))
        ));
        assert_eq!(*env.trace.exported_operations.borrow(), operations);
        assert_eq!(*env.trace.exported_names.borrow(), names);
        assert_eq!(
            *env.trace.events.borrow(),
            [
                "schema",
                "host_admit",
                "host_key",
                "schema",
                "pair",
                "host_type",
                "branch",
                "call_type",
                "branch",
                "group",
                "member",
                "field"
            ]
        );
    }
}

#[test]
fn pure_preparation_and_conditional_host_call_paths_require_one_original_schema_row() {
    let env = Environment::default();
    let value = bind(
        "position",
        number(),
        choose(env.host(17), call(17, local("position"))),
    );
    let expression = group(GroupKind::Sequence, vec![branch("move", value, 1)]);
    assert!(check(&expression, &[], &env, LIMITS).is_ok());
    assert_eq!(
        env.trace
            .events
            .borrow()
            .iter()
            .filter(|e| **e == "host_key")
            .count(),
        1
    );
    let expression = group(
        GroupKind::Sequence,
        vec![branch("move", choose(env.host(17), call(19, number())), 1)],
    );
    let prefix = [("unused", PureType::Result(env.receipt(1)))];
    let before = env.trace.clones.get();
    assert!(matches!(
        check(&expression, &prefix, &env, LIMITS),
        Err(Error::Group {
            group_index: 0,
            error: GroupTypeError::InconsistentOperation { branch_index: 0 }
        })
    ));
    assert_eq!(env.trace.clones.get(), before);
}

#[test]
fn whole_cold_shape_and_all_call_schemas_precede_native_mapping_and_type_queries() {
    let env = Environment::default();
    let expression = group(
        GroupKind::Sequence,
        vec![
            branch("first", env.host(17), 1),
            branch("cold", call(99, number()), 1),
        ],
    );
    let prefix = [("unused", PureType::Result(env.receipt(1)))];
    assert!(matches!(
        check(&expression, &prefix, &env, LIMITS),
        Err(Error::Call {
            call_index: 0,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    ));
    assert_eq!(*env.trace.events.borrow(), ["schema"]);
    assert_eq!(env.trace.clones.get(), 0);
    env.trace.events.borrow_mut().clear();
    let expression = group(
        GroupKind::Sequence,
        vec![
            branch("first", env.host(17), 1),
            branch("cold", field(env.host(17)), 1),
        ],
    );
    assert!(check(&expression, &prefix, &env, LIMITS).is_err());
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn native_pair_admission_rejects_equal_lookalike_keys_foreign_results_and_missing_mapping() {
    for fake in [17, 18] {
        let env = Environment::default();
        let mut host = env.host(17);
        let Ir::Host { effect } = &mut host else {
            unreachable!()
        };
        effect.fake_operation = Some(fake);
        let expression = group(GroupKind::Sequence, vec![branch("native", host, 1)]);
        assert!(matches!(
            check(&expression, &[], &env, LIMITS),
            Err(Error::Group {
                error: GroupTypeError::HostDeclaration {
                    branch_index: 0,
                    host_index: 0
                },
                ..
            })
        ));
        assert!(!env.trace.events.borrow().contains(&"host_type"));
    }
    let env = Environment {
        missing_operation: true,
        ..Default::default()
    };
    let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Group {
            error: GroupTypeError::MissingOperation {
                branch_index: 0,
                host_index: 0
            },
            ..
        })
    ));
}

#[test]
fn cold_native_row_version_grants_and_parameter_ceilings_fail_before_prefix_clones() {
    for (version, grants, expected) in [
        (8, vec![31], OperationCatalogError::UnsupportedVersion),
        (9, vec![32], OperationCatalogError::CapabilityDenied),
    ] {
        let env = Environment::default();
        let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
        let prefix = [("unused", PureType::Result(env.receipt(1)))];
        let result = with_schemas(&env, version, &grants, |schemas| {
            infer_host_group_flow_type(&expression, &prefix, &env, schemas, LIMITS)
        });
        assert!(
            matches!(result, Err(Error::Group { error: GroupTypeError::HostSchema { error: CallTypeError::Catalog(error), .. }, .. }) if error == expected)
        );
        assert_eq!(env.trace.clones.get(), 0);
        assert_eq!(
            *env.trace.events.borrow(),
            ["host_admit", "host_key", "schema"]
        );
    }
    let env = Environment::default();
    let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
    let mut limits = LIMITS;
    limits.group.flow.call.max_arguments = 0;
    assert!(matches!(
        check(&expression, &[], &env, limits),
        Err(Error::Group {
            error: GroupTypeError::HostSchema {
                error: CallTypeError::ParameterLimit,
                ..
            },
            ..
        })
    ));
}

#[test]
fn physical_group_host_call_member_and_depth_budgets_remain_inclusive_and_explicit() {
    let env = Environment::default();
    let expression = group(
        GroupKind::Parallel,
        vec![
            branch("native", env.host(17), 1),
            branch("computed", call(18, number()), 2),
        ],
    );
    let mut exact = LIMITS;
    exact.group.flow.call.pure.max_nodes = 4;
    exact.group.flow.call.pure.max_depth = 2;
    exact.group.max_groups = 1;
    exact.group.max_branches = 2;
    exact.group.flow.max_calls = 1;
    exact.max_hosts = 1;
    assert!(check(&expression, &[], &env, exact).is_ok());
    for which in 0..6 {
        env.trace.events.borrow_mut().clear();
        let mut limits = exact;
        match which {
            0 => limits.max_hosts = 0,
            1 => limits.group.flow.max_calls = 0,
            2 => limits.group.max_groups = 0,
            3 => limits.group.max_branches = 1,
            4 => limits.group.flow.call.pure.max_nodes = 3,
            5 => limits.group.flow.call.pure.max_depth = 1,
            _ => unreachable!(),
        }
        assert!(check(&expression, &[], &env, limits).is_err());
        assert!(env.trace.events.borrow().is_empty());
    }
    let mut limits = exact;
    limits.max_hosts = 16_385;
    assert!(matches!(
        check(&expression, &[], &env, limits),
        Err(Error::InvalidLimits)
    ));
}

#[test]
fn captured_preparation_nested_groups_scalar_exits_and_cold_impure_guards_stay_rejected() {
    let env = Environment::default();
    for value in [
        bind("r", env.host(17), call(17, number())),
        group(GroupKind::Sequence, vec![branch("nested", env.host(17), 1)]),
        choose(env.host(17), number()),
        Ir::Choose {
            when: Box::new(env.host(17)),
            then: Box::new(env.host(17)),
            otherwise: Box::new(env.host(17)),
        },
    ] {
        let expression = group(GroupKind::Sequence, vec![branch("move", value, 1)]);
        assert!(matches!(
            check(&expression, &[], &env, LIMITS),
            Err(Error::Group {
                error: GroupTypeError::MemberPreparation { .. },
                ..
            })
        ));
    }
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn group_member_pure_initializers_and_cold_call_positions_do_not_count_host_leaves_as_calls() {
    let env = Environment::default();
    let bad_call = Ir::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value: env.host(17),
        }],
    };
    let expression = group(
        GroupKind::Sequence,
        vec![branch("move", choose(env.host(17), bad_call), 1)],
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Call {
            call_index: 0,
            error: CallTypeError::Inference {
                error: PureTypeError::Impure,
                ..
            }
        })
    ));
    let expression = group(
        GroupKind::Sequence,
        vec![branch(
            "move",
            bind("n", env.host(17), call(17, number())),
            1,
        )],
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Group {
            error: GroupTypeError::MemberPreparation {
                error: PreparedCallTypeError::Preparation(PureTypeError::Impure),
                ..
            },
            ..
        })
    ));
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn sibling_members_and_previous_sequence_receipts_never_become_preparation_prefixes() {
    let env = Environment::default();
    let expression = group(
        GroupKind::Sequence,
        vec![
            branch("first", bind("n", number(), env.host(17)), 1),
            branch("next", call(17, local("n")), 1),
        ],
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Call {
            error: CallTypeError::Inference {
                error: PureTypeError::UnknownLocal,
                ..
            },
            ..
        })
    ));
    let expression = group(
        GroupKind::Sequence,
        vec![
            branch("first", env.host(17), 1),
            branch("next", call(17, local("first")), 1),
        ],
    );
    assert!(check(&expression, &[], &env, LIMITS).is_err());
    assert!(!env.trace.events.borrow().contains(&"group"));
}

#[test]
fn branch_declarations_missing_group_types_and_export_keys_remain_closed() {
    let env = Environment::default();
    let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 2)]);
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Group {
            error: GroupTypeError::BranchType { branch_index: 0 },
            ..
        })
    ));
    assert!(!env.trace.events.borrow().contains(&"group"));
    let env = Environment {
        missing_group: true,
        ..Default::default()
    };
    assert!(matches!(
        check(
            &group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]),
            &[],
            &env,
            LIMITS
        ),
        Err(Error::Group {
            error: GroupTypeError::ResultType,
            ..
        })
    ));
    let env = Environment::default();
    for (name, operation) in [("missing", 17), ("native", 19)] {
        let expression = bind(
            "g",
            group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]),
            field(member(name, operation)),
        );
        assert!(check(&expression, &[], &env, LIMITS).is_err());
    }
}

#[test]
fn existing_call_group_and_opaque_leaf_entries_do_not_silently_admit_mixed_groups() {
    let env = Environment::default();
    let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
    with_schemas(&env, 9, &[31, 32], |schemas| {
        assert!(infer_group_flow_type(&expression, &[], &env, schemas, LIMITS.group).is_err());
        assert!(matches!(
            infer_host_flow_type(
                &expression,
                &[],
                &env,
                schemas,
                HostFlowTypeLimits {
                    flow: LIMITS.group.flow,
                    max_hosts: 16
                }
            ),
            Err(Error::UnsupportedFlow)
        ));
    });
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn native_mapping_pair_declaration_and_construction_unwind_never_publish_or_retry_members() {
    for phase in ["host_key", "pair", "host_type", "branch", "group"] {
        let env = Environment::default();
        let expression = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
        let prefix = [("unused", PureType::Result(env.receipt(1)))];
        let before = Rc::strong_count(&env.trace);
        env.trace.panic_on.set(Some(phase));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(
                &expression,
                &prefix,
                &env,
                LIMITS
            )))
            .is_err()
        );
        assert_eq!(Rc::strong_count(&env.trace), before);
        assert_eq!(
            env.trace
                .events
                .borrow()
                .iter()
                .filter(|event| **event == phase)
                .count(),
            1
        );
        assert_eq!(prefix[0].0, "unused");
        env.trace.panic_on.set(None);
        assert!(check(&expression, &prefix, &env, LIMITS).is_ok());
    }
}

#[test]
fn hidden_graph_and_mutated_policy_need_fresh_admission_and_errors_never_echo_payloads() {
    let env = Environment::default();
    let mut expression = group(
        GroupKind::Sequence,
        vec![branch("secret-native", env.host(17), 1)],
    );
    assert!(check(&expression, &[], &env, LIMITS).is_ok());
    let Ir::Group { branches, .. } = &mut expression else {
        unreachable!()
    };
    let Ir::Host { effect } = &mut branches[0].value else {
        unreachable!()
    };
    effect.graph_nodes = 5;
    let error = check(&expression, &[], &env, LIMITS).err().unwrap();
    assert_eq!(format!("{error:?}"), "HostAdmission { host_index: 0 }");
    assert!(!error.to_string().contains("secret-native"));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn host_and_call_cursors_stay_ordered_across_surrounding_captures_and_multiple_groups() {
    let env = Environment::default();
    let first = group(GroupKind::Sequence, vec![branch("native", env.host(17), 1)]);
    let second = group(
        GroupKind::Parallel,
        vec![
            branch("native", choose(env.host(18), call(18, number())), 2),
            branch("computed", call(17, number()), 1),
        ],
    );
    let expression = bind(
        "before",
        env.host(17),
        bind(
            "first",
            first,
            bind(
                "g",
                second,
                bind(
                    "after",
                    call(17, field(member("native", 18))),
                    field(local("before")),
                ),
            ),
        ),
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    ));
    assert_eq!(
        env.trace
            .events
            .borrow()
            .iter()
            .filter(|event| **event == "group")
            .count(),
        2
    );
    assert_eq!(
        env.trace
            .events
            .borrow()
            .iter()
            .filter(|event| **event == "host_key")
            .count(),
        2
    );
    assert_eq!(
        env.trace
            .events
            .borrow()
            .iter()
            .filter(|event| **event == "host_admit")
            .count(),
        3
    );
}

#[test]
fn unrelated_gui_profile_borrows_nonclone_operation_effect_and_result_declarations() {
    struct Field;
    struct Operation {
        name: &'static str,
        private: Rc<()>,
    }
    struct Declaration(u8);
    struct NativeEffect {
        operation: Operation,
        declaration: Declaration,
    }
    struct Gui {
        owner: Rc<()>,
        exported: Cell<usize>,
    }
    impl PureTypeEnvironment<Field, Operation> for Gui {
        type Result = u8;
        fn field_type(&self, _: &u8, _: &Field) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &u8, _: &str, _: &Operation) -> Option<u8> {
            None
        }
    }
    impl<'s> CallFlowEnvironment<'s, Field, Operation, Declaration> for Gui {
        fn call_result_type(
            &self,
            _: &Operation,
            declaration: &'s Declaration,
        ) -> Option<PureType<u8>> {
            Some(PureType::Result(declaration.0))
        }
    }
    impl<'e, 's> HostFlowEnvironment<'e, 's, Field, Operation, Declaration, NativeEffect> for Gui {
        fn admit_host(&self, effect: &'e NativeEffect) -> bool {
            Rc::ptr_eq(&effect.operation.private, &self.owner)
        }
        fn host_result_type(&self, effect: &'e NativeEffect) -> Option<PureType<u8>> {
            Some(PureType::Result(effect.declaration.0))
        }
    }
    impl<'e, 's> GroupFlowEnvironment<'e, 's, Field, Operation, Declaration, Declaration> for Gui {
        fn group_branch_type_matches(
            &self,
            declaration: &Declaration,
            inferred: &PureType<u8>,
        ) -> bool {
            *inferred == PureType::Result(declaration.0)
        }
        fn group_result_type(
            &self,
            _: GroupKind,
            members: &[GroupMemberType<'e, Operation, u8>],
        ) -> Option<u8> {
            self.exported
                .set(std::ptr::from_ref(members[0].operation) as usize);
            Some(9)
        }
    }
    impl<'e, 's>
        HostGroupFlowEnvironment<'e, 's, Field, Operation, Declaration, NativeEffect, Declaration>
        for Gui
    {
        fn host_group_operation(&self, effect: &'e NativeEffect) -> Option<&'e Operation> {
            Some(&effect.operation)
        }
        fn admit_host_group_operation(
            &self,
            effect: &'e NativeEffect,
            operation: &'e Operation,
            declaration: &'s Declaration,
        ) -> bool {
            std::ptr::eq(operation, &effect.operation) && declaration.0 == effect.declaration.0
        }
    }
    struct Schemas<'s> {
        row: &'s OperationSchema<'s, &'s str, &'s str, ScalarTypeSet, Declaration, ()>,
    }
    impl<'s> PreparedCallSchemas<'s, Operation> for Schemas<'s> {
        type Key = &'s str;
        type Domain = ScalarTypeSet;
        type Result = Declaration;
        type Capability = ();
        fn select(
            &self,
            operation: &Operation,
        ) -> Result<SelectedPreparedCall<'s, Operation, Self>, OperationCatalogError> {
            if operation.name == self.row.key {
                Ok(self.row)
            } else {
                Err(OperationCatalogError::UnknownOperation)
            }
        }
    }
    let gui = Gui {
        owner: Rc::new(()),
        exported: Cell::new(0),
    };
    let effect = Box::new(NativeEffect {
        operation: Operation {
            name: "click",
            private: gui.owner.clone(),
        },
        declaration: Declaration(1),
    });
    let original = std::ptr::from_ref(&effect.operation) as usize;
    let expression: Computation<Field, Operation, NativeEffect, Declaration> = Computation::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "button".into(),
            value: Computation::Host { effect },
            result_type: Declaration(1),
        }],
    };
    let row = OperationSchema {
        key: "click",
        parameters: &[],
        result: Declaration(1),
        required_capability: (),
    };
    assert_eq!(
        infer_host_group_flow_type(&expression, &[], &gui, &Schemas { row: &row }, LIMITS).unwrap(),
        PureType::Result(9)
    );
    assert_eq!(gui.exported.get(), original);
}
