use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits};
use leselang_hir::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError as Error, CallFlowTypeLimits, GroupFlowEnvironment,
    GroupFlowTypeLimits, GroupMemberType, HostFlowEnvironment, HostFlowTypeLimits,
    infer_call_flow_type, infer_group_flow_type, infer_host_flow_type,
};
use leselang_hir::ir::{Computation, ComputedArgument, GroupKind};
use leselang_hir::prepared_typing::{PreparedCallSchemas, SelectedPreparedCall};
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
    admitted: RefCell<Vec<usize>>,
    queried: RefCell<Vec<usize>>,
    clones: Cell<usize>,
    panic_phase: Cell<Option<&'static str>>,
}
impl Trace {
    fn record(&self, phase: &'static str) {
        self.events.borrow_mut().push(phase);
        assert_ne!(self.panic_phase.get(), Some(phase), "private native hook");
    }
}
struct Tag {
    id: u8,
    trace: Rc<Trace>,
}
impl Clone for Tag {
    fn clone(&self) -> Self {
        self.trace.clones.set(self.trace.clones.get() + 1);
        Self {
            id: self.id,
            trace: self.trace.clone(),
        }
    }
}
impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
// Native effects and type identifiers deliberately have no Debug or wire codec.
struct Native {
    operation: u32,
    declaration: u8,
    hidden_nodes: usize,
    version: u32,
    granted: bool,
    owner: Rc<Trace>,
}
type Ir = Computation<u8, u32, Native, ()>;
#[derive(Default)]
struct Environment {
    trace: Rc<Trace>,
    join_common: bool,
    missing_type: bool,
}
impl Environment {
    fn tag(&self, id: u8) -> Tag {
        Tag {
            id,
            trace: self.trace.clone(),
        }
    }
    fn native(&self, operation: u32) -> Native {
        Native {
            operation,
            declaration: if operation == 17 { 1 } else { 2 },
            hidden_nodes: 4,
            version: 9,
            granted: true,
            owner: self.trace.clone(),
        }
    }
    fn host(&self, operation: u32) -> Ir {
        Ir::Host {
            effect: Box::new(self.native(operation)),
        }
    }
}
impl PureTypeEnvironment<u8, u32> for Environment {
    type Result = Tag;
    fn field_type(&self, result: &Tag, field: &u8) -> Option<ScalarType> {
        self.trace.record("field");
        ((*field == 7 && matches!(result.id, 1..=3)) || (*field == 8 && result.id == 1))
            .then_some(ScalarType::Integer)
    }
    fn member_result(&self, group: &Tag, name: &str, operation: &u32) -> Option<Tag> {
        self.trace.record("member");
        (group.id == 9 && name == "move" && *operation == 17).then(|| self.tag(1))
    }
    fn join_results(&self, left: Tag, right: Tag) -> Option<Tag> {
        self.trace.record("join");
        if left == right {
            Some(left)
        } else {
            self.join_common.then(|| self.tag(3))
        }
    }
}
impl<'s> CallFlowEnvironment<'s, u8, u32, u8> for Environment {
    fn call_result_type(&self, _: &u32, declaration: &'s u8) -> Option<PureType<Tag>> {
        self.trace.record("call_type");
        Some(PureType::Result(self.tag(*declaration)))
    }
}
impl<'e, 's> HostFlowEnvironment<'e, 's, u8, u32, u8, Native> for Environment {
    fn admit_host(&self, effect: &'e Native) -> bool {
        self.trace.record("admit");
        self.trace
            .admitted
            .borrow_mut()
            .push(std::ptr::from_ref(effect) as usize);
        Rc::ptr_eq(&effect.owner, &self.trace)
            && effect.hidden_nodes <= 4
            && effect.version == 9
            && effect.granted
            && matches!((effect.operation, effect.declaration), (17, 1) | (18, 2))
    }
    fn host_result_type(&self, effect: &'e Native) -> Option<PureType<Tag>> {
        self.trace.record("host_type");
        self.trace
            .queried
            .borrow_mut()
            .push(std::ptr::from_ref(effect) as usize);
        (!self.missing_type).then(|| PureType::Result(self.tag(effect.declaration)))
    }
}
impl<'e, 's> GroupFlowEnvironment<'e, 's, u8, u32, u8, ()> for Environment {
    fn group_branch_type_matches(&self, _: &(), _: &PureType<Tag>) -> bool {
        unreachable!()
    }
    fn group_result_type(&self, _: GroupKind, _: &[GroupMemberType<'e, u32, Tag>]) -> Option<Tag> {
        unreachable!()
    }
}
struct Schemas<'a, 's> {
    host: CallTypeHost<'a, 's, u32, ScalarTypeSet, u8, u8, Environment>,
}
impl<'s> PreparedCallSchemas<'s, u32> for Schemas<'_, 's> {
    type Key = u32;
    type Domain = ScalarTypeSet;
    type Result = u8;
    type Capability = u8;
    fn select(
        &self,
        operation: &u32,
    ) -> Result<SelectedPreparedCall<'s, u32, Self>, OperationCatalogError> {
        self.host.environment.trace.record("schema");
        self.host.select(operation)
    }
}
const LIMITS: HostFlowTypeLimits = HostFlowTypeLimits {
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
    max_hosts: 16,
};
fn with_schemas<T>(
    env: &Environment,
    version: u32,
    grants: &[u8],
    f: impl FnOnce(&Schemas<'_, '_>) -> T,
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
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 2,
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
    prefix: &[(&'e str, PureType<Tag>)],
    env: &Environment,
    limits: HostFlowTypeLimits,
) -> Result<PureType<Tag>, Error> {
    with_schemas(env, 9, &[31, 32], |schemas| {
        infer_host_flow_type(expression, prefix, env, schemas, limits)
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
fn field(value: Ir, field: u8) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field,
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
fn choose(when: Ir, then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(when),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}

#[test]
fn original_host_capture_and_call_successor_share_one_lexical_type_chain() {
    let env = Environment::default();
    let host = env.host(17);
    let Ir::Host { effect } = &host else {
        unreachable!()
    };
    let original = std::ptr::from_ref(effect.as_ref()) as usize;
    let expression = bind(
        "receipt",
        host,
        bind(
            "next",
            call(18, field(local("receipt"), 7)),
            field(local("next"), 7),
        ),
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    ));
    assert_eq!(
        *env.trace.events.borrow(),
        [
            "schema",
            "admit",
            "host_type",
            "field",
            "call_type",
            "field"
        ]
    );
    assert_eq!(*env.trace.admitted.borrow(), [original]);
    assert_eq!(*env.trace.queried.borrow(), [original]);
}

#[test]
fn cold_native_admission_precedes_prefix_clone_and_guard_or_field_queries() {
    let env = Environment::default();
    let mut invalid = env.native(17);
    invalid.granted = false;
    let expression = choose(
        boolean(),
        bind("r", env.host(17), field(local("r"), 7)),
        Ir::Host {
            effect: Box::new(invalid),
        },
    );
    let prefix = [("unused", PureType::Result(env.tag(1)))];
    assert!(matches!(
        check(&expression, &prefix, &env, LIMITS),
        Err(Error::HostAdmission { host_index: 1 })
    ));
    assert_eq!(*env.trace.events.borrow(), ["admit", "admit"]);
    assert_eq!(env.trace.clones.get(), 0);
}

#[test]
fn all_cold_call_policy_checks_precede_opaque_admission_and_semantic_queries() {
    for (operation, version, grants, expected) in [
        (99, 9, vec![31, 32], OperationCatalogError::UnknownOperation),
        (
            18,
            8,
            vec![31, 32],
            OperationCatalogError::UnsupportedVersion,
        ),
        (18, 9, vec![31], OperationCatalogError::CapabilityDenied),
    ] {
        let env = Environment::default();
        let expression = choose(boolean(), env.host(17), call(operation, number()));
        let prefix = [("unused", PureType::Result(env.tag(1)))];
        let result = with_schemas(&env, version, &grants, |schemas| {
            infer_host_flow_type(&expression, &prefix, &env, schemas, LIMITS)
        });
        assert!(
            matches!(result, Err(Error::Call { call_index: 0, error: CallTypeError::Catalog(error) }) if error == expected)
        );
        assert_eq!(*env.trace.events.borrow(), ["schema"]);
        assert_eq!(env.trace.clones.get(), 0);
    }
    let env = Environment::default();
    let expression = choose(
        boolean(),
        env.host(17),
        Ir::Call {
            operation: 18,
            arguments: vec![],
        },
    );
    assert!(matches!(
        check(&expression, &[], &env, LIMITS),
        Err(Error::Call {
            error: CallTypeError::Names(_),
            ..
        })
    ));
    assert_eq!(*env.trace.events.borrow(), ["schema"]);
}

#[test]
fn complete_physical_limits_prefix_and_names_are_checked_before_any_native_hook() {
    for variant in 0..6 {
        let env = Environment::default();
        let mut limits = LIMITS;
        let mut expression = choose(boolean(), env.host(17), call(18, number()));
        let prefix = [("unused", PureType::Result(env.tag(1)))];
        match variant {
            0 => limits.flow.call.pure.max_nodes = 4,
            1 => limits.flow.call.pure.max_depth = 1,
            2 => limits.flow.call.pure.max_bindings = 0,
            3 => limits.max_hosts = 16_385,
            4 => expression = bind("not.valid", env.host(17), boolean()),
            5 => limits.flow.max_calls = 0,
            _ => unreachable!(),
        }
        assert!(check(&expression, &prefix, &env, limits).is_err());
        assert!(env.trace.events.borrow().is_empty());
        assert_eq!(env.trace.clones.get(), 0);
    }
}

#[test]
fn host_counts_and_language_node_depth_limits_are_inclusive_and_zero_denies() {
    let env = Environment::default();
    let expression = choose(boolean(), env.host(17), env.host(17));
    let mut limits = LIMITS;
    limits.max_hosts = 2;
    limits.flow.call.pure.max_nodes = 4;
    limits.flow.call.pure.max_depth = 1;
    assert!(matches!(
        check(&expression, &[], &env, limits),
        Ok(PureType::Result(Tag { id: 1, .. }))
    ));
    for count in [0, 1] {
        limits.max_hosts = count;
        assert!(matches!(
            check(&expression, &[], &env, limits),
            Err(Error::HostLimit)
        ));
    }
    limits.max_hosts = 1;
    limits.flow.call.pure.max_nodes = 1;
    limits.flow.call.pure.max_depth = 0;
    assert!(check(&env.host(17), &[], &env, limits).is_ok());
    limits.flow.call.pure.max_nodes = 0;
    assert!(matches!(
        check(&env.host(17), &[], &env, limits),
        Err(Error::Structure(_))
    ));
}

#[test]
fn maximum_recursive_depth_and_invalid_prefixes_are_checked_without_native_work() {
    let env = Environment::default();
    let mut expression = env.host(17);
    for index in 0..64 {
        expression = bind(&format!("n{index}"), number(), expression);
    }
    let mut limits = LIMITS;
    limits.flow.call.pure.max_nodes = 131;
    limits.flow.call.pure.max_depth = 64;
    limits.flow.call.pure.max_bindings = 65;
    assert!(check(&expression, &[], &env, limits).is_ok());
    env.trace.events.borrow_mut().clear();
    expression = bind("extra", number(), expression);
    assert!(matches!(
        check(&expression, &[], &env, limits),
        Err(Error::Structure(_) | Error::Pure(PureTypeError::Structure(_)))
    ));
    assert!(env.trace.events.borrow().is_empty());
    for prefix in [
        vec![("not.valid", PureType::Result(env.tag(1)))],
        vec![
            ("same", PureType::Result(env.tag(1))),
            ("same", PureType::Result(env.tag(2))),
        ],
    ] {
        assert!(matches!(
            check(&env.host(17), &prefix, &env, LIMITS),
            Err(Error::Pure(PureTypeError::InvalidScope))
        ));
    }
    assert!(env.trace.events.borrow().is_empty());
    assert_eq!(env.trace.clones.get(), 0);
}

#[test]
fn hidden_native_graph_foreign_owner_schema_version_and_grants_need_explicit_admission() {
    let env = Environment::default();
    for variant in 0..5 {
        let mut effect = env.native(17);
        match variant {
            0 => effect.hidden_nodes = 5,
            1 => effect.owner = Rc::default(),
            2 => effect.declaration = 2,
            3 => effect.version = 8,
            4 => effect.granted = false,
            _ => unreachable!(),
        }
        assert!(matches!(
            check(
                &Ir::Host {
                    effect: Box::new(effect)
                },
                &[],
                &env,
                LIMITS
            ),
            Err(Error::HostAdmission { host_index: 0 })
        ));
    }
    assert_eq!(*env.trace.events.borrow(), ["admit"; 5]);
}

#[test]
fn guards_call_arguments_and_projection_loop_fold_recovery_operands_stay_pure_even_cold() {
    let env = Environment::default();
    let expressions = [
        choose(env.host(17), number(), number()),
        call(17, env.host(17)),
        field(env.host(17), 7),
        Ir::Recover {
            value: Box::new(number()),
            fallback: Box::new(env.host(17)),
        },
        Ir::Loop {
            name: "n".into(),
            initial: Box::new(number()),
            condition: Box::new(boolean()),
            next: Box::new(env.host(17)),
            limit: 0,
        },
        Ir::Fold {
            name: "n".into(),
            item: "s".into(),
            items: Box::new(Ir::Strings { items: vec![] }),
            initial: Box::new(number()),
            next: Box::new(env.host(17)),
            limit: 0,
        },
    ];
    for expression in expressions {
        assert!(check(&expression, &[], &env, LIMITS).is_err());
    }
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn host_call_joins_retain_only_common_exports_and_do_not_grant_atomic_execution() {
    let env = Environment {
        join_common: true,
        ..Default::default()
    };
    for (field_name, accepted) in [(7, true), (8, false)] {
        let expression = bind(
            "r",
            choose(boolean(), env.host(17), call(18, number())),
            field(local("r"), field_name),
        );
        let result = check(&expression, &[], &env, LIMITS);
        if accepted {
            assert!(matches!(result, Ok(PureType::Scalar(ScalarType::Integer))));
        } else {
            assert!(matches!(
                result,
                Err(Error::Pure(PureTypeError::FieldNotExported))
            ));
        }
    }
    let exact_only = Environment::default();
    assert!(matches!(
        check(
            &choose(boolean(), exact_only.host(17), call(18, number())),
            &[],
            &exact_only,
            LIMITS
        ),
        Err(Error::BranchTypes)
    ));
    assert!(matches!(
        check(
            &choose(number(), env.host(17), env.host(17)),
            &[],
            &env,
            LIMITS
        ),
        Err(Error::ConditionType)
    ));
}

#[test]
fn lexical_sibling_alias_shadow_forward_and_active_binding_limits_are_preserved() {
    let env = Environment::default();
    let branch = || bind("r", env.host(17), field(local("r"), 7));
    assert!(check(&choose(boolean(), branch(), branch()), &[], &env, LIMITS).is_ok());
    assert!(matches!(
        check(
            &choose(boolean(), branch(), field(local("r"), 7)),
            &[],
            &env,
            LIMITS
        ),
        Err(Error::Pure(PureTypeError::UnknownLocal))
    ));
    assert!(matches!(
        check(
            &bind("r", env.host(17), bind("r", env.host(17), number())),
            &[],
            &env,
            LIMITS
        ),
        Err(Error::Pure(PureTypeError::ShadowedBinding))
    ));
    assert!(matches!(
        check(
            &bind("r", call(17, field(local("r"), 7)), boolean()),
            &[],
            &env,
            LIMITS
        ),
        Err(Error::Call {
            error: CallTypeError::Inference {
                error: PureTypeError::UnknownLocal,
                ..
            },
            ..
        })
    ));
    assert!(
        check(
            &bind(
                "r",
                env.host(17),
                bind("alias", local("r"), field(local("alias"), 7))
            ),
            &[],
            &env,
            LIMITS
        )
        .is_ok()
    );
    let mut limits = LIMITS;
    limits.flow.call.pure.max_bindings = 1;
    assert!(matches!(
        check(
            &bind("r", env.host(17), bind("alias", local("r"), number())),
            &[],
            &env,
            limits
        ),
        Err(Error::Pure(PureTypeError::BindingLimit))
    ));
}

#[test]
fn exact_large_prefix_ceiling_and_active_growth_are_not_residual_small_product_limits() {
    let env = Environment::default();
    let names = (0..1_024).map(|n| format!("v{n}")).collect::<Vec<_>>();
    let prefix = names
        .iter()
        .map(|name| (name.as_str(), PureType::<Tag>::Scalar(ScalarType::Integer)))
        .collect::<Vec<_>>();
    let mut limits = LIMITS;
    limits.flow.call.pure.max_bindings = 1_024;
    assert!(check(&env.host(17), &prefix, &env, limits).is_ok());
    assert!(matches!(
        check(&bind("new", env.host(17), boolean()), &prefix, &env, limits),
        Err(Error::Pure(PureTypeError::BindingLimit))
    ));
    limits.flow.call.pure.max_bindings = 1_025;
    assert!(matches!(
        check(&env.host(17), &[], &env, limits),
        Err(Error::Pure(PureTypeError::InvalidLimits))
    ));
}

#[test]
fn call_only_and_group_entries_still_reject_hosts_and_host_entry_rejects_new_groups() {
    let env = Environment::default();
    let host = env.host(17);
    with_schemas(&env, 9, &[31, 32], |schemas| {
        assert!(matches!(
            infer_call_flow_type(&host, &[], &env, schemas, LIMITS.flow),
            Err(Error::UnsupportedFlow)
        ));
        assert!(matches!(
            infer_group_flow_type(
                &host,
                &[],
                &env,
                schemas,
                GroupFlowTypeLimits {
                    flow: LIMITS.flow,
                    max_groups: 1,
                    max_branches: 1
                }
            ),
            Err(Error::UnsupportedFlow)
        ));
    });
    let group = Ir::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![],
    };
    assert!(matches!(
        check(&group, &[], &env, LIMITS),
        Err(Error::UnsupportedFlow)
    ));
    assert!(env.trace.events.borrow().is_empty());
}

#[test]
fn native_hook_unwind_drops_temporary_scope_without_prefix_mutation_or_retry() {
    for phase in ["admit", "host_type", "field"] {
        let env = Environment::default();
        let expression = bind("r", env.host(17), field(local("r"), 7));
        let prefix = [("prior", PureType::Result(env.tag(1)))];
        let before = Rc::strong_count(&env.trace);
        env.trace.panic_phase.set(Some(phase));
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
        assert_eq!(prefix[0].0, "prior");
        assert_eq!(
            env.trace
                .events
                .borrow()
                .iter()
                .filter(|event| **event == phase)
                .count(),
            1
        );
        env.trace.panic_phase.set(None);
        assert!(check(&expression, &prefix, &env, LIMITS).is_ok());
    }
}

#[test]
fn missing_type_and_recheck_after_payload_mutation_use_closed_redacted_failures() {
    let env = Environment {
        missing_type: true,
        ..Default::default()
    };
    assert!(matches!(
        check(&env.host(17), &[], &env, LIMITS),
        Err(Error::HostResultType { host_index: 0 })
    ));
    let env = Environment::default();
    let mut expression = env.host(17);
    assert!(check(&expression, &[], &env, LIMITS).is_ok());
    let Ir::Host { effect } = &mut expression else {
        unreachable!()
    };
    effect.granted = false;
    let error = match check(&expression, &[], &env, LIMITS) {
        Err(error) => error,
        Ok(_) => panic!(),
    };
    assert_eq!(format!("{error:?}"), "HostAdmission { host_index: 0 }");
    assert_eq!(error.to_string(), "opaque host type admission rejected");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn externally_declared_members_keep_exact_exported_operation_identity() {
    let env = Environment::default();
    let prefix = [("panel", PureType::Result(env.tag(9)))];
    for (operation, accepted) in [(17, true), (18, false)] {
        let expression = bind(
            "ignored",
            env.host(17),
            field(
                Ir::Member {
                    group: "panel".into(),
                    name: "move".into(),
                    operation,
                },
                7,
            ),
        );
        assert_eq!(check(&expression, &prefix, &env, LIMITS).is_ok(), accepted);
    }
}

#[test]
fn unrelated_gui_host_uses_nonclone_slots_and_original_owned_declaration_identity() {
    struct GuiField;
    struct GuiOperation(Rc<()>);
    struct GuiEffect {
        owner: Rc<()>,
        declaration: Rc<GuiDeclaration>,
    }
    struct GuiDeclaration;
    impl PartialEq for GuiDeclaration {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self, other)
        }
    }
    struct Gui {
        owner: Rc<()>,
        declaration: Rc<GuiDeclaration>,
        admissions: Cell<usize>,
        queries: Cell<usize>,
    }
    impl PureTypeEnvironment<GuiField, GuiOperation> for Gui {
        type Result = Rc<GuiDeclaration>;
        fn field_type(&self, result: &Self::Result, _: &GuiField) -> Option<ScalarType> {
            Rc::ptr_eq(result, &self.declaration).then_some(ScalarType::Boolean)
        }
        fn member_result(
            &self,
            _: &Self::Result,
            _: &str,
            operation: &GuiOperation,
        ) -> Option<Self::Result> {
            Rc::ptr_eq(&operation.0, &self.owner).then(|| self.declaration.clone())
        }
    }
    impl<'s> CallFlowEnvironment<'s, GuiField, GuiOperation, ()> for Gui {
        fn call_result_type(&self, _: &GuiOperation, _: &'s ()) -> Option<PureType<Self::Result>> {
            None
        }
    }
    impl<'e, 's> HostFlowEnvironment<'e, 's, GuiField, GuiOperation, (), GuiEffect> for Gui {
        fn admit_host(&self, effect: &'e GuiEffect) -> bool {
            self.admissions.set(self.admissions.get() + 1);
            Rc::ptr_eq(&effect.owner, &self.owner)
                && Rc::ptr_eq(&effect.declaration, &self.declaration)
        }
        fn host_result_type(&self, effect: &'e GuiEffect) -> Option<PureType<Self::Result>> {
            self.queries.set(self.queries.get() + 1);
            Some(PureType::Result(effect.declaration.clone()))
        }
    }
    struct NoCalls;
    impl<'s> PreparedCallSchemas<'s, GuiOperation> for NoCalls {
        type Key = ();
        type Domain = ScalarTypeSet;
        type Result = ();
        type Capability = ();
        fn select(
            &self,
            _: &GuiOperation,
        ) -> Result<SelectedPreparedCall<'s, GuiOperation, Self>, OperationCatalogError> {
            Err(OperationCatalogError::UnknownOperation)
        }
    }
    let gui = Gui {
        owner: Rc::new(()),
        declaration: Rc::new(GuiDeclaration),
        admissions: Cell::new(0),
        queries: Cell::new(0),
    };
    let expression: Computation<GuiField, GuiOperation, GuiEffect, GuiDeclaration> =
        Computation::Bind {
            name: "panel".into(),
            value: Box::new(Computation::Host {
                effect: Box::new(GuiEffect {
                    owner: gui.owner.clone(),
                    declaration: gui.declaration.clone(),
                }),
            }),
            body: Box::new(Computation::Local {
                name: "panel".into(),
            }),
        };
    let result = infer_host_flow_type(&expression, &[], &gui, &NoCalls, LIMITS)
        .ok()
        .unwrap();
    let PureType::Result(result) = result else {
        panic!()
    };
    assert!(Rc::ptr_eq(&result, &gui.declaration));
    assert_eq!(gui.admissions.get(), 1);
    assert_eq!(gui.queries.get(), 1);
}

#[test]
fn result_metadata_can_borrow_the_original_nonclone_opaque_declaration() {
    struct Declaration {
        private: String,
    }
    impl PartialEq for Declaration {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self, other)
        }
    }
    struct NativeEffect {
        declaration: Box<Declaration>,
    }
    struct Borrowing<'e>(std::marker::PhantomData<&'e Declaration>);
    impl<'e> PureTypeEnvironment<(), u32> for Borrowing<'e> {
        type Result = &'e Declaration;
        fn field_type(&self, _: &&'e Declaration, _: &()) -> Option<ScalarType> {
            None
        }
        fn member_result(&self, _: &&'e Declaration, _: &str, _: &u32) -> Option<Self::Result> {
            None
        }
    }
    impl<'e, 's> CallFlowEnvironment<'s, (), u32, ()> for Borrowing<'e> {
        fn call_result_type(&self, _: &u32, _: &'s ()) -> Option<PureType<Self::Result>> {
            None
        }
    }
    impl<'e, 's> HostFlowEnvironment<'e, 's, (), u32, (), NativeEffect> for Borrowing<'e> {
        fn admit_host(&self, effect: &'e NativeEffect) -> bool {
            effect.declaration.private.len() <= 16
        }
        fn host_result_type(&self, effect: &'e NativeEffect) -> Option<PureType<Self::Result>> {
            Some(PureType::Result(effect.declaration.as_ref()))
        }
    }
    struct NoCalls;
    impl<'s> PreparedCallSchemas<'s, u32> for NoCalls {
        type Key = ();
        type Domain = ScalarTypeSet;
        type Result = ();
        type Capability = ();
        fn select(
            &self,
            _: &u32,
        ) -> Result<SelectedPreparedCall<'s, u32, Self>, OperationCatalogError> {
            Err(OperationCatalogError::UnknownOperation)
        }
    }
    let declaration = Box::new(Declaration {
        private: "original".into(),
    });
    let original = std::ptr::from_ref(declaration.as_ref());
    let expression: Computation<(), u32, NativeEffect, ()> = Computation::Host {
        effect: Box::new(NativeEffect { declaration }),
    };
    let result = infer_host_flow_type(
        &expression,
        &[],
        &Borrowing(std::marker::PhantomData),
        &NoCalls,
        LIMITS,
    )
    .ok()
    .unwrap();
    let PureType::Result(result) = result else {
        panic!()
    };
    assert_eq!(std::ptr::from_ref(result), original);
}
