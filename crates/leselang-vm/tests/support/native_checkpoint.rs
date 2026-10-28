//! Host-owned disk images exercise the original native VM, not a production journal.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_IMAGE_BYTES: usize = 128 * 1024;

pub(super) trait DiskProfile: Profile {
    fn state(&self) -> Value;
    fn load(&self, state: Value) -> Result<(), Refusal>;
    fn encode(value: &Self::Value) -> Value;
    fn decode(value: Value) -> Result<Self::Value, Refusal>;
}
impl DiskProfile for Counter {
    fn state(&self) -> Value {
        self.0.get().into()
    }
    fn load(&self, value: Value) -> Result<(), Refusal> {
        self.0
            .set(value.as_u64().filter(|v| *v <= 100).ok_or(Refusal)?);
        Ok(())
    }
    fn encode(value: &ScalarValue) -> Value {
        serde_json::to_value(value).unwrap()
    }
    fn decode(value: Value) -> Result<ScalarValue, Refusal> {
        let value = serde_json::from_value(value).map_err(|_| Refusal)?;
        Self::validate(&value)?;
        Ok(value)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ack {
    text: String,
    visible: bool,
}
impl DiskProfile for Panel {
    fn state(&self) -> Value {
        self.0.borrow().clone().into()
    }
    fn load(&self, value: Value) -> Result<(), Refusal> {
        self.0.replace(
            value
                .as_str()
                .filter(|v| v.len() <= 64)
                .ok_or(Refusal)?
                .into(),
        );
        Ok(())
    }
    fn encode(value: &WidgetAck) -> Value {
        serde_json::to_value(Ack {
            text: value.text.clone(),
            visible: value.visible,
        })
        .unwrap()
    }
    fn decode(value: Value) -> Result<WidgetAck, Refusal> {
        let Ack { text, visible } = serde_json::from_value(value).map_err(|_| Refusal)?;
        let value = WidgetAck { text, visible };
        Self::validate(&value)?;
        Ok(value)
    }
}

pub(super) struct Store {
    path: PathBuf,
    cleanup: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum PublicationPoint {
    PartialWrite,
    FileSynced,
    Renamed,
}
impl Store {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "leselang-native-checkpoint-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self {
            path,
            cleanup: true,
        }
    }
    pub(super) fn open_existing(path: &std::path::Path) -> Self {
        assert!(path.is_absolute());
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_dir());
        Self {
            path: path.into(),
            cleanup: false,
        }
    }
    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub(super) fn require_published(&self, name: &str) -> std::io::Result<()> {
        match fs::symlink_metadata(self.path.join(format!("{name}.pending"))) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
            Ok(_) => Err(std::io::Error::other("unresolved fixture publication")),
        }
    }
    pub(super) fn write<T: Serialize>(&self, name: &str, value: &T) {
        let bytes = serde_json::to_vec(value).unwrap();
        assert!(bytes.len() <= MAX_IMAGE_BYTES);
        fs::write(self.path.join(name), bytes).unwrap();
    }
    // Serialized fixture writer only; rename is not a cross-writer transaction or power-loss proof.
    pub(super) fn replace<T: Serialize>(
        &self,
        name: &str,
        value: &T,
        fail_before_publish: bool,
    ) -> std::io::Result<()> {
        self.replace_with_hook(name, value, fail_before_publish, |_| {})
    }
    pub(super) fn replace_with_hook<T: Serialize>(
        &self,
        name: &str,
        value: &T,
        fail_before_publish: bool,
        mut hook: impl FnMut(PublicationPoint),
    ) -> std::io::Result<()> {
        use std::io::Write;
        let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(std::io::Error::other("fixture image limit"));
        }
        let pending = self.path.join(format!("{name}.pending"));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        let result = (|| {
            let split = bytes.len() / 2;
            file.write_all(&bytes[..split])?;
            hook(PublicationPoint::PartialWrite);
            file.write_all(&bytes[split..])?;
            file.sync_all()?;
            hook(PublicationPoint::FileSynced);
            drop(file);
            if fail_before_publish {
                return Err(std::io::Error::other(
                    "injected fixture publication refusal",
                ));
            }
            fs::rename(&pending, self.path.join(name))?;
            hook(PublicationPoint::Renamed);
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(pending);
        }
        result
    }
    pub(super) fn read(&self, name: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        fs::File::open(self.path.join(name))
            .unwrap()
            .take(MAX_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= MAX_IMAGE_BYTES);
        bytes
    }
    fn ledger(&self) -> Ledger {
        serde_json::from_slice(&self.read("ledger.json")).unwrap()
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
enum Captured {
    Scalar(ScalarValue),
    Native(Value),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    site: Vec<usize>,
    name: String,
    capture: Vec<(String, Captured)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operand {
    parameter: usize,
    argument: usize,
    name: String,
    value: ScalarValue,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Call {
    operation: String,
    operands: Vec<Operand>,
}
impl Call {
    pub(super) fn save<P: Profile>(call: &Request<'_, P>) -> Self {
        Self {
            operation: call.schema().key.clone(),
            operands: call
                .arguments()
                .iter()
                .map(|a| Operand {
                    parameter: a.parameter_index,
                    argument: a.argument_index,
                    name: a.name.into(),
                    value: a.value.clone(),
                })
                .collect(),
        }
    }
    pub(super) fn restore<'s, P: Profile>(
        &self,
        host: &Host<'_, 's, P>,
        identity: &Identity,
    ) -> Result<Request<'s, P>, Refusal> {
        let source = host.source();
        let schema = source
            .catalog
            .authorize(&self.operation, source.version, source.granted)
            .map_err(|_| Refusal)?;
        let operands = self
            .operands
            .iter()
            .map(|arg| call_evaluation::PreparedArgument {
                parameter_index: arg.parameter,
                argument_index: arg.argument,
                name: arg.name.as_str(),
                value: arg.value.clone(),
            })
            .collect();
        PreparedCall::restore(schema, operands, EFFECTS.max_arguments, |_, _| {
            host.authorize(identity)
        })
        .map_err(|_| Refusal)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    format: u32,
    source: String,
    ir: Node,
    catalog: u32,
    revision: u64,
    serial: u64,
    remaining: u64,
    limits: [usize; 6],
    frames: Vec<Frame>,
    call: Call,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settled {
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
    settled: Vec<Settled>,
    terminal: Option<Finished>,
}

pub(super) fn limits() -> [usize; 6] {
    [
        PURE.max_nodes,
        PURE.max_depth,
        PURE.max_bindings,
        EFFECTS.max_arguments,
        EFFECTS.max_branches,
        STACK.max_frames,
    ]
}
pub(super) fn frame_site(root: &Node, name: &str, body: &Node) -> Vec<usize> {
    let mut pending = vec![(root, Vec::new())];
    while let Some((node, path)) = pending.pop() {
        if let Node::Bind {
            name: actual,
            body: actual_body,
            ..
        } = node
            && actual == name
            && std::ptr::eq(actual_body.as_ref(), body)
        {
            return path;
        }
        for (index, child) in node.children().enumerate() {
            let mut path = path.clone();
            path.push(index);
            pending.push((child, path));
        }
    }
    panic!("live return site must belong to the original IR")
}
pub(super) fn node_at<'a>(root: &'a Node, path: &[usize]) -> Result<&'a Node, Refusal> {
    if path.len() > PURE.max_depth {
        return Err(Refusal);
    }
    path.iter().try_fold(root, |node, index| {
        node.children().nth(*index).ok_or(Refusal)
    })
}

// The fixture owns effect deduplication and state persistence. No VM algorithm is duplicated.
fn settle<P: DiskProfile>(
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
    if let Some(row) = ledger.settled.iter().find(|row| row.serial == serial) {
        if row.input != input {
            return Err(Refusal);
        }
        return Ok(Reply {
            value: P::decode(row.reply.clone())?,
            buffer: Box::new(23),
        });
    }
    if serial != ledger.settled.len() as u64 + 1 {
        return Err(Refusal);
    }
    let reply = host.dispatch(dispatch)?;
    ledger.settled.push(Settled {
        serial,
        input,
        reply: P::encode(&reply.value),
    });
    ledger.state = host.profile.state();
    store.write("ledger.json", ledger);
    Ok(reply)
}

fn save<P: DiskProfile>(
    store: &Store,
    cut: usize,
    dispatched: bool,
    budget: u64,
) -> LocalExecutionLease {
    let mut lease = None;
    fixture::<P>(|host, scope| {
        lease = Some(scope.lease());
        let root = compile(P::PROGRAM, &host).unwrap();
        let mut ledger = Ledger {
            state: host.profile.state(),
            settled: Vec::new(),
            terminal: None,
        };
        store.write("ledger.json", &ledger);
        let mut owner = scope.bind(Some(PipelineVm::<P>::new(
            &root,
            Vec::new(),
            Fuel::new(budget),
            STACK,
        )));
        for index in 0..=cut {
            let (scope, slot) = owner.as_parts_mut();
            let vm = slot.as_mut().unwrap();
            let EffectSessionPoll::Request(dispatch) = vm.poll_in_scope(&host, scope).unwrap()
            else {
                panic!("pending call")
            };
            if index < cut {
                let identity = dispatch.identity.clone();
                let reply = settle(&host, dispatch, store, &mut ledger).unwrap();
                vm.try_accept_in_scope(&identity, reply, &host, scope)
                    .unwrap();
                continue;
            }
            let call = Call::save(&dispatch.request);
            if dispatched {
                drop(settle(&host, dispatch, store, &mut ledger).unwrap());
            }
            let saved = slot.take().unwrap().try_into_pending_binding().unwrap();
            let frames = saved
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
                                (
                                    name.into(),
                                    match value {
                                        PureValue::Scalar(value) => Captured::Scalar(value),
                                        PureValue::Result(value) => {
                                            Captured::Native(P::encode(&value))
                                        }
                                    },
                                )
                            })
                            .collect(),
                    }
                })
                .collect();
            store.write(
                "checkpoint.json",
                &Image {
                    format: 1,
                    source: P::PROGRAM.into(),
                    ir: root.clone(),
                    catalog: host.version.get(),
                    revision: saved.identity.revision,
                    serial: saved.identity.serial,
                    remaining: saved.fuel.remaining(),
                    limits: limits(),
                    frames,
                    call,
                },
            );
        }
    });
    let lease = lease.unwrap();
    assert!(!lease.is_open());
    lease
}

