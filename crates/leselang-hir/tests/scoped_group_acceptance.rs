use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::prepare_native_group_members,
    parallel_evaluation::*,
    prepared_parallel::PreparedParallelSession,
    prepared_sequence::PreparedSequenceSession,
    sequence_evaluation::*,
};
use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionScope, ReplyAcceptanceError, ReplyAuthority, ReplyEnd,
};

const PHASES: [&str; 5] = ["identity", "authority", "borrow", "type", "value"];
const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 64,
    max_depth: 8,
    max_branches: 3,
};
struct Private;
#[derive(Default)]
struct Control {
    scope: LocalExecutionScope,
    events: RefCell<Vec<&'static str>>,
    close_at: Cell<Option<&'static str>>,
    refuse_at: Cell<Option<&'static str>>,
    unwind_at: Cell<Option<&'static str>>,
    cleanup_unwind: Cell<Option<&'static str>>,
    drops: RefCell<Vec<(&'static str, usize, bool)>>,
    preparations: RefCell<Vec<usize>>,
    interrupt_index: Cell<usize>,
    duplicate: Cell<bool>,
}
impl Control {
    fn visit(&self, phase: &'static str) -> bool {
        self.events.borrow_mut().push(phase);
        if self.close_at.get() == Some(phase) {
            self.scope.close();
        }
        assert_ne!(self.unwind_at.get(), Some(phase), "native callback unwind");
        self.refuse_at.get() != Some(phase)
    }
}
// Native owners deliberately have no Clone/Debug/serde/Send implementation.
struct Owned {
    kind: &'static str,
    index: usize,
    bytes: Box<[u8; 4]>,
    control: Rc<Control>,
}
impl Owned {
    fn new(kind: &'static str, index: usize, control: &Rc<Control>) -> Self {
        Self {
            kind,
            index,
            bytes: Box::new([1, 2, 3, 4]),
            control: control.clone(),
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        self.control
            .drops
            .borrow_mut()
            .push((self.kind, self.index, self.control.scope.is_open()));
        if self.control.cleanup_unwind.get() == Some(self.kind) {
            self.control.cleanup_unwind.set(None);
            panic!("native cleanup unwind");
        }
    }
}
impl PartialEq for Owned {
    fn eq(&self, other: &Self) -> bool {
        self.control.visit("identity")
            && (self.index == other.index || self.control.duplicate.get())
            && Rc::ptr_eq(&self.control, &other.control)
    }
}
impl Borrow<usize> for Owned {
    fn borrow(&self) -> &usize {
        self.control.visit("borrow");
        &self.index
    }
}
struct Domain {
    index: usize,
    control: Rc<Control>,
}
impl HostResultDomain<usize> for Domain {
    type Error = Private;
    fn matches_type(&self, _: &usize) -> bool {
        self.control.visit("type")
    }
    fn validate_value(&self, value: &usize) -> Result<(), Private> {
        if self.control.visit("value") && *value == self.index {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type Node = Computation<(), (), (), usize>;
type Branch = ComputedBranch<Node, usize>;
type Accepted<'a> = SequenceAccepted<'a, Node, usize, Owned, Domain, Owned>;
struct Environment<'a> {
    domains: &'a [Domain],
    control: Rc<Control>,
}
impl ReplyAuthority<Owned> for Environment<'_> {
    type Error = Private;
    fn authorize(&self, _: &Owned) -> Result<(), Private> {
        if self.control.visit("authority") {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
impl<'a> SequenceEvaluationEnvironment<'a, (), (), (), usize> for Environment<'a> {
    type Identity = Owned;
    type Declaration = Domain;
    type Request = Owned;
    type Error = Private;
    fn preflight_member(&self, _: usize, _: &'a Branch) -> Result<(), Private> {
        Ok(())
    }
    fn prepare_member(
        &self,
        index: usize,
        branch: &'a Branch,
        fuel: &mut Fuel,
    ) -> SequencePreparation<'a, Owned, Domain, Owned, Private> {
        self.control.preparations.borrow_mut().push(index);
        if index == self.control.interrupt_index.get() && !self.control.visit("prepare") {
            return Err(Private);
        }
        self.correlate(
            index,
            branch,
            Owned::new("request", index, &self.control),
            fuel,
        )
    }
}
impl<'a> Environment<'a> {
    fn correlate(
        &self,
        index: usize,
        branch: &'a Branch,
        request: Owned,
        fuel: &mut Fuel,
    ) -> SequencePreparation<'a, Owned, Domain, Owned, Private> {
        fuel.charge(1).map_err(|_| Private)?;
        assert_eq!(branch.result_type, index);
        Ok(PreparedSequenceMember {
            identity: Owned::new("identity", index, &self.control),
            declaration: &self.domains[index],
            request,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Entry {
    Sequence,
    Parallel,
    PreparedSequence,
    PreparedParallel,
}
const ENTRIES: [Entry; 4] = [
    Entry::Sequence,
    Entry::Parallel,
    Entry::PreparedSequence,
    Entry::PreparedParallel,
];
impl Entry {
    fn kind(self) -> GroupKind {
        match self {
            Self::Sequence | Self::PreparedSequence => GroupKind::Sequence,
            Self::Parallel | Self::PreparedParallel => GroupKind::Parallel,
        }
    }
}
enum Collector<'a> {
    Sequence(SequenceSession<'a, Node, usize, Owned, Domain>),
    Parallel(ParallelSession<'a, Node, usize, Owned, Domain, Owned>),
    PreparedSequence(PreparedSequenceSession<'a, Node, usize, Owned, Domain, Owned>),
    PreparedParallel(PreparedParallelSession<'a, Node, usize, Owned, Domain, Owned>),
}
#[derive(Debug)]
enum Rejection {
    Sequence(SequenceReplyError<Private, Private>),
    Parallel(ParallelReplyError<Private, Private>),
}
impl Rejection {
    fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Sequence(SequenceReplyError::Reply(ReplyAcceptanceError::Closed(
                ReplyEnd::Cancelled
            ))) | Self::Parallel(ParallelReplyError::Reply(ReplyAcceptanceError::Closed(
                ReplyEnd::Cancelled
            )))
        )
    }
    fn is_not_awaiting(&self) -> bool {
        matches!(
            self,
            Self::Sequence(SequenceReplyError::NotAwaiting(_))
                | Self::Parallel(ParallelReplyError::NotAwaiting(_))
        )
    }
}
impl<'a> Collector<'a> {
    fn start(
        entry: Entry,
        node: &'a Node,
        env: &Environment<'a>,
        fuel: &mut Fuel,
    ) -> (Self, Vec<Owned>) {
        let (mut collector, requests) = Self::ready(entry, node, env, fuel);
        if !requests.is_empty() {
            return (collector, requests);
        }
        let (requests, results) = collector.poll(env);
        assert!(results.is_empty());
        (collector, requests)
    }
    fn ready(
        entry: Entry,
        node: &'a Node,
        env: &Environment<'a>,
        fuel: &mut Fuel,
    ) -> (Self, Vec<Owned>) {
        let collector = match entry {
            Entry::Sequence => {
                Self::Sequence(SequenceSession::start(node, env, Fuel::new(1000), LIMITS).unwrap())
            }
            Entry::Parallel => {
                Self::Parallel(ParallelSession::start(node, env, Fuel::new(1000), LIMITS).unwrap())
            }
            Entry::PreparedSequence | Entry::PreparedParallel => {
                let Node::Group {
                    group_kind,
                    branches,
                } = node
                else {
                    panic!()
                };
                let prepared = prepare_native_group_members(
                    *group_kind,
                    branches,
                    3,
                    fuel,
                    |branch| Ok(branch.name.as_str()),
                    |_| Ok(()),
                    |index, _, fuel| {
                        fuel.charge(1).map_err(|_| Private)?;
                        Ok::<_, Private>(Owned::new("request", index, &env.control))
                    },
                )
                .unwrap();
                match entry {
                    Entry::PreparedSequence => Self::PreparedSequence(
                        PreparedSequenceSession::start(
                            node,
                            prepared,
                            fuel,
                            LIMITS,
                            |_, _, _| Ok(()),
                            |i, b, request, f| env.correlate(i, b, request, f),
                        )
                        .unwrap(),
                    ),
                    Entry::PreparedParallel => {
                        let (session, requests) = PreparedParallelSession::start(
                            node,
                            prepared,
                            fuel,
                            LIMITS,
                            |_, _, _| Ok(()),
                            |i, b, request, f| env.correlate(i, b, request, f),
                        )
                        .unwrap();
                        return (
                            Self::PreparedParallel(session),
                            requests.into_iter().map(|r| r.request).collect(),
                        );
                    }
                    _ => unreachable!(),
                }
            }
        };
        (collector, vec![])
    }
    fn fuel(&self, borrowed: &Fuel) -> u64 {
        match self {
            Self::Sequence(s) => s.fuel_remaining(),
            Self::Parallel(s) => s.fuel_remaining(),
            _ => borrowed.remaining(),
        }
    }
    fn end(&self) -> Option<SequenceEnd> {
        match self {
            Self::Sequence(s) => match s.status() {
                SequenceStatus::Terminal(end) => Some(end),
                _ => None,
            },
            Self::PreparedSequence(s) => match s.status() {
                SequenceStatus::Terminal(end) => Some(end),
                _ => None,
            },
            Self::Parallel(s) => Self::parallel_end(s.status()),
            Self::PreparedParallel(s) => Self::parallel_end(s.status()),
        }
    }
    fn parallel_end(status: ParallelStatus) -> Option<SequenceEnd> {
        match status {
            ParallelStatus::Terminal(end) => Some(match end {
                ParallelEnd::Completed => SequenceEnd::Completed,
                ParallelEnd::Cancelled => SequenceEnd::Cancelled,
                ParallelEnd::Failed => SequenceEnd::Failed,
                ParallelEnd::HostUncertain => SequenceEnd::HostUncertain,
            }),
            _ => None,
        }
    }
    fn accept(
        &mut self,
        index: usize,
        key: &Owned,
        reply: Owned,
        env: &Environment<'a>,
        scoped: bool,
    ) -> Result<Option<Accepted<'a>>, (Owned, Rejection)> {
        let scope = &env.control.scope;
        match self {
            Self::Sequence(s) => {
                let result = if scoped {
                    s.try_accept_in_scope(key, reply, env, scope)
                } else {
                    s.try_accept(key, reply, env)
                };
                result
                    .map(Some)
                    .map_err(|e| (e.reply, Rejection::Sequence(e.error)))
            }
            Self::PreparedSequence(s) => {
                let result = if scoped {
                    s.try_accept_in_scope(key, reply, env, scope)
                } else {
                    s.try_accept(key, reply, env)
                };
                result
                    .map(Some)
                    .map_err(|e| (e.reply, Rejection::Sequence(e.error)))
            }
            Self::Parallel(s) => {
                let result = if scoped {
                    s.try_accept_in_scope(index, key, reply, env, scope)
                } else {
                    s.try_accept(index, key, reply, env)
                };
                result
                    .map(|()| None)
                    .map_err(|e| (e.reply, Rejection::Parallel(e.error)))
            }
            Self::PreparedParallel(s) => {
                let result = if scoped {
                    s.try_accept_in_scope(index, key, reply, env, scope)
                } else {
                    s.try_accept(index, key, reply, env)
                };
                result
                    .map(|()| None)
                    .map_err(|e| (e.reply, Rejection::Parallel(e.error)))
            }
        }
    }
    fn poll(&mut self, env: &Environment<'a>) -> (Vec<Owned>, Vec<Accepted<'a>>) {
        self.poll_result(env, false).unwrap()
    }
    fn poll_result(
        &mut self,
        env: &Environment<'a>,
        scoped: bool,
    ) -> Result<(Vec<Owned>, Vec<Accepted<'a>>), ()> {
        fn sequence<'a>(poll: SequencePoll<Owned>) -> (Vec<Owned>, Vec<Accepted<'a>>) {
            match poll {
                SequencePoll::Request { request, .. } => (vec![request], vec![]),
                _ => (vec![], vec![]),
            }
        }
        fn parallel<'a, Request>(
            poll: ParallelPoll<Request, Accepted<'a>>,
        ) -> (Vec<Request>, Vec<Accepted<'a>>) {
            match poll {
                ParallelPoll::Requests(requests) => {
                    (requests.into_iter().map(|r| r.request).collect(), vec![])
                }
                ParallelPoll::Results(results) => (vec![], results),
                _ => (vec![], vec![]),
            }
        }
        let scope = &env.control.scope;
        Ok(match self {
            Self::Sequence(s) => sequence(
                if scoped {
                    s.poll_in_scope(env, scope)
                } else {
                    s.poll(env)
                }
                .map_err(|_| ())?,
            ),
            Self::PreparedSequence(s) => {
                let admit = |_: usize, _: &'a Branch, _: &Owned, _: &'a Domain, _: &Owned| {
                    if env.control.visit("publish") {
                        Ok(())
                    } else {
                        Err(Private)
                    }
                };
                sequence(
                    if scoped {
                        s.poll_in_scope(admit, scope)
                    } else {
                        s.poll(admit)
                    }
                    .map_err(|_| ())?,
                )
            }
            Self::Parallel(s) => parallel(
                if scoped {
                    s.poll_in_scope(env, scope)
                } else {
                    s.poll(env)
                }
                .map_err(|_| ())?,
            ),
            Self::PreparedParallel(s) => {
                let (requests, results) = parallel(
                    if scoped {
                        s.poll_in_scope(scope)
                    } else {
                        s.poll()
                    }
                    .map_err(|_| ())?,
                );
                assert!(requests.is_empty());
                (vec![], results)
            }
        })
    }
    fn cancel(&mut self) -> bool {
        match self {
            Self::Sequence(s) => s.cancel(),
            Self::Parallel(s) => s.cancel(),
            Self::PreparedSequence(s) => s.cancel(),
            Self::PreparedParallel(s) => s.cancel(),
        }
    }
}
fn fixture(entry: Entry, test: impl for<'a> FnOnce(&'a Node, &Environment<'a>)) {
    let control = Rc::new(Control::default());
    let node = Node::Group {
        group_kind: entry.kind(),
        branches: (0..3)
            .map(|index| Branch {
                name: format!("m{index}"),
                value: Node::Host {
                    effect: Box::new(()),
                },
                result_type: index,
            })
            .collect(),
    };
    let domains = (0..3)
        .map(|index| Domain {
            index,
            control: control.clone(),
        })
        .collect::<Vec<_>>();
    test(
        &node,
        &Environment {
            domains: &domains,
            control,
        },
    );
}
fn accept_one<'a>(
    collector: &mut Collector<'a>,
    index: usize,
    env: &Environment<'a>,
    scoped: bool,
) -> Option<Accepted<'a>> {
    collector
        .accept(
            index,
            &Owned::new("key", index, &env.control),
            Owned::new("reply", index, &env.control),
            env,
            scoped,
        )
        .unwrap_or_else(|_| panic!("valid reply rejected"))
}

#[test]
fn all_four_entries_cancel_at_each_native_phase_without_rearming_or_refilling() {
    for entry in ENTRIES {
        for (phase_index, phase) in PHASES.into_iter().enumerate() {
            fixture(entry, |node, env| {
                let sibling = LocalExecutionScope::new();
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
                let handed_off = accept_one(&mut collector, 0, env, true);
                let (next_requests, _) = if entry.kind() == GroupKind::Sequence {
                    collector.poll(env)
                } else {
                    (vec![], vec![])
                };
                env.control.events.borrow_mut().clear();
                env.control.drops.borrow_mut().clear();
                env.control.close_at.set(Some(phase));
                let before = collector.fuel(&fuel);
                let reply = Owned::new("reply", 1, &env.control);
                let pointer = reply.bytes.as_ptr();
                let (reply, error) = collector
                    .accept(1, &Owned::new("key", 1, &env.control), reply, env, true)
                    .err()
                    .unwrap();
                assert!(error.is_cancelled(), "{entry:?}: {error:?}");
                assert_eq!(reply.bytes.as_ptr(), pointer);
                assert_eq!(*env.control.events.borrow(), PHASES[..=phase_index]);
                assert_eq!(collector.end(), Some(SequenceEnd::Cancelled));
                assert!(!env.control.scope.is_open());
                assert!(sibling.is_open());
                assert_eq!(collector.fuel(&fuel), before);
                assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
                let identity_drops = env
                    .control
                    .drops
                    .borrow()
                    .iter()
                    .filter(|(kind, _, _)| *kind == "identity")
                    .count();
                assert_eq!(
                    identity_drops,
                    match entry {
                        Entry::Sequence => 1,
                        Entry::PreparedSequence => 2,
                        Entry::Parallel | Entry::PreparedParallel => 3,
                    }
                );
                if entry.kind() == GroupKind::Parallel {
                    assert!(
                        env.control
                            .drops
                            .borrow()
                            .iter()
                            .any(|&(kind, index, _)| kind == "reply" && index == 0)
                    );
                } else {
                    assert!(handed_off.is_some());
                    assert!(
                        !env.control
                            .drops
                            .borrow()
                            .iter()
                            .any(|&(kind, index, _)| kind == "reply" && index == 0)
                    );
                }
                let events = env.control.events.borrow().len();
                let (_, late_error) = collector
                    .accept(1, &Owned::new("key", 1, &env.control), reply, env, false)
                    .err()
                    .unwrap();
                assert!(late_error.is_not_awaiting());
                let (new_requests, results) = collector.poll(env);
                assert!(new_requests.is_empty() && results.is_empty());
                assert!(!collector.cancel());
                assert_eq!(env.control.events.borrow().len(), events);
                drop((requests, next_requests, handed_off));
            });
        }
    }
}

#[test]
fn scoped_refusal_preserves_original_input_domain_rows_prefix_and_ordered_completion() {
    for entry in ENTRIES {
        for phase in ["identity", "authority", "type", "value"] {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
                let (first, rejected_index, last) = if entry.kind() == GroupKind::Parallel {
                    (2, 0, 1)
                } else {
                    (0, 1, 2)
                };
                let mut results = accept_one(&mut collector, first, env, true)
                    .into_iter()
                    .collect::<Vec<_>>();
                if entry.kind() == GroupKind::Sequence {
                    collector.poll(env);
                }
                let before = collector.fuel(&fuel);
                env.control.refuse_at.set(Some(phase));
                let reply = Owned::new("reply", rejected_index, &env.control);
                let pointer = reply.bytes.as_ptr();
                let (reply, error) = collector
                    .accept(
                        rejected_index,
                        &Owned::new("key", rejected_index, &env.control),
                        reply,
                        env,
                        true,
                    )
                    .err()
                    .unwrap();
                assert!(!error.is_cancelled() && !error.is_not_awaiting());
                assert_eq!(reply.bytes.as_ptr(), pointer);
                assert!(env.control.scope.is_open());
                assert_eq!(collector.end(), None);
                assert_eq!(collector.fuel(&fuel), before);
                env.control.refuse_at.set(None);
                results.extend(
                    collector
                        .accept(
                            rejected_index,
                            &Owned::new("key", rejected_index, &env.control),
                            reply,
                            env,
                            true,
                        )
                        .unwrap_or_else(|_| panic!()),
                );
                if entry.kind() == GroupKind::Sequence {
                    let (published, _) = collector.poll(env);
                    assert_eq!(published.len(), 1);
                    assert_eq!(published[0].index, last);
                }
                results.extend(accept_one(&mut collector, last, env, true));
                let (_, batch) = collector.poll(env);
                results.extend(batch);
                assert_eq!(results.len(), 3);
                let Node::Group { branches, .. } = node else {
                    panic!()
                };
                for (index, result) in results.iter().enumerate() {
                    let (actual, branch, identity, domain, reply) = result.as_parts();
                    assert_eq!((actual, identity.index, reply.index), (index, index, index));
                    assert!(std::ptr::eq(branch, &branches[index]));
                    assert!(std::ptr::eq(domain, &env.domains[index]));
                }
                assert_eq!(results[rejected_index].as_parts().4.bytes.as_ptr(), pointer);
                assert_eq!(collector.end(), Some(SequenceEnd::Completed));
                assert!(env.control.scope.is_open());
                env.control.scope.close();
                let (_, error) = collector
                    .accept(
                        0,
                        &Owned::new("key", 0, &env.control),
                        Owned::new("reply", 0, &env.control),
                        env,
                        true,
                    )
                    .err()
                    .unwrap();
                assert!(error.is_not_awaiting());
                assert_eq!(collector.end(), Some(SequenceEnd::Completed));
                drop(requests);
            });
        }
    }
}

#[test]
fn preclosed_scope_cancels_even_invalid_member_or_ready_results_without_native_hooks() {
    for entry in ENTRIES {
        for completed in 0..=3 {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
                let mut results = Vec::new();
                for index in 0..completed {
                    if index > 0 && entry.kind() == GroupKind::Sequence {
                        collector.poll(env);
                    }
                    results.extend(accept_one(&mut collector, index, env, true));
                }
                env.control.scope.close();
                env.control.events.borrow_mut().clear();
                let (_, error) = collector
                    .accept(
                        usize::MAX,
                        &Owned::new("key", 0, &env.control),
                        Owned::new("reply", 0, &env.control),
                        env,
                        true,
                    )
                    .err()
                    .unwrap();
                assert!(error.is_not_awaiting());
                let expected = if completed == 3 && entry.kind() == GroupKind::Sequence {
                    SequenceEnd::Completed
                } else {
                    SequenceEnd::Cancelled
                };
                assert_eq!(collector.end(), Some(expected));
                assert!(env.control.events.borrow().is_empty());
                let (published, retained) = collector.poll(env);
                assert!(published.is_empty() && retained.is_empty());
                drop((requests, results));
            });
        }
    }
}

