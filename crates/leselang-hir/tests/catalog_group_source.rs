use leselang_hir::{
    call_evaluation::{CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope},
    catalog_group_source::*,
    catalog_program::{CatalogProgramLimits, compile_catalog_program},
    catalog_source::{
        CatalogSourceEnvironment, CatalogSourceLimits, CatalogSourceResult, compile_catalog_source,
    },
    control_source::ControlSourceLimits,
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationFault, EffectEvaluationLimits,
        evaluate_resumable_effects_in_scope,
    },
    effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings, ResumableEffectOutcome},
    effect_session::{
        CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
        EffectSessionPoll, EffectSessionStatus,
    },
    effect_stack::EffectStackLimits,
    effect_stack_session::StackedEffectSession,
    flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::{PreparedNativeGroup, prepare_native_group_members},
    parallel_evaluation::{
        ParallelAccepted, ParallelEnd, ParallelEvaluationLimits, ParallelPoll, ParallelRequest,
        PreparedParallelMember,
    },
    prepared_parallel::PreparedParallelSession,
    prepared_sequence::{PreparedSequenceError, PreparedSequenceSession},
    pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, scalar_copy_cost,
    },
    pure_typing::{PureType, PureTypeEnvironment},
    sequence_evaluation::{SequenceEnd, SequencePoll, SequenceStatus},
    source_call::{SourceCallHost, SourceCallLimits},
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, SyntaxTree, parse};
use std::{
    borrow::Borrow,
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Normal,
    WrongOperation,
    WrongDeclaration,
    BranchError,
    BranchUnwind,
    GroupNone,
    UnstableGroup,
}
#[derive(Default)]
struct Trace {
    mode: Cell<Mode>,
    maps: Cell<usize>,
    branches: Cell<usize>,
    groups: Cell<usize>,
    keys_dropped: Cell<usize>,
    declarations_dropped: Cell<usize>,
}
struct Private(&'static str);
struct Field;
struct Opaque;
struct Key {
    label: &'static str,
    owner: Rc<Trace>,
    owned: bool,
}
impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.label
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && Rc::ptr_eq(&self.owner, &other.owner)
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        if self.owned {
            self.owner
                .keys_dropped
                .set(self.owner.keys_dropped.get() + 1);
        }
    }
}
struct Domain {
    owner: Rc<Trace>,
}
struct Declaration<'a> {
    original: &'a Domain,
    owner: Rc<Trace>,
}
impl Drop for Declaration<'_> {
    fn drop(&mut self) {
        self.owner
            .declarations_dropped
            .set(self.owner.declarations_dropped.get() + 1);
    }
}
struct Member<'a> {
    name: String,
    key: &'static str,
    domain: &'a Domain,
}
struct Signature<'a> {
    kind: GroupKind,
    members: Vec<Member<'a>>,
    revision: usize,
}
#[derive(Clone)]
enum Meta<'a> {
    Record(&'a Domain),
    Group(Rc<Signature<'a>>),
}
impl PartialEq for Meta<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Record(a), Self::Record(b)) => std::ptr::eq(*a, *b),
            (Self::Group(a), Self::Group(b)) => {
                a.kind == b.kind
                    && a.revision == b.revision
                    && a.members.len() == b.members.len()
                    && a.members.iter().zip(&b.members).all(|(a, b)| {
                        a.name == b.name && a.key == b.key && std::ptr::eq(a.domain, b.domain)
                    })
            }
            _ => false,
        }
    }
}
type Node<'a> = Computation<Field, Key, Opaque, Declaration<'a>>;
type Host<'h, 's> = SourceCallHost<'h, 's, Key, ScalarTypeSet, Domain, u8>;
struct Environment<'s> {
    owner: Rc<Trace>,
    foreign: &'s Domain,
}
impl<'s> PureTypeEnvironment<Field, Key> for Environment<'s> {
    type Result = Meta<'s>;
    fn field_type(&self, result: &Meta<'s>, _: &Field) -> Option<ScalarType> {
        matches!(result, Meta::Record(domain) if Rc::ptr_eq(&domain.owner, &self.owner))
            .then_some(ScalarType::Integer)
    }
    fn member_result(&self, result: &Meta<'s>, name: &str, operation: &Key) -> Option<Meta<'s>> {
        let Meta::Group(signature) = result else {
            return None;
        };
        if !Rc::ptr_eq(&operation.owner, &self.owner) {
            return None;
        }
        signature
            .members
            .iter()
            .find(|member| member.name == name && member.key == operation.label)
            .map(|member| Meta::Record(member.domain))
    }
}
impl<'s> CallFlowEnvironment<'s, Field, Key, Domain> for Environment<'s> {
    fn call_result_type(&self, key: &Key, domain: &'s Domain) -> Option<PureType<Meta<'s>>> {
        (Rc::ptr_eq(&key.owner, &self.owner) && Rc::ptr_eq(&domain.owner, &self.owner))
            .then_some(PureType::Result(Meta::Record(domain)))
    }
}
impl<'source, 's> CatalogSourceEnvironment<'source, 's, Field, Key, Domain> for Environment<'s> {
    type Error = Private;
    fn source_operation(&self, original: &Key) -> Result<Key, Private> {
        self.owner.maps.set(self.owner.maps.get() + 1);
        Ok(Key {
            label: if self.owner.mode.get() == Mode::WrongOperation {
                "counter.other"
            } else {
                original.label
            },
            owner: original.owner.clone(),
            owned: true,
        })
    }
    fn source_field(
        &self,
        result: &Meta<'s>,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Private> {
        Ok(if name == "count" {
            self.field_type(result, &Field).map(|ty| (Field, ty))
        } else {
            None
        })
    }
    fn source_member(
        &self,
        result: &Meta<'s>,
        _: &'source str,
        name: &'source str,
    ) -> Result<Option<(Key, Meta<'s>)>, Private> {
        let Meta::Group(signature) = result else {
            return Ok(None);
        };
        Ok(signature
            .members
            .iter()
            .find(|member| member.name == name)
            .map(|member| {
                (
                    Key {
                        label: member.key,
                        owner: self.owner.clone(),
                        owned: true,
                    },
                    Meta::Record(member.domain),
                )
            }))
    }
}
impl<'e, 's> GroupFlowEnvironment<'e, 's, Field, Key, Domain, Declaration<'s>> for Environment<'s> {
    fn group_branch_type_matches(
        &self,
        declared: &Declaration<'s>,
        inferred: &PureType<Meta<'s>>,
    ) -> bool {
        Rc::ptr_eq(&declared.owner, &self.owner)
            && matches!(inferred, PureType::Result(Meta::Record(domain)) if std::ptr::eq(*domain, declared.original))
    }
    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'e, Key, Meta<'s>>],
    ) -> Option<Meta<'s>> {
        self.owner.groups.set(self.owner.groups.get() + 1);
        if self.owner.mode.get() == Mode::GroupNone {
            return None;
        }
        let mut rows = Vec::new();
        for member in members {
            let PureType::Result(Meta::Record(domain)) = &member.inferred else {
                return None;
            };
            if !Rc::ptr_eq(&member.operation.owner, &self.owner) {
                return None;
            }
            rows.push(Member {
                name: member.name.into(),
                key: member.operation.label,
                domain,
            });
        }
        Some(Meta::Group(Rc::new(Signature {
            kind,
            members: rows,
            revision: if self.owner.mode.get() == Mode::UnstableGroup {
                self.owner.groups.get()
            } else {
                0
            },
        })))
    }
}
impl<'source, 's> CatalogGroupSourceEnvironment<'source, 's, Field, Key, Domain, Declaration<'s>>
    for Environment<'s>
{
    fn source_group_branch(
        &self,
        _: &'source NamedArgument,
        _: &Key,
        declaration: &'s Domain,
        _: &PureType<Meta<'s>>,
    ) -> Result<Declaration<'s>, Private> {
        self.owner.branches.set(self.owner.branches.get() + 1);
        if self.owner.branches.get() == 2 {
            assert!(
                self.owner.mode.get() != Mode::BranchUnwind,
                "private native branch unwind"
            );
            if self.owner.mode.get() == Mode::BranchError {
                return Err(Private("private branch payload"));
            }
        }
        Ok(Declaration {
            original: if self.owner.mode.get() == Mode::WrongDeclaration {
                self.foreign
            } else {
                declaration
            },
            owner: self.owner.clone(),
        })
    }
}
const LIMITS: CatalogGroupSourceLimits = CatalogGroupSourceLimits {
    expression: CatalogSourceLimits {
        control: ControlSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 256,
                max_source_depth: 32,
                max_lowered_nodes: 256,
                max_lowered_depth: 32,
                max_arguments: 64,
            },
            max_bindings: 8,
        },
        max_calls: 16,
    },
    max_groups: 4,
    max_branches: 64,
};
fn expression(text: &str) -> Expression {
    let tree = parse(&format!("fn main() = {text}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn fixture(test: impl for<'host, 'schema> FnOnce(&Host<'host, 'schema>, &Environment<'schema>)) {
    let owner = Rc::new(Trace::default());
    let parameters = [NamedParameter::required(
        "n",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [
        OperationSchema {
            key: Key {
                label: "counter.left",
                owner: owner.clone(),
                owned: false,
            },
            parameters: &parameters,
            required_capability: 31,
            result: Domain {
                owner: owner.clone(),
            },
        },
        OperationSchema {
            key: Key {
                label: "counter.right",
                owner: owner.clone(),
                owned: false,
            },
            parameters: &parameters,
            required_capability: 32,
            result: Domain {
                owner: owner.clone(),
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
    let foreign = Domain {
        owner: Rc::new(Trace::default()),
    };
    let host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[31, 32],
    };
    let env = Environment {
        owner,
        foreign: &foreign,
    };
    test(&host, &env);
}
fn compile<'s>(
    text: &Expression,
    limits: CatalogGroupSourceLimits,
    host: &Host<'_, 's>,
    env: &Environment<'s>,
) -> CatalogGroupSourceResult<Node<'s>, Meta<'s>, Private> {
    compile_catalog_group_source(text, &[], limits, host, env)
}
fn nodes(node: &Node<'_>) -> usize {
    1 + node.children().map(nodes).sum::<usize>()
}
fn private_error(error: &CatalogGroupSourceError<Private>) -> Option<&'static str> {
    use leselang_hir::{control_source::ControlSourceError, group_source::GroupSourceError};
    match error {
        CatalogGroupSourceError::BranchMapping { error, .. } => Some(error.0),
        CatalogGroupSourceError::Control(error) => match error.as_ref() {
            ControlSourceError::Native { error, .. } => private_error(error),
            _ => None,
        },
        CatalogGroupSourceError::Group(error) => match error.as_ref() {
            GroupSourceError::Native { error, .. } => private_error(error),
            _ => None,
        },
        _ => None,
    }
}
const BATCH: &str = "all(left: counter.left(n: 42), right: counter.right(n: 7))";

#[test]
fn complete_group_control_expression_preserves_native_declarations_without_slot_copies() {
    fixture(|host, env| {
        let source = expression(
            "bind(seed: 40, body: bind(batch: all(left: counter.left(n: add(left: seed, right: 2)), right: counter.right(n: 7)), body: bind(selected: member(value: batch, name: \"left\"), body: choose(when: gt(left: field(value: selected, name: \"count\"), right: 40), then: field(value: selected, name: \"count\"), otherwise: 0))))",
        );
        let (node, ty) = compile(&source, LIMITS, host, env).unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
        let Node::Bind { body, .. } = &node else {
            panic!()
        };
        let Node::Bind { value, .. } = body.as_ref() else {
            panic!()
        };
        let Node::Group {
            group_kind,
            branches,
        } = value.as_ref()
        else {
            panic!()
        };
        assert_eq!(*group_kind, GroupKind::Parallel);
        for (index, branch) in branches.iter().enumerate() {
            let key = if index == 0 {
                "counter.left"
            } else {
                "counter.right"
            };
            assert!(std::ptr::eq(
                branch.result_type.original,
                &host.catalog.lookup(key, 7).unwrap().result
            ));
        }
        assert_eq!(env.owner.branches.get(), 2);
        assert_eq!(env.owner.maps.get(), 2);
        drop(node);
        assert_eq!(env.owner.declarations_dropped.get(), 2);
        assert_eq!(env.owner.keys_dropped.get(), 3);
    });
}

#[test]
fn sequential_and_parallel_groups_keep_closed_mode_and_original_member_metadata() {
    for (text, kind, count) in [
        ("seq(one: counter.left(n: 1))", GroupKind::Sequence, 1),
        (BATCH, GroupKind::Parallel, 2),
    ] {
        fixture(|host, env| {
            let source = expression(text);
            let (node, ty) = compile(&source, LIMITS, host, env).unwrap();
            let PureType::Result(Meta::Group(signature)) = ty else {
                panic!()
            };
            assert_eq!(signature.kind, kind);
            assert_eq!(signature.members.len(), count);
            assert_eq!(nodes(&node), 1 + 2 * count);
        });
    }
}

#[test]
fn every_cold_group_shape_and_member_profile_passes_before_any_native_mapping() {
    for text in [
        "all()",
        "all(one: counter.left(n: 1))",
        "all(one: counter.left(n: 1), one: counter.right(n: 2))",
        "all(left: 1, right: counter.right(n: 2))",
        "all(left: choose(when: true, then: counter.left(n: 1), otherwise: counter.left(n: 2)), right: counter.right(n: 2))",
        "all(left: seq(one: counter.left(n: 1)), right: counter.right(n: 2))",
        "all(left: counter.left(n: seq(one: counter.left(n: 1))), right: counter.right(n: 2))",
        "choose(when: true, then: counter.left(n: 1), otherwise: all(one: counter.right(n: 2)))",
        "all(left: counter.left(n: 1), right: counter.right(n: left))",
        "repeat(times: 2, body: counter.left(n: 1))",
    ] {
        fixture(|host, env| {
            let source = expression(text);
            assert!(
                compile(&source, LIMITS, host, env).is_err(),
                "accepted {text}"
            );
            assert_eq!(env.owner.maps.get(), 0, "mapped before cold gate: {text}");
            assert_eq!(env.owner.branches.get(), 0);
            assert_eq!(env.owner.groups.get(), 0);
        });
    }
}

#[test]
fn all_cold_catalog_signatures_versions_and_grants_precede_group_slot_mapping() {
    for text in [
        "all(left: counter.left(n: 1), right: counter.unknown(n: 2))",
        "all(left: counter.left(n: 1), right: counter.right(other: 2))",
        "all(left: counter.left(n: 1), right: missing_helper(n: 2))",
    ] {
        fixture(|host, env| {
            assert!(compile(&expression(text), LIMITS, host, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
            assert_eq!(env.owner.branches.get(), 0);
        });
    }
    for bad_version in [false, true] {
        fixture(|host, env| {
            let bad = SourceCallHost {
                catalog: host.catalog,
                version: if bad_version { 8 } else { 7 },
                granted: &[31],
            };
            assert!(compile(&expression(BATCH), LIMITS, &bad, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
        });
    }
}

#[test]
fn group_call_and_whole_expression_limits_are_inclusive_without_per_leaf_reset() {
    fixture(|host, env| {
        let source = expression(
            "bind(batch: all(left: counter.left(n: 1), right: counter.right(n: 2)), body: 0)",
        );
        let mut exact = LIMITS;
        exact.max_groups = 1;
        exact.max_branches = 2;
        exact.expression.max_calls = 2;
        exact.expression.control.source.max_lowered_nodes = 7;
        let (node, _) = compile(&source, exact, host, env).unwrap();
        assert_eq!(nodes(&node), 7);
        exact.expression.control.source.max_lowered_nodes = 6;
        assert!(compile(&source, exact, host, env).is_err());
    });
    for case in 0..5 {
        fixture(|host, env| {
            let mut limits = LIMITS;
            match case {
                0 => limits.max_groups = 0,
                1 => limits.max_branches = 1,
                2 => limits.expression.max_calls = 1,
                3 => limits.max_groups = 16_385,
                _ => limits.max_branches = 65,
            }
            assert!(compile(&expression(BATCH), limits, host, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
        });
    }
    fixture(|host, env| {
        let source = expression(
            "choose(when: true, then: seq(one: counter.left(n: 1)), otherwise: seq(one: counter.left(n: 2)))",
        );
        assert!(
            compile(
                &source,
                CatalogGroupSourceLimits {
                    max_groups: 1,
                    ..LIMITS
                },
                host,
                env
            )
            .is_err()
        );
        assert_eq!(env.owner.maps.get(), 0);
        let (node, _) = compile(
            &source,
            CatalogGroupSourceLimits {
                max_groups: 2,
                ..LIMITS
            },
            host,
            env,
        )
        .unwrap();
        assert_eq!(nodes(&node), 8);
    });
}

#[test]
fn maximum_group_width_retains_one_shifted_output_forest_and_original_declarations() {
    let text = format!(
        "all({})",
        (0..64)
            .map(|index| format!("task_{index}: counter.left(n: 1)"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    fixture(|host, env| {
        let source = expression(&text);
        let mut limits = LIMITS;
        limits.max_groups = 1;
        limits.expression.max_calls = 64;
        limits.expression.control.source.max_lowered_nodes = 129;
        let (node, ty) = compile(&source, limits, host, env).unwrap();
        assert_eq!(nodes(&node), 129);
        let PureType::Result(Meta::Group(signature)) = ty else {
            panic!()
        };
        assert_eq!(signature.members.len(), 64);
        let original = &host.catalog.lookup("counter.left", 7).unwrap().result;
        assert!(
            signature
                .members
                .iter()
                .all(|member| std::ptr::eq(member.domain, original))
        );
        assert_eq!(env.owner.maps.get(), 64);
        assert_eq!(env.owner.branches.get(), 64);
        drop(node);
        assert_eq!(env.owner.keys_dropped.get(), 64);
        assert_eq!(env.owner.declarations_dropped.get(), 64);
    });
    fixture(|host, env| {
        let source = expression(&text);
        let mut limits = LIMITS;
        limits.expression.max_calls = 64;
        limits.expression.control.source.max_lowered_nodes = 128;
        assert!(compile(&source, limits, host, env).is_err());
        assert_eq!(env.owner.groups.get(), 0);
        assert_eq!(
            env.owner.declarations_dropped.get(),
            env.owner.branches.get()
        );
    });
}

#[test]
fn mismatched_operation_branch_declaration_and_unstable_group_observation_fail_closed() {
    for mode in [
        Mode::WrongOperation,
        Mode::WrongDeclaration,
        Mode::GroupNone,
        Mode::UnstableGroup,
    ] {
        fixture(|host, env| {
            env.owner.mode.set(mode);
            let result = compile(&expression(BATCH), LIMITS, host, env);
            assert!(result.is_err());
            let error = result.err().unwrap();
            assert!(!format!("{error:?}").contains("private"));
            assert!(std::error::Error::source(&error).is_none());
            if mode == Mode::WrongDeclaration {
                assert_eq!(env.owner.groups.get(), 0);
            }
        });
    }
}

#[test]
fn native_branch_error_or_unwind_releases_owned_prefix_once_without_partial_output() {
    for mode in [Mode::BranchError, Mode::BranchUnwind] {
        fixture(|host, env| {
            env.owner.mode.set(mode);
            let source = expression(BATCH);
            let result = catch_unwind(AssertUnwindSafe(|| compile(&source, LIMITS, host, env)));
            if mode == Mode::BranchUnwind {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().err().unwrap();
                assert_eq!(private_error(&error), Some("private branch payload"));
                assert!(!format!("{error} {error:?}").contains("private branch payload"));
            }
            assert_eq!(env.owner.maps.get(), 2);
            assert_eq!(env.owner.branches.get(), 2);
            assert_eq!(env.owner.groups.get(), 0);
            assert_eq!(env.owner.keys_dropped.get(), 2);
            assert_eq!(env.owner.declarations_dropped.get(), 1);
        });
    }
}

#[test]
fn group_exports_and_choose_joins_never_union_unrelated_names_or_modes() {
    for text in [
        "bind(batch: seq(one: counter.left(n: 1)), body: member(value: batch, name: \"missing\"))",
        "bind(batch: seq(one: counter.left(n: 1)), body: field(value: batch, name: \"count\"))",
        "choose(when: true, then: seq(one: counter.left(n: 1)), otherwise: seq(other: counter.right(n: 2)))",
        "choose(when: true, then: all(left: counter.left(n: 1), right: counter.right(n: 2)), otherwise: seq(left: counter.left(n: 1), right: counter.right(n: 2)))",
    ] {
        fixture(|host, env| {
            assert!(compile(&expression(text), LIMITS, host, env).is_err());
        });
    }
}

#[test]
fn closed_reference_preflight_reuses_loop_fold_and_bind_scopes_without_native_alias_fallback() {
    fixture(|host, env| {
        let source = expression(
            "all(left: counter.left(n: loop(i: 0, while: lt(left: i, right: 2), next: add(left: i, right: 1), limit: 3)), right: counter.right(n: fold(total: 0, items: strings(a: \"a\"), item: \"part\", next: add(left: total, right: len(value: part)), limit: 1)))",
        );
        assert!(compile(&source, LIMITS, host, env).is_ok());
    });
    for text in [
        "all(left: counter.left(n: 1), right: counter.right(n: missing))",
        "all(left: counter.left(n: loop(i: 0, while: true, next: missing, limit: 1)), right: counter.right(n: 1))",
        "all(left: counter.left(n: fold(total: 0, items: strings(a: \"a\"), item: \"part\", next: missing, limit: 1)), right: counter.right(n: 1))",
        "bind(seed: 1, body: choose(when: true, then: counter.left(n: seed), otherwise: counter.right(n: missing)))",
        "bind(seed: seed, body: counter.left(n: seed))",
    ] {
        fixture(|host, env| {
            assert!(compile(&expression(text), LIMITS, host, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
            assert_eq!(env.owner.branches.get(), 0);
        });
    }
    fixture(|host, env| {
        let source = expression("seq(one: counter.left(n: seed))");
        let ty = PureType::Scalar(ScalarType::Integer);
        let result: CatalogGroupSourceResult<Node<'_>, Meta<'_>, Private> =
            compile_catalog_group_source(&source, &[("seed", &ty)], LIMITS, host, env);
        assert!(result.is_ok());
        let mut limited = LIMITS;
        limited.expression.control.max_bindings = 0;
        let result: CatalogGroupSourceResult<Node<'_>, Meta<'_>, Private> =
            compile_catalog_group_source(&source, &[("seed", &ty)], limited, host, env);
        assert!(result.is_err());
    });
}

#[test]
fn group_profile_is_additive_and_original_call_only_compiler_still_rejects_groups() {
    fixture(|host, env| {
        let source = expression(BATCH);
        let result: Result<(Node<'_>, PureType<Meta<'_>>), _> =
            compile_catalog_source(&source, &[], LIMITS.expression, host, env);
        assert!(result.is_err());
        assert_eq!(env.owner.maps.get(), 0);
        let mut limits = LIMITS;
        limits.max_groups = 0;
        limits.max_branches = 0;
        let (node, ty) = compile(&expression("add(left: 1, right: 2)"), limits, host, env).unwrap();
        assert_eq!(nodes(&node), 3);
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
    });
}

const PROGRAM: CatalogGroupProgramLimits = CatalogGroupProgramLimits {
    program: CatalogProgramLimits {
        expression: LIMITS.expression,
        max_functions: 32,
        max_parameters: 8,
        max_compilation_nodes: 4096,
        max_expansion_depth: 32,
    },
    max_groups: LIMITS.max_groups,
    max_branches: LIMITS.max_branches,
};
fn program_tree(text: &str) -> SyntaxTree {
    let tree = parse(text);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree
}
fn compile_program<'s>(
    tree: &SyntaxTree,
    entry: &str,
    limits: CatalogGroupProgramLimits,
    host: &Host<'_, 's>,
    env: &Environment<'s>,
) -> CatalogSourceResult<Node<'s>, Meta<'s>, Private> {
    let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    compile_catalog_group_program(&declarations, entry, limits, host, env)
}
struct NoProjections;
impl PureEvaluationEnvironment<Field, Key> for NoProjections {
    type Result = ();
    type Error = Private;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, Private> {
        Err(Private("unexpected field"))
    }
    fn member(&self, _: &(), _: &str, _: &Key) -> Result<(), Private> {
        Err(Private("unexpected member"))
    }
}

#[test]
fn group_program_entry_uses_forward_pure_helpers_without_native_slot_copies() {
    fixture(|host, env| {
        let tree = program_tree(
            "fn start() = bind(n: 40, body: all(left: counter.left(n: plus(n: n)), right: counter.right(n: twice(n: 3))))\n\
             fn twice(n: integer) = add(left: n, right: n)\n\
             fn plus(n: integer) = add(left: n, right: 2)",
        );
        let original = format!("{tree:?}");
        let (node, ty) = compile_program(&tree, "start", PROGRAM, host, env).unwrap();
        assert_eq!(format!("{tree:?}"), original);
        let PureType::Result(Meta::Group(signature)) = ty else {
            panic!()
        };
        assert_eq!(signature.kind, GroupKind::Parallel);
        let Node::Bind { body, .. } = &node else {
            panic!()
        };
        let Node::Group { branches, .. } = body.as_ref() else {
            panic!()
        };
        let mut bindings = vec![(
            "n",
            leselang_hir::pure_evaluation::PureValue::Scalar(ScalarValue::Integer(40)),
        )];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        for (branch, expected) in branches.iter().zip([42, 6]) {
            let Node::Call {
                operation,
                arguments,
            } = &branch.value
            else {
                panic!()
            };
            let schema = host.catalog.lookup(operation.label, 7).unwrap();
            assert!(std::ptr::eq(branch.result_type.original, &schema.result));
            let prepared = evaluate_call_arguments_in_scope(
                arguments,
                schema,
                &mut scope,
                &NoProjections,
                &mut fuel,
                CallEvaluationLimits {
                    pure: PureEvaluationLimits {
                        max_nodes: 256,
                        max_depth: 32,
                        max_bindings: 8,
                    },
                    max_arguments: 64,
                },
            )
            .unwrap();
            assert_eq!(
                prepared.arguments()[0].value,
                ScalarValue::Integer(expected)
            );
        }
        assert_eq!(scope.len(), 1);
        assert_eq!(env.owner.maps.get(), 2);
        assert_eq!(env.owner.branches.get(), 2);
        drop(scope);
        drop(node);
        assert_eq!(env.owner.keys_dropped.get(), 2);
        assert_eq!(env.owner.declarations_dropped.get(), 2);
    });
}

#[test]
fn group_program_entry_combines_ordinary_effect_helpers_and_closed_result_projections() {
    fixture(|host, env| {
        let tree = program_tree(
            "fn main() = bind(record: fetch(n: 1), body: bind(batch: seq(one: counter.left(n: field(value: record, name: \"count\"))), body: bind(selected: member(value: batch, name: \"one\"), body: plus(n: field(value: selected, name: \"count\")))))\n\
             fn fetch(n: integer) = counter.right(n: n)\n\
             fn plus(n: integer) = add(left: n, right: 1)",
        );
        let (node, ty) = compile_program(&tree, "main", PROGRAM, host, env).unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
        assert_eq!(env.owner.maps.get(), 3); // Unused validation, invocation and group member.
        assert_eq!(env.owner.branches.get(), 1);
        drop(node);
        assert_eq!(env.owner.keys_dropped.get(), 4); // Three call keys and one member projection.
        assert_eq!(env.owner.declarations_dropped.get(), 1);
    });
}

#[test]
fn group_program_cold_unused_helpers_references_and_dependency_policy_precede_native_mapping() {
    let entry = "fn main() = all(left: counter.left(n: 1), right: counter.right(n: 2))";
    for tail in [
        "fn unused() = missing",
        "fn unused() = choose(when: true, then: 0, otherwise: missing)",
        "fn unused() = bind(local: local, body: local)",
        "fn unused() = loop(i: 0, while: true, next: missing, limit: 1)",
        "fn unused() = fold(total: 0, items: strings(a: \"a\"), item: \"part\", next: missing, limit: 1)",
        "fn unused() = counter.unknown(n: 1)",
        "fn unused() = counter.right(wrong: 1)",
        "fn unused() = helper(wrong: 1)\nfn helper(n: integer) = n",
        "fn unused() = main()",
        "fn a() = b()\nfn b() = a()",
        "fn add(n: integer) = n",
        "fn unused() = all(left: counter.left(n: 1), right: counter.right(n: 2))",
    ] {
        fixture(|host, env| {
            let tree = program_tree(&format!("{entry}\n{tail}"));
            assert!(
                compile_program(&tree, "main", PROGRAM, host, env).is_err(),
                "{tail}"
            );
            assert_eq!(env.owner.maps.get(), 0, "{tail}");
            assert_eq!(env.owner.branches.get(), 0);
            assert_eq!(env.owner.groups.get(), 0);
        });
    }
    for text in [
        "fn main() = seq(one: counter.left(n: missing))",
        "fn main() = bind(n: 1, body: seq(one: counter.left(n: leak())))\nfn leak() = n",
        "fn main(n: integer) = n",
        "fn other() = 1",
    ] {
        fixture(|host, env| {
            assert!(compile_program(&program_tree(text), "main", PROGRAM, host, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
        });
    }
}

#[test]
fn group_program_helpers_cannot_masquerade_as_atomic_members_or_return_groups() {
    for text in [
        "fn main() = seq(one: fetch(n: 1))\nfn fetch(n: integer) = counter.left(n: n)",
        "fn main() = all(left: fetch(), right: counter.right(n: 2))\nfn fetch() = counter.left(n: 1)",
        "fn main() = groups()\nfn groups() = seq(one: counter.left(n: 1))",
        "fn main() = seq(one: counter.left(n: groups()))\nfn groups() = seq(one: counter.left(n: 1))",
        "fn main() = repeat(times: 2, body: counter.left(n: 1))",
    ] {
        fixture(|host, env| {
            assert!(
                compile_program(&program_tree(text), "main", PROGRAM, host, env).is_err(),
                "{text}"
            );
            assert_eq!(env.owner.maps.get(), 0);
            assert_eq!(env.owner.branches.get(), 0);
        });
    }
}

#[test]
fn group_program_preserves_shared_nonrefunding_work_depth_and_whole_output_limits() {
    let text = "fn main() = seq(one: counter.left(n: inc(n: 1)))\nfn inc(n: integer) = add(left: n, right: 1)";
    fixture(|host, env| {
        let tree = program_tree(text);
        let mut exact = PROGRAM;
        exact.program.max_compilation_nodes = 11;
        exact.program.max_expansion_depth = 1;
        exact.program.expression.control.source.max_lowered_nodes = 7;
        exact.max_groups = 1;
        exact.max_branches = 1;
        let (node, _) = compile_program(&tree, "main", exact, host, env).unwrap();
        assert_eq!(nodes(&node), 7);
    });
    for case in 0..6 {
        fixture(|host, env| {
            let mut limits = PROGRAM;
            match case {
                0 => limits.program.max_compilation_nodes = 10,
                1 => limits.program.max_expansion_depth = 0,
                2 => limits.program.expression.control.source.max_lowered_nodes = 6,
                3 => limits.program.expression.max_calls = 0,
                4 => limits.max_groups = 0,
                _ => limits.program.expression.control.max_bindings = 0,
            }
            let result = compile_program(&program_tree(text), "main", limits, host, env);
            assert!(result.is_err(), "case {case}");
            drop(result);
            assert_eq!(env.owner.keys_dropped.get(), env.owner.maps.get());
            assert_eq!(
                env.owner.declarations_dropped.get(),
                env.owner.branches.get()
            );
        });
    }
}

#[test]
fn group_program_counts_cold_groups_and_calls_across_unused_declarations_and_choose_arms() {
    let text = "fn main() = choose(when: true, then: seq(one: counter.left(n: 1)), otherwise: seq(one: counter.left(n: 2)))\nfn unused() = counter.right(n: 3)";
    for case in 0..5 {
        fixture(|host, env| {
            let mut limits = PROGRAM;
            match case {
                0 => limits.max_groups = 1,
                1 => limits.program.expression.max_calls = 2,
                2 => limits.max_branches = 65,
                3 => limits.program.max_functions = 1,
                _ => limits.program.max_parameters = 9,
            }
            assert!(compile_program(&program_tree(text), "main", limits, host, env).is_err());
            assert_eq!(env.owner.maps.get(), 0);
        });
    }
    fixture(|host, env| {
        let denied = SourceCallHost {
            catalog: host.catalog,
            version: 7,
            granted: &[31],
        };
        assert!(compile_program(&program_tree(text), "main", PROGRAM, &denied, env).is_err());
        assert_eq!(env.owner.maps.get(), 0);
    });
    fixture(|host, env| {
        let tree = program_tree(text);
        let mut limits = PROGRAM;
        limits.max_groups = 2;
        limits.program.expression.max_calls = 3;
        limits.program.max_expansion_depth = 0;
        assert!(compile_program(&tree, "main", limits, host, env).is_ok());
    });
    fixture(|host, env| {
        let tree = program_tree(
            "fn main() = bind(record: twice(), body: seq(one: counter.right(n: 1)))\n\
             fn twice() = bind(saved: fetch(), body: fetch())\n\
             fn fetch() = counter.left(n: 1)",
        );
        let mut limits = PROGRAM;
        // Two native sites in the source forest become three in the expanded entry.
        limits.program.expression.max_calls = 2;
        assert!(compile_program(&tree, "main", limits, host, env).is_err());
        assert_eq!(env.owner.maps.get(), 6);
        assert_eq!(env.owner.keys_dropped.get(), 6);
        assert_eq!(env.owner.branches.get(), 1);
        assert_eq!(env.owner.declarations_dropped.get(), 1);
    });
}

#[test]
fn group_program_branch_failure_and_unwind_release_expanded_native_output_once() {
    let text = "fn main() = all(left: counter.left(n: inc(n: 1)), right: counter.right(n: inc(n: 2)))\nfn inc(n: integer) = add(left: n, right: 1)";
    for mode in [Mode::BranchError, Mode::BranchUnwind] {
        fixture(|host, env| {
            env.owner.mode.set(mode);
            let tree = program_tree(text);
            let before = Rc::strong_count(&env.owner);
            let result = catch_unwind(AssertUnwindSafe(|| {
                compile_program(&tree, "main", PROGRAM, host, env)
            }));
            if mode == Mode::BranchUnwind {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().err().unwrap();
                assert!(!format!("{error} {error:?}").contains("private branch payload"));
                assert!(std::error::Error::source(&error).is_none());
            }
            assert_eq!(Rc::strong_count(&env.owner), before);
            assert_eq!(env.owner.maps.get(), 2);
            assert_eq!(env.owner.keys_dropped.get(), 2);
            assert_eq!(env.owner.branches.get(), 2);
            assert_eq!(env.owner.declarations_dropped.get(), 1);
            assert_eq!(env.owner.groups.get(), 0);
        });
    }
}

#[test]
fn legacy_program_remains_group_rejecting_and_new_entry_needs_no_groups_for_pure_helpers() {
    fixture(|host, env| {
        let tree = program_tree(&format!("fn main() = {BATCH}\nfn pure(n: integer) = n"));
        let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
        let legacy: CatalogSourceResult<Node<'_>, Meta<'_>, Private> =
            compile_catalog_program(&declarations, "main", PROGRAM.program, host, env);
        assert!(legacy.is_err());
        assert_eq!(env.owner.maps.get(), 0);
        let tree =
            program_tree("fn main() = inc(n: 1)\nfn inc(n: integer) = add(left: n, right: 1)");
        let limits = CatalogGroupProgramLimits {
            max_groups: 0,
            max_branches: 0,
            ..PROGRAM
        };
        let (node, ty) = compile_program(&tree, "main", limits, host, env).unwrap();
        assert!(node.is_pure());
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
    });
}

fn compile_group_helpers<'s>(
    tree: &SyntaxTree,
    limits: CatalogGroupProgramLimits,
    host: &Host<'_, 's>,
    env: &Environment<'s>,
) -> CatalogSourceResult<Node<'s>, Meta<'s>, Private> {
    let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    compile_catalog_group_helper_program(&declarations, "main", limits, host, env)
}

#[test]
fn group_returning_forward_helpers_move_original_slots_through_shared_hygiene() {
    fixture(|host, env| {
        let tree = program_tree(
            "fn main() = bind(n: 40, body: bind(batch: relay(n: n), body: member(value: batch, name: \"left\")))\n\
             fn relay(n: integer) = collect(n: n)\n\
             fn collect(n: integer) = all(left: counter.left(n: inc(n: n)), right: counter.right(n: n))\n\
             fn inc(n: integer) = add(left: n, right: 1)",
        );
        let before = format!("{tree:?}");
        let (node, ty) = compile_group_helpers(&tree, PROGRAM, host, env).unwrap();
        assert_eq!(format!("{tree:?}"), before);
        let PureType::Result(Meta::Record(domain)) = ty else {
            panic!()
        };
        assert!(std::ptr::eq(
            domain,
            &host.catalog.lookup("counter.left", 7).unwrap().result
        ));
        let mut pending = vec![&node];
        let mut groups = 0;
        while let Some(part) = pending.pop() {
            if let Node::Group { branches, .. } = part {
                groups += 1;
                for branch in branches {
                    let Node::Call { operation, .. } = &branch.value else {
                        panic!()
                    };
                    assert!(std::ptr::eq(
                        branch.result_type.original,
                        &host.catalog.lookup(operation.label, 7).unwrap().result
                    ));
                }
            }
            pending.extend(part.children());
        }
        assert_eq!(groups, 1);
        assert_eq!(env.owner.maps.get(), 6);
        assert_eq!(env.owner.branches.get(), 6);
        drop(node);
        assert_eq!(env.owner.keys_dropped.get(), 7);
        assert_eq!(env.owner.declarations_dropped.get(), 6);
    });
}

type Prepared<'s> = PreparedCall<'s, Key, ScalarTypeSet, Domain, u8>;
struct GroupPreparation<'a, 'h, 's>(&'a Host<'h, 's>);
impl PureEvaluationEnvironment<Field, Key> for GroupPreparation<'_, '_, '_> {
    type Result = ();
    type Error = Private;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, Private> {
        Err(Private("no field"))
    }
    fn member(&self, _: &(), _: &str, _: &Key) -> Result<(), Private> {
        Err(Private("no member"))
    }
}
impl<'ir, 's> EffectEvaluationEnvironment<'ir, Field, Key, Opaque, Declaration<'s>>
    for GroupPreparation<'_, '_, 's>
{
    type Request = Vec<Prepared<'s>>;
    type Capture = ();
    fn preflight_effect(&self, node: &'ir Node<'s>) -> Result<(), Private> {
        match node {
            Node::Group { .. } => Ok(()),
            Node::Call { operation, .. } => self
                .0
                .catalog
                .authorize(operation.label, self.0.version, self.0.granted)
                .map(|_| ())
                .map_err(|_| Private("policy")),
            _ => Err(Private("unsupported effect")),
        }
    }
    fn prepare_effect(
        &self,
        node: &'ir Node<'s>,
        scope: &mut ScopeFrame<'_, 'ir, leselang_hir::pure_evaluation::PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Private>> {
        let Node::Group { branches, .. } = node else {
            return Err(Private("not a group").into());
        };
        let mut prepared = Vec::new();
        for branch in branches {
            let Node::Call {
                operation,
                arguments,
            } = &branch.value
            else {
                return Err(Private("not a call").into());
            };
            let schema = self
                .0
                .catalog
                .authorize(operation.label, self.0.version, self.0.granted)
                .map_err(|_| Private("live policy"))?;
            if !std::ptr::eq(branch.result_type.original, &schema.result) {
                return Err(Private("foreign declaration").into());
            }
            let call = evaluate_call_arguments_in_scope(
                arguments,
                schema,
                scope,
                self,
                fuel,
                CallEvaluationLimits {
                    pure: PureEvaluationLimits {
                        max_nodes: 256,
                        max_depth: 32,
                        max_bindings: 8,
                    },
                    max_arguments: 64,
                },
            )
            .map_err(|_| Private("actual operands"))?;
            prepared.push(call);
        }
        Ok(prepared)
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node<'s>,
        scope: &ScopeFrame<'_, 'ir, leselang_hir::pure_evaluation::PureValue<()>>,
        _: &mut Fuel,
    ) -> Result<(), Private> {
        if scope.is_empty() {
            Ok(())
        } else {
            Err(Private("unexpected outer capture"))
        }
    }
}

#[test]
fn parameterized_group_helper_prepares_original_operands_before_parameter_scope_is_dropped() {
    fixture(|host, env| {
        let tree = program_tree(
            "fn main() = bind(batch: collect(n: 40), body: 0)\n\
             fn collect(n: integer) = all(left: counter.left(n: add(left: n, right: 1)), right: counter.right(n: n))",
        );
        let (node, _) = compile_group_helpers(&tree, PROGRAM, host, env).unwrap();
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let ResumableEffectOutcome::Suspended {
            request,
            continuation,
        } = evaluate_resumable_effects_in_scope(
            &node,
            &mut scope,
            &GroupPreparation(host),
            &mut fuel,
            EffectEvaluationLimits {
                pure: PureEvaluationLimits {
                    max_nodes: 256,
                    max_depth: 32,
                    max_bindings: 8,
                },
                max_arguments: 64,
                max_branches: 64,
            },
        )
        .unwrap()
        else {
            panic!()
        };
        assert_eq!(scope.len(), 0);
        assert_eq!(continuation.name(), "batch");
        for (call, (key, expected)) in request
            .iter()
            .zip([("counter.left", 41), ("counter.right", 40)])
        {
            assert!(std::ptr::eq(
                call.schema(),
                host.catalog.lookup(key, 7).unwrap()
            ));
            assert_eq!(call.arguments()[0].value, ScalarValue::Integer(expected));
        }
        assert_eq!(fuel.remaining(), 92);
        // This native adapter hands off prepared data; it never dispatches effects.
    });
}

type ParamBranch<'s> = ComputedBranch<Node<'s>, Declaration<'s>>;
type ParamSchema<'s> = OperationSchema<'s, Key, &'s str, ScalarTypeSet, Domain, u8>;
struct ParamRecord<'s> {
    schema: &'s ParamSchema<'s>,
    count: u64,
}
struct ParamReply<'ir, 's> {
    group: &'ir Node<'s>,
    rows: Vec<(&'ir ParamBranch<'s>, Rc<ParamRecord<'s>>)>,
    buffer: Box<u8>,
}
#[derive(Clone)]
enum ParamView<'ir, 's> {
    Group(Rc<ParamReply<'ir, 's>>),
    Record(Rc<ParamRecord<'s>>),
}
struct ParamDomain<'ir, 'h, 's> {
    group: &'ir Node<'s>,
    source: &'h Host<'h, 's>,
    live: Cell<bool>,
}
impl<'s> ParamDomain<'_, '_, 's> {
    fn schema(&self, branch: &ParamBranch<'s>) -> Result<&'s ParamSchema<'s>, Private> {
        if !self.live.get() {
            return Err(Private("revoked native policy"));
        }
        let Node::Call { operation, .. } = &branch.value else {
            return Err(Private("non-atomic member"));
        };
        let schema = self
            .source
            .catalog
            .authorize(operation.label, self.source.version, self.source.granted)
            .map_err(|_| Private("native policy"))?;
        if operation != &schema.key
            || !std::ptr::eq(branch.result_type.original, &schema.result)
            || !Rc::ptr_eq(&branch.result_type.owner, &schema.result.owner)
        {
            return Err(Private("foreign native domain"));
        }
        Ok(schema)
    }
    fn record(&self, record: &ParamRecord<'s>) -> Result<(), Private> {
        if !self.live.get() || record.count > 200 {
            return Err(Private("native record policy"));
        }
        let current = self
            .source
            .catalog
            .authorize(
                record.schema.key.label,
                self.source.version,
                self.source.granted,
            )
            .map_err(|_| Private("record authorization"))?;
        if !std::ptr::eq(current, record.schema) {
            return Err(Private("record schema identity"));
        }
        Ok(())
    }
}
impl<'ir, 's> HostResultDomain<ParamReply<'ir, 's>> for ParamDomain<'ir, '_, 's> {
    type Error = Private;
    fn matches_type(&self, reply: &ParamReply<'ir, 's>) -> bool {
        std::ptr::eq(self.group, reply.group)
    }
    fn validate_value(&self, reply: &ParamReply<'ir, 's>) -> Result<(), Private> {
        let Node::Group { branches, .. } = self.group else {
            return Err(Private("not a group"));
        };
        if !self.matches_type(reply) || reply.rows.len() != branches.len() {
            return Err(Private("native group shape"));
        }
        for (branch, (original, record)) in branches.iter().zip(&reply.rows) {
            if !std::ptr::eq(branch, *original)
                || !std::ptr::eq(self.schema(branch)?, record.schema)
            {
                return Err(Private("native group original row"));
            }
            self.record(record)?;
        }
        Ok(())
    }
}
struct ParamRequest<'ir, 's> {
    group: &'ir Node<'s>,
    prepared: PreparedNativeGroup<'ir, ParamBranch<'s>, Prepared<'s>>,
}
struct ParamRuntime<'d, 'ir, 'h, 's> {
    domain: &'d ParamDomain<'ir, 'h, 's>,
    calls: Cell<usize>,
    captures: Cell<usize>,
    restores: Cell<usize>,
    projections: Cell<usize>,
    captured_sizes: std::cell::RefCell<Vec<usize>>,
    amounts: std::cell::RefCell<Vec<u64>>,
    fail_prepare: Cell<bool>,
}
impl<'d, 'ir, 'h, 's> ParamRuntime<'d, 'ir, 'h, 's> {
    fn new(domain: &'d ParamDomain<'ir, 'h, 's>) -> Self {
        Self {
            domain,
            calls: Cell::new(0),
            captures: Cell::new(0),
            restores: Cell::new(0),
            projections: Cell::new(0),
            captured_sizes: Default::default(),
            amounts: Default::default(),
            fail_prepare: Cell::new(false),
        }
    }
    fn invoke(&self, request: ParamRequest<'ir, 's>) -> ParamReply<'ir, 's> {
        let (_, members) = request.prepared.into_parts();
        let mut rows = Vec::new();
        for member in members {
            let (_, original, prepared) = member.into_parts();
            let (schema, mut arguments) = prepared.into_parts();
            assert!(std::ptr::eq(
                schema,
                self.domain
                    .schema(original)
                    .unwrap_or_else(|_| panic!("native dispatch denied"))
            ));
            let ScalarValue::Integer(count) = arguments.remove(0).value else {
                panic!()
            };
            self.calls.set(self.calls.get() + 1);
            self.amounts.borrow_mut().push(count);
            rows.push((original, Rc::new(ParamRecord { schema, count })));
        }
        ParamReply {
            group: request.group,
            rows,
            buffer: Box::new(9),
        }
    }
}
impl<'ir, 's> PureEvaluationEnvironment<Field, Key> for ParamRuntime<'_, 'ir, '_, 's> {
    type Result = ParamView<'ir, 's>;
    type Error = Private;
    fn field(&self, value: &Self::Result, _: &Field) -> Result<ScalarValue, Private> {
        let ParamView::Record(record) = value else {
            return Err(Private("not a record"));
        };
        self.domain.record(record)?;
        Ok(ScalarValue::Integer(record.count))
    }
    fn member(
        &self,
        value: &Self::Result,
        name: &str,
        operation: &Key,
    ) -> Result<Self::Result, Private> {
        let ParamView::Group(group) = value else {
            return Err(Private("not a group"));
        };
        self.domain.validate_value(group.as_ref())?;
        let (original, record) = group
            .rows
            .iter()
            .find(|(b, _)| b.name == name)
            .ok_or(Private("missing member"))?;
        let Node::Call {
            operation: expected,
            ..
        } = &original.value
        else {
            return Err(Private("not a call"));
        };
        if operation != expected {
            return Err(Private("member operation identity"));
        }
        Ok(ParamView::Record(record.clone()))
    }
}
impl<'ir, 's> EffectEvaluationEnvironment<'ir, Field, Key, Opaque, Declaration<'s>>
    for ParamRuntime<'_, 'ir, '_, 's>
{
    type Request = ParamRequest<'ir, 's>;
    type Capture = RestoredEffectBindings<'ir, ParamView<'ir, 's>>;
    fn preflight_effect(&self, node: &'ir Node<'s>) -> Result<(), Private> {
        if std::ptr::eq(node, self.domain.group) {
            let Node::Group { branches, .. } = node else {
                return Err(Private("bad group"));
            };
            for branch in branches {
                self.domain.schema(branch)?;
            }
            return Ok(());
        }
        let Node::Group { branches, .. } = self.domain.group else {
            return Err(Private("bad root"));
        };
        let branch = branches
            .iter()
            .find(|b| std::ptr::eq(&b.value, node))
            .ok_or(Private("foreign leaf"))?;
        self.domain.schema(branch).map(|_| ())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node<'s>,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Private>> {
        if !std::ptr::eq(node, self.domain.group) {
            return Err(Private("foreign preparation").into());
        }
        let Node::Group {
            group_kind,
            branches,
        } = node
        else {
            return Err(Private("not a group").into());
        };
        let prepared = prepare_native_group_members(
            *group_kind,
            branches,
            64,
            fuel,
            |b| Ok(b.name.as_str()),
            |b| self.domain.schema(b).map(|_| ()),
            |index, branch, fuel| {
                if index == 1 && self.fail_prepare.get() {
                    return Err(Private("late preparation refusal"));
                }
                let schema = self.domain.schema(branch)?;
                let Node::Call { arguments, .. } = &branch.value else {
                    return Err(Private("not a call"));
                };
                evaluate_call_arguments_in_scope(
                    arguments,
                    schema,
                    scope,
                    self,
                    fuel,
                    CallEvaluationLimits {
                        pure: PureEvaluationLimits {
                            max_nodes: 256,
                            max_depth: 32,
                            max_bindings: 8,
                        },
                        max_arguments: 64,
                    },
                )
                .map_err(|_| Private("actual argument rejection"))
            },
        )
        .map_err(|_| Private("complete group preparation rejected"))?;
        Ok(ParamRequest {
            group: node,
            prepared,
        })
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node<'s>,
        scope: &ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Private> {
        let mut saved = Vec::new();
        for (name, value) in scope.bindings() {
            let extra = if let PureValue::Scalar(value) = value {
                scalar_copy_cost(value)
            } else {
                0
            };
            fuel.charge(1 + extra)
                .map_err(|_| Private("capture cost"))?;
            saved.push((*name, value.clone()));
        }
        self.captures.set(self.captures.get() + 1);
        self.captured_sizes.borrow_mut().push(saved.len());
        Ok(saved)
    }
}
impl<'ir, 'h, 's>
    EffectReentryEnvironment<
        'ir,
        Field,
        Key,
        Opaque,
        Declaration<'s>,
        u64,
        ParamDomain<'ir, 'h, 's>,
        ParamReply<'ir, 's>,
    > for ParamRuntime<'_, 'ir, 'h, 's>
{
    fn restore_capture(
        &self,
        identity: &u64,
        declaration: &ParamDomain<'ir, 'h, 's>,
        saved: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Private>> {
        if *identity != 73 || !std::ptr::eq(declaration, self.domain) || !self.domain.live.get() {
            return Err(Private("capture provenance").into());
        }
        self.restores.set(self.restores.get() + 1);
        Ok(saved)
    }
    fn bind_reply(
        &self,
        identity: &u64,
        declaration: &ParamDomain<'ir, 'h, 's>,
        reply: ParamReply<'ir, 's>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<Self::Result>, CalculationFailure<Private>> {
        if *identity != 73 || !std::ptr::eq(declaration, self.domain) {
            return Err(Private("reply provenance").into());
        }
        self.domain.validate_value(&reply)?;
        fuel.charge(1).map_err(|_| Private("projection fuel"))?;
        self.projections.set(self.projections.get() + 1);
        Ok(PureValue::Result(ParamView::Group(Rc::new(reply))))
    }
}
impl<'d, 'ir, 'h, 's> EffectSessionEnvironment<'ir, 'd, Field, Key, Opaque, Declaration<'s>>
    for ParamRuntime<'d, 'ir, 'h, 's>
{
    type Identity = u64;
    type Declaration = ParamDomain<'ir, 'h, 's>;
    type Reply = ParamReply<'ir, 's>;
    type Dispatch = ParamRequest<'ir, 's>;
    fn correlate_request(
        &self,
        request: Self::Request,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'d, u64, Self::Declaration, Self::Dispatch, Private> {
        self.preflight_effect(request.group)?;
        fuel.charge(2)
            .map_err(|_| Private("bounded native invocation reservation"))?;
        Ok(CorrelatedEffectRequest {
            identity: 73,
            declaration: self.domain,
            dispatch: request,
        })
    }
}
impl ReplyAuthority<u64> for ParamRuntime<'_, '_, '_, '_> {
    type Error = Private;
    fn authorize(&self, identity: &u64) -> Result<(), Private> {
        if *identity == 73 && self.domain.live.get() {
            Ok(())
        } else {
            Err(Private("revoked authority"))
        }
    }
}
fn parameterized_return_tree() -> SyntaxTree {
    parameterized_group_return_tree("all")
}
fn parameterized_group_return_tree(kind: &str) -> SyntaxTree {
    program_tree(&format!(
        "fn main() = bind(seed: 2, body: bind(answer: collect(n: 40), body: add(left: answer, right: seed)))\n\
        fn collect(n: integer) = bind(batch: {kind}(left: counter.left(n: add(left: n, right: 1)), right: counter.right(n: n)),\
            body: bind(selected: member(value: batch, name: \"left\"),\
                body: add(left: n, right: field(value: selected, name: \"count\"))))"
    ))
}
fn only_group<'ir, 's>(node: &'ir Node<'s>) -> &'ir Node<'s> {
    if matches!(node, Node::Group { .. }) {
        return node;
    }
    node.children()
        .find_map(|child| {
            let mut pending = vec![child];
            while let Some(current) = pending.pop() {
                if matches!(current, Node::Group { .. }) {
                    return Some(current);
                }
                pending.extend(current.children());
            }
            None
        })
        .unwrap()
}
const PARAM_STACK: EffectStackLimits = EffectStackLimits {
    effects: EffectEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
        },
        max_arguments: 64,
        max_branches: 64,
    },
    max_frames: 4,
};

#[test]
fn parsed_parameterized_group_helper_prepares_live_operands_and_restores_both_scopes_through_owned_session()
 {
    fixture(|host, env| {
        let tree = parameterized_return_tree();
        let (node, ty) = compile_group_helpers(&tree, PROGRAM, host, env).unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
        let domain = ParamDomain {
            group: only_group(&node),
            source: host,
            live: Cell::new(true),
        };
        let runtime = ParamRuntime::new(&domain);
        let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
        let EffectSessionPoll::Request(request) = session.poll(&runtime).unwrap() else {
            panic!()
        };
        assert_eq!(runtime.calls.get(), 0);
        assert_eq!(runtime.captures.get(), 2);
        assert_eq!(*runtime.captured_sizes.borrow(), [2, 1]);
        assert!(std::ptr::eq(
            request.prepared.metadata().members(),
            match domain.group {
                Node::Group { branches, .. } => &branches[..],
                _ => panic!(),
            }
        ));
        let amounts: Vec<_> = request
            .prepared
            .members()
            .iter()
            .map(|row| match &row.prepared().arguments()[0].value {
                ScalarValue::Integer(n) => *n,
                _ => panic!(),
            })
            .collect();
        assert_eq!(amounts, [41, 40]);
        let waiting_fuel = session.fuel_remaining();
        assert_eq!(waiting_fuel, 81);
        for _ in 0..2 {
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert_eq!(session.fuel_remaining(), waiting_fuel);
            assert_eq!(runtime.calls.get(), 0);
            assert_eq!(runtime.captures.get(), 2);
        }
        let mut reply = runtime.invoke(request);
        let pointer = (&*reply.buffer as *const u8) as usize;
        reply.rows.swap(0, 1);
        let rejected = session.try_accept(&73, reply, &runtime).unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        assert!(matches!(session.status(), EffectSessionStatus::Awaiting));
        let mut reply = rejected.reply;
        reply.rows.swap(0, 1);
        let rejected = session.try_accept(&74, reply, &runtime).unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        session.try_accept(&73, rejected.reply, &runtime).unwrap();
        assert_eq!(runtime.restores.get(), 0);
        assert_eq!(runtime.projections.get(), 0);
        assert_eq!(session.fuel_remaining(), waiting_fuel);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(83)))
        ));
        assert_eq!(runtime.calls.get(), 2);
        assert_eq!(*runtime.amounts.borrow(), [41, 40]);
        assert_eq!(runtime.restores.get(), 2);
        assert_eq!(runtime.projections.get(), 1);
        assert_eq!(session.fuel_remaining(), 68);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
        assert_eq!(session.fuel_remaining(), 68);
        assert_eq!(runtime.calls.get(), 2);
    });
}

#[test]
fn parameterized_group_session_cancellation_revocation_or_late_preparation_refusal_stops_successors()
 {
    for phase in 0..3 {
        fixture(|host, env| {
            let tree = parameterized_return_tree();
            let (node, _) = compile_group_helpers(&tree, PROGRAM, host, env).unwrap();
            let domain = ParamDomain {
                group: only_group(&node),
                source: host,
                live: Cell::new(true),
            };
            let runtime = ParamRuntime::new(&domain);
            let mut session =
                StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
            if phase == 2 {
                runtime.fail_prepare.set(true);
                assert!(session.poll(&runtime).is_err());
                assert_eq!(runtime.calls.get(), 0);
                assert_eq!(runtime.captures.get(), 0);
            } else {
                let EffectSessionPoll::Request(request) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let reply = runtime.invoke(request);
                session.try_accept(&73, reply, &runtime).unwrap();
                let fuel = session.fuel_remaining();
                if phase == 0 {
                    assert!(session.cancel());
                } else {
                    domain.live.set(false);
                    assert!(session.poll(&runtime).is_err());
                }
                assert_eq!(session.fuel_remaining(), fuel);
                assert_eq!(runtime.calls.get(), 2);
            }
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            let end = if phase == 0 {
                EffectSessionEnd::Cancelled
            } else {
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(end));
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
        });
    }
}

struct ParamInput<'s> {
    record: Rc<ParamRecord<'s>>,
    buffer: Box<u8>,
}
impl<'s> Borrow<ParamRecord<'s>> for ParamInput<'s> {
    fn borrow(&self) -> &ParamRecord<'s> {
        &self.record
    }
}
impl<'s> HostResultDomain<ParamRecord<'s>> for ParamSchema<'s> {
    type Error = Private;
    fn matches_type(&self, record: &ParamRecord<'s>) -> bool {
        std::ptr::eq(self, record.schema)
    }
    fn validate_value(&self, record: &ParamRecord<'s>) -> Result<(), Private> {
        if self.matches_type(record) && record.count <= 200 {
            Ok(())
        } else {
            Err(Private("typed member domain"))
        }
    }
}
type ParamAccepted<'ir, 's> =
    ParallelAccepted<'ir, Node<'s>, Declaration<'s>, (u64, usize), ParamSchema<'s>, ParamInput<'s>>;
type ParamBarrier<'ir, 's> = PreparedParallelSession<
    'ir,
    Node<'s>,
    Declaration<'s>,
    (u64, usize),
    ParamSchema<'s>,
    ParamInput<'s>,
>;
struct ParamCollected<'ir, 's> {
    group: &'ir Node<'s>,
    rows: Vec<ParamAccepted<'ir, 's>>,
    buffer: Box<u8>,
}
type ParamSequence<'ir, 's> = PreparedSequenceSession<
    'ir,
    Node<'s>,
    Declaration<'s>,
    (u64, usize),
    ParamSchema<'s>,
    Prepared<'s>,
>;
enum ParamDispatch<'ir, 's> {
    Parallel {
        barrier: ParamBarrier<'ir, 's>,
        requests: Vec<ParallelRequest<Prepared<'s>>>,
    },
    Sequence(ParamSequence<'ir, 's>),
}
impl<'ir, 's> HostResultDomain<ParamCollected<'ir, 's>> for ParamDomain<'ir, '_, 's> {
    type Error = Private;
    fn matches_type(&self, reply: &ParamCollected<'ir, 's>) -> bool {
        std::ptr::eq(self.group, reply.group)
    }
    fn validate_value(&self, reply: &ParamCollected<'ir, 's>) -> Result<(), Private> {
        let Node::Group { branches, .. } = self.group else {
            return Err(Private("bad group"));
        };
        if !self.matches_type(reply) || reply.rows.len() != branches.len() {
            return Err(Private("incomplete group reply"));
        }
        for (i, row) in reply.rows.iter().enumerate() {
            let (index, original, identity, domain, input) = row.as_parts();
            if index != i
                || !std::ptr::eq(original, &branches[i])
                || *identity != (73, i)
                || !std::ptr::eq(domain, self.schema(original)?)
                || !std::ptr::eq(domain, input.record.schema)
            {
                return Err(Private("original accepted row policy"));
            }
            self.record(&input.record)?;
        }
        Ok(())
    }
}
struct ParamBarrierRuntime<'r, 'd, 'ir, 'h, 's> {
    inner: &'r ParamRuntime<'d, 'ir, 'h, 's>,
    refuse_correlation: Cell<bool>,
}
impl<'ir, 's> ParamBarrierRuntime<'_, '_, 'ir, '_, 's> {
    fn admit_prepared(
        &self,
        original: &'ir ParamBranch<'s>,
        prepared: &Prepared<'s>,
    ) -> Result<(), Private> {
        if !std::ptr::eq(prepared.schema(), self.inner.domain.schema(original)?) {
            return Err(Private("prepared original schema"));
        }
        Ok(())
    }
    fn correlate_member(
        &self,
        index: usize,
        original: &'ir ParamBranch<'s>,
        prepared: Prepared<'s>,
        fuel: &mut Fuel,
    ) -> Result<PreparedParallelMember<'ir, (u64, usize), ParamSchema<'s>, Prepared<'s>>, Private>
    {
        if index == 1 && self.refuse_correlation.get() {
            return Err(Private("late correlation refusal"));
        }
        self.admit_prepared(original, &prepared)?;
        fuel.charge(1)
            .map_err(|_| Private("bounded native invocation reservation"))?;
        Ok(PreparedParallelMember {
            identity: (73, index),
            declaration: self.inner.domain.schema(original)?,
            request: prepared,
        })
    }
    fn publish_sequence(
        &self,
        sequence: &mut ParamSequence<'ir, 's>,
    ) -> Result<SequencePoll<Prepared<'s>>, PreparedSequenceError<Private>> {
        sequence.poll(|index, original, identity, declaration, request| {
            self.authorize(identity)?;
            self.admit_prepared(original, request)?;
            if *identity != (73, index)
                || !std::ptr::eq(declaration, self.inner.domain.schema(original)?)
            {
                return Err(Private("fresh sequential publication"));
            }
            Ok(())
        })
    }
    fn invoke_member(&self, request: ParallelRequest<Prepared<'s>>) -> ParamInput<'s> {
        let (schema, mut arguments) = request.request.into_parts();
        let Node::Group { branches, .. } = self.inner.domain.group else {
            panic!()
        };
        assert!(std::ptr::eq(
            schema,
            self.inner
                .domain
                .schema(&branches[request.index])
                .unwrap_or_else(|_| panic!())
        ));
        let ScalarValue::Integer(count) = arguments.remove(0).value else {
            panic!()
        };
        self.inner.calls.set(self.inner.calls.get() + 1);
        self.inner.amounts.borrow_mut().push(count);
        ParamInput {
            record: Rc::new(ParamRecord { schema, count }),
            buffer: Box::new(8),
        }
    }
}
impl<'ir, 's> PureEvaluationEnvironment<Field, Key> for ParamBarrierRuntime<'_, '_, 'ir, '_, 's> {
    type Result = ParamView<'ir, 's>;
    type Error = Private;
    fn field(&self, value: &Self::Result, field: &Field) -> Result<ScalarValue, Private> {
        self.inner.field(value, field)
    }
    fn member(
        &self,
        value: &Self::Result,
        name: &str,
        operation: &Key,
    ) -> Result<Self::Result, Private> {
        self.inner.member(value, name, operation)
    }
}
impl<'ir, 's> EffectEvaluationEnvironment<'ir, Field, Key, Opaque, Declaration<'s>>
    for ParamBarrierRuntime<'_, '_, 'ir, '_, 's>
{
    type Request = ParamRequest<'ir, 's>;
    type Capture = RestoredEffectBindings<'ir, ParamView<'ir, 's>>;
    fn preflight_effect(&self, node: &'ir Node<'s>) -> Result<(), Private> {
        self.inner.preflight_effect(node)
    }
    fn prepare_effect(
        &self,
        node: &'ir Node<'s>,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Private>> {
        self.inner.prepare_effect(node, scope, fuel)
    }
    fn capture(
        &self,
        name: &'ir str,
        body: &'ir Node<'s>,
        scope: &ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Private> {
        self.inner.capture(name, body, scope, fuel)
    }
}
impl<'ir, 'h, 's>
    EffectReentryEnvironment<
        'ir,
        Field,
        Key,
        Opaque,
        Declaration<'s>,
        u64,
        ParamDomain<'ir, 'h, 's>,
        ParamCollected<'ir, 's>,
    > for ParamBarrierRuntime<'_, '_, 'ir, 'h, 's>
{
    fn restore_capture(
        &self,
        identity: &u64,
        declaration: &ParamDomain<'ir, 'h, 's>,
        saved: Self::Capture,
        fuel: &mut Fuel,
        limits: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Private>> {
        self.inner
            .restore_capture(identity, declaration, saved, fuel, limits)
    }
    fn bind_reply(
        &self,
        identity: &u64,
        declaration: &ParamDomain<'ir, 'h, 's>,
        reply: ParamCollected<'ir, 's>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<Self::Result>, CalculationFailure<Private>> {
        if *identity != 73 || !std::ptr::eq(declaration, self.inner.domain) {
            return Err(Private("outer provenance").into());
        }
        self.inner.domain.validate_value(&reply)?;
        let group =
            assemble_group_reply(reply.group, reply.rows, PARAM_PARALLEL, fuel, |whole, _| {
                let Node::Group { branches, .. } = whole.group() else {
                    return Err(Private("group assembly"));
                };
                for (i, row) in whole.members().iter().enumerate() {
                    let (index, original, identity, domain, input) = row.as_parts();
                    if index != i
                        || *identity != (73, i)
                        || !std::ptr::eq(original, &branches[i])
                        || !std::ptr::eq(domain, self.inner.domain.schema(original)?)
                        || !std::ptr::eq(domain, input.record.schema)
                    {
                        return Err(Private("fresh group assembly"));
                    }
                    self.inner.domain.record(&input.record)?;
                }
                Ok(())
            })
            .map_err(|_| Private("closed original group assembly"))?;
        let group_node = group.group();
        let rows = group
            .into_members()
            .into_iter()
            .map(|row| {
                let (_, original, _, _, input) = row.into_parts();
                (original, input.record)
            })
            .collect();
        self.inner.bind_reply(
            identity,
            declaration,
            ParamReply {
                group: group_node,
                rows,
                buffer: reply.buffer,
            },
            fuel,
        )
    }
}
const PARAM_PARALLEL: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
impl<'d, 'ir, 'h, 's> EffectSessionEnvironment<'ir, 'd, Field, Key, Opaque, Declaration<'s>>
    for ParamBarrierRuntime<'_, 'd, 'ir, 'h, 's>
{
    type Identity = u64;
    type Declaration = ParamDomain<'ir, 'h, 's>;
    type Reply = ParamCollected<'ir, 's>;
    type Dispatch = ParamDispatch<'ir, 's>;
    fn correlate_request(
        &self,
        request: Self::Request,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'d, u64, Self::Declaration, Self::Dispatch, Private> {
        let Node::Group { group_kind, .. } = request.group else {
            return Err(Private("not a group").into());
        };
        let dispatch = match group_kind {
            GroupKind::Parallel => {
                let (barrier, requests) = ParamBarrier::start(
                    request.group,
                    request.prepared,
                    fuel,
                    PARAM_PARALLEL,
                    |_, original, prepared| self.admit_prepared(original, prepared),
                    |index, original, prepared, fuel| {
                        self.correlate_member(index, original, prepared, fuel)
                    },
                )
                .map_err(|_| Private("complete prepared barrier"))?;
                ParamDispatch::Parallel { barrier, requests }
            }
            GroupKind::Sequence => {
                let sequence = ParamSequence::start(
                    request.group,
                    request.prepared,
                    fuel,
                    PARAM_PARALLEL,
                    |_, original, prepared| self.admit_prepared(original, prepared),
                    |index, original, prepared, fuel| {
                        self.correlate_member(index, original, prepared, fuel)
                    },
                )
                .map_err(|_| Private("complete prepared sequence"))?;
                ParamDispatch::Sequence(sequence)
            }
        };
        Ok(CorrelatedEffectRequest {
            identity: 73,
            declaration: self.inner.domain,
            dispatch,
        })
    }
}
impl ReplyAuthority<u64> for ParamBarrierRuntime<'_, '_, '_, '_, '_> {
    type Error = Private;
    fn authorize(&self, identity: &u64) -> Result<(), Private> {
        self.inner.authorize(identity)
    }
}
impl ReplyAuthority<(u64, usize)> for ParamBarrierRuntime<'_, '_, '_, '_, '_> {
    type Error = Private;
    fn authorize(&self, identity: &(u64, usize)) -> Result<(), Private> {
        self.inner.authorize(&identity.0)?;
        let Node::Group { branches, .. } = self.inner.domain.group else {
            return Err(Private("member group"));
        };
        self.inner
            .domain
            .schema(branches.get(identity.1).ok_or(Private("member identity"))?)
            .map(|_| ())
    }
}

#[test]
fn parsed_parameterized_helper_uses_prepared_original_member_barrier_and_one_enclosing_meter() {
    fixture(|host, env| {
        let (node, _) =
            compile_group_helpers(&parameterized_return_tree(), PROGRAM, host, env).unwrap();
        let domain = ParamDomain {
            group: only_group(&node),
            source: host,
            live: Cell::new(true),
        };
        let inner = ParamRuntime::new(&domain);
        let runtime = ParamBarrierRuntime {
            inner: &inner,
            refuse_correlation: Cell::new(false),
        };
        let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
        let EffectSessionPoll::Request(ParamDispatch::Parallel {
            mut barrier,
            requests,
        }) = session.poll(&runtime).unwrap()
        else {
            panic!()
        };
        assert_eq!(session.fuel_remaining(), 78);
        assert_eq!(inner.calls.get(), 0);
        assert_eq!(*inner.captured_sizes.borrow(), [2, 1]);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        for request in requests.into_iter().rev() {
            let index = request.index;
            let input = runtime.invoke_member(request);
            let pointer = (&*input.buffer as *const u8) as usize;
            let rejected = barrier
                .try_accept(index, &(74, index), input, &runtime)
                .unwrap_err();
            assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
            barrier
                .try_accept(index, &(73, index), rejected.reply, &runtime)
                .unwrap();
            if index == 1 {
                assert!(matches!(
                    barrier.poll().unwrap(),
                    ParallelPoll::Awaiting { remaining: 1 }
                ));
            }
            assert_eq!(session.fuel_remaining(), 78);
            assert_eq!(inner.restores.get(), 0);
        }
        let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
            panic!()
        };
        assert!(matches!(
            barrier.poll().unwrap(),
            ParallelPoll::Terminal(ParallelEnd::Completed)
        ));
        let mut reply = ParamCollected {
            group: domain.group,
            rows,
            buffer: Box::new(9),
        };
        reply.rows.swap(0, 1);
        let rejected = session.try_accept(&73, reply, &runtime).unwrap_err();
        let mut reply = rejected.reply;
        reply.rows.swap(0, 1);
        session.try_accept(&73, reply, &runtime).unwrap();
        assert_eq!(session.fuel_remaining(), 78);
        assert_eq!(inner.projections.get(), 0);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(83)))
        ));
        assert_eq!(inner.calls.get(), 2);
        assert_eq!(*inner.amounts.borrow(), [40, 41]);
        assert_eq!(inner.restores.get(), 2);
        assert_eq!(inner.projections.get(), 1);
        assert_eq!(session.fuel_remaining(), 62);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
        assert_eq!(session.fuel_remaining(), 62);
    });
}

