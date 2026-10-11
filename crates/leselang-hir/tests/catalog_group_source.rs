use leselang_hir::{
    call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope},
    catalog_group_source::*,
    catalog_program::{CatalogProgramLimits, compile_catalog_program},
    catalog_source::{
        CatalogSourceEnvironment, CatalogSourceLimits, CatalogSourceResult, compile_catalog_source,
    },
    control_source::ControlSourceLimits,
    flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
    ir::{Computation, GroupKind},
    pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits},
    pure_typing::{PureType, PureTypeEnvironment},
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
