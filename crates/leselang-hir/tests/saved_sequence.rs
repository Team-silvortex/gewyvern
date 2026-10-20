//! Consumer-owned saved Seq model, not a production journal or crash transaction.
use leselang_hir::{
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::prepare_native_group_members,
    prepared_sequence::PreparedSequenceSession,
    sequence_evaluation::*,
};
use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionScope, PendingReply, ReplyAuthority,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

type Node = Computation<(), (), (), ()>;
type Session<'a> = PreparedSequenceSession<'a, Node, (), Identity, Domain, Request>;
type Row<'a, T> = SavedSequenceMember<'a, Identity, Domain, Request, Value<T>>;
const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 65,
    max_depth: 1,
    max_branches: 64,
};
const FRONTIERS: [[u8; 3]; 7] = [
    [2, 2, 2],
    [1, 2, 2],
    [0, 2, 2],
    [0, 1, 2],
    [0, 0, 2],
    [0, 0, 1],
    [0, 0, 0],
];

#[derive(Default)]
struct Control {
    comparisons: Cell<usize>,
    admissions: Cell<usize>,
    publications: RefCell<Vec<usize>>,
    preparations: Cell<usize>,
    identities_dropped: Cell<usize>,
    requests_dropped: Cell<usize>,
    values_dropped: Cell<usize>,
    denied: Cell<bool>,
    equality_unwind: Cell<bool>,
    authority_unwind: Cell<bool>,
    cleanup_unwind: Cell<bool>,
    restore_scope: RefCell<Option<Rc<LocalExecutionScope>>>,
    equality_closes: Cell<bool>,
    cleanup_open: Cell<usize>,
    cleanup_observed: Cell<usize>,
}
impl Control {
    fn observe_cleanup(&self) {
        if let Some(scope) = self.restore_scope.borrow().as_ref() {
            self.cleanup_observed.set(self.cleanup_observed.get() + 1);
            if scope.is_open() {
                self.cleanup_open.set(self.cleanup_open.get() + 1);
            }
        }
    }
}
struct Identity {
    index: usize,
    control: Rc<Control>,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.control
            .comparisons
            .set(self.control.comparisons.get() + 1);
        if self.control.equality_closes.get() {
            self.control
                .restore_scope
                .borrow()
                .as_ref()
                .unwrap()
                .close();
        }
        assert!(
            !self.control.equality_unwind.get(),
            "native equality unwind"
        );
        self.index == other.index && Rc::ptr_eq(&self.control, &other.control)
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        self.control.observe_cleanup();
        self.control
            .identities_dropped
            .set(self.control.identities_dropped.get() + 1);
        assert!(
            !self.control.cleanup_unwind.replace(false),
            "native cleanup unwind"
        );
    }
}
fn key(c: &Rc<Control>, index: usize) -> Identity {
    Identity {
        index,
        control: c.clone(),
    }
}
struct Request {
    index: Box<usize>,
    control: Rc<Control>,
}
impl Drop for Request {
    fn drop(&mut self) {
        self.control.observe_cleanup();
        self.control
            .requests_dropped
            .set(self.control.requests_dropped.get() + 1);
    }
}
fn request(c: &Rc<Control>, index: usize) -> Request {
    Request {
        index: Box::new(index),
        control: c.clone(),
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
        self.control.observe_cleanup();
        self.control
            .values_dropped
            .set(self.control.values_dropped.get() + 1);
    }
}
fn value<T: Data>(c: &Rc<Control>, index: usize) -> Value<T> {
    Value {
        data: Box::new(T::sample(index)),
        control: c.clone(),
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
    fn validate_value(&self, data: &T) -> Result<(), Self::Error> {
        if self.control.denied.get() || *data != T::sample(self.index) {
            Err("domain")
        } else {
            Ok(())
        }
    }
}
impl ReplyAuthority<Identity> for Control {
    type Error = &'static str;
    fn authorize(&self, key: &Identity) -> Result<(), Self::Error> {
        assert!(!self.authority_unwind.get(), "native authority unwind");
        if self.denied.get() || !std::ptr::eq(self, &*key.control) {
            Err("authority")
        } else {
            Ok(())
        }
    }
}
fn group(count: usize) -> Node {
    Node::Group {
        group_kind: GroupKind::Sequence,
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
fn domains(c: &Rc<Control>, count: usize) -> Vec<Domain> {
    (0..count)
        .map(|index| Domain {
            index,
            control: c.clone(),
        })
        .collect()
}
fn rows<'a, T: Data>(
    node: &'a Node,
    domains: &'a [Domain],
    c: &Rc<Control>,
    states: &[u8],
) -> Vec<Row<'a, T>> {
    branches(node)
        .iter()
        .zip(states)
        .enumerate()
        .map(|(index, (branch, state))| {
            let reply = match state {
                0 => SavedSequenceReply::Accepted(
                    PendingReply::new(key(c, index), (), &domains[index])
                        .try_accept::<_, T, _>(&key(c, index), value::<T>(c, index), &**c)
                        .unwrap(),
                ),
                1 => SavedSequenceReply::Awaiting {
                    identity: key(c, index),
                    declaration: &domains[index],
                },
                2 => SavedSequenceReply::Prepared(PreparedSequenceMember {
                    identity: key(c, index),
                    declaration: &domains[index],
                    request: request(c, index),
                }),
                _ => unreachable!(),
            };
            SavedSequenceMember {
                index,
                name: &branch.name,
                reply,
            }
        })
        .collect()
}
fn admit_request(
    c: &Rc<Control>,
    index: usize,
    id: &Identity,
    domain: &Domain,
    request: &Request,
) -> Result<(), &'static str> {
    c.authorize(id)?;
    if id.index != index
        || domain.index != index
        || *request.index != index
        || !Rc::ptr_eq(&domain.control, c)
        || !Rc::ptr_eq(&request.control, c)
    {
        return Err("original request");
    }
    Ok(())
}
fn admit<T: Data>(c: &Rc<Control>, rows: &[Row<'_, T>]) -> Result<(), &'static str> {
    c.admissions.set(c.admissions.get() + 1);
    for row in rows {
        c.authorize(row.reply.identity())?;
        if row.reply.identity().index != row.index {
            return Err("original identity");
        }
        let domain = match &row.reply {
            SavedSequenceReply::Prepared(member) => {
                admit_request(
                    c,
                    row.index,
                    &member.identity,
                    member.declaration,
                    &member.request,
                )?;
                member.declaration
            }
            SavedSequenceReply::Awaiting { declaration, .. } => *declaration,
            SavedSequenceReply::Accepted(reply) => {
                let (_, (), domain, value) = reply.as_parts();
                domain.validate_value(&*value.data)?;
                domain
            }
        };
        if domain.index != row.index || !Rc::ptr_eq(&domain.control, c) {
            return Err("original domain");
        }
    }
    Ok(())
}

#[test]
fn all_seven_saved_frontiers_resume_without_republishing_or_copying_the_prefix() {
    fn run<T: Data>() {
        for states in FRONTIERS {
            let c = Rc::new(Control::default());
            let node = group(3);
            let domains = domains(&c, 3);
            let saved = rows::<T>(&node, &domains, &c, &states);
            let pointers: Vec<_> = saved
                .iter()
                .filter_map(|row| match &row.reply {
                    SavedSequenceReply::Accepted(reply) => {
                        Some((row.index, &*reply.as_parts().3.data as *const T))
                    }
                    _ => None,
                })
                .collect();
            let request_pointers: Vec<_> = saved
                .iter()
                .filter_map(|row| match &row.reply {
                    SavedSequenceReply::Prepared(member) => {
                        Some((row.index, &*member.request.index as *const usize))
                    }
                    _ => None,
                })
                .collect();
            let (mut session, mut accepted) =
                Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).unwrap();
            let prefix = states.iter().take_while(|state| **state == 0).count();
            assert_eq!(accepted.len(), prefix);
            for (index, state) in states.iter().enumerate().skip(prefix) {
                let polled = session
                    .poll(|i, _, id, domain, req| admit_request(&c, i, id, domain, req))
                    .unwrap();
                match polled {
                    SequencePoll::Request { index: i, request } => {
                        assert_eq!(i, index);
                        assert_eq!(*state, 2);
                        assert_eq!(
                            Some(&*request.index as *const usize),
                            request_pointers
                                .iter()
                                .find(|(i, _)| *i == index)
                                .map(|(_, p)| *p)
                        );
                        c.publications.borrow_mut().push(index);
                    }
                    SequencePoll::Awaiting { index: i } => {
                        assert_eq!(i, index);
                        assert_eq!(*state, 1);
                    }
                    _ => panic!(),
                }
                assert!(matches!(
                    session
                        .poll(|_, _, _, _, _| -> Result<(), ()> { panic!("waiting rechecked") })
                        .unwrap(),
                    SequencePoll::Awaiting { .. }
                ));
                let reply = value::<T>(&c, index);
                let pointer = &*reply.data as *const T;
                let rejected = session
                    .try_accept::<_, T, _>(&key(&c, index + 9), reply, &*c)
                    .unwrap_err();
                assert_eq!(&*rejected.reply.data as *const T, pointer);
                accepted.push(
                    session
                        .try_accept::<_, T, _>(&key(&c, index), rejected.reply, &*c)
                        .unwrap(),
                );
                assert!(
                    session
                        .try_accept::<_, T, _>(&key(&c, index), value::<T>(&c, index), &*c)
                        .is_err()
                );
            }
            assert_eq!(
                session.status(),
                SequenceStatus::Terminal(SequenceEnd::Completed)
            );
            assert!(matches!(
                session
                    .poll(|_, _, _, _, _| -> Result<(), ()> { panic!() })
                    .unwrap(),
                SequencePoll::Terminal(SequenceEnd::Completed)
            ));
            assert!(!session.cancel());
            let mut fuel = Fuel::new(4);
            let reply =
                assemble_group_reply(&node, accepted, LIMITS, &mut fuel, |_, _| Ok::<_, ()>(()))
                    .unwrap();
            assert_eq!(fuel.remaining(), 0);
            for (index, result) in reply.members().iter().enumerate() {
                let (position, original, id, domain, value) = result.as_parts();
                assert_eq!(position, index);
                assert_eq!(id.index, index);
                assert!(std::ptr::eq(original, &branches(&node)[index]));
                assert!(std::ptr::eq(domain, &domains[index]));
                assert_eq!(*value.data, T::sample(index));
                if let Some((_, pointer)) = pointers.iter().find(|(i, _)| *i == index) {
                    assert_eq!(&*value.data as *const T, *pointer);
                }
            }
            assert_eq!(
                *c.publications.borrow(),
                states
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| **s == 2)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            );
            assert_eq!(c.admissions.get(), 1);
            assert_eq!(c.preparations.get(), 0);
        }
    }
    run::<u64>();
    run::<String>();
}