#[test]
fn prepared_helper_barrier_cancel_revocation_or_late_correlation_denial_never_reenters_caller() {
    for phase in 0..4 {
        fixture(|host, env| {
            let (node, _) =
                compile_group_helpers(&parameterized_return_tree(), PROGRAM, host, env).unwrap();
            let domain = ParamDomain {
                group: only_group(&node),
                source: host,
                live: Cell::new(true),
            };
            let inner = ParamRuntime::new(&domain);
            let runtime = ParamBarrierRuntime {
                inner: &inner,
                refuse_correlation: Cell::new(phase == 3),
            };
            let mut session =
                StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
            if phase == 3 {
                assert!(session.poll(&runtime).is_err());
                assert_eq!(inner.calls.get(), 0);
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
            } else {
                let EffectSessionPoll::Request(ParamDispatch::Parallel {
                    mut barrier,
                    mut requests,
                }) = session.poll(&runtime).unwrap()
                else {
                    panic!()
                };
                let request = requests.pop().unwrap();
                let input = runtime.invoke_member(request);
                barrier.try_accept(1, &(73, 1), input, &runtime).unwrap();
                if phase == 0 {
                    assert!(session.cancel());
                    assert!(barrier.cancel());
                    assert!(matches!(
                        barrier.poll().unwrap(),
                        ParallelPoll::Terminal(ParallelEnd::Cancelled)
                    ));
                } else {
                    let request = requests.pop().unwrap();
                    let input = runtime.invoke_member(request);
                    barrier.try_accept(0, &(73, 0), input, &runtime).unwrap();
                    let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
                        panic!()
                    };
                    session
                        .try_accept(
                            &73,
                            ParamCollected {
                                group: domain.group,
                                rows,
                                buffer: Box::new(9),
                            },
                            &runtime,
                        )
                        .unwrap();
                    if phase == 1 {
                        assert!(session.cancel());
                    } else {
                        domain.live.set(false);
                        assert!(session.poll(&runtime).is_err());
                    }
                }
                assert_eq!(session.fuel_remaining(), 78);
            }
            assert_eq!(inner.restores.get(), 0);
            assert_eq!(inner.projections.get(), 0);
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
        });
    }
}