#[derive(Clone, Copy)]
enum Policy {
    Allowed,
    DeniedAtRestore,
    DeniedAtDispatch,
}

fn reopen<P: DiskProfile>(
    store: &Store,
    candidate: &[u8],
    old: &LocalExecutionLease,
    policy: Policy,
) -> Result<Finished, Refusal> {
    // A trusted fixture record supplies correspondence; byte equality is not a signature.
    if candidate.len() > MAX_IMAGE_BYTES || candidate != store.read("checkpoint.json") {
        return Err(Refusal);
    }
    let image: Image = serde_json::from_slice(candidate).map_err(|_| Refusal)?;
    if image.format != 1
        || image.source != P::PROGRAM
        || image.catalog != 31
        || image.revision != 1
        || image.limits != limits()
        || !(1..=2).contains(&image.serial)
        || image.frames.is_empty()
        || image.frames.len() > STACK.max_frames
    {
        return Err(Refusal);
    }
    let mut outcome = Err(Refusal);
    fixture::<P>(|host, scope| {
        outcome = (|| {
            let mut ledger = store.ledger();
            host.profile.load(ledger.state.clone())?;
            host.invoked
                .replace(ledger.settled.iter().map(|row| row.input.clone()).collect());
            host.serial.set(image.serial);
            host.granted.set(!matches!(policy, Policy::DeniedAtRestore));
            let identity = Identity {
                owner: host.lease.clone(),
                serial: image.serial,
                revision: image.revision,
            };
            host.authorize(&identity)?;
            if let Some(terminal) = ledger.terminal {
                return Ok(terminal);
            }
            let root = compile(&image.source, &host).map_err(|_| Refusal)?;
            if root != image.ir {
                return Err(Refusal);
            }
            let frames = image
                .frames
                .iter()
                .map(|frame| {
                    let Node::Bind { name, body, .. } = node_at(&root, &frame.site)? else {
                        return Err(Refusal);
                    };
                    if *name != frame.name || frame.capture.len() > PURE.max_bindings {
                        return Err(Refusal);
                    }
                    let capture = frame
                        .capture
                        .iter()
                        .map(|(name, value)| {
                            Ok((
                                name.as_str(),
                                match value {
                                    Captured::Scalar(value) => PureValue::Scalar(value.clone()),
                                    Captured::Native(value) => {
                                        PureValue::Result(P::decode(value.clone())?)
                                    }
                                },
                            ))
                        })
                        .collect::<Result<_, Refusal>>()?;
                    Ok(effect_stack::SavedEffectReturnFrame {
                        name: name.as_str(),
                        body: body.as_ref(),
                        capture,
                    })
                })
                .collect::<Result<Vec<_>, Refusal>>()?;
            let source = host.source();
            let schema = source
                .catalog
                .authorize(&image.call.operation, source.version, source.granted)
                .map_err(|_| Refusal)?;
            let mut vm = LocalVm::restore_pending(
                scope,
                identity.clone(),
                &schema.result,
                frames,
                Fuel::new(image.remaining),
                STACK,
                |id, _, _, fuel| {
                    host.authorize(id)?;
                    if fuel.remaining() != image.remaining {
                        return Err(Refusal);
                    }
                    Ok(())
                },
            )
            .map_err(|_| Refusal)?;
            assert_eq!(vm.fuel_remaining(), image.remaining);
            assert!(matches!(
                vm.poll(&host).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert!(!old.same_scope(&vm.lease()));
            host.granted
                .set(!matches!(policy, Policy::DeniedAtDispatch));
            let request = image.call.restore(&host, &identity)?;
            let mut reply = settle(
                &host,
                Dispatch {
                    identity: identity.clone(),
                    request,
                },
                store,
                &mut ledger,
            )?;
            // The old local capability cannot be relabelled as the new owner, even at the same serial.
            let stale = Identity {
                owner: old.clone(),
                serial: identity.serial,
                revision: identity.revision,
            };
            let pointer = &*reply.buffer as *const u8;
            reply = vm.try_accept(&stale, reply, &host).unwrap_err().reply;
            assert_eq!(&*reply.buffer as *const u8, pointer);
            assert_eq!(vm.fuel_remaining(), image.remaining);
            vm.try_accept(&identity, reply, &host).unwrap();
            loop {
                match vm.poll(&host) {
                    Ok(EffectSessionPoll::Request(dispatch)) => {
                        let identity = dispatch.identity.clone();
                        let reply = settle(&host, dispatch, store, &mut ledger)?;
                        vm.try_accept(&identity, reply, &host).unwrap();
                    }
                    Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                        assert_eq!(value, P::expected());
                        let finished = Finished {
                            value: Some(value),
                            remaining: vm.fuel_remaining(),
                        };
                        ledger.terminal = Some(finished.clone());
                        store.write("ledger.json", &ledger);
                        assert!(!vm.lease().is_open());
                        return Ok(finished);
                    }
                    Err(_) => {
                        assert!(!vm.lease().is_open());
                        assert_eq!(
                            vm.status(),
                            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                        );
                        let finished = Finished {
                            value: None,
                            remaining: vm.fuel_remaining(),
                        };
                        ledger.terminal = Some(finished.clone());
                        store.write("ledger.json", &ledger);
                        return Ok(finished);
                    }
                    _ => return Err(Refusal),
                }
            }
        })();
    });
    outcome
}

#[test]
fn disk_reopen_rebuilds_two_hosts_and_keeps_settled_calls_once_only() {
    fn check<P: DiskProfile>() {
        let remaining = complete::<P>(10_000).unwrap();
        for cut in 0..2 {
            for dispatched in [false, true] {
                let store = Store::new();
                let old = save::<P>(&store, cut, dispatched, 10_000);
                let bytes = store.read("checkpoint.json");
                let done = reopen::<P>(&store, &bytes, &old, Policy::Allowed).unwrap();
                assert_eq!(
                    done,
                    Finished {
                        value: Some(P::expected()),
                        remaining
                    }
                );
                let ledger = store.read("ledger.json");
                assert_eq!(
                    store
                        .ledger()
                        .settled
                        .iter()
                        .map(|r| r.input.clone())
                        .collect::<Vec<_>>(),
                    P::arguments()
                );
                assert_eq!(
                    reopen::<P>(&store, &bytes, &old, Policy::Allowed).unwrap(),
                    done
                );
                assert_eq!(store.read("ledger.json"), ledger);
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_reopen_preserves_exact_completion_fuel_without_refilling_saved_frames() {
    fn check<P: DiskProfile>() {
        let used = 10_000 - complete::<P>(10_000).unwrap();
        for cut in 0..2 {
            for dispatched in [false, true] {
                for budget in [used - 1, used, used + 1] {
                    let store = Store::new();
                    let old = save::<P>(&store, cut, dispatched, budget);
                    let outcome = reopen::<P>(
                        &store,
                        &store.read("checkpoint.json"),
                        &old,
                        Policy::Allowed,
                    )
                    .unwrap();
                    assert_eq!(outcome.value.is_some(), budget >= used);
                    if budget >= used {
                        assert_eq!(outcome.remaining, budget - used);
                    }
                    assert_eq!(store.ledger().settled.len(), 2);
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn changed_disk_candidates_never_reconstruct_or_dispatch_from_self_asserted_metadata() {
    fn check<P: DiskProfile>() {
        for cut in 0..2 {
            for dispatched in [false, true] {
                let store = Store::new();
                let old = save::<P>(&store, cut, dispatched, 10_000);
                let original = store.read("checkpoint.json");
                let ledger = store.read("ledger.json");
                for field in [
                    "format",
                    "source",
                    "ir",
                    "catalog",
                    "revision",
                    "serial",
                    "remaining",
                    "limits",
                    "frames",
                    "call",
                ] {
                    let mut forged: Value = serde_json::from_slice(&original).unwrap();
                    forged[field] = Value::Null;
                    assert!(
                        reopen::<P>(
                            &store,
                            &serde_json::to_vec(&forged).unwrap(),
                            &old,
                            Policy::Allowed
                        )
                        .is_err()
                    );
                }
                for bytes in [
                    &original[..original.len() - 1],
                    &vec![b' '; MAX_IMAGE_BYTES + 1],
                ] {
                    assert!(reopen::<P>(&store, bytes, &old, Policy::Allowed).is_err());
                }
                assert_eq!(store.read("ledger.json"), ledger);
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn disk_reopen_requires_current_authority_before_restore_and_before_cached_reply_use() {
    fn check<P: DiskProfile>() {
        for cut in 0..2 {
            for dispatched in [false, true] {
                for policy in [Policy::DeniedAtRestore, Policy::DeniedAtDispatch] {
                    let store = Store::new();
                    let old = save::<P>(&store, cut, dispatched, 10_000);
                    let ledger = store.read("ledger.json");
                    assert!(
                        reopen::<P>(&store, &store.read("checkpoint.json"), &old, policy).is_err()
                    );
                    assert_eq!(store.read("ledger.json"), ledger);
                    assert!(!old.is_open());
                }
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}