#[test]
fn scoped_callback_unwind_revokes_before_pending_suffix_and_accepted_sibling_cleanup() {
    for entry in ENTRIES {
        for phase in PHASES {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
                let accepted = accept_one(&mut collector, 0, env, true);
                if entry.kind() == GroupKind::Sequence {
                    collector.poll(env);
                }
                env.control.drops.borrow_mut().clear();
                env.control.unwind_at.set(Some(phase));
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    accept_one(&mut collector, 1, env, true);
                }));
                env.control.unwind_at.set(None);
                assert!(panic.is_err());
                assert_eq!(collector.end(), Some(SequenceEnd::HostUncertain));
                assert!(!env.control.scope.is_open());
                assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
                assert!(!collector.cancel());
                drop((accepted, requests));
            });
        }
    }
}

#[test]
fn scoped_cancellation_seal_survives_retained_owner_cleanup_unwind() {
    for (entry, cleanup, expected) in [
        (Entry::Sequence, "identity", SequenceEnd::HostUncertain),
        (
            Entry::PreparedSequence,
            "identity",
            SequenceEnd::HostUncertain,
        ),
        (Entry::PreparedSequence, "request", SequenceEnd::Cancelled),
        (Entry::Parallel, "reply", SequenceEnd::Cancelled),
        (Entry::PreparedParallel, "reply", SequenceEnd::Cancelled),
    ] {
        fixture(entry, |node, env| {
            let mut fuel = Fuel::new(1000);
            let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
            let accepted = accept_one(&mut collector, 0, env, true);
            if entry.kind() == GroupKind::Sequence {
                collector.poll(env);
            }
            env.control.drops.borrow_mut().clear();
            env.control.close_at.set(Some("authority"));
            env.control.cleanup_unwind.set(Some(cleanup));
            let panic = catch_unwind(AssertUnwindSafe(|| {
                accept_one(&mut collector, 1, env, true);
            }));
            // Disarm native test panics before assertions to avoid double unwind.
            env.control.cleanup_unwind.set(None);
            assert!(panic.is_err());
            assert_eq!(collector.end(), Some(expected));
            assert!(!env.control.scope.is_open());
            assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
            assert!(!collector.cancel());
            drop((accepted, requests));
        });
    }
}