#[test]
fn parsed_parameterized_seq_helper_publishes_only_after_each_reply_then_restores_one_root_meter() {
    fixture(|host, env| {
        let (node, _) =
            compile_group_helpers(&parameterized_group_return_tree("seq"), PROGRAM, host, env)
                .unwrap();
        let domain = ParamDomain {
            group: only_group(&node),
            source: host,
            live: Cell::new(true),
        };
        let inner = ParamRuntime::new(&domain);
        let runtime = ParamBarrierRuntime {
            inner: &inner,
            refuse_correlation: Cell::new(false),
        };
        let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
        let EffectSessionPoll::Request(ParamDispatch::Sequence(mut sequence)) =
            session.poll(&runtime).unwrap()
        else {
            panic!()
        };
        assert_eq!(session.fuel_remaining(), 78);
        assert_eq!(inner.calls.get(), 0);
        assert_eq!(*inner.captured_sizes.borrow(), [2, 1]);
        let mut rows = Vec::new();
        for index in 0..2 {
            assert_eq!(sequence.status(), SequenceStatus::Ready { index });
            let SequencePoll::Request {
                index: published,
                request,
            } = runtime.publish_sequence(&mut sequence).unwrap()
            else {
                panic!()
            };
            assert_eq!(published, index);
            let ScalarValue::Integer(amount) = request.arguments()[0].value else {
                panic!()
            };
            assert_eq!(amount, [41, 40][index]);
            assert!(
                matches!(runtime.publish_sequence(&mut sequence).unwrap(), SequencePoll::Awaiting { index: awaiting } if awaiting == index)
            );
            let input = runtime.invoke_member(ParallelRequest { index, request });
            let pointer = (&*input.buffer as *const u8) as usize;
            let rejected = sequence
                .try_accept(&(74, index), input, &runtime)
                .unwrap_err();
            assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
            rows.push(
                sequence
                    .try_accept(&(73, index), rejected.reply, &runtime)
                    .unwrap(),
            );
            assert_eq!(inner.calls.get(), index + 1);
            assert_eq!(session.fuel_remaining(), 78);
            assert_eq!(inner.restores.get(), 0);
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Awaiting
            ));
        }
        assert_eq!(
            sequence.status(),
            SequenceStatus::Terminal(SequenceEnd::Completed)
        );
        assert!(matches!(
            runtime.publish_sequence(&mut sequence).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Completed)
        ));
        session
            .try_accept(
                &73,
                ParamCollected {
                    group: domain.group,
                    rows,
                    buffer: Box::new(9),
                },
                &runtime,
            )
            .unwrap();
        assert_eq!(inner.projections.get(), 0);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(83)))
        ));
        assert_eq!(*inner.amounts.borrow(), [41, 40]);
        assert_eq!(inner.restores.get(), 2);
        assert_eq!(inner.projections.get(), 1);
        assert_eq!(session.fuel_remaining(), 62);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
    });
}

