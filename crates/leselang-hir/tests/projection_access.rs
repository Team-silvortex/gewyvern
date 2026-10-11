use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::projection_access::{
    ProjectionAccessError, ProjectionAccessLimits, ProjectionAccessPhase, ProjectionAccessPolicy,
    observe_projection_accesses,
};
use leselang_runtime_core::{
    BinaryOperator, ScalarValue, ScopeFrame, StructureError, UnaryOperator,
};

struct Key(u8);
struct Operation(&'static str, u8);
struct Host(&'static str, u8);
struct Tag;
type Node = Computation<Key, Operation, Host, Tag>;

struct Observation {
    mask: u8,
    group: bool,
    drops: Rc<Cell<usize>>,
    poison: bool,
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if self.poison {
            panic!("trusted native observation destructor");
        }
    }
}

struct Failure(Rc<Cell<usize>>);
impl Drop for Failure {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[derive(Default)]
struct Policy {
    events: Vec<&'static str>,
    drops: Rc<Cell<usize>>,
    errors: Rc<Cell<usize>>,
    refuse: Option<&'static str>,
    unwind: Option<&'static str>,
    poison_saved: bool,
}

impl Policy {
    fn value(&self, mask: u8) -> Observation {
        Observation {
            mask,
            group: false,
            drops: self.drops.clone(),
            poison: false,
        }
    }

    fn hook(&mut self, label: &'static str) -> Result<(), Failure> {
        self.events.push(label);
        assert_ne!(self.unwind, Some(label), "trusted native hook unwind");
        if self.refuse == Some(label) {
            Err(Failure(self.errors.clone()))
        } else {
            Ok(())
        }
    }
}

impl<'a> ProjectionAccessPolicy<'a, Key, Operation, Host> for Policy {
    type Observation = Observation;
    type Error = Failure;

    fn local(&mut self, value: Option<&Observation>) -> Result<Option<Observation>, Failure> {
        self.hook("local")?;
        Ok(value.map(|value| {
            let mut output = self.value(value.mask);
            output.group = value.group;
            output
        }))
    }

    fn field(&mut self, value: Option<&Observation>, field: &'a Key) -> Result<(), Failure> {
        self.hook("field")?;
        if value.is_some_and(|value| value.mask & field.0 != 0) {
            Ok(())
        } else {
            Err(Failure(self.errors.clone()))
        }
    }

    fn host(&mut self, effect: &'a Host) -> Result<Option<Observation>, Failure> {
        self.hook(effect.0)?;
        let mut value = self.value(effect.1);
        value.poison = self.poison_saved && effect.0 == "saved";
        Ok(Some(value))
    }

    fn call(&mut self, operation: &'a Operation) -> Result<Option<Observation>, Failure> {
        self.hook(operation.0)?;
        Ok(Some(self.value(operation.1)))
    }

    fn member(
        &mut self,
        value: Option<&Observation>,
        _: &'a str,
        _: &'a str,
        operation: &'a Operation,
    ) -> Result<Option<Observation>, Failure> {
        self.hook("member")?;
        if value.is_some_and(|value| value.group) {
            Ok(Some(self.value(operation.1)))
        } else {
            Err(Failure(self.errors.clone()))
        }
    }

    fn join(
        &mut self,
        left: Option<Observation>,
        right: Option<Observation>,
    ) -> Result<Option<Observation>, Failure> {
        self.hook("join")?;
        Ok(match (left, right) {
            (Some(left), Some(right)) => Some(self.value(left.mask & right.mask)),
            _ => None,
        })
    }
}

fn limits() -> ProjectionAccessLimits {
    ProjectionAccessLimits {
        max_nodes: 1024,
        max_depth: 64,
    }
}
fn literal() -> Node {
    Node::Literal {
        value: ScalarValue::None,
    }
}
fn host(name: &'static str, mask: u8) -> Node {
    Node::Host {
        effect: Box::new(Host(name, mask)),
    }
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn field(value: Node, mask: u8) -> Node {
    Node::Field {
        value: Box::new(value),
        field: Key(mask),
    }
}

fn refused<Value, Error>(result: Result<Value, Error>) -> Error {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected refusal without native Debug bounds"),
    }
}

#[test]
fn all_control_nodes_follow_original_order_and_cold_scopes_without_evaluation() {
    let node = Node::Strings {
        items: vec![
            field(bind("alias", host("bound", 1), local("alias")), 1),
            Node::Choose {
                when: Box::new(literal()),
                then: Box::new(host("left", 3)),
                otherwise: Box::new(host("right", 1)),
            },
            Node::Fold {
                name: "acc".into(),
                item: "entry".into(),
                limit: 0,
                items: Box::new(host("items", 1)),
                initial: Box::new(host("fold_initial", 1)),
                next: Box::new(Node::Strings {
                    items: vec![local("acc"), local("entry")],
                }),
            },
            Node::Loop {
                name: "state".into(),
                limit: 0,
                initial: Box::new(host("loop_initial", 1)),
                condition: Box::new(local("state")),
                next: Box::new(host("loop_next", 1)),
            },
            Node::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(host("binary_left", 1)),
                right: Box::new(host("binary_right", 1)),
            },
            Node::Recover {
                value: Box::new(host("recover_value", 1)),
                fallback: Box::new(host("recover_cold", 1)),
            },
            Node::Unary {
                operator: UnaryOperator::Not,
                value: Box::new(host("unary", 1)),
            },
            Node::Call {
                operation: Operation("call", 1),
                arguments: vec![
                    ComputedArgument {
                        name: "one".into(),
                        value: host("arg_one", 1),
                    },
                    ComputedArgument {
                        name: "two".into(),
                        value: host("arg_two", 1),
                    },
                ],
            },
            Node::Group {
                group_kind: GroupKind::Parallel,
                branches: vec![
                    ComputedBranch {
                        name: "one".into(),
                        value: host("branch_one", 1),
                        result_type: Tag,
                    },
                    ComputedBranch {
                        name: "two".into(),
                        value: host("branch_two", 1),
                        result_type: Tag,
                    },
                ],
            },
            Node::Member {
                group: "root".into(),
                name: "button".into(),
                operation: Operation("ignored", 1),
            },
            local("unknown"),
            literal(),
        ],
    };
    let mut policy = Policy::default();
    let mut root = policy.value(1);
    root.group = true;
    let mut values = vec![("root", Some(root))];
    let mut scope = ScopeFrame::new(&mut values);
    assert!(
        observe_projection_accesses(&node, &mut scope, limits(), &mut policy)
            .unwrap()
            .is_none()
    );
    assert_eq!(scope.len(), 1);
    assert!(scope.get("alias").is_none());
    assert_eq!(
        policy.events,
        [
            "bound",
            "local",
            "field",
            "left",
            "right",
            "join",
            "items",
            "fold_initial",
            "local",
            "local",
            "loop_initial",
            "local",
            "loop_next",
            "binary_left",
            "binary_right",
            "recover_value",
            "recover_cold",
            "unary",
            "arg_one",
            "arg_two",
            "call",
            "branch_one",
            "branch_two",
            "member",
            "local"
        ]
    );
}

#[test]
fn closed_join_and_member_policy_are_explicit_and_never_inferred_from_native_tags() {
    let node = field(
        Node::Choose {
            when: Box::new(literal()),
            then: Box::new(host("left", 3)),
            otherwise: Box::new(host("right", 1)),
        },
        2,
    );
    let member = Node::Member {
        group: "absent".into(),
        name: "member".into(),
        operation: Operation("op", 1),
    };
    let mut policy = Policy::default();
    let mut values = vec![];
    let mut scope = ScopeFrame::new(&mut values);
    let error = refused(observe_projection_accesses(
        &node,
        &mut scope,
        limits(),
        &mut policy,
    ));
    assert!(matches!(
        error,
        ProjectionAccessError::Native {
            phase: ProjectionAccessPhase::Field,
            ..
        }
    ));
    assert_eq!(policy.events, ["left", "right", "join", "field"]);
    assert!(!format!("{error:?} {error}").contains("Failure"));
    assert!(std::error::Error::source(&error).is_none());
    drop(error);
    assert_eq!(policy.errors.get(), 1);
    assert!(matches!(
        observe_projection_accesses(&member, &mut scope, limits(), &mut policy),
        Err(ProjectionAccessError::Native {
            phase: ProjectionAccessPhase::Member,
            ..
        })
    ));
}

#[test]
fn physical_cold_preflight_rejects_before_all_native_hooks_at_inclusive_boundaries() {
    let node = Node::Choose {
        when: Box::new(literal()),
        then: Box::new(host("hot", 1)),
        otherwise: Box::new(host("cold", 1)),
    };
    for (limits, expected) in [
        (
            ProjectionAccessLimits {
                max_nodes: 0,
                max_depth: 64,
            },
            Some(StructureError::NodeLimit),
        ),
        (
            ProjectionAccessLimits {
                max_nodes: 3,
                max_depth: 64,
            },
            Some(StructureError::NodeLimit),
        ),
        (
            ProjectionAccessLimits {
                max_nodes: 4,
                max_depth: 0,
            },
            Some(StructureError::DepthLimit),
        ),
        (
            ProjectionAccessLimits {
                max_nodes: usize::MAX,
                max_depth: 64,
            },
            None,
        ),
        (
            ProjectionAccessLimits {
                max_nodes: 4,
                max_depth: 65,
            },
            None,
        ),
    ] {
        let mut policy = Policy::default();
        let mut values = vec![("parent", None)];
        let mut scope = ScopeFrame::new(&mut values);
        let error = refused(observe_projection_accesses(
            &node,
            &mut scope,
            limits,
            &mut policy,
        ));
        match expected {
            Some(expected) => assert!(
                matches!(error, ProjectionAccessError::Structure(actual) if actual == expected)
            ),
            None => assert!(matches!(error, ProjectionAccessError::InvalidLimits)),
        }
        assert!(policy.events.is_empty());
        assert_eq!(scope.len(), 1);
    }
    let mut policy = Policy::default();
    let mut values = vec![];
    let mut scope = ScopeFrame::new(&mut values);
    let value = observe_projection_accesses(
        &node,
        &mut scope,
        ProjectionAccessLimits {
            max_nodes: 4,
            max_depth: 1,
        },
        &mut policy,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value.mask, 1);
}

#[test]
fn native_refusals_keep_original_error_identity_phase_and_stop_later_hooks() {
    for (node, refused_label, phase) in [
        (local("unknown"), "local", ProjectionAccessPhase::Local),
        (
            field(host("value", 1), 1),
            "field",
            ProjectionAccessPhase::Field,
        ),
        (host("native", 1), "native", ProjectionAccessPhase::Host),
        (
            Node::Call {
                operation: Operation("operation", 1),
                arguments: vec![],
            },
            "operation",
            ProjectionAccessPhase::Call,
        ),
        (
            Node::Member {
                group: "unknown".into(),
                name: "member".into(),
                operation: Operation("operation", 1),
            },
            "member",
            ProjectionAccessPhase::Member,
        ),
        (
            Node::Choose {
                when: Box::new(literal()),
                then: Box::new(host("left", 1)),
                otherwise: Box::new(host("right", 1)),
            },
            "join",
            ProjectionAccessPhase::Join,
        ),
    ] {
        let node = Node::Strings {
            items: vec![node, host("never", 1)],
        };
        let mut policy = Policy {
            refuse: Some(refused_label),
            ..Policy::default()
        };
        let mut values = vec![];
        let error = refused(observe_projection_accesses(
            &node,
            &mut ScopeFrame::new(&mut values),
            limits(),
            &mut policy,
        ));
        let ProjectionAccessError::Native {
            phase: actual,
            error,
        } = error
        else {
            panic!("native refusal");
        };
        assert_eq!(actual, phase);
        assert!(Rc::ptr_eq(&error.0, &policy.errors));
        assert!(!policy.events.contains(&"never"));
        drop(error);
        assert_eq!(policy.errors.get(), 1);
    }
}

#[test]
fn value_preparation_precedes_duplicate_refusal_and_loop_fold_locals_grant_no_fields() {
    for node in [
        bind("parent", host("prepared", 1), host("never", 1)),
        Node::Fold {
            name: "tmp".into(),
            item: "tmp".into(),
            items: Box::new(literal()),
            initial: Box::new(literal()),
            next: Box::new(host("never", 1)),
            limit: 0,
        },
    ] {
        let mut policy = Policy::default();
        let mut values = vec![("parent", None)];
        let mut scope = ScopeFrame::new(&mut values);
        assert!(matches!(
            observe_projection_accesses(&node, &mut scope, limits(), &mut policy),
            Err(ProjectionAccessError::DuplicateBinding)
        ));
        assert!(!policy.events.contains(&"never"));
        assert_eq!(scope.len(), 1);
        assert!(scope.get("tmp").is_none());
    }
    let node = Node::Loop {
        name: "tmp".into(),
        initial: Box::new(host("initial", 3)),
        condition: Box::new(literal()),
        next: Box::new(field(local("tmp"), 1)),
        limit: 0,
    };
    let mut policy = Policy::default();
    let mut values = vec![];
    assert!(
        observe_projection_accesses(
            &node,
            &mut ScopeFrame::new(&mut values),
            limits(),
            &mut policy
        )
        .is_err()
    );
    assert!(values.is_empty());
}

#[test]
fn refusal_and_hook_unwind_restore_nested_suffix_without_replaying_or_rolling_back_policy() {
    let node = bind(
        "saved",
        host("saved", 1),
        bind("inner", host("inner", 1), host("body", 1)),
    );
    for unwind in [false, true] {
        let mut policy = Policy::default();
        if unwind {
            policy.unwind = Some("body");
        } else {
            policy.refuse = Some("body");
        }
        let mut values = vec![("parent", None)];
        let mut scope = ScopeFrame::new(&mut values);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            observe_projection_accesses(&node, &mut scope, limits(), &mut policy)
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = refused(outcome.unwrap());
            assert!(matches!(
                error,
                ProjectionAccessError::Native {
                    phase: ProjectionAccessPhase::Host,
                    ..
                }
            ));
            drop(error);
            assert_eq!(policy.errors.get(), 1);
        }
        assert_eq!(scope.len(), 1);
        assert!(scope.get("saved").is_none());
        assert!(scope.get("inner").is_none());
        assert_eq!(policy.events, ["saved", "inner", "body"]);
        assert_eq!(policy.drops.get(), 2);
    }
}

