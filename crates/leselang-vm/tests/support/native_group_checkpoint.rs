// Host-owned disk fixture for one pending Seq/All, its parent frames and a successor.
use super::super::checkpoint_acceptance::{Call, DiskProfile, Store, frame_site, limits, node_at};
use super::*;
use effect_stack::SavedEffectReturnFrame;
use parallel_evaluation::{SavedParallelMember, SavedParallelReply};
use sequence_evaluation::{SavedSequenceMember, SavedSequenceReply, SequenceEnd};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];
const MAX_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
enum SeqPhase {
    Ready,
    Awaiting,
    Settled,
}

#[derive(Clone, Copy)]
enum Cut {
    All { dispatched: usize, mask: usize },
    Seq { accepted: usize, phase: SeqPhase },
}
impl Cut {
    fn kind(self) -> GroupKind {
        match self {
            Self::All { .. } => GroupKind::Parallel,
            Self::Seq { .. } => GroupKind::Sequence,
        }
    }
}

fn source<P: GroupProfile>(kind: GroupKind) -> String {
    match kind {
        GroupKind::Parallel => P::SOURCE.into(),
        GroupKind::Sequence => P::SOURCE.replacen("= all(", "= seq(", 1),
    }
}

trait GroupProfile: DiskProfile {
    const SOURCE: &'static str;
    fn output() -> ScalarValue;
    fn inputs() -> Vec<ScalarValue>;
}
impl GroupProfile for Counter {
    const SOURCE: &str = r#"
fn step(n: integer) = counter.add(amount: n)
fn batch(n: integer) = all(a: step(n: n), b: step(n: add(left: n, right: 1)), c: step(n: add(left: n, right: 2)))
fn run(n: integer) = bind(group: batch(n: n), body: bind(last: member(value: group, name: "c"), body: step(n: add(left: field(value: last, name: "value"), right: 1))))
fn main() = bind(seed: add(left: 1, right: 1), body: bind(result: run(n: seed), body: add(left: result, right: 1)))
"#;
    fn output() -> ScalarValue {
        ScalarValue::Integer(20)
    }
    fn inputs() -> Vec<ScalarValue> {
        [2, 3, 4, 10]
            .into_iter()
            .map(ScalarValue::Integer)
            .collect()
    }
}
impl GroupProfile for Panel {
    const SOURCE: &str = r#"
fn step(text: string) = panel.set_text(text: text)
fn batch(text: string) = all(a: step(text: concat(left: text, right: "A")), b: step(text: concat(left: text, right: "B")), c: step(text: concat(left: text, right: "C")))
fn run(text: string) = bind(group: batch(text: text), body: bind(last: member(value: group, name: "c"), body: step(text: concat(left: field(value: last, name: "text"), right: "!"))))
fn main() = bind(seed: concat(left: "s", right: "eed"), body: bind(result: run(text: seed), body: concat(left: field(value: result, name: "text"), right: ".")))
"#;
    fn output() -> ScalarValue {
        ScalarValue::String("seedC!.".into())
    }
    fn inputs() -> Vec<ScalarValue> {
        ["seedA", "seedB", "seedC", "seedC!"]
            .into_iter()
            .map(|s| ScalarValue::String(s.into()))
            .collect()
    }
}
type Vm<'r, 'ir, P> = leselang_vm::native::Vm<
    'ir,
    'r,
    Node,
    View<'ir, P>,
    RestoredEffectBindings<'ir, View<'ir, P>>,
    Identity,
    Plan<'ir, P>,
    Receipt<'ir, P>,