#[test]
fn parsed_prepared_seq_cancel_or_fresh_revocation_never_invokes_the_second_member_or_caller() {
    for phase in 0..4 {
        fixture(|host, env| {
            let (node, _) =
                compile_group_helpers(&parameterized_group_return_tree("seq"), PROGRAM, host, env)
                    .unwrap();
            let domain = ParamDomain {
                group: only_group(&node),
                source: host,
                live: Cell::new(true),
            };
            let inner = ParamRuntime::new(&domain);
            let runtime = ParamBarrierRuntime {
                inner: &inner,
                refuse_correlation: Cell::new(phase == 3),
            };
            let mut session =
                StackedEffectSession::new(&node, Vec::new(), Fuel::new(100), PARAM_STACK);
            if phase == 3 {
                assert!(session.poll(&runtime).is_err());
                assert_eq!(inner.calls.get(), 0);
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
            } else {
                let EffectSessionPoll::Request(ParamDispatch::Sequence(mut sequence)) =
                    session.poll(&runtime).unwrap()
                else {
                    panic!()
                };
                if phase != 0 {
                    let SequencePoll::Request { index, request } =
                        runtime.publish_sequence(&mut sequence).unwrap()
                    else {
                        panic!()
                    };
                    let input = runtime.invoke_member(ParallelRequest { index, request });
                    let accepted = sequence.try_accept(&(73, index), input, &runtime).unwrap();
                    drop(accepted);
                    assert_eq!(inner.calls.get(), 1);
                }
                if phase == 2 {
                    domain.live.set(false);
                    assert!(runtime.publish_sequence(&mut sequence).is_err());
                    assert_eq!(
                        sequence.status(),
                        SequenceStatus::Terminal(SequenceEnd::Failed)
                    );
                } else {
                    assert!(sequence.cancel());
                    assert_eq!(
                        sequence.status(),
                        SequenceStatus::Terminal(SequenceEnd::Cancelled)
                    );
                }
                assert!(session.cancel());
                assert!(matches!(
                    runtime.publish_sequence(&mut sequence).unwrap(),
                    SequencePoll::Terminal(_)
                ));
                assert_eq!(session.fuel_remaining(), 78);
            }
            assert_eq!(inner.restores.get(), 0);
            assert_eq!(inner.projections.get(), 0);
            assert!(inner.calls.get() <= 1);
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
        });
    }
}