#[test]
fn scope_destructor_unwind_releases_owned_output_and_native_error_before_handoff() {
    let node = bind("saved", host("saved", 1), host("body", 1));
    for refuse in [false, true] {
        let mut policy = Policy {
            poison_saved: true,
            refuse: refuse.then_some("body"),
            ..Policy::default()
        };
        let mut values = vec![("parent", None)];
        let mut scope = ScopeFrame::new(&mut values);
        assert!(
            catch_unwind(AssertUnwindSafe(|| observe_projection_accesses(
                &node,
                &mut scope,
                limits(),
                &mut policy
            )))
            .is_err()
        );
        assert_eq!(scope.len(), 1);
        assert_eq!(policy.events, ["saved", "body"]);
        assert_eq!(policy.drops.get(), if refuse { 1 } else { 2 });
        assert_eq!(policy.errors.get(), usize::from(refuse));
    }
}

#[test]
fn field_temporary_destructor_unwind_keeps_native_refusal_locally_owned() {
    let node = field(host("saved", 1), 2);
    let mut policy = Policy {
        poison_saved: true,
        ..Policy::default()
    };
    let mut values = vec![];
    assert!(
        catch_unwind(AssertUnwindSafe(|| observe_projection_accesses(
            &node,
            &mut ScopeFrame::new(&mut values),
            limits(),
            &mut policy
        )))
        .is_err()
    );
    assert_eq!(policy.errors.get(), 1);
    assert_eq!(policy.drops.get(), 1);
    assert!(values.is_empty());
}