>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    site: Vec<usize>,
    name: String,
    capture: Vec<(String, ScalarValue)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    index: usize,
    name: String,
    serial: u64,
    call: Call,
    published: bool,
    accepted: Option<Value>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    format: u32,
    kind: GroupKind,
    source: String,
    ir: Node,
    site: Vec<usize>,
    root_serial: u64,
    watermark: u64,
    revision: u64,
    catalog: u32,
    remaining: u64,
    limits: [usize; 6],
    frames: Vec<Frame>,
    members: Vec<Member>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recorded {
    serial: u64,
    input: ScalarValue,
    reply: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finished {
    value: Option<ScalarValue>,
    remaining: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    state: Value,
    replies: Vec<Recorded>,
    terminal: Option<Finished>,
}
fn ledger(store: &Store) -> Ledger {
    serde_json::from_slice(&store.read("group-ledger.json")).unwrap()
}
fn site(root: &Node, target: &Node) -> Vec<usize> {
    let mut pending = vec![(root, Vec::new())];
    while let Some((node, path)) = pending.pop() {
        if std::ptr::eq(node, target) {
            return path;
        }
        for (i, child) in node.children().enumerate() {
            let mut next = path.clone();
            next.push(i);
            pending.push((child, next));
        }
    }
    panic!("group must belong to the compiled IR")
}
fn cached<P: GroupProfile>(ledger: &Ledger, serial: u64) -> Result<Reply<P::Value>, Refusal> {
    let row = ledger
        .replies
        .iter()
        .find(|r| r.serial == serial)
        .ok_or(Refusal)?;
    Ok(Reply {
        value: P::decode(row.reply.clone())?,
        buffer: Box::new(71),
    })
}
// Dispatch order is a fixture host policy. Group root 1 never represents an external effect.
fn settle<P: GroupProfile>(
    host: &Host<'_, '_, P>,
    dispatch: Dispatch<'_, P>,
    store: &Store,
    ledger: &mut Ledger,
) -> Result<Reply<P::Value>, Refusal> {
    host.authorize(&dispatch.identity)?;
    let serial = dispatch.identity.serial;
    let input = dispatch
        .request
        .arguments()
        .first()
        .ok_or(Refusal)?
        .value
        .clone();
    if let Some(row) = ledger.replies.iter().find(|r| r.serial == serial) {
        if row.input != input {
            return Err(Refusal);
        }
        return cached::<P>(ledger, serial);
    }
    if serial != ledger.replies.len() as u64 + 2 {
        return Err(Refusal);
    }
    let reply = host.dispatch(dispatch)?;
    ledger.replies.push(Recorded {
        serial,
        input,
        reply: P::encode(&reply.value),
    });
    ledger.state = host.profile.state();
    store.write("group-ledger.json", ledger);
    Ok(reply)
}

fn live<P: GroupProfile>(budget: u64) -> Finished {
    live_kind::<P>(budget, GroupKind::Parallel)
}

fn live_kind<P: GroupProfile>(budget: u64, kind: GroupKind) -> Finished {
    let mut result = None;
    fixture::<P>(|host, scope| {
        let root = compile(&source::<P>(kind), &host).unwrap();
        let plans = plans(&root, &host);
        let runtime = Runtime {
            host: &host,
            plans: &plans,
        };
        let mut owner = scope.bind(Vm::new(&root, Vec::new(), Fuel::new(budget), STACK));
        loop {
            let (scope, vm) = owner.as_parts_mut();
            let value = match vm.poll_in_scope(&runtime, scope) {
                Ok(EffectSessionPoll::Request(active)) => {
                    let id = active.plan.id();
                    let reply = runtime.collect(active).unwrap();
                    vm.try_accept_in_scope(&id, reply, &runtime, scope).unwrap();
                    continue;
                }
                Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                    assert_eq!(value, P::output());
                    Some(value)
                }
                Err(_) => None,
                _ => panic!("unexpected live group state"),
            };
            assert!(!scope.is_open());
            assert_eq!(*host.invoked.borrow(), P::inputs());
            result = Some(Finished {
                value,
                remaining: vm.fuel_remaining(),
            });
            break;
        }
    });
    result.unwrap()
}

fn save<P: GroupProfile>(
    store: &Store,
    dispatched: usize,
    mask: usize,
    budget: u64,
) -> LocalExecutionLease {
    assert!(dispatched <= 3 && mask < 1 << dispatched);
    save_cut::<P>(store, Cut::All { dispatched, mask }, budget)
}