#[test]
fn group_helper_compilation_does_not_silently_enable_nested_suspension_scheduling() {
    fixture(|host, env| {
        let tree = program_tree(
            "fn main() = bind(answer: collect(), body: 0)\n\
             fn collect() = bind(batch: all(left: counter.left(n: 1), right: counter.right(n: 2)), body: 0)",
        );
        let (node, _) = compile_group_helpers(&tree, PROGRAM, host, env).unwrap();
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let error = evaluate_resumable_effects_in_scope(
            &node,
            &mut scope,
            &GroupPreparation(host),
            &mut fuel,
            EffectEvaluationLimits {
                pure: PureEvaluationLimits {
                    max_nodes: 256,
                    max_depth: 32,
                    max_bindings: 8,
                },
                max_arguments: 64,
                max_branches: 64,
            },
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            CalculationFailure::External(EffectEvaluationFault::NestedSuspension)
        ));
        assert_eq!(scope.len(), 0);
        assert!(fuel.remaining() < 100);
    });
}

#[test]
fn whole_unused_group_helper_headers_references_and_catalog_policy_precede_native_hooks() {
    for tail in [
        "fn unused() = all(one: counter.left(n: 1))",
        "fn unused() = all(left: counter.left(n: 1), left: counter.right(n: 2))",
        "fn unused() = seq(one: counter.left(n: missing))",
        "fn unused() = seq(one: counter.unknown(n: 1))",
        "fn unused() = seq(one: counter.right(wrong: 1))",
        "fn unused() = seq(one: fetch())\nfn fetch() = counter.left(n: 1)",
        "fn unused() = all(left: seq(one: counter.left(n: 1)), right: counter.right(n: 2))",
        "fn unused() = repeat(times: 2, body: counter.left(n: 1))",
        "fn a() = b()\nfn b() = a()",
        "fn unused() = main()",
    ] {
        fixture(|host, env| {
            let tree = program_tree(&format!("fn main() = counter.left(n: 1)\n{tail}"));
            assert!(
                compile_group_helpers(&tree, PROGRAM, host, env).is_err(),
                "{tail}"
            );
            assert_eq!(env.owner.maps.get(), 0, "{tail}");
            assert_eq!(env.owner.branches.get(), 0);
        });
    }
    fixture(|host, env| {
        let tree = program_tree("fn main() = 0\nfn unused() = seq(one: counter.right(n: 1))");
        let denied = SourceCallHost {
            catalog: host.catalog,
            version: 7,
            granted: &[31],
        };
        assert!(compile_group_helpers(&tree, PROGRAM, &denied, env).is_err());
        assert_eq!(env.owner.maps.get(), 0);
    });
}