#[test]
fn all_twenty_noncontiguous_frontiers_refuse_before_identity_or_native_admission() {
    let mut rejected = 0;
    for encoded in 0..27 {
        let states = [encoded % 3, (encoded / 3) % 3, encoded / 9];
        if FRONTIERS.contains(&states) {
            continue;
        }
        let c = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&c, 3);
        let saved = rows::<u64>(&node, &domains, &c, &states);
        c.comparisons.set(0);
        assert!(matches!(
            Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)),
            Err(SequenceRestoreError::Frontier { .. })
        ));
        assert_eq!(c.comparisons.get(), 0);
        assert_eq!(c.admissions.get(), 0);
        rejected += 1;
    }
    assert_eq!(rejected, 20);
}

#[test]
fn saved_shape_count_and_order_fail_before_native_work() {
    for mode in 0..7 {
        let c = Rc::new(Control::default());
        let mut node = group(3);
        if mode == 6 {
            let Node::Group { group_kind, .. } = &mut node else {
                panic!()
            };
            *group_kind = GroupKind::Parallel;
        }
        let domains = domains(&c, 3);
        let mut saved = rows::<u64>(&node, &domains, &c, &[2, 2, 2]);
        let mut limits = LIMITS;
        match mode {
            0 => {
                saved.pop();
            }
            1 => saved[1].index = 2,
            2 => saved.swap(0, 2),
            3 => saved[2].name = "forged",
            4 => limits.max_nodes = 3,
            5 => limits.max_branches = 65,
            6 => {}
            _ => unreachable!(),
        }
        assert!(Session::restore(&node, saved, limits, |rows| admit(&c, rows)).is_err());
        assert_eq!(c.admissions.get(), 0);
        assert_eq!(c.comparisons.get(), 0);
        assert_eq!(c.identities_dropped.get(), 3);
        assert_eq!(c.requests_dropped.get(), 3);
    }
}

