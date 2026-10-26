//! This integration target is a separate crate: host policies must be implementable
//! without inherent impls on the foreign IR and without the product profile.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::computation_profile::ComputationProfile;
use leselang_hir::ir::Computation;

#[derive(Clone, Copy)]
enum Mode {
    Accept,
    Refuse,
    Unwind,
}

struct State {
    calls: RefCell<Vec<&'static str>>,
    mode: Cell<Mode>,
    scope: Cell<(usize, usize)>,
    groups: Cell<(usize, usize)>,
    drops: Cell<usize>,
}

impl State {
    fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            mode: Cell::new(Mode::Accept),
            scope: Cell::new((0, 0)),
            groups: Cell::new((0, 0)),
            drops: Cell::new(0),
        }
    }
}

// Deliberately no Clone, Copy, Debug, serde or thread-safety bounds.
struct Field;
struct Operation<'a>(&'a str);
struct ResultType<'a>(&'a str);
struct Group<'a>(&'a str);
struct NativeError<'a>(&'a str);

struct Host<'a, const ID: usize> {
    state: &'a State,
    label: &'a str,
}

impl<const ID: usize> Drop for Host<'_, ID> {
    fn drop(&mut self) {
        self.state.drops.set(self.state.drops.get() + 1);
    }
}

type Tree<'a, const ID: usize> = Computation<Field, Operation<'a>, Host<'a, ID>, ResultType<'a>>;

fn observe<'a, 'tree, const ID: usize>(
    tree: &'tree Tree<'a, ID>,
    name: &'static str,
) -> &'tree Host<'a, ID> {
    let Computation::Host { effect } = tree else {
        panic!("unexpected traversal or substituted tree");
    };
    effect.state.calls.borrow_mut().push(name);
    if matches!(effect.state.mode.get(), Mode::Unwind) {
        panic!("native observation unwound");
    }
    effect
}

fn validate<'a, const ID: usize>(host: &Host<'a, ID>) -> Result<(), NativeError<'a>> {
    if matches!(host.state.mode.get(), Mode::Refuse) {
        Err(NativeError(host.label))
    } else {
        Ok(())
    }
}