#[test]
fn cold_group_declaration_forest_and_expanded_helper_groups_have_distinct_total_gates() {
    let two_sites = "fn main() = 0\nfn one() = seq(one: counter.left(n: 1))\nfn two() = seq(one: counter.right(n: 1))";
    fixture(|host, env| {
        let mut limits = PROGRAM;
        limits.max_groups = 1;
        assert!(compile_group_helpers(&program_tree(two_sites), limits, host, env).is_err());
        assert_eq!(env.owner.maps.get(), 0);
        limits.max_groups = 2;
        assert!(compile_group_helpers(&program_tree(two_sites), limits, host, env).is_ok());
    });
    let expanded = "fn main() = bind(first: collect(), body: collect())\nfn collect() = seq(one: counter.left(n: 1))";
    fixture(|host, env| {
        let tree = program_tree(expanded);
        let mut limits = PROGRAM;
        limits.max_groups = 1;
        assert!(compile_group_helpers(&tree, limits, host, env).is_err());
        assert_eq!(env.owner.maps.get(), 3); // One unused validation and two expansions.
        assert_eq!(env.owner.keys_dropped.get(), 3);
        assert_eq!(env.owner.declarations_dropped.get(), 3);
        limits.max_groups = 2;
        let (node, _) = compile_group_helpers(&tree, limits, host, env).unwrap();
        assert_eq!(nodes(&node), 7);
    });
}