#[test]
fn unscoped_collectors_do_not_infer_an_unrelated_scope_policy() {
    for entry in ENTRIES {
        fixture(entry, |node, env| {
            let mut fuel = Fuel::new(1000);
            let (mut collector, requests) = Collector::start(entry, node, env, &mut fuel);
            env.control.events.borrow_mut().clear();
            env.control.close_at.set(Some("identity"));
            let accepted = accept_one(&mut collector, 0, env, false);
            assert!(!env.control.scope.is_open());
            assert_eq!(*env.control.events.borrow(), PHASES);
            assert_eq!(collector.end(), None);
            assert!(collector.cancel());
            drop((accepted, requests));
        });
    }
}

#[test]
fn scoped_poll_preclosed_ready_waiting_prefix_and_ready_results_never_publish_or_charge() {
    for entry in ENTRIES {
        for stage in 0..4 {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = if stage == 0 {
                    Collector::ready(entry, node, env, &mut fuel)
                } else {
                    Collector::start(entry, node, env, &mut fuel)
                };
                let mut handed_off = Vec::new();
                let count = match stage {
                    2 => 1,
                    3 => 3,
                    _ => 0,
                };
                for index in 0..count {
                    if index > 0 && entry.kind() == GroupKind::Sequence {
                        collector.poll(env);
                    }
                    handed_off.extend(accept_one(&mut collector, index, env, true));
                }
                let before = collector.fuel(&fuel);
                env.control.scope.close();
                env.control.events.borrow_mut().clear();
                env.control.drops.borrow_mut().clear();
                let preparations = env.control.preparations.borrow().len();
                let (published, results) = collector.poll_result(env, true).unwrap();
                assert!(published.is_empty() && results.is_empty());
                assert!(env.control.events.borrow().is_empty());
                assert_eq!(env.control.preparations.borrow().len(), preparations);
                assert_eq!(collector.fuel(&fuel), before);
                assert_eq!(
                    collector.end(),
                    Some(if stage == 3 && entry.kind() == GroupKind::Sequence {
                        SequenceEnd::Completed
                    } else {
                        SequenceEnd::Cancelled
                    })
                );
                assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
                assert!(!collector.cancel());
                drop((requests, handed_off));
            });
        }
    }
}

