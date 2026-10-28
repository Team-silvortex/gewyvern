//! A small, in-memory host using only the public native facade.
//! The bounded host loop is not a general scheduler, durable backend or GUI adapter.
use std::{cell::Cell, collections::VecDeque, process::ExitCode};

use call_evaluation::{CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope};
use catalog_program::{CatalogProgramLimits, compile_catalog_program};
use catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits};
use control_source::ControlSourceLimits;
use effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits};
use effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings};
use effect_session::{
    CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
    EffectSessionPoll, EffectSessionStatus,
};
use effect_stack::EffectStackLimits;
use flow_typing::CallFlowEnvironment;
use leselang_vm::native::{HostWorkOutcome, LocalVm, core::*, hir::*, syntax::parse};
use pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, scalar_copy_cost,
};
use pure_typing::{PureType, PureTypeEnvironment};
use source_call::{SourceCallHost, SourceCallLimits};

const SOURCE: &str = r#"
fn increment(n: integer) = bind(receipt: counter.add(amount: n), body: receipt)
fn main() = bind(first: increment(n: 19), body: increment(n: add(left: first, right: 4)))
"#;
const STACK: EffectStackLimits = EffectStackLimits {
    effects: EffectEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
        },
        max_arguments: 8,
        max_branches: 8,
    },
    max_frames: 8,
};
const PROGRAM: CatalogProgramLimits = CatalogProgramLimits {
    expression: CatalogSourceLimits {
        control: ControlSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 256,
                max_source_depth: 32,
                max_lowered_nodes: 256,
                max_lowered_depth: 32,
                max_arguments: 8,
            },
            max_bindings: 8,
        },
        max_calls: 8,
    },
    max_functions: 8,
    max_parameters: 8,
    max_compilation_nodes: 1024,
    max_expansion_depth: 16,
};
type Error = &'static str;
type Node = ir::Computation<(), String, (), ()>;
type Catalog<'s> = OperationCatalog<'s, String, &'s str, ScalarTypeSet, ScalarTypeSet, u8>;
type Request<'s> = PreparedCall<'s, String, ScalarTypeSet, ScalarTypeSet, u8>;

