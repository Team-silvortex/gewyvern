//! Saved flat-All consumer model. No filesystem/process-crash or atomic dispatch claim.
use std::{
    borrow::Borrow,
    cell::Cell,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::prepare_native_group_members,
    parallel_evaluation::*,
    prepared_parallel::PreparedParallelSession,
};
use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionScope, PendingReply, ReplyAuthority,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

type Node = Computation<(), (), (), ()>;
type Session<'a, T> = PreparedParallelSession<'a, Node, (), Identity, Domain, Value<T>>;
type Row<'a, T> = SavedParallelMember<'a, Identity, Domain, Value<T>>;
const LIMITS: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 65,
    max_depth: 1,
    max_branches: 64,
};

#[derive(Default)]
struct Control {
    equalities: Cell<usize>,
    admissions: Cell<usize>,
    preparations: Cell<usize>,
    dispatches: Cell<usize>,
    identities_dropped: Cell<usize>,
    values_dropped: Cell<usize>,
    denied: Cell<bool>,
    equality_unwind: Cell<bool>,
    authority_unwind: Cell<bool>,
    cleanup_unwind: Cell<bool>,
}
struct Identity {
    id: usize,
    control: Rc<Control>,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.control
            .equalities
            .set(self.control.equalities.get() + 1);
        assert!(!self.control.equality_unwind.get(), "identity unwind");
        self.id == other.id && Rc::ptr_eq(&self.control, &other.control)
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        self.control
            .identities_dropped
            .set(self.control.identities_dropped.get() + 1);
        assert!(
            !self.control.cleanup_unwind.replace(false),
            "cleanup unwind"
        );
    }
}
fn identity(control: &Rc<Control>, id: usize) -> Identity {
    Identity {
        id,
        control: control.clone(),
    }
}
struct Value<T> {
    data: Box<T>,
    control: Rc<Control>,
}
impl<T> Borrow<T> for Value<T> {
    fn borrow(&self) -> &T {
        &self.data
    }
}
impl<T> Drop for Value<T> {
    fn drop(&mut self) {
        self.control
            .values_dropped
            .set(self.control.values_dropped.get() + 1);
    }
}
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
struct Domain {
    index: usize,
    control: Rc<Control>,
}
impl<T: Data> HostResultDomain<T> for Domain {
    type Error = &'static str;
    fn matches_type(&self, _: &T) -> bool {
        true
    }
    fn validate_value(&self, value: &T) -> Result<(), Self::Error> {
        if self.control.denied.get() || *value != T::sample(self.index) {
            Err("domain")
        } else {
            Ok(())
        }
    }
}
impl ReplyAuthority<Identity> for Control {
    type Error = &'static str;
    fn authorize(&self, key: &Identity) -> Result<(), Self::Error> {
        assert!(!self.authority_unwind.get(), "authority unwind");
        if self.denied.get() || !std::ptr::eq(self, &*key.control) {
            Err("authority")
        } else {
            Ok(())
        }
    }
}
fn group(count: usize) -> Node {
    Node::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..count)
            .map(|index| ComputedBranch {
                name: format!("m{index}"),
                value: Node::Host {
                    effect: Box::new(()),
                },
                result_type: (),
            })
            .collect(),
    }
}
fn branches(node: &Node) -> &[ComputedBranch<Node, ()>] {
    let Node::Group { branches, .. } = node else {
        panic!()
    };
    branches
}
fn domains(control: &Rc<Control>, count: usize) -> Vec<Domain> {
    (0..count)
        .map(|index| Domain {
            index,
            control: control.clone(),
        })
        .collect()
}
fn value<T: Data>(control: &Rc<Control>, index: usize) -> Value<T> {
    Value {
        data: Box::new(T::sample(index)),
        control: control.clone(),
    }
}
fn rows<'a, T: Data>(
    node: &'a Node,
    domains: &'a [Domain],
    control: &Rc<Control>,
    mask: u64,
) -> Vec<Row<'a, T>> {
    branches(node)
        .iter()
        .enumerate()
        .map(|(index, branch)| {
            let reply = if mask & (1 << index) == 0 {
                SavedParallelReply::Awaiting {
                    identity: identity(control, index),
                    declaration: &domains[index],
                }
            } else {
                let accepted = PendingReply::new(identity(control, index), (), &domains[index])
                    .try_accept::<_, T, _>(
                        &identity(control, index),
                        value::<T>(control, index),
                        &**control,
                    )
                    .unwrap();
                SavedParallelReply::Accepted(accepted)
            };
            Row {
                index,
                name: &branch.name,
                reply,
            }
        })
        .collect()
}
fn admit<T: Data>(control: &Rc<Control>, rows: &[Row<'_, T>]) -> Result<(), &'static str> {
    control.admissions.set(control.admissions.get() + 1);
    for row in rows {
        control.authorize(row.reply.identity())?;
        if row.reply.identity().id != row.index {
            return Err("original identity");
        }
        let domain = match &row.reply {
            SavedParallelReply::Awaiting { declaration, .. } => *declaration,
            SavedParallelReply::Accepted(accepted) => {
                let (_, (), domain, reply) = accepted.as_parts();
                domain.validate_value(&*reply.data)?;
                domain
            }
        };
        if domain.index != row.index || !Rc::ptr_eq(&domain.control, control) {
            return Err("original domain");
        }
    }
    Ok(())
}

#[test]
fn every_partial_mask_restores_the_original_barrier_and_order_without_a_new_meter() {
    fn run<T: Data>() {
        for mask in 0..8 {
            let control = Rc::new(Control::default());
            let node = group(3);
            let domains = domains(&control, 3);
            let saved = rows::<T>(&node, &domains, &control, mask);
            let pointers: Vec<_> = saved
                .iter()
                .filter_map(|row| match &row.reply {
                    SavedParallelReply::Accepted(reply) => {
                        Some((row.index, &*reply.as_parts().3.data as *const T))
                    }
                    _ => None,
                })
                .collect();
            let mut session =
                Session::restore(&node, saved, LIMITS, |rows| admit(&control, rows)).unwrap();
            for index in (0..3).rev().filter(|index| mask & (1 << index) == 0) {
                assert!(matches!(
                    session.poll().unwrap(),
                    ParallelPoll::Awaiting { .. }
                ));
                let wrong = value::<T>(&control, index);
                let pointer = &*wrong.data as *const T;
                let rejected = session
                    .try_accept::<T, _>(index, &identity(&control, index + 9), wrong, &*control)
                    .unwrap_err();
                assert_eq!(&*rejected.reply.data as *const T, pointer);
                session
                    .try_accept::<T, _>(
                        index,
                        &identity(&control, index),
                        rejected.reply,
                        &*control,
                    )
                    .unwrap();
                assert!(
                    session
                        .try_accept::<T, _>(
                            index,
                            &identity(&control, index),
                            value::<T>(&control, index),
                            &*control
                        )
                        .is_err()
                );
            }
            let ParallelPoll::Results(accepted) = session.poll().unwrap() else {
                panic!()
            };
            let mut fuel = Fuel::new(4);
            let reply =
                assemble_group_reply(&node, accepted, LIMITS, &mut fuel, |_, _| Ok::<_, ()>(()))
                    .unwrap();
            assert_eq!(fuel.remaining(), 0);
            for (index, row) in reply.members().iter().enumerate() {
                let (position, original, key, domain, result) = row.as_parts();
                assert_eq!(position, index);
                assert!(std::ptr::eq(original, &branches(&node)[index]));
                assert_eq!(key.id, index);
                assert!(std::ptr::eq(domain, &domains[index]));
                assert_eq!(*result.data, T::sample(index));
                if let Some((_, pointer)) = pointers.iter().find(|(i, _)| *i == index) {
                    assert_eq!(&*result.data as *const T, *pointer);
                }
            }
            assert!(matches!(
                session.poll().unwrap(),
                ParallelPoll::Terminal(ParallelEnd::Completed)
            ));
            assert!(!session.cancel());
            assert_eq!(control.admissions.get(), 1);
            assert_eq!(control.preparations.get(), 0);
            assert_eq!(control.dispatches.get(), 0);
        }
    }
    run::<u64>();
    run::<String>();
}

#[test]
fn malformed_saved_shape_count_and_rows_refuse_before_equality_or_admission() {
    for case in 0..8 {
        let control = Rc::new(Control::default());
        let mut node = group(3);
        if case == 6 {
            let Node::Group { group_kind, .. } = &mut node else {
                panic!()
            };
            *group_kind = GroupKind::Sequence;
        }
        let domains = domains(&control, 3);
        let mut saved = rows::<u64>(&node, &domains, &control, 0);
        let mut limits = LIMITS;
        match case {
            0 => {
                saved.pop();
            }
            1 => saved[2].index = 1,
            2 => saved.swap(0, 2),
            3 => saved[1].name = "forged",
            4 => limits.max_branches = 2,
            5 => limits.max_nodes = 3,
            6 => {}
            7 => limits.max_branches = 65,
            _ => unreachable!(),
        }
        assert!(Session::restore(&node, saved, limits, |rows| admit(&control, rows)).is_err());
        assert_eq!(control.equalities.get(), 0);
        assert_eq!(control.admissions.get(), 0);
        assert_eq!(control.identities_dropped.get(), 3);
    }
}

#[test]
fn duplicate_saved_identity_including_accepted_rows_refuses_before_host_admission() {
    for mask in [0, 1, 2, 3] {
        let control = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&control, 3);
        let mut saved = rows::<u64>(&node, &domains, &control, mask);
        let replacement = identity(&control, 0);
        match &mut saved[1].reply {
            SavedParallelReply::Awaiting { identity, .. } => *identity = replacement,
            SavedParallelReply::Accepted(_) => {
                let accepted = PendingReply::new(replacement, (), &domains[1])
                    .try_accept::<_, u64, _>(
                        &identity(&control, 0),
                        value::<u64>(&control, 1),
                        &*control,
                    )
                    .unwrap();
                saved[1].reply = SavedParallelReply::Accepted(accepted);
            }
        }
        assert!(matches!(
            Session::restore(&node, saved, LIMITS, |rows| admit(&control, rows)),
            Err(ParallelRestoreError::Parallel(
                ParallelEvaluationError::DuplicateIdentity {
                    index: 1,
                    previous: 0
                }
            ))
        ));
        assert_eq!(control.admissions.get(), 0);
    }
}

#[test]
fn whole_saved_admission_rechecks_accepted_authority_and_unwind_publishes_nothing() {
    for mode in 0..5 {
        let control = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&control, 3);
        let saved = rows::<u64>(&node, &domains, &control, 5);
        let identities_before = control.identities_dropped.get();
        if mode == 0 {
            control.denied.set(true);
        }
        if mode == 1 {
            control.equality_unwind.set(true);
        }
        if mode == 4 {
            control.cleanup_unwind.set(true);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            Session::restore(&node, saved, LIMITS, |rows| {
                assert_ne!(mode, 2, "admission unwind");
                if mode >= 3 {
                    return Err("refusal");
                }
                admit(&control, rows)
            })
        }));
        if mode == 0 || mode == 3 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        assert_eq!(control.identities_dropped.get() - identities_before, 3);
        assert_eq!(control.values_dropped.get(), 2);
        assert_eq!(control.preparations.get(), 0);
    }
}

