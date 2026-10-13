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
        assert!(
            !self.control.equality_unwind.get(),
            "native equality unwind"
        );
        self.index == other.index && Rc::ptr_eq(&self.control, &other.control)
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
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
    let mut saved = Vec::new();
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
        let reply = match state {
            0 => {
                drop(
                    session
                        .try_accept::<_, T, _>(&key(c, index), value::<T>(c, index), &**c)
                        .unwrap(),
                );
                WireState::Accepted {
                    value: T::sample(index),
                }
            }
            1 => WireState::Awaiting,
            2 => WireState::Prepared { request: index },
            _ => unreachable!(),
        };
        saved.push(WireRow {
            index,
            name: branches(&node)[index].name.clone(),
            identity: index,
            reply,
        });
    }
    drop(session);
    Journal {
        bytes: serde_json::to_vec(&Image {
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