#[test]
fn group_return_helper_wrappers_keep_inclusive_work_depth_and_whole_output_quotas() {
    let text = "fn main() = collect(n: 1)\nfn collect(n: integer) = all(left: counter.left(n: n), right: counter.right(n: n))";
    fixture(|host, env| {
        let mut limits = PROGRAM;
        limits.program.max_compilation_nodes = 13;
        limits.program.max_expansion_depth = 1;
        limits.program.expression.control.source.max_lowered_nodes = 7;
        limits.program.expression.control.source.max_lowered_depth = 3;
        let (node, _) = compile_group_helpers(&program_tree(text), limits, host, env).unwrap();
        assert_eq!(nodes(&node), 7);
    });
    for case in 0..4 {
        fixture(|host, env| {
            let mut limits = PROGRAM;
            match case {
                0 => limits.program.max_compilation_nodes = 12,
                1 => limits.program.max_expansion_depth = 0,
                2 => limits.program.expression.control.source.max_lowered_nodes = 6,
                _ => limits.program.expression.control.source.max_lowered_depth = 2,
            }
            assert!(compile_group_helpers(&program_tree(text), limits, host, env).is_err());
            assert_eq!(env.owner.keys_dropped.get(), env.owner.maps.get());
            assert_eq!(
                env.owner.declarations_dropped.get(),
                env.owner.branches.get()
            );
        });
    }
}