#[test]
fn scoped_publication_revocation_stops_after_original_callback_without_partial_batch_or_refund() {
    for (entry, phase, index) in [
        (Entry::Sequence, "prepare", 0),
        (Entry::Parallel, "prepare", 0),
        (Entry::Parallel, "prepare", 1),
        (Entry::Parallel, "prepare", 2),
        (Entry::Parallel, "identity", 0),
        (Entry::PreparedSequence, "publish", 0),
    ] {
        for refuse in [false, true] {
            fixture(entry, |node, env| {
                let sibling = LocalExecutionScope::new();
                let mut fuel = Fuel::new(1000);
                let (mut collector, requests) = Collector::ready(entry, node, env, &mut fuel);
                assert!(requests.is_empty());
                env.control.events.borrow_mut().clear();
                env.control.interrupt_index.set(index);
                env.control.close_at.set(Some(phase));
                if phase == "identity" {
                    env.control.duplicate.set(!refuse);
                }
                if refuse {
                    env.control.refuse_at.set(Some(phase));
                }
                let before = collector.fuel(&fuel);
                let (published, results) = collector.poll_result(env, true).unwrap();
                assert!(published.is_empty() && results.is_empty());
                assert_eq!(collector.end(), Some(SequenceEnd::Cancelled));
                assert!(!env.control.scope.is_open());
                assert!(sibling.is_open());
                let debit = if phase == "publish" {
                    0
                } else if phase == "identity" {
                    6
                } else {
                    (index * 2 + if refuse { 1 } else { 2 }) as u64
                };
                assert_eq!(collector.fuel(&fuel), before - debit);
                assert_eq!(
                    env.control
                        .events
                        .borrow()
                        .iter()
                        .filter(|&&p| p == phase)
                        .count(),
                    1
                );
                if phase == "prepare" {
                    assert_eq!(
                        *env.control.preparations.borrow(),
                        (0..=index).collect::<Vec<_>>()
                    );
                }
                assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
                let cleanup = env.control.drops.borrow().len();
                let events = env.control.events.borrow().len();
                collector.poll(env);
                assert!(!collector.cancel());
                assert_eq!(
                    (
                        env.control.drops.borrow().len(),
                        env.control.events.borrow().len()
                    ),
                    (cleanup, events)
                );
            });
        }
    }
}