#[derive(Clone)]
struct Identity {
    owner: LocalExecutionLease,
    serial: u64,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_scope(&other.owner) && self.serial == other.serial
    }
}
struct Work<'s> {
    identity: Identity,
    request: Request<'s>,
}
struct Host<'h, 's> {
    catalog: &'h Catalog<'s>,
    lease: LocalExecutionLease,
    granted: Cell<bool>,
    serial: Cell<u64>,
    settled: Cell<u64>,
    total: Cell<u64>,
}
impl<'s> Host<'_, 's> {
    fn new<'h>(
        catalog: &'h Catalog<'s>,
        lease: LocalExecutionLease,
        granted: bool,
    ) -> Host<'h, 's> {
        Host {
            catalog,
            lease,
            granted: Cell::new(granted),
            serial: Cell::new(0),
            settled: Cell::new(0),
            total: Cell::new(0),
        }
    }
    fn source(&self) -> SourceCallHost<'_, 's, String, ScalarTypeSet, ScalarTypeSet, u8> {
        SourceCallHost {
            catalog: self.catalog,
            version: self.catalog.version(),
            granted: if self.granted.get() { &[1] } else { &[] },
        }
    }
    fn schema(
        &self,
        name: &str,
    ) -> Result<&'s OperationSchema<'s, String, &'s str, ScalarTypeSet, ScalarTypeSet, u8>, Error>
    {
        self.catalog
            .authorize(name, self.catalog.version(), self.source().granted)
            .map_err(|_| "operation denied")
    }
    fn invoke(&self, work: Work<'s>) -> Result<(Identity, ScalarValue), Error> {
        self.authorize(&work.identity)?;
        if !std::ptr::eq(
            self.schema(&work.request.schema().key)?,
            work.request.schema(),
        ) || self.settled.get().checked_add(1) != Some(work.identity.serial)
        {
            return Err("stale dispatch");
        }
        let (_, arguments) = work.request.into_parts();
        let mut arguments = arguments.into_iter();
        let Some(argument) = arguments.next() else {
            return Err("missing amount");
        };
        let ScalarValue::Integer(amount) = argument.value else {
            return Err("invalid amount");
        };
        if arguments.next().is_some() {
            return Err("extra operand");
        }
        let total = self
            .total
            .get()
            .checked_add(amount)
            .filter(|value| *value <= 100)
            .ok_or("counter limit exceeded")?;
        // Only this adapter mutates native state, after all current-policy checks.
        self.total.set(total);
        self.settled.set(work.identity.serial);
        Ok((work.identity, ScalarValue::Integer(total)))
    }
}
impl ReplyAuthority<Identity> for Host<'_, '_> {
    type Error = Error;
    fn authorize(&self, id: &Identity) -> Result<(), Error> {
        if !self.lease.is_open()
            || !id.owner.same_scope(&self.lease)
            || id.serial == 0
            || id.serial != self.serial.get()
            || !self.granted.get()
        {
            Err("execution authority revoked")
        } else {
            Ok(())
        }
    }
}
impl PureTypeEnvironment<(), String> for Host<'_, '_> {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &String) -> Option<()> {
        None
    }
}
impl<'s> CallFlowEnvironment<'s, (), String, ScalarTypeSet> for Host<'_, 's> {
    fn call_result_type(&self, _: &String, _: &'s ScalarTypeSet) -> Option<PureType<()>> {
        Some(PureType::Scalar(ScalarType::Integer))
    }
}
impl<'a, 's> CatalogSourceEnvironment<'a, 's, (), String, ScalarTypeSet> for Host<'_, 's> {
    type Error = Error;
    fn source_operation(&self, name: &String) -> Result<String, Error> {
        Ok(name.clone())
    }
    fn source_field(&self, _: &(), _: &'a str) -> Result<Option<((), ScalarType)>, Error> {
        Ok(None)
    }
    fn source_member(&self, _: &(), _: &'a str, _: &'a str) -> Result<Option<(String, ())>, Error> {
        Ok(None)
    }
}
impl PureEvaluationEnvironment<(), String> for Host<'_, '_> {
    type Result = ();
    type Error = Error;
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, Error> {
        Err("no native fields")
    }
    fn member(&self, _: &(), _: &str, _: &String) -> Result<(), Error> {
        Err("no native groups")
    }
}
impl<'ir, 's> EffectEvaluationEnvironment<'ir, (), String, (), ()> for Host<'_, 's> {
    type Request = Request<'s>;
    type Capture = RestoredEffectBindings<'ir, ()>;
    fn preflight_effect(&self, node: &'ir Node) -> Result<(), Error> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err("only atomic calls are adapted");
        };
        self.schema(operation)?
            .bind_arguments(
                &arguments
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| "invalid signature")?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Error>> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err("only atomic calls are adapted".into());
        };
        evaluate_call_arguments_in_scope(
            arguments,
            self.schema(operation)?,
            scope,
            self,
            fuel,
            CallEvaluationLimits {
                pure: STACK.effects.pure,
                max_arguments: STACK.effects.max_arguments,
            },
        )
        .map_err(|_| "call preparation failed".into())
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node,
        scope: &ScopeFrame<'_, 'ir, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Error> {
        let mut cost = 0_u64;
        for (_, value) in scope.bindings() {
            let PureValue::Scalar(value) = value else {
                return Err("unexpected native capture");
            };
            cost = cost
                .checked_add(1 + scalar_copy_cost(value))
                .ok_or("capture limit")?;
        }
        fuel.charge(cost).map_err(|_| "capture fuel exhausted")?;
        Ok(scope.bindings().to_vec())
    }
}
impl<'ir, 's>
    EffectReentryEnvironment<'ir, (), String, (), (), Identity, ScalarTypeSet, ScalarValue>
    for Host<'_, 's>
{
    fn restore_capture(
        &self,
        id: &Identity,
        _: &ScalarTypeSet,
        capture: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Error>> {
        self.authorize(id)?;
        Ok(capture)
    }
    fn bind_reply(
        &self,
        id: &Identity,
        declaration: &ScalarTypeSet,
        reply: ScalarValue,
        _: &mut Fuel,
    ) -> Result<PureValue<()>, CalculationFailure<Error>> {
        self.authorize(id)?;
        declaration
            .validate(&reply)
            .map_err(|_| "invalid counter reply")?;
        Ok(PureValue::Scalar(reply))
    }
}
impl<'ir, 's> EffectSessionEnvironment<'ir, 's, (), String, (), ()> for Host<'_, 's> {
    type Identity = Identity;
    type Declaration = ScalarTypeSet;
    type Reply = ScalarValue;
    type Dispatch = Work<'s>;
    fn correlate_request(
        &self,
        request: Self::Request,
        _: &mut Fuel,
    ) -> EffectCorrelation<'s, Identity, ScalarTypeSet, Work<'s>, Error> {
        let schema = self.schema(&request.schema().key)?;
        if !std::ptr::eq(schema, request.schema()) {
            return Err("foreign schema".into());
        }
        let serial = self
            .serial
            .get()
            .checked_add(1)
            .ok_or("identity exhausted")?;
        self.serial.set(serial);
        let identity = Identity {
            owner: self.lease.clone(),
            serial,
        };
        Ok(CorrelatedEffectRequest {
            identity: identity.clone(),
            declaration: &schema.result,
            dispatch: Work { identity, request },
        })
    }
}

#[derive(Clone, Copy)]
enum Control {
    Run,
    Deny,
    CancelAfter(u64),
    RevokeAfter(u64),
}
#[derive(Debug, PartialEq)]
struct Summary {
    end: EffectSessionEnd,
    value: Option<u64>,
    total: u64,
    calls: u64,
    remaining: u64,
}
#[derive(Debug)]
struct Failure {
    message: String,
    total: u64,
    calls: u64,
    remaining: Option<u64>,
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            message,
            total: 0,
            calls: 0,
            remaining: None,
        }
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (total={}, calls={}, remaining={:?}; prior effects are not rolled back)",
            self.message, self.total, self.calls, self.remaining
        )
    }
}