#[test]
fn duplicate_prefix_pending_and_queued_identities_never_reach_snapshot_admission() {
    for duplicate in [1, 2] {
        let c = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&c, 3);
        let mut saved = rows::<u64>(&node, &domains, &c, &[0, 1, 2]);
        match &mut saved[duplicate].reply {
            SavedSequenceReply::Awaiting { identity, .. } => *identity = key(&c, 0),
            SavedSequenceReply::Prepared(member) => member.identity = key(&c, 0),
            _ => panic!(),
        }
        assert!(matches!(
            Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)),
            Err(SequenceRestoreError::DuplicateIdentity { .. })
        ));
        assert_eq!(c.admissions.get(), 0);
        assert_eq!(c.values_dropped.get(), 1);
        assert_eq!(c.requests_dropped.get(), 1);
    }
}

#[test]
fn restored_snapshot_native_refusals_and_unwind_release_all_owned_parts() {
    for mode in 0..7 {
        let c = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&c, 3);
        let mut saved = rows::<u64>(&node, &domains, &c, &[0, 1, 2]);
        if mode == 0 {
            c.denied.set(true);
        }
        if mode == 1 {
            c.equality_unwind.set(true);
        }
        if mode == 4 {
            c.cleanup_unwind.set(true);
        }
        if mode == 5 {
            let SavedSequenceReply::Prepared(member) = &mut saved[2].reply else {
                panic!()
            };
            member.declaration = &domains[1];
        }
        if mode == 6 {
            let SavedSequenceReply::Prepared(member) = &mut saved[2].reply else {
                panic!()
            };
            *member.request.index = 99;
        }
        let before = c.identities_dropped.get();
        let result = catch_unwind(AssertUnwindSafe(|| {
            Session::restore(&node, saved, LIMITS, |rows| {
                assert_ne!(mode, 2, "native snapshot unwind");
                if mode == 3 || mode == 4 {
                    return Err("native refusal");
                }
                admit(&c, rows)
            })
        }));
        if [1, 2, 4].contains(&mode) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(c.identities_dropped.get() - before, 3);
        assert_eq!(c.requests_dropped.get(), 1);
        assert_eq!(c.values_dropped.get(), 1);
    }
}

#[test]
fn restored_publication_and_acceptance_reuse_live_policy_and_terminal_cleanup() {
    for mode in 0..5 {
        let c = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&c, 3);
        let saved = rows::<u64>(
            &node,
            &domains,
            &c,
            if mode < 2 { &[0, 2, 2] } else { &[0, 1, 2] },
        );
        let (mut session, prefix) =
            Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).unwrap();
        let scope = LocalExecutionScope::new();
        match mode {
            0 => {
                c.denied.set(true);
                assert!(
                    session
                        .poll(|i, _, id, domain, req| admit_request(&c, i, id, domain, req))
                        .is_err()
                );
                assert_eq!(
                    session.status(),
                    SequenceStatus::Terminal(SequenceEnd::Failed)
                );
            }
            1 => {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| session.poll_in_scope(
                        |_, _, _, _, _| -> Result<(), ()> { panic!("publication unwind") },
                        &scope
                    )))
                    .is_err()
                );
                assert!(!scope.is_open());
                assert_eq!(
                    session.status(),
                    SequenceStatus::Terminal(SequenceEnd::HostUncertain)
                );
            }
            2 => {
                c.authority_unwind.set(true);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| session
                        .try_accept_in_scope::<_, u64, _>(
                            &key(&c, 1),
                            value::<u64>(&c, 1),
                            &*c,
                            &scope
                        )))
                    .is_err()
                );
                assert!(!scope.is_open());
                assert_eq!(
                    session.status(),
                    SequenceStatus::Terminal(SequenceEnd::HostUncertain)
                );
            }
            3 => {
                scope.close();
                assert!(matches!(
                    session
                        .poll_in_scope(|_, _, _, _, _| -> Result<(), ()> { panic!() }, &scope)
                        .unwrap(),
                    SequencePoll::Terminal(SequenceEnd::Cancelled)
                ));
            }
            4 => {
                assert!(session.cancel());
            }
            _ => unreachable!(),
        }
        assert!(!session.cancel());
        assert_eq!(c.requests_dropped.get(), if mode < 2 { 2 } else { 1 });
        // Already handed-off prefix ownership remains with the host, not the session.
        assert_eq!(prefix.len(), 1);
        assert_eq!(*prefix[0].as_parts().4.data, 40);
        drop(prefix);
        assert_eq!(c.values_dropped.get(), if mode == 2 { 2 } else { 1 });
    }
}