#[test]
fn scoped_poll_failure_and_unwind_revoke_before_retained_prefix_or_suffix_cleanup() {
    for (entry, phase, index) in [
        (Entry::Sequence, "prepare", 0),
        (Entry::Parallel, "prepare", 1),
        (Entry::Parallel, "identity", 0),
        (Entry::PreparedSequence, "publish", 0),
    ] {
        for unwind in [false, true] {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, _) = Collector::ready(entry, node, env, &mut fuel);
                env.control.interrupt_index.set(index);
                if unwind {
                    env.control.unwind_at.set(Some(phase));
                } else if phase == "identity" {
                    env.control.duplicate.set(true);
                } else {
                    env.control.refuse_at.set(Some(phase));
                }
                let outcome = catch_unwind(AssertUnwindSafe(|| collector.poll_result(env, true)));
                env.control.unwind_at.set(None);
                assert!(if unwind {
                    outcome.is_err()
                } else {
                    matches!(outcome, Ok(Err(())))
                });
                assert_eq!(
                    collector.end(),
                    Some(if unwind {
                        SequenceEnd::HostUncertain
                    } else {
                        SequenceEnd::Failed
                    })
                );
                assert!(!env.control.scope.is_open());
                assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
                let before = collector.fuel(&fuel);
                let events = env.control.events.borrow().len();
                collector.poll(env);
                assert_eq!(collector.fuel(&fuel), before);
                assert_eq!(env.control.events.borrow().len(), events);
                assert!(!collector.cancel());
            });
        }
    }
}