fn with_catalog<T>(
    use_catalog: impl FnOnce(&Catalog<'_>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let parameters = [NamedParameter::required(
        "amount",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: "counter.add".to_owned(),
        parameters: &parameters,
        result: ScalarTypeSet::only(ScalarType::Integer),
        required_capability: 1,
    }];
    let catalog = OperationCatalog::new(
        1,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .map_err(|_| "invalid host catalog")?;
    use_catalog(&catalog)
}

fn compile(source: &str, host: &Host<'_, '_>) -> Result<Node, Failure> {
    if source.len() > 16 * 1024 {
        return Err("source byte limit exceeded".into());
    }
    let parsed = parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err("syntax rejected".into());
    }
    let declarations: Vec<_> = parsed.function.iter().chain(&parsed.helpers).collect();
    let (node, _) = compile_catalog_program::<(), _, (), (), _, _, _, _>(
        &declarations,
        "main",
        PROGRAM,
        &host.source(),
        host,
    )
    .map_err(|e| e.to_string())?;
    Ok(node)
}

fn run(source: &str, budget: u64, control: Control) -> Result<Summary, Failure> {
    with_catalog(|catalog| {
        let owner = LocalExecutionScope::new();
        let host = Host::new(catalog, owner.lease(), !matches!(control, Control::Deny));
        let node = compile(source, &host)?;
        let mut vm = LocalVm::new(owner, &node, Vec::new(), Fuel::new(budget), STACK);
        let result = (|| -> Result<Summary, String> {
            let (end, value) = loop {
                match vm.poll(&host).map_err(|_| "language execution refused")? {
                    EffectSessionPoll::Request(work) => {
                        if matches!(control, Control::CancelAfter(n) if host.settled.get() == n) {
                            vm.cancel();
                            continue;
                        }
                        if matches!(control, Control::RevokeAfter(n) if host.settled.get() == n) {
                            host.granted.set(false);
                        }
                        // The VM guards host work; invoking it never implicitly accepts a reply.
                        let reply = vm
                            .with_host_outcome(work, |work| match host.invoke(work) {
                                Ok(reply) => HostWorkOutcome::Pending(Ok(reply)),
                                Err(error) => HostWorkOutcome::Failed(Err(error)),
                            })
                            .map_err(|_| "VM is not awaiting work")?;
                        let (identity, reply) = reply.map_err(str::to_owned)?;
                        vm.try_accept(&identity, reply, &host)
                            .map_err(|_| "reply refused")?;
                    }
                    EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(value))) => {
                        break (EffectSessionEnd::Completed, Some(value));
                    }
                    EffectSessionPoll::Terminal(end) => break (end, None),
                    _ => return Err("unexpected host/session state".into()),
                }
            };
            if host.lease.is_open() {
                return Err("terminal execution still has live authority".into());
            }
            Ok(Summary {
                end,
                value,
                total: host.total.get(),
                calls: host.settled.get(),
                remaining: vm.fuel_remaining(),
            })
        })();
        result.map_err(|message| Failure {
            message,
            total: host.total.get(),
            calls: host.settled.get(),
            remaining: Some(vm.fuel_remaining()),
        })
    })
}

