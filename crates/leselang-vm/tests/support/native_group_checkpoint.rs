// Host-owned disk fixture for one pending Seq/All, its parent frames and a successor.
use super::super::checkpoint_acceptance::{
    Call, DiskProfile, PublicationPoint, Store, frame_site, limits, node_at,
};
use super::*;
use effect_stack::SavedEffectReturnFrame;
use group_reply::restore_group_reply;
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum SeqPhase {
    Ready,
    Awaiting,
    Settled,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
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

fn source_with_groups<P: GroupProfile>(kind: GroupKind, predecessors: &[GroupKind]) -> String {
    assert!(predecessors.len() <= 2);
    let (helpers, _) = P::SOURCE.split_once("fn main()").unwrap();
    let mut source = match predecessors.len() {
        0 => P::SOURCE.into(),
        1 => format!("{helpers}{}", P::CAPTURED_MAIN),
        _ => format!("{helpers}{}", P::MULTI_MAIN),
    };
    if kind == GroupKind::Sequence {
        source = source.replacen("= all(", "= seq(", 1);
    }
    if predecessors.len() == 2 {
        let batch = helpers
            .lines()
            .find(|line| line.starts_with("fn batch("))
            .unwrap();
        for (name, kind) in ["before_one", "before_two"].into_iter().zip(predecessors) {
            let mut definition = batch.replacen("fn batch(", &format!("fn {name}("), 1);
            if *kind == GroupKind::Sequence {
                definition = definition.replacen("= all(", "= seq(", 1);
            }
            source.push_str(&format!("\n{definition}\n"));
        }
    } else if let Some(prior) = predecessors.first() {
        assert_eq!(*prior, kind, "single-predecessor legacy fixture");
    }
    source
}

trait GroupProfile: DiskProfile {
    const SOURCE: &'static str;
    const CAPTURED_MAIN: &'static str;
    const MULTI_MAIN: &'static str;
    fn output() -> ScalarValue;
    fn inputs() -> Vec<ScalarValue>;
    fn captured_output() -> ScalarValue;
    fn captured_inputs() -> Vec<ScalarValue>;
    fn multi_output() -> ScalarValue;
    fn multi_inputs() -> Vec<ScalarValue>;
    fn output_for(groups: usize) -> ScalarValue {
        match groups {
            0 => Self::output(),
            1 => Self::captured_output(),
            2 => Self::multi_output(),
            _ => unreachable!(),
        }
    }
    fn inputs_for(groups: usize) -> Vec<ScalarValue> {
        match groups {
            0 => Self::inputs(),
            1 => Self::captured_inputs(),
            2 => Self::multi_inputs(),
            _ => unreachable!(),
        }
    }
}
impl GroupProfile for Counter {
    const MULTI_MAIN: &str = r#"
fn main() = bind(prior: before_one(n: 1), body:
 bind(alias: prior, body: bind(picked: member(value: prior, name: "c"), body:
 bind(other: before_two(n: 1), body: bind(other_alias: other, body:
 bind(other_picked: member(value: other, name: "c"), body:
 bind(result: run(n: 2), body:
 add(left: result, right: add(left: field(value: picked, name: "value"), right:
 add(left: field(value: other_picked, name: "value"), right:
 add(left: field(value: member(value: alias, name: "a"), name: "value"),
 right: field(value: member(value: other_alias, name: "a"), name: "value"))))))))))))
"#;
    fn multi_output() -> ScalarValue {
        ScalarValue::Integer(69)
    }
    fn multi_inputs() -> Vec<ScalarValue> {
        [1, 2, 3, 1, 2, 3, 2, 3, 4, 22]
            .into_iter()
            .map(ScalarValue::Integer)
            .collect()
    }
    const CAPTURED_MAIN: &str = r#"
fn main() = bind(prior: batch(n: 1), body: bind(alias: prior, body: bind(picked: member(value: prior, name: "c"), body: bind(seed: 2, body: bind(result: run(n: seed), body: add(left: result, right: add(left: field(value: member(value: alias, name: "a"), name: "value"), right: field(value: picked, name: "value"))))))))
"#;
    fn captured_output() -> ScalarValue {
        ScalarValue::Integer(38)
    }
    fn captured_inputs() -> Vec<ScalarValue> {
        [1, 2, 3, 2, 3, 4, 16]
            .into_iter()
            .map(ScalarValue::Integer)
            .collect()
    }
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
    const MULTI_MAIN: &str = r#"
fn main() = bind(prior: before_one(text: "old"), body:
 bind(alias: prior, body: bind(picked: member(value: prior, name: "c"), body:
 bind(other: before_two(text: "new"), body: bind(other_alias: other, body:
 bind(other_picked: member(value: other, name: "c"), body:
 bind(result: run(text: "seed"), body:
 concat(left: field(value: result, name: "text"), right:
 concat(left: field(value: picked, name: "text"), right:
 concat(left: field(value: other_picked, name: "text"), right:
 concat(left: field(value: member(value: alias, name: "a"), name: "text"),
 right: field(value: member(value: other_alias, name: "a"), name: "text"))))))))))))
"#;
    fn multi_output() -> ScalarValue {
        ScalarValue::String("seedC!oldCnewColdAnewA".into())
    }
    fn multi_inputs() -> Vec<ScalarValue> {
        [
            "oldA", "oldB", "oldC", "newA", "newB", "newC", "seedA", "seedB", "seedC", "seedC!",
        ]
        .into_iter()
        .map(|s| ScalarValue::String(s.into()))
        .collect()
    }
    const CAPTURED_MAIN: &str = r#"
fn main() = bind(prior: batch(text: "old"), body: bind(alias: prior, body: bind(picked: member(value: prior, name: "c"), body: bind(seed: "seed", body: bind(result: run(text: seed), body: concat(left: field(value: result, name: "text"), right: concat(left: field(value: member(value: alias, name: "a"), name: "text"), right: field(value: picked, name: "text"))))))))
"#;
    fn captured_output() -> ScalarValue {
        ScalarValue::String("seedC!oldAoldC".into())
    }
    fn captured_inputs() -> Vec<ScalarValue> {
        ["oldA", "oldB", "oldC", "seedA", "seedB", "seedC", "seedC!"]
            .into_iter()
            .map(|s| ScalarValue::String(s.into()))
            .collect()
    }
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
    capture: Vec<(String, Captured)>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
enum Captured {
    Scalar(ScalarValue),
    Group(u64),
    Member { serial: u64, reply: Value },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedGroup {
    site: Vec<usize>,
    serial: u64,
    kind: GroupKind,
    rows: Vec<CompletedRow>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedRow {
    index: usize,
    name: String,
    serial: u64,
    reply: Value,
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
    captured: bool,
    predecessors: Vec<GroupKind>,
    completed: Vec<CompletedGroup>,
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
    end: FinishKind,
    value: Option<ScalarValue>,
    remaining: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum FinishKind {
    Completed,
    Failed,
    Cancelled,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    state: Value,
    roots: Vec<u64>,
    groups: Vec<GroupOrigin>,
    captures: Vec<CaptureOrigin>,
    replies: Vec<Recorded>,
    cancellation_requested: bool,
    terminal: Option<Finished>,
}
fn recorded_terminal(image: &Image, ledger: &Ledger) -> Result<Option<Finished>, Refusal> {
    let Some(done) = &ledger.terminal else {
        return Ok(None);
    };
    let valid = match done.end {
        FinishKind::Cancelled => {
            ledger.cancellation_requested
                && done.value.is_none()
                && done.remaining == image.remaining
        }
        FinishKind::Completed => {
            !ledger.cancellation_requested
                && done.value.is_some()
                && done.remaining <= image.remaining
        }
        FinishKind::Failed => {
            !ledger.cancellation_requested
                && done.value.is_none()
                && done.remaining <= image.remaining
        }
    };
    if !valid {
        return Err(Refusal);
    }
    Ok(Some(done.clone()))
}

fn persist_cancellation(
    store: &Store,
    ledger: &mut Ledger,
    image: &Image,
    scope: &LocalExecutionScope,
    fault: Fault,
) -> Result<Finished, Refusal> {
    assert!(
        !scope.is_open(),
        "revoke before writing cancellation intent"
    );
    if let Some(done) = recorded_terminal(image, ledger)? {
        return Ok(done);
    }
    if !ledger.cancellation_requested {
        ledger.cancellation_requested = true;
        store
            .replace_with_hook(
                "group-ledger.json",
                ledger,
                fault == Fault::CancelIntentWriteFailed,
                |point| pause_publication(store, fault, false, point),
            )
            .map_err(|_| Refusal)?;
    }
    if fault == Fault::ExitAfterCancelIntent {
        store.write("intent-exit-pid.json", &std::process::id());
        // Deliberately skip stack cleanup after the published intent, before terminal acknowledgement.
        std::process::exit(73);
    }
    let done = Finished {
        end: FinishKind::Cancelled,
        value: None,
        remaining: image.remaining,
    };
    ledger.terminal = Some(done.clone());
    store
        .replace_with_hook(
            "group-ledger.json",
            ledger,
            fault == Fault::CancelTerminalWriteFailed,
            |point| pause_publication(store, fault, true, point),
        )
        .map_err(|_| Refusal)?;
    Ok(done)
}
fn pause_publication(store: &Store, fault: Fault, terminal: bool, point: PublicationPoint) {
    if fault == Fault::KillPublication(terminal, point) {
        store
            .replace(
                "kill-ready.json",
                &(std::process::id(), terminal, point),
                false,
            )
            .unwrap();
        // The parent kills this owned child only after observing the atomically published marker.
        std::thread::sleep(std::time::Duration::from_secs(60));
        panic!("publication worker was not terminated by its parent");
    }
}
#[derive(PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupOrigin {
    site: Vec<usize>,
    serial: u64,
    kind: GroupKind,
}
#[derive(PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureOrigin {
    frame: Vec<usize>,
    name: String,
    group: u64,
    member: Option<u64>,
}
fn capture_origins(
    frames: &[Frame],
    groups: &[CompletedGroup],
) -> Result<Vec<CaptureOrigin>, Refusal> {
    let mut origins = Vec::new();
    for frame in frames {
        for (name, value) in &frame.capture {
            let (group, member) = match value {
                Captured::Scalar(_) => continue,
                Captured::Group(serial) => (*serial, None),
                Captured::Member { serial, .. } => {
                    let group = groups
                        .iter()
                        .find(|g| g.rows.iter().any(|r| r.serial == *serial))
                        .ok_or(Refusal)?;
                    (group.serial, Some(*serial))
                }
            };
            origins.push(CaptureOrigin {
                frame: frame.site.clone(),
                name: name.clone(),
                group,
                member,
            });
        }
    }
    Ok(origins)
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
// Dispatch order is fixture host policy; group roots never represent external effects.
fn settle<P: GroupProfile>(
    host: &Host<'_, '_, P>,
    dispatch: Dispatch<'_, P>,
    store: &Store,
    ledger: &mut Ledger,
) -> Result<Reply<P::Value>, Refusal> {
    host.authorize(&dispatch.identity)?;
    store
        .require_published("group-ledger.json")
        .map_err(|_| Refusal)?;
    // A stale in-memory adapter ledger cannot ignore a committed cancellation fence.
    let current = self::ledger(store);
    if current.cancellation_requested || current.terminal.is_some() {
        return Err(Refusal);
    }
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
    let mut expected = ledger.replies.last().map_or(1, |r| r.serial + 1);
    while ledger.roots.contains(&expected) {
        expected += 1;
    }
    if serial != expected {
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
    live_with_capture::<P>(budget, kind, false)
}

fn live_with_capture<P: GroupProfile>(budget: u64, kind: GroupKind, captured: bool) -> Finished {
    live_with_groups::<P>(budget, kind, &vec![kind; usize::from(captured)])
}

fn live_with_groups<P: GroupProfile>(
    budget: u64,
    kind: GroupKind,
    predecessors: &[GroupKind],
) -> Finished {
    let mut result = None;
    fixture::<P>(|host, scope| {
        let root = compile(&source_with_groups::<P>(kind, predecessors), &host).unwrap();
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
                    assert_eq!(value, P::output_for(predecessors.len()));
                    Some(value)
                }
                Err(_) => None,
                _ => panic!("unexpected live group state"),
            };
            assert!(!scope.is_open());
            assert_eq!(*host.invoked.borrow(), P::inputs_for(predecessors.len()));
            result = Some(Finished {
                end: if value.is_some() {
                    FinishKind::Completed
                } else {
                    FinishKind::Failed
                },
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
    save_with_capture::<P>(store, cut, budget, false)
}

fn save_with_capture<P: GroupProfile>(
    store: &Store,
    cut: Cut,
    budget: u64,
    captured: bool,
) -> LocalExecutionLease {
    save_with_groups::<P>(store, cut, budget, &vec![cut.kind(); usize::from(captured)])
}

fn save_with_groups<P: GroupProfile>(
    store: &Store,
    cut: Cut,
    budget: u64,
    predecessors: &[GroupKind],
) -> LocalExecutionLease {
    let mut old = None;
    fixture::<P>(|host, scope| {
        old = Some(scope.lease());
        let root = compile(&source_with_groups::<P>(cut.kind(), predecessors), &host).unwrap();
        let plans = plans(&root, &host);
        let runtime = Runtime {
            host: &host,
            plans: &plans,
        };
        let mut owner = scope.bind(Some(Vm::new(&root, Vec::new(), Fuel::new(budget), STACK)));
        let (scope, slot) = owner.as_parts_mut();
        let mut ledger = Ledger {
            state: host.profile.state(),
            roots: (0..=predecessors.len()).map(|i| 1 + i as u64 * 4).collect(),
            groups: Vec::new(),
            captures: Vec::new(),
            replies: Vec::new(),
            cancellation_requested: false,
            terminal: None,
        };
        store.write("group-ledger.json", &ledger);
        for (index, kind) in predecessors.iter().enumerate() {
            let vm = slot.as_mut().unwrap();
            let EffectSessionPoll::Request(active) = vm.poll_in_scope(&runtime, scope).unwrap()
            else {
                panic!("completed predecessor")
            };
            let id = active.plan.id();
            assert_eq!(id.serial, 1 + index as u64 * 4);
            ledger.groups.push(GroupOrigin {
                site: site(&root, active.plan.node),
                serial: id.serial,
                kind: *kind,
            });
            let reply = runtime.collect(active).unwrap();
            let Response::Group(rows) = &reply.response else {
                panic!("group predecessor")
            };
            for (row, input) in rows
                .iter()
                .zip(host.invoked.borrow().iter().skip(index * 3))
            {
                let (_, _, id, _, reply) = row.as_parts();
                ledger.replies.push(Recorded {
                    serial: id.serial,
                    input: input.clone(),
                    reply: P::encode(&reply.value),
                });
            }
            ledger.state = host.profile.state();
            store.write("group-ledger.json", &ledger);
            vm.try_accept_in_scope(&id, reply, &runtime, scope).unwrap();
        }
        let EffectSessionPoll::Request(active) = slot
            .as_mut()
            .unwrap()
            .poll_in_scope(&runtime, scope)
            .unwrap()
        else {
            panic!("pending group")
        };
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
        let mut completed = Vec::new();
        let mut group_owners = Vec::new();
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
                            let value = match value {
                                PureValue::Scalar(value) => Captured::Scalar(value),
                                PureValue::Result(View::Atom(id, value)) => Captured::Member {
                                    serial: id.serial,
                                    reply: P::encode(&value),
                                },
                                PureValue::Result(View::Group(id, group)) => {
                                    if let Some((_, original)) =
                                        group_owners.iter().find(|(serial, _)| *serial == id.serial)
                                    {
                                        assert!(
                                            Rc::ptr_eq(original, &group),
                                            "aliases share one original group"
                                        );
                                    } else {
                                        completed.push(CompletedGroup {
                                            site: site(&root, group.group()),
                                            serial: id.serial,
                                            kind: group.kind(),
                                            rows: group
                                                .members()
                                                .iter()
                                                .map(|row| {
                                                    let (index, branch, id, _, reply) =
                                                        row.as_parts();
                                                    CompletedRow {
                                                        index,
                                                        name: branch.name.clone(),
                                                        serial: id.serial,
                                                        reply: P::encode(&reply.value),
                                                    }
                                                })
                                                .collect(),
                                        });
                                        group_owners.push((id.serial, group));
                                    }
                                    Captured::Group(id.serial)
                                }
                            };
                            (name.into(), value)
                        })
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        assert!(frames.len() >= 2);
        ledger.captures = capture_origins(&frames, &completed).unwrap();
        store.write("group-ledger.json", &ledger);
        store.write(
            "group-checkpoint.json",
            &Image {
                format: 5,
                captured: !predecessors.is_empty(),
                predecessors: predecessors.to_vec(),
                completed,
                kind: cut.kind(),
                source: source_with_groups::<P>(cut.kind(), predecessors),
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
    CaptureDenied,
    CaptureRevoked,
    CaptureDeniedAt(usize),
    CaptureRevokedAt(usize),
    PersistCancelled,
    CancelIntentWriteFailed,
    CancelTerminalWriteFailed,
    CancelDenied,
    ExitAfterCancelIntent,
    KillPublication(bool, PublicationPoint),
}
impl Fault {
    fn persists_cancellation(self) -> bool {
        matches!(
            self,
            Self::PersistCancelled
                | Self::CancelIntentWriteFailed
                | Self::CancelTerminalWriteFailed
                | Self::CancelDenied
                | Self::ExitAfterCancelIntent
                | Self::KillPublication(..)
        )
    }
    fn cancels(self) -> bool {
        self == Self::Cancelled || self.persists_cancellation()
    }
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
    cancel_observed: bool,
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
                    || row.serial != self.image.root_serial + i as u64 + 1
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
        if self.fault.cancels() {
            self.cancel_observed = true;
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
            ]
            .into_iter()
            .chain(
                self.image
                    .completed
                    .iter()
                    .map(|group| self.identity(group.rows[index].serial)),
            ) {
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

fn restore_completed<'ir, P: GroupProfile>(
    runtime: &Runtime<'_, 'ir, '_, '_, P>,
    root: &'ir Node,
    image: &Image,
    ledger: &Ledger,
    scope: &LocalExecutionScope,
    fault: Fault,
) -> Result<Vec<View<'ir, P>>, Refusal> {
    let mut groups = Vec::new();
    for (group_index, saved) in image.completed.iter().enumerate() {
        // Resolve the path in the fresh admitted IR, never in the deserialized copy.
        let plan = runtime.plan(node_at(root, &saved.site)?)?;
        let Node::Group {
            group_kind,
            branches,
            ..
        } = plan.node
        else {
            return Err(Refusal);
        };
        if saved.serial != 1 + group_index as u64 * 4
            || saved.kind != *group_kind
            || saved.rows.len() != 3
            || branches.len() != 3
            || plan.identity.borrow().is_some()
        {
            return Err(Refusal);
        }
        let identity = |serial| Identity {
            owner: runtime.host.lease.clone(),
            serial,
            revision: image.revision,
        };
        let id = identity(saved.serial);
        runtime.authorize(&id)?;
        plan.identity.replace(Some(id.clone()));
        plan.members
            .replace(saved.rows.iter().map(|r| identity(r.serial)).collect());
        let rows = saved
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.index != i
                    || row.name != branches[i].name
                    || row.serial != saved.serial + i as u64 + 1
                {
                    return Err(Refusal);
                }
                let recorded = ledger
                    .replies
                    .iter()
                    .find(|r| r.serial == row.serial)
                    .ok_or(Refusal)?;
                if row.reply != recorded.reply {
                    return Err(Refusal);
                }
                let id = identity(row.serial);
                let reply = PendingReply::new(id.clone(), (), plan.domain)
                    .try_accept_in_scope(&id, cached::<P>(ledger, row.serial)?, runtime.host, scope)
                    .map_err(|_| Refusal)?;
                Ok(SavedSequenceMember {
                    index: i,
                    name: branches[i].name.as_str(),
                    reply: SavedSequenceReply::Accepted(reply),
                })
            })
            .collect::<Result<Vec<SavedSequence<'_, '_, P>>, _>>()?;
        let rows = match saved.kind {
            GroupKind::Sequence => {
                let (mut queue, prefix) = Sequence::restore_in_scope(
                    plan.node,
                    rows,
                    BATCH,
                    |_| runtime.authorize(&id),
                    scope,
                )
                .map_err(|_| Refusal)?;
                assert!(matches!(
                    queue
                        .poll_in_scope(
                            |_, _, _, _, _| -> Result<(), Refusal> {
                                panic!("completed group never publishes")
                            },
                            scope
                        )
                        .unwrap(),
                    SequencePoll::Terminal(SequenceEnd::Completed)
                ));
                prefix
            }
            GroupKind::Parallel => {
                let rows = rows
                    .into_iter()
                    .map(|row| {
                        let SavedSequenceReply::Accepted(reply) = row.reply else {
                            unreachable!()
                        };
                        SavedParallelMember {
                            index: row.index,
                            name: row.name,
                            reply: SavedParallelReply::Accepted(reply),
                        }
                    })
                    .collect();
                let mut barrier = Parallel::restore_in_scope(
                    plan.node,
                    rows,
                    BATCH,
                    |_| runtime.authorize(&id),
                    scope,
                )
                .map_err(|_| Refusal)?;
                let ParallelPoll::Results(rows) =
                    barrier.poll_in_scope(scope).map_err(|_| Refusal)?
                else {
                    return Err(Refusal);
                };
                rows
            }
        };
        let group = restore_group_reply(plan.node, rows, BATCH, |whole| {
            if fault == Fault::CaptureDenied || fault == Fault::CaptureDeniedAt(group_index) {
                return Err(Refusal);
            }
            if fault == Fault::CaptureRevoked || fault == Fault::CaptureRevokedAt(group_index) {
                scope.close();
            }
            runtime.authorize(&id)?;
            plan.validate_rows(whole.members())
        })
        .map_err(|_| Refusal)?;
        groups.push(View::Group(id, Rc::new(group)));
    }
    Ok(groups)
}

fn reopen<P: GroupProfile>(
    store: &Store,
    candidate: &[u8],
    old: &LocalExecutionLease,
    order: [usize; 3],
    fault: Fault,
) -> Result<Finished, Refusal> {
    // Correspondence against trusted committed fixture bytes, not storage authentication.
    store
        .require_published("group-ledger.json")
        .map_err(|_| Refusal)?;
    if candidate.len() > MAX_BYTES || candidate != store.read("group-checkpoint.json") {
        return Err(Refusal);
    }
    let image: Image = serde_json::from_slice(candidate).map_err(|_| Refusal)?;
    if image.format != 5
        || image.predecessors.len() > 2
        || image.captured != !image.predecessors.is_empty()
        || (image.predecessors.len() == 1 && image.predecessors[0] != image.kind)
        || image.source != source_with_groups::<P>(image.kind, &image.predecessors)
        || image.catalog != 31
        || image.revision != 1
        || image.root_serial != 1 + 4 * image.predecessors.len() as u64
        || image.watermark != image.root_serial + 3
        || image.completed.len() != image.predecessors.len()
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
            if ledger.roots
                != (0..=image.predecessors.len())
                    .map(|i| 1 + 4 * i as u64)
                    .collect::<Vec<_>>()
                || ledger.groups.len() != image.completed.len()
                || ledger.groups.iter().zip(&image.completed).enumerate().any(
                    |(i, (original, saved))| {
                        original.site != saved.site
                            || original.serial != saved.serial
                            || original.kind != saved.kind
                            || saved.kind != image.predecessors[i]
                    },
                )
                || ledger.captures != capture_origins(&image.frames, &image.completed)?
            {
                return Err(Refusal);
            }
            let identity = |serial| Identity {
                owner: host.lease.clone(),
                serial,
                revision: image.revision,
            };
            host.serial.set(image.watermark);
            host.granted.set(fault != Fault::CancelDenied);
            let id = identity(image.root_serial);
            host.authorize(&id)?;
            if let Some(done) = recorded_terminal(&image, &ledger)? {
                return Ok(done);
            }
            if ledger.cancellation_requested {
                scope.close();
                if fault.persists_cancellation() {
                    return persist_cancellation(store, &mut ledger, &image, &scope, fault);
                }
                return Err(Refusal);
            }
            host.profile.load(ledger.state.clone())?;
            host.invoked
                .replace(ledger.replies.iter().map(|r| r.input.clone()).collect());
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
            let completed = restore_completed(&runtime, &root, &image, &ledger, &scope, fault)?;
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
                            .map(|(n, v)| {
                                let value = match v {
                                    Captured::Scalar(value) => PureValue::Scalar(value.clone()),
                                    Captured::Group(serial) => PureValue::Result(completed.iter().find(|g| matches!(g, View::Group(id, _) if id.serial == *serial)).ok_or(Refusal)?.clone()),
                                    Captured::Member { serial, reply } => {
                                        let member = completed.iter().find_map(|g| match g {
                                            View::Group(_, group) => group.members().iter().find(|r| r.as_parts().2.serial == *serial),
                                            _ => None,
                                        }).ok_or(Refusal)?;
                                        let (_, _, id, _, value) = member.as_parts();
                                        if *reply != P::encode(&value.value) { return Err(Refusal); }
                                        host.authorize(id)?;
                                        PureValue::Result(View::Atom(id.clone(), value.value.clone()))
                                    }
                                };
                                Ok((n.as_str(), value))
                            })
                            .collect::<Result<Vec<_>, Refusal>>()?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if image.captured {
                let bindings: Vec<_> = frames.iter().flat_map(|f| &f.capture).collect();
                let find = |name| {
                    bindings
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, v)| v)
                        .ok_or(Refusal)
                };
                let (
                    PureValue::Result(View::Group(prior_id, prior)),
                    PureValue::Result(View::Group(alias_id, alias)),
                ) = (find("prior")?, find("alias")?)
                else {
                    return Err(Refusal);
                };
                if prior_id != alias_id || !Rc::ptr_eq(prior, alias) {
                    return Err(Refusal);
                }
                assert_eq!(
                    completed.len(),
                    image.predecessors.len(),
                    "one restored owner for every group alias"
                );
                if image.predecessors.len() == 2 {
                    let (
                        PureValue::Result(View::Group(other_id, other)),
                        PureValue::Result(View::Group(other_alias_id, other_alias)),
                    ) = (find("other")?, find("other_alias")?)
                    else {
                        return Err(Refusal);
                    };
                    if other_id != other_alias_id
                        || !Rc::ptr_eq(other, other_alias)
                        || prior_id == other_id
                        || Rc::ptr_eq(prior, other)
                    {
                        return Err(Refusal);
                    }
                }
            }
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
                                || row.serial != image.root_serial + i as u64 + 1
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
                    if fault.cancels() {
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
                        if fault.persists_cancellation() {
                            return persist_cancellation(store, &mut ledger, &image, scope, fault);
                        }
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
                            // A valid earlier receipt from the same operation is still not this member.
                            for refused in std::iter::once(stale).chain(
                                image
                                    .completed
                                    .iter()
                                    .map(|group| identity(group.rows[i].serial)),
                            ) {
                                let pointer = &*reply.buffer as *const u8;
                                reply = barrier
                                    .try_accept_in_scope(i, &refused, reply, &host, scope)
                                    .unwrap_err()
                                    .reply;
                                assert_eq!(&*reply.buffer as *const u8, pointer);
                            }
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
                    let (outcome, cancelled) = {
                        let mut recovery = SequenceRecovery {
                            host: &host,
                            plan,
                            scope,
                            image: &image,
                            ledger: &mut ledger,
                            store,
                            old,
                            fault,
                            cancel_observed: false,
                        };
                        let outcome = recovery.resume();
                        (outcome, recovery.cancel_observed)
                    };
                    match outcome {
                        Ok(rows) => rows,
                        Err(error) => {
                            assert!(!scope.is_open());
                            assert!(matches!(
                                vm.poll_in_scope(&runtime, scope).unwrap(),
                                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                            ));
                            if fault.persists_cancellation() && cancelled {
                                return persist_cancellation(
                                    store,
                                    &mut ledger,
                                    &image,
                                    scope,
                                    fault,
                                );
                            }
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
                        assert_eq!(value, P::output_for(image.predecessors.len()));
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
                assert_eq!(
                    *host.invoked.borrow(),
                    P::inputs_for(image.predecessors.len())
                );
                let done = Finished {
                    end: if value.is_some() {
                        FinishKind::Completed
                    } else {
                        FinishKind::Failed
                    },
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

fn captured_cuts() -> Vec<Cut> {
    let mut cuts = sequence_cuts();
    for dispatched in 0..=3 {
        for mask in 0..1 << dispatched {
            cuts.push(Cut::All { dispatched, mask });
        }
    }
    assert_eq!(cuts.len(), 25);
    cuts
}

#[test]
fn disk_captured_groups_preserve_aliases_and_selected_members_without_replaying_predecessors() {
    fn check<P: GroupProfile>() {
        let mut cases = 0;
        for cut in captured_cuts() {
            let expected = live_with_capture::<P>(10_000, cut.kind(), true);
            let orders = if cut.kind() == GroupKind::Parallel {
                &ORDERS[..]
            } else {
                &ORDERS[..1]
            };
            for order in orders {
                let store = Store::new();
                let old = save_with_capture::<P>(&store, cut, 10_000, true);
                let bytes = store.read("group-checkpoint.json");
                let image: Image = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(image.completed.len(), 1);
                assert_eq!(image.completed[0].serial, 1);
                assert_eq!(image.root_serial, 5);
                assert_eq!(image.watermark, 8);
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, *order, Fault::None).unwrap(),
                    expected
                );
                let saved = store.read("group-ledger.json");
                assert_eq!(
                    ledger(&store)
                        .replies
                        .iter()
                        .map(|r| r.serial)
                        .collect::<Vec<_>>(),
                    [2, 3, 4, 6, 7, 8, 9]
                );
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, *order, Fault::None).unwrap(),
                    expected
                );
                assert_eq!(store.read("group-ledger.json"), saved);
                cases += 1;
            }
        }
        assert_eq!(cases, 100);
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_captured_group_restore_never_charges_completed_assembly_again() {
    fn check<P: GroupProfile>() {
        for cut in captured_cuts() {
            let used = 10_000 - live_with_capture::<P>(10_000, cut.kind(), true).remaining;
            for budget in [used - 1, used, used + 1] {
                let expected = live_with_capture::<P>(budget, cut.kind(), true);
                assert_eq!(expected.value.is_some(), budget >= used);
                let store = Store::new();
                let old = save_with_capture::<P>(&store, cut, budget, true);
                assert_eq!(
                    reopen::<P>(
                        &store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [2, 0, 1],
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
fn disk_captured_owner_refusal_or_cancellation_never_runs_later_work() {
    fn check<P: GroupProfile>() {
        for cut in captured_cuts() {
            for fault in [
                Fault::ParentDenied,
                Fault::ChildDenied,
                Fault::ChildRevoked,
                Fault::Cancelled,
                Fault::CaptureDenied,
                Fault::CaptureRevoked,
            ] {
                let store = Store::new();
                let old = save_with_capture::<P>(&store, cut, 10_000, true);
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
fn disk_captured_corrupt_graph_and_alias_records_refuse_before_pending_dispatch() {
    fn check<P: GroupProfile>() {
        for cut in [
            Cut::All {
                dispatched: 0,
                mask: 0,
            },
            Cut::Seq {
                accepted: 0,
                phase: SeqPhase::Ready,
            },
        ] {
            for corruption in 0..12 {
                let store = Store::new();
                let old = save_with_capture::<P>(&store, cut, 10_000, true);
                let mut image: Image =
                    serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
                match corruption {
                    0 => image.completed[0].serial = image.root_serial,
                    1 => image.completed[0].site = image.site.clone(),
                    2 => image.completed[0].rows[1].serial = 2,
                    3 => image.completed[0].rows[0].name = "foreign".into(),
                    4 => {
                        image.completed[0].rows.pop();
                    }
                    5 => image.completed[0].rows[0].reply = Value::Null,
                    6 => image.completed.clear(),
                    7 => {
                        image.completed[0].kind = if cut.kind() == GroupKind::Parallel {
                            GroupKind::Sequence
                        } else {
                            GroupKind::Parallel
                        }
                    }
                    8..=11 => {
                        let mut changed = 0;
                        for frame in &mut image.frames {
                            if corruption == 11 {
                                let before = frame.capture.len();
                                frame.capture.retain(|(n, _)| n != "alias");
                                changed += before - frame.capture.len();
                                continue;
                            }
                            for (name, value) in &mut frame.capture {
                                match (corruption, name.as_str(), value) {
                                    (8, "alias", Captured::Group(serial)) => {
                                        *serial = image.root_serial;
                                        changed += 1;
                                    }
                                    (9, "picked", Captured::Member { serial, .. }) => {
                                        *serial = image.root_serial + 1;
                                        changed += 1;
                                    }
                                    (10, "picked", Captured::Member { reply, .. }) => {
                                        *reply = Value::Null;
                                        changed += 1;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        assert!(changed > 0);
                    }
                    _ => unreachable!(),
                }
                // Corrupt the committed writer output, not merely an untrusted candidate.
                store.write("group-checkpoint.json", &image);
                let saved = store.read("group-ledger.json");
                assert!(
                    reopen::<P>(
                        &store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [0, 1, 2],
                        Fault::None
                    )
                    .is_err(),
                    "corruption {corruption}"
                );
                assert_eq!(store.read("group-ledger.json"), saved);
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

fn predecessor_pairs() -> [[GroupKind; 2]; 4] {
    use GroupKind::{Parallel as All, Sequence as Seq};
    [[Seq, Seq], [Seq, All], [All, Seq], [All, All]]
}

#[test]
fn disk_mixed_two_predecessors_keep_distinct_owners_and_every_pending_frontier() {
    fn check<P: GroupProfile>() {
        let mut cases = 0;
        for predecessors in predecessor_pairs() {
            for cut in captured_cuts() {
                let expected = live_with_groups::<P>(10_000, cut.kind(), &predecessors);
                let orders = if cut.kind() == GroupKind::Parallel {
                    &ORDERS[..]
                } else {
                    &ORDERS[..1]
                };
                for order in orders {
                    let store = Store::new();
                    let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
                    let bytes = store.read("group-checkpoint.json");
                    let image: Image = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(image.completed.len(), 2);
                    assert_eq!(image.root_serial, 9);
                    assert_eq!(image.watermark, 12);
                    assert_eq!(
                        reopen::<P>(&store, &bytes, &old, *order, Fault::None).unwrap(),
                        expected
                    );
                    let saved = store.read("group-ledger.json");
                    assert_eq!(
                        ledger(&store)
                            .replies
                            .iter()
                            .map(|r| r.serial)
                            .collect::<Vec<_>>(),
                        [2, 3, 4, 6, 7, 8, 10, 11, 12, 13]
                    );
                    assert_eq!(
                        reopen::<P>(&store, &bytes, &old, *order, Fault::None).unwrap(),
                        expected
                    );
                    assert_eq!(store.read("group-ledger.json"), saved);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 400);
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_mixed_two_completed_assemblies_keep_exact_parent_completion_cost() {
    fn check<P: GroupProfile>() {
        for predecessors in predecessor_pairs() {
            for cut in captured_cuts() {
                let used =
                    10_000 - live_with_groups::<P>(10_000, cut.kind(), &predecessors).remaining;
                for budget in [used - 1, used, used + 1] {
                    let expected = live_with_groups::<P>(budget, cut.kind(), &predecessors);
                    assert_eq!(expected.value.is_some(), budget >= used);
                    let store = Store::new();
                    let old = save_with_groups::<P>(&store, cut, budget, &predecessors);
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
fn disk_mixed_refusal_at_each_predecessor_or_pending_owner_stops_later_work() {
    fn check<P: GroupProfile>() {
        for predecessors in predecessor_pairs() {
            for cut in captured_cuts() {
                for fault in [
                    Fault::ParentDenied,
                    Fault::ChildDenied,
                    Fault::ChildRevoked,
                    Fault::Cancelled,
                    Fault::CaptureDeniedAt(0),
                    Fault::CaptureDeniedAt(1),
                    Fault::CaptureRevokedAt(0),
                    Fault::CaptureRevokedAt(1),
                ] {
                    let store = Store::new();
                    let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
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
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_mixed_same_schema_graphs_and_captured_members_cannot_be_exchanged() {
    fn check<P: GroupProfile>() {
        for predecessors in predecessor_pairs() {
            for cut in [
                Cut::All {
                    dispatched: 0,
                    mask: 0,
                },
                Cut::Seq {
                    accepted: 0,
                    phase: SeqPhase::Ready,
                },
            ] {
                for corruption in 0..12 {
                    let store = Store::new();
                    let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
                    let mut image: Image =
                        serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
                    match corruption {
                        0 => image.completed.swap(0, 1),
                        1 => {
                            let (first, second) = image.completed.split_at_mut(1);
                            std::mem::swap(&mut first[0].site, &mut second[0].site);
                        }
                        2 => image.completed[1].serial = 1,
                        3 => {
                            image.completed[1].rows[0].serial = image.completed[0].rows[0].serial;
                            image.completed[1].rows[0].reply =
                                image.completed[0].rows[0].reply.clone();
                        }
                        4 => {
                            image.completed[1].rows[2].reply =
                                image.completed[0].rows[2].reply.clone()
                        }
                        5..=7 | 10 => {
                            let mut changed = 0;
                            for frame in &mut image.frames {
                                for (name, value) in &mut frame.capture {
                                    let replacement = match (corruption, name.as_str()) {
                                        (5, "other_alias") => Some(Captured::Group(1)),
                                        (6, "other_picked") => Some(Captured::Member {
                                            serial: 4,
                                            reply: image.completed[0].rows[2].reply.clone(),
                                        }),
                                        (7, "picked") => Some(Captured::Member {
                                            serial: 8,
                                            reply: image.completed[1].rows[2].reply.clone(),
                                        }),
                                        (10, "prior") => Some(Captured::Group(5)),
                                        _ => None,
                                    };
                                    if let Some(replacement) = replacement {
                                        *value = replacement;
                                        changed += 1;
                                    }
                                }
                            }
                            assert!(changed > 0);
                        }
                        8 => image.members[0].serial = 2,
                        9 => {
                            image.completed.pop();
                        }
                        11 => {
                            image.completed[1].kind = if predecessors[1] == GroupKind::Sequence {
                                GroupKind::Parallel
                            } else {
                                GroupKind::Sequence
                            }
                        }
                        _ => unreachable!(),
                    }
                    // Valid-looking receipts from another completed group still lack capture provenance.
                    store.write("group-checkpoint.json", &image);
                    let saved = store.read("group-ledger.json");
                    assert!(
                        reopen::<P>(
                            &store,
                            &store.read("group-checkpoint.json"),
                            &old,
                            [0, 1, 2],
                            Fault::None
                        )
                        .is_err(),
                        "corruption {corruption}"
                    );
                    assert_eq!(store.read("group-ledger.json"), saved);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

fn cancellation_cases() -> Vec<(Cut, Vec<GroupKind>)> {
    let mut cases = Vec::new();
    for cut in captured_cuts() {
        cases.push((cut, Vec::new()));
        cases.push((cut, vec![cut.kind()]));
        for pair in predecessor_pairs() {
            cases.push((cut, pair.to_vec()));
        }
    }
    assert_eq!(cases.len(), 150);
    cases
}

fn effects(store: &Store) -> Value {
    let mut value: Value = serde_json::from_slice(&store.read("group-ledger.json")).unwrap();
    let fields = value.as_object_mut().unwrap();
    fields.remove("terminal");
    fields.remove("cancellation_requested");
    value
}

fn stale_dispatch_is_fenced<P: GroupProfile>(store: &Store, image: &Image, mut stale: Ledger) {
    let before = store.read("group-ledger.json");
    fixture::<P>(|host, _scope| {
        host.serial.set(image.watermark);
        let id = Identity {
            owner: host.lease.clone(),
            serial: image.members[0].serial,
            revision: image.revision,
        };
        let request = image.members[0].call.restore(&host, &id).unwrap();
        assert!(
            settle(
                &host,
                Dispatch {
                    identity: id,
                    request
                },
                store,
                &mut stale
            )
            .is_err()
        );
        assert!(
            host.invoked.borrow().is_empty(),
            "stale adapter must consult the persisted fence"
        );
    });
    assert_eq!(store.read("group-ledger.json"), before);
}

#[test]
fn disk_cancel_acknowledgement_survives_reopen_and_blocks_stale_adapter_dispatch() {
    fn check<P: GroupProfile>() {
        for (cut, predecessors) in cancellation_cases() {
            let store = Store::new();
            let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
            let bytes = store.read("group-checkpoint.json");
            let image: Image = serde_json::from_slice(&bytes).unwrap();
            let stale = ledger(&store);
            let before = effects(&store);
            let cancelled =
                reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::PersistCancelled).unwrap();
            assert_eq!(
                cancelled,
                Finished {
                    end: FinishKind::Cancelled,
                    value: None,
                    remaining: image.remaining
                }
            );
            assert_eq!(effects(&store), before);
            assert!(ledger(&store).cancellation_requested);
            let committed = store.read("group-ledger.json");
            assert!(reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::CancelDenied).is_err());
            assert_eq!(store.read("group-ledger.json"), committed);
            for fault in [Fault::None, Fault::PersistCancelled] {
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, [2, 0, 1], fault).unwrap(),
                    cancelled
                );
                assert_eq!(store.read("group-ledger.json"), committed);
            }
            stale_dispatch_is_fenced::<P>(&store, &image, stale);
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_cancel_committed_intent_fences_recovery_until_terminal_retry_succeeds() {
    fn check<P: GroupProfile>() {
        for (cut, predecessors) in cancellation_cases() {
            let store = Store::new();
            let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
            let bytes = store.read("group-checkpoint.json");
            let image: Image = serde_json::from_slice(&bytes).unwrap();
            let stale = ledger(&store);
            let before = effects(&store);
            assert!(
                reopen::<P>(
                    &store,
                    &bytes,
                    &old,
                    [0, 1, 2],
                    Fault::CancelTerminalWriteFailed
                )
                .is_err()
            );
            assert!(ledger(&store).cancellation_requested);
            assert!(ledger(&store).terminal.is_none());
            let intent = store.read("group-ledger.json");
            for fault in [
                Fault::None,
                Fault::CancelTerminalWriteFailed,
                Fault::CancelDenied,
            ] {
                assert!(reopen::<P>(&store, &bytes, &old, [0, 1, 2], fault).is_err());
                assert_eq!(store.read("group-ledger.json"), intent);
            }
            stale_dispatch_is_fenced::<P>(&store, &image, stale);
            let done =
                reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::PersistCancelled).unwrap();
            assert_eq!(done.end, FinishKind::Cancelled);
            assert_eq!(done.remaining, image.remaining);
            assert_eq!(effects(&store), before);
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_cancel_refusal_before_intent_never_claims_a_persisted_cancellation() {
    fn check<P: GroupProfile>() {
        for (cut, predecessors) in cancellation_cases() {
            for fault in [Fault::CancelDenied, Fault::CancelIntentWriteFailed] {
                let store = Store::new();
                let old = save_with_groups::<P>(&store, cut, 10_000, &predecessors);
                let bytes = store.read("group-checkpoint.json");
                let before = store.read("group-ledger.json");
                assert!(reopen::<P>(&store, &bytes, &old, [0, 1, 2], fault).is_err());
                assert_eq!(store.read("group-ledger.json"), before);
                assert!(!ledger(&store).cancellation_requested);
                assert!(ledger(&store).terminal.is_none());
                // No acknowledgement or restart fence exists yet; explicit retry must confirm success.
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::PersistCancelled)
                        .unwrap()
                        .end,
                    FinishKind::Cancelled
                );
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_cancel_never_overwrites_an_already_committed_success_or_failure() {
    fn check<P: GroupProfile>() {
        for (cut, predecessors) in cancellation_cases() {
            let used = 10_000 - live_with_groups::<P>(10_000, cut.kind(), &predecessors).remaining;
            for (budget, kind) in [
                (used - 1, FinishKind::Failed),
                (used, FinishKind::Completed),
            ] {
                let store = Store::new();
                let old = save_with_groups::<P>(&store, cut, budget, &predecessors);
                let bytes = store.read("group-checkpoint.json");
                let done = reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::None).unwrap();
                assert_eq!(done.end, kind);
                let before = store.read("group-ledger.json");
                for fault in [
                    Fault::PersistCancelled,
                    Fault::CancelIntentWriteFailed,
                    Fault::CancelTerminalWriteFailed,
                ] {
                    assert_eq!(
                        reopen::<P>(&store, &bytes, &old, [0, 1, 2], fault).unwrap(),
                        done
                    );
                    assert_eq!(store.read("group-ledger.json"), before);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_cancel_inconsistent_terminal_records_fail_closed() {
    fn check<P: GroupProfile>() {
        for cut in [
            Cut::All {
                dispatched: 0,
                mask: 0,
            },
            Cut::Seq {
                accepted: 0,
                phase: SeqPhase::Ready,
            },
        ] {
            for corruption in 0..6 {
                let store = Store::new();
                let old = save_cut::<P>(&store, cut, 10_000);
                let bytes = store.read("group-checkpoint.json");
                reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::PersistCancelled).unwrap();
                let mut corrupt = ledger(&store);
                let done = corrupt.terminal.as_mut().unwrap();
                match corruption {
                    0 => done.value = Some(P::output()),
                    1 => done.remaining += 1,
                    2 => corrupt.cancellation_requested = false,
                    3 => done.end = FinishKind::Completed,
                    4 => done.end = FinishKind::Failed,
                    5 => {
                        done.end = FinishKind::Completed;
                        done.value = Some(P::output());
                    }
                    _ => unreachable!(),
                }
                store.write("group-ledger.json", &corrupt);
                let before = store.read("group-ledger.json");
                for fault in [Fault::None, Fault::PersistCancelled] {
                    assert!(reopen::<P>(&store, &bytes, &old, [0, 1, 2], fault).is_err());
                    assert_eq!(store.read("group-ledger.json"), before);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_cancel_does_not_relabel_a_failed_collector_restore_as_confirmed_cancellation() {
    fn check<P: GroupProfile>() {
        for cut in [
            Cut::All {
                dispatched: 0,
                mask: 0,
            },
            Cut::Seq {
                accepted: 0,
                phase: SeqPhase::Ready,
            },
        ] {
            let store = Store::new();
            let old = save_cut::<P>(&store, cut, 10_000);
            let mut image: Image =
                serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
            if cut.kind() == GroupKind::Sequence {
                image.members[0].published = true;
                image.members[1].published = true;
            } else {
                image.members[0].serial = image.members[1].serial;
            }
            store.write("group-checkpoint.json", &image);
            let before = store.read("group-ledger.json");
            assert!(
                reopen::<P>(
                    &store,
                    &store.read("group-checkpoint.json"),
                    &old,
                    [0, 1, 2],
                    Fault::PersistCancelled
                )
                .is_err()
            );
            assert_eq!(store.read("group-ledger.json"), before);
        }
    }
    check::<Counter>();
    check::<Panel>();
}

mod process_reopen {
    use super::*;
    use std::{
        fs,
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    const WORKER: &str = "mixed::disk_reopen::process_reopen::disk_process_worker";
    const DIRECTORY: &str = "LESELANG_NATIVE_PROCESS_DIRECTORY";
    const PHASE: &str = "LESELANG_NATIVE_PROCESS_PHASE";

    #[derive(Clone, Copy, Serialize, Deserialize)]
    enum NativeHost {
        Counter,
        Panel,
    }
    #[derive(Clone, Copy, Serialize, Deserialize)]
    enum Tamper {
        None,
        Candidate,
        Member,
        Graph,
        Authority,
        KillPublication(bool, PublicationPoint),
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Job {
        format: u32,
        parent: u32,
        host: NativeHost,
        cut: Cut,
        predecessors: [GroupKind; 2],
        budget: u64,
        tamper: Tamper,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Report {
        pid: u32,
        parent: u32,
        phase: String,
        outcome: Option<Finished>,
        refused: bool,
    }
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn spawn(store: &Store, phase: &str) -> (ChildGuard, std::path::PathBuf) {
        let output_path = store.path().join(format!("{phase}.log"));
        let output = fs::File::create(&output_path).unwrap();
        let child = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", WORKER, "--nocapture", "--test-threads=1"])
                .env(DIRECTORY, store.path())
                .env(PHASE, phase)
                .stdin(Stdio::null())
                .stdout(output.try_clone().unwrap())
                .stderr(output)
                .spawn()
                .unwrap(),
        );
        (child, output_path)
    }
    fn run(store: &Store, phase: &str, exits_after_intent: bool) -> Option<Report> {
        let (mut child, output_path) = spawn(store, phase);
        let pid = child.0.id();
        assert_ne!(pid, std::process::id());
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            assert!(
                fs::metadata(&output_path).unwrap().len() <= MAX_BYTES as u64,
                "native restart child exceeded output limit"
            );
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "native restart child exceeded deadline"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(
            status.code(),
            Some(if exits_after_intent { 73 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&store.read(&format!("{phase}.log")))
        );
        if exits_after_intent {
            assert_eq!(
                serde_json::from_slice::<u32>(&store.read("intent-exit-pid.json")).unwrap(),
                pid
            );
            assert!(
                !store.path().join(format!("{phase}.report.json")).exists(),
                "no cancellation acknowledgement before process exit"
            );
            return None;
        }
        let report: Report =
            serde_json::from_slice(&store.read(&format!("{phase}.report.json"))).unwrap();
        assert_eq!(report.pid, pid);
        assert_eq!(report.parent, std::process::id());
        assert_eq!(report.phase, phase);
        Some(report)
    }
    fn run_killed(store: &Store, terminal: bool, point: PublicationPoint) -> u32 {
        let (mut child, output_path) = spawn(store, "kill-publication");
        let pid = child.0.id();
        assert_ne!(pid, std::process::id());
        let deadline = Instant::now() + Duration::from_secs(30);
        while !store.path().join("kill-ready.json").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "worker exited before kill marker"
            );
            assert!(
                Instant::now() < deadline,
                "publication kill handshake timed out"
            );
            assert!(fs::metadata(&output_path).unwrap().len() <= MAX_BYTES as u64);
            std::thread::sleep(Duration::from_millis(5));
        }
        let marker: (u32, bool, PublicationPoint) =
            serde_json::from_slice(&store.read("kill-ready.json")).unwrap();
        assert_eq!(marker, (pid, terminal, point));
        assert!(child.0.try_wait().unwrap().is_none());
        child.0.kill().unwrap();
        let status = child.0.wait().unwrap();
        assert!(!status.success());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(9));
        }
        assert!(
            !store.path().join("kill-publication.report.json").exists(),
            "killed writer cannot acknowledge cancellation"
        );
        pid
    }
    fn execute<P: GroupProfile>(store: &Store, job: &Job, phase: &str) -> Report {
        let mut report = Report {
            pid: std::process::id(),
            parent: job.parent,
            phase: phase.into(),
            outcome: None,
            refused: false,
        };
        if matches!(
            phase,
            "prepare" | "cancel" | "intent-exit" | "kill-publication"
        ) {
            let old = save_with_groups::<P>(store, job.cut, job.budget, &job.predecessors);
            if phase != "prepare" {
                let fault = if phase == "kill-publication" {
                    let Tamper::KillPublication(terminal, point) = job.tamper else {
                        panic!("missing publication kill point");
                    };
                    Fault::KillPublication(terminal, point)
                } else if phase == "cancel" {
                    Fault::PersistCancelled
                } else {
                    Fault::ExitAfterCancelIntent
                };
                report.outcome = Some(
                    reopen::<P>(
                        store,
                        &store.read("group-checkpoint.json"),
                        &old,
                        [0, 1, 2],
                        fault,
                    )
                    .unwrap(),
                );
                assert_eq!(phase, "cancel", "intent exit must not return");
            }
        } else {
            let fault = match phase {
                "resume" | "refuse" => {
                    if matches!(job.tamper, Tamper::Authority) {
                        Fault::CancelDenied
                    } else {
                        Fault::None
                    }
                }
                "retry-cancel" | "replay-cancel" | "refuse-cancel" => Fault::PersistCancelled,
                _ => panic!("unknown child phase"),
            };
            let name = if matches!(job.tamper, Tamper::Candidate) {
                "candidate.json"
            } else {
                "group-checkpoint.json"
            };
            // This closed lease is only a stale-reply probe; no process-local lease crosses the wire.
            let old = LocalExecutionScope::new().lease();
            assert!(!old.is_open());
            let outcome = reopen::<P>(store, &store.read(name), &old, [2, 0, 1], fault);
            if matches!(phase, "refuse" | "refuse-cancel") {
                assert!(outcome.is_err());
                report.refused = true;
                if matches!(job.tamper, Tamper::KillPublication(..)) {
                    let image: Image = serde_json::from_slice(&store.read(name)).unwrap();
                    let mut stale = ledger(store);
                    stale.cancellation_requested = false;
                    stale.terminal = None;
                    stale_dispatch_is_fenced::<P>(store, &image, stale);
                }
            } else {
                report.outcome = Some(outcome.unwrap());
            }
        }
        report
    }

    #[test]
    fn disk_process_worker() {
        let Some(directory) = std::env::var_os(DIRECTORY) else {
            return;
        };
        let store = Store::open_existing(std::path::Path::new(&directory));
        let phase = std::env::var(PHASE).unwrap();
        assert!(
            [
                "prepare",
                "cancel",
                "intent-exit",
                "kill-publication",
                "resume",
                "refuse",
                "retry-cancel",
                "replay-cancel",
                "refuse-cancel"
            ]
            .contains(&phase.as_str())
        );
        let job: Job = serde_json::from_slice(&store.read("process-job.json")).unwrap();
        assert_eq!(job.format, 1);
        assert_ne!(job.parent, std::process::id());
        assert!((1..=10_000).contains(&job.budget));
        let report = match job.host {
            NativeHost::Counter => execute::<Counter>(&store, &job, &phase),
            NativeHost::Panel => execute::<Panel>(&store, &job, &phase),
        };
        store.write(&format!("{phase}.report.json"), &report);
    }

    fn cases() -> Vec<(Cut, [GroupKind; 2])> {
        use GroupKind::{Parallel as All, Sequence as Seq};
        let mut cases = Vec::new();
        for pair in [[Seq, All], [All, Seq]] {
            for cut in [
                Cut::All {
                    dispatched: 0,
                    mask: 0,
                },
                Cut::All {
                    dispatched: 2,
                    mask: 1,
                },
                Cut::All {
                    dispatched: 3,
                    mask: 7,
                },
                Cut::Seq {
                    accepted: 0,
                    phase: SeqPhase::Ready,
                },
                Cut::Seq {
                    accepted: 1,
                    phase: SeqPhase::Awaiting,
                },
                Cut::Seq {
                    accepted: 1,
                    phase: SeqPhase::Settled,
                },
                Cut::Seq {
                    accepted: 3,
                    phase: SeqPhase::Ready,
                },
            ] {
                cases.push((cut, pair));
            }
        }
        assert_eq!(cases.len(), 14);
        cases
    }
    fn job(
        store: &Store,
        host: NativeHost,
        cut: Cut,
        predecessors: [GroupKind; 2],
        budget: u64,
        tamper: Tamper,
    ) {
        store.write(
            "process-job.json",
            &Job {
                format: 1,
                parent: std::process::id(),
                host,
                cut,
                predecessors,
                budget,
                tamper,
            },
        );
    }

    #[test]
    fn disk_process_restores_exact_budget_and_replays_terminal_in_a_third_process() {
        fn check<P: GroupProfile>(host: NativeHost) {
            for (cut, predecessors) in cases() {
                let used =
                    10_000 - live_with_groups::<P>(10_000, cut.kind(), &predecessors).remaining;
                for budget in [used - 1, used, used + 1] {
                    let expected = live_with_groups::<P>(budget, cut.kind(), &predecessors);
                    let store = Store::new();
                    job(&store, host, cut, predecessors, budget, Tamper::None);
                    let writer = run(&store, "prepare", false).unwrap();
                    assert!(writer.outcome.is_none());
                    let reader = run(&store, "resume", false).unwrap();
                    assert_ne!(writer.pid, reader.pid);
                    assert_eq!(reader.outcome, Some(expected.clone()));
                    assert_eq!(
                        ledger(&store)
                            .replies
                            .iter()
                            .map(|r| r.serial)
                            .collect::<Vec<_>>(),
                        [2, 3, 4, 6, 7, 8, 10, 11, 12, 13]
                    );
                    let committed = store.read("group-ledger.json");
                    let replay = run(&store, "replay-cancel", false).unwrap();
                    assert_ne!(writer.pid, replay.pid);
                    assert_ne!(reader.pid, replay.pid);
                    assert_eq!(replay.outcome, Some(expected));
                    assert_eq!(store.read("group-ledger.json"), committed);
                }
            }
        }
        check::<Counter>(NativeHost::Counter);
        check::<Panel>(NativeHost::Panel);
    }

    #[test]
    fn disk_process_confirmed_cancellation_survives_two_fresh_readers() {
        for host in [NativeHost::Counter, NativeHost::Panel] {
            for (cut, predecessors) in cases() {
                let store = Store::new();
                job(&store, host, cut, predecessors, 10_000, Tamper::None);
                let writer = run(&store, "cancel", false).unwrap();
                assert_eq!(writer.outcome.as_ref().unwrap().end, FinishKind::Cancelled);
                let committed = store.read("group-ledger.json");
                let reader = run(&store, "resume", false).unwrap();
                let replay = run(&store, "replay-cancel", false).unwrap();
                assert_ne!(writer.pid, reader.pid);
                assert_ne!(reader.pid, replay.pid);
                assert_ne!(writer.pid, replay.pid);
                assert_eq!(reader.outcome, writer.outcome);
                assert_eq!(replay.outcome, writer.outcome);
                assert_eq!(store.read("group-ledger.json"), committed);
            }
        }
    }

    #[test]
    fn disk_process_exit_after_intent_blocks_restart_until_explicit_cancellation_retry() {
        for host in [NativeHost::Counter, NativeHost::Panel] {
            for (cut, predecessors) in cases() {
                let store = Store::new();
                job(&store, host, cut, predecessors, 10_000, Tamper::None);
                assert!(run(&store, "intent-exit", true).is_none());
                let writer: u32 =
                    serde_json::from_slice(&store.read("intent-exit-pid.json")).unwrap();
                assert!(ledger(&store).cancellation_requested);
                assert!(ledger(&store).terminal.is_none());
                let committed = store.read("group-ledger.json");
                let before = effects(&store);
                let blocked = run(&store, "refuse", false).unwrap();
                assert!(blocked.refused);
                assert_eq!(store.read("group-ledger.json"), committed);
                let retry = run(&store, "retry-cancel", false).unwrap();
                assert_eq!(retry.outcome.as_ref().unwrap().end, FinishKind::Cancelled);
                assert_eq!(effects(&store), before);
                let terminal = store.read("group-ledger.json");
                let replay = run(&store, "replay-cancel", false).unwrap();
                assert_eq!(replay.outcome, retry.outcome);
                assert_eq!(store.read("group-ledger.json"), terminal);
                let mut pids = [writer, blocked.pid, retry.pid, replay.pid];
                pids.sort_unstable();
                assert!(pids.windows(2).all(|pair| pair[0] != pair[1]));
            }
        }
    }

    #[test]
    fn disk_process_fresh_readers_refuse_changed_candidates_graphs_and_authority() {
        for host in [NativeHost::Counter, NativeHost::Panel] {
            for tamper in [
                Tamper::Candidate,
                Tamper::Member,
                Tamper::Graph,
                Tamper::Authority,
            ] {
                let store = Store::new();
                let cut = Cut::All {
                    dispatched: 2,
                    mask: 1,
                };
                job(
                    &store,
                    host,
                    cut,
                    [GroupKind::Sequence, GroupKind::Parallel],
                    10_000,
                    tamper,
                );
                let writer = run(&store, "prepare", false).unwrap();
                let mut image: Image =
                    serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
                let file = match tamper {
                    Tamper::Candidate => {
                        image.remaining += 1;
                        "candidate.json"
                    }
                    Tamper::Member => {
                        image.members[0].serial = image.members[1].serial;
                        "group-checkpoint.json"
                    }
                    Tamper::Graph => {
                        image.completed.swap(0, 1);
                        "group-checkpoint.json"
                    }
                    Tamper::Authority => "group-checkpoint.json",
                    Tamper::None => unreachable!(),
                    Tamper::KillPublication(..) => unreachable!(),
                };
                store.write(file, &image);
                let before = store.read("group-ledger.json");
                let reader = run(&store, "refuse", false).unwrap();
                assert_ne!(writer.pid, reader.pid);
                assert!(reader.refused);
                assert_eq!(store.read("group-ledger.json"), before);
            }
        }
    }

    #[test]
    fn disk_process_killed_publication_never_admits_or_promotes_uncommitted_bytes() {
        let mut cases = 0;
        for host in [NativeHost::Counter, NativeHost::Panel] {
            for cut in [
                Cut::All {
                    dispatched: 2,
                    mask: 1,
                },
                Cut::Seq {
                    accepted: 1,
                    phase: SeqPhase::Awaiting,
                },
            ] {
                for terminal in [false, true] {
                    for point in [
                        PublicationPoint::PartialWrite,
                        PublicationPoint::FileSynced,
                        PublicationPoint::Renamed,
                    ] {
                        let store = Store::new();
                        job(
                            &store,
                            host,
                            cut,
                            [GroupKind::Sequence, GroupKind::Parallel],
                            10_000,
                            Tamper::KillPublication(terminal, point),
                        );
                        let writer = run_killed(&store, terminal, point);
                        let pending = store.path().join("group-ledger.json.pending");
                        let published = point == PublicationPoint::Renamed;
                        assert_eq!(pending.exists(), !published);
                        let before = store.read("group-ledger.json");
                        let before_effects = effects(&store);
                        assert_eq!(ledger(&store).cancellation_requested, terminal || published);
                        assert_eq!(ledger(&store).terminal.is_some(), terminal && published);
                        let reader = if terminal && published {
                            run(&store, "resume", false).unwrap()
                        } else {
                            let refused = run(&store, "refuse", false).unwrap();
                            assert!(refused.refused);
                            refused
                        };
                        let mut pids = vec![writer, reader.pid];
                        assert_eq!(store.read("group-ledger.json"), before);
                        if !published {
                            let bytes = store.read("group-ledger.json.pending");
                            let refused_retry = run(&store, "refuse-cancel", false).unwrap();
                            assert!(refused_retry.refused);
                            assert_ne!(refused_retry.pid, writer);
                            assert_ne!(refused_retry.pid, reader.pid);
                            pids.push(refused_retry.pid);
                            assert_eq!(store.read("group-ledger.json"), before);
                            assert_eq!(store.read("group-ledger.json.pending"), bytes);
                            if point == PublicationPoint::PartialWrite {
                                assert!(serde_json::from_slice::<Ledger>(&bytes).is_err());
                            } else {
                                let staged: Ledger = serde_json::from_slice(&bytes).unwrap();
                                assert!(staged.cancellation_requested);
                                assert_eq!(staged.terminal.is_some(), terminal);
                            }
                            // Only this parent owns the directory and has reaped its sole writer.
                            // Discard, never publish, the orphan before an explicit fresh cancel retry.
                            fs::remove_file(&pending).unwrap();
                        }
                        let retry = run(&store, "retry-cancel", false).unwrap();
                        assert_eq!(retry.outcome.as_ref().unwrap().end, FinishKind::Cancelled);
                        let image: Image =
                            serde_json::from_slice(&store.read("group-checkpoint.json")).unwrap();
                        assert_eq!(retry.outcome.as_ref().unwrap().remaining, image.remaining);
                        let committed = store.read("group-ledger.json");
                        let replay = run(&store, "replay-cancel", false).unwrap();
                        assert_eq!(replay.outcome, retry.outcome);
                        assert_eq!(store.read("group-ledger.json"), committed);
                        assert_eq!(effects(&store), before_effects);
                        assert!(!pending.exists());
                        pids.extend([retry.pid, replay.pid]);
                        pids.sort_unstable();
                        assert!(pids.windows(2).all(|p| p[0] != p[1]));
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 24);
    }
}

#[test]
fn disk_unpublished_nodes_block_reopen_and_stale_dispatch_without_cleanup() {
    fn check<P: GroupProfile>() {
        for cut in [
            Cut::All {
                dispatched: 2,
                mask: 1,
            },
            Cut::Seq {
                accepted: 1,
                phase: SeqPhase::Awaiting,
            },
        ] {
            for directory in [false, true] {
                let store = Store::new();
                let old = save_cut::<P>(&store, cut, 10_000);
                let bytes = store.read("group-checkpoint.json");
                let image: Image = serde_json::from_slice(&bytes).unwrap();
                let committed = store.read("group-ledger.json");
                let pending = store.path().join("group-ledger.json.pending");
                if directory {
                    std::fs::create_dir(&pending).unwrap();
                } else {
                    std::fs::write(&pending, b"{").unwrap();
                }
                for fault in [Fault::None, Fault::PersistCancelled, Fault::CancelDenied] {
                    assert!(reopen::<P>(&store, &bytes, &old, [0, 1, 2], fault).is_err());
                    assert_eq!(store.read("group-ledger.json"), committed);
                    assert!(
                        pending.exists(),
                        "recovery never removes unresolved publication"
                    );
                }
                stale_dispatch_is_fenced::<P>(&store, &image, ledger(&store));
                assert!(
                    store
                        .replace("group-ledger.json", &ledger(&store), false)
                        .is_err()
                );
                assert_eq!(store.read("group-ledger.json"), committed);
                if directory {
                    assert!(std::fs::read_dir(&pending).unwrap().next().is_none());
                    std::fs::remove_dir(&pending).unwrap();
                } else {
                    assert_eq!(store.read("group-ledger.json.pending"), b"{");
                    std::fs::remove_file(&pending).unwrap();
                }
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, [0, 1, 2], Fault::PersistCancelled)
                        .unwrap()
                        .end,
                    FinishKind::Cancelled
                );
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}