#[test]
fn scoped_poll_cleanup_unwind_preserves_cancelled_or_conservative_uncertain_reason() {
    for (entry, phase, close, cleanup, expected) in [
        (
            Entry::Sequence,
            "prepare",
            true,
            "identity",
            SequenceEnd::Cancelled,
        ),
        (
            Entry::Parallel,
            "identity",
            true,
            "request",
            SequenceEnd::Cancelled,
        ),
        (
            Entry::PreparedSequence,
            "publish",
            true,
            "request",
            SequenceEnd::Cancelled,
        ),
        (
            Entry::Parallel,
            "prepare",
            false,
            "identity",
            SequenceEnd::HostUncertain,
        ),
    ] {
        fixture(entry, |node, env| {
            let mut fuel = Fuel::new(1000);
            let (mut collector, _) = Collector::ready(entry, node, env, &mut fuel);
            if close {
                env.control.close_at.set(Some(phase));
            } else {
                env.control.interrupt_index.set(1);
                env.control.refuse_at.set(Some(phase));
            }
            env.control.cleanup_unwind.set(Some(cleanup));
            let outcome = catch_unwind(AssertUnwindSafe(|| collector.poll_result(env, true)));
            env.control.cleanup_unwind.set(None);
            assert!(outcome.is_err());
            assert_eq!(collector.end(), Some(expected));
            assert!(!env.control.scope.is_open());
            assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
            assert!(!collector.cancel());
        });
    }
}

