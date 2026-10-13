//! Host-owned byte-codec models, not process-crash or external-effect transactions.
use std::{
    borrow::Borrow,
    cell::Cell,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    group_reply::{GroupReply, GroupReplyError, assemble_group_reply, restore_group_reply},
    ir::{Computation, ComputedBranch, GroupKind},
    parallel_evaluation::{
        ParallelEvaluation, ParallelPoll, SavedParallelMember, SavedParallelReply,
    },
    prepared_parallel::PreparedParallelSession,
    prepared_sequence::PreparedSequenceSession,
    sequence_evaluation::*,
};
use leselang_runtime_core::{Fuel, HostResultDomain, PendingReply, ReplyAuthority};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

type Node = Computation<(), usize, (), usize>;
type NativeDomain<T> = dyn HostResultDomain<T, Error = &'static str>;
type Row<'a, T> = SequenceAccepted<'a, Node, usize, u64, NativeDomain<T>, Input<T>>;
type Group<'a, T> = GroupReply<'a, Node, usize, u64, NativeDomain<T>, Input<T>>;
const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 65,
    max_depth: 1,
    max_branches: 64,
};
const KINDS: [GroupKind; 2] = [GroupKind::Sequence, GroupKind::Parallel];

trait Data: Serialize + DeserializeOwned + Eq + Debug {
    fn sample(index: usize) -> Self;
}
impl Data for u64 {
    fn sample(index: usize) -> Self {
        40 + index as u64
    }
}
impl Data for String {
    fn sample(index: usize) -> Self {
        format!("panel-{index}")
    }
}
struct Input<T> {
    data: Box<T>,
    drops: Rc<Cell<usize>>,
}
impl<T> Borrow<T> for Input<T> {
    fn borrow(&self) -> &T {
        &self.data
    }
}
impl<T> Drop for Input<T> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Domain {
    index: usize,
    denied: Cell<bool>,
}
impl<T: Data> HostResultDomain<T> for Domain {
    type Error = &'static str;
    fn matches_type(&self, _: &T) -> bool {
        true
    }
    fn validate_value(&self, value: &T) -> Result<(), Self::Error> {
        (!self.denied.get() && *value == T::sample(self.index))
            .then_some(())
            .ok_or("domain")
    }
}
struct Host {
    domains: Vec<Domain>,
    generation: Cell<u64>,
    granted: Cell<bool>,
    preparations: Cell<usize>,
    dispatches: Cell<usize>,
}
impl Host {
    fn new(count: usize) -> Self {
        Self {
            domains: (0..count)
                .map(|index| Domain {
                    index,
                    denied: Cell::new(false),
                })
                .collect(),
            generation: Cell::new(1),
            granted: Cell::new(true),
            preparations: Cell::new(0),
            dispatches: Cell::new(0),
        }
    }
    fn key(&self, index: usize) -> u64 {
        self.generation.get() * 100 + index as u64
    }
}
impl ReplyAuthority<u64> for Host {
    type Error = &'static str;
    fn authorize(&self, id: &u64) -> Result<(), Self::Error> {
        (self.granted.get() && *id / 100 == self.generation.get())
            .then_some(())
            .ok_or("authority")
    }
}
struct Environment<'a, T>(&'a Host, std::marker::PhantomData<T>);
impl<'a, T: Data + 'static> SequenceEvaluationEnvironment<'a, (), usize, (), usize>
    for Environment<'a, T>
{
    type Identity = u64;
    type Declaration = NativeDomain<T>;
    type Request = usize;
    type Error = &'static str;
    fn preflight_member(
        &self,
        _: usize,
        _: &'a ComputedBranch<Node, usize>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn prepare_member(
        &self,
        index: usize,
        _: &'a ComputedBranch<Node, usize>,
        _: &mut Fuel,
    ) -> SequencePreparation<'a, u64, Self::Declaration, usize, Self::Error> {
        self.0.preparations.set(self.0.preparations.get() + 1);
        Ok(PreparedSequenceMember {
            identity: self.0.key(index),
            declaration: &self.0.domains[index],
            request: index,
        })
    }
}
fn group(kind: GroupKind, count: usize) -> Node {
    Node::Group {
        group_kind: kind,
        branches: (0..count)
            .map(|index| ComputedBranch {
                name: format!("m{index}"),
                result_type: index,
                value: Node::Call {
                    operation: index,
                    arguments: Vec::new(),
                },
            })
            .collect(),
    }
}
fn branches(node: &Node) -> &[ComputedBranch<Node, usize>] {
    let Node::Group { branches, .. } = node else {
        panic!()
    };
    branches
}
fn input<T: Data>(index: usize, drops: &Rc<Cell<usize>>) -> Input<T> {
    Input {
        data: Box::new(T::sample(index)),
        drops: drops.clone(),
    }
}
fn complete<'a, T: Data + 'static>(
    node: &'a Node,
    host: &'a Host,
    drops: &Rc<Cell<usize>>,
    budget: u64,
) -> (Vec<Row<'a, T>>, Fuel) {
    let env = Environment::<T>(host, std::marker::PhantomData);
    let Node::Group { group_kind, .. } = node else {
        panic!()
    };
    match group_kind {
        GroupKind::Sequence => {
            let mut session =
                SequenceEvaluation::start(node, &env, Fuel::new(budget), LIMITS).unwrap();
            let mut rows = Vec::new();
            loop {
                match session.poll(&env).unwrap() {
                    SequencePoll::Request { index, .. } => {
                        host.dispatches.set(host.dispatches.get() + 1);
                        rows.push(
                            session
                                .try_accept::<_, T, _>(&host.key(index), input(index, drops), host)
                                .unwrap(),
                        );
                    }
                    SequencePoll::Terminal(SequenceEnd::Completed) => break,
                    _ => panic!(),
                }
            }
            (rows, session.into_completed_fuel().unwrap())
        }
        GroupKind::Parallel => {
            let mut session =
                ParallelEvaluation::start(node, &env, Fuel::new(budget), LIMITS).unwrap();
            let ParallelPoll::Requests(requests) = session.poll(&env).unwrap() else {
                panic!()
            };
            for request in requests.into_iter().rev() {
                host.dispatches.set(host.dispatches.get() + 1);
                session
                    .try_accept::<T, _>(
                        request.index,
                        &host.key(request.index),
                        input(request.index, drops),
                        host,
                    )
                    .unwrap();
            }
            let ParallelPoll::Results(rows) = session.poll(&env).unwrap() else {
                panic!()
            };
            (rows, session.into_completed_fuel().unwrap())
        }
    }
}
fn admit<T: Data + 'static>(reply: &Group<'_, T>, host: &Host) -> Result<(), &'static str> {
    for (index, member) in reply.members().iter().enumerate() {
        let (position, branch, id, domain, value) = member.as_parts();
        host.authorize(id)?;
        let expected: &NativeDomain<T> = &host.domains[index];
        if position != index
            || branch.result_type != index
            || *id != host.key(index)
            // Every domain in this fixture is Domain; vtable addresses may differ
            // across coercion sites even for the very same host-owned instance.
            || !std::ptr::addr_eq(domain, expected)
        {
            return Err("original row");
        }
        domain.validate_value(&value.data)?;
    }
    Ok(())
}

#[test]
fn restored_groups_move_unsized_domain_rows_without_charging_assembly_again() {
    fn restore_without_native_traits<'a, Identity, Declaration: ?Sized, Reply>(
        node: &'a Node,
        rows: Vec<SequenceAccepted<'a, Node, usize, Identity, Declaration, Reply>>,
        admit: impl FnOnce(
            &GroupReply<'a, Node, usize, Identity, Declaration, Reply>,
        ) -> Result<(), &'static str>,
    ) -> GroupReply<'a, Node, usize, Identity, Declaration, Reply> {
        restore_group_reply(node, rows, LIMITS, admit).unwrap()
    }
    for kind in KINDS {
        let node = group(kind, 3);
        let host = Host::new(3);
        let drops = Rc::new(Cell::new(0));
        let (rows, mut fuel) = complete::<String>(&node, &host, &drops, 8);
        let assembled =
            assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host)).unwrap();
        assert_eq!(fuel.remaining(), 0);
        let pointers: Vec<_> = assembled
            .members()
            .iter()
            .map(|r| r.as_parts().4.data.as_ptr())
            .collect();
        let restored =
            restore_without_native_traits(&node, assembled.into_members(), |g| admit(g, &host));
        assert!(std::ptr::eq(restored.group(), &node));
        assert_eq!(restored.kind(), kind);
        for (row, pointer) in restored.members().iter().zip(pointers) {
            assert_eq!(row.as_parts().4.data.as_ptr(), pointer);
        }
        assert_eq!(fuel.remaining(), 0);
        assert_eq!(host.preparations.get(), 3);
        assert_eq!(host.dispatches.get(), 3);
        assert_eq!(drops.get(), 0);
        drop(restored);
        assert_eq!(drops.get(), 3);
    }
}