#[test]
fn completed_maximum_sequence_is_terminal_and_unsized_domains_remain_supported() {
    let c = Rc::new(Control::default());
    let node = group(64);
    let domains = domains(&c, 64);
    let saved = rows::<u64>(&node, &domains, &c, &[0; 64]);
    let saved: Vec<
        SavedSequenceMember<'_, _, dyn HostResultDomain<u64, Error = &'static str>, Request, _>,
    > = saved
        .into_iter()
        .map(|row| {
            let SavedSequenceReply::Accepted(reply) = row.reply else {
                panic!()
            };
            let (id, (), domain, value) = reply.into_parts();
            let actual = key(&c, id.index);
            let domain: &dyn HostResultDomain<u64, Error = &'static str> = domain;
            let accepted = PendingReply::new(id, (), domain)
                .try_accept(&actual, value, &*c)
                .unwrap();
            SavedSequenceMember {
                index: row.index,
                name: row.name,
                reply: SavedSequenceReply::Accepted(accepted),
            }
        })
        .collect();
    let (session, prefix) =
        PreparedSequenceSession::restore(&node, saved, LIMITS, |_| Ok::<_, ()>(())).unwrap();
    let saved = session.try_into_saved(prefix).unwrap();
    let (mut session, prefix) =
        PreparedSequenceSession::restore(&node, saved, LIMITS, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(prefix.len(), 64);
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
    assert!(!session.cancel());
    assert!(matches!(
        session
            .poll(|_, _, _, _, _| -> Result<(), ()> { panic!() })
            .unwrap(),
        SequencePoll::Terminal(SequenceEnd::Completed)
    ));
    drop(prefix);
    assert_eq!(c.values_dropped.get(), 64);
}

mod transfer {
    use super::*;

    type Prefix<'a, T> = Vec<SequenceAccepted<'a, Node, (), Identity, Domain, Value<T>>>;
    struct Live<'a, T> {
        session: Session<'a>,
        prefix: Prefix<'a, T>,
        flying: Option<Request>,
        fuel: Fuel,
        request_pointers: Vec<*const usize>,
    }
    fn publish(c: &Rc<Control>, session: &mut Session<'_>) -> Request {
        let SequencePoll::Request { index, request } = session
            .poll(|i, _, id, domain, req| admit_request(c, i, id, domain, req))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(index, *request.index);
        c.publications.borrow_mut().push(index);
        request
    }
    fn live<'a, T: Data>(
        node: &'a Node,
        domains: &'a [Domain],
        c: &Rc<Control>,
        cut: usize,
    ) -> Live<'a, T> {
        let mut fuel = Fuel::new(1000);
        let prepared = prepare_native_group_members(
            GroupKind::Sequence,
            branches(node),
            64,
            &mut fuel,
            |row| Ok::<_, ()>(row.name.as_str()),
            |_| Ok(()),
            |index, _, fuel| {
                fuel.charge(1).unwrap();
                c.preparations.set(c.preparations.get() + 1);
                Ok(request(c, index))
            },
        )
        .unwrap();
        let request_pointers = prepared
            .members()
            .iter()
            .map(|row| &*row.prepared().index as *const usize)
            .collect();
        let mut session = Session::start(
            node,
            prepared,
            &mut fuel,
            LIMITS,
            |_, _, _| Ok::<_, ()>(()),
            |index, _, request, _| {
                Ok(PreparedSequenceMember {
                    identity: key(c, index),
                    declaration: &domains[index],
                    request,
                })
            },
        )
        .unwrap();
        let mut prefix = Vec::new();
        let mut flying = None;
        for step in 0..cut {
            let index = step / 2;
            if step % 2 == 0 {
                flying = Some(publish(c, &mut session));
            } else {
                drop(flying.take().unwrap());
                prefix.push(
                    session
                        .try_accept::<_, T, _>(&key(c, index), value::<T>(c, index), &**c)
                        .unwrap(),
                );
            }
        }
        Live {
            session,
            prefix,
            flying,
            fuel,
            request_pointers,
        }
    }

    mod scoped {
        use super::*;

        struct Probe(Rc<Control>, Rc<Cell<usize>>);
        impl Drop for Probe {
            fn drop(&mut self) {
                self.0.observe_cleanup();
                self.1.set(self.1.get() + 1);
            }
        }
        struct NativeError {
            marker: Box<u8>,
            _probe: Probe,
        }
        fn watch(c: &Rc<Control>) -> Rc<LocalExecutionScope> {
            let scope = Rc::new(LocalExecutionScope::new());
            c.restore_scope.replace(Some(scope.clone()));
            c.comparisons.set(0);
            c.admissions.set(0);
            scope
        }
        fn saved<'a>(node: &'a Node, domains: &'a [Domain], c: &Rc<Control>) -> Vec<Row<'a, u64>> {
            let live = live::<u64>(node, domains, c, 3);
            drop(live.flying);
            live.session.try_into_saved(live.prefix).unwrap()
        }

        #[test]
        fn scoped_seq_preflight_closes_before_rows_and_unused_hook_cleanup() {
            for fault in 0..6 {
                let c = Rc::new(Control::default());
                let node = group(3);
                let domains = domains(&c, 3);
                let mut rows = saved(&node, &domains, &c);
                let mut limits = LIMITS;
                match fault {
                    1 => limits.max_branches = 0,
                    2 => {
                        rows.pop();
                    }
                    3 => rows[0].index = 7,
                    4 => {
                        let (left, right) = rows.split_at_mut(1);
                        std::mem::swap(&mut left[0].reply, &mut right[0].reply);
                    }
                    5 => {
                        if let SavedSequenceReply::Awaiting { identity, .. } = &mut rows[1].reply {
                            identity.index = 0;
                        }
                    }
                    _ => {}
                }
                let scope = watch(&c);
                if fault == 0 {
                    scope.close();
                }
                let dropped = Rc::new(Cell::new(0));
                let probe = Probe(c.clone(), dropped.clone());
                let calls = &c.admissions;
                let output = Session::restore_in_scope(
                    &node,
                    rows,
                    limits,
                    move |_| {
                        let _keep = &probe;
                        calls.set(calls.get() + 1);
                        Ok::<_, ()>(())
                    },
                    &scope,
                );
                assert!(output.is_err());
                if fault == 0 {
                    assert!(matches!(output, Err(SequenceRestoreError::ScopeClosed)));
                }
                assert!(!scope.is_open());
                assert_eq!(c.admissions.get(), 0);
                assert_eq!(c.comparisons.get(), usize::from(fault == 5));
                assert_eq!(dropped.get(), 1);
                assert_eq!(c.cleanup_observed.get(), if fault == 2 { 4 } else { 6 });
                assert_eq!(c.cleanup_open.get(), 0);
            }
        }

        #[test]
        fn scoped_seq_equality_revocation_and_unwind_stop_before_admission() {
            for mode in 0..3 {
                let unwind = mode == 2;
                let c = Rc::new(Control::default());
                let node = group(3);
                let domains = domains(&c, 3);
                let mut rows = saved(&node, &domains, &c);
                if mode == 1 {
                    let SavedSequenceReply::Awaiting { identity, .. } = &mut rows[1].reply else {
                        panic!()
                    };
                    identity.index = 0;
                }
                let scope = watch(&c);
                c.equality_closes.set(!unwind);
                c.equality_unwind.set(unwind);
                let dropped = Rc::new(Cell::new(0));
                let probe = Probe(c.clone(), dropped.clone());
                let calls = &c.admissions;
                let output = catch_unwind(AssertUnwindSafe(|| {
                    Session::restore_in_scope(
                        &node,
                        rows,
                        LIMITS,
                        move |_| {
                            let _keep = &probe;
                            calls.set(calls.get() + 1);
                            Ok::<_, ()>(())
                        },
                        &scope,
                    )
                }));
                if unwind {
                    assert!(output.is_err());
                } else {
                    assert!(matches!(
                        output.unwrap(),
                        Err(SequenceRestoreError::ScopeClosed)
                    ));
                }
                assert!(!scope.is_open());
                assert_eq!(c.comparisons.get(), 1);
                assert_eq!(c.admissions.get(), 0);
                assert_eq!(dropped.get(), 1);
                assert_eq!(c.cleanup_observed.get(), 6);
                assert_eq!(c.cleanup_open.get(), 0);
            }
        }

        #[test]
        fn scoped_seq_native_refusal_revocation_and_cleanup_unwind_preserve_fencing() {
            for fault in 0..5 {
                let c = Rc::new(Control::default());
                let node = group(3);
                let domains = domains(&c, 3);
                let rows = saved(&node, &domains, &c);
                let scope = watch(&c);
                let dropped = Rc::new(Cell::new(0));
                let marker = Box::new(42);
                let pointer = &*marker as *const u8;
                let output = catch_unwind(AssertUnwindSafe(|| {
                    Session::restore_in_scope(
                        &node,
                        rows,
                        LIMITS,
                        |_| {
                            c.admissions.set(c.admissions.get() + 1);
                            if fault == 1 || fault == 2 {
                                scope.close();
                            }
                            assert_ne!(fault, 3, "native admission unwind");
                            if fault == 1 {
                                return Ok(());
                            }
                            if fault == 4 {
                                c.cleanup_unwind.set(true);
                            }
                            Err(NativeError {
                                marker,
                                _probe: Probe(c.clone(), dropped.clone()),
                            })
                        },
                        &scope,
                    )
                }));
                match fault {
                    0 => {
                        let Err(SequenceRestoreError::Native(error)) = output.unwrap() else {
                            panic!()
                        };
                        assert_eq!(&*error.marker as *const u8, pointer);
                        drop(error);
                    }
                    1 | 2 => assert!(matches!(
                        output.unwrap(),
                        Err(SequenceRestoreError::ScopeClosed)
                    )),
                    _ => assert!(output.is_err()),
                }
                assert!(!scope.is_open());
                assert_eq!(c.admissions.get(), 1);
                assert_eq!(dropped.get(), usize::from(matches!(fault, 0 | 2 | 4)));
                assert_eq!(c.cleanup_observed.get(), 5 + dropped.get());
                assert_eq!(c.cleanup_open.get(), 0);
            }
        }

        #[test]
        fn scoped_seq_success_retains_each_original_frontier_owner_without_meter_changes() {
            fn check<T: Data>() {
                for cut in 0..=6 {
                    let c = Rc::new(Control::default());
                    let node = group(3);
                    let domains = domains(&c, 3);
                    let live = live::<T>(&node, &domains, &c, cut);
                    let status = live.session.status();
                    let remaining = live.fuel.remaining();
                    drop(live.flying);
                    let rows = live.session.try_into_saved(live.prefix).unwrap();
                    let pointer = |row: &Row<'_, T>| match &row.reply {
                        SavedSequenceReply::Prepared(member) => {
                            &*member.request.index as *const usize as usize
                        }
                        SavedSequenceReply::Awaiting { .. } => 0,
                        SavedSequenceReply::Accepted(reply) => {
                            &*reply.as_parts().3.data as *const T as usize
                        }
                    };
                    let pointers: Vec<_> = rows.iter().map(pointer).collect();
                    let before = (
                        c.identities_dropped.get(),
                        c.requests_dropped.get(),
                        c.values_dropped.get(),
                    );
                    let scope = watch(&c);
                    let (session, prefix) = Session::restore_in_scope(
                        &node,
                        rows,
                        LIMITS,
                        |rows| admit(&c, rows),
                        &scope,
                    )
                    .unwrap();
                    assert_eq!(session.status(), status);
                    assert!(scope.is_open());
                    assert_eq!(live.fuel.remaining(), remaining);
                    let rows = session.try_into_saved(prefix).unwrap();
                    assert_eq!(rows.iter().map(pointer).collect::<Vec<_>>(), pointers);
                    assert_eq!(
                        (
                            c.identities_dropped.get(),
                            c.requests_dropped.get(),
                            c.values_dropped.get()
                        ),
                        before
                    );
                    scope.close();
                    drop(rows);
                    assert_eq!(c.cleanup_open.get(), 0);
                }
            }
            check::<u64>();
            check::<String>();
        }
    }

    #[test]
    fn actual_seq_frontiers_move_original_prefix_pending_and_suffix_without_native_callbacks() {
        fn check<T: Data>() {
            for count in [3, 64] {
                let cuts: Vec<_> = if count == 3 {
                    (0..=6).collect()
                } else {
                    vec![0, 1, 63, 64, 127, 128]
                };
                for cut in cuts {
                    let c = Rc::new(Control::default());
                    let node = group(count);
                    let domains = domains(&c, count);
                    let Live {
                        mut session,
                        mut prefix,
                        mut flying,
                        fuel,
                        request_pointers,
                    } = live::<T>(&node, &domains, &c, cut);
                    let remaining = fuel.remaining();
                    let pointers: Vec<_> = prefix
                        .iter()
                        .map(|row| &*row.as_parts().4.data as *const T)
                        .collect();
                    for _ in 0..3 {
                        let status = session.status();
                        let counts = (
                            c.comparisons.get(),
                            c.admissions.get(),
                            c.identities_dropped.get(),
                            c.requests_dropped.get(),
                            c.values_dropped.get(),
                        );
                        c.equality_unwind.set(true);
                        c.authority_unwind.set(true);
                        let saved = session.try_into_saved(prefix).unwrap();
                        assert_eq!(
                            (
                                c.comparisons.get(),
                                c.admissions.get(),
                                c.identities_dropped.get(),
                                c.requests_dropped.get(),
                                c.values_dropped.get()
                            ),
                            counts
                        );
                        c.equality_unwind.set(false);
                        c.authority_unwind.set(false);
                        for (index, row) in saved.iter().enumerate() {
                            assert_eq!(row.index, index);
                            assert!(std::ptr::eq(row.name, branches(&node)[index].name.as_str()));
                            let domain = match &row.reply {
                                SavedSequenceReply::Accepted(reply) => {
                                    assert!(index < cut / 2);
                                    assert_eq!(
                                        &*reply.as_parts().3.data as *const T,
                                        pointers[index]
                                    );
                                    reply.as_parts().2
                                }
                                SavedSequenceReply::Awaiting { declaration, .. } => {
                                    assert_eq!(cut % 2, 1);
                                    assert_eq!(index, cut / 2);
                                    *declaration
                                }
                                SavedSequenceReply::Prepared(member) => {
                                    assert!(index >= cut.div_ceil(2));
                                    assert_eq!(
                                        &*member.request.index as *const usize,
                                        request_pointers[index]
                                    );
                                    member.declaration
                                }
                            };
                            assert_eq!(row.reply.identity().index, index);
                            assert!(std::ptr::eq(domain, &domains[index]));
                        }
                        (session, prefix) =
                            Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).unwrap();
                        assert_eq!(session.status(), status);
                        assert_eq!(fuel.remaining(), remaining);
                    }
                    for (index, pointer) in request_pointers.iter().enumerate().skip(prefix.len()) {
                        let request = flying.take().unwrap_or_else(|| publish(&c, &mut session));
                        assert_eq!(&*request.index as *const usize, *pointer);
                        drop(request);
                        let input = value::<T>(&c, index);
                        let pointer = &*input.data as *const T;
                        let rejected = session
                            .try_accept::<_, T, _>(&key(&c, index + count), input, &*c)
                            .unwrap_err();
                        assert_eq!(&*rejected.reply.data as *const T, pointer);
                        prefix.push(
                            session
                                .try_accept::<_, T, _>(&key(&c, index), rejected.reply, &*c)
                                .unwrap(),
                        );
                    }
                    let saved = session.try_into_saved(prefix).unwrap();
                    assert!(
                        saved
                            .iter()
                            .all(|r| matches!(r.reply, SavedSequenceReply::Accepted(_)))
                    );
                    let (mut session, prefix) =
                        Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).unwrap();
                    assert!(matches!(
                        session
                            .poll(|_, _, _, _, _| -> Result<(), ()> { panic!() })
                            .unwrap(),
                        SequencePoll::Terminal(SequenceEnd::Completed)
                    ));
                    drop(prefix);
                    assert_eq!(*c.publications.borrow(), (0..count).collect::<Vec<_>>());
                    assert_eq!(c.preparations.get(), count);
                    assert_eq!(c.requests_dropped.get(), count);
                    assert_eq!(c.values_dropped.get(), count);
                    assert_eq!(fuel.remaining(), remaining);
                }
            }
        }
        check::<u64>();
        check::<String>();
    }

    #[test]
    fn malformed_seq_prefix_returns_both_owners_unchanged_before_native_equality() {
        for case in 0..3 {
            let c = Rc::new(Control::default());
            let node = group(3);
            let foreign = group(3);
            let domains = domains(&c, 3);
            let Live {
                session,
                mut prefix,
                ..
            } = live::<String>(&node, &domains, &c, 4);
            let Live {
                prefix: mut other, ..
            } = live::<String>(&foreign, &domains, &c, 4);
            let removed = match case {
                0 => prefix.pop(),
                1 => {
                    prefix.swap(0, 1);
                    None
                }
                _ => {
                    std::mem::swap(&mut prefix[0], &mut other[0]);
                    None
                }
            };
            let pointers: Vec<_> = prefix
                .iter()
                .map(|row| &*row.as_parts().4.data as *const String)
                .collect();
            let drops = (
                c.identities_dropped.get(),
                c.requests_dropped.get(),
                c.values_dropped.get(),
            );
            c.equality_unwind.set(true);
            let (session, mut prefix) = session.try_into_saved(prefix).unwrap_err();
            assert_eq!(session.status(), SequenceStatus::Ready { index: 2 });
            assert_eq!(
                prefix
                    .iter()
                    .map(|row| &*row.as_parts().4.data as *const String)
                    .collect::<Vec<_>>(),
                pointers
            );
            assert_eq!(
                (
                    c.identities_dropped.get(),
                    c.requests_dropped.get(),
                    c.values_dropped.get()
                ),
                drops
            );
            match case {
                0 => prefix.push(removed.unwrap()),
                1 => prefix.swap(0, 1),
                _ => std::mem::swap(&mut prefix[0], &mut other[0]),
            }
            let saved = session.try_into_saved(prefix).unwrap();
            c.equality_unwind.set(false);
            let (session, prefix) =
                Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).unwrap();
            assert_eq!(session.status(), SequenceStatus::Ready { index: 2 });
            assert_eq!(prefix.len(), 2);
        }
    }

    #[test]
    fn same_ir_seq_prefix_still_requires_original_execution_admission() {
        let c = Rc::new(Control::default());
        let foreign = Rc::new(Control::default());
        let node = group(3);
        let domains = domains(&c, 3);
        let foreign_domains = super::domains(&foreign, 3);
        let Live {
            session, prefix, ..
        } = live::<u64>(&node, &domains, &c, 2);
        let Live { prefix: other, .. } = live::<u64>(&node, &foreign_domains, &foreign, 2);
        let pointer = &*prefix[0].as_parts().4.data as *const u64;
        let saved = session.try_into_saved(other).unwrap();
        assert!(Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).is_err());
        assert_eq!(&*prefix[0].as_parts().4.data as *const u64, pointer);
        assert_eq!(*c.publications.borrow(), [0]);
        assert_eq!(*foreign.publications.borrow(), [0]);
    }

    #[test]
    fn seq_transfer_rejects_cancelled_failed_and_uncertain_owners_without_losing_prefix() {
        for case in 0..4 {
            let c = Rc::new(Control::default());
            let node = group(3);
            let domains = domains(&c, 3);
            let Live {
                mut session,
                prefix,
                ..
            } = live::<u64>(&node, &domains, &c, 2);
            let pointer = &*prefix[0].as_parts().4.data as *const u64;
            let scope = LocalExecutionScope::new();
            let end = match case {
                0 => {
                    session.cancel();
                    SequenceEnd::Cancelled
                }
                1 => {
                    assert!(session.poll(|_, _, _, _, _| Err(())).is_err());
                    SequenceEnd::Failed
                }
                2 => {
                    drop(publish(&c, &mut session));
                    c.authority_unwind.set(true);
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| session
                            .try_accept_in_scope::<_, u64, _>(
                                &key(&c, 1),
                                value::<u64>(&c, 1),
                                &*c,
                                &scope
                            )))
                        .is_err()
                    );
                    assert!(!scope.is_open());
                    SequenceEnd::HostUncertain
                }
                _ => {
                    scope.close();
                    assert!(matches!(
                        session
                            .poll_in_scope(|_, _, _, _, _| -> Result<(), ()> { panic!() }, &scope)
                            .unwrap(),
                        SequencePoll::Terminal(SequenceEnd::Cancelled)
                    ));
                    SequenceEnd::Cancelled
                }
            };
            let (mut session, prefix) = session.try_into_saved(prefix).unwrap_err();
            assert_eq!(session.status(), SequenceStatus::Terminal(end));
            assert_eq!(&*prefix[0].as_parts().4.data as *const u64, pointer);
            assert!(!session.cancel());
        }
    }

    #[test]
    fn transferred_seq_native_refusal_and_cleanup_unwind_release_each_retained_owner_once() {
        for mode in 0..3 {
            let c = Rc::new(Control::default());
            let node = group(3);
            let domains = domains(&c, 3);
            let Live {
                session,
                prefix,
                flying,
                fuel,
                ..
            } = live::<u64>(&node, &domains, &c, 3);
            let saved = session.try_into_saved(prefix).unwrap();
            let counts = (
                c.identities_dropped.get(),
                c.requests_dropped.get(),
                c.values_dropped.get(),
            );
            let remaining = fuel.remaining();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                if mode == 2 {
                    c.cleanup_unwind.set(true);
                    drop(saved);
                } else {
                    c.denied.set(mode == 0);
                    c.authority_unwind.set(mode == 1);
                    assert!(
                        Session::restore(&node, saved, LIMITS, |rows| admit(&c, rows)).is_err()
                    );
                }
            }));
            assert_eq!(outcome.is_err(), mode != 0);
            assert_eq!(
                (
                    c.identities_dropped.get(),
                    c.requests_dropped.get(),
                    c.values_dropped.get()
                ),
                (counts.0 + 3, counts.1 + 1, counts.2 + 1)
            );
            drop(flying);
            assert_eq!(c.requests_dropped.get(), 3);
            assert_eq!(fuel.remaining(), remaining);
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "state", deny_unknown_fields)]
enum WireState<T> {
    Accepted { value: T },
    Awaiting,
    Prepared { request: usize },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRow<T> {
    index: usize,
    name: String,
    identity: usize,
    reply: WireState<T>,
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
    bytes: Vec<u8>,
    settled: Option<Vec<u8>>,
}

fn checkpoint<T: Data>(c: &Rc<Control>, states: [u8; 3]) -> Journal {
    let node = group(3);
    let domains = domains(c, 3);
    let mut fuel = Fuel::new(18);
    let prepared = prepare_native_group_members(
        GroupKind::Sequence,
        branches(&node),
        3,
        &mut fuel,
        |row| Ok::<_, ()>(row.name.as_str()),
        |_| Ok(()),
        |index, _, fuel| {
            fuel.charge(1).unwrap();
            c.preparations.set(c.preparations.get() + 1);
            Ok(request(c, index))
        },
    )
    .unwrap();
    let mut session = Session::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |_, _, _| Ok::<_, ()>(()),
        |index, _, request, fuel| {
            fuel.charge(1).unwrap();
            Ok(PreparedSequenceMember {
                identity: key(c, index),
                declaration: &domains[index],
                request,
            })
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 4);
    let mut prefix = Vec::new();
    for (index, state) in states.iter().enumerate() {
        if *state != 2 {
            let SequencePoll::Request {
                index: published,
                request,
            } = session
                .poll(|i, _, id, domain, req| admit_request(c, i, id, domain, req))
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(published, index);
            assert_eq!(*request.index, index);
            c.publications.borrow_mut().push(index);
        }
        if *state == 0 {
            prefix.push(
                session
                    .try_accept::<_, T, _>(&key(c, index), value::<T>(c, index), &**c)
                    .unwrap(),
            );
        }
    }
    let saved = session.try_into_saved(prefix).unwrap();
    let rows = saved
        .iter()
        .map(|row| WireRow {
            index: row.index,
            name: row.name.to_owned(),
            identity: row.reply.identity().index,
            reply: match &row.reply {
                SavedSequenceReply::Accepted(reply) => WireState::Accepted {
                    value: reply.as_parts().3.data.as_ref(),
                },
                SavedSequenceReply::Awaiting { .. } => WireState::Awaiting,
                SavedSequenceReply::Prepared(member) => WireState::Prepared {
                    request: *member.request.index,
                },
            },
        })
        .collect();
    #[derive(Serialize)]
    struct ImageView<'a, T> {
        node: &'a Node,
        rows: Vec<WireRow<&'a T>>,
        fuel: u64,
        revision: u64,
    }
    Journal {
        bytes: serde_json::to_vec(&ImageView {
            node: &node,
            rows,
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
    c: &Rc<Control>,
    receive: bool,
) -> Result<Option<Vec<T>>, &'static str> {
    if candidate.len() > 65536 || candidate != journal.bytes {
        return Err("snapshot correspondence");
    }
    if let Some(bytes) = &journal.settled {
        return Ok(Some(serde_json::from_slice(bytes).unwrap()));
    }
    if c.denied.get() {
        return Err("authority");
    }
    let image: Image<T> = serde_json::from_slice(candidate).map_err(|_| "codec")?;
    if image.revision != 7 || image.fuel != 4 {
        return Err("budget/revision");
    }
    let names: Vec<_> = image.rows.iter().map(|row| row.name.clone()).collect();
    let domains = domains(c, image.rows.len());
    let mut saved = Vec::new();
    for (position, row) in image.rows.into_iter().enumerate() {
        let reply = match row.reply {
            WireState::Prepared { request: index } => {
                SavedSequenceReply::Prepared(PreparedSequenceMember {
                    identity: key(c, row.identity),
                    declaration: &domains[position],
                    request: request(c, index),
                })
            }
            WireState::Awaiting => SavedSequenceReply::Awaiting {
                identity: key(c, row.identity),
                declaration: &domains[position],
            },
            WireState::Accepted { value } => SavedSequenceReply::Accepted(
                PendingReply::new(key(c, row.identity), (), &domains[position])
                    .try_accept::<_, T, _>(
                        &key(c, row.identity),
                        Value {
                            data: Box::new(value),
                            control: c.clone(),
                        },
                        &**c,
                    )
                    .map_err(|_| "saved receipt")?,
            ),
        };
        saved.push(SavedSequenceMember {
            index: row.index,
            name: &names[position],
            reply,
        });
    }
    let (mut session, mut accepted) =
        Session::restore(&image.node, saved, LIMITS, |rows| admit(c, rows))
            .map_err(|_| "restore")?;
    let mut record: serde_json::Value = serde_json::from_slice(&journal.bytes).unwrap();
    let pending = match session
        .poll(|i, _, id, domain, req| admit_request(c, i, id, domain, req))
        .map_err(|_| "publication")?
    {
        SequencePoll::Request { index, request } => {
            assert_eq!(*request.index, index);
            c.publications.borrow_mut().push(index);
            record["rows"][index]["reply"] =
                serde_json::to_value(WireState::<T>::Awaiting).unwrap();
            journal.bytes = serde_json::to_vec(&record).unwrap();
            Some(index)
        }
        SequencePoll::Awaiting { index } => Some(index),
        SequencePoll::Terminal(SequenceEnd::Completed) => None,
        _ => return Err("terminal state"),
    };
    if let Some(index) = pending {
        if !receive {
            return Ok(None);
        }
        accepted.push(
            session
                .try_accept::<_, T, _>(&key(c, index), value::<T>(c, index), &**c)
                .map_err(|_| "receipt")?,
        );
        record["rows"][index]["reply"] = serde_json::to_value(WireState::Accepted {
            value: T::sample(index),
        })
        .unwrap();
        journal.bytes = serde_json::to_vec(&record).unwrap();
    }
    if session.status() != SequenceStatus::Terminal(SequenceEnd::Completed) {
        return Ok(None);
    }
    let mut fuel = Fuel::new(image.fuel);
    let reply = assemble_group_reply(&image.node, accepted, LIMITS, &mut fuel, |_, _| {
        Ok::<_, ()>(())
    })
    .map_err(|_| "assembly")?;
    assert_eq!(fuel.remaining(), 0);
    let output: Vec<_> = reply
        .members()
        .iter()
        .map(|row| serde_json::to_value(&*row.as_parts().4.data).unwrap())
        .collect();
    let bytes = serde_json::to_vec(&output).unwrap();
    journal.settled = Some(bytes.clone());
    record["fuel"] = serde_json::json!(fuel.remaining());
    journal.bytes = serde_json::to_vec(&record).unwrap();
    Ok(Some(serde_json::from_slice(&bytes).unwrap()))
}

#[test]
fn two_serialized_models_resume_every_frontier_without_repreparation_or_duplicate_publication() {
    fn run<T: Data>() {
        for states in FRONTIERS {
            let c = Rc::new(Control::default());
            let mut journal = checkpoint::<T>(&c, states);
            assert_eq!(c.preparations.get(), 3);
            let mut output = None;
            for _ in 0..4 {
                let bytes = journal.bytes.clone();
                if let Some(result) = recover::<T>(&mut journal, &bytes, &c, false).unwrap() {
                    output = Some(result);
                    break;
                }
                let published = c.publications.borrow().clone();
                let bytes = journal.bytes.clone();
                assert!(
                    recover::<T>(&mut journal, &bytes, &c, false)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(*c.publications.borrow(), published);
                let bytes = journal.bytes.clone();
                if let Some(result) = recover::<T>(&mut journal, &bytes, &c, true).unwrap() {
                    output = Some(result);
                    break;
                }
            }
            assert_eq!(output.unwrap(), (0..3).map(T::sample).collect::<Vec<_>>());
            assert_eq!(*c.publications.borrow(), vec![0, 1, 2]);
            assert_eq!(c.preparations.get(), 3);
            let admissions = c.admissions.get();
            let bytes = journal.bytes.clone();
            assert!(
                recover::<T>(&mut journal, &bytes, &c, true)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(c.admissions.get(), admissions);
            let image: Image<T> = serde_json::from_slice(&journal.bytes).unwrap();
            assert_eq!(image.fuel, 0);
        }
    }
    run::<u64>();
    run::<String>();
}

#[test]
fn serialized_graph_and_live_refusals_preserve_the_committed_sequence_frontier() {
    fn run<T: Data>() {
        let c = Rc::new(Control::default());
        let mut journal = checkpoint::<T>(&c, [0, 1, 2]);
        let bytes = journal.bytes.clone();
        let publications = c.publications.borrow().clone();
        for field in ["fuel", "revision", "rows", "node"] {
            let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            forged[field] = serde_json::json!(999);
            assert!(
                recover::<T>(
                    &mut journal,
                    &serde_json::to_vec(&forged).unwrap(),
                    &c,
                    true
                )
                .is_err()
            );
            assert_eq!(journal.bytes, bytes);
        }
        c.denied.set(true);
        assert!(recover::<T>(&mut journal, &bytes, &c, true).is_err());
        assert_eq!(journal.bytes, bytes);
        assert_eq!(*c.publications.borrow(), publications);
        assert_eq!(c.preparations.get(), 3);
        assert!(journal.settled.is_none());
    }
    run::<u64>();
    run::<String>();
}