#[test]
fn scoped_prepared_poll_seals_before_superseded_error_cleanup_and_preserves_normal_error() {
    for close in [false, true] {
        fixture(Entry::PreparedSequence, |node, env| {
            let mut fuel = Fuel::new(1000);
            let (Collector::PreparedSequence(mut session), _) =
                Collector::ready(Entry::PreparedSequence, node, env, &mut fuel)
            else {
                panic!()
            };
            let error = Owned::new("error", 0, &env.control);
            let pointer = error.bytes.as_ptr();
            let before = fuel.remaining();
            if close {
                env.control.cleanup_unwind.set(Some("error"));
            }
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                session.poll_in_scope(
                    |_, _, _, _, _| {
                        if close {
                            env.control.scope.close();
                        }
                        Err(error)
                    },
                    &env.control.scope,
                )
            }));
            env.control.cleanup_unwind.set(None);
            if close {
                assert!(outcome.is_err());
                assert_eq!(
                    session.status(),
                    SequenceStatus::Terminal(SequenceEnd::Cancelled)
                );
            } else {
                let Err(leselang_hir::prepared_sequence::PreparedSequenceError::Sequence(
                    SequenceEvaluationError::Native { error, .. },
                )) = outcome.unwrap()
                else {
                    panic!()
                };
                assert_eq!(error.bytes.as_ptr(), pointer);
                assert_eq!(
                    session.status(),
                    SequenceStatus::Terminal(SequenceEnd::Failed)
                );
                drop(error);
            }
            assert!(!env.control.scope.is_open());
            assert_eq!(fuel.remaining(), before);
            assert!(env.control.drops.borrow().iter().all(|(_, _, live)| !live));
        });
    }
}

