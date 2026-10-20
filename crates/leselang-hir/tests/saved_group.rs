//! Host-owned byte-codec models, not process-crash or external-effect transactions.
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
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
use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionLease, PendingReply, ReplyAuthority,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    index: usize,
    name: String,
}
impl Borrow<str> for Operation {
    fn borrow(&self) -> &str {
        &self.name
    }
}
fn op(index: usize) -> Operation {
    Operation {
        index,
        name: format!("operation{index}"),
    }
}
type Node = Computation<(), Operation, (), usize>;
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
    lease: RefCell<Option<LocalExecutionLease>>,
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
            lease: RefCell::new(None),
        }
    }
    fn key(&self, index: usize) -> u64 {
        self.generation.get() * 100 + index as u64
    }
}
impl ReplyAuthority<u64> for Host {
    type Error = &'static str;
    fn authorize(&self, id: &u64) -> Result<(), Self::Error> {
        (self.granted.get()
            && self
                .lease
                .borrow()
                .as_ref()
                .is_none_or(LocalExecutionLease::is_open)
            && *id / 100 == self.generation.get())
        .then_some(())
        .ok_or("authority")
    }
}
struct Environment<'a, T>(&'a Host, std::marker::PhantomData<T>);
impl<'a, T: Data + 'static> SequenceEvaluationEnvironment<'a, (), Operation, (), usize>
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
                    operation: op(index),
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
            || branch.result_type != branch_operation(branch).index
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
fn branch_operation(branch: &ComputedBranch<Node, usize>) -> &Operation {
    let Node::Call { operation, .. } = &branch.value else {
        panic!("fixture member call")
    };
    operation
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
                        operation: op(0),
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
    rebind_with_limits(node, host, values, drops, LIMITS)
}
fn rebind_with_limits<'a, T: Data + 'static>(
    node: &'a Node,
    host: &'a Host,
    values: Vec<(u64, T)>,
    drops: &Rc<Cell<usize>>,
    limits: SequenceEvaluationLimits,
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
                PreparedSequenceSession::restore(node, saved, limits, |rows| {
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
            let mut session = PreparedParallelSession::restore(node, saved, limits, |rows| {
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
        saved: Vec<(&'a Host, &'a Image<T>)>,
        captured_hosts: Vec<&'a Host>,
        expected_groups: usize,
        saved_group_limits: SequenceEvaluationLimits,
        groups: RefCell<Vec<Option<Rc<Group<'a, T>>>>>,
        drops: Rc<Cell<usize>>,
        restored: Cell<usize>,
        rebuilt: Cell<usize>,
        projected: Cell<usize>,
        deny_outer: Cell<bool>,
        deny_capture: Cell<bool>,
        revoke_outer: Cell<bool>,
        revoke_group: usize,
    }
    impl<'a, T: Projection> Runtime<'a, T> {
        fn new(host: &'a Host, saved: Option<&'a Image<T>>, drops: Rc<Cell<usize>>) -> Self {
            Self {
                host,
                active_host: None,
                saved: saved.into_iter().map(|image| (host, image)).collect(),
                captured_hosts: vec![host],
                expected_groups: 1,
                saved_group_limits: LIMITS,
                groups: RefCell::new(vec![None]),
                drops,
                restored: Cell::new(0),
                rebuilt: Cell::new(0),
                projected: Cell::new(0),
                deny_outer: Cell::new(false),
                deny_capture: Cell::new(false),
                revoke_outer: Cell::new(false),
                revoke_group: 0,
            }
        }
        fn view_host(&self, identity: u64) -> &Host {
            self.active_host
                .filter(|host| identity / 100 == host.generation.get())
                .or_else(|| {
                    self.captured_hosts
                        .iter()
                        .copied()
                        .find(|host| identity / 100 == host.generation.get())
                })
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
        fn saved_group(&self, index: usize) -> Result<Rc<Group<'a, T>>, &'static str> {
            if self.deny_capture.get() {
                return Err("captured authority");
            }
            let (host, saved) = *self.saved.get(index).ok_or("missing snapshot")?;
            if let Some(group) = self.groups.borrow().get(index).and_then(Option::as_ref) {
                self.admit_view(&View::Group(group.clone()))?;
                return Ok(group.clone());
            }
            for (index, (id, value)) in saved.rows.iter().enumerate() {
                if *id != host.key(index) {
                    return Err("original group receipt");
                }
                host.authorize(id)?;
                host.domains[index].validate_value(value)?;
            }
            // Byte ingress and each three-row rebuild are separate bounded host work.
            // This does not pay language preparation or completed assembly a second time.
            let rows = rebind_with_limits(
                &saved.node,
                host,
                saved.rows.clone(),
                &self.drops,
                self.saved_group_limits,
            );
            let group = restore_group_reply(&saved.node, rows, self.saved_group_limits, |g| {
                if !saved.assembled {
                    return Err("assembly incomplete");
                }
                admit(g, host)
            });
            let group = Rc::new(group.map_err(|_| "group restore")?);
            self.rebuilt.set(self.rebuilt.get() + 1);
            self.groups.borrow_mut()[index] = Some(group.clone());
            Ok(group)
        }
    }
    impl<'a, T: Projection> PureEvaluationEnvironment<(), Operation> for Runtime<'a, T> {
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
            operation: &Operation,
        ) -> Result<Self::Result, Self::Error> {
            self.admit_view(value)?;
            let View::Group(group) = value else {
                return Err("group required");
            };
            let row = group
                .lookup(3, name, |row| {
                    let (_, branch, id, domain, value) = row.as_parts();
                    if branch_operation(branch) != operation {
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
    impl<'a, T: Projection> EffectEvaluationEnvironment<'a, (), Operation, (), usize>
        for Runtime<'a, T>
    {
        type Request = usize;
        type Capture = Capture<'a, T>;
        fn preflight_effect(&self, node: &'a Node) -> Result<(), &'static str> {
            self.host.authorize(&PENDING)?;
            match node {
                Node::Call {
                    operation: Operation { index: 0, .. },
                    arguments,
                } if arguments.is_empty() => Ok(()),
                Node::Call {
                    operation: Operation { index: 9, .. },
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
            Ok(operation.index)
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
        EffectReentryEnvironment<'a, (), Operation, (), usize, u64, Domain, Input<T>>
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
                self.captured_hosts[self.revoke_group].domains[2]
                    .denied
                    .set(true);
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
                        rows.push((
                            saved.name.as_str(),
                            PureValue::Result(View::Group(self.saved_group(saved.group)?)),
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
            assert_eq!(groups.len(), self.expected_groups * 2);
            let mut distinct = Vec::new();
            for group in &groups {
                assert_eq!(
                    groups
                        .iter()
                        .filter(|other| Rc::ptr_eq(group, other))
                        .count(),
                    2
                );
                if !distinct.iter().any(|other| Rc::ptr_eq(group, other)) {
                    distinct.push((*group).clone());
                }
            }
            assert_eq!(distinct.len(), self.expected_groups);
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
            operation: op(2),
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
                    operation: op(0),
                    arguments: vec![],
                }),
                body: Box::new(body),
            }),
            body: Box::new(choose(
                literal(ScalarValue::Boolean(false)),
                Node::Call {
                    operation: op(9),
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
        result: StackedControlResult<'a, (), Operation, (), usize, E>,
        runtime: &Runtime<'a, T>,
        fuel: &Fuel,
    ) -> Report
    where
        E: EffectEvaluationEnvironment<
                'a,
                (),
                Operation,
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
        snapshots_for_groups(continuation, &[assembled])
    }
    fn snapshots_for_groups<T: Projection>(
        continuation: EffectReturnStack<'_, Node, Capture<'_, T>>,
        groups: &[&Rc<Group<'_, T>>],
    ) -> Vec<Frame<T>> {
        assert!((1..=2).contains(&groups.len()));
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
                    max_bindings: 2 + 2 * groups.len(),
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
                            PureValue::Result(View::Group(g)) => CapturedValue::Group(
                                groups
                                    .iter()
                                    .position(|original| Rc::ptr_eq(g, original))
                                    .ok_or("unregistered capture")?,
                            ),
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
            parallel_evaluation::{ParallelEnd, ParallelRequest, PreparedParallelMember},
        };
        use leselang_runtime_core::{NamedParameter, OperationSchema, ScalarTypeSet};

        type Schema<'a, T> =
            OperationSchema<'a, Operation, &'a str, ScalarTypeSet, &'a NativeDomain<T>, ()>;
        type Call<'a, T> = PreparedCall<'a, Operation, ScalarTypeSet, &'a NativeDomain<T>, ()>;
        type Queue<'a, T> = PreparedSequenceSession<'a, Node, usize, u64, NativeDomain<T>, usize>;
        type QueueState<'a, T> = (Queue<'a, T>, Vec<Row<'a, T>>, Option<Input<T>>);
        type Barrier<'a, T> =
            PreparedParallelSession<'a, Node, usize, u64, NativeDomain<T>, Input<T>>;
        enum Work<'a, T> {
            Sequence(Queue<'a, T>),
            Parallel(Barrier<'a, T>, Vec<ParallelRequest<usize>>),
        }
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
        impl<'a, T: Projection> PureEvaluationEnvironment<(), Operation> for Adapter<'a, T> {
            type Result = View<'a, T>;
            type Error = &'static str;
            fn field(&self, v: &Self::Result, f: &()) -> Result<ScalarValue, Self::Error> {
                self.captured.field(v, f)
            }
            fn member(
                &self,
                v: &Self::Result,
                n: &str,
                op: &Operation,
            ) -> Result<Self::Result, Self::Error> {
                self.captured.member(v, n, op)
            }
        }
        impl<'a, T: Projection> EffectEvaluationEnvironment<'a, (), Operation, (), usize>
            for Adapter<'a, T>
        {
            type Request = Work<'a, T>;
            type Capture = Capture<'a, T>;
            fn preflight_effect(&self, node: &'a Node) -> Result<(), &'static str> {
                self.active.authorize(&self.active.key(90))?;
                match node {
                    Node::Group { branches, .. } if branches.len() == 3 => Ok(()),
                    Node::Call {
                        operation,
                        arguments,
                    } if operation.index < 3 && arguments.len() == 1 => Ok(()),
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
                let Node::Group { group_kind, .. } = node else {
                    return Err("group required".into());
                };
                let prepared = prepare_native_group_members(
                    *group_kind,
                    branches(node),
                    3,
                    fuel,
                    |row| Ok::<_, &'static str>(&row.name),
                    |_| Ok(()),
                    |index, row, fuel| {
                        let Node::Call {
                            operation,
                            arguments,
                        } = &row.value
                        else {
                            return Err("call");
                        };
                        if *operation != self.schemas[index].key {
                            return Err("operation schema");
                        }
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
                if *group_kind == GroupKind::Parallel {
                    return Barrier::start(
                        node,
                        prepared,
                        fuel,
                        QUEUE_LIMITS,
                        |_, _, _| Ok::<_, &'static str>(()),
                        |index, _, call, _| {
                            self.inventory.borrow_mut().push(Some(call));
                            Ok(PreparedParallelMember {
                                identity: self.active.key(index),
                                declaration: &self.active.domains[index],
                                request: index,
                            })
                        },
                    )
                    .map(|(barrier, requests)| Work::Parallel(barrier, requests))
                    .map_err(|_| "parallel correlation".into());
                }
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
                .map(Work::Sequence)
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
            EffectReentryEnvironment<'a, (), Operation, (), usize, u64, Root<'a>, Rc<Group<'a, T>>>
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
            operation: Operation,
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
            additional: Vec<Image<T>>,
            frames: Vec<Frame<T>>,
            tree: Node,
            rows: Vec<SavedRow<T>>,
            remaining: u64,
        }
        fn tree<T: Projection>(kind: GroupKind) -> Node {
            let mut tree = program::<T>();
            let Node::Bind { value, .. } = &mut tree else {
                panic!()
            };
            let Node::Bind { value, body, .. } = &mut **value else {
                panic!()
            };
            let mut node = group(kind, 3);
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
                        operation: op(index),
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
                        operation: op(2),
                    }),
                    literal(ScalarValue::None),
                ),
                literal(ScalarValue::None),
            );
            tree
        }
        fn active_node(tree: &Node) -> &Node {
            if matches!(tree, Node::Group { .. }) {
                return tree;
            }
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
            node: &Node,
        ) -> Vec<Schema<'a, T>> {
            branches(node)
                .iter()
                .enumerate()
                .map(|(index, branch)| OperationSchema {
                    key: branch_operation(branch).clone(),
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
            captured.saved_group_limits = QUEUE_LIMITS;
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
                self.active
                    .authorize(&self.active.key(index))
                    .expect("live dispatch");
                let call = self.inventory.borrow_mut()[index].take().unwrap();
                assert_eq!(call.schema().key, self.schemas[index].key);
                assert_eq!(
                    call.arguments()[0].value,
                    T::sample(call.schema().key.index).scalar()
                );
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
                            || call.schema().key != self.schemas[index].key
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
            queue: Queue<'a, T>,
            rows: Vec<Row<'a, T>>,
            inbox: Option<Input<T>>,
            continuation: EffectReturnStack<'a, Node, Capture<'a, T>>,
            fuel: &mut Fuel,
        ) -> Result<Report, &'static str> {
            let rows = collect_sequence(env, queue, rows, inbox)?;
            assemble_and_resume(env, node, rows, continuation, fuel)
        }
        fn collect_sequence<'a, T: Projection>(
            env: &Adapter<'a, T>,
            mut queue: Queue<'a, T>,
            mut rows: Vec<Row<'a, T>>,
            mut inbox: Option<Input<T>>,
        ) -> Result<Vec<Row<'a, T>>, &'static str> {
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
            Ok(rows)
        }
        struct Flight<'a, T> {
            assembled: Rc<Group<'a, T>>,
            assembly_remaining: u64,
            additional: Vec<(Rc<Group<'a, T>>, u64)>,
            tree: &'a Node,
            work: Work<'a, T>,
            continuation: EffectReturnStack<'a, Node, Capture<'a, T>>,
            fuel: Fuel,
        }
        fn group_image<T: Projection>(group: &Group<'_, T>, remaining: u64) -> Image<T> {
            Image {
                node: group.group().clone(),
                rows: group
                    .members()
                    .iter()
                    .map(|r| (*r.as_parts().2, (*r.as_parts().4.data).clone()))
                    .collect(),
                remaining,
                assembled: true,
            }
        }
        fn assemble_and_resume<'a, T: Projection>(
            env: &Adapter<'a, T>,
            node: &'a Node,
            rows: Vec<Row<'a, T>>,
            continuation: EffectReturnStack<'a, Node, Capture<'a, T>>,
            fuel: &mut Fuel,
        ) -> Result<Report, &'static str> {
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
            let root = env.active.key(90);
            let mut pending = PendingReply::new(root, continuation, &domain);
            let pointer = Rc::as_ptr(&group);
            let rejected = pending
                .try_accept::<_, Group<'_, T>, _>(&999, group, env.active)
                .unwrap_err();
            assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
            assert_eq!(env.captured.restored.get(), 0);
            let accepted = pending
                .try_accept::<_, Group<'_, T>, _>(&root, rejected.reply, env.active)
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
            initial_with_tree::<T>(kind, cut, allowance, save, tree::<T>(GroupKind::Sequence))
        }
        fn initial_with_tree<T: Projection>(
            kind: GroupKind,
            cut: usize,
            allowance: Option<u64>,
            save: bool,
            tree: Node,
        ) -> Initial {
            let node = group(kind, 3);
            let host = Host::new(3);
            let active = Host::new(3);
            active.generation.set(2);
            let parameters = [NamedParameter::required(
                "value",
                ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
            )];
            let schemas = schemas::<T>(&active, &parameters, active_node(&tree));
            let env = adapter(&host, &active, &schemas, None);
            let (rows, mut fuel) = complete::<T>(&node, &host, &env.captured.drops, 1000);
            let assembled = Rc::new(
                assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host))
                    .unwrap(),
            );
            let assembly_remaining = fuel.remaining();
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
                request: work,
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
            drop(bindings);
            checkpoint(
                &env,
                Flight {
                    assembled,
                    assembly_remaining,
                    additional: Vec::new(),
                    tree: &tree,
                    work,
                    continuation,
                    fuel,
                },
                cut,
                allowance,
                save,
            )
        }
        fn checkpoint<'a, T: Projection>(
            env: &Adapter<'a, T>,
            flight: Flight<'a, T>,
            cut: usize,
            allowance: Option<u64>,
            save: bool,
        ) -> Initial {
            let Flight {
                assembled,
                assembly_remaining,
                additional,
                tree,
                work,
                continuation,
                mut fuel,
            } = flight;
            let Work::Sequence(mut queue) = work else {
                panic!("wrong group mode")
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
                            .try_accept::<_, T, _>(&env.active.key(index), reply, env.active)
                            .unwrap(),
                    );
                } else {
                    inbox = Some(reply);
                }
            }
            if let Some(allowance) = allowance {
                fuel.charge(fuel.remaining() - allowance).unwrap();
            }
            assert_eq!(
                (
                    env.captured.host.preparations.get(),
                    env.active.preparations.get()
                ),
                (3, 3)
            );
            if !save {
                let result = finish(
                    env,
                    active_node(tree),
                    queue,
                    rows,
                    inbox,
                    continuation,
                    &mut fuel,
                )
                .unwrap();
                assert_eq!(
                    (
                        env.captured.host.dispatches.get(),
                        env.active.dispatches.get()
                    ),
                    (3, 3)
                );
                return Initial::Finished(result);
            }
            let captured_count = 1 + additional.len();
            let frames = snapshots_for_groups(
                continuation,
                &std::iter::once(&assembled)
                    .chain(additional.iter().map(|(group, _)| group))
                    .collect::<Vec<_>>(),
            );
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
                    operation: schema.key.clone(),
                    name: arg.name.into(),
                    parameter: arg.parameter_index,
                    argument: arg.argument_index,
                    value: arg.value,
                }));
            }
            drop(queue);
            let image = Saved {
                additional: additional
                    .iter()
                    .map(|(group, remaining)| group_image(group, *remaining))
                    .collect(),
                captured: Image {
                    node: assembled.group().clone(),
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
            drop((assembled, additional));
            assert_eq!(
                env.captured.drops.get(),
                3 * captured_count + cut.div_ceil(2)
            );
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
            decode_with_origin(candidate, committed, Some(992), SEED)
        }
        fn decode_with_origin<T: Projection>(
            candidate: &[u8],
            committed: &[u8],
            assembly_budget: Option<u64>,
            seed_identity: u64,
        ) -> Result<Saved<T>, &'static str> {
            if candidate.len() > 64 * 1024 || candidate != committed {
                return Err("store admission");
            }
            let saved: Saved<T> = serde_json::from_slice(candidate).map_err(|_| "decode")?;
            if saved.rows.len() != 3
                || !saved.additional.is_empty()
                || saved.frames.len() != 2
                || saved.captured.rows.len() != 3
                || !saved.captured.assembled
                || saved.captured.remaining > 1000
                || assembly_budget.is_some_and(|remaining| saved.captured.remaining != remaining)
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
                        if v.id == seed_identity && v.value == T::sample(0) {
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
            let schemas = schemas::<T>(&active, &parameters, active_node(&saved.tree));
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

        mod source {
            use super::*;
            use leselang_hir::{
                catalog_group_source::{
                    CatalogGroupProgramLimits, CatalogGroupSourceEnvironment,
                    CatalogGroupSourceLimits, compile_catalog_prepared_group_helper_program,
                    compile_catalog_prepared_group_source,
                },
                catalog_program::CatalogProgramLimits,
                catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits},
                control_source::ControlSourceLimits,
                flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
                pure_typing::{PureType, PureTypeEnvironment},
                source_call::{SourceCallHost, SourceCallLimits},
            };
            use leselang_runtime_core::{OperationCatalog, OperationCatalogLimits, ScalarType};

            #[derive(Clone, Debug, PartialEq)]
            enum Meta {
                Atom(usize),
                Group(GroupKind, Vec<(String, Operation)>),
            }
            struct Types(ScalarType);
            impl PureTypeEnvironment<(), Operation> for Types {
                type Result = Meta;
                fn field_type(&self, result: &Meta, _: &()) -> Option<ScalarType> {
                    matches!(result, Meta::Atom(0..=2)).then_some(self.0)
                }
                fn member_result(&self, result: &Meta, name: &str, op: &Operation) -> Option<Meta> {
                    let Meta::Group(_, rows) = result else {
                        return None;
                    };
                    rows.iter()
                        .any(|(n, key)| n == name && key == op)
                        .then_some(Meta::Atom(op.index))
                }
            }
            impl<'s> CallFlowEnvironment<'s, (), Operation, usize> for Types {
                fn call_result_type(
                    &self,
                    op: &Operation,
                    declaration: &'s usize,
                ) -> Option<PureType<Meta>> {
                    if op != &super::super::super::op(*declaration) {
                        return None;
                    }
                    match op.index {
                        0..=2 => Some(PureType::Result(Meta::Atom(op.index))),
                        9 => Some(PureType::Scalar(self.0)),
                        _ => None,
                    }
                }
            }
            impl<'src, 's> CatalogSourceEnvironment<'src, 's, (), Operation, usize> for Types {
                type Error = &'static str;
                fn source_operation(&self, original: &Operation) -> Result<Operation, Self::Error> {
                    Ok(original.clone())
                }
                fn source_field(
                    &self,
                    result: &Meta,
                    name: &'src str,
                ) -> Result<Option<((), ScalarType)>, Self::Error> {
                    Ok((name == "value")
                        .then(|| self.field_type(result, &()))
                        .flatten()
                        .map(|ty| ((), ty)))
                }
                fn source_member(
                    &self,
                    result: &Meta,
                    _: &'src str,
                    name: &'src str,
                ) -> Result<Option<(Operation, Meta)>, Self::Error> {
                    let Meta::Group(_, rows) = result else {
                        return Ok(None);
                    };
                    Ok(rows
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, op)| (op.clone(), Meta::Atom(op.index))))
                }
            }
            impl<'e, 's> GroupFlowEnvironment<'e, 's, (), Operation, usize, usize> for Types {
                fn group_branch_type_matches(
                    &self,
                    tag: &usize,
                    inferred: &PureType<Meta>,
                ) -> bool {
                    *inferred == PureType::Result(Meta::Atom(*tag))
                }
                fn group_result_type(
                    &self,
                    kind: GroupKind,
                    members: &[GroupMemberType<'e, Operation, Meta>],
                ) -> Option<Meta> {
                    let mut rows = Vec::new();
                    for member in members {
                        if !self
                            .group_branch_type_matches(&member.operation.index, &member.inferred)
                        {
                            return None;
                        }
                        rows.push((member.name.into(), member.operation.clone()));
                    }
                    Some(Meta::Group(kind, rows))
                }
            }
            impl<'src, 's> CatalogGroupSourceEnvironment<'src, 's, (), Operation, usize, usize> for Types {
                fn source_group_branch(
                    &self,
                    _: &'src leselang_syntax::NamedArgument,
                    op: &Operation,
                    _: &'s usize,
                    _: &PureType<Meta>,
                ) -> Result<usize, Self::Error> {
                    Ok(op.index)
                }
            }
            const COMPILE: CatalogGroupSourceLimits = CatalogGroupSourceLimits {
                expression: CatalogSourceLimits {
                    control: ControlSourceLimits {
                        source: SourceCallLimits {
                            max_source_nodes: 256,
                            max_source_depth: 32,
                            max_lowered_nodes: 256,
                            max_lowered_depth: 32,
                            max_arguments: 4,
                        },
                        max_bindings: 8,
                    },
                    max_calls: 4,
                },
                max_groups: 1,
                max_branches: 3,
            };
            fn compile<T: Projection>(
                source: &str,
                captured: GroupKind,
                version: u32,
                granted: &[u8],
            ) -> Result<Node, String> {
                compile_model::<T>(source, Some(captured), version, granted)
            }
            pub(super) fn compile_model<T: Projection>(
                source: &str,
                captured: Option<GroupKind>,
                version: u32,
                granted: &[u8],
            ) -> Result<Node, String> {
                compile_model_with_budget::<T>(source, captured, version, granted, 7, 4)
            }
            pub(super) fn compile_model_with_budget<T: Projection>(
                source: &str,
                captured: Option<GroupKind>,
                version: u32,
                granted: &[u8],
                max_calls: usize,
                max_groups: usize,
            ) -> Result<Node, String> {
                compile_model_with_result::<T>(
                    source, captured, version, granted, max_calls, max_groups, None,
                )
            }
            pub(super) fn compile_model_with_result<T: Projection>(
                source: &str,
                captured: Option<GroupKind>,
                version: u32,
                granted: &[u8],
                max_calls: usize,
                max_groups: usize,
                final_group: Option<GroupKind>,
            ) -> Result<Node, String> {
                let syntax = leselang_syntax::parse(source);
                if !syntax.diagnostics.is_empty() {
                    return Err(format!("{:?}", syntax.diagnostics));
                }
                let ty = T::sample(0).scalar().scalar_type();
                let parameters = [NamedParameter::required("value", ScalarTypeSet::only(ty))];
                let schemas: Vec<_> = [0, 1, 2, 9]
                    .into_iter()
                    .map(|index| OperationSchema {
                        key: op(index),
                        parameters: if index == 9 { &[] } else { &parameters },
                        result: index,
                        required_capability: if index == 9 { 8 } else { 7 },
                    })
                    .collect();
                let catalog = OperationCatalog::new(
                    31,
                    &schemas,
                    OperationCatalogLimits {
                        max_operations: 4,
                        max_parameters_per_operation: 1,
                    },
                )
                .unwrap();
                let host = SourceCallHost {
                    catalog: &catalog,
                    version,
                    granted,
                };
                let (tree, result) = if let Some(captured) = captured {
                    let keep = PureType::Scalar(ScalarType::Boolean);
                    let seed = PureType::Result(Meta::Atom(0));
                    let group = PureType::Result(Meta::Group(
                        captured,
                        (0..3).map(|i| (format!("m{i}"), op(i))).collect(),
                    ));
                    // Only type observations enter compilation; runtime captures come from the host.
                    let prefix = [
                        ("keep", &keep),
                        ("seed", &seed),
                        ("batch", &group),
                        ("alias", &group),
                    ];
                    let expression = &syntax.function.as_ref().ok_or("entry")?.body;
                    compile_catalog_prepared_group_source(
                        expression,
                        &prefix,
                        COMPILE,
                        &host,
                        &Types(ty),
                    )
                    .map_err(|e| e.to_string())?
                } else {
                    let declarations: Vec<_> =
                        syntax.function.iter().chain(&syntax.helpers).collect();
                    let mut expression = COMPILE.expression;
                    expression.max_calls = max_calls;
                    compile_catalog_prepared_group_helper_program(
                        &declarations,
                        "main",
                        CatalogGroupProgramLimits {
                            program: CatalogProgramLimits {
                                expression,
                                max_functions: 3,
                                max_parameters: 0,
                                max_compilation_nodes: 1024,
                                max_expansion_depth: 16,
                            },
                            max_groups,
                            max_branches: 3,
                        },
                        &host,
                        &Types(ty),
                    )
                    .map_err(|e| e.to_string())?
                };
                let expected = match final_group {
                    Some(kind) => PureType::Result(Meta::Group(
                        kind,
                        (0..3).map(|i| (format!("m{i}"), op(i))).collect(),
                    )),
                    None => PureType::Scalar(ty),
                };
                assert_eq!(result, expected);
                Ok(tree)
            }
            pub(super) fn program<T: Projection>(kind: GroupKind) -> String {
                let kind = if kind == GroupKind::Sequence {
                    "seq"
                } else {
                    "all"
                };
                let members = (0..3).map(|i| format!(
                    "m{i}: operation{i}(value: field(value: member(value: alias, name: \"m{i}\"), name: \"value\"))"
                )).collect::<Vec<_>>().join(", ");
                program_with_group::<T>(&format!("{kind}({members})"), "m2")
            }
            pub(super) fn program_with_group<T: Projection>(group: &str, last: &str) -> String {
                let scalar = match T::sample(0).scalar() {
                    ScalarValue::Integer(value) => value.to_string(),
                    ScalarValue::String(value) => serde_json::to_string(&value).unwrap(),
                    _ => panic!("fixture payload"),
                };
                format!(
                    "fn main() = bind(output: bind(reply: {group}, body: \
                    choose(when: keep, then: choose(when: eq(left: field(value: seed, name: \"value\"), right: {scalar}), \
                    then: field(value: member(value: reply, name: \"{last}\"), name: \"value\"), otherwise: {scalar}), otherwise: {scalar})), \
                    body: choose(when: false, then: operation9(), otherwise: output))"
                )
            }
            fn initial<T: Projection>(
                kind: GroupKind,
                active: GroupKind,
                frontier: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
            ) -> Initial {
                let tree = compile::<T>(&program::<T>(active), kind, 31, &[7, 8]).unwrap();
                match active {
                    GroupKind::Sequence => {
                        super::initial_with_tree::<T>(kind, frontier, allowance, save, tree)
                    }
                    GroupKind::Parallel => parallel::initial_with_tree::<T>(
                        kind, frontier, order, allowance, save, tree,
                    ),
                }
            }
            pub(super) fn resume<T: Projection>(
                saved: &Saved<T>,
                active: GroupKind,
                frontier: usize,
                order: [usize; 3],
                mode: usize,
            ) -> Result<Report, &'static str> {
                match active {
                    GroupKind::Sequence => {
                        let (report, restored, dispatched, rebuilt) = super::restored(saved, mode)?;
                        if mode == 0 && report.value.is_ok() {
                            assert_eq!(
                                (restored, dispatched, rebuilt),
                                (3 - frontier.div_ceil(2), 3 - frontier.div_ceil(2), 1)
                            );
                        }
                        Ok(report)
                    }
                    GroupKind::Parallel => {
                        let (report, rebuilt) = parallel::restored(saved, order, mode)?;
                        if mode == 0 && report.value.is_ok() {
                            assert_eq!(rebuilt, 1);
                        }
                        Ok(report)
                    }
                }
            }
            pub(super) fn cases(mut run: impl FnMut(GroupKind, GroupKind, usize, [usize; 3])) {
                for captured in KINDS {
                    for frontier in 0..7 {
                        run(captured, GroupKind::Sequence, frontier, [0, 1, 2]);
                    }
                    for mask in 0..8 {
                        for order in parallel::ORDERS {
                            run(captured, GroupKind::Parallel, mask, order);
                        }
                    }
                }
            }
            #[test]
            fn source_compiled_seq_and_all_restore_bytes_through_the_original_walker() {
                fn run<T: Projection>() {
                    cases(|kind, active, frontier, order| {
                        let Initial::Finished(expected) =
                            initial::<T>(kind, active, frontier, order, None, false)
                        else {
                            panic!()
                        };
                        let Initial::Saved(bytes) =
                            initial::<T>(kind, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let decoded = decode::<T>(&bytes, &bytes).unwrap();
                        let actual = resume(&decoded, active, frontier, order, 0).unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                        assert_eq!((actual.restored, actual.projected), (2, 1));
                    });
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn source_compiled_recovery_matches_every_assembly_and_return_fuel_cutoff() {
                fn run<T: Projection>() {
                    cases(|kind, active, frontier, order| {
                        let Initial::Saved(bytes) =
                            initial::<T>(kind, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let decoded = decode::<T>(&bytes, &bytes).unwrap();
                        let needed = decoded.remaining
                            - resume(&decoded, active, frontier, order, 0)
                                .unwrap()
                                .remaining;
                        for allowance in 0..=needed + 1 {
                            let Initial::Finished(expected) =
                                initial::<T>(kind, active, frontier, order, Some(allowance), false)
                            else {
                                panic!()
                            };
                            let Initial::Saved(bytes) =
                                initial::<T>(kind, active, frontier, order, Some(allowance), true)
                            else {
                                panic!()
                            };
                            let decoded = decode::<T>(&bytes, &bytes).unwrap();
                            let actual = resume(&decoded, active, frontier, order, 0).unwrap();
                            assert_eq!(
                                actual, expected,
                                "{active:?}/{frontier}/{order:?}/{allowance}"
                            );
                            assert_eq!(
                                actual.value,
                                if allowance < needed {
                                    Err(Failure::Fuel)
                                } else {
                                    Ok(T::sample(2).scalar())
                                }
                            );
                        }
                    });
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn source_compiled_cold_and_current_authority_checks_still_guard_return_frames() {
                cases(|kind, active, frontier, order| {
                    let Initial::Saved(bytes) =
                        initial::<String>(kind, active, frontier, order, None, true)
                    else {
                        panic!()
                    };
                    let decoded = decode::<String>(&bytes, &bytes).unwrap();
                    assert!(resume(&decoded, active, frontier, order, 1).is_err());
                    assert!(resume(&decoded, active, frontier, order, 2).is_err());
                    let cold = resume(&decoded, active, frontier, order, 3).unwrap();
                    assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                    assert_eq!(
                        (cold.remaining, cold.restored, cold.projected),
                        (decoded.remaining - 4, 0, 0)
                    );
                    let late = resume(&decoded, active, frontier, order, 4).unwrap();
                    assert!(late.value.is_err());
                    assert_eq!((late.restored, late.projected), (2, 1));
                });
            }
            #[test]
            fn source_compiler_refuses_bad_names_types_and_cold_capabilities_before_execution() {
                for kind in KINDS {
                    let text = program::<String>(kind);
                    assert!(compile::<String>(&text, kind, 31, &[7, 8]).is_ok());
                    for invalid in [
                        text.replace("operation1(", "missing("),
                        text.replace("name: \"value\"", "name: \"missing\""),
                        text.replace("name: \"m1\"", "name: \"missing\""),
                        text.replace("value: alias", "value: absent"),
                        text.replace("operation9()", "operation9(value: 0)"),
                        text.replace("operation9()", "missing()"),
                        text.replace("operation1(value: field(value: member(value: alias, name: \"m1\"), name: \"value\"))", "operation1(value: 1)"),
                        text.replace("fn main()", "fn main("),
                    ] {
                        assert_ne!(invalid, text);
                        assert!(compile::<String>(&invalid, kind, 31, &[7, 8]).is_err(), "{invalid}");
                    }
                    assert!(compile::<String>(&text, kind, 30, &[7, 8]).is_err());
                    assert!(compile::<String>(&text, kind, 31, &[]).is_err());
                    assert!(compile::<String>(&text, kind, 31, &[7]).is_err());
                }
            }
        }

        mod whole_program {
            use super::*;

            struct Bootstrap<'r, 'a, T> {
                next: &'r Adapter<'a, T>,
                owner: &'a Host,
                bindings: usize,
                restored: Cell<usize>,
                projected: Cell<usize>,
            }
            impl<'a, T: Projection> PureEvaluationEnvironment<(), Operation> for Bootstrap<'_, 'a, T> {
                type Result = View<'a, T>;
                type Error = &'static str;
                fn field(
                    &self,
                    value: &Self::Result,
                    field: &(),
                ) -> Result<ScalarValue, Self::Error> {
                    self.next.field(value, field)
                }
                fn member(
                    &self,
                    value: &Self::Result,
                    name: &str,
                    operation: &Operation,
                ) -> Result<Self::Result, Self::Error> {
                    self.next.member(value, name, operation)
                }
            }
            impl<'a, T: Projection> EffectEvaluationEnvironment<'a, (), Operation, (), usize>
                for Bootstrap<'_, 'a, T>
            {
                type Request = Work<'a, T>;
                type Capture = Capture<'a, T>;
                fn preflight_effect(&self, node: &'a Node) -> Result<(), Self::Error> {
                    self.next.preflight_effect(node)
                }
                fn prepare_effect(
                    &self,
                    node: &'a Node,
                    scope: &mut ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                    fuel: &mut Fuel,
                ) -> Result<Self::Request, CalculationFailure<Self::Error>> {
                    self.next.prepare_effect(node, scope, fuel)
                }
                fn capture(
                    &self,
                    name: &'a str,
                    body: &'a Node,
                    scope: &ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                    fuel: &mut Fuel,
                ) -> Result<Self::Capture, Self::Error> {
                    self.next.capture(name, body, scope, fuel)
                }
            }
            impl<'a, T: Projection>
                EffectReentryEnvironment<
                    'a,
                    (),
                    Operation,
                    (),
                    usize,
                    u64,
                    Root<'a>,
                    Rc<Group<'a, T>>,
                > for Bootstrap<'_, 'a, T>
            {
                fn restore_capture(
                    &self,
                    id: &u64,
                    domain: &Root<'a>,
                    capture: Self::Capture,
                    _: &mut Fuel,
                    _: EffectEvaluationLimits,
                ) -> Result<RestoredEffectBindings<'a, Self::Result>, CalculationFailure<Self::Error>>
                {
                    self.owner.authorize(id)?;
                    if *id != self.owner.key(90) || !std::ptr::eq(domain.host, self.owner) {
                        return Err("bootstrap owner".into());
                    }
                    let Capture::Live(rows) = capture else {
                        return Err("bootstrap capture".into());
                    };
                    if rows.len() != self.bindings {
                        return Err("preseeded capture".into());
                    }
                    for (_, value) in &rows {
                        if let PureValue::Result(view) = value {
                            self.next.captured.admit_view(view)?;
                        }
                    }
                    self.restored.set(self.restored.get() + 1);
                    Ok(rows)
                }
                fn bind_reply(
                    &self,
                    id: &u64,
                    domain: &Root<'a>,
                    reply: Rc<Group<'a, T>>,
                    _: &mut Fuel,
                ) -> Result<PureValue<Self::Result>, CalculationFailure<Self::Error>>
                {
                    self.owner.authorize(id)?;
                    domain.validate_value(&reply)?;
                    self.projected.set(self.projected.get() + 1);
                    Ok(PureValue::Result(View::Group(reply)))
                }
            }
            fn program<T: Projection>(captured: GroupKind, active: GroupKind) -> String {
                let kind = if captured == GroupKind::Sequence {
                    "seq"
                } else {
                    "all"
                };
                let members = (0..3)
                    .map(|i| {
                        let value = match T::sample(i).scalar() {
                            ScalarValue::Integer(v) => v.to_string(),
                            ScalarValue::String(v) => serde_json::to_string(&v).unwrap(),
                            _ => panic!("fixture payload"),
                        };
                        format!("m{i}: operation{i}(value: {value})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let tail = source::program::<T>(active);
                let tail = tail.strip_prefix("fn main() = ").unwrap();
                format!(
                    "fn seed_group() = {kind}({members})\n\
                    fn main() = bind(batch: seed_group(), body: bind(alias: batch, body: bind(keep: true, \
                    body: bind(seed: member(value: batch, name: \"m0\"), body: {tail}))))"
                )
            }
            fn group_sites(tree: &Node) -> Vec<&Node> {
                group_sites_with_count(tree, 2)
            }
            fn group_sites_with_count(tree: &Node, count: usize) -> Vec<&Node> {
                let mut pending = vec![tree];
                let mut groups = Vec::new();
                let mut seen = 0;
                while let Some(node) = pending.pop() {
                    seen += 1;
                    assert!(seen <= STACK.effects.pure.max_nodes);
                    if matches!(node, Node::Group { .. }) {
                        groups.push(node);
                    }
                    let children: Vec<_> = node.children().collect();
                    pending.extend(children.into_iter().rev());
                }
                assert_eq!(groups.len(), count);
                groups
            }
            fn collect_group<'a, T: Projection>(
                env: &Adapter<'a, T>,
                work: Work<'a, T>,
            ) -> Vec<Row<'a, T>> {
                match work {
                    Work::Sequence(queue) => {
                        collect_sequence(env, queue, Vec::new(), None).unwrap()
                    }
                    Work::Parallel(mut barrier, requests) => {
                        for request in requests.into_iter().rev() {
                            let reply = env.invoke(request.request);
                            barrier
                                .try_accept::<T, _>(
                                    request.index,
                                    &env.active.key(request.index),
                                    reply,
                                    env.active,
                                )
                                .unwrap();
                        }
                        let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
                            panic!("bootstrap barrier")
                        };
                        assert!(matches!(
                            barrier.poll().unwrap(),
                            ParallelPoll::Terminal(ParallelEnd::Completed)
                        ));
                        rows
                    }
                }
            }
            fn initial<T: Projection>(
                kind: GroupKind,
                active_kind: GroupKind,
                frontier: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
            ) -> Initial {
                initial_source::<T>(
                    &program::<T>(kind, active_kind),
                    active_kind,
                    frontier,
                    order,
                    allowance,
                    save,
                )
            }
            fn initial_source<T: Projection>(
                text: &str,
                active_kind: GroupKind,
                frontier: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
            ) -> Initial {
                let tree = source::compile_model::<T>(text, None, 31, &[7, 8]).unwrap();
                let nodes = group_sites(&tree);
                let host = Host::new(3);
                let active = Host::new(3);
                active.generation.set(2);
                let parameters = [NamedParameter::required(
                    "value",
                    ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                )];
                let old_schemas = schemas::<T>(&host, &parameters, nodes[0]);
                let new_schemas = schemas::<T>(&active, &parameters, nodes[1]);
                let next = adapter(&host, &active, &new_schemas, None);
                let mut first = adapter(&host, &host, &old_schemas, None);
                first.captured.drops = next.captured.drops.clone();
                let mut fuel = Fuel::new(1000);
                let mut bindings = Vec::new();
                let StackedEffectOutcome::Suspended {
                    request,
                    continuation,
                } = evaluate_stacked_effects_in_scope(
                    &tree,
                    &mut ScopeFrame::new(&mut bindings),
                    &first,
                    &mut fuel,
                    STACK,
                )
                .unwrap()
                else {
                    panic!("first source group")
                };
                assert!(bindings.is_empty());
                assert_eq!(continuation.frame_count(), 1);
                let rows = collect_group(&first, request);
                let assembled = Rc::new(
                    assemble_group_reply(nodes[0], rows, QUEUE_LIMITS, &mut fuel, |g, _| {
                        admit(g, &host)
                    })
                    .unwrap(),
                );
                let assembly_remaining = fuel.remaining();
                let bootstrap = Bootstrap {
                    next: &next,
                    owner: &host,
                    bindings: 0,
                    restored: Cell::new(0),
                    projected: Cell::new(0),
                };
                let domain = Root {
                    host: &host,
                    node: nodes[0],
                };
                let mut pending = PendingReply::new(host.key(90), continuation, &domain);
                let wrong = pending
                    .try_accept::<_, Group<'_, T>, _>(&active.key(90), assembled.clone(), &host)
                    .unwrap_err();
                assert!(Rc::ptr_eq(&wrong.reply, &assembled));
                assert_eq!(
                    (
                        bootstrap.restored.get(),
                        bootstrap.projected.get(),
                        active.preparations.get(),
                        active.dispatches.get()
                    ),
                    (0, 0, 0, 0)
                );
                host.granted.set(false);
                let revoked = pending
                    .try_accept::<_, Group<'_, T>, _>(&host.key(90), wrong.reply, &host)
                    .unwrap_err();
                assert!(Rc::ptr_eq(&revoked.reply, &assembled));
                host.granted.set(true);
                host.domains[2].denied.set(true);
                let denied = pending
                    .try_accept::<_, Group<'_, T>, _>(&host.key(90), revoked.reply, &host)
                    .unwrap_err();
                assert!(Rc::ptr_eq(&denied.reply, &assembled));
                assert_eq!(
                    (
                        bootstrap.restored.get(),
                        bootstrap.projected.get(),
                        active.preparations.get(),
                        active.dispatches.get()
                    ),
                    (0, 0, 0, 0)
                );
                host.domains[2].denied.set(false);
                let accepted = pending
                    .try_accept::<_, Group<'_, T>, _>(&host.key(90), denied.reply, &host)
                    .unwrap();
                let StackedEffectOutcome::Suspended {
                    request: work,
                    continuation,
                } = resume_accepted_stacked_effects(accepted, &bootstrap, &mut fuel, STACK)
                    .unwrap()
                else {
                    panic!("second source group")
                };
                assert_eq!(
                    (bootstrap.restored.get(), bootstrap.projected.get()),
                    (1, 1)
                );
                assert_eq!((host.preparations.get(), host.dispatches.get()), (3, 3));
                assert!(first.inventory.borrow().iter().all(Option::is_none));
                assert_eq!(
                    (next.captured.restored.get(), next.captured.projected.get()),
                    (0, 0)
                );
                // Save only the compiled pending group and original return bodies, never recompile on resume.
                let flight = Flight {
                    assembled,
                    assembly_remaining,
                    additional: Vec::new(),
                    tree: nodes[1],
                    work,
                    continuation,
                    fuel,
                };
                match active_kind {
                    GroupKind::Sequence => checkpoint(&next, flight, frontier, allowance, save),
                    GroupKind::Parallel => {
                        parallel::checkpoint(&next, flight, frontier, order, allowance, save)
                    }
                }
            }
            fn decode<T: Projection>(
                bytes: &[u8],
                committed: &[u8],
            ) -> Result<Saved<T>, &'static str> {
                decode_with_origin(bytes, committed, None, 100)
            }
            #[test]
            fn helper_origin_groups_create_all_captures_before_byte_checkpoint_recovery() {
                fn run<T: Projection>() {
                    source::cases(|kind, active, frontier, order| {
                        let Initial::Finished(expected) =
                            initial::<T>(kind, active, frontier, order, None, false)
                        else {
                            panic!()
                        };
                        let Initial::Saved(bytes) =
                            initial::<T>(kind, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let saved = decode::<T>(&bytes, &bytes).unwrap();
                        for frame in &saved.frames {
                            assert_eq!(
                                (frame.locals.len(), frame.results.len(), frame.groups.len()),
                                (1, 1, 2)
                            );
                            assert_eq!(frame.results[0].result.id, 100);
                        }
                        for row in branches(&saved.captured.node) {
                            let Node::Call { arguments, .. } = &row.value else {
                                panic!()
                            };
                            assert_eq!(arguments.len(), 1);
                        }
                        let actual = source::resume(&saved, active, frontier, order, 0).unwrap();
                        assert_eq!(actual, expected);
                        assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                        assert_eq!((actual.restored, actual.projected), (2, 1));
                    });
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn helper_origin_recovery_preserves_every_assembly_and_return_fuel_cutoff() {
                fn run<T: Projection>() {
                    source::cases(|kind, active, frontier, order| {
                        let Initial::Saved(bytes) =
                            initial::<T>(kind, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let saved = decode::<T>(&bytes, &bytes).unwrap();
                        let needed = saved.remaining
                            - source::resume(&saved, active, frontier, order, 0)
                                .unwrap()
                                .remaining;
                        for allowance in 0..=needed + 1 {
                            let Initial::Finished(expected) =
                                initial::<T>(kind, active, frontier, order, Some(allowance), false)
                            else {
                                panic!()
                            };
                            let Initial::Saved(bytes) =
                                initial::<T>(kind, active, frontier, order, Some(allowance), true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            let actual =
                                source::resume(&saved, active, frontier, order, 0).unwrap();
                            assert_eq!(
                                actual, expected,
                                "{kind:?}/{active:?}/{frontier}/{order:?}/{allowance}"
                            );
                            assert_eq!(
                                actual.value,
                                if allowance < needed {
                                    Err(Failure::Fuel)
                                } else {
                                    Ok(T::sample(2).scalar())
                                }
                            );
                        }
                    });
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn helper_origin_source_and_cold_helpers_are_checked_without_external_prefix() {
                for kind in KINDS {
                    for active in KINDS {
                        let text = program::<String>(kind, active);
                        assert!(source::compile_model::<String>(&text, None, 31, &[7, 8]).is_ok());
                        for bad in [
                            text.replace("batch: seed_group()", "batch: absent()"),
                            text.replace("batch: seed_group()", "batch: seed_group(value: 1)"),
                            text.replace("operation1(value: \"panel-1\")", "operation1(value: 1)"),
                            format!("{text}\nfn unused() = missing()"),
                            format!("{text}\nfn unused() = unused()"),
                            text.replace("when: keep", "when: external"),
                            text.replace("operation9()", "operation9(value: 1)"),
                        ] {
                            assert_ne!(bad, text);
                            assert!(
                                source::compile_model::<String>(&bad, None, 31, &[7, 8]).is_err(),
                                "{bad}"
                            );
                        }
                        assert!(source::compile_model::<String>(&text, None, 30, &[7, 8]).is_err());
                        assert!(source::compile_model::<String>(&text, None, 31, &[7]).is_err());
                        assert!(source::compile_model::<String>(&text, None, 31, &[]).is_err());
                    }
                }
            }
            #[test]
            fn helper_origin_checkpoint_rechecks_current_and_cold_return_authority() {
                source::cases(|kind, active, frontier, order| {
                    let Initial::Saved(bytes) =
                        initial::<String>(kind, active, frontier, order, None, true)
                    else {
                        panic!()
                    };
                    let saved = decode::<String>(&bytes, &bytes).unwrap();
                    assert!(source::resume(&saved, active, frontier, order, 1).is_err());
                    assert!(source::resume(&saved, active, frontier, order, 2).is_err());
                    let cold = source::resume(&saved, active, frontier, order, 3).unwrap();
                    assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                    assert_eq!(
                        (cold.remaining, cold.restored, cold.projected),
                        (saved.remaining - 4, 0, 0)
                    );
                    let late = source::resume(&saved, active, frontier, order, 4).unwrap();
                    assert!(late.value.is_err());
                    assert_eq!((late.restored, late.projected), (2, 1));
                });
            }
            #[test]
            fn helper_origin_forged_graph_capture_identity_and_budget_do_not_enter_recovery() {
                for active in KINDS {
                    let Initial::Saved(bytes) =
                        initial::<String>(GroupKind::Parallel, active, 2, [2, 0, 1], None, true)
                    else {
                        panic!()
                    };
                    for case in 0..7 {
                        let mut saved = decode::<String>(&bytes, &bytes).unwrap();
                        match case {
                            0 => saved.frames[0].results[0].result.id = SEED,
                            1 => saved.captured.node = group(GroupKind::Sequence, 3),
                            2 => saved.rows.swap(0, 2),
                            3 => saved.frames[0].groups[0].group = 1,
                            4 => saved.tree = literal(ScalarValue::None),
                            5 => saved.captured.remaining = 1001,
                            _ => saved.remaining += 1,
                        }
                        let candidate = serde_json::to_vec(&saved).unwrap();
                        assert!(decode::<String>(&candidate, &bytes).is_err());
                    }
                    assert!(decode::<String>(&vec![b' '; 64 * 1024 + 1], &bytes).is_err());
                }
            }

            mod multiple {
                use super::*;

                fn program<T: Projection>(
                    first: GroupKind,
                    second: GroupKind,
                    active: GroupKind,
                ) -> String {
                    let scalar = |i| match T::sample(i).scalar() {
                        ScalarValue::Integer(v) => v.to_string(),
                        ScalarValue::String(v) => serde_json::to_string(&v).unwrap(),
                        _ => panic!("fixture payload"),
                    };
                    let group = |kind| {
                        format!(
                            "{}({})",
                            if kind == GroupKind::Sequence {
                                "seq"
                            } else {
                                "all"
                            },
                            (0..3)
                                .map(|i| format!("m{i}: operation{i}(value: {})", scalar(i)))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    let tail = source::program::<T>(active)
                        .replace("member(value: alias, name: \"m1\")", "member(value: second_alias, name: \"m1\")")
                        .replace("when: keep", &format!("when: eq(left: field(value: member(value: second_alias, name: \"m0\"), name: \"value\"), right: {})", scalar(0)));
                    let tail = tail.strip_prefix("fn main() = ").unwrap();
                    format!(
                        "fn seed_first() = {}\nfn seed_second() = {}\nfn main() = bind(batch: seed_first(), body: bind(alias: batch, body: bind(second: seed_second(), body: bind(second_alias: second, body: bind(keep: true, body: bind(seed: member(value: batch, name: \"m0\"), body: {tail}))))))",
                        group(first),
                        group(second)
                    )
                }
                fn compile<T: Projection>(text: &str) -> Result<Node, String> {
                    source::compile_model_with_budget::<T>(text, None, 31, &[7, 8], 10, 3)
                }
                fn initial<T: Projection>(
                    first_kind: GroupKind,
                    second_kind: GroupKind,
                    active_kind: GroupKind,
                    frontier: usize,
                    order: [usize; 3],
                    allowance: Option<u64>,
                    save: bool,
                ) -> Initial {
                    let tree =
                        compile::<T>(&program::<T>(first_kind, second_kind, active_kind)).unwrap();
                    initial_tree::<T>(tree, 3, active_kind, frontier, order, allowance, save)
                }
                fn initial_tree<T: Projection>(
                    tree: Node,
                    groups: usize,
                    active_kind: GroupKind,
                    frontier: usize,
                    order: [usize; 3],
                    allowance: Option<u64>,
                    save: bool,
                ) -> Initial {
                    let nodes = group_sites_with_count(&tree, groups);
                    let hosts = [Host::new(3), Host::new(3), Host::new(3)];
                    for (i, host) in hosts.iter().enumerate() {
                        host.generation.set(i as u64 + 1);
                    }
                    let parameters = [NamedParameter::required(
                        "value",
                        ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                    )];
                    let schemas: Vec<_> = hosts
                        .iter()
                        .zip(&nodes)
                        .map(|(host, node)| schemas::<T>(host, &parameters, node))
                        .collect();
                    let drops = Rc::new(Cell::new(0));
                    let envs: Vec<_> = hosts
                        .iter()
                        .zip(&schemas)
                        .map(|(host, schemas)| {
                            let mut env = adapter(&hosts[0], host, schemas, None);
                            env.captured.drops = drops.clone();
                            env.captured.captured_hosts.push(&hosts[1]);
                            env.captured.expected_groups = 2;
                            env
                        })
                        .collect();
                    let mut bindings = Vec::new();
                    let mut fuel = Fuel::new(1000);
                    let mut outcome = evaluate_stacked_effects_in_scope(
                        &tree,
                        &mut ScopeFrame::new(&mut bindings),
                        &envs[0],
                        &mut fuel,
                        STACK,
                    )
                    .unwrap();
                    assert!(bindings.is_empty());
                    let mut completed = Vec::new();
                    for i in 0..2 {
                        let StackedEffectOutcome::Suspended {
                            request,
                            continuation,
                        } = outcome
                        else {
                            panic!("bootstrap group")
                        };
                        assert_eq!(continuation.frame_count(), 1);
                        let rows = collect_group(&envs[i], request);
                        let group = Rc::new(
                            assemble_group_reply(
                                nodes[i],
                                rows,
                                QUEUE_LIMITS,
                                &mut fuel,
                                |g, _| admit(g, &hosts[i]),
                            )
                            .unwrap(),
                        );
                        let remaining = fuel.remaining();
                        let domain = Root {
                            host: &hosts[i],
                            node: nodes[i],
                        };
                        let bootstrap = Bootstrap {
                            next: &envs[i + 1],
                            owner: &hosts[i],
                            bindings: 2 * i,
                            restored: Cell::new(0),
                            projected: Cell::new(0),
                        };
                        let mut pending =
                            PendingReply::new(hosts[i].key(90), continuation, &domain);
                        let wrong = pending
                            .try_accept::<_, Group<'_, T>, _>(
                                &hosts[i + 1].key(90),
                                group.clone(),
                                &hosts[i],
                            )
                            .unwrap_err();
                        assert!(Rc::ptr_eq(&wrong.reply, &group));
                        assert_eq!(
                            (
                                bootstrap.restored.get(),
                                bootstrap.projected.get(),
                                hosts[i + 1].preparations.get()
                            ),
                            (0, 0, 0)
                        );
                        let accepted = pending
                            .try_accept::<_, Group<'_, T>, _>(
                                &hosts[i].key(90),
                                wrong.reply,
                                &hosts[i],
                            )
                            .unwrap();
                        outcome =
                            resume_accepted_stacked_effects(accepted, &bootstrap, &mut fuel, STACK)
                                .unwrap();
                        assert_eq!(
                            (bootstrap.restored.get(), bootstrap.projected.get()),
                            (1, 1)
                        );
                        assert_eq!(
                            (hosts[i].preparations.get(), hosts[i].dispatches.get()),
                            (3, 3)
                        );
                        assert!(envs[i].inventory.borrow().iter().all(Option::is_none));
                        completed.push((group, remaining));
                    }
                    let StackedEffectOutcome::Suspended {
                        request: work,
                        continuation,
                    } = outcome
                    else {
                        panic!("pending third group")
                    };
                    let (assembled, assembly_remaining) = completed.remove(0);
                    assert!(!Rc::ptr_eq(&assembled, &completed[0].0));
                    let flight = Flight {
                        assembled,
                        assembly_remaining,
                        additional: completed,
                        tree: nodes[2],
                        work,
                        continuation,
                        fuel,
                    };
                    match active_kind {
                        GroupKind::Sequence => {
                            checkpoint(&envs[2], flight, frontier, allowance, save)
                        }
                        GroupKind::Parallel => {
                            parallel::checkpoint(&envs[2], flight, frontier, order, allowance, save)
                        }
                    }
                }
                fn decode<T: Projection>(
                    bytes: &[u8],
                    committed: &[u8],
                ) -> Result<Saved<T>, &'static str> {
                    if bytes.len() > 64 * 1024 || bytes != committed {
                        return Err("store admission");
                    }
                    let saved: Saved<T> = serde_json::from_slice(bytes).map_err(|_| "decode")?;
                    if saved.additional.len() != 1
                        || saved.frames.len() != 2
                        || saved.rows.len() != 3
                    {
                        return Err("table bounds");
                    }
                    let mut previous = 1000;
                    for (index, image) in std::iter::once(&saved.captured)
                        .chain(&saved.additional)
                        .enumerate()
                    {
                        if !image.assembled || image.rows.len() != 3 || image.remaining > previous {
                            return Err("group bounds");
                        }
                        for (position, (id, _)) in image.rows.iter().enumerate() {
                            if *id != (index as u64 + 1) * 100 + position as u64 {
                                return Err("group identity");
                            }
                        }
                        previous = image.remaining;
                    }
                    if saved.remaining > previous {
                        return Err("budget");
                    }
                    for frame in &saved.frames {
                        if frame.locals.len() != 1
                            || frame.results.len() != 1
                            || frame.groups.len() != 4
                        {
                            return Err("capture shape");
                        }
                        validate_captured_bindings(
                            frame,
                            CaptureValidationLimits {
                                max_saved_bindings: 6,
                                max_name_bytes: 64,
                            },
                            |v| {
                                if v.id == 100 && v.value == T::sample(0) {
                                    Ok(())
                                } else {
                                    Err("atom")
                                }
                            },
                            |g| if *g < 2 { Ok(()) } else { Err("group ref") },
                            |_| Ok(()),
                        )
                        .map_err(|_| "capture bounds")?;
                        for (group, (name, index)) in frame.groups.iter().zip([
                            ("batch", 0),
                            ("alias", 0),
                            ("second", 1),
                            ("second_alias", 1),
                        ]) {
                            if group.name != name || group.group != index {
                                return Err("group alias");
                            }
                        }
                    }
                    Ok(saved)
                }
                fn configure<'a, T: Projection>(
                    env: &mut Adapter<'a, T>,
                    second: &'a Host,
                    saved: Option<&'a Image<T>>,
                ) {
                    env.captured.captured_hosts.push(second);
                    env.captured.expected_groups = 2;
                    if let Some(saved) = saved {
                        env.captured.saved.push((second, saved));
                        env.captured.groups.borrow_mut().push(None);
                    }
                }
                fn restored<T: Projection>(
                    saved: &Saved<T>,
                    order: [usize; 3],
                    mode: usize,
                ) -> Result<Report, &'static str> {
                    let first = Host::new(3);
                    let second = Host::new(3);
                    second.generation.set(2);
                    let active = Host::new(3);
                    active.generation.set(3);
                    let parameters = [NamedParameter::required(
                        "value",
                        ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                    )];
                    let schemas = schemas::<T>(&active, &parameters, &saved.tree);
                    let mut env = adapter(&first, &active, &schemas, Some(&saved.captured));
                    configure(&mut env, &second, Some(&saved.additional[0]));
                    active.granted.set(mode != 1);
                    env.captured.deny_outer.set(mode == 3);
                    env.captured.revoke_outer.set(mode == 4 || mode == 5);
                    env.captured.revoke_group = usize::from(mode == 5);
                    if mode == 6 {
                        first.domains[2].denied.set(true);
                    }
                    if mode == 7 {
                        second.domains[2].denied.set(true);
                    }
                    let Node::Group { group_kind, .. } = &saved.tree else {
                        return Err("pending group");
                    };
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
                    let mut fuel = Fuel::new(saved.remaining);
                    let result = match group_kind {
                        GroupKind::Sequence => {
                            restored_queue(saved, &env).and_then(|(queue, rows, inbox)| {
                                if mode == 2 {
                                    active.granted.set(false);
                                }
                                finish(
                                    &env,
                                    &saved.tree,
                                    queue,
                                    rows,
                                    inbox,
                                    continuation,
                                    &mut fuel,
                                )
                            })
                        }
                        GroupKind::Parallel => parallel::restore_barrier(saved, &env).and_then(
                            |(mut barrier, mut inbox)| {
                                if mode == 2 {
                                    active.granted.set(false);
                                }
                                let rows =
                                    parallel::complete(&env, &mut barrier, &mut inbox, order)?;
                                assemble_and_resume(
                                    &env,
                                    &saved.tree,
                                    rows,
                                    continuation,
                                    &mut fuel,
                                )
                            },
                        ),
                    };
                    assert_eq!(
                        (
                            first.preparations.get(),
                            second.preparations.get(),
                            active.preparations.get(),
                            first.dispatches.get(),
                            second.dispatches.get()
                        ),
                        (0, 0, 0, 0, 0)
                    );
                    if [1, 2].contains(&mode) {
                        assert_eq!(
                            (active.dispatches.get(), env.captured.rebuilt.get()),
                            (0, 0)
                        );
                    }
                    if let Ok(report) = &result
                        && report.value.is_ok()
                    {
                        assert_eq!(env.captured.rebuilt.get(), 2);
                        let unpublished = saved
                            .rows
                            .iter()
                            .filter(|row| matches!(row, SavedRow::Prepared(_)))
                            .count();
                        assert_eq!(
                            (env.restored_calls.get(), active.dispatches.get()),
                            (unpublished, unpublished)
                        );
                    }
                    result
                }
                mod owned {
                    use super::*;
                    use leselang_hir::{
                        effect_session::{
                            EffectCorrelation, EffectSessionAcceptance, EffectSessionEnd,
                            EffectSessionEnvironment, EffectSessionPoll, EffectSessionStatus,
                            HostWorkOutcome,
                        },
                        effect_stack_session::{
                            StackedEffectSession, StackedEffectSessionPollResult,
                            StackedSessionRestoreError,
                        },
                        local_effect_session::LocalStackedEffectSession,
                    };
                    use leselang_runtime_core::{LocalExecutionScope, ScopedLocalExecution};

                    // This bounded source family has only pure return bodies after its third group.
                    // A new effect must not silently acquire a fabricated root declaration.
                    impl<'a: 's, 's, T: Projection>
                        EffectSessionEnvironment<'a, 's, (), Operation, (), usize>
                        for Adapter<'a, T>
                    {
                        type Identity = u64;
                        type Declaration = Root<'a>;
                        type Reply = Rc<Group<'a, T>>;
                        type Dispatch = Work<'a, T>;
                        fn correlate_request(
                            &self,
                            _: Self::Request,
                            _: &mut Fuel,
                        ) -> EffectCorrelation<'s, u64, Root<'a>, Self::Dispatch, &'static str>
                        {
                            Err("bounded return fixture has no successor".into())
                        }
                    }
                    type OwnedSession<'a, 's, T> = LocalStackedEffectSession<
                        'a,
                        's,
                        (),
                        Operation,
                        (),
                        usize,
                        View<'a, T>,
                        Capture<'a, T>,
                        u64,
                        Root<'a>,
                        Rc<Group<'a, T>>,
                    >;
                    type RawSession<'a, 's, T> = StackedEffectSession<
                        'a,
                        's,
                        (),
                        Operation,
                        (),
                        usize,
                        View<'a, T>,
                        Capture<'a, T>,
                        u64,
                        Root<'a>,
                        Rc<Group<'a, T>>,
                    >;
                    // Fixture-only routing: both owners execute the same saved graphs,
                    // collectors and meter. There is no interpreter in this adapter.
                    enum Session<'a, 's, T> {
                        Owned(OwnedSession<'a, 's, T>),
                        Borrowed(ScopedLocalExecution<RawSession<'a, 's, T>>),
                    }
                    impl<'a: 's, 's, T: Projection> Session<'a, 's, T> {
                        fn restore_pending(
                            scope: LocalExecutionScope,
                            identity: u64,
                            declaration: &'s Root<'a>,
                            frames: Vec<SavedEffectReturnFrame<'a, Node, Capture<'a, T>>>,
                            fuel: Fuel,
                            admit: impl FnOnce(
                                &u64,
                                &'s Root<'a>,
                                &[SavedEffectReturnFrame<'a, Node, Capture<'a, T>>],
                                &Fuel,
                            ) -> Result<(), &'static str>,
                            borrowed: bool,
                        ) -> Result<Self, StackedSessionRestoreError<&'static str>>
                        {
                            if borrowed {
                                let vm = RawSession::restore_pending_in_scope(
                                    identity,
                                    declaration,
                                    frames,
                                    fuel,
                                    STACK,
                                    admit,
                                    &scope,
                                )?;
                                Ok(Self::Borrowed(scope.bind(vm)))
                            } else {
                                OwnedSession::restore_pending(
                                    scope,
                                    identity,
                                    declaration,
                                    frames,
                                    fuel,
                                    STACK,
                                    admit,
                                )
                                .map(Self::Owned)
                            }
                        }
                        fn status(&self) -> EffectSessionStatus {
                            match self {
                                Self::Owned(vm) => vm.status(),
                                Self::Borrowed(owner) => owner.value().status(),
                            }
                        }
                        fn fuel_remaining(&self) -> u64 {
                            match self {
                                Self::Owned(vm) => vm.fuel_remaining(),
                                Self::Borrowed(owner) => owner.value().fuel_remaining(),
                            }
                        }
                        fn cancel(&mut self) -> bool {
                            match self {
                                Self::Owned(vm) => vm.cancel(),
                                Self::Borrowed(owner) => {
                                    let (scope, vm) = owner.as_parts_mut();
                                    vm.cancel_in_scope(scope)
                                }
                            }
                        }
                        fn poll<E>(
                            &mut self,
                            env: &E,
                        ) -> StackedEffectSessionPollResult<View<'a, T>, Work<'a, T>, &'static str>
                        where
                            E: EffectSessionEnvironment<
                                    'a,
                                    's,
                                    (),
                                    Operation,
                                    (),
                                    usize,
                                    Identity = u64,
                                    Declaration = Root<'a>,
                                    Reply = Rc<Group<'a, T>>,
                                    Dispatch = Work<'a, T>,
                                > + EffectEvaluationEnvironment<
                                    'a,
                                    (),
                                    Operation,
                                    (),
                                    usize,
                                    Capture = Capture<'a, T>,
                                > + PureEvaluationEnvironment<
                                    (),
                                    Operation,
                                    Result = View<'a, T>,
                                    Error = &'static str,
                                >,
                        {
                            match self {
                                Self::Owned(vm) => vm.poll(env),
                                Self::Borrowed(owner) => {
                                    let (scope, vm) = owner.as_parts_mut();
                                    vm.poll_in_scope(env, scope)
                                }
                            }
                        }
                        fn try_accept(
                            &mut self,
                            identity: &u64,
                            reply: Rc<Group<'a, T>>,
                            authority: &Host,
                        ) -> EffectSessionAcceptance<Rc<Group<'a, T>>, &'static str, &'static str>
                        {
                            match self {
                                Self::Owned(vm) => vm.try_accept(identity, reply, authority),
                                Self::Borrowed(owner) => {
                                    let (scope, vm) = owner.as_parts_mut();
                                    vm.try_accept_in_scope(identity, reply, authority, scope)
                                }
                            }
                        }
                        fn with_host_outcome<Input, Output>(
                            &mut self,
                            input: Input,
                            work: impl FnOnce(Input) -> HostWorkOutcome<Output>,
                        ) -> Result<Output, Input> {
                            match self {
                                Self::Owned(vm) => vm.with_host_outcome(input, work),
                                Self::Borrowed(owner) => {
                                    let (scope, vm) = owner.as_parts_mut();
                                    vm.with_host_outcome_in_scope(input, work, scope)
                                }
                            }
                        }
                        fn with_metered_host_outcome<Input, Output>(
                            &mut self,
                            input: Input,
                            work: impl FnOnce(Input, &mut Fuel) -> HostWorkOutcome<Output>,
                        ) -> Result<Output, Input> {
                            match self {
                                Self::Owned(vm) => vm.with_metered_host_outcome(input, work),
                                Self::Borrowed(owner) => {
                                    let (scope, vm) = owner.as_parts_mut();
                                    vm.with_metered_host_outcome_in_scope(input, work, scope)
                                }
                            }
                        }
                    }
                    enum Collector<'a, T> {
                        Sequence(QueueState<'a, T>),
                        Parallel(Barrier<'a, T>, Vec<Option<Input<T>>>),
                    }
                    impl<'a, T: Projection> Collector<'a, T> {
                        fn restore(
                            saved: &'a Saved<T>,
                            env: &Adapter<'a, T>,
                        ) -> Result<Self, &'static str> {
                            match &saved.tree {
                                Node::Group {
                                    group_kind: GroupKind::Sequence,
                                    ..
                                } => restored_queue(saved, env).map(Self::Sequence),
                                Node::Group {
                                    group_kind: GroupKind::Parallel,
                                    ..
                                } => parallel::restore_barrier(saved, env)
                                    .map(|(queue, inbox)| Self::Parallel(queue, inbox)),
                                _ => Err("pending group"),
                            }
                        }
                        fn complete(
                            self,
                            env: &Adapter<'a, T>,
                            order: [usize; 3],
                        ) -> Result<Vec<Row<'a, T>>, &'static str> {
                            match self {
                                Self::Sequence((queue, rows, inbox)) => {
                                    collect_sequence(env, queue, rows, inbox)
                                }
                                Self::Parallel(mut queue, mut inbox) => {
                                    parallel::complete(env, &mut queue, &mut inbox, order)
                                }
                            }
                        }
                        fn cancel(mut self, env: &Adapter<'a, T>, lease: &LocalExecutionLease) {
                            assert!(!lease.is_open());
                            assert!(env.active.authorize(&env.active.key(0)).is_err());
                            let input = input::<T>(0, &env.captured.drops);
                            let address = &*input.data as *const T;
                            match &mut self {
                                Self::Sequence((queue, _, _)) => {
                                    queue.cancel();
                                    let rejected = queue
                                        .try_accept::<_, T, _>(
                                            &env.active.key(0),
                                            input,
                                            env.active,
                                        )
                                        .unwrap_err();
                                    assert_eq!(&*rejected.reply.data as *const T, address);
                                    assert!(matches!(
                                        env.poll(queue).unwrap(),
                                        SequencePoll::Terminal(_)
                                    ));
                                }
                                Self::Parallel(queue, _) => {
                                    queue.cancel();
                                    let rejected = queue
                                        .try_accept::<T, _>(
                                            0,
                                            &env.active.key(0),
                                            input,
                                            env.active,
                                        )
                                        .unwrap_err();
                                    assert_eq!(&*rejected.reply.data as *const T, address);
                                    assert!(matches!(
                                        queue.poll().unwrap(),
                                        ParallelPoll::Terminal(_)
                                    ));
                                }
                            }
                            drop(self);
                            env.inventory.borrow_mut().clear();
                        }
                    }

                    #[derive(Clone, Copy, Eq, PartialEq)]
                    enum Stop {
                        None,
                        BeforeCollection,
                        AfterAssembly,
                        AfterAcceptance,
                        DropOwner,
                        DenyCollector,
                        DenyCapture,
                        DenyOuter,
                    }

                    fn run<T: Projection>(
                        saved: &Saved<T>,
                        order: [usize; 3],
                        stop: Stop,
                        borrowed: bool,
                    ) -> Option<Report> {
                        let first = Host::new(3);
                        let second = Host::new(3);
                        second.generation.set(2);
                        let active = Host::new(3);
                        active.generation.set(3);
                        let scope = LocalExecutionScope::new();
                        let lease = scope.lease();
                        for host in [&first, &second, &active] {
                            host.lease.replace(Some(lease.clone()));
                        }
                        let parameters = [NamedParameter::required(
                            "value",
                            ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                        )];
                        let schemas = schemas::<T>(&active, &parameters, &saved.tree);
                        let mut env = adapter(&first, &active, &schemas, Some(&saved.captured));
                        configure(&mut env, &second, Some(&saved.additional[0]));
                        let domain = Root {
                            host: &active,
                            node: &saved.tree,
                        };
                        let mut session = Session::restore_pending(
                            scope, active.key(90), &domain,
                            saved.frames.iter().map(|frame| SavedEffectReturnFrame {
                                name: &frame.name, body: &frame.body, capture: Capture::Saved(frame),
                            }).collect(), Fuel::new(saved.remaining),
                            |id, declaration, frames, fuel| {
                                active.authorize(id)?;
                                if *id != active.key(90) || !std::ptr::eq(declaration, &domain)
                                    || fuel.remaining() != saved.remaining || frames.len() != saved.frames.len()
                                { return Err("pending correspondence"); }
                                for (frame, original) in frames.iter().zip(&saved.frames) {
                                    if frame.name != original.name || !std::ptr::eq(frame.body, &original.body)
                                        || !matches!(frame.capture, Capture::Saved(value) if std::ptr::eq(value, original))
                                    { return Err("return correspondence"); }
                                }
                                Ok(())
                            }, borrowed,
                        ).unwrap();
                        assert!(matches!(
                            session.poll(&env).unwrap(),
                            EffectSessionPoll::Awaiting
                        ));
                        let Ok(collector) = session.with_host_outcome((), |()| {
                            match Collector::restore(saved, &env) {
                                Ok(collector) => HostWorkOutcome::Pending(Ok(collector)),
                                Err(error) => HostWorkOutcome::Failed(Err(error)),
                            }
                        }) else {
                            panic!("pending owner")
                        };
                        let collector = collector.unwrap();
                        assert_eq!(session.fuel_remaining(), saved.remaining);
                        if stop == Stop::DenyCollector {
                            active.granted.set(false);
                        }
                        if stop == Stop::DenyCapture {
                            first.domains[2].denied.set(true);
                        }
                        if stop == Stop::DenyOuter {
                            env.captured.deny_outer.set(true);
                        }
                        if matches!(stop, Stop::BeforeCollection | Stop::DropOwner) {
                            if stop == Stop::DropOwner {
                                drop(session);
                            } else {
                                let Ok(collector) = session
                                    .with_host_outcome(collector, HostWorkOutcome::Cancelled)
                                else {
                                    panic!()
                                };
                                assert_eq!(
                                    session.status(),
                                    EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                                );
                                assert_eq!(session.fuel_remaining(), saved.remaining);
                                collector.cancel(&env, &lease);
                                assert!(!session.cancel());
                                assert_eq!(
                                    (
                                        active.dispatches.get(),
                                        env.captured.restored.get(),
                                        env.captured.rebuilt.get()
                                    ),
                                    (0, 0, 0)
                                );
                                return None;
                            }
                            collector.cancel(&env, &lease);
                            assert_eq!(
                                (
                                    active.dispatches.get(),
                                    env.captured.restored.get(),
                                    env.captured.rebuilt.get()
                                ),
                                (0, 0, 0)
                            );
                            return None;
                        }
                        let Ok(assembled) =
                            session.with_metered_host_outcome(collector, |collector, fuel| {
                                let result = collector
                                    .complete(&env, order)
                                    .map_err(Failure::Native)
                                    .and_then(|rows| {
                                        assemble_group_reply(
                                            &saved.tree,
                                            rows,
                                            QUEUE_LIMITS,
                                            fuel,
                                            |group, _| admit(group, &active),
                                        )
                                        .map(Rc::new)
                                        .map_err(|error| {
                                            match error {
                                                GroupReplyError::Fuel => Failure::Fuel,
                                                GroupReplyError::Native(error) => {
                                                    Failure::Native(error)
                                                }
                                                _ => panic!("assembly structure"),
                                            }
                                        })
                                    });
                                if result.is_ok() {
                                    HostWorkOutcome::Pending(result)
                                } else {
                                    HostWorkOutcome::Failed(result)
                                }
                            })
                        else {
                            panic!("meter owner")
                        };
                        let value = match assembled {
                            Err(error) => {
                                assert_eq!(
                                    session.status(),
                                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                                );
                                assert!(!lease.is_open());
                                env.inventory.borrow_mut().clear();
                                Err(error)
                            }
                            Ok(reply) => {
                                let remaining = session.fuel_remaining();
                                assert_eq!(remaining, saved.remaining - 4);
                                let pointer = Rc::as_ptr(&reply);
                                let wrong = session.try_accept(&999, reply, &active).unwrap_err();
                                assert_eq!(Rc::as_ptr(&wrong.reply), pointer);
                                assert!(lease.is_open());
                                if stop == Stop::AfterAssembly {
                                    assert!(session.cancel());
                                    drop(wrong);
                                } else {
                                    session
                                        .try_accept(&active.key(90), wrong.reply, &active)
                                        .unwrap();
                                    assert_eq!(session.fuel_remaining(), remaining);
                                    if stop == Stop::AfterAcceptance {
                                        assert!(session.cancel());
                                    }
                                }
                                if matches!(stop, Stop::AfterAssembly | Stop::AfterAcceptance) {
                                    assert!(!lease.is_open());
                                    assert!(matches!(
                                        session.poll(&env).unwrap(),
                                        EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                                    ));
                                    assert_eq!(session.fuel_remaining(), remaining);
                                    assert_eq!(
                                        (
                                            env.captured.restored.get(),
                                            env.captured.projected.get(),
                                            env.captured.rebuilt.get()
                                        ),
                                        (0, 0, 0)
                                    );
                                    assert_eq!(
                                        active.dispatches.get(),
                                        saved
                                            .rows
                                            .iter()
                                            .filter(|r| matches!(r, SavedRow::Prepared(_)))
                                            .count()
                                    );
                                    return None;
                                }
                                match session.poll(&env) {
                                    Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                                        Ok(value)
                                    }
                                    Err(CalculationFailure::External(
                                        EffectStackFault::Control(EffectEvaluationFault::Pure(
                                            PureEvaluationFault::FuelExhausted,
                                        )),
                                    )) => Err(Failure::Fuel),
                                    Err(CalculationFailure::External(
                                        EffectStackFault::Control(EffectEvaluationFault::Native(
                                            error,
                                        )),
                                    )) => Err(Failure::Native(error)),
                                    _ => panic!("bounded return"),
                                }
                            }
                        };
                        assert!(!lease.is_open());
                        assert!(!session.cancel());
                        assert!(env.inventory.borrow().iter().all(Option::is_none));
                        assert_eq!(
                            (
                                first.preparations.get(),
                                second.preparations.get(),
                                active.preparations.get(),
                                first.dispatches.get(),
                                second.dispatches.get()
                            ),
                            (0, 0, 0, 0, 0)
                        );
                        let unpublished = saved
                            .rows
                            .iter()
                            .filter(|r| matches!(r, SavedRow::Prepared(_)))
                            .count();
                        if stop == Stop::DenyCollector {
                            assert_eq!(active.dispatches.get(), 0);
                        } else {
                            assert_eq!(active.dispatches.get(), unpublished);
                        }
                        Some(Report {
                            value,
                            remaining: session.fuel_remaining(),
                            restored: env.captured.restored.get(),
                            projected: env.captured.projected.get(),
                        })
                    }

                    #[test]
                    fn owned_checkpoint_uses_one_meter_from_collectors_through_both_capture_returns()
                     {
                        fn check<T: Projection>() {
                            let mut count = 0;
                            cases(|first, second, active, frontier, order| {
                                let Initial::Saved(bytes) = initial::<T>(
                                    first, second, active, frontier, order, None, true,
                                ) else {
                                    panic!()
                                };
                                let saved = decode::<T>(&bytes, &bytes).unwrap();
                                for borrowed in [false, true] {
                                    assert_eq!(
                                        run(&saved, order, Stop::None, borrowed).unwrap(),
                                        restored(&saved, order, 0).unwrap()
                                    );
                                }
                                count += 1;
                            });
                            assert_eq!(count, 220);
                        }
                        check::<u64>();
                        check::<String>();
                    }
                    #[test]
                    fn owned_checkpoint_all_fuel_cutoffs_match_the_original_manual_reentry() {
                        fn check<T: Projection>() {
                            cases(|first, second, active, frontier, order| {
                                let Initial::Saved(bytes) = initial::<T>(
                                    first, second, active, frontier, order, None, true,
                                ) else {
                                    panic!()
                                };
                                let mut saved = decode::<T>(&bytes, &bytes).unwrap();
                                let needed =
                                    saved.remaining - restored(&saved, order, 0).unwrap().remaining;
                                for allowance in 0..=needed + 1 {
                                    // Explicit test-only reduced grants; never increase the committed budget.
                                    assert!(allowance <= saved.captured.remaining);
                                    saved.remaining = allowance;
                                    for borrowed in [false, true] {
                                        assert_eq!(
                                            run(&saved, order, Stop::None, borrowed).unwrap(),
                                            restored(&saved, order, 0).unwrap(),
                                            "fuel={allowance}, borrowed={borrowed}"
                                        );
                                    }
                                }
                            });
                        }
                        check::<u64>();
                        check::<String>();
                    }
                    #[test]
                    fn owned_checkpoint_cancellation_hands_back_collectors_after_revocation() {
                        fn check<T: Projection>() {
                            cases(|first, second, active, frontier, order| {
                                let Initial::Saved(bytes) = initial::<T>(
                                    first, second, active, frontier, order, None, true,
                                ) else {
                                    panic!()
                                };
                                let saved = decode::<T>(&bytes, &bytes).unwrap();
                                for stop in [
                                    Stop::BeforeCollection,
                                    Stop::AfterAssembly,
                                    Stop::AfterAcceptance,
                                    Stop::DropOwner,
                                ] {
                                    for borrowed in [false, true] {
                                        assert!(run(&saved, order, stop, borrowed).is_none());
                                    }
                                }
                            });
                        }
                        check::<u64>();
                        check::<String>();
                    }
                    #[test]
                    fn owned_checkpoint_live_collector_capture_and_cold_return_denials_close_parent()
                     {
                        cases(|first, second, active, frontier, order| {
                            let Initial::Saved(bytes) = initial::<String>(
                                first, second, active, frontier, order, None, true,
                            ) else {
                                panic!()
                            };
                            let saved = decode::<String>(&bytes, &bytes).unwrap();
                            for stop in [Stop::DenyCollector, Stop::DenyCapture, Stop::DenyOuter] {
                                for borrowed in [false, true] {
                                    let report = run(&saved, order, stop, borrowed).unwrap();
                                    assert!(matches!(report.value, Err(Failure::Native(_))));
                                    assert_eq!(report.projected, 0);
                                    assert!(report.remaining <= saved.remaining);
                                    if stop != Stop::DenyCapture {
                                        assert_eq!(report.restored, 0);
                                    }
                                }
                            }
                        });
                    }

                    mod successor {
                        use super::*;
                        use leselang_hir::effect_session::CorrelatedEffectRequest;

                        // Both adapters use the existing evaluator. The fourth host only
                        // receives work selected by a restored source frame, never by the codec.
                        struct Reentry<'r, 'a, T> {
                            restored: &'r Adapter<'a, T>,
                            next: &'r Adapter<'a, T>,
                            domain: &'a Root<'a>,
                            correlated: Cell<usize>,
                        }
                        impl<'a, T: Projection> PureEvaluationEnvironment<(), Operation> for Reentry<'_, 'a, T> {
                            type Result = View<'a, T>;
                            type Error = &'static str;
                            fn field(
                                &self,
                                value: &Self::Result,
                                field: &(),
                            ) -> Result<ScalarValue, Self::Error> {
                                self.restored.field(value, field)
                            }
                            fn member(
                                &self,
                                value: &Self::Result,
                                name: &str,
                                op: &Operation,
                            ) -> Result<Self::Result, Self::Error> {
                                self.restored.member(value, name, op)
                            }
                        }
                        impl<'a, T: Projection>
                            EffectEvaluationEnvironment<'a, (), Operation, (), usize>
                            for Reentry<'_, 'a, T>
                        {
                            type Request = (&'a Node, Work<'a, T>);
                            type Capture = Capture<'a, T>;
                            fn preflight_effect(&self, node: &'a Node) -> Result<(), Self::Error> {
                                if std::ptr::eq(node, self.domain.node) {
                                    self.next.preflight_effect(node)
                                } else {
                                    self.restored.preflight_effect(node)
                                }
                            }
                            fn prepare_effect(
                                &self,
                                node: &'a Node,
                                scope: &mut ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                                fuel: &mut Fuel,
                            ) -> Result<Self::Request, CalculationFailure<Self::Error>>
                            {
                                if !std::ptr::eq(node, self.domain.node) {
                                    return Err("unexpected successor".into());
                                }
                                self.next
                                    .prepare_effect(node, scope, fuel)
                                    .map(|work| (node, work))
                            }
                            fn capture(
                                &self,
                                name: &'a str,
                                body: &'a Node,
                                scope: &ScopeFrame<'_, 'a, PureValue<Self::Result>>,
                                fuel: &mut Fuel,
                            ) -> Result<Self::Capture, Self::Error> {
                                self.restored.capture(name, body, scope, fuel)
                            }
                        }
                        impl<'a, T: Projection>
                            EffectReentryEnvironment<
                                'a,
                                (),
                                Operation,
                                (),
                                usize,
                                u64,
                                Root<'a>,
                                Rc<Group<'a, T>>,
                            > for Reentry<'_, 'a, T>
                        {
                            fn restore_capture(
                                &self,
                                id: &u64,
                                domain: &Root<'a>,
                                capture: Self::Capture,
                                fuel: &mut Fuel,
                                limits: EffectEvaluationLimits,
                            ) -> Result<
                                RestoredEffectBindings<'a, Self::Result>,
                                CalculationFailure<Self::Error>,
                            > {
                                self.restored
                                    .restore_capture(id, domain, capture, fuel, limits)
                            }
                            fn bind_reply(
                                &self,
                                id: &u64,
                                domain: &Root<'a>,
                                reply: Rc<Group<'a, T>>,
                                fuel: &mut Fuel,
                            ) -> Result<PureValue<Self::Result>, CalculationFailure<Self::Error>>
                            {
                                if std::ptr::eq(domain, self.domain) {
                                    self.next.bind_reply(id, domain, reply, fuel)
                                } else {
                                    self.restored.bind_reply(id, domain, reply, fuel)
                                }
                            }
                        }
                        impl<'a, T: Projection>
                            EffectSessionEnvironment<'a, 'a, (), Operation, (), usize>
                            for Reentry<'_, 'a, T>
                        {
                            type Identity = u64;
                            type Declaration = Root<'a>;
                            type Reply = Rc<Group<'a, T>>;
                            type Dispatch = Work<'a, T>;
                            fn correlate_request(
                                &self,
                                (node, work): Self::Request,
                                _: &mut Fuel,
                            ) -> EffectCorrelation<'a, u64, Root<'a>, Self::Dispatch, Self::Error>
                            {
                                self.next.active.authorize(&self.next.active.key(90))?;
                                if !std::ptr::eq(node, self.domain.node)
                                    || !std::ptr::eq(self.domain.host, self.next.active)
                                    || self.correlated.get() != 0
                                {
                                    return Err("successor correspondence".into());
                                }
                                self.correlated.set(1);
                                Ok(CorrelatedEffectRequest {
                                    identity: self.next.active.key(90),
                                    declaration: self.domain,
                                    dispatch: work,
                                })
                            }
                        }

                        fn checkpoint<T: Projection>(
                            first: GroupKind,
                            second: GroupKind,
                            active: GroupKind,
                            frontier: usize,
                            order: [usize; 3],
                            next: GroupKind,
                        ) -> Vec<u8> {
                            let text = program::<T>(first, second, active);
                            let tail = format!(
                                "{}(m0: operation0(value: field(value: seed, name: \"value\")), \
                                m1: operation1(value: field(value: member(value: second_alias, name: \"m1\"), name: \"value\")), \
                                m2: operation2(value: output))",
                                if next == GroupKind::Sequence {
                                    "seq"
                                } else {
                                    "all"
                                },
                            );
                            let final_body =
                                "choose(when: false, then: operation9(), otherwise: output)";
                            assert_eq!(text.matches(final_body).count(), 1);
                            let text = text.replace(final_body, &tail);
                            let tree = source::compile_model_with_result::<T>(
                                &text,
                                None,
                                31,
                                &[7, 8],
                                12,
                                4,
                                Some(next),
                            )
                            .unwrap();
                            let Initial::Saved(bytes) =
                                initial_tree::<T>(tree, 4, active, frontier, order, None, true)
                            else {
                                panic!("source checkpoint")
                            };
                            bytes
                        }

                        #[derive(Clone, Copy, PartialEq)]
                        enum Stop {
                            None,
                            BeforeReentry,
                            AfterRequest,
                            CancelRequest,
                        }
                        #[derive(Debug, PartialEq)]
                        struct Outcome {
                            values: Option<Vec<ScalarValue>>,
                            remaining: u64,
                            resumed: usize,
                            dispatched: usize,
                            end: EffectSessionEnd,
                        }
                        fn run<T: Projection>(
                            saved: &Saved<T>,
                            order: [usize; 3],
                            borrowed: bool,
                            stop: Stop,
                        ) -> Outcome {
                            let hosts: Vec<_> = (1..=4)
                                .map(|generation| {
                                    let host = Host::new(3);
                                    host.generation.set(generation);
                                    host
                                })
                                .collect();
                            let scope = LocalExecutionScope::new();
                            let lease = scope.lease();
                            for host in &hosts {
                                host.lease.replace(Some(lease.clone()));
                            }
                            let parameters = [NamedParameter::required(
                                "value",
                                ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                            )];
                            let next_node =
                                group_sites_with_count(&saved.frames.last().unwrap().body, 1)[0];
                            let current_schemas = schemas::<T>(&hosts[2], &parameters, &saved.tree);
                            let next_schemas = schemas::<T>(&hosts[3], &parameters, next_node);
                            let mut current = adapter(
                                &hosts[0],
                                &hosts[2],
                                &current_schemas,
                                Some(&saved.captured),
                            );
                            configure(&mut current, &hosts[1], Some(&saved.additional[0]));
                            let mut next = adapter(&hosts[0], &hosts[3], &next_schemas, None);
                            configure(&mut next, &hosts[1], None);
                            let current_domain = Root {
                                host: &hosts[2],
                                node: &saved.tree,
                            };
                            let next_domain = Root {
                                host: &hosts[3],
                                node: next_node,
                            };
                            let env = Reentry {
                                restored: &current,
                                next: &next,
                                domain: &next_domain,
                                correlated: Cell::new(0),
                            };
                            let mut session = Session::restore_pending(
                                scope, hosts[2].key(90), &current_domain,
                                saved.frames.iter().map(|frame| SavedEffectReturnFrame {
                                    name: &frame.name, body: &frame.body, capture: Capture::Saved(frame),
                                }).collect(),
                                Fuel::new(saved.remaining),
                                |id, domain, frames, fuel| {
                                    hosts[2].authorize(id)?;
                                    if *id != hosts[2].key(90) || !std::ptr::eq(domain, &current_domain)
                                        || fuel.remaining() != saved.remaining || frames.len() != saved.frames.len()
                                        || !frames.iter().zip(&saved.frames).all(|(frame, original)| {
                                            frame.name == original.name && std::ptr::eq(frame.body, &original.body)
                                                && matches!(frame.capture, Capture::Saved(value) if std::ptr::eq(value, original))
                                        })
                                    { return Err("checkpoint correspondence"); }
                                    Ok(())
                                }, borrowed,
                            ).unwrap();
                            let mut values = None;
                            let mut execute = || {
                                assert!(matches!(
                                    session.poll(&env).unwrap(),
                                    EffectSessionPoll::Awaiting
                                ));
                                let Ok(reply) =
                                    session.with_metered_host_outcome((), |(), fuel| {
                                        let result = Collector::restore(saved, &current)
                                            .and_then(|collector| {
                                                collector.complete(&current, order)
                                            })
                                            .and_then(|rows| {
                                                assemble_group_reply(
                                                    &saved.tree,
                                                    rows,
                                                    QUEUE_LIMITS,
                                                    fuel,
                                                    |group, _| admit(group, &hosts[2]),
                                                )
                                                .map(Rc::new)
                                                .map_err(|_| "assembly")
                                            });
                                        if result.is_ok() {
                                            HostWorkOutcome::Pending(result)
                                        } else {
                                            HostWorkOutcome::Failed(result)
                                        }
                                    })
                                else {
                                    panic!("restored meter")
                                };
                                let Ok(reply) = reply else { return };
                                let stale = reply.clone();
                                session
                                    .try_accept(&hosts[2].key(90), reply, &hosts[2])
                                    .unwrap();
                                if stop == Stop::BeforeReentry {
                                    hosts[3].granted.set(false);
                                }
                                let work = match session.poll(&env) {
                                    Ok(EffectSessionPoll::Request(work)) => work,
                                    Err(_) => return,
                                    _ => panic!("restored source must select its fourth group"),
                                };
                                assert_eq!(
                                    (
                                        env.correlated.get(),
                                        hosts[3].preparations.get(),
                                        hosts[3].dispatches.get()
                                    ),
                                    (1, 3, 0)
                                );
                                assert_eq!(
                                    (
                                        current.captured.restored.get(),
                                        current.captured.rebuilt.get()
                                    ),
                                    (2, 2)
                                );
                                let fuel = session.fuel_remaining();
                                let pointer = Rc::as_ptr(&stale);
                                let refused = session
                                    .try_accept(&hosts[2].key(90), stale, &hosts[3])
                                    .unwrap_err();
                                assert_eq!(Rc::as_ptr(&refused.reply), pointer);
                                // Even with the current identity, an old group's domain is not a new receipt.
                                let refused = session
                                    .try_accept(&hosts[3].key(90), refused.reply, &hosts[3])
                                    .unwrap_err();
                                assert_eq!(Rc::as_ptr(&refused.reply), pointer);
                                assert_eq!(session.fuel_remaining(), fuel);
                                assert!(matches!(
                                    session.poll(&env).unwrap(),
                                    EffectSessionPoll::Awaiting
                                ));
                                if stop == Stop::CancelRequest {
                                    let Ok(work) =
                                        session.with_host_outcome(work, HostWorkOutcome::Cancelled)
                                    else {
                                        panic!()
                                    };
                                    assert!(!lease.is_open());
                                    drop(work);
                                    return;
                                }
                                if stop == Stop::AfterRequest {
                                    hosts[3].granted.set(false);
                                }
                                let Ok(reply) =
                                    session.with_metered_host_outcome(work, |work, fuel| {
                                        let result =
                                            hosts[3].authorize(&hosts[3].key(90)).and_then(|()| {
                                                let rows = collect_group(&next, work);
                                                assemble_group_reply(
                                                    next_node,
                                                    rows,
                                                    QUEUE_LIMITS,
                                                    fuel,
                                                    |group, _| admit(group, &hosts[3]),
                                                )
                                                .map(Rc::new)
                                                .map_err(|_| "successor assembly")
                                            });
                                        if result.is_ok() {
                                            HostWorkOutcome::Pending(result)
                                        } else {
                                            HostWorkOutcome::Failed(result)
                                        }
                                    })
                                else {
                                    panic!("successor meter")
                                };
                                let Ok(reply) = reply else { return };
                                session
                                    .try_accept(&hosts[3].key(90), reply, &hosts[3])
                                    .unwrap();
                                match session.poll(&env) {
                                    Ok(EffectSessionPoll::Value(PureValue::Result(
                                        View::Group(group),
                                    ))) => {
                                        assert!(std::ptr::eq(group.group(), next_node));
                                        values = Some(
                                            group
                                                .members()
                                                .iter()
                                                .map(|row| row.as_parts().4.data.scalar())
                                                .collect(),
                                        );
                                    }
                                    Err(_) => (),
                                    _ => panic!("native final reply"),
                                }
                            };
                            execute();
                            assert!(!lease.is_open());
                            let EffectSessionStatus::Terminal(end) = session.status() else {
                                panic!("terminal owner")
                            };
                            let remaining = session.fuel_remaining();
                            let counts: Vec<_> = hosts
                                .iter()
                                .map(|host| (host.preparations.get(), host.dispatches.get()))
                                .collect();
                            assert_eq!(&counts[..2], &[(0, 0), (0, 0)]);
                            assert_eq!(counts[2].0, 0);
                            assert_eq!(
                                counts[2].1,
                                saved
                                    .rows
                                    .iter()
                                    .filter(|row| matches!(row, SavedRow::Prepared(_)))
                                    .count()
                            );
                            assert!(
                                matches!(session.poll(&env).unwrap(), EffectSessionPoll::Terminal(value) if value == end)
                            );
                            assert!(
                                session
                                    .with_host_outcome((), |()| -> HostWorkOutcome<()> {
                                        panic!("sealed work")
                                    })
                                    .is_err()
                            );
                            assert_eq!(session.fuel_remaining(), remaining);
                            assert_eq!(
                                counts,
                                hosts
                                    .iter()
                                    .map(|host| (host.preparations.get(), host.dispatches.get()))
                                    .collect::<Vec<_>>()
                            );
                            Outcome {
                                values,
                                remaining,
                                resumed: env.correlated.get(),
                                dispatched: hosts[3].dispatches.get(),
                                end,
                            }
                        }

                        #[test]
                        fn decoded_source_frames_continue_to_a_distinct_host_without_replaying_settled_groups()
                         {
                            fn check<T: Projection>() {
                                let mut count = 0;
                                cases(|first, second, active, frontier, order| {
                                    for next in KINDS {
                                        let bytes = checkpoint::<T>(
                                            first, second, active, frontier, order, next,
                                        );
                                        let saved = decode::<T>(&bytes, &bytes).unwrap();
                                        let owned = run(&saved, order, false, Stop::None);
                                        assert_eq!(owned, run(&saved, order, true, Stop::None));
                                        assert_eq!(
                                            owned.values,
                                            Some((0..3).map(|i| T::sample(i).scalar()).collect())
                                        );
                                        assert_eq!(
                                            (owned.resumed, owned.dispatched, owned.end),
                                            (1, 3, EffectSessionEnd::Completed)
                                        );
                                        count += 1;
                                    }
                                });
                                assert_eq!(count, 440);
                            }
                            check::<u64>();
                            check::<String>();
                        }
                        #[test]
                        fn decoded_successors_share_the_checkpoint_meter_at_every_cutoff() {
                            fn check<T: Projection>() {
                                for active in KINDS {
                                    for next in KINDS {
                                        let bytes = checkpoint::<T>(
                                            GroupKind::Sequence,
                                            GroupKind::Parallel,
                                            active,
                                            1,
                                            [2, 0, 1],
                                            next,
                                        );
                                        let mut saved = decode::<T>(&bytes, &bytes).unwrap();
                                        let used = saved.remaining
                                            - run(&saved, [2, 0, 1], false, Stop::None).remaining;
                                        for budget in 0..=used + 1 {
                                            assert!(budget <= saved.captured.remaining);
                                            saved.remaining = budget; // Test-only reduced grant, never a refill.
                                            let result = run(&saved, [2, 0, 1], false, Stop::None);
                                            assert_eq!(
                                                result,
                                                run(&saved, [2, 0, 1], true, Stop::None)
                                            );
                                            assert_eq!(result.values.is_some(), budget >= used);
                                            assert_eq!(
                                                result.end,
                                                if budget >= used {
                                                    EffectSessionEnd::Completed
                                                } else {
                                                    EffectSessionEnd::Failed
                                                }
                                            );
                                        }
                                    }
                                }
                            }
                            check::<u64>();
                            check::<String>();
                        }
                        #[test]
                        fn decoded_successor_revocation_and_cancellation_stop_before_new_dispatch()
                        {
                            fn check<T: Projection>() {
                                for active in KINDS {
                                    for next in KINDS {
                                        let bytes = checkpoint::<T>(
                                            GroupKind::Parallel,
                                            GroupKind::Sequence,
                                            active,
                                            1,
                                            [1, 2, 0],
                                            next,
                                        );
                                        let saved = decode::<T>(&bytes, &bytes).unwrap();
                                        for stop in [
                                            Stop::BeforeReentry,
                                            Stop::AfterRequest,
                                            Stop::CancelRequest,
                                        ] {
                                            for borrowed in [false, true] {
                                                let result = run(&saved, [1, 2, 0], borrowed, stop);
                                                assert!(result.values.is_none());
                                                assert_eq!(result.dispatched, 0);
                                                assert_eq!(
                                                    result.resumed,
                                                    usize::from(stop != Stop::BeforeReentry)
                                                );
                                                assert_eq!(
                                                    result.end,
                                                    if stop == Stop::CancelRequest {
                                                        EffectSessionEnd::Cancelled
                                                    } else {
                                                        EffectSessionEnd::Failed
                                                    }
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            check::<u64>();
                            check::<String>();
                        }
                    }
                }

                fn cases(mut run: impl FnMut(GroupKind, GroupKind, GroupKind, usize, [usize; 3])) {
                    for first in KINDS {
                        source::cases(|second, active, frontier, order| {
                            run(first, second, active, frontier, order)
                        });
                    }
                }
                #[test]
                fn two_source_captures_restore_distinct_graphs_and_shared_aliases() {
                    fn run<T: Projection>() {
                        let mut count = 0;
                        cases(|first, second, active, frontier, order| {
                            let Initial::Finished(expected) =
                                initial::<T>(first, second, active, frontier, order, None, false)
                            else {
                                panic!()
                            };
                            let Initial::Saved(bytes) =
                                initial::<T>(first, second, active, frontier, order, None, true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            let actual = restored(&saved, order, 0).unwrap();
                            assert_eq!(actual, expected);
                            assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                            assert_eq!((actual.restored, actual.projected), (2, 1));
                            count += 1;
                        });
                        assert_eq!(count, 220);
                    }
                    run::<u64>();
                    run::<String>();
                }
                #[test]
                fn two_source_captures_match_every_pending_assembly_and_return_fuel_cutoff() {
                    fn run<T: Projection>() {
                        cases(|first, second, active, frontier, order| {
                            let Initial::Saved(bytes) =
                                initial::<T>(first, second, active, frontier, order, None, true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            let needed =
                                saved.remaining - restored(&saved, order, 0).unwrap().remaining;
                            for allowance in 0..=needed + 1 {
                                let Initial::Finished(expected) = initial::<T>(
                                    first,
                                    second,
                                    active,
                                    frontier,
                                    order,
                                    Some(allowance),
                                    false,
                                ) else {
                                    panic!()
                                };
                                let Initial::Saved(bytes) = initial::<T>(
                                    first,
                                    second,
                                    active,
                                    frontier,
                                    order,
                                    Some(allowance),
                                    true,
                                ) else {
                                    panic!()
                                };
                                let saved = decode::<T>(&bytes, &bytes).unwrap();
                                let actual = restored(&saved, order, 0).unwrap();
                                assert_eq!(
                                    actual, expected,
                                    "{first:?}/{second:?}/{active:?}/{frontier}/{order:?}/{allowance}"
                                );
                                assert_eq!(
                                    actual.value,
                                    if allowance < needed {
                                        Err(Failure::Fuel)
                                    } else {
                                        Ok(T::sample(2).scalar())
                                    }
                                );
                            }
                        });
                    }
                    run::<u64>();
                    run::<String>();
                }
                #[test]
                fn two_source_captures_recheck_each_group_and_both_return_authorities() {
                    cases(|first, second, active, frontier, order| {
                        let Initial::Saved(bytes) =
                            initial::<String>(first, second, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let saved = decode::<String>(&bytes, &bytes).unwrap();
                        for mode in [1, 2] {
                            assert!(restored(&saved, order, mode).is_err());
                        }
                        let cold = restored(&saved, order, 3).unwrap();
                        assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                        assert_eq!(
                            (cold.remaining, cold.restored, cold.projected),
                            (saved.remaining - 4, 0, 0)
                        );
                        for mode in [4, 5, 6, 7] {
                            let result = restored(&saved, order, mode).unwrap();
                            assert_eq!(result.value, Err(Failure::Native("domain")));
                            assert_eq!(
                                (result.restored, result.projected),
                                if mode <= 5 { (2, 1) } else { (1, 0) }
                            );
                        }
                    });
                }
                #[test]
                fn two_cached_groups_remain_separate_and_revalidate_policy_on_every_alias() {
                    let Initial::Saved(bytes) = initial::<String>(
                        GroupKind::Sequence,
                        GroupKind::Sequence,
                        GroupKind::Sequence,
                        0,
                        [0, 1, 2],
                        None,
                        true,
                    ) else {
                        panic!()
                    };
                    let saved = decode::<String>(&bytes, &bytes).unwrap();
                    let first = Host::new(3);
                    let second = Host::new(3);
                    second.generation.set(2);
                    let active = Host::new(3);
                    active.generation.set(3);
                    let parameters = [NamedParameter::required(
                        "value",
                        ScalarTypeSet::only(String::sample(0).scalar().scalar_type()),
                    )];
                    let schemas = schemas::<String>(&active, &parameters, &saved.tree);
                    let mut env = adapter(&first, &active, &schemas, Some(&saved.captured));
                    configure(&mut env, &second, Some(&saved.additional[0]));
                    let groups = [
                        env.captured.saved_group(0).unwrap(),
                        env.captured.saved_group(1).unwrap(),
                    ];
                    assert!(!Rc::ptr_eq(&groups[0], &groups[1]));
                    for (index, host) in [&first, &second].into_iter().enumerate() {
                        assert!(Rc::ptr_eq(
                            &groups[index],
                            &env.captured.saved_group(index).unwrap()
                        ));
                        host.granted.set(false);
                        assert!(env.captured.saved_group(index).is_err());
                        assert!(env.captured.saved_group(1 - index).is_ok());
                        host.granted.set(true);
                        host.domains[1].denied.set(true);
                        assert!(env.captured.saved_group(index).is_err());
                        host.domains[1].denied.set(false);
                        assert!(Rc::ptr_eq(
                            &groups[index],
                            &env.captured.saved_group(index).unwrap()
                        ));
                    }
                    assert!(env.captured.saved_group(2).is_err());
                    assert_eq!(
                        (
                            env.captured.rebuilt.get(),
                            first.preparations.get(),
                            second.preparations.get(),
                            active.dispatches.get()
                        ),
                        (2, 0, 0, 0)
                    );
                }
                #[test]
                fn multi_capture_source_rejects_missing_aliases_and_insufficient_graph_budgets() {
                    for first in KINDS {
                        for second in KINDS {
                            for active in KINDS {
                                let text = program::<String>(first, second, active);
                                assert!(compile::<String>(&text).is_ok());
                                for bad in [
                                    text.replace("value: second_alias", "value: absent"),
                                    text.replace("second: seed_second()", "second: missing()"),
                                    text.replace("operation9()", "operation9(value: 0)"),
                                    text.replace("name: \"m1\"", "name: \"missing\""),
                                ] {
                                    assert_ne!(bad, text);
                                    assert!(compile::<String>(&bad).is_err());
                                }
                                assert!(
                                    source::compile_model_with_budget::<String>(
                                        &text,
                                        None,
                                        31,
                                        &[7, 8],
                                        9,
                                        3
                                    )
                                    .is_err()
                                );
                                assert!(
                                    source::compile_model_with_budget::<String>(
                                        &text,
                                        None,
                                        31,
                                        &[7, 8],
                                        10,
                                        2
                                    )
                                    .is_err()
                                );
                                assert!(
                                    source::compile_model_with_budget::<String>(
                                        &text,
                                        None,
                                        31,
                                        &[7],
                                        10,
                                        3
                                    )
                                    .is_err()
                                );
                            }
                        }
                    }
                }
                #[test]
                fn multi_capture_checkpoint_rejects_cross_graph_refs_owners_and_budget_changes() {
                    for active in KINDS {
                        let Initial::Saved(bytes) = initial::<String>(
                            GroupKind::Sequence,
                            GroupKind::Parallel,
                            active,
                            0,
                            [0, 1, 2],
                            None,
                            true,
                        ) else {
                            panic!()
                        };
                        for case in 0..8 {
                            let mut saved = decode::<String>(&bytes, &bytes).unwrap();
                            match case {
                                0 => saved.frames[0].groups[3].group = 0,
                                1 => saved.frames[0].groups[3].group = 2,
                                2 => saved.additional[0].rows[0].0 = 100,
                                3 => saved.additional.clear(),
                                4 => saved.additional[0].assembled = false,
                                5 => saved.additional[0].remaining = 1001,
                                6 => saved.remaining = saved.additional[0].remaining + 1,
                                _ => saved.frames[1].groups.swap(0, 2),
                            }
                            let candidate = serde_json::to_vec(&saved).unwrap();
                            assert_ne!(candidate, bytes);
                            assert!(decode::<String>(&candidate, &bytes).is_err());
                            assert!(decode::<String>(&candidate, &candidate).is_err());
                        }
                        assert!(decode::<String>(&vec![b' '; 64 * 1024 + 1], &bytes).is_err());
                    }
                }
            }

            mod repeat {
                use super::*;

                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                enum Form {
                    Sequence,
                    Parallel,
                    Repeat,
                    TailRepeat,
                }
                const FORMS: [Form; 4] = [
                    Form::Sequence,
                    Form::Parallel,
                    Form::Repeat,
                    Form::TailRepeat,
                ];
                impl Form {
                    fn kind(self) -> GroupKind {
                        if self == Self::Parallel {
                            GroupKind::Parallel
                        } else {
                            GroupKind::Sequence
                        }
                    }
                    fn repeats(self) -> bool {
                        matches!(self, Self::Repeat | Self::TailRepeat)
                    }
                    fn names(self) -> [&'static str; 3] {
                        match self {
                            Self::Repeat => ["iteration_1", "iteration_2", "iteration_3"],
                            Self::TailRepeat => ["head", "tail__iteration_1", "tail__iteration_2"],
                            _ => ["m0", "m1", "m2"],
                        }
                    }
                    fn operations(self) -> [usize; 3] {
                        match self {
                            Self::Repeat => [0, 0, 0],
                            Self::TailRepeat => [0, 1, 1],
                            _ => [0, 1, 2],
                        }
                    }
                    fn expression(self, argument: impl Fn(usize) -> String) -> String {
                        let call = |i| format!("operation{i}(value: {})", argument(i));
                        match self {
                            Self::Repeat => format!("repeat(times: 3, body: {})", call(0)),
                            Self::TailRepeat => format!(
                                "seq(head: {}, tail: repeat(times: 2, body: {}))",
                                call(0),
                                call(1)
                            ),
                            _ => format!(
                                "{}(m0: {}, m1: {}, m2: {})",
                                if self == Self::Parallel { "all" } else { "seq" },
                                call(0),
                                call(1),
                                call(2)
                            ),
                        }
                    }
                }
                fn program<T: Projection>(captured: Form, active: Form) -> String {
                    let first = captured.expression(|i| match T::sample(i).scalar() {
                        ScalarValue::Integer(v) => v.to_string(),
                        ScalarValue::String(v) => serde_json::to_string(&v).unwrap(),
                        _ => panic!("fixture payload"),
                    });
                    let names = captured.names();
                    let second = active.expression(|i| {
                        format!(
                            "field(value: member(value: alias, name: \"{}\"), name: \"value\")",
                            names[i]
                        )
                    });
                    let tail = source::program_with_group::<T>(&second, active.names()[2]);
                    let tail = tail.strip_prefix("fn main() = ").unwrap();
                    format!(
                        "fn seed_group() = {first}\nfn main() = bind(batch: seed_group(), body: bind(alias: batch, body: bind(keep: true, body: bind(seed: member(value: batch, name: \"{}\"), body: {tail}))))",
                        names[0]
                    )
                }
                fn cases(mut run: impl FnMut(Form, Form, usize, [usize; 3])) {
                    for captured in FORMS {
                        for active in FORMS {
                            if !captured.repeats() && !active.repeats() {
                                continue;
                            }
                            if active == Form::Parallel {
                                for mask in 0..8 {
                                    for order in parallel::ORDERS {
                                        run(captured, active, mask, order);
                                    }
                                }
                            } else {
                                for frontier in 0..7 {
                                    run(captured, active, frontier, [0, 1, 2]);
                                }
                            }
                        }
                    }
                }
                fn initial<T: Projection>(
                    captured: Form,
                    active: Form,
                    frontier: usize,
                    order: [usize; 3],
                    allowance: Option<u64>,
                    save: bool,
                ) -> Initial {
                    initial_source::<T>(
                        &program::<T>(captured, active),
                        active.kind(),
                        frontier,
                        order,
                        allowance,
                        save,
                    )
                }
                fn check_shape(node: &Node, form: Form) {
                    let Node::Group {
                        group_kind,
                        branches,
                    } = node
                    else {
                        panic!()
                    };
                    assert_eq!(*group_kind, form.kind());
                    assert_eq!(
                        branches.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
                        form.names()
                    );
                    for (branch, operation) in branches.iter().zip(form.operations()) {
                        assert_eq!(branch_operation(branch), &op(operation));
                        assert_eq!(branch.result_type, operation);
                    }
                }
                #[test]
                fn repeated_source_groups_restore_expanded_names_operations_and_original_returns() {
                    fn run<T: Projection>() {
                        let mut count = 0;
                        cases(|captured, active, frontier, order| {
                            let Initial::Finished(expected) =
                                initial::<T>(captured, active, frontier, order, None, false)
                            else {
                                panic!()
                            };
                            let Initial::Saved(bytes) =
                                initial::<T>(captured, active, frontier, order, None, true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            check_shape(&saved.captured.node, captured);
                            check_shape(&saved.tree, active);
                            let actual =
                                source::resume(&saved, active.kind(), frontier, order, 0).unwrap();
                            assert_eq!(actual, expected);
                            assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                            assert_eq!((actual.restored, actual.projected), (2, 1));
                            count += 1;
                        });
                        assert_eq!(count, 166);
                    }
                    run::<u64>();
                    run::<String>();
                }
                #[test]
                fn repeated_source_recovery_preserves_every_assembly_and_return_fuel_cutoff() {
                    fn run<T: Projection>() {
                        cases(|captured, active, frontier, order| {
                            let Initial::Saved(bytes) =
                                initial::<T>(captured, active, frontier, order, None, true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            let needed = saved.remaining
                                - source::resume(&saved, active.kind(), frontier, order, 0)
                                    .unwrap()
                                    .remaining;
                            for allowance in 0..=needed + 1 {
                                let Initial::Finished(expected) = initial::<T>(
                                    captured,
                                    active,
                                    frontier,
                                    order,
                                    Some(allowance),
                                    false,
                                ) else {
                                    panic!()
                                };
                                let Initial::Saved(bytes) = initial::<T>(
                                    captured,
                                    active,
                                    frontier,
                                    order,
                                    Some(allowance),
                                    true,
                                ) else {
                                    panic!()
                                };
                                let saved = decode::<T>(&bytes, &bytes).unwrap();
                                let actual =
                                    source::resume(&saved, active.kind(), frontier, order, 0)
                                        .unwrap();
                                assert_eq!(
                                    actual, expected,
                                    "{captured:?}/{active:?}/{frontier}/{order:?}/{allowance}"
                                );
                                assert_eq!(
                                    actual.value,
                                    if allowance < needed {
                                        Err(Failure::Fuel)
                                    } else {
                                        Ok(T::sample(2).scalar())
                                    }
                                );
                            }
                        });
                    }
                    run::<u64>();
                    run::<String>();
                }
                #[test]
                fn repeated_source_bounds_and_cold_authority_fail_before_native_preparation() {
                    let mut count = 0;
                    for captured in FORMS {
                        for active in FORMS {
                            if !captured.repeats() && !active.repeats() {
                                continue;
                            }
                            let text = program::<String>(captured, active);
                            assert!(
                                source::compile_model::<String>(&text, None, 31, &[7, 8]).is_ok()
                            );
                            let bounds = |value: &str| {
                                text.replace("times: 3", &format!("times: {value}"))
                                    .replace("times: 2", &format!("times: {value}"))
                            };
                            for invalid in [
                                bounds("0"),
                                bounds("-1"),
                                bounds("4"),
                                bounds("add(left: 1, right: 2)"),
                                text.replace("times:", "count:"),
                                format!("{text}\nfn unused() = missing()"),
                                text.replace("operation9()", "operation9(value: 0)"),
                            ] {
                                assert_ne!(invalid, text);
                                assert!(
                                    source::compile_model::<String>(&invalid, None, 31, &[7, 8])
                                        .is_err(),
                                    "{invalid}"
                                );
                                count += 1;
                            }
                            assert!(
                                source::compile_model::<String>(&text, None, 30, &[7, 8]).is_err()
                            );
                            assert!(
                                source::compile_model::<String>(&text, None, 31, &[7]).is_err()
                            );
                            assert!(source::compile_model::<String>(&text, None, 31, &[]).is_err());
                            count += 3;
                        }
                    }
                    assert_eq!(count, 120);
                }
                #[test]
                fn repeated_source_recovery_rechecks_current_cold_and_second_return_authority() {
                    cases(|captured, active, frontier, order| {
                        let Initial::Saved(bytes) =
                            initial::<String>(captured, active, frontier, order, None, true)
                        else {
                            panic!()
                        };
                        let saved = decode::<String>(&bytes, &bytes).unwrap();
                        assert!(source::resume(&saved, active.kind(), frontier, order, 1).is_err());
                        assert!(source::resume(&saved, active.kind(), frontier, order, 2).is_err());
                        let cold =
                            source::resume(&saved, active.kind(), frontier, order, 3).unwrap();
                        assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                        assert_eq!(
                            (cold.remaining, cold.restored, cold.projected),
                            (saved.remaining - 4, 0, 0)
                        );
                        let late =
                            source::resume(&saved, active.kind(), frontier, order, 4).unwrap();
                        assert!(late.value.is_err());
                        assert_eq!((late.restored, late.projected), (2, 1));
                    });
                }
                #[test]
                fn identical_operations_cannot_exchange_member_receipts_after_restore() {
                    fn run<T: Projection>() {
                        for (form, current, other) in
                            [(Form::Repeat, 0, 1), (Form::TailRepeat, 1, 2)]
                        {
                            let frontier = current * 2 + 1;
                            let Initial::Saved(bytes) =
                                initial::<T>(Form::Repeat, form, frontier, [0, 1, 2], None, true)
                            else {
                                panic!()
                            };
                            let saved = decode::<T>(&bytes, &bytes).unwrap();
                            let host = Host::new(3);
                            let active = Host::new(3);
                            active.generation.set(2);
                            let parameters = [NamedParameter::required(
                                "value",
                                ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                            )];
                            let schemas = schemas::<T>(&active, &parameters, &saved.tree);
                            assert_eq!(schemas[current].key, schemas[other].key);
                            let env = adapter(&host, &active, &schemas, Some(&saved.captured));
                            let (mut queue, mut rows, inbox) =
                                restored_queue(&saved, &env).unwrap();
                            let inbox = inbox.unwrap();
                            let original = &*inbox.data as *const T;
                            let status = queue.status();
                            let wrong = queue
                                .try_accept::<_, T, _>(&active.key(other), inbox, &active)
                                .unwrap_err();
                            assert_eq!(&*wrong.reply.data as *const T, original);
                            assert_eq!(queue.status(), status);
                            let foreign = input(other, &env.captured.drops);
                            let foreign_ptr = &*foreign.data as *const T;
                            let denied = queue
                                .try_accept::<_, T, _>(&active.key(current), foreign, &active)
                                .unwrap_err();
                            assert_eq!(&*denied.reply.data as *const T, foreign_ptr);
                            assert_eq!(queue.status(), status);
                            rows.push(
                                queue
                                    .try_accept::<_, T, _>(
                                        &active.key(current),
                                        wrong.reply,
                                        &active,
                                    )
                                    .unwrap(),
                            );
                            let status = queue.status();
                            let duplicate = input(current, &env.captured.drops);
                            let duplicate_ptr = &*duplicate.data as *const T;
                            let rejected = queue
                                .try_accept::<_, T, _>(&active.key(current), duplicate, &active)
                                .unwrap_err();
                            assert_eq!(&*rejected.reply.data as *const T, duplicate_ptr);
                            assert_eq!(queue.status(), status);
                            assert_eq!(
                                (
                                    host.dispatches.get(),
                                    active.dispatches.get(),
                                    env.captured.restored.get()
                                ),
                                (0, 0, 0)
                            );
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
                            .unwrap();
                            let actual = finish(
                                &env,
                                &saved.tree,
                                queue,
                                rows,
                                None,
                                continuation,
                                &mut Fuel::new(saved.remaining),
                            )
                            .unwrap();
                            assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                            assert_eq!(
                                (
                                    host.preparations.get(),
                                    active.preparations.get(),
                                    host.dispatches.get(),
                                    active.dispatches.get()
                                ),
                                (0, 0, 0, 2 - current)
                            );
                        }
                    }
                    run::<u64>();
                    run::<String>();
                }
                #[test]
                fn repeated_source_forged_expansion_operands_and_graphs_do_not_enter_recovery() {
                    for form in [Form::Repeat, Form::TailRepeat] {
                        let Initial::Saved(bytes) =
                            initial::<String>(Form::Repeat, form, 0, [0, 1, 2], None, true)
                        else {
                            panic!()
                        };
                        for case in 0..8 {
                            let mut saved = decode::<String>(&bytes, &bytes).unwrap();
                            match case {
                                0 => {
                                    let Node::Group { branches, .. } = &mut saved.tree else {
                                        panic!()
                                    };
                                    branches[2].name = "iteration_9".into();
                                }
                                1 => {
                                    let Node::Group { branches, .. } = &mut saved.tree else {
                                        panic!()
                                    };
                                    branches[2].result_type = 9;
                                }
                                2 => {
                                    let SavedRow::Prepared(operand) = &mut saved.rows[2] else {
                                        panic!()
                                    };
                                    operand.operation = op(9);
                                }
                                3 => {
                                    let SavedRow::Prepared(operand) = &mut saved.rows[2] else {
                                        panic!()
                                    };
                                    operand.value = ScalarValue::Integer(99);
                                }
                                4 => saved.frames[0].groups[0].group = 1,
                                5 => saved.captured.node = group(GroupKind::Sequence, 3),
                                6 => saved.captured.rows.swap(0, 2),
                                _ => saved.remaining += 1,
                            }
                            let candidate = serde_json::to_vec(&saved).unwrap();
                            assert_ne!(candidate, bytes);
                            assert!(decode::<String>(&candidate, &bytes).is_err());
                        }
                    }
                }
            }
        }

        mod parallel {
            use super::*;
            use leselang_runtime_core::LocalExecutionScope;

            pub(super) const ORDERS: [[usize; 3]; 6] = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ];
            type Inbox<T> = Vec<Option<Input<T>>>;
            type BarrierState<'a, T> = (Barrier<'a, T>, Inbox<T>);

            pub(super) fn complete<'a, T: Projection>(
                env: &Adapter<'a, T>,
                barrier: &mut Barrier<'a, T>,
                inbox: &mut Inbox<T>,
                order: [usize; 3],
            ) -> Result<Vec<Row<'a, T>>, &'static str> {
                for index in order {
                    let Some(reply) = inbox[index].take() else {
                        continue;
                    };
                    let remaining = 1 + inbox.iter().filter(|r| r.is_some()).count();
                    assert!(
                        matches!(barrier.poll().unwrap(), ParallelPoll::Awaiting { remaining: n } if n == remaining)
                    );
                    let status = barrier.status();
                    let pointer = &*reply.data as *const T;
                    let wrong = barrier
                        .try_accept::<T, _>(index, &999, reply, env.active)
                        .unwrap_err();
                    assert_eq!(&*wrong.reply.data as *const T, pointer);
                    assert_eq!(barrier.status(), status);
                    let wrong = barrier
                        .try_accept::<T, _>(
                            (index + 1) % 3,
                            &env.active.key(index),
                            wrong.reply,
                            env.active,
                        )
                        .unwrap_err();
                    assert_eq!(&*wrong.reply.data as *const T, pointer);
                    assert_eq!(barrier.status(), status);
                    barrier
                        .try_accept::<T, _>(index, &env.active.key(index), wrong.reply, env.active)
                        .map_err(|_| "receipt")?;
                    let status = barrier.status();
                    let duplicate = input::<T>(index, &env.captured.drops);
                    let pointer = &*duplicate.data as *const T;
                    let duplicate = barrier
                        .try_accept::<T, _>(index, &env.active.key(index), duplicate, env.active)
                        .unwrap_err();
                    assert_eq!(&*duplicate.reply.data as *const T, pointer);
                    assert_eq!(barrier.status(), status);
                    assert_eq!(
                        (env.captured.restored.get(), env.captured.projected.get()),
                        (0, 0)
                    );
                }
                let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
                    return Err("barrier");
                };
                for (index, row) in rows.iter().enumerate() {
                    assert_eq!(
                        (row.as_parts().0, *row.as_parts().2),
                        (index, env.active.key(index))
                    );
                    assert_eq!(*row.as_parts().4.data, T::sample(index));
                }
                assert!(matches!(
                    barrier.poll().unwrap(),
                    ParallelPoll::Terminal(ParallelEnd::Completed)
                ));
                assert!(!barrier.cancel());
                Ok(rows)
            }
            fn initial<T: Projection>(
                kind: GroupKind,
                mask: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
            ) -> Initial {
                initial_with_tree::<T>(
                    kind,
                    mask,
                    order,
                    allowance,
                    save,
                    tree::<T>(GroupKind::Parallel),
                )
            }
            pub(super) fn initial_with_tree<T: Projection>(
                kind: GroupKind,
                mask: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
                tree: Node,
            ) -> Initial {
                let node = group(kind, 3);
                let host = Host::new(3);
                let active = Host::new(3);
                active.generation.set(2);
                let parameters = [NamedParameter::required(
                    "value",
                    ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                )];
                let schemas = schemas::<T>(&active, &parameters, active_node(&tree));
                let env = adapter(&host, &active, &schemas, None);
                let (rows, mut fuel) =
                    super::super::super::complete::<T>(&node, &host, &env.captured.drops, 1000);
                let assembled = Rc::new(
                    assemble_group_reply(&node, rows, LIMITS, &mut fuel, |g, _| admit(g, &host))
                        .unwrap(),
                );
                let assembly_remaining = fuel.remaining();
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
                    request: work,
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
                drop(bindings);
                checkpoint(
                    &env,
                    Flight {
                        assembled,
                        assembly_remaining,
                        additional: Vec::new(),
                        tree: &tree,
                        work,
                        continuation,
                        fuel,
                    },
                    mask,
                    order,
                    allowance,
                    save,
                )
            }
            pub(super) fn checkpoint<'a, T: Projection>(
                env: &Adapter<'a, T>,
                flight: Flight<'a, T>,
                mask: usize,
                order: [usize; 3],
                allowance: Option<u64>,
                save: bool,
            ) -> Initial {
                let Flight {
                    assembled,
                    assembly_remaining,
                    additional,
                    tree,
                    work,
                    continuation,
                    mut fuel,
                } = flight;
                let Work::Parallel(mut barrier, requests) = work else {
                    panic!("wrong group mode")
                };
                assert_eq!(continuation.frame_count(), 2);
                let mut inbox: Inbox<T> = (0..3).map(|_| None).collect();
                for request in requests.into_iter().rev() {
                    assert_eq!(request.index, request.request);
                    env.active
                        .authorize(&env.active.key(request.index))
                        .unwrap();
                    inbox[request.index] = Some(env.invoke(request.request));
                }
                let values: Vec<_> = inbox
                    .iter()
                    .map(|r| (*r.as_ref().unwrap().data).clone())
                    .collect();
                for index in order.into_iter().rev().filter(|i| mask & (1 << i) != 0) {
                    barrier
                        .try_accept::<T, _>(
                            index,
                            &env.active.key(index),
                            inbox[index].take().unwrap(),
                            env.active,
                        )
                        .unwrap();
                }
                if let Some(allowance) = allowance {
                    fuel.charge(fuel.remaining() - allowance).unwrap();
                }
                assert_eq!(
                    (
                        env.captured.host.preparations.get(),
                        env.active.preparations.get(),
                        env.captured.host.dispatches.get(),
                        env.active.dispatches.get()
                    ),
                    (3, 3, 3, 3)
                );
                assert!(env.inventory.borrow().iter().all(Option::is_none));
                if !save {
                    let rows = complete(env, &mut barrier, &mut inbox, order).unwrap();
                    return Initial::Finished(
                        assemble_and_resume(env, active_node(tree), rows, continuation, &mut fuel)
                            .unwrap(),
                    );
                }
                let captured_count = 1 + additional.len();
                let image = Saved {
                    additional: additional
                        .iter()
                        .map(|(group, remaining)| group_image(group, *remaining))
                        .collect(),
                    captured: Image {
                        node: assembled.group().clone(),
                        rows: assembled
                            .members()
                            .iter()
                            .map(|r| (*r.as_parts().2, (*r.as_parts().4.data).clone()))
                            .collect(),
                        remaining: assembly_remaining,
                        assembled: true,
                    },
                    frames: snapshots_for_groups(
                        continuation,
                        &std::iter::once(&assembled)
                            .chain(additional.iter().map(|(group, _)| group))
                            .collect::<Vec<_>>(),
                    ),
                    tree: tree.clone(),
                    rows: values
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| {
                            if mask & (1 << i) == 0 {
                                SavedRow::Awaiting(v)
                            } else {
                                SavedRow::Accepted(v)
                            }
                        })
                        .collect(),
                    remaining: fuel.remaining(),
                };
                let bytes = serde_json::to_vec(&image).unwrap();
                drop((barrier, inbox, assembled, additional));
                assert_eq!(env.captured.drops.get(), 3 * (captured_count + 1));
                Initial::Saved(bytes)
            }
            fn saved<T: Projection>(
                kind: GroupKind,
                mask: usize,
                order: [usize; 3],
                allowance: Option<u64>,
            ) -> Vec<u8> {
                let Initial::Saved(bytes) = initial::<T>(kind, mask, order, allowance, true) else {
                    panic!()
                };
                bytes
            }
            pub(super) fn restore_barrier<'a, T: Projection>(
                saved: &'a Saved<T>,
                env: &Adapter<'a, T>,
            ) -> Result<BarrierState<'a, T>, &'static str> {
                let node = active_node(&saved.tree);
                // All publication happened before this checkpoint; no Prepared row may rearm it.
                if saved
                    .rows
                    .iter()
                    .any(|row| matches!(row, SavedRow::Prepared(_)))
                {
                    return Err("unpublished All row");
                }
                let mut rows = Vec::new();
                let mut inbox = Vec::new();
                for (index, state) in saved.rows.iter().enumerate() {
                    let identity = env.active.key(index);
                    let declaration: &NativeDomain<T> = &env.active.domains[index];
                    let value = match state {
                        SavedRow::Accepted(v) | SavedRow::Awaiting(v) => v,
                        SavedRow::Prepared(_) => unreachable!(),
                    };
                    let input = Input {
                        data: Box::new(value.clone()),
                        drops: env.captured.drops.clone(),
                    };
                    let reply = match state {
                        SavedRow::Accepted(_) => {
                            inbox.push(None);
                            SavedParallelReply::Accepted(
                                PendingReply::new(identity, (), declaration)
                                    .try_accept::<_, T, _>(&identity, input, env.active)
                                    .map_err(|_| "accepted row")?,
                            )
                        }
                        SavedRow::Awaiting(_) => {
                            inbox.push(Some(input));
                            SavedParallelReply::Awaiting {
                                identity,
                                declaration,
                            }
                        }
                        SavedRow::Prepared(_) => unreachable!(),
                    };
                    rows.push(SavedParallelMember {
                        index,
                        name: &branches(node)[index].name,
                        reply,
                    });
                }
                let barrier = Barrier::restore(node, rows, QUEUE_LIMITS, |rows| {
                    for row in rows {
                        env.active.authorize(row.reply.identity())?;
                    }
                    Ok::<_, &'static str>(())
                })
                .map_err(|_| "saved barrier")?;
                Ok((barrier, inbox))
            }
            pub(super) fn restored<T: Projection>(
                saved: &Saved<T>,
                order: [usize; 3],
                mode: usize,
            ) -> Result<(Report, usize), &'static str> {
                let host = Host::new(3);
                let active = Host::new(3);
                active.generation.set(2);
                let parameters = [NamedParameter::required(
                    "value",
                    ScalarTypeSet::only(T::sample(0).scalar().scalar_type()),
                )];
                let schemas = schemas::<T>(&active, &parameters, active_node(&saved.tree));
                let env = adapter(&host, &active, &schemas, Some(&saved.captured));
                active.granted.set(mode != 1);
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
                    let (mut barrier, mut inbox) = restore_barrier(saved, &env)?;
                    assert_eq!(env.captured.rebuilt.get(), 0);
                    if mode == 2 {
                        active.granted.set(false);
                    }
                    if mode == 5 {
                        active.domains[2].denied.set(true);
                    }
                    let mut fuel = Fuel::new(saved.remaining);
                    let rows = complete(&env, &mut barrier, &mut inbox, order)?;
                    assemble_and_resume(
                        &env,
                        active_node(&saved.tree),
                        rows,
                        continuation,
                        &mut fuel,
                    )
                })();
                if [1, 2, 5].contains(&mode) {
                    assert!(result.is_err());
                    assert_eq!(
                        (
                            env.captured.restored.get(),
                            env.captured.projected.get(),
                            env.captured.rebuilt.get()
                        ),
                        (0, 0, 0)
                    );
                }
                assert_eq!(
                    (
                        host.preparations.get(),
                        active.preparations.get(),
                        host.dispatches.get(),
                        active.dispatches.get(),
                        env.restored_calls.get()
                    ),
                    (0, 0, 0, 0, 0)
                );
                assert!(env.inventory.borrow().is_empty());
                result.map(|report| (report, env.captured.rebuilt.get()))
            }
            #[test]
            fn captured_all_eight_masks_and_six_orders_restore_without_requests_or_replay() {
                fn run<T: Projection>() {
                    for kind in KINDS {
                        for mask in 0..8 {
                            for order in ORDERS {
                                let Initial::Finished(expected) =
                                    initial::<T>(kind, mask, order, None, false)
                                else {
                                    panic!()
                                };
                                let bytes = saved::<T>(kind, mask, order, None);
                                let image = decode::<T>(&bytes, &bytes).unwrap();
                                let (actual, rebuilt) = restored(&image, order, 0).unwrap();
                                assert_eq!(actual, expected);
                                assert_eq!(actual.value, Ok(T::sample(2).scalar()));
                                assert_eq!((actual.restored, actual.projected, rebuilt), (2, 1, 1));
                            }
                        }
                    }
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn captured_all_every_assembly_return_fuel_cutoff_matches_live_at_every_mask_and_order()
            {
                fn run<T: Projection>() {
                    for kind in KINDS {
                        for mask in 0..8 {
                            for order in ORDERS {
                                let bytes = saved::<T>(kind, mask, order, None);
                                let image = decode::<T>(&bytes, &bytes).unwrap();
                                let needed = image.remaining
                                    - restored(&image, order, 0).unwrap().0.remaining;
                                for cutoff in 0..=needed + 1 {
                                    let Initial::Finished(expected) =
                                        initial::<T>(kind, mask, order, Some(cutoff), false)
                                    else {
                                        panic!()
                                    };
                                    let bytes = saved::<T>(kind, mask, order, Some(cutoff));
                                    let image = decode::<T>(&bytes, &bytes).unwrap();
                                    let (actual, _) = restored(&image, order, 0).unwrap();
                                    assert_eq!(
                                        actual, expected,
                                        "mask={mask} order={order:?} fuel={cutoff}"
                                    );
                                    assert_eq!(
                                        actual.value,
                                        if cutoff < needed {
                                            Err(Failure::Fuel)
                                        } else {
                                            Ok(T::sample(2).scalar())
                                        }
                                    );
                                }
                            }
                        }
                    }
                }
                run::<u64>();
                run::<String>();
            }
            #[test]
            fn captured_all_authority_domain_and_cold_return_refusals_do_not_restore_captures_early()
             {
                for kind in KINDS {
                    for mask in 0..8 {
                        let bytes = saved::<String>(kind, mask, ORDERS[4], None);
                        let image = decode::<String>(&bytes, &bytes).unwrap();
                        for mode in [1, 2, 5] {
                            assert!(restored(&image, ORDERS[4], mode).is_err());
                        }
                        let (cold, rebuilt) = restored(&image, ORDERS[4], 3).unwrap();
                        assert_eq!(cold.value, Err(Failure::Native("cold schema")));
                        assert_eq!(
                            (cold.remaining, cold.restored, cold.projected, rebuilt),
                            (image.remaining - 4, 0, 0, 0)
                        );
                        let (late, rebuilt) = restored(&image, ORDERS[4], 4).unwrap();
                        assert_eq!(late.value, Err(Failure::Native("domain")));
                        assert_eq!((late.restored, late.projected, rebuilt), (2, 1, 1));
                    }
                }
            }
            #[test]
            fn captured_all_cancelled_partial_or_ready_barriers_drop_results_without_parent_reentry()
             {
                for mask in 0..8 {
                    for scoped in [false, true] {
                        let bytes = saved::<String>(GroupKind::Parallel, mask, ORDERS[5], None);
                        let image = decode::<String>(&bytes, &bytes).unwrap();
                        let host = Host::new(3);
                        let active = Host::new(3);
                        active.generation.set(2);
                        let parameters = [NamedParameter::required(
                            "value",
                            ScalarTypeSet::only(String::sample(0).scalar().scalar_type()),
                        )];
                        let schemas =
                            schemas::<String>(&active, &parameters, active_node(&image.tree));
                        let env = adapter(&host, &active, &schemas, Some(&image.captured));
                        let continuation = restore_effect_return_stack(
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
                        .unwrap();
                        let (mut barrier, inbox) = restore_barrier(&image, &env).unwrap();
                        let fuel = Fuel::new(image.remaining);
                        if scoped {
                            let scope = LocalExecutionScope::new();
                            scope.close();
                            assert!(matches!(
                                barrier.poll_in_scope(&scope).unwrap(),
                                ParallelPoll::Terminal(ParallelEnd::Cancelled)
                            ));
                        } else {
                            assert!(barrier.cancel());
                        }
                        assert!(!barrier.cancel());
                        assert!(matches!(
                            barrier.poll().unwrap(),
                            ParallelPoll::Terminal(ParallelEnd::Cancelled)
                        ));
                        assert_eq!(env.captured.drops.get(), mask.count_ones() as usize);
                        for (index, reply) in inbox.into_iter().enumerate() {
                            if let Some(reply) = reply {
                                let pointer = &*reply.data as *const String;
                                let rejected = barrier
                                    .try_accept::<String, _>(
                                        index,
                                        &active.key(index),
                                        reply,
                                        &active,
                                    )
                                    .unwrap_err();
                                assert_eq!(&*rejected.reply.data as *const String, pointer);
                            }
                        }
                        drop((barrier, continuation));
                        assert_eq!(env.captured.drops.get(), 3);
                        assert_eq!(fuel.remaining(), image.remaining);
                        assert_eq!(
                            (
                                env.captured.restored.get(),
                                env.captured.projected.get(),
                                env.captured.rebuilt.get(),
                                active.dispatches.get()
                            ),
                            (0, 0, 0, 0)
                        );
                    }
                }
            }
            #[test]
            fn captured_all_forged_completion_graph_captures_and_budget_never_rearm_a_batch() {
                let bytes = saved::<String>(GroupKind::Sequence, 5, ORDERS[3], None);
                for case in 0..7 {
                    let mut image = decode::<String>(&bytes, &bytes).unwrap();
                    match case {
                        0 => image.rows[0] = SavedRow::Awaiting(String::sample(0)),
                        1 => image.rows.swap(0, 2),
                        2 => image.rows[1] = SavedRow::Accepted("forged".into()),
                        3 => image.frames.swap(0, 1),
                        4 => image.frames[0].groups[0].group = 1,
                        5 => image.remaining += 1,
                        _ => image.tree = tree::<String>(GroupKind::Sequence),
                    }
                    assert!(
                        decode::<String>(&serde_json::to_vec(&image).unwrap(), &bytes).is_err()
                    );
                }
                let mut image = decode::<String>(&bytes, &bytes).unwrap();
                image.rows[1] = SavedRow::Prepared(Operand {
                    operation: op(1),
                    name: "value".into(),
                    parameter: 0,
                    argument: 0,
                    value: String::sample(1).scalar(),
                });
                assert_eq!(restored(&image, ORDERS[3], 0), Err("unpublished All row"));
            }
        }
    }
}