#[test]
fn complete_cold_membership_checks_precede_restore_admission() {
    for kind in KINDS {
        for case in 0..4 {
            let node = group(kind, 3);
            let foreign = group(kind, 3);
            let host = Host::new(3);
            let drops = Rc::new(Cell::new(0));
            let (mut rows, fuel) = complete::<u64>(&node, &host, &drops, 100);
            match case {
                0 => {
                    rows.pop();
                }
                1 => rows.swap(1, 2),
                2 => {}
                _ => {
                    let (mut duplicate, _) = complete::<u64>(&node, &host, &drops, 100);
                    rows[2] = duplicate.remove(0);
                }
            }
            let called = Cell::new(false);
            let result = restore_group_reply(
                if case == 2 { &foreign } else { &node },
                rows,
                LIMITS,
                |_| {
                    called.set(true);
                    Ok::<_, ()>(())
                },
            );
            assert!(matches!(
                result,
                Err(GroupReplyError::Count | GroupReplyError::Row { .. })
            ));
            assert!(!called.get());
            assert_eq!(fuel.remaining(), 96);
            assert_eq!(drops.get(), if case == 3 { 6 } else { 3 });
        }
    }
}

#[test]
fn maximum_arity_and_all_shape_limits_match_live_assembly() {
    for kind in KINDS {
        let node = group(kind, 64);
        let host = Host::new(64);
        let drops = Rc::new(Cell::new(0));
        let (rows, _) = complete::<u64>(&node, &host, &drops, 200);
        assert_eq!(
            restore_group_reply(&node, rows, LIMITS, |g| admit(g, &host))
                .unwrap()
                .members()
                .len(),
            64
        );
        for case in 0..10 {
            let mut invalid = group(kind, if case == 0 { 65 } else { 3 });
            let mut limits = LIMITS;
            match case {
                1 => limits.max_branches = 65,
                2 => limits.max_nodes = 2,
                3 => limits.max_depth = 0,
                4 => limits.max_nodes = usize::MAX,
                5 => limits.max_depth = usize::MAX,
                6 => {
                    invalid = Node::Call {
                        operation: 0,
                        arguments: vec![],
                    }
                }
                7..=9 => {
                    let Node::Group { branches, .. } = &mut invalid else {
                        panic!()
                    };
                    match case {
                        7 => branches[2].name = "bad name".into(),
                        8 => branches[2].name = "m0".into(),
                        _ => {
                            branches[2].value = Node::Local {
                                name: "notAtomic".into(),
                            }
                        }
                    }
                }
                _ => {}
            }
            let rows: Vec<Row<'_, u64>> = Vec::new();
            let result = restore_group_reply(&invalid, rows, limits, |_| -> Result<(), ()> {
                panic!("cold gate")
            });
            assert!(matches!(
                result,
                Err(GroupReplyError::InvalidLimits
                    | GroupReplyError::NotGroup
                    | GroupReplyError::Shape(_)
                    | GroupReplyError::Candidate { .. })
            ));
        }
    }
}

#[test]
fn accepted_receipts_are_not_saved_assembly_or_fresh_native_authority() {
    for kind in KINDS {
        for case in 0..4 {
            let node = group(kind, 3);
            let host = Host::new(3);
            let drops = Rc::new(Cell::new(0));
            let (rows, _) = complete::<u64>(&node, &host, &drops, 100);
            match case {
                1 => host.granted.set(false),
                2 => host.generation.set(2),
                3 => host.domains[2].denied.set(true),
                _ => {}
            }
            let result = restore_group_reply(&node, rows, LIMITS, |g| {
                if case == 0 {
                    return Err("assembly not committed");
                }
                admit(g, &host)
            });
            assert!(matches!(result, Err(GroupReplyError::Native(_))));
            assert_eq!(drops.get(), 3);
            assert_eq!(host.dispatches.get(), 3);
        }
    }
}

#[test]
fn restored_lookup_rechecks_live_policy_after_whole_group_admission() {
    for kind in KINDS {
        let node = group(kind, 3);
        let host = Host::new(3);
        let drops = Rc::new(Cell::new(0));
        let (rows, _) = complete::<String>(&node, &host, &drops, 100);
        let restored = restore_group_reply(&node, rows, LIMITS, |g| admit(g, &host)).unwrap();
        let lookup = || {
            restored.lookup(64, "m2", |r| {
                let (_, _, id, domain, value) = r.as_parts();
                host.authorize(id)?;
                domain.validate_value(&value.data)?;
                Ok::<_, &'static str>(true)
            })
        };
        assert_eq!(*lookup().unwrap().unwrap().as_parts().4.data, "panel-2");
        host.granted.set(false);
        assert!(lookup().is_err());
        host.granted.set(true);
        host.domains[2].denied.set(true);
        assert!(lookup().is_err());
        assert_eq!(host.dispatches.get(), 3);
    }
}

struct PrivateError(Box<str>);
struct OnDrop(Rc<Cell<usize>>);
impl Drop for OnDrop {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
#[test]
fn owned_native_error_and_callback_cleanup_survive_refusal_cold_failure_and_unwind() {
    for case in 0..3 {
        let node = group(GroupKind::Parallel, 3);
        let host = Host::new(3);
        let drops = Rc::new(Cell::new(0));
        let (mut rows, _) = complete::<u64>(&node, &host, &drops, 100);
        if case == 1 {
            rows.pop();
        }
        let hook_drops = Rc::new(Cell::new(0));
        let owner = OnDrop(hook_drops.clone());
        let secret: Box<str> = "private-native-detail".into();
        let pointer = secret.as_ptr();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            restore_group_reply(&node, rows, LIMITS, move |_| {
                let _owned = owner;
                assert_ne!(case, 1, "cold callback");
                assert_ne!(case, 2, "native unwind");
                Err(PrivateError(secret))
            })
        }));
        if case == 2 {
            assert!(outcome.is_err());
        } else {
            let error = outcome.unwrap().unwrap_err();
            assert!(!format!("{error:?}: {error}").contains("private-native-detail"));
            if case == 0 {
                let GroupReplyError::Native(error) = error else {
                    panic!()
                };
                assert_eq!(error.0.as_ptr(), pointer);
            } else {
                assert!(matches!(error, GroupReplyError::Count));
            }
        }
        assert_eq!(hook_drops.get(), 1);
        assert_eq!(drops.get(), 3);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image<T> {
    node: Node,
    rows: Vec<(u64, T)>,
    remaining: u64,
    assembled: bool,
}
fn save<T: Data + 'static>(kind: GroupKind, budget: u64) -> Vec<u8> {
    let node = group(kind, 3);
    let host = Host::new(3);
    let drops = Rc::new(Cell::new(0));
    let (rows, mut fuel) = complete::<T>(&node, &host, &drops, budget);
    let reply =
        assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host)).unwrap();
    let saved_rows: Vec<_> = reply
        .members()
        .iter()
        .map(|r| {
            let (_, _, id, _, value) = r.as_parts();
            (*id, &*value.data)
        })
        .collect();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "node": node, "rows": saved_rows, "remaining": fuel.remaining(), "assembled": true,
    }))
    .unwrap();
    assert_eq!(host.preparations.get(), 3);
    assert_eq!(host.dispatches.get(), 3);
    drop(reply);
    assert_eq!(drops.get(), 3);
    bytes
}
fn decode<T: Data>(candidate: &[u8], committed: &[u8]) -> Result<Image<T>, &'static str> {
    if candidate.len() > 64 * 1024 || candidate != committed {
        return Err("store admission");
    }
    let image: Image<T> = serde_json::from_slice(candidate).map_err(|_| "decode")?;
    if !image.assembled || image.remaining > 92 || image.rows.len() != 3 {
        return Err("saved assembly");
    }
    Ok(image)
}
fn rebind<'a, T: Data + 'static>(
    node: &'a Node,
    host: &'a Host,
    values: Vec<(u64, T)>,
    drops: &Rc<Cell<usize>>,
) -> Vec<Row<'a, T>> {
    let receipts: Vec<_> = values
        .into_iter()
        .enumerate()
        .map(|(index, (id, data))| {
            let domain: &NativeDomain<T> = &host.domains[index];
            PendingReply::new(id, (), domain)
                .try_accept::<_, T, _>(
                    &id,
                    Input {
                        data: Box::new(data),
                        drops: drops.clone(),
                    },
                    host,
                )
                .unwrap()
        })
        .collect();
    let Node::Group {
        group_kind,
        branches,
    } = node
    else {
        panic!()
    };
    match group_kind {
        GroupKind::Sequence => {
            let saved = receipts
                .into_iter()
                .enumerate()
                .map(|(index, reply)| SavedSequenceMember {
                    index,
                    name: &branches[index].name,
                    reply: SavedSequenceReply::<_, _, (), _>::Accepted(reply),
                })
                .collect();
            let (mut session, rows) =
                PreparedSequenceSession::restore(node, saved, LIMITS, |rows| {
                    for row in rows {
                        host.authorize(row.reply.identity())?;
                    }
                    Ok::<_, &'static str>(())
                })
                .unwrap();
            assert!(matches!(
                session
                    .poll(|_, _, _, _, _| -> Result<(), ()> { panic!("restored publication") })
                    .unwrap(),
                SequencePoll::Terminal(SequenceEnd::Completed)
            ));
            rows
        }
        GroupKind::Parallel => {
            let saved = receipts
                .into_iter()
                .enumerate()
                .map(|(index, reply)| SavedParallelMember {
                    index,
                    name: &branches[index].name,
                    reply: SavedParallelReply::Accepted(reply),
                })
                .collect();
            let mut session = PreparedParallelSession::restore(node, saved, LIMITS, |rows| {
                for row in rows {
                    host.authorize(row.reply.identity())?;
                }
                Ok::<_, &'static str>(())
            })
            .unwrap();
            let ParallelPoll::Results(rows) = session.poll().unwrap() else {
                panic!()
            };
            assert!(matches!(session.poll().unwrap(), ParallelPoll::Terminal(_)));
            rows
        }
    }
}