#[test]
fn scoped_and_unscoped_successful_polling_keep_original_rows_requests_and_exact_fuel() {
    for entry in ENTRIES {
        let mut budgets = Vec::new();
        for scoped in [false, true] {
            fixture(entry, |node, env| {
                let mut fuel = Fuel::new(1000);
                let (mut collector, mut requests) = Collector::ready(entry, node, env, &mut fuel);
                if requests.is_empty() {
                    requests = collector.poll_result(env, scoped).unwrap().0;
                }
                let mut rows = Vec::new();
                let mut trace = vec![collector.fuel(&fuel)];
                let order = if entry.kind() == GroupKind::Parallel {
                    [2, 0, 1]
                } else {
                    [0, 1, 2]
                };
                for index in order {
                    if requests.is_empty() {
                        requests = collector.poll_result(env, scoped).unwrap().0;
                    }
                    let request =
                        requests.remove(requests.iter().position(|r| r.index == index).unwrap());
                    assert_eq!(request.index, index);
                    rows.extend(accept_one(&mut collector, index, env, scoped));
                    trace.push(collector.fuel(&fuel));
                }
                rows.extend(collector.poll_result(env, scoped).unwrap().1);
                assert_eq!(collector.end(), Some(SequenceEnd::Completed));
                assert!(env.control.scope.is_open());
                assert_eq!(rows.len(), 3);
                let Node::Group { branches, .. } = node else {
                    panic!()
                };
                for (index, row) in rows.iter().enumerate() {
                    let (actual, branch, _, domain, _) = row.as_parts();
                    assert_eq!(actual, index);
                    assert!(std::ptr::eq(branch, &branches[index]));
                    assert!(std::ptr::eq(domain, &env.domains[index]));
                }
                budgets.push(trace);
            });
        }
        assert_eq!(budgets[0], budgets[1]);
    }
}

#[test]
fn unscoped_polling_does_not_adopt_an_unrelated_closed_scope() {
    for entry in ENTRIES {
        fixture(entry, |node, env| {
            let mut fuel = Fuel::new(1000);
            let (mut collector, requests) = Collector::ready(entry, node, env, &mut fuel);
            if let Entry::PreparedParallel = entry {
                for index in 0..3 {
                    accept_one(&mut collector, index, env, false);
                }
            }
            env.control.scope.close();
            let (published, results) = collector.poll(env);
            if let Entry::PreparedParallel = entry {
                assert_eq!(results.len(), 3);
            } else {
                assert!(!published.is_empty());
            }
            drop(requests);
        });
    }
}

#[test]
fn scoped_poll_exhaustion_closes_without_preparation_or_refund() {
    for entry in [Entry::Sequence, Entry::Parallel] {
        fixture(entry, |node, env| {
            let mut collector = match entry {
                Entry::Sequence => Collector::Sequence(
                    SequenceSession::start(node, env, Fuel::new(1), LIMITS).unwrap(),
                ),
                Entry::Parallel => Collector::Parallel(
                    ParallelSession::start(node, env, Fuel::new(1), LIMITS).unwrap(),
                ),
                _ => unreachable!(),
            };
            assert!(collector.poll_result(env, true).is_err());
            assert_eq!(collector.end(), Some(SequenceEnd::Failed));
            assert!(!env.control.scope.is_open());
            assert!(env.control.preparations.borrow().is_empty());
            assert_eq!(collector.fuel(&Fuel::new(99)), 0);
        });
    }
}