impl<'a, const ID: usize> ComputationProfile<Field, Operation<'a>, ResultType<'a>>
    for Host<'a, ID>
{
    type Error = NativeError<'a>;
    type GroupLocal = Group<'a>;
    type Capability = &'a str;

    fn validate_structure(tree: &Tree<'a, ID>) -> Result<(), Self::Error> {
        validate(observe(tree, "structure"))
    }

    fn is_atomic_tail(tree: &Tree<'a, ID>) -> bool {
        observe(tree, "tail");
        ID == 0
    }

    fn prepared_atomic_operation(tree: &Tree<'a, ID>) -> Option<Operation<'a>> {
        let host = observe(tree, "operation");
        (ID == 0).then_some(Operation(host.label))
    }

    fn is_atomic_capture(tree: &Tree<'a, ID>) -> bool {
        observe(tree, "capture");
        ID == 0
    }

    fn is_result_chain(tree: &Tree<'a, ID>) -> bool {
        observe(tree, "chain");
        ID == 0
    }

    fn atomic_chain_bound(tree: &Tree<'a, ID>) -> Option<usize> {
        observe(tree, "chain-bound");
        (ID == 0).then_some(7)
    }

    fn atomic_flow_bound(tree: &Tree<'a, ID>) -> Option<usize> {
        observe(tree, "flow-bound");
        (ID == 0).then_some(11)
    }

    fn is_result_flow(tree: &Tree<'a, ID>) -> bool {
        observe(tree, "flow");
        ID == 0
    }

    fn can_return_without_suspending(tree: &Tree<'a, ID>) -> bool {
        observe(tree, "early-return");
        ID == 0
    }

    fn validate_in_scope(
        tree: &Tree<'a, ID>,
        scope: &[(String, ResultType<'a>)],
    ) -> Result<ResultType<'a>, Self::Error> {
        let host = observe(tree, "scope");
        host.state.scope.set((scope.as_ptr() as usize, scope.len()));
        validate(host)?;
        Ok(ResultType(scope.first().map_or(host.label, |row| row.1.0)))
    }

    fn validate_in_group_scope(
        tree: &Tree<'a, ID>,
        scope: &[(String, ResultType<'a>)],
        groups: &[Self::GroupLocal],
    ) -> Result<ResultType<'a>, Self::Error> {
        let host = observe(tree, "groups");
        host.state.scope.set((scope.as_ptr() as usize, scope.len()));
        host.state
            .groups
            .set((groups.as_ptr() as usize, groups.len()));
        validate(host)?;
        Ok(ResultType(groups.first().map_or(host.label, |row| row.0)))
    }

    fn required_capabilities(tree: &Tree<'a, ID>) -> BTreeSet<Self::Capability> {
        let host = observe(tree, "capabilities");
        BTreeSet::from([host.label])
    }
}

fn tree<'a, const ID: usize>(state: &'a State, label: &'a str) -> Tree<'a, ID> {
    Computation::Host {
        effect: Box::new(Host { state, label }),
    }
}

fn exercise<const ID: usize>() {
    let state = State::new();
    let label = String::from("borrowed-host");
    let tree = tree::<ID>(&state, &label);
    assert!(tree.validate_structure().is_ok());
    assert_eq!(tree.is_atomic_tail(), ID == 0);
    match (tree.prepared_atomic_operation(), ID) {
        (Some(Operation(value)), 0) => assert_eq!(value, label),
        (None, 1) => {}
        _ => panic!("operation changed"),
    }
    assert_eq!(tree.is_atomic_capture(), ID == 0);
    assert_eq!(tree.is_result_chain(), ID == 0);
    assert_eq!(tree.atomic_chain_bound(), (ID == 0).then_some(7));
    assert_eq!(tree.atomic_flow_bound(), (ID == 0).then_some(11));
    assert_eq!(tree.is_result_flow(), ID == 0);
    assert_eq!(tree.can_return_without_suspending(), ID == 0);

    // No preflight, clone, deduplication, reordering or replacement by the facade.
    let scope = vec![
        ("same".into(), ResultType("first")),
        ("same".into(), ResultType("second")),
    ];
    let groups = vec![Group("first-group"), Group("second-group")];
    let Ok(ResultType(value)) = tree.validate_in_scope(&scope) else {
        panic!("scope rejected")
    };
    assert_eq!(value, "first");
    assert_eq!(state.scope.get(), (scope.as_ptr() as usize, scope.len()));
    let Ok(ResultType(value)) = tree.validate_in_group_scope(&scope, &groups) else {
        panic!("group rejected")
    };
    assert_eq!(value, "first-group");
    assert_eq!(state.scope.get(), (scope.as_ptr() as usize, scope.len()));
    assert_eq!(state.groups.get(), (groups.as_ptr() as usize, groups.len()));
    assert_eq!(
        tree.required_capabilities(),
        BTreeSet::from([label.as_str()])
    );
    assert_eq!(
        *state.calls.borrow(),
        [
            "structure",
            "tail",
            "operation",
            "capture",
            "chain",
            "chain-bound",
            "flow-bound",
            "flow",
            "early-return",
            "scope",
            "groups",
            "capabilities",
        ]
    );
    assert_eq!(state.drops.get(), 0);
    drop(tree);
    assert_eq!(state.drops.get(), 1);
}

#[test]
fn external_profile_keeps_all_twelve_method_paths_and_borrowed_move_only_slots() {
    exercise::<0>();
    exercise::<1>();
}

#[test]
fn errors_keep_native_identity_and_do_not_retry_other_observations() {
    let state = State::new();
    let message = String::from("native-refusal");
    let tree = tree::<0>(&state, &message);
    state.mode.set(Mode::Refuse);
    for error in [
        tree.validate_structure().err(),
        tree.validate_in_scope(&[]).err(),
        tree.validate_in_group_scope(&[], &[]).err(),
    ] {
        let Some(NativeError(value)) = error else {
            panic!("lost refusal")
        };
        assert!(std::ptr::eq(value, message.as_str()));
    }
    assert_eq!(*state.calls.borrow(), ["structure", "scope", "groups"]);
    assert_eq!(state.drops.get(), 0);
}

#[test]
fn unwind_propagates_once_and_keeps_borrowed_tree_owned_by_caller() {
    let state = State::new();
    let tree = tree::<1>(&state, "unwind");
    state.mode.set(Mode::Unwind);
    assert!(catch_unwind(AssertUnwindSafe(|| tree.atomic_flow_bound())).is_err());
    assert_eq!(*state.calls.borrow(), ["flow-bound"]);
    assert_eq!(state.drops.get(), 0);
    state.mode.set(Mode::Accept);
    assert_eq!(tree.atomic_flow_bound(), None);
    assert_eq!(*state.calls.borrow(), ["flow-bound", "flow-bound"]);
    drop(tree);
    assert_eq!(state.drops.get(), 1);
}

#[test]
fn current_policy_is_observed_again_instead_of_cached_after_success_or_refusal() {
    let state = State::new();
    let tree = tree::<0>(&state, "current");
    assert!(tree.validate_structure().is_ok());
    state.mode.set(Mode::Refuse);
    assert!(matches!(
        tree.validate_structure(),
        Err(NativeError("current"))
    ));
    state.mode.set(Mode::Accept);
    assert!(tree.validate_structure().is_ok());
    assert_eq!(
        *state.calls.borrow(),
        ["structure", "structure", "structure"]
    );
}