#[test]
fn two_byte_codec_hosts_restore_assembled_seq_and_all_after_old_objects_are_dropped() {
    fn run<T: Data + 'static>() {
        for kind in KINDS {
            for budget in [8, 9, 100] {
                let committed = save::<T>(kind, budget);
                // No old IR, schema, receipt, collector or meter survives save().
                let image = decode::<T>(&committed, &committed).unwrap();
                let host = Host::new(3);
                let drops = Rc::new(Cell::new(0));
                let rows = rebind(&image.node, &host, image.rows, &drops);
                let fuel = Fuel::new(image.remaining);
                let reply = restore_group_reply(&image.node, rows, LIMITS, |g| {
                    assert!(image.assembled);
                    assert_eq!(fuel.remaining(), budget - 8);
                    admit(g, &host)
                })
                .unwrap();
                for index in 0..3 {
                    let row = reply
                        .lookup(64, &format!("m{index}"), |r| {
                            host.authorize(r.as_parts().2)?;
                            Ok::<_, &'static str>(true)
                        })
                        .unwrap()
                        .unwrap();
                    assert!(std::ptr::eq(
                        row.as_parts().1,
                        &branches(&image.node)[index]
                    ));
                    assert_eq!(*row.as_parts().4.data, T::sample(index));
                }
                assert_eq!(fuel.remaining(), budget - 8);
                assert_eq!(host.preparations.get(), 0);
                assert_eq!(host.dispatches.get(), 0);
                drop(reply);
                assert_eq!(drops.get(), 3);
            }
        }
    }
    run::<u64>();
    run::<String>();
}

#[test]
fn forged_saved_assembly_graph_payload_or_fuel_never_reaches_rebinding() {
    fn run<T: Data + 'static>() {
        for kind in KINDS {
            let committed = save::<T>(kind, 100);
            for case in 0..5 {
                let mut image = decode::<T>(&committed, &committed).unwrap();
                match case {
                    0 => image.assembled = false,
                    1 => image.remaining += 1,
                    2 => image.rows[2].0 += 1,
                    3 => image.rows[2].1 = T::sample(9),
                    _ => {
                        let Node::Group { branches, .. } = &mut image.node else {
                            panic!()
                        };
                        branches[2].name = "redirected".into();
                    }
                }
                let forged = serde_json::to_vec(&image).unwrap();
                assert!(decode::<T>(&forged, &committed).is_err());
                if case == 0 {
                    assert!(decode::<T>(&forged, &forged).is_err());
                }
            }
            assert!(decode::<T>(&vec![b' '; 64 * 1024 + 1], &committed).is_err());
        }
    }
    run::<u64>();
    run::<String>();
}

mod captured {
    use super::*;
    use std::cell::RefCell;