#[test]
fn conditional_refusal_cleanup_releases_error_if_left_native_destructor_unwinds() {
    let node = Node::Choose {
        when: Box::new(literal()),
        then: Box::new(host("saved", 1)),
        otherwise: Box::new(host("body", 1)),
    };
    let mut policy = Policy {
        poison_saved: true,
        refuse: Some("body"),
        ..Policy::default()
    };
    let mut values = vec![];
    assert!(
        catch_unwind(AssertUnwindSafe(|| observe_projection_accesses(
            &node,
            &mut ScopeFrame::new(&mut values),
            limits(),
            &mut policy
        )))
        .is_err()
    );
    assert_eq!(policy.events, ["saved", "body"]);
    assert_eq!(policy.errors.get(), 1);
    assert_eq!(policy.drops.get(), 1);
    assert!(values.is_empty());
}

#[test]
fn independent_borrowed_schema_requires_no_native_traits_or_product_field_masks() {
    struct NativeField;
    struct NativeOperation;
    struct NativeEffect(String);
    struct NativeResult;
    struct Borrowed;
    impl<'a> ProjectionAccessPolicy<'a, NativeField, NativeOperation, NativeEffect> for Borrowed {
        type Observation = &'a str;
        type Error = &'a str;
        fn local(&mut self, value: Option<&&'a str>) -> Result<Option<&'a str>, &'a str> {
            Ok(value.copied())
        }
        fn field(&mut self, _: Option<&&'a str>, _: &'a NativeField) -> Result<(), &'a str> {
            Ok(())
        }
        fn host(&mut self, value: &'a NativeEffect) -> Result<Option<&'a str>, &'a str> {
            Ok(Some(&value.0))
        }
        fn call(&mut self, _: &'a NativeOperation) -> Result<Option<&'a str>, &'a str> {
            Ok(None)
        }
        fn member(
            &mut self,
            _: Option<&&'a str>,
            _: &'a str,
            _: &'a str,
            _: &'a NativeOperation,
        ) -> Result<Option<&'a str>, &'a str> {
            Ok(None)
        }
        fn join(
            &mut self,
            left: Option<&'a str>,
            _: Option<&'a str>,
        ) -> Result<Option<&'a str>, &'a str> {
            Ok(left)
        }
    }
    let node: Computation<NativeField, NativeOperation, NativeEffect, NativeResult> =
        Computation::Bind {
            name: "native-alias".into(),
            value: Box::new(Computation::Host {
                effect: Box::new(NativeEffect("borrowed-native-data".into())),
            }),
            body: Box::new(Computation::Local {
                name: "native-alias".into(),
            }),
        };
    let mut values = vec![];
    let value = observe_projection_accesses(
        &node,
        &mut ScopeFrame::new(&mut values),
        limits(),
        &mut Borrowed,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value, "borrowed-native-data");
    assert!(values.is_empty());
}