type CounterVm<'ir, 's> = LocalVm<
    'ir,
    's,
    Node,
    (),
    RestoredEffectBindings<'ir, ()>,
    Identity,
    ScalarTypeSet,
    ScalarValue,
>;
struct Receipt {
    identity: Identity,
    value: ScalarValue,
}
struct Execution<'ir, 'h, 's> {
    host: Host<'h, 's>,
    vm: CounterVm<'ir, 's>,
    value: Option<u64>,
}
impl<'ir, 'h, 's> Execution<'ir, 'h, 's> {
    fn new(catalog: &'h Catalog<'s>, node: &'ir Node, budget: u64) -> Self {
        let owner = LocalExecutionScope::new();
        let host = Host::new(catalog, owner.lease(), true);
        Self {
            host,
            vm: LocalVm::new(owner, node, Vec::new(), Fuel::new(budget), STACK),
            value: None,
        }
    }
    fn poll(&mut self) -> Result<Option<Work<'s>>, Error> {
        match self.vm.poll(&self.host) {
            Ok(EffectSessionPoll::Request(work)) => Ok(Some(work)),
            Ok(EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(value)))) => {
                self.value = Some(value);
                Ok(None)
            }
            Ok(EffectSessionPoll::Awaiting | EffectSessionPoll::Terminal(_)) => Ok(None),
            Err(_) if matches!(self.vm.status(), EffectSessionStatus::Terminal(_)) => Ok(None),
            _ => Err("unexpected queued execution state"),
        }
    }
    fn dispatch(&mut self, work: Work<'s>) -> Result<Receipt, Error> {
        let host = &self.host;
        let (identity, value) = self
            .vm
            .with_host_outcome(work, |work| match host.invoke(work) {
                Ok(receipt) => HostWorkOutcome::Pending(Ok(receipt)),
                Err(error) => HostWorkOutcome::Failed(Err(error)),
            })
            .map_err(|_| "dispatch is no longer admitted")??;
        Ok(Receipt { identity, value })
    }
    fn accept(&mut self, receipt: Receipt) -> Result<(), Receipt> {
        self.vm
            .try_accept(&receipt.identity, receipt.value, &self.host)
            .map_err(|rejected| Receipt {
                identity: receipt.identity,
                value: rejected.reply,
            })
    }
    fn summary(&self) -> Result<Summary, Failure> {
        let EffectSessionStatus::Terminal(end) = self.vm.status() else {
            return Err("execution is still pending".into());
        };
        if self.host.lease.is_open() || (end == EffectSessionEnd::Completed && self.value.is_none())
        {
            return Err("terminal execution has no final handoff".into());
        }
        Ok(Summary {
            end,
            value: self.value,
            total: self.host.total.get(),
            calls: self.host.settled.get(),
            remaining: self.vm.fuel_remaining(),
        })
    }
}

enum Event<'s> {
    Dispatch(usize, Work<'s>),
    Reply(usize, Receipt),
}
#[derive(Clone, Copy)]
enum QueueControl {
    Run,
    CancelBeforeDispatch,
    CancelBeforeReply,
    RevokeBeforeReply,
}
#[derive(Debug)]
struct QueuedSummary {
    executions: [Summary; 2],
    ticks: usize,
    peak_events: usize,
    discarded_events: usize,
    dispatch_order: Vec<usize>,
}
impl std::fmt::Display for QueuedSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Queued {{ executions: {:?}, ticks: {}, peak_events: {}, discarded_events: {}, dispatch_order: {:?} }}",
            self.executions,
            self.ticks,
            self.peak_events,
            self.discarded_events,
            self.dispatch_order
        )
    }
}