    use leselang_hir::{
        effect_evaluation::{
            EffectEvaluationEnvironment, EffectEvaluationFault, EffectEvaluationLimits,
        },
        effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings},
        effect_stack::{
            EffectReturnStack, EffectStackFault, EffectStackLimits, SavedEffectReturnFrame,
            StackedControlResult, StackedEffectOutcome, evaluate_stacked_effects_in_scope,
            restore_effect_return_stack, resume_accepted_stacked_effects,
        },
        pure_evaluation::{
            PureEvaluationEnvironment, PureEvaluationFault, PureEvaluationLimits, PureValue,
            scalar_copy_cost,
        },
    };
    use leselang_runtime_core::{
        BinaryOperator, CalculationFailure, CaptureSnapshotHooks, CaptureSnapshotLimits,
        CaptureValidationLimits, CapturedBindings, CapturedValue, ScalarValue, ScopeFrame,
        snapshot_captured_scope_state, validate_captured_bindings,
    };

    const STACK: EffectStackLimits = EffectStackLimits {
        effects: EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8,
            },
            max_arguments: 1,
            max_branches: 3,
        },
        max_frames: 2,
    };
    const PENDING: u64 = 190;
    const SEED: u64 = 180;
    trait Projection: Data + Clone + 'static {
        fn scalar(&self) -> ScalarValue;
    }
    impl Projection for u64 {
        fn scalar(&self) -> ScalarValue {
            ScalarValue::Integer(*self)
        }
    }
    impl Projection for String {
        fn scalar(&self) -> ScalarValue {
            ScalarValue::String(self.clone())
        }
    }
    enum View<'a, T> {
        Atom(u64, Rc<T>),
        Group(Rc<Group<'a, T>>),
    }
    impl<T> Clone for View<'_, T> {
        fn clone(&self) -> Self {
            match self {
                Self::Atom(id, data) => Self::Atom(*id, data.clone()),
                Self::Group(group) => Self::Group(group.clone()),
            }
        }
    }
    #[derive(Serialize, Deserialize)]
    struct Atom<T> {
        id: u64,
        value: T,
    }
    type Frame<T> = CapturedBindings<Node, Atom<T>, usize>;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Checkpoint<T> {
        group: Image<T>,
        frames: Vec<Frame<T>>,
        pending: Atom<T>,
        remaining: u64,
    }
    enum Capture<'a, T> {
        Live(RestoredEffectBindings<'a, View<'a, T>>),
        Saved(&'a Frame<T>),
    }
    struct Runtime<'a, T> {
        host: &'a Host,
        active_host: Option<&'a Host>,
        saved: Option<&'a Image<T>>,
        group: RefCell<Option<Rc<Group<'a, T>>>>,
        drops: Rc<Cell<usize>>,
        restored: Cell<usize>,
        rebuilt: Cell<usize>,
        projected: Cell<usize>,
        deny_outer: Cell<bool>,
        deny_capture: Cell<bool>,
        revoke_outer: Cell<bool>,
    }
    impl<'a, T: Projection> Runtime<'a, T> {
        fn new(host: &'a Host, saved: Option<&'a Image<T>>, drops: Rc<Cell<usize>>) -> Self {
            Self {
                host,
                active_host: None,
                saved,
                group: RefCell::new(None),
                drops,
                restored: Cell::new(0),
                rebuilt: Cell::new(0),
                projected: Cell::new(0),
                deny_outer: Cell::new(false),
                deny_capture: Cell::new(false),
                revoke_outer: Cell::new(false),
            }
        }
        fn view_host(&self, identity: u64) -> &Host {
            self.active_host
                .filter(|host| identity / 100 == host.generation.get())
                .unwrap_or(self.host)
        }
        fn admit_view(&self, view: &View<'_, T>) -> Result<(), &'static str> {
            if self.deny_capture.get() {
                return Err("captured authority");
            }
            match view {
                View::Atom(id, value) => {
                    let host = self.view_host(*id);
                    host.authorize(id)?;
                    if ![PENDING, SEED].contains(id) && *id % 100 > 2 {
                        return Err("captured identity");
                    }
                    let index = if [PENDING, SEED].contains(id) {
                        0
                    } else {
                        (*id % 100) as usize
                    };
                    host.domains[index].validate_value(&**value)
                }
                View::Group(group) => {
                    admit(group, self.view_host(*group.members()[0].as_parts().2))
                }
            }
        }
        fn saved_group(&self) -> Result<Rc<Group<'a, T>>, &'static str> {
            if self.deny_capture.get() {
                return Err("captured authority");
            }
            if let Some(group) = self.group.borrow().as_ref() {
                self.admit_view(&View::Group(group.clone()))?;
                return Ok(group.clone());
            }
            let saved = self.saved.ok_or("missing snapshot")?;
            for (index, (id, value)) in saved.rows.iter().enumerate() {
                if *id != self.host.key(index) {
                    return Err("original group receipt");
                }
                self.host.authorize(id)?;
                self.host.domains[index].validate_value(value)?;
            }
            // Byte ingress and the single three-row rebuild are separate bounded host work.
            // This does not pay language preparation or completed assembly a second time.
            let rows = rebind(&saved.node, self.host, saved.rows.clone(), &self.drops);
            let group = restore_group_reply(&saved.node, rows, LIMITS, |g| {
                if !saved.assembled {
                    return Err("assembly incomplete");
                }
                admit(g, self.host)
            });
            let group = Rc::new(group.map_err(|_| "group restore")?);
            self.rebuilt.set(self.rebuilt.get() + 1);
            self.group.replace(Some(group.clone()));
            Ok(group)
        }
    }
    impl<'a, T: Projection> PureEvaluationEnvironment<(), usize> for Runtime<'a, T> {
        type Result = View<'a, T>;
        type Error = &'static str;
        fn field(&self, value: &Self::Result, _: &()) -> Result<ScalarValue, Self::Error> {
            self.admit_view(value)?;
            let View::Atom(_, value) = value else {
                return Err("atom required");
            };
            Ok(value.scalar())
        }
        fn member(
            &self,
            value: &Self::Result,
            name: &str,
            operation: &usize,
        ) -> Result<Self::Result, Self::Error> {
            self.admit_view(value)?;
            let View::Group(group) = value else {
                return Err("group required");
            };
            let row = group
                .lookup(3, name, |row| {
                    let (index, _, id, domain, value) = row.as_parts();
                    if index != *operation {
                        return Err("member operation");
                    }
                    self.view_host(*id).authorize(id)?;
                    domain.validate_value(&value.data)?;
                    Ok(true)
                })
                .map_err(|_| "member")?
                .ok_or("missing member")?;
            Ok(View::Atom(
                *row.as_parts().2,
                Rc::new((*row.as_parts().4.data).clone()),
            ))
        }
    }
    impl<'a, T: Projection> EffectEvaluationEnvironment<'a, (), usize, (), usize> for Runtime<'a, T> {
        type Request = usize;
        type Capture = Capture<'a, T>;
        fn preflight_effect(&self, node: &'a Node) -> Result<(), &'static str> {
            self.host.authorize(&PENDING)?;
            match node {
                Node::Call {
                    operation: 0,
                    arguments,
                } if arguments.is_empty() => Ok(()),
                Node::Call {
                    operation: 9,
                    arguments,
                } if arguments.is_empty() && !self.deny_outer.get() => Ok(()),
                _ => Err("cold schema"),
            }
        }
        fn prepare_effect(
            &self,
            node: &'a Node,
            _: &mut ScopeFrame<'_, 'a, PureValue<Self::Result>>,
            _: &mut Fuel,
        ) -> Result<usize, CalculationFailure<&'static str>> {
            self.preflight_effect(node)?;
            let Node::Call { operation, .. } = node else {
                unreachable!()
            };
            self.host.preparations.set(self.host.preparations.get() + 1);
            Ok(*operation)
        }
        fn capture(
            &self,
            _: &'a str,
            _: &'a Node,
            scope: &ScopeFrame<'_, 'a, PureValue<Self::Result>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Capture, &'static str> {
            let mut rows = Vec::new();
            for (name, value) in scope.bindings() {
                let copy = match value {
                    PureValue::Scalar(s) => scalar_copy_cost(s),
                    _ => 0,
                };
                fuel.charge(1 + copy).map_err(|_| "capture fuel")?;
                rows.push((*name, value.clone()));
            }
            Ok(Capture::Live(rows))
        }
    }
    impl<'a, T: Projection>
        EffectReentryEnvironment<'a, (), usize, (), usize, u64, Domain, Input<T>>
        for Runtime<'a, T>
    {
        fn restore_capture(
            &self,
            id: &u64,
            _: &Domain,
            capture: Self::Capture,
            _: &mut Fuel,
            _: EffectEvaluationLimits,
        ) -> Result<RestoredEffectBindings<'a, Self::Result>, CalculationFailure<&'static str>>
        {
            self.host.authorize(id)?;
            self.restored.set(self.restored.get() + 1);
            if self.restored.get() == 2 && self.revoke_outer.get() {
                self.host.domains[2].denied.set(true);
            }
            let rows = match capture {
                Capture::Live(rows) => rows,
                Capture::Saved(frame) => {
                    let mut rows = Vec::new();
                    for local in &frame.locals {
                        rows.push((local.name.as_str(), PureValue::Scalar(local.value.clone())));
                    }
                    for saved in &frame.results {
                        rows.push((
                            saved.name.as_str(),
                            PureValue::Result(View::Atom(
                                saved.result.id,
                                Rc::new(saved.result.value.clone()),
                            )),
                        ));
                    }
                    for saved in &frame.groups {
                        if saved.group != 0 {
                            return Err("unknown group".into());
                        }
                        rows.push((
                            saved.name.as_str(),
                            PureValue::Result(View::Group(self.saved_group()?)),
                        ));
                    }
                    rows
                }
            };
            for (_, value) in &rows {
                if let PureValue::Result(value) = value {
                    self.admit_view(value)?;
                }
            }
            let groups: Vec<_> = rows
                .iter()
                .filter_map(|(_, v)| match v {
                    PureValue::Result(View::Group(g)) => Some(g),
                    _ => None,
                })
                .collect();
            assert_eq!(groups.len(), 2);
            assert!(Rc::ptr_eq(groups[0], groups[1]));
            Ok(rows)
        }
        fn bind_reply(
            &self,
            id: &u64,
            domain: &Domain,
            reply: Input<T>,
            _: &mut Fuel,
        ) -> Result<PureValue<Self::Result>, CalculationFailure<&'static str>> {
            self.host.authorize(id)?;
            domain.validate_value(&*reply.data)?;
            self.projected.set(self.projected.get() + 1);
            Ok(PureValue::Result(View::Atom(
                *id,
                Rc::new((*reply.data).clone()),
            )))
        }
    }
    fn local(name: &str) -> Node {
        Node::Local { name: name.into() }
    }
    fn field(value: Node) -> Node {
        Node::Field {
            value: Box::new(value),
            field: (),
        }
    }
    fn literal(value: ScalarValue) -> Node {
        Node::Literal { value }
    }
    fn choose(when: Node, then: Node, otherwise: Node) -> Node {
        Node::Choose {
            when: Box::new(when),
            then: Box::new(then),
            otherwise: Box::new(otherwise),
        }
    }
    fn program<T: Projection>() -> Node {
        let equal = |name| Node::Binary {
            operator: BinaryOperator::Eq,
            left: Box::new(field(local(name))),
            right: Box::new(literal(T::sample(0).scalar())),
        };
        let selected = field(Node::Member {
            group: "alias".into(),
            name: "m2".into(),
            operation: 2,
        });
        let body = choose(
            local("keep"),
            choose(
                equal("seed"),
                choose(equal("reply"), selected, literal(ScalarValue::None)),
                literal(ScalarValue::None),
            ),
            literal(ScalarValue::None),
        );
        Node::Bind {
            name: "output".into(),
            value: Box::new(Node::Bind {
                name: "reply".into(),
                value: Box::new(Node::Call {
                    operation: 0,
                    arguments: vec![],
                }),
                body: Box::new(body),
            }),
            body: Box::new(choose(
                literal(ScalarValue::Boolean(false)),
                Node::Call {
                    operation: 9,
                    arguments: vec![],
                },
                local("output"),
            )),
        }
    }
    #[derive(Debug, Eq, PartialEq)]
    enum Failure {
        Fuel,
        Native(&'static str),
    }
    #[derive(Debug, Eq, PartialEq)]
    struct Report {
        value: Result<ScalarValue, Failure>,
        remaining: u64,
        restored: usize,
        projected: usize,
    }
    fn report<'a, T: Projection, E>(
        result: StackedControlResult<'a, (), usize, (), usize, E>,
        runtime: &Runtime<'a, T>,
        fuel: &Fuel,
    ) -> Report
    where
        E: EffectEvaluationEnvironment<
                'a,
                (),
                usize,
                (),
                usize,
                Result = View<'a, T>,
                Error = &'static str,
            >,
    {
        let value = match result {
            Ok(StackedEffectOutcome::Value(PureValue::Scalar(value))) => Ok(value),
            Ok(_) => panic!("unexpected work"),
            Err(CalculationFailure::External(EffectStackFault::Control(
                EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted),
            ))) => Err(Failure::Fuel),
            Err(CalculationFailure::External(EffectStackFault::Control(
                EffectEvaluationFault::Native(error),
            ))) => Err(Failure::Native(error)),
            Err(error) => panic!("unexpected re-entry failure: {error:?}"),
        };
        Report {
            value,
            remaining: fuel.remaining(),
            restored: runtime.restored.get(),
            projected: runtime.projected.get(),
        }
    }
    enum Initial {
        Saved(Vec<u8>),
        Finished(Report),
    }
    fn snapshots<T: Projection>(
        continuation: EffectReturnStack<'_, Node, Capture<'_, T>>,
        assembled: &Rc<Group<'_, T>>,
    ) -> Vec<Frame<T>> {
        let mut frames = Vec::new();
        for frame in continuation.into_frames() {
            let (name, body, capture) = frame.into_parts();
            let Capture::Live(mut rows) = capture else {
                panic!()
            };
            let saved = snapshot_captured_scope_state(
                name,
                body,
                &ScopeFrame::new(&mut rows),
                &mut (),
                CaptureSnapshotLimits {
                    max_bindings: 4,
                    max_name_bytes: 64,
                },
                CaptureSnapshotHooks {
                    row: |_: &str, value: &PureValue<View<'_, T>>, _: &mut ()| {
                        Ok::<_, &'static str>(match value {
                            PureValue::Scalar(s) => CapturedValue::Scalar(s.clone()),
                            PureValue::Result(View::Atom(id, v)) => CapturedValue::Result(Atom {
                                id: *id,
                                value: (**v).clone(),
                            }),
                            PureValue::Result(View::Group(g)) => {
                                assert!(Rc::ptr_eq(g, assembled));
                                CapturedValue::Group(0)
                            }
                        })
                    },
                    body: |body: &Node, _: &mut ()| Ok(body.clone()),
                },
            )
            .unwrap();
            frames.push(saved);
        }
        frames
    }
    fn initial<T: Projection>(kind: GroupKind, allowance: Option<u64>, save: bool) -> Initial {
        let node = group(kind, 3);
        let host = Host::new(3);
        let drops = Rc::new(Cell::new(0));
        let (rows, mut fuel) = complete::<T>(&node, &host, &drops, 1000);
        let assembled = Rc::new(
            assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host)).unwrap(),
        );
        let assembly_remaining = fuel.remaining();
        let runtime = Runtime::<T>::new(&host, None, drops.clone());
        let tree = program::<T>();
        let mut bindings = vec![
            ("keep", PureValue::Scalar(ScalarValue::Boolean(true))),
            (
                "seed",
                PureValue::Result(View::Atom(SEED, Rc::new(T::sample(0)))),
            ),
            ("batch", PureValue::Result(View::Group(assembled.clone()))),
            ("alias", PureValue::Result(View::Group(assembled.clone()))),
        ];
        let outcome = evaluate_stacked_effects_in_scope(
            &tree,
            &mut ScopeFrame::new(&mut bindings),
            &runtime,
            &mut fuel,
            STACK,
        )
        .unwrap();
        let StackedEffectOutcome::Suspended {
            request,
            continuation,
        } = outcome
        else {
            panic!()
        };
        assert_eq!(request, 0);
        assert_eq!(continuation.frame_count(), 2);
        host.dispatches.set(host.dispatches.get() + 1);
        let incoming = input::<T>(0, &drops);
        if let Some(allowance) = allowance {
            fuel.charge(fuel.remaining() - allowance).unwrap();
        }
        assert_eq!(host.preparations.get(), 4);
        assert_eq!(host.dispatches.get(), 4);
        if !save {
            let accepted = PendingReply::new(PENDING, continuation, &host.domains[0])
                .try_accept::<_, T, _>(&PENDING, incoming, &host)
                .unwrap();
            let result = resume_accepted_stacked_effects(accepted, &runtime, &mut fuel, STACK);
            return Initial::Finished(report::<T, Runtime<'_, T>>(result, &runtime, &fuel));
        }
        let frames = snapshots(continuation, &assembled);
        let image = Checkpoint {
            group: Image {
                node: node.clone(),
                rows: assembled
                    .members()
                    .iter()
                    .map(|r| (*r.as_parts().2, (*r.as_parts().4.data).clone()))
                    .collect(),
                remaining: assembly_remaining,
                assembled: true,
            },
            frames,
            pending: Atom {
                id: PENDING,
                value: (*incoming.data).clone(),
            },
            remaining: fuel.remaining(),
        };
        let bytes = serde_json::to_vec(&image).unwrap();
        drop((bindings, assembled, incoming));
        assert_eq!(drops.get(), 4);
        // Only host-defined bytes escape. All original runtime/schema/IR/view objects end here.
        Initial::Saved(bytes)
    }
    fn bytes<T: Projection>(kind: GroupKind, allowance: Option<u64>) -> Vec<u8> {
        let Initial::Saved(bytes) = initial::<T>(kind, allowance, true) else {
            panic!()
        };
        bytes
    }
    fn decode<T: Projection>(
        candidate: &[u8],
        committed: &[u8],
    ) -> Result<Checkpoint<T>, &'static str> {
        if candidate.len() > 64 * 1024 || candidate != committed {
            return Err("store admission");
        }
        let image: Checkpoint<T> = serde_json::from_slice(candidate).map_err(|_| "decode")?;
        if image.frames.len() != 2
            || image.group.rows.len() != 3
            || !image.group.assembled
            || image.group.remaining != 992
            || image.remaining > image.group.remaining
            || image.pending.id != PENDING
        {
            return Err("checkpoint bounds");
        }
        for (i, frame) in image.frames.iter().enumerate() {
            validate_captured_bindings(
                frame,
                CaptureValidationLimits {
                    max_saved_bindings: 4,
                    max_name_bytes: 64,
                },
                |r| {
                    if r.id == SEED && r.value == T::sample(0) {
                        Ok(())
                    } else {
                        Err("saved result")
                    }
                },
                |g| if *g == 0 { Ok(()) } else { Err("saved group") },
                |_| Ok(()),
            )
            .map_err(|_| "capture structure")?;
            if frame.name != ["reply", "output"][i]
                || frame.locals.len() != 1
                || frame.locals[0].name != "keep"
                || frame.results.len() != 1
                || frame.results[0].name != "seed"
                || frame.groups.len() != 2
                || frame.groups[0].name != "batch"
                || frame.groups[1].name != "alias"
            {
                return Err("original lexical capture");
            }
        }
        Ok(image)
    }
    fn stack<T: Projection>(image: &Checkpoint<T>) -> EffectReturnStack<'_, Node, Capture<'_, T>> {
        restore_effect_return_stack(
            image
                .frames
                .iter()
                .map(|frame| SavedEffectReturnFrame {
                    name: &frame.name,
                    body: &frame.body,
                    capture: Capture::Saved(frame),
                })
                .collect(),
            STACK,
        )
        .unwrap()
    }
    fn restore<'a, T: Projection>(
        image: &'a Checkpoint<T>,
        runtime: &Runtime<'a, T>,
        retry: bool,
    ) -> Report {
        let mut pending =
            PendingReply::new(image.pending.id, stack(image), &runtime.host.domains[0]);
        let incoming = Input {
            data: Box::new(image.pending.value.clone()),
            drops: runtime.drops.clone(),
        };
        let incoming = if retry {
            let pointer = &*incoming.data as *const T;
            let rejected = pending
                .try_accept::<_, T, _>(&999, incoming, runtime.host)
                .unwrap_err();
            assert_eq!(&*rejected.reply.data as *const T, pointer);
            assert_eq!(runtime.restored.get(), 0);
            assert_eq!(runtime.projected.get(), 0);
            rejected.reply
        } else {
            incoming
        };
        let accepted = pending
            .try_accept::<_, T, _>(&image.pending.id, incoming, runtime.host)
            .unwrap();
        let mut fuel = Fuel::new(image.remaining);
        let result = resume_accepted_stacked_effects(accepted, runtime, &mut fuel, STACK);
        let result = report::<T, Runtime<'_, T>>(result, runtime, &fuel);
        assert!(
            pending
                .try_accept::<_, T, _>(&image.pending.id, input(0, &runtime.drops), runtime.host)
                .is_err()
        );
        assert_eq!(runtime.host.preparations.get(), 0);
        assert_eq!(runtime.host.dispatches.get(), 0);
        result
    }

    #[test]
    fn byte_captured_scalar_atom_group_alias_and_two_returns_use_the_original_walker() {
        fn run<T: Projection>() {
            for kind in KINDS {
                let Initial::Finished(continuous) = initial::<T>(kind, None, false) else {
                    panic!()
                };
                let bytes = bytes::<T>(kind, None);
                let image = decode::<T>(&bytes, &bytes).unwrap();
                let host = Host::new(3);
                let runtime = Runtime::new(&host, Some(&image.group), Rc::new(Cell::new(0)));
                let recovered = restore(&image, &runtime, false);
                assert_eq!(recovered.value, Ok(T::sample(2).scalar()));
                assert_eq!(recovered, continuous);
                assert_eq!(recovered.restored, 2);
                assert_eq!(recovered.projected, 1);
                assert_eq!(runtime.rebuilt.get(), 1);
            }
        }
        run::<u64>();
        run::<String>();
    }
    #[test]
    fn captured_byte_reentry_matches_every_original_fuel_cutoff_without_refill() {
        fn run<T: Projection>() {
            for kind in KINDS {
                let bytes = bytes::<T>(kind, None);
                let image = decode::<T>(&bytes, &bytes).unwrap();
                let Initial::Finished(full) = initial::<T>(kind, None, false) else {
                    panic!()
                };
                let needed = image.remaining - full.remaining;
                assert!(needed > 8);
                for cutoff in 0..=needed + 1 {
                    let Initial::Finished(continuous) = initial::<T>(kind, Some(cutoff), false)
                    else {
                        panic!()
                    };
                    let bytes = self::bytes::<T>(kind, Some(cutoff));
                    let image = decode::<T>(&bytes, &bytes).unwrap();
                    let host = Host::new(3);
                    let runtime = Runtime::new(&host, Some(&image.group), Rc::new(Cell::new(0)));
                    let restored = restore(&image, &runtime, false);
                    assert_eq!(restored, continuous, "cutoff={cutoff}");
                    assert_eq!(restored.value.is_ok(), cutoff >= needed);
                    if cutoff < needed {
                        assert_eq!(restored.value, Err(Failure::Fuel));
                    }
                }
            }
        }
        run::<u64>();
        run::<String>();
    }
    #[test]
    fn denied_outer_cold_effect_blocks_all_decoded_capture_materialization() {
        for kind in KINDS {
            let bytes = bytes::<String>(kind, None);
            let image = decode::<String>(&bytes, &bytes).unwrap();
            let host = Host::new(3);
            let runtime = Runtime::new(&host, Some(&image.group), Rc::new(Cell::new(0)));
            runtime.deny_outer.set(true);
            let result = restore(&image, &runtime, false);
            assert!(result.value.is_err());
            assert_eq!(result.remaining, image.remaining);
            assert_eq!(runtime.restored.get(), 0);
            assert_eq!(runtime.rebuilt.get(), 0);
            assert_eq!(runtime.projected.get(), 0);
        }
    }
    #[test]
    fn saved_capture_authority_and_cached_group_policy_are_rechecked_per_return() {
        for kind in KINDS {
            for case in 0..3 {
                let late = case == 2;
                let bytes = bytes::<u64>(kind, None);
                let image = decode::<u64>(&bytes, &bytes).unwrap();
                let host = Host::new(3);
                let runtime = Runtime::new(&host, Some(&image.group), Rc::new(Cell::new(0)));
                runtime.deny_capture.set(case == 0);
                if case == 1 {
                    host.domains[2].denied.set(true);
                }
                runtime.revoke_outer.set(late);
                let result = restore(&image, &runtime, false);
                assert!(result.value.is_err());
                assert_eq!(runtime.restored.get(), if late { 2 } else { 1 });
                assert_eq!(runtime.projected.get(), usize::from(late));
                assert_eq!(runtime.rebuilt.get(), usize::from(late));
            }
        }
    }
    #[test]
    fn forged_capture_alias_scalar_frame_order_body_and_budget_fail_before_reentry() {
        let committed = bytes::<String>(GroupKind::Parallel, None);
        let oversized = vec![b' '; 64 * 1024 + 1];
        assert!(decode::<String>(&oversized, &oversized).is_err());
        for case in 0..8 {
            let mut image = decode::<String>(&committed, &committed).unwrap();
            match case {
                0 => image.frames[0].locals[0].value = ScalarValue::Boolean(false),
                1 => image.frames[0].groups[1].group = 1,
                2 => image.frames[0].groups[1].name = "seed".into(),
                3 => image.frames.swap(0, 1),
                4 => image.frames[0].body = literal(ScalarValue::Integer(999)),
                5 => image.remaining += 1,
                6 => image.frames[0].results[0].result.id = 170,
                _ => image.pending.id += 1,
            }
            let forged = serde_json::to_vec(&image).unwrap();
            assert!(decode::<String>(&forged, &committed).is_err());
            if [1, 2, 3, 6, 7].contains(&case) {
                assert!(decode::<String>(&forged, &forged).is_err());
            }
        }
    }
    #[test]
    fn restored_pending_rejects_wrong_identity_without_losing_input_or_replaying_captures() {
        for kind in KINDS {
            let bytes = bytes::<String>(kind, None);
            let image = decode::<String>(&bytes, &bytes).unwrap();
            let host = Host::new(3);
            let runtime = Runtime::new(&host, Some(&image.group), Rc::new(Cell::new(0)));
            let result = restore(&image, &runtime, true);
            assert_eq!(result.value, Ok(ScalarValue::String("panel-2".into())));
            assert_eq!(runtime.restored.get(), 2);
            assert_eq!(runtime.projected.get(), 1);
            assert_eq!(runtime.rebuilt.get(), 1);
        }
    }

    mod inflight {
        use super::*;
        use leselang_hir::{
            call_evaluation::{
                CallEvaluationLimits, PreparedArgument, PreparedCall,
                evaluate_call_arguments_in_scope,
            },
            ir::ComputedArgument,
            native_group_preparation::prepare_native_group_members,
        };
        use leselang_runtime_core::{NamedParameter, OperationSchema, ScalarTypeSet};

        type Schema<'a, T> =
            OperationSchema<'a, usize, &'a str, ScalarTypeSet, &'a NativeDomain<T>, ()>;
        type Call<'a, T> = PreparedCall<'a, usize, ScalarTypeSet, &'a NativeDomain<T>, ()>;
        type Queue<'a, T> = PreparedSequenceSession<'a, Node, usize, u64, NativeDomain<T>, usize>;
        type QueueState<'a, T> = (Queue<'a, T>, Vec<Row<'a, T>>, Option<Input<T>>);
        const ROOT: u64 = 290;
        const QUEUE_LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
            max_nodes: 64,
            max_depth: 8,
            max_branches: 3,
        };
        struct Adapter<'a, T> {
            captured: Runtime<'a, T>,
            active: &'a Host,
            schemas: &'a [Schema<'a, T>],
            inventory: RefCell<Vec<Option<Call<'a, T>>>>,
            restored_calls: Cell<usize>,
        }
        impl<'a, T: Projection> PureEvaluationEnvironment<(), usize> for Adapter<'a, T> {
            type Result = View<'a, T>;
            type Error = &'static str;
            fn field(&self, v: &Self::Result, f: &()) -> Result<ScalarValue, Self::Error> {
                self.captured.field(v, f)
            }
            fn member(
                &self,
                v: &Self::Result,
                n: &str,
                op: &usize,
            ) -> Result<Self::Result, Self::Error> {
                self.captured.member(v, n, op)
            }
        }
        impl<'a, T: Projection> EffectEvaluationEnvironment<'a, (), usize, (), usize> for Adapter<'a, T> {
            type Request = Queue<'a, T>;
            type Capture = Capture<'a, T>;
            fn preflight_effect(&self, node: &'a Node) -> Result<(), &'static str> {
                self.active.authorize(&ROOT)?;
                match node {
                    Node::Group {
                        group_kind: GroupKind::Sequence,
                        branches,
                    } if branches.len() == 3 => Ok(()),
                    Node::Call {
                        operation,
                        arguments,
                    } if *operation < 3 && arguments.len() == 1 => Ok(()),
                    _ => self.captured.preflight_effect(node),
                }
            }
            fn prepare_effect(
                &self,
                node: &'a Node,
                scope: &mut ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                fuel: &mut Fuel,
            ) -> Result<Self::Request, CalculationFailure<&'static str>> {
                self.preflight_effect(node)?;
                let prepared = prepare_native_group_members(
                    GroupKind::Sequence,
                    branches(node),
                    3,
                    fuel,
                    |row| Ok::<_, &'static str>(&row.name),
                    |_| Ok(()),
                    |index, row, fuel| {
                        let Node::Call { arguments, .. } = &row.value else {
                            return Err("call");
                        };
                        self.active
                            .preparations
                            .set(self.active.preparations.get() + 1);
                        evaluate_call_arguments_in_scope(
                            arguments,
                            &self.schemas[index],
                            scope,
                            self,
                            fuel,
                            CallEvaluationLimits {
                                pure: STACK.effects.pure,
                                max_arguments: 1,
                            },
                        )
                        .map_err(|_| "operands")
                    },
                )
                .map_err(|_| "preparation")?;
                Queue::start(
                    node,
                    prepared,
                    fuel,
                    QUEUE_LIMITS,
                    |_, _, _| Ok::<_, &'static str>(()),
                    |index, _, call, _| {
                        self.inventory.borrow_mut().push(Some(call));
                        Ok(PreparedSequenceMember {
                            identity: self.active.key(index),
                            declaration: &self.active.domains[index],
                            request: index,
                        })
                    },
                )
                .map_err(|_| "correlation".into())
            }
            fn capture(
                &self,
                name: &'a str,
                body: &'a Node,
                scope: &ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                fuel: &mut Fuel,
            ) -> Result<Self::Capture, &'static str> {
                self.captured.capture(name, body, scope, fuel)
            }
        }
        struct Root<'a> {
            host: &'a Host,
            node: &'a Node,
        }
        impl<'a, T: Projection> HostResultDomain<Group<'a, T>> for Root<'a> {
            type Error = &'static str;
            fn matches_type(&self, group: &Group<'a, T>) -> bool {
                std::ptr::eq(group.group(), self.node)
            }
            fn validate_value(&self, group: &Group<'a, T>) -> Result<(), Self::Error> {
                admit(group, self.host)
            }
        }
        impl<'a, T: Projection>
            EffectReentryEnvironment<'a, (), usize, (), usize, u64, Root<'a>, Rc<Group<'a, T>>>
            for Adapter<'a, T>
        {
            fn restore_capture(
                &self,
                id: &u64,
                _: &Root<'a>,
                capture: Self::Capture,
                fuel: &mut Fuel,
                limits: EffectEvaluationLimits,
            ) -> Result<RestoredEffectBindings<'a, Self::Result>, CalculationFailure<&'static str>>
            {
                self.active.authorize(id)?;
                self.captured.restore_capture(
                    &PENDING,
                    &self.captured.host.domains[0],
                    capture,
                    fuel,
                    limits,
                )
            }
            fn bind_reply(
                &self,
                id: &u64,
                domain: &Root<'a>,
                reply: Rc<Group<'a, T>>,
                _: &mut Fuel,
            ) -> Result<PureValue<Self::Result>, CalculationFailure<&'static str>> {
                self.active.authorize(id)?;
                domain.validate_value(&reply)?;
                self.captured
                    .projected
                    .set(self.captured.projected.get() + 1);
                Ok(PureValue::Result(View::Group(reply)))
            }
        }
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Operand {
            operation: usize,
            name: String,
            parameter: usize,
            argument: usize,
            value: ScalarValue,
        }
        #[derive(Serialize, Deserialize)]
        #[serde(tag = "state", content = "value", deny_unknown_fields)]
        enum SavedRow<T> {
            Accepted(T),
            Awaiting(T),
            Prepared(Operand),
        }
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Saved<T> {
            captured: Image<T>,
            frames: Vec<Frame<T>>,
            tree: Node,
            rows: Vec<SavedRow<T>>,
            remaining: u64,
        }
        fn tree<T: Projection>() -> Node {
            let mut tree = program::<T>();
            let Node::Bind { value, .. } = &mut tree else {
                panic!()
            };
            let Node::Bind { value, body, .. } = &mut **value else {
                panic!()
            };
            let mut node = group(GroupKind::Sequence, 3);
            let Node::Group { branches, .. } = &mut node else {
                panic!()
            };
            for (index, branch) in branches.iter_mut().enumerate() {
                let Node::Call { arguments, .. } = &mut branch.value else {
                    panic!()
                };
                arguments.push(ComputedArgument {
                    name: "value".into(),
                    value: field(Node::Member {
                        group: "alias".into(),
                        name: format!("m{index}"),
                        operation: index,
                    }),
                });
            }
            **value = node;
            **body = choose(
                local("keep"),
                choose(
                    Node::Binary {
                        operator: BinaryOperator::Eq,
                        left: Box::new(field(local("seed"))),
                        right: Box::new(literal(T::sample(0).scalar())),
                    },
                    field(Node::Member {
                        group: "reply".into(),
                        name: "m2".into(),
                        operation: 2,
                    }),
                    literal(ScalarValue::None),
                ),
                literal(ScalarValue::None),
            );
            tree
        }
        fn active_node(tree: &Node) -> &Node {
            let Node::Bind { value, .. } = tree else {
                panic!()
            };
            let Node::Bind { value, .. } = &**value else {
                panic!()
            };
            value
        }
        fn schemas<'a, T: Projection>(
            host: &'a Host,
            parameters: &'a [NamedParameter<&'a str, ScalarTypeSet>],
        ) -> Vec<Schema<'a, T>> {
            (0..3)
                .map(|index| OperationSchema {
                    key: index,
                    parameters,
                    result: &host.domains[index] as &NativeDomain<T>,
                    required_capability: (),
                })
                .collect()
        }
        fn adapter<'a, T: Projection>(
            host: &'a Host,
            active: &'a Host,
            schemas: &'a [Schema<'a, T>],
            saved: Option<&'a Image<T>>,
        ) -> Adapter<'a, T> {
            let mut captured = Runtime::new(host, saved, Rc::new(Cell::new(0)));
            captured.active_host = Some(active);
            Adapter {
                captured,
                active,
                schemas,
                inventory: RefCell::new(Vec::new()),
                restored_calls: Cell::new(0),
            }
        }
        impl<'a, T: Projection> Adapter<'a, T> {
            fn invoke(&self, index: usize) -> Input<T> {
                let call = self.inventory.borrow_mut()[index].take().unwrap();
                assert_eq!(call.schema().key, index);
                assert_eq!(call.arguments()[0].value, T::sample(index).scalar());
                self.active.dispatches.set(self.active.dispatches.get() + 1);
                input(index, &self.captured.drops)
            }
            fn poll(&self, queue: &mut Queue<'a, T>) -> Result<SequencePoll<usize>, &'static str> {
                queue
                    .poll(|index, _, id, domain, request| {
                        self.active.authorize(id)?;
                        let inventory = self.inventory.borrow();
                        let call = inventory[*request].as_ref().ok_or("missing request")?;
                        if index != *request
                            || call.schema().key != index
                            || !std::ptr::addr_eq(domain, call.schema().result)
                        {
                            return Err("queued correspondence");
                        }
                        Ok(())
                    })
                    .map_err(|_| "publication")
            }
        }
        fn finish<'a, T: Projection>(
            env: &Adapter<'a, T>,
            node: &'a Node,
            mut queue: Queue<'a, T>,
            mut rows: Vec<Row<'a, T>>,
            mut inbox: Option<Input<T>>,
            continuation: EffectReturnStack<'a, Node, Capture<'a, T>>,
            fuel: &mut Fuel,
        ) -> Result<Report, &'static str> {
            for index in rows.len()..3 {
                let reply = match env.poll(&mut queue)? {
                    SequencePoll::Awaiting { index: actual } => {
                        assert_eq!(actual, index);
                        inbox.take().ok_or("inbox")?
                    }
                    SequencePoll::Request {
                        index: actual,
                        request,
                    } => {
                        assert_eq!((actual, request), (index, index));
                        env.invoke(index)
                    }
                    _ => return Err("frontier"),
                };
                rows.push(
                    queue
                        .try_accept::<_, T, _>(&env.active.key(index), reply, env.active)
                        .map_err(|_| "receipt")?,
                );
            }
            assert!(matches!(
                env.poll(&mut queue)?,
                SequencePoll::Terminal(SequenceEnd::Completed)
            ));
            assert!(env.inventory.borrow().iter().all(Option::is_none));
            let group = match assemble_group_reply(node, rows, QUEUE_LIMITS, fuel, |g, _| {
                admit(g, env.active)
            }) {
                Ok(group) => Rc::new(group),
                Err(GroupReplyError::Fuel) => {
                    return Ok(Report {
                        value: Err(Failure::Fuel),
                        remaining: fuel.remaining(),
                        restored: 0,
                        projected: 0,
                    });
                }
                Err(GroupReplyError::Native(error)) => return Err(error),
                Err(error) => panic!("unexpected group assembly: {error:?}"),
            };
            let domain = Root {
                host: env.active,
                node,
            };
            let mut pending = PendingReply::new(ROOT, continuation, &domain);
            let pointer = Rc::as_ptr(&group);
            let rejected = pending
                .try_accept::<_, Group<'_, T>, _>(&999, group, env.active)
                .unwrap_err();
            assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
            assert_eq!(env.captured.restored.get(), 0);
            let accepted = pending
                .try_accept::<_, Group<'_, T>, _>(&ROOT, rejected.reply, env.active)
                .unwrap();
            Ok(report::<T, Adapter<'_, T>>(
                resume_accepted_stacked_effects(accepted, env, fuel, STACK),
                &env.captured,
                fuel,
            ))
        }
        fn initial<T: Projection>(
            kind: GroupKind,
            cut: usize,
            allowance: Option<u64>,
            save: bool,
        ) -> Initial {
            let node = group(kind, 3);
            let host = Host::new(3);
            let active = Host::new(3);
            active.generation.set(2);
            let parameters = [NamedParameter::required(
                "value",
                ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
            )];
            let schemas = schemas::<T>(&active, &parameters);
            let env = adapter(&host, &active, &schemas, None);
            let (rows, mut fuel) = complete::<T>(&node, &host, &env.captured.drops, 1000);
            let assembled = Rc::new(
                assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host))
                    .unwrap(),
            );
            let assembly_remaining = fuel.remaining();
            let tree = tree::<T>();
            let mut bindings = vec![
                ("keep", PureValue::Scalar(ScalarValue::Boolean(true))),
                (
                    "seed",
                    PureValue::Result(View::Atom(SEED, Rc::new(T::sample(0)))),
                ),
                ("batch", PureValue::Result(View::Group(assembled.clone()))),
                ("alias", PureValue::Result(View::Group(assembled.clone()))),
            ];
            let StackedEffectOutcome::Suspended {
                request: mut queue,
                continuation,
            } = evaluate_stacked_effects_in_scope(
                &tree,
                &mut ScopeFrame::new(&mut bindings),
                &env,
                &mut fuel,
                STACK,
            )
            .unwrap()
            else {
                panic!()
            };
            assert_eq!(continuation.frame_count(), 2);
            let mut rows = Vec::new();
            let mut inbox = None;
            for index in 0..cut.div_ceil(2) {
                assert!(
                    matches!(env.poll(&mut queue).unwrap(), SequencePoll::Request { index: actual, .. } if actual == index)
                );
                let reply = env.invoke(index);
                if index < cut / 2 {
                    rows.push(
                        queue
                            .try_accept::<_, T, _>(&active.key(index), reply, &active)
                            .unwrap(),
                    );
                } else {
                    inbox = Some(reply);
                }
            }
            if let Some(allowance) = allowance {
                fuel.charge(fuel.remaining() - allowance).unwrap();
            }
            assert_eq!((host.preparations.get(), active.preparations.get()), (3, 3));
            if !save {
                let result = finish(
                    &env,
                    active_node(&tree),
                    queue,
                    rows,
                    inbox,
                    continuation,
                    &mut fuel,
                )
                .unwrap();
                assert_eq!((host.dispatches.get(), active.dispatches.get()), (3, 3));
                return Initial::Finished(result);
            }
            let frames = snapshots(continuation, &assembled);
            let mut saved_rows: Vec<_> = rows
                .into_iter()
                .map(|row| SavedRow::Accepted((*row.into_parts().4.data).clone()))
                .collect();
            if let Some(reply) = inbox {
                saved_rows.push(SavedRow::Awaiting((*reply.data).clone()));
            }
            for call in env.inventory.borrow_mut().iter_mut().skip(saved_rows.len()) {
                let call = call.take().unwrap();
                let (schema, mut args) = call.into_parts();
                let arg = args.pop().unwrap();
                assert!(args.is_empty());
                saved_rows.push(SavedRow::Prepared(Operand {
                    operation: schema.key,
                    name: arg.name.into(),
                    parameter: arg.parameter_index,
                    argument: arg.argument_index,
                    value: arg.value,
                }));
            }
            drop(queue);
            let image = Saved {
                captured: Image {
                    node: node.clone(),
                    rows: assembled
                        .members()
                        .iter()
                        .map(|r| (*r.as_parts().2, (*r.as_parts().4.data).clone()))
                        .collect(),
                    remaining: assembly_remaining,
                    assembled: true,
                },
                frames,
                tree: tree.clone(),
                rows: saved_rows,
                remaining: fuel.remaining(),
            };
            let bytes = serde_json::to_vec(&image).unwrap();
            drop((bindings, assembled));
            assert_eq!(env.captured.drops.get(), 3 + cut.div_ceil(2));
            assert!(env.inventory.borrow().iter().all(Option::is_none));
            // No borrowed graph, prepared call, native view, schema, or meter leaves this scope.
            Initial::Saved(bytes)
        }
        fn saved<T: Projection>(kind: GroupKind, cut: usize, allowance: Option<u64>) -> Vec<u8> {
            let Initial::Saved(bytes) = initial::<T>(kind, cut, allowance, true) else {
                panic!()
            };
            bytes
        }
        fn decode<T: Projection>(
            candidate: &[u8],
            committed: &[u8],
        ) -> Result<Saved<T>, &'static str> {
            if candidate.len() > 64 * 1024 || candidate != committed {
                return Err("store admission");
            }
            let saved: Saved<T> = serde_json::from_slice(candidate).map_err(|_| "decode")?;
            if saved.rows.len() != 3
                || saved.frames.len() != 2
                || saved.captured.rows.len() != 3
                || !saved.captured.assembled
                || saved.captured.remaining != 992
                || saved.remaining > saved.captured.remaining
            {
                return Err("bounds");
            }
            for frame in &saved.frames {
                validate_captured_bindings(
                    frame,
                    CaptureValidationLimits {
                        max_saved_bindings: 4,
                        max_name_bytes: 64,
                    },
                    |v| {
                        if v.id == SEED && v.value == T::sample(0) {
                            Ok(())
                        } else {
                            Err("capture")
                        }
                    },
                    |g| if *g == 0 { Ok(()) } else { Err("group") },
                    |_| Ok(()),
                )
                .map_err(|_| "capture bounds")?;
            }
            Ok(saved)
        }
        fn restored_queue<'a, T: Projection>(
            saved: &'a Saved<T>,
            env: &Adapter<'a, T>,
        ) -> Result<QueueState<'a, T>, &'static str> {
            let node = active_node(&saved.tree);
            let mut rows = Vec::new();
            let mut inbox = None;
            for (index, state) in saved.rows.iter().enumerate() {
                let identity = env.active.key(index);
                let declaration: &NativeDomain<T> = &env.active.domains[index];
                let reply = match state {
                    SavedRow::Prepared(operand) => {
                        let call = PreparedCall::restore(
                            &env.schemas[index],
                            vec![PreparedArgument {
                                parameter_index: operand.parameter,
                                argument_index: operand.argument,
                                name: &operand.name,
                                value: operand.value.clone(),
                            }],
                            1,
                            |schema, _| {
                                env.active.authorize(&identity)?;
                                if schema.key != operand.operation {
                                    return Err("schema");
                                }
                                env.restored_calls.set(env.restored_calls.get() + 1);
                                Ok(())
                            },
                        )
                        .map_err(|_| "saved operands")?;
                        env.inventory.borrow_mut().push(Some(call));
                        SavedSequenceReply::Prepared(PreparedSequenceMember {
                            identity,
                            declaration,
                            request: index,
                        })
                    }
                    SavedRow::Awaiting(value) => {
                        env.inventory.borrow_mut().push(None);
                        inbox = Some(Input {
                            data: Box::new(value.clone()),
                            drops: env.captured.drops.clone(),
                        });
                        SavedSequenceReply::Awaiting {
                            identity,
                            declaration,
                        }
                    }
                    SavedRow::Accepted(value) => {
                        env.inventory.borrow_mut().push(None);
                        SavedSequenceReply::Accepted(
                            PendingReply::new(identity, (), declaration)
                                .try_accept::<_, T, _>(
                                    &identity,
                                    Input {
                                        data: Box::new(value.clone()),
                                        drops: env.captured.drops.clone(),
                                    },
                                    env.active,
                                )
                                .map_err(|_| "accepted row")?,
                        )
                    }
                };
                rows.push(SavedSequenceMember {
                    index,
                    name: &branches(node)[index].name,
                    reply,
                });
            }
            let (queue, accepted) = Queue::restore(node, rows, QUEUE_LIMITS, |rows| {
                for row in rows {
                    env.active.authorize(row.reply.identity())?;
                }
                Ok::<_, &'static str>(())
            })
            .map_err(|_| "saved frontier")?;
            Ok((queue, accepted, inbox))
        }
        fn restored<T: Projection>(
            saved: &Saved<T>,
            mode: usize,
        ) -> Result<(Report, usize, usize, usize), &'static str> {
            let host = Host::new(3);
            let active = Host::new(3);
            active.generation.set(2);
            let parameters = [NamedParameter::required(
                "value",
                ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
            )];
            let schemas = schemas::<T>(&active, &parameters);
            let env = adapter(&host, &active, &schemas, Some(&saved.captured));
            if mode == 1 {
                active.granted.set(false);
            }
            env.captured.deny_outer.set(mode == 3);
            env.captured.revoke_outer.set(mode == 4);
            let result = (|| {
                let continuation = restore_effect_return_stack(
                    saved
                        .frames
                        .iter()
                        .map(|frame| SavedEffectReturnFrame {
                            name: &frame.name,
                            body: &frame.body,
                            capture: Capture::Saved(frame),
                        })
                        .collect(),
                    STACK,
                )
                .map_err(|_| "return stack")?;
                let (queue, rows, inbox) = restored_queue(saved, &env)?;
                if mode == 2 {
                    active.granted.set(false);
                }
                let mut fuel = Fuel::new(saved.remaining);
                finish(
                    &env,
                    active_node(&saved.tree),
                    queue,
                    rows,
                    inbox,
                    continuation,
                    &mut fuel,
                )
            })();
            assert_eq!(
                (
                    host.preparations.get(),
                    active.preparations.get(),
                    host.dispatches.get()
                ),
                (0, 0, 0)
            );
            if mode == 1 || mode == 2 {
                assert_eq!(active.dispatches.get(), 0);
            }
            if mode == 1 {
                assert_eq!(env.restored_calls.get(), 0);
            }
            result.map(|r| {
                (
                    r,
                    env.restored_calls.get(),
                    active.dispatches.get(),
                    env.captured.rebuilt.get(),
                )
            })
        }
        #[test]
        fn captured_seq_all_seven_frontiers_rebuild_operands_not_old_requests() {
            fn run<T: Projection>() {
                for kind in KINDS {
                    for cut in 0..7 {
                        let Initial::Finished(expected) = initial::<T>(kind, cut, None, false)
                        else {
                            panic!()
                        };
                        let bytes = saved::<T>(kind, cut, None);
                        let decoded = decode::<T>(&bytes, &bytes).unwrap();
                        let (actual, restored, dispatched, rebuilt) =
                            self::restored(&decoded, 0).unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                        assert_eq!(
                            (restored, dispatched, rebuilt),
                            (3 - cut.div_ceil(2), 3 - cut.div_ceil(2), 1)
                        );
                        assert_eq!((actual.restored, actual.projected), (2, 1));
                    }
                }
            }
            run::<u64>();
            run::<String>();
        }
        #[test]
        fn captured_seq_assembly_and_return_fuel_match_every_cutoff_at_every_frontier() {
            fn run<T: Projection>() {
                for kind in KINDS {
                    for cut in 0..7 {
                        let bytes = saved::<T>(kind, cut, None);
                        let decoded = decode::<T>(&bytes, &bytes).unwrap();
                        let needed = decoded.remaining - restored(&decoded, 0).unwrap().0.remaining;
                        for allowance in 0..=needed + 1 {
                            let Initial::Finished(expected) =
                                initial::<T>(kind, cut, Some(allowance), false)
                            else {
                                panic!()
                            };
                            let bytes = saved::<T>(kind, cut, Some(allowance));
                            let decoded = decode::<T>(&bytes, &bytes).unwrap();
                            let actual = restored(&decoded, 0).unwrap().0;
                            assert_eq!(actual, expected, "cut={cut} fuel={allowance}");
                            if allowance < needed {
                                assert_eq!(actual.value, Err(Failure::Fuel));
                            } else {
                                assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                            }
                        }
                    }
                }
            }
            run::<u64>();
            run::<String>();
        }
        #[test]
        fn captured_seq_revocation_blocks_restore_or_publication_and_cold_returns() {
            for kind in KINDS {
                for cut in 0..7 {
                    let bytes = saved::<String>(kind, cut, None);
                    let decoded = decode::<String>(&bytes, &bytes).unwrap();
                    assert!(restored(&decoded, 1).is_err());
                    assert!(restored(&decoded, 2).is_err());
                    let (cold, _, _, rebuilds) = restored(&decoded, 3).unwrap();
                    assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                    assert_eq!(
                        (cold.remaining, cold.restored, cold.projected, rebuilds),
                        (decoded.remaining - 4, 0, 0, 0)
                    );
                    let (late, _, _, rebuilds) = restored(&decoded, 4).unwrap();
                    assert!(late.value.is_err());
                    assert_eq!((late.restored, late.projected, rebuilds), (2, 1, 1));
                }
            }
        }
        #[test]
        fn captured_seq_forged_operands_frontier_captures_graph_and_budget_are_not_committed() {
            let bytes = saved::<String>(GroupKind::Parallel, 2, None);
            for case in 0..7 {
                let mut image = decode::<String>(&bytes, &bytes).unwrap();
                match case {
                    0 => image.remaining += 1,
                    1 => image.rows.swap(0, 2),
                    2 => {
                        if let SavedRow::Prepared(op) = &mut image.rows[1] {
                            op.value = ScalarValue::Integer(9);
                        }
                    }
                    3 => image.frames[0].groups[0].group = 1,
                    4 => image.frames.swap(0, 1),
                    5 => image.tree = literal(ScalarValue::None),
                    _ => image.captured.rows[0].0 = 200,
                }
                let candidate = serde_json::to_vec(&image).unwrap();
                assert!(decode::<String>(&candidate, &bytes).is_err());
            }
            assert!(decode::<String>(&vec![b' '; 64 * 1024 + 1], &bytes).is_err());
        }
    }
}