#[test]
fn saved_native_domain_and_owner_lookalikes_are_not_fresh_authority() {
    for foreign_owner in [false, true] {
        let control = Rc::new(Control::default());
        let foreign = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&control, 3);
        let mut saved = rows::<u64>(&node, &domains, &control, 1);
        if foreign_owner {
            saved[2].reply = SavedParallelReply::Awaiting {
                identity: identity(&foreign, 2),
                declaration: &domains[2],
            };
        } else {
            saved[2].reply = SavedParallelReply::Awaiting {
                identity: identity(&control, 2),
                declaration: &domains[1],
            };
        }
        assert!(matches!(
            Session::restore(&node, saved, LIMITS, |rows| admit(&control, rows)),
            Err(ParallelRestoreError::Native(_))
        ));
        assert_eq!(control.admissions.get(), 1);
        assert_eq!(control.values_dropped.get(), 1);
    }
}

#[test]
fn restored_cancel_scope_and_acceptance_unwind_keep_existing_terminal_fences() {
    for mode in 0..4 {
        let control = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&control, 3);
        let saved = rows::<u64>(&node, &domains, &control, if mode == 1 { 7 } else { 1 });
        let mut session =
            Session::restore(&node, saved, LIMITS, |rows| admit(&control, rows)).unwrap();
        let scope = LocalExecutionScope::new();
        match mode {
            0 | 1 => {
                assert!(session.cancel());
            }
            2 => {
                scope.close();
                assert!(matches!(
                    session.poll_in_scope(&scope).unwrap(),
                    ParallelPoll::Terminal(ParallelEnd::Cancelled)
                ));
            }
            3 => {
                control.authority_unwind.set(true);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| session.try_accept_in_scope::<u64, _>(
                        1,
                        &identity(&control, 1),
                        value::<u64>(&control, 1),
                        &*control,
                        &scope
                    )))
                    .is_err()
                );
                assert!(!scope.is_open());
            }
            _ => unreachable!(),
        }
        let end = if mode == 3 {
            ParallelEnd::HostUncertain
        } else {
            ParallelEnd::Cancelled
        };
        assert_eq!(session.status(), ParallelStatus::Terminal(end));
        assert!(matches!(session.poll().unwrap(), ParallelPoll::Terminal(_)));
        assert!(!session.cancel());
        assert_eq!(
            control.values_dropped.get(),
            if mode == 1 {
                3
            } else if mode == 3 {
                2
            } else {
                1
            }
        );
    }
}