fn run_queued(
    source: &str,
    budgets: [u64; 2],
    control: QueueControl,
) -> Result<QueuedSummary, Failure> {
    with_catalog(|catalog| {
        let compilation_owner = LocalExecutionScope::new();
        let compiler = Host::new(catalog, compilation_owner.lease(), true);
        let node = compile(source, &compiler)?;
        drop(compiler);
        drop(compilation_owner);
        // Share immutable compiled IR, never a live owner, capture scope or meter.
        let mut tasks = [
            Execution::new(catalog, &node, budgets[0]),
            Execution::new(catalog, &node, budgets[1]),
        ];
        let mut queue = VecDeque::with_capacity(2);
        let mut peak_events = 0;
        let mut discarded_events = 0;
        let mut dispatch_order = Vec::new();
        for tick in 0..64 {
            for index in [1, 0] {
                if tasks[index].vm.status() == EffectSessionStatus::Ready
                    && let Some(work) = tasks[index].poll()?
                {
                    if queue.len() == 2 {
                        return Err("host event queue limit exceeded".into());
                    }
                    queue.push_back(Event::Dispatch(index, work));
                }
            }
            peak_events = peak_events.max(queue.len());
            if tasks
                .iter()
                .all(|task| matches!(task.vm.status(), EffectSessionStatus::Terminal(_)))
                && queue.is_empty()
            {
                return Ok(QueuedSummary {
                    executions: [tasks[0].summary()?, tasks[1].summary()?],
                    ticks: tick,
                    peak_events,
                    discarded_events,
                    dispatch_order,
                });
            }
            let event = queue
                .pop_front()
                .ok_or("awaiting execution lost its event")?;
            match event {
                Event::Dispatch(index, work) => {
                    let task = &mut tasks[index];
                    if index == 0 && matches!(control, QueueControl::CancelBeforeDispatch) {
                        task.vm.cancel();
                    }
                    match task.dispatch(work) {
                        Ok(receipt) => {
                            dispatch_order.push(index);
                            queue.push_back(Event::Reply(index, receipt));
                        }
                        Err(_) if matches!(task.vm.status(), EffectSessionStatus::Terminal(_)) => {
                            discarded_events += 1;
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Event::Reply(index, receipt) => {
                    let task = &mut tasks[index];
                    if index == 0 {
                        match control {
                            QueueControl::CancelBeforeReply => {
                                task.vm.cancel();
                            }
                            QueueControl::RevokeBeforeReply => task.host.granted.set(false),
                            _ => {}
                        }
                    }
                    if let Err(receipt) = task.accept(receipt) {
                        discarded_events += 1;
                        // A stale reply cannot change a terminal reason. Live denial is explicit host policy.
                        if task.vm.status() == EffectSessionStatus::Awaiting {
                            let _ = task.vm.with_host_outcome(receipt, HostWorkOutcome::Failed);
                        }
                    }
                }
            }
        }
        Err("host event tick limit exceeded".into())
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mode = args.next();
    if args.next().is_some() {
        eprintln!("only one mode is accepted");
        return ExitCode::from(2);
    }
    let queued = match mode.as_deref() {
        Some("--queued") => Some(QueueControl::Run),
        Some("--queued-cancel-pending") => Some(QueueControl::CancelBeforeDispatch),
        Some("--queued-cancel-reply") => Some(QueueControl::CancelBeforeReply),
        Some("--queued-revoke") => Some(QueueControl::RevokeBeforeReply),
        _ => None,
    };
    if let Some(control) = queued {
        return match run_queued(SOURCE, [1000; 2], control) {
            Ok(summary) => {
                println!("{summary}");
                if summary.executions.iter().any(|result| {
                    matches!(
                        result.end,
                        EffectSessionEnd::Failed | EffectSessionEnd::HostUncertain
                    )
                }) {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => {
                eprintln!("queued counter refused: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let control = match mode.as_deref() {
        None => Control::Run,
        Some("--cancel") => Control::CancelAfter(1),
        Some("--deny") => Control::Deny,
        Some("--revoke") => Control::RevokeAfter(1),
        _ => {
            eprintln!(
                "usage: native_counter [--cancel|--deny|--revoke|--queued|--queued-cancel-pending|--queued-cancel-reply|--queued-revoke]"
            );
            return ExitCode::from(2);
        }
    };
    match run(SOURCE, 1000, control) {
        Ok(summary) => {
            println!("{summary:?}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("native counter refused: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_instances_share_ir_without_sharing_effects_or_fuel() {
        let immediate = run(SOURCE, 1000, Control::Run).unwrap();
        let queued = run_queued(SOURCE, [1000; 2], QueueControl::Run).unwrap();
        assert_eq!(queued.executions[0], immediate);
        assert_eq!(queued.executions[1], immediate);
        assert_eq!(queued.dispatch_order, [1, 0, 1, 0]);
        assert_eq!(
            (queued.ticks, queued.peak_events, queued.discarded_events),
            (8, 2, 0)
        );
    }
    #[test]
    fn queued_cancellation_distinguishes_pending_work_from_committed_effects() {
        for (control, total, calls, dispatches) in [
            (QueueControl::CancelBeforeDispatch, 0, 0, vec![1, 1]),
            (QueueControl::CancelBeforeReply, 19, 1, vec![1, 0, 1]),
        ] {
            let queued = run_queued(SOURCE, [1000; 2], control).unwrap();
            let cancelled = &queued.executions[0];
            assert_eq!(cancelled.end, EffectSessionEnd::Cancelled);
            assert_eq!(
                (cancelled.value, cancelled.total, cancelled.calls),
                (None, total, calls)
            );
            assert_eq!(
                queued.executions[1],
                run(SOURCE, 1000, Control::Run).unwrap()
            );
            assert_eq!(queued.discarded_events, 1);
            assert_eq!(queued.dispatch_order, dispatches);
        }
    }
    #[test]
    fn queued_revocation_refuses_reentry_without_reverting_host_state() {
        let queued = run_queued(SOURCE, [1000; 2], QueueControl::RevokeBeforeReply).unwrap();
        let revoked = &queued.executions[0];
        assert_eq!(revoked.end, EffectSessionEnd::Failed);
        assert_eq!((revoked.value, revoked.total, revoked.calls), (None, 19, 1));
        assert_eq!(
            queued.executions[1],
            run(SOURCE, 1000, Control::Run).unwrap()
        );
        assert_eq!(queued.discarded_events, 1);
        assert_eq!(queued.dispatch_order, [1, 0, 1]);
    }
    #[test]
    fn exhausted_instance_does_not_stop_or_borrow_fuel_from_its_sibling() {
        let used = 1000 - run(SOURCE, 1000, Control::Run).unwrap().remaining;
        let exact = run(SOURCE, used, Control::Run).unwrap();
        for budget in 0..used {
            let queued = run_queued(SOURCE, [budget, used], QueueControl::Run).unwrap();
            assert_eq!(queued.executions[0].end, EffectSessionEnd::Failed);
            assert!(queued.executions[0].remaining <= budget);
            assert_eq!(queued.executions[1], exact);
        }
    }
    #[test]
    fn delayed_foreign_and_wrong_typed_receipts_keep_the_original_handoff() {
        with_catalog(|catalog| {
            let owner = LocalExecutionScope::new();
            let node = compile(SOURCE, &Host::new(catalog, owner.lease(), true))?;
            let mut first = Execution::new(catalog, &node, 1000);
            let mut second = Execution::new(catalog, &node, 1000);
            let first_work = first.poll()?.unwrap();
            let second_work = second.poll()?.unwrap();
            let remaining = first.vm.fuel_remaining();
            for _ in 0..8 {
                assert!(first.poll()?.is_none());
                assert_eq!(first.vm.fuel_remaining(), remaining);
                assert_eq!(first.host.settled.get(), 0);
            }
            let first_receipt = first.dispatch(first_work)?;
            let second_receipt = second.dispatch(second_work)?;
            assert_eq!(first.vm.status(), EffectSessionStatus::Awaiting);
            assert!(first.poll()?.is_none());
            assert_eq!(first.vm.fuel_remaining(), remaining);
            let first_receipt = second.accept(first_receipt).unwrap_err();
            assert_eq!(second.vm.status(), EffectSessionStatus::Awaiting);
            assert!(matches!(first_receipt.value, ScalarValue::Integer(19)));
            let Receipt { identity, value } = first_receipt;
            let rejected = first
                .accept(Receipt {
                    identity,
                    value: ScalarValue::Boolean(true),
                })
                .unwrap_err();
            assert!(matches!(rejected.value, ScalarValue::Boolean(true)));
            assert_eq!(first.vm.status(), EffectSessionStatus::Awaiting);
            assert!(
                first
                    .accept(Receipt {
                        identity: rejected.identity,
                        value
                    })
                    .is_ok()
            );
            assert!(second.accept(second_receipt).is_ok());
            assert_eq!(first.vm.status(), EffectSessionStatus::Ready);
            assert_eq!(first.vm.fuel_remaining(), remaining);
            assert_eq!((first.host.total.get(), second.host.total.get()), (19, 19));
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn queued_work_and_late_receipts_never_reopen_cancelled_or_dropped_owners() {
        with_catalog(|catalog| {
            let owner = LocalExecutionScope::new();
            let node = compile(SOURCE, &Host::new(catalog, owner.lease(), true))?;
            let mut pending = Execution::new(catalog, &node, 1000);
            let work = pending.poll()?.unwrap();
            pending.vm.cancel();
            assert!(pending.dispatch(work).is_err());
            assert_eq!(
                (pending.host.total.get(), pending.host.settled.get()),
                (0, 0)
            );
            let mut late = Execution::new(catalog, &node, 1000);
            let work = late.poll()?.unwrap();
            let receipt = late.dispatch(work)?;
            late.vm.cancel();
            let remaining = late.vm.fuel_remaining();
            assert!(late.accept(receipt).is_err());
            assert_eq!(
                late.vm.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
            );
            assert_eq!(late.vm.fuel_remaining(), remaining);
            assert_eq!((late.host.total.get(), late.host.settled.get()), (19, 1));
            let mut dropped = Execution::new(catalog, &node, 1000);
            let work = dropped.poll()?.unwrap();
            assert!(work.identity.owner.is_open());
            drop(dropped);
            assert!(!work.identity.owner.is_open());
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn source_calls_capture_and_reenter_without_product_types() {
        let result = run(SOURCE, 1000, Control::Run).unwrap();
        assert_eq!(result.end, EffectSessionEnd::Completed);
        assert_eq!(
            (result.value, result.total, result.calls),
            (Some(42), 42, 2)
        );
        assert!(result.remaining < 1000);
    }
    #[test]
    fn cancellation_keeps_the_first_effect_and_never_dispatches_the_second() {
        let result = run(SOURCE, 1000, Control::CancelAfter(1)).unwrap();
        assert_eq!(result.end, EffectSessionEnd::Cancelled);
        assert_eq!((result.value, result.total, result.calls), (None, 19, 1));
    }
    #[test]
    fn revoked_authority_is_not_reused_after_compilation() {
        let denied = run(SOURCE, 1000, Control::Deny).unwrap_err();
        assert_eq!((denied.calls, denied.total, denied.remaining), (0, 0, None));
        for count in [0, 1] {
            let revoked = run(SOURCE, 1000, Control::RevokeAfter(count)).unwrap_err();
            assert_eq!((revoked.calls, revoked.total), (count, count * 19));
            assert!(revoked.remaining.is_some());
        }
    }
    #[test]
    fn native_failure_preserves_prior_effects_without_committing_the_failed_call() {
        let error = run(
            "fn main() = bind(first: counter.add(amount: 19), body: counter.add(amount: 100))",
            1000,
            Control::Run,
        )
        .unwrap_err();
        assert_eq!((error.calls, error.total), (1, 19));
        assert_eq!(error.message, "counter limit exceeded");
    }
    #[test]
    fn original_budget_is_not_refilled_between_host_calls() {
        let used = 1000 - run(SOURCE, 1000, Control::Run).unwrap().remaining;
        for budget in 0..used {
            assert!(run(SOURCE, budget, Control::Run).is_err());
        }
        assert_eq!(run(SOURCE, used, Control::Run).unwrap().remaining, 0);
    }
    #[test]
    fn cold_invalid_sources_are_refused_before_execution() {
        for text in [
            "fn main() = counter.unknown(amount: 1)",
            "fn main() = counter.add(amount: true)",
            "fn main() = choose(when: true, then: counter.add(amount: 1), otherwise: counter.unknown(amount: 2))",
            "fn main() = bind(answer: counter.add(amount: 1), body: unknown)",
        ] {
            let error = run(text, 1000, Control::Run).unwrap_err();
            assert_eq!((error.calls, error.total, error.remaining), (0, 0, None));
        }
        assert!(run(&" ".repeat(16 * 1024 + 1), 1000, Control::Run).is_err());
    }
}