#[test]
fn group_return_helpers_cannot_be_scalar_operands_or_union_incompatible_exports() {
    for text in [
        "fn main() = counter.left(n: collect())\nfn collect() = seq(one: counter.left(n: 1))",
        "fn main() = wrap(n: collect())\nfn wrap(n: integer) = n\nfn collect() = seq(one: counter.left(n: 1))",
        "fn main() = collect()\nfn collect() = choose(when: true, then: seq(one: counter.left(n: 1)), otherwise: seq(other: counter.right(n: 1)))",
        "fn main() = collect()\nfn collect() = choose(when: true, then: all(left: counter.left(n: 1), right: counter.right(n: 1)), otherwise: seq(left: counter.left(n: 1), right: counter.right(n: 1)))",
    ] {
        fixture(|host, env| {
            assert!(compile_group_helpers(&program_tree(text), PROGRAM, host, env).is_err());
            assert_eq!(env.owner.keys_dropped.get(), env.owner.maps.get());
            assert_eq!(
                env.owner.declarations_dropped.get(),
                env.owner.branches.get()
            );
        });
    }
}

#[test]
fn group_helper_native_failure_or_unwind_drops_unused_validation_without_retry_or_copies() {
    let text = "fn main() = collect()\nfn collect() = all(left: counter.left(n: 1), right: counter.right(n: 2))";
    for mode in [
        Mode::BranchError,
        Mode::BranchUnwind,
        Mode::WrongDeclaration,
        Mode::UnstableGroup,
    ] {
        fixture(|host, env| {
            env.owner.mode.set(mode);
            let tree = program_tree(text);
            let before = Rc::strong_count(&env.owner);
            let result = catch_unwind(AssertUnwindSafe(|| {
                compile_group_helpers(&tree, PROGRAM, host, env)
            }));
            if mode == Mode::BranchUnwind {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().err().unwrap();
                assert!(!format!("{error} {error:?}").contains("private branch payload"));
                assert!(std::error::Error::source(&error).is_none());
            }
            assert_eq!(Rc::strong_count(&env.owner), before);
            assert_eq!(env.owner.keys_dropped.get(), env.owner.maps.get());
            assert!(env.owner.maps.get() <= 2);
        });
    }
}

#[test]
fn group_helper_profile_is_opt_in_and_entry_only_or_call_only_boundaries_are_unchanged() {
    fixture(|host, env| {
        let tree =
            program_tree("fn main() = collect()\nfn collect() = seq(one: counter.left(n: 1))");
        assert!(compile_program(&tree, "main", PROGRAM, host, env).is_err());
        let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
        let old: CatalogSourceResult<Node<'_>, Meta<'_>, Private> =
            compile_catalog_program(&declarations, "main", PROGRAM.program, host, env);
        assert!(old.is_err());
        assert_eq!(env.owner.maps.get(), 0);
        assert!(compile_group_helpers(&tree, PROGRAM, host, env).is_ok());
    });
    fixture(|host, env| {
        let limits = CatalogGroupProgramLimits {
            max_groups: 0,
            max_branches: 0,
            ..PROGRAM
        };
        let tree =
            program_tree("fn main() = inc(n: 1)\nfn inc(n: integer) = add(left: n, right: 1)");
        assert!(compile_group_helpers(&tree, limits, host, env).is_ok());
        assert_eq!(env.owner.maps.get(), 0);
    });
}