fn save_cut<P: GroupProfile>(store: &Store, cut: Cut, budget: u64) -> LocalExecutionLease {
    let mut old = None;
    fixture::<P>(|host, scope| {
        old = Some(scope.lease());
        let root = compile(&source::<P>(cut.kind()), &host).unwrap();
        let plans = plans(&root, &host);
        let runtime = Runtime {
            host: &host,
            plans: &plans,
        };
        let mut owner = scope.bind(Some(Vm::new(&root, Vec::new(), Fuel::new(budget), STACK)));
        let (scope, slot) = owner.as_parts_mut();
        let EffectSessionPoll::Request(active) = slot
            .as_mut()
            .unwrap()
            .poll_in_scope(&runtime, scope)
            .unwrap()
        else {
            panic!("pending group")
        };
        let mut ledger = Ledger {
            state: host.profile.state(),
            replies: Vec::new(),
            terminal: None,
        };
        store.write("group-ledger.json", &ledger);
        let members = match (cut, active.work) {
            (
                Cut::All { dispatched, mask },
                Work::Parallel {
                    mut barrier,
                    requests,
                },
            ) => {
                let calls: Vec<_> = requests
                    .iter()
                    .map(|r| Call::save(&r.request.request))
                    .collect();
                for request in requests.into_iter().take(dispatched) {
                    let index = request.index;
                    let id = request.request.identity.clone();
                    let reply = settle(&host, request.request, store, &mut ledger).unwrap();
                    if mask & (1 << index) != 0 {
                        barrier
                            .try_accept_in_scope(index, &id, reply, &host, scope)
                            .unwrap();
                    }
                }
                let rows = barrier.try_into_saved().unwrap();
                rows.into_iter()
                    .zip(calls)
                    .map(|(row, call)| {
                        let serial = row.reply.identity().serial;
                        let accepted = match row.reply {
                            SavedParallelReply::Accepted(reply) => {
                                Some(P::encode(&reply.as_parts().3.value))
                            }
                            SavedParallelReply::Awaiting { .. } => None,
                        };
                        Member {
                            index: row.index,
                            name: row.name.into(),
                            serial,
                            call,
                            published: true,
                            accepted,
                        }
                    })
                    .collect()
            }
            (Cut::Seq { accepted, phase }, Work::Sequence(mut queue)) => {
                assert!(accepted <= 3 && (accepted < 3 || phase == SeqPhase::Ready));
                let mut prefix = Vec::new();
                let mut calls: Vec<Option<Call>> = (0..3).map(|_| None).collect();
                for expected in 0..accepted + usize::from(phase != SeqPhase::Ready) {
                    let SequencePoll::Request { index, request } =
                        runtime.publish(&mut queue).unwrap()
                    else {
                        panic!("original next request")
                    };
                    assert_eq!(index, expected);
                    calls[index] = Some(Call::save(&request.request));
                    if index < accepted || phase == SeqPhase::Settled {
                        let id = request.identity.clone();
                        let reply = settle(&host, request, store, &mut ledger).unwrap();
                        if index < accepted {
                            prefix
                                .push(queue.try_accept_in_scope(&id, reply, &host, scope).unwrap());
                        }
                    }
                }
                queue
                    .try_into_saved(prefix)
                    .unwrap()
                    .into_iter()
                    .map(|row| {
                        let serial = row.reply.identity().serial;
                        let (published, accepted, call) = match row.reply {
                            SavedSequenceReply::Prepared(member) => {
                                (false, None, Call::save(&member.request.request))
                            }
                            SavedSequenceReply::Awaiting { .. } => {
                                (true, None, calls[row.index].take().unwrap())
                            }
                            SavedSequenceReply::Accepted(reply) => (
                                true,
                                Some(P::encode(&reply.as_parts().3.value)),
                                calls[row.index].take().unwrap(),
                            ),
                        };
                        Member {
                            index: row.index,
                            name: row.name.into(),
                            serial,
                            call,
                            published,
                            accepted,
                        }
                    })
                    .collect()
            }
            _ => panic!("checkpoint shape must match original source"),
        };
        let pending = slot.take().unwrap().try_into_pending_binding().unwrap();
        let frames = pending
            .continuation
            .into_frames()
            .into_iter()
            .map(|frame| {
                let (name, body, capture) = frame.into_parts();
                Frame {
                    site: frame_site(&root, name, body),
                    name: name.into(),
                    capture: capture
                        .into_iter()
                        .map(|(name, value)| {
                            let PureValue::Scalar(value) = value else {
                                panic!("this cut has only pure captured locals")
                            };
                            (name.into(), value)
                        })
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        assert!(frames.len() >= 2);
        store.write(
            "group-checkpoint.json",
            &Image {
                format: 2,
                kind: cut.kind(),
                source: source::<P>(cut.kind()),
                ir: root.clone(),
                site: site(&root, active.plan.node),
                root_serial: pending.identity.serial,
                watermark: host.serial.get(),
                revision: pending.identity.revision,
                catalog: host.version.get(),
                remaining: pending.fuel.remaining(),
                limits: limits(),
                frames,
                members,
            },
        );
    });
    let old = old.unwrap();
    assert!(!old.is_open());
    old
}

#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    ParentDenied,
    ChildDenied,
    ChildRevoked,
    Cancelled,
    PublicationDenied(usize),
    ExpectFrontierRefusal,
}

struct SequenceRecovery<'a, 'ir, 'h, 's, P> {
    host: &'a Host<'h, 's, P>,
    plan: &'a Plan<'ir, P>,
    scope: &'a LocalExecutionScope,
    image: &'a Image,
    ledger: &'a mut Ledger,
    store: &'a Store,
    old: &'a LocalExecutionLease,
    fault: Fault,
}
impl<'ir, 's, P: GroupProfile> SequenceRecovery<'_, 'ir, '_, 's, P> {
    fn identity(&self, serial: u64) -> Identity {
        Identity {
            owner: self.host.lease.clone(),
            serial,
            revision: self.image.revision,
        }
    }
    fn publish(
        &self,
        queue: &mut Sequence<'ir, 's, P>,
        published: &mut Vec<usize>,
    ) -> Result<SequencePoll<Dispatch<'s, P>>, Refusal> {
        queue
            .poll_in_scope(
                |index, original, id, domain, request| {
                    let Node::Group { branches, .. } = self.plan.node else {
                        return Err(Refusal);
                    };
                    assert!(
                        !self.image.members[index].published,
                        "awaiting work must not be republished"
                    );
                    if self.fault == Fault::PublicationDenied(index) {
                        self.host.granted.set(false);
                    }
                    self.host.authorize(id)?;
                    if !std::ptr::eq(original, &branches[index])
                        || *id != request.identity
                        || !std::ptr::eq(domain, &request.request.schema().result)
                    {
                        return Err(Refusal);
                    }
                    published.push(index);
                    Ok(())
                },
                self.scope,
            )
            .map_err(|_| Refusal)
    }
    fn resume(&mut self) -> Result<Vec<Accepted<'ir, P>>, Refusal> {
        let result = self.resume_inner();
        if result.is_err() {
            self.scope.close();
        }
        result
    }
    fn resume_inner(&mut self) -> Result<Vec<Accepted<'ir, P>>, Refusal> {
        let Node::Group { branches, .. } = self.plan.node else {
            return Err(Refusal);
        };
        let before = self.host.invoked.borrow().len();
        let mut awaiting = Vec::new();
        let rows = self
            .image
            .members
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.index != i
                    || row.name != branches[i].name
                    || row.serial != i as u64 + 2
                    || (row.accepted.is_some() && !row.published)
                {
                    return Err(Refusal);
                }
                let id = self.identity(row.serial);
                let request = row.call.restore(self.host, &id)?;
                let reply = if let Some(value) = &row.accepted {
                    let recorded = self
                        .ledger
                        .replies
                        .iter()
                        .find(|r| r.serial == row.serial)
                        .ok_or(Refusal)?;
                    if *value != recorded.reply {
                        return Err(Refusal);
                    }
                    SavedSequenceReply::Accepted(
                        PendingReply::new(id.clone(), (), self.plan.domain)
                            .try_accept_in_scope(
                                &id,
                                cached::<P>(self.ledger, row.serial)?,
                                self.host,
                                self.scope,
                            )
                            .map_err(|_| Refusal)?,
                    )
                } else if row.published {
                    awaiting.push((
                        i,
                        Dispatch {
                            identity: id.clone(),
                            request,
                        },
                    ));
                    SavedSequenceReply::Awaiting {
                        identity: id,
                        declaration: self.plan.domain,
                    }
                } else {
                    SavedSequenceReply::Prepared(PreparedSequenceMember {
                        identity: id.clone(),
                        declaration: self.plan.domain,
                        request: Dispatch {
                            identity: id,
                            request,
                        },
                    })
                };
                Ok(SavedSequenceMember {
                    index: i,
                    name: branches[i].name.as_str(),
                    reply,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let admitted = Cell::new(0);
        let restored = Sequence::restore_in_scope(
            self.plan.node,
            rows,
            BATCH,
            |rows| {
                admitted.set(admitted.get() + 1);
                assert!(
                    self.fault != Fault::ExpectFrontierRefusal,
                    "malformed frontier must fail before native admission"
                );
                if self.fault == Fault::ChildDenied {
                    return Err(Refusal);
                }
                if self.fault == Fault::ChildRevoked {
                    self.scope.close();
                }
                self.host.authorize(&self.plan.id())?;
                for (i, row) in rows.iter().enumerate() {
                    if *row.reply.identity() != self.identity(self.image.members[i].serial) {
                        return Err(Refusal);
                    }
                    self.host.authorize(row.reply.identity())?;
                }
                Ok(())
            },
            self.scope,
        );
        let (mut queue, mut prefix) = match restored {
            Ok(value) => value,
            Err(error) => {
                assert!(!self.scope.is_open());
                if self.fault == Fault::ExpectFrontierRefusal {
                    assert_eq!(admitted.get(), 0);
                    assert!(matches!(
                        error,
                        sequence_evaluation::SequenceRestoreError::Frontier { .. }
                    ));
                }
                return Err(Refusal);
            }
        };
        assert_eq!(admitted.get(), 1);
        assert!(awaiting.len() <= 1);
        assert_eq!(self.host.serial.get(), self.image.watermark);
        assert_eq!(self.host.invoked.borrow().len(), before);
        let mut published = Vec::new();
        if self.fault == Fault::Cancelled {
            self.scope.close();
            let end = if prefix.len() == 3 {
                SequenceEnd::Completed
            } else {
                SequenceEnd::Cancelled
            };
            assert!(
                matches!(self.publish(&mut queue, &mut published)?, SequencePoll::Terminal(actual) if actual == end)
            );
            assert!(published.is_empty());
            return Err(Refusal);
        }
        while prefix.len() < 3 {
            let expected = prefix.len();
            let (index, dispatch) = if let Some((index, request)) = awaiting.pop() {
                assert!(
                    matches!(self.publish(&mut queue, &mut published)?, SequencePoll::Awaiting { index: actual } if actual == index)
                );
                assert!(published.is_empty());
                (index, request)
            } else {
                let SequencePoll::Request { index, request } =
                    self.publish(&mut queue, &mut published)?
                else {
                    return Err(Refusal);
                };
                (index, request)
            };
            assert_eq!(index, expected);
            let count = published.len();
            assert!(
                matches!(self.publish(&mut queue, &mut published)?, SequencePoll::Awaiting { index: actual } if actual == index)
            );
            assert_eq!(published.len(), count);
            let id = dispatch.identity.clone();
            let mut reply = settle(self.host, dispatch, self.store, self.ledger)?;
            let wrong = self.identity(if index < 2 {
                id.serial + 1
            } else {
                id.serial - 1
            });
            for refused_id in [
                Identity {
                    owner: self.old.clone(),
                    ..id.clone()
                },
                wrong,
            ] {
                let pointer = &*reply.buffer as *const u8;
                reply = queue
                    .try_accept_in_scope(&refused_id, reply, self.host, self.scope)
                    .unwrap_err()
                    .reply;
                assert_eq!(&*reply.buffer as *const u8, pointer);
                assert!(
                    matches!(self.publish(&mut queue, &mut published)?, SequencePoll::Awaiting { index: actual } if actual == index)
                );
                assert_eq!(published.len(), count);
            }
            prefix.push(
                queue
                    .try_accept_in_scope(&id, reply, self.host, self.scope)
                    .unwrap(),
            );
            let duplicate = cached::<P>(self.ledger, id.serial)?;
            let pointer = &*duplicate.buffer as *const u8;
            let rejected = queue
                .try_accept_in_scope(&id, duplicate, self.host, self.scope)
                .unwrap_err();
            assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
            assert_eq!(
                published.len(),
                count,
                "acceptance must not publish its successor"
            );
            assert!(self.scope.is_open());
        }
        assert!(matches!(
            self.publish(&mut queue, &mut published)?,
            SequencePoll::Terminal(SequenceEnd::Completed)
        ));
        assert_eq!(
            published,
            self.image
                .members
                .iter()
                .filter(|m| !m.published)
                .map(|m| m.index)
                .collect::<Vec<_>>()
        );
        assert_eq!(self.host.serial.get(), self.image.watermark);
        Ok(prefix)
    }
}

fn reopen<P: GroupProfile>(
    store: &Store,
    candidate: &[u8],
    old: &LocalExecutionLease,
    order: [usize; 3],
    fault: Fault,
) -> Result<Finished, Refusal> {
    // Correspondence against trusted committed fixture bytes, not storage authentication.
    if candidate.len() > MAX_BYTES || candidate != store.read("group-checkpoint.json") {
        return Err(Refusal);
    }
    let image: Image = serde_json::from_slice(candidate).map_err(|_| Refusal)?;
    if image.format != 2
        || image.source != source::<P>(image.kind)
        || image.catalog != 31
        || image.revision != 1
        || image.root_serial != 1
        || image.watermark != 4
        || image.limits != limits()
        || image.members.len() != 3
        || image.frames.is_empty()
        || image.frames.len() > STACK.max_frames
    {
        return Err(Refusal);
    }
    let mut result = Err(Refusal);
    let mut new_lease = None;
    fixture::<P>(|host, scope| {
        new_lease = Some(scope.lease());
        result = (|| {
            let mut ledger = ledger(store);
            host.profile.load(ledger.state.clone())?;
            host.invoked
                .replace(ledger.replies.iter().map(|r| r.input.clone()).collect());
            host.serial.set(image.watermark);
            let identity = |serial| Identity {
                owner: host.lease.clone(),
                serial,
                revision: image.revision,
            };
            let id = identity(image.root_serial);
            host.authorize(&id)?;
            if let Some(done) = ledger.terminal {
                return Ok(done);
            }
            let root = compile(&image.source, &host).map_err(|_| Refusal)?;
            if root != image.ir {
                return Err(Refusal);
            }
            let plans = plans(&root, &host);
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let plan = runtime.plan(node_at(&root, &image.site)?)?;
            let Node::Group {
                group_kind,
                branches,
                ..
            } = plan.node
            else {
                return Err(Refusal);
            };
            if branches.len() != 3 || *group_kind != image.kind {
                return Err(Refusal);
            }
            plan.identity.replace(Some(id.clone()));
            plan.members
                .replace(image.members.iter().map(|m| identity(m.serial)).collect());
            let frames = image
                .frames
                .iter()
                .map(|f| {
                    let Node::Bind { name, body, .. } = node_at(&root, &f.site)? else {
                        return Err(Refusal);
                    };
                    if *name != f.name || f.capture.len() > PURE.max_bindings {
                        return Err(Refusal);
                    }
                    Ok(SavedEffectReturnFrame {
                        name: name.as_str(),
                        body: body.as_ref(),
                        capture: f
                            .capture
                            .iter()
                            .map(|(n, v)| (n.as_str(), PureValue::Scalar(v.clone())))
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut owner = scope.bind(None::<Vm<'_, '_, P>>);
            let (scope, slot) = owner.as_parts_mut();
            host.granted.set(fault != Fault::ParentDenied);
            let restored = Vm::restore_pending_in_scope(
                id.clone(),
                plan,
                frames,
                Fuel::new(image.remaining),
                STACK,
                |actual, declaration, _, fuel| {
                    runtime.authorize(actual)?;
                    if *actual != id
                        || !std::ptr::eq(declaration, plan)
                        || fuel.remaining() != image.remaining
                    {
                        return Err(Refusal);
                    }
                    Ok(())
                },
                scope,
            );
            let vm = match restored {
                Ok(vm) => slot.insert(vm),
                Err(_) => {
                    assert!(!scope.is_open());
                    return Err(Refusal);
                }
            };
            assert!(matches!(
                vm.poll_in_scope(&runtime, scope).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert!(!old.same_scope(&scope.lease()));
            let rows = match group_kind {
                GroupKind::Parallel => {
                    let mut requests = Vec::new();
                    let rows = image
                        .members
                        .iter()
                        .enumerate()
                        .map(|(i, row)| {
                            if row.index != i
                                || row.name != branches[i].name
                                || row.serial != i as u64 + 2
                                || !row.published
                            {
                                return Err(Refusal);
                            }
                            let id = identity(row.serial);
                            requests.push(Some(row.call.restore(&host, &id)?));
                            let reply = match &row.accepted {
                                None => SavedParallelReply::Awaiting {
                                    identity: id,
                                    declaration: plan.domain,
                                },
                                Some(value) => {
                                    let recorded = ledger
                                        .replies
                                        .iter()
                                        .find(|r| r.serial == row.serial)
                                        .ok_or(Refusal)?;
                                    if *value != recorded.reply {
                                        return Err(Refusal);
                                    }
                                    let reply = cached::<P>(&ledger, row.serial)?;
                                    SavedParallelReply::Accepted(
                                        PendingReply::new(id.clone(), (), plan.domain)
                                            .try_accept_in_scope(&id, reply, &host, scope)
                                            .map_err(|_| Refusal)?,
                                    )
                                }
                            };
                            Ok(SavedParallelMember {
                                index: i,
                                name: branches[i].name.as_str(),
                                reply,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let barrier = Parallel::restore_in_scope(
                        plan.node,
                        rows,
                        BATCH,
                        |rows| {
                            if fault == Fault::ChildDenied {
                                return Err(Refusal);
                            }
                            if fault == Fault::ChildRevoked {
                                scope.close();
                            }
                            runtime.authorize(&id)?;
                            for (i, row) in rows.iter().enumerate() {
                                if *row.reply.identity() != identity(image.members[i].serial) {
                                    return Err(Refusal);
                                }
                                runtime.authorize(row.reply.identity())?;
                            }
                            Ok(())
                        },
                        scope,
                    );
                    let mut barrier = match barrier {
                        Ok(barrier) => barrier,
                        Err(_) => {
                            assert!(!scope.is_open());
                            assert!(matches!(
                                vm.poll_in_scope(&runtime, scope).unwrap(),
                                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                            ));
                            return Err(Refusal);
                        }
                    };
                    assert_eq!(vm.fuel_remaining(), image.remaining);
                    assert_eq!(host.serial.get(), image.watermark);
                    if fault == Fault::Cancelled {
                        scope.close();
                        assert!(matches!(
                            barrier.poll_in_scope(scope),
                            Ok(ParallelPoll::Terminal(
                                parallel_evaluation::ParallelEnd::Cancelled
                            ))
                        ));
                        assert!(matches!(
                            vm.poll_in_scope(&runtime, scope).unwrap(),
                            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                        ));
                        return Err(Refusal);
                    }
                    // Unsettled external work is host-owned; restoration itself publishes no requests.
                    let mut replies = Vec::new();
                    for (i, row) in image.members.iter().enumerate() {
                        let reply = if row.accepted.is_some() {
                            None
                        } else {
                            Some(settle(
                                &host,
                                Dispatch {
                                    identity: identity(row.serial),
                                    request: requests[i].take().unwrap(),
                                },
                                store,
                                &mut ledger,
                            )?)
                        };
                        replies.push(reply);
                    }
                    for i in order {
                        let current = identity(image.members[i].serial);
                        if let Some(mut reply) = replies[i].take() {
                            let stale = Identity {
                                owner: old.clone(),
                                ..current.clone()
                            };
                            let pointer = &*reply.buffer as *const u8;
                            reply = barrier
                                .try_accept_in_scope(i, &stale, reply, &host, scope)
                                .unwrap_err()
                                .reply;
                            assert_eq!(&*reply.buffer as *const u8, pointer);
                            barrier
                                .try_accept_in_scope(i, &current, reply, &host, scope)
                                .unwrap();
                        }
                        let duplicate = cached::<P>(&ledger, current.serial)?;
                        let pointer = &*duplicate.buffer as *const u8;
                        let rejected = barrier
                            .try_accept_in_scope(i, &current, duplicate, &host, scope)
                            .unwrap_err();
                        assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
                        assert!(scope.is_open());
                    }
                    let ParallelPoll::Results(rows) = barrier.poll_in_scope(scope).unwrap() else {
                        return Err(Refusal);
                    };
                    assert!(!matches!(
                        barrier.poll_in_scope(scope),
                        Ok(ParallelPoll::Results(_))
                    ));
                    rows
                }
                GroupKind::Sequence => {
                    let mut recovery = SequenceRecovery {
                        host: &host,
                        plan,
                        scope,
                        image: &image,
                        ledger: &mut ledger,
                        store,
                        old,
                        fault,
                    };
                    match recovery.resume() {
                        Ok(rows) => rows,
                        Err(error) => {
                            assert!(!scope.is_open());
                            assert!(matches!(
                                vm.poll_in_scope(&runtime, scope).unwrap(),
                                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                            ));
                            return Err(error);
                        }
                    }
                }
            };
            assert_eq!(host.serial.get(), image.watermark);
            let mut reply = Receipt {
                node: plan.node,
                identity: id.clone(),
                response: Response::Group(rows),
                buffer: Box::new(91),
            };
            let pointer = &*reply.buffer as *const u8;
            let stale = Identity {
                owner: old.clone(),
                ..id.clone()
            };
            reply = vm
                .try_accept_in_scope(&stale, reply, &runtime, scope)
                .unwrap_err()
                .reply;
            assert_eq!(&*reply.buffer as *const u8, pointer);
            assert_eq!(vm.fuel_remaining(), image.remaining);
            vm.try_accept_in_scope(&id, reply, &runtime, scope).unwrap();
            loop {
                let value = match vm.poll_in_scope(&runtime, scope) {
                    Ok(EffectSessionPoll::Request(active)) => {
                        let Work::Atom(dispatch) = active.work else {
                            return Err(Refusal);
                        };
                        let id = active.plan.id();
                        let reply = settle(&host, dispatch, store, &mut ledger)?;
                        vm.try_accept_in_scope(
                            &id,
                            Receipt {
                                node: active.plan.node,
                                identity: id.clone(),
                                response: Response::Atom(reply),
                                buffer: Box::new(92),
                            },
                            &runtime,
                            scope,
                        )
                        .unwrap();
                        continue;
                    }
                    Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                        assert_eq!(value, P::output());
                        Some(value)
                    }
                    Err(_) => {
                        assert_eq!(
                            vm.status(),
                            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                        );
                        None
                    }
                    _ => return Err(Refusal),
                };
                assert!(!scope.is_open());
                assert_eq!(*host.invoked.borrow(), P::inputs());
                let done = Finished {
                    value,
                    remaining: vm.fuel_remaining(),
                };
                ledger.terminal = Some(done.clone());
                store.write("group-ledger.json", &ledger);
                return Ok(done);
            }
        })();
    });
    assert!(!new_lease.unwrap().is_open());
    result
}

#[test]
fn disk_all_reopens_parent_and_child_across_every_settled_prefix_mask_and_receipt_order() {
    fn check<P: GroupProfile>() {
        let expected = live::<P>(10_000);
        for dispatched in 0..=3 {
            for mask in 0..1 << dispatched {
                for order in ORDERS {
                    let store = Store::new();
                    let old = save::<P>(&store, dispatched, mask, 10_000);
                    let bytes = store.read("group-checkpoint.json");
                    assert_eq!(
                        reopen::<P>(&store, &bytes, &old, order, Fault::None).unwrap(),
                        expected
                    );
                    let saved = store.read("group-ledger.json");
                    assert_eq!(ledger(&store).replies.len(), 4);
                    assert_eq!(
                        reopen::<P>(&store, &bytes, &old, order, Fault::None).unwrap(),
                        expected
                    );
                    assert_eq!(store.read("group-ledger.json"), saved);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_all_preserves_parent_assembly_and_successor_completion_fuel() {
    fn check<P: GroupProfile>() {
        let used = 10_000 - live::<P>(10_000).remaining;
        for budget in [used - 1, used, used + 1] {
            let expected = live::<P>(budget);
            assert_eq!(expected.value.is_some(), budget >= used);
            for dispatched in 0..=3 {
                for mask in 0..1 << dispatched {
                    let store = Store::new();
                    let old = save::<P>(&store, dispatched, mask, budget);
                    assert_eq!(
                        reopen::<P>(
                            &store,
                            &store.read("group-checkpoint.json"),
                            &old,
                            [2, 1, 0],
                            Fault::None
                        )
                        .unwrap(),
                        expected
                    );
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_all_parent_or_child_refusal_and_cancellation_revoke_the_shared_owner() {
    fn check<P: GroupProfile>() {
        for dispatched in 0..=3 {
            for mask in 0..1 << dispatched {
                for fault in [
                    Fault::ParentDenied,
                    Fault::ChildDenied,
                    Fault::ChildRevoked,
                    Fault::Cancelled,
                ] {
                    let store = Store::new();
                    let old = save::<P>(&store, dispatched, mask, 10_000);
                    let saved = store.read("group-ledger.json");
                    assert!(
                        reopen::<P>(
                            &store,
                            &store.read("group-checkpoint.json"),
                            &old,
                            [2, 1, 0],
                            fault
                        )
                        .is_err()
                    );
                    assert_eq!(store.read("group-ledger.json"), saved);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_all_changed_candidates_cannot_relabel_members_frames_or_remaining_fuel() {
    fn check<P: GroupProfile>() {
        for dispatched in 0..=3 {
            for mask in 0..1 << dispatched {
                let store = Store::new();
                let old = save::<P>(&store, dispatched, mask, 10_000);
                let bytes = store.read("group-checkpoint.json");
                let saved = store.read("group-ledger.json");
                for field in [
                    "source",
                    "ir",
                    "watermark",
                    "remaining",
                    "frames",
                    "members",
                ] {
                    let mut changed: Value = serde_json::from_slice(&bytes).unwrap();
                    changed[field] = Value::Null;
                    assert!(
                        reopen::<P>(
                            &store,
                            &serde_json::to_vec(&changed).unwrap(),
                            &old,
                            [2, 1, 0],
                            Fault::None
                        )
                        .is_err()
                    );
                }
                for changed in [&bytes[..bytes.len() - 1], &vec![b' '; MAX_BYTES + 1]] {
                    assert!(reopen::<P>(&store, changed, &old, [2, 1, 0], Fault::None).is_err());
                }
                assert_eq!(store.read("group-ledger.json"), saved);
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

fn sequence_cuts() -> Vec<Cut> {
    (0..=3)
        .flat_map(|accepted| {
            [SeqPhase::Ready, SeqPhase::Awaiting, SeqPhase::Settled]
                .into_iter()
                .filter(move |phase| accepted < 3 || *phase == SeqPhase::Ready)
                .map(move |phase| Cut::Seq { accepted, phase })
        })
        .collect()
}

#[test]
fn disk_seq_reopens_every_prefix_without_republishing_awaiting_or_replaying_settled_work() {
    fn check<P: GroupProfile>() {
        let expected = live_kind::<P>(10_000, GroupKind::Sequence);
        assert_eq!(sequence_cuts().len(), 10);
        for cut in sequence_cuts() {
            let store = Store::new();
            let old = save_cut::<P>(&store, cut, 10_000);
            let bytes = store.read("group-checkpoint.json");
            assert_eq!(
                reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::None).unwrap(),
                expected
            );
            let saved = store.read("group-ledger.json");
            assert_eq!(ledger(&store).replies.len(), 4);
            assert_eq!(
                reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::None).unwrap(),
                expected
            );
            assert_eq!(store.read("group-ledger.json"), saved);
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_seq_prefix_and_parent_reentry_preserve_exact_completion_budget() {
    fn check<P: GroupProfile>() {
        let used = 10_000 - live_kind::<P>(10_000, GroupKind::Sequence).remaining;
        for budget in [used - 1, used, used + 1] {
            let expected = live_kind::<P>(budget, GroupKind::Sequence);
            assert_eq!(expected.value.is_some(), budget >= used);
            for cut in sequence_cuts() {
                let store = Store::new();
                let old = save_cut::<P>(&store, cut, budget);
                assert_eq!(
                    reopen::<P>(
                        &store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [0, 1, 2],
                        Fault::None
                    )
                    .unwrap(),
                    expected
                );
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_seq_parent_child_refusal_and_cancellation_never_release_the_suffix() {
    fn check<P: GroupProfile>() {
        for cut in sequence_cuts() {
            for fault in [
                Fault::ParentDenied,
                Fault::ChildDenied,
                Fault::ChildRevoked,
                Fault::Cancelled,
            ] {
                let store = Store::new();
                let old = save_cut::<P>(&store, cut, 10_000);
                let saved = store.read("group-ledger.json");
                assert!(
                    reopen::<P>(
                        &store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [0, 1, 2],
                        fault
                    )
                    .is_err()
                );
                assert_eq!(store.read("group-ledger.json"), saved);
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_seq_each_unpublished_member_requires_fresh_publication_authority() {
    fn check<P: GroupProfile>() {
        let mut cases = 0;
        for cut in sequence_cuts() {
            let Cut::Seq { accepted, phase } = cut else {
                unreachable!()
            };
            for denied in accepted + usize::from(phase != SeqPhase::Ready)..3 {
                let store = Store::new();
                let old = save_cut::<P>(&store, cut, 10_000);
                assert!(
                    reopen::<P>(
                        &store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [0, 1, 2],
                        Fault::PublicationDenied(denied)
                    )
                    .is_err()
                );
                let log = ledger(&store);
                assert_eq!(
                    log.replies
                        .iter()
                        .map(|r| r.input.clone())
                        .collect::<Vec<_>>(),
                    P::inputs()[..denied]
                );
                assert!(log.terminal.is_none());
                cases += 1;
            }
        }
        assert_eq!(cases, 12);
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_seq_malformed_host_written_frontiers_fail_before_native_batch_admission() {
    fn check<P: GroupProfile>() {
        for corruption in 0..3 {
            let store = Store::new();
            let old = save_cut::<P>(
                &store,
                Cut::Seq {
                    accepted: if corruption == 2 { 3 } else { 0 },
                    phase: SeqPhase::Ready,
                },
                10_000,
            );
            let mut image: Image =
                serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
            match corruption {
                0 => image.members[1].published = true,
                1 => {
                    image.members[0].published = true;
                    image.members[1].published = true;
                }
                2 => {
                    image.members[1].published = false;
                    image.members[1].accepted = None;
                }
                _ => unreachable!(),
            }
            // Deliberately corrupt the trusted fixture record, so correspondence alone cannot reject it.
            store.write("group-checkpoint.json", &image);
            let saved = store.read("group-ledger.json");
            assert!(
                reopen::<P>(
                    &store,
                    &store.read("group-checkpoint.json"),
                    &old,
                    [0, 1, 2],
                    Fault::ExpectFrontierRefusal
                )
                .is_err()
            );
            assert_eq!(store.read("group-ledger.json"), saved);
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_seq_changed_candidates_never_replace_the_committed_frontier() {
    fn check<P: GroupProfile>() {
        for cut in sequence_cuts() {
            let store = Store::new();
            let old = save_cut::<P>(&store, cut, 10_000);
            let bytes = store.read("group-checkpoint.json");
            let saved = store.read("group-ledger.json");
            for field in [
                "source",
                "ir",
                "watermark",
                "remaining",
                "frames",
                "members",
            ] {
                let mut changed: Value = serde_json::from_slice(&bytes).unwrap();
                changed[field] = Value::Null;
                assert!(
                    reopen::<P>(
                        &store,
                        &serde_json::to_vec(&changed).unwrap(),
                        &old,
                        [0, 1, 2],
                        Fault::None
                    )
                    .is_err()
                );
            }
            for changed in [&bytes[..bytes.len() - 1], &vec![b' '; MAX_BYTES + 1]] {
                assert!(reopen::<P>(&store, changed, &old, [0, 1, 2], Fault::None).is_err());
            }
            assert_eq!(store.read("group-ledger.json"), saved);
        }
    }
    check::<Counter>();
    check::<Panel>();
}