#[test]
fn maximum_saved_barrier_and_unsized_domains_preserve_native_ownership() {
    let control = Rc::new(Control::default());
    let node = group(64);
    let domains = domains(&control, 64);
    let saved = rows::<u64>(&node, &domains, &control, u64::MAX);
    let saved: Vec<SavedParallelMember<'_, _, dyn HostResultDomain<u64, Error = &'static str>, _>> =
        saved
            .into_iter()
            .map(|row| {
                let SavedParallelReply::Accepted(accepted) = row.reply else {
                    panic!()
                };
                let (key, (), domain, reply) = accepted.into_parts();
                let actual = identity(&control, key.id);
                let domain: &dyn HostResultDomain<u64, Error = &'static str> = domain;
                let accepted = PendingReply::new(key, (), domain)
                    .try_accept(&actual, reply, &*control)
                    .unwrap();
                SavedParallelMember {
                    index: row.index,
                    name: row.name,
                    reply: SavedParallelReply::Accepted(accepted),
                }
            })
            .collect();
    let mut session =
        PreparedParallelSession::restore(&node, saved, LIMITS, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(session.status(), ParallelStatus::ReadyResults);
    assert!(!format!("{session:?}").contains("panel"));
    let ParallelPoll::Results(results) = session.poll().unwrap() else {
        panic!()
    };
    assert_eq!(results.len(), 64);
    drop(results);
    assert_eq!(control.values_dropped.get(), 64);
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRow<T> {
    index: usize,
    name: String,
    identity: usize,
    accepted: Option<T>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image<T> {
    node: Node,
    rows: Vec<WireRow<T>>,
    fuel: u64,
    revision: u64,
}
struct Journal {
    pending: Vec<u8>,
    settled: Option<Vec<u8>>,
}

fn publish<T: Data>(control: &Rc<Control>, mask: u64) -> Journal {
    let node = group(3);
    let domains = domains(control, 3);
    let mut fuel = Fuel::new(17);
    let prepared = prepare_native_group_members(
        GroupKind::Parallel,
        branches(&node),
        3,
        &mut fuel,
        |row| Ok::<_, ()>(row.name.as_str()),
        |_| Ok(()),
        |index, _, fuel| {
            fuel.charge(1).unwrap();
            control.preparations.set(control.preparations.get() + 1);
            Ok(index)
        },
    )
    .unwrap();
    let (mut session, requests) = Session::<T>::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |_, _, _| Ok::<_, ()>(()),
        |index, _, request, fuel| {
            fuel.charge(1).unwrap();
            Ok(PreparedParallelMember {
                identity: identity(control, index),
                declaration: &domains[index],
                request,
            })
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 3);
    let mut saved = Vec::new();
    for request in requests {
        control.dispatches.set(control.dispatches.get() + 1);
        let index = request.index;
        assert_eq!(request.request, index);
        let accepted = if mask & (1 << index) != 0 {
            session
                .try_accept::<T, _>(
                    index,
                    &identity(control, index),
                    value::<T>(control, index),
                    &**control,
                )
                .unwrap();
            Some(T::sample(index))
        } else {
            None
        };
        saved.push(WireRow {
            index,
            name: branches(&node)[index].name.clone(),
            identity: index,
            accepted,
        });
    }
    drop(session);
    Journal {
        pending: serde_json::to_vec(&Image {
            node,
            rows: saved,
            fuel: fuel.remaining(),
            revision: 7,
        })
        .unwrap(),
        settled: None,
    }
}

fn recover<T: Data>(
    journal: &mut Journal,
    candidate: &[u8],
    control: &Rc<Control>,
    reply: Option<usize>,
) -> Result<Option<Vec<T>>, &'static str> {
    if candidate.len() > 65_536 {
        return Err("snapshot bytes");
    }
    if candidate != journal.pending {
        return Err("snapshot correspondence");
    }
    if let Some(bytes) = &journal.settled {
        return Ok(Some(serde_json::from_slice(bytes).unwrap()));
    }
    if control.denied.get() {
        return Err("authority");
    }
    let image: Image<T> = serde_json::from_slice(candidate).map_err(|_| "codec")?;
    if image.revision != 7 || image.fuel != 3 {
        return Err("saved budget/revision");
    }
    let names: Vec<_> = image.rows.iter().map(|row| row.name.clone()).collect();
    let domains = domains(control, image.rows.len());
    let mut rows = Vec::new();
    for (position, row) in image.rows.into_iter().enumerate() {
        let native = match row.accepted {
            None => SavedParallelReply::Awaiting {
                identity: identity(control, row.identity),
                declaration: &domains[position],
            },
            Some(data) => SavedParallelReply::Accepted(
                PendingReply::new(identity(control, row.identity), (), &domains[position])
                    .try_accept::<_, T, _>(
                        &identity(control, row.identity),
                        Value {
                            data: Box::new(data),
                            control: control.clone(),
                        },
                        &**control,
                    )
                    .map_err(|_| "saved reply")?,
            ),
        };
        rows.push(SavedParallelMember {
            index: row.index,
            name: names[position].as_str(),
            reply: native,
        });
    }
    let mut session = Session::restore(&image.node, rows, LIMITS, |rows| admit(control, rows))
        .map_err(|_| "barrier")?;
    if let Some(index) = reply {
        session
            .try_accept::<T, _>(
                index,
                &identity(control, index),
                value::<T>(control, index),
                &**control,
            )
            .map_err(|_| "receipt")?;
        let mut saved: serde_json::Value = serde_json::from_slice(&journal.pending).unwrap();
        saved["rows"][index]["accepted"] = serde_json::to_value(T::sample(index)).unwrap();
        journal.pending = serde_json::to_vec(&saved).unwrap();
    }
    match session.poll().unwrap() {
        ParallelPoll::Awaiting { .. } => Ok(None),
        ParallelPoll::Results(results) => {
            let output: Vec<_> = results
                .iter()
                .map(|result| serde_json::to_value(&*result.as_parts().4.data).unwrap())
                .collect();
            let bytes = serde_json::to_vec(&output).unwrap();
            journal.settled = Some(bytes.clone());
            Ok(Some(serde_json::from_slice(&bytes).unwrap()))
        }
        _ => Err("unexpected restored state"),
    }
}

#[test]
fn two_serialized_host_models_restore_every_partial_mask_without_repreparing_or_redispatching() {
    fn run<T: Data>() {
        for mask in 0..8 {
            let control = Rc::new(Control::default());
            let mut journal = publish::<T>(&control, mask);
            assert_eq!(control.preparations.get(), 3);
            assert_eq!(control.dispatches.get(), 3);
            assert!(recover::<T>(&mut journal, b"forged", &control, None).is_err());
            for index in (0..3).rev().filter(|index| mask & (1 << index) == 0) {
                let bytes = journal.pending.clone();
                let output = recover::<T>(&mut journal, &bytes, &control, Some(index));
                assert!(output.is_ok());
            }
            let bytes = journal.pending.clone();
            let output = recover::<T>(&mut journal, &bytes, &control, None)
                .unwrap()
                .unwrap();
            assert_eq!(output, (0..3).map(T::sample).collect::<Vec<_>>());
            let admissions = control.admissions.get();
            assert!(
                recover::<T>(&mut journal, &bytes, &control, Some(0))
                    .unwrap()
                    .is_some()
            );
            assert_eq!(control.admissions.get(), admissions);
            assert_eq!(control.preparations.get(), 3);
            assert_eq!(control.dispatches.get(), 3);
            let saved: Image<T> = serde_json::from_slice(&journal.pending).unwrap();
            assert_eq!(saved.fuel, 3);
        }
    }
    run::<u64>();
    run::<String>();
}

#[test]
fn serialized_graph_budget_receipt_and_live_refusals_leave_committed_rows_unchanged() {
    fn run<T: Data>() {
        let control = Rc::new(Control::default());
        let mut journal = publish::<T>(&control, 1);
        for field in ["fuel", "revision", "rows"] {
            let bytes = journal.pending.clone();
            let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            forged[field] = serde_json::json!(999);
            assert!(
                recover::<T>(
                    &mut journal,
                    &serde_json::to_vec(&forged).unwrap(),
                    &control,
                    None
                )
                .is_err()
            );
            assert_eq!(journal.pending, bytes);
        }
        let bytes = journal.pending.clone();
        control.denied.set(true);
        assert!(recover::<T>(&mut journal, &bytes, &control, Some(1)).is_err());
        assert_eq!(journal.pending, bytes);
        control.denied.set(false);
        assert!(recover::<T>(&mut journal, &bytes, &control, Some(0)).is_err());
        assert!(recover::<T>(&mut journal, &bytes, &control, Some(4)).is_err());
        assert_eq!(journal.pending, bytes);
        assert_eq!(control.preparations.get(), 3);
        assert_eq!(control.dispatches.get(), 3);
        assert!(journal.settled.is_none());
    }
    run::<u64>();
    run::<String>();
}
