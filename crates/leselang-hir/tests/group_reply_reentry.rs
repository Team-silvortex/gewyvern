//! Unrelated host: parsed All, move-only native rows, authenticated group binding.
//! This composes existing bounded profiles, not a new universal group compiler.

use leselang_hir::{
    catalog_group_source::{
        CatalogGroupProgramLimits, CatalogGroupSourceEnvironment, CatalogGroupSourceLimits,
        compile_catalog_group_helper_program, compile_catalog_group_program,
        compile_catalog_group_source,
    },
    catalog_program::CatalogProgramLimits,
    catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits},
    control_source::ControlSourceLimits,
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, evaluate_resumable_effects_in_scope,
    },
    effect_reentry::{
        EffectContinuation, EffectReentryEnvironment, RestoredEffectBindings,
        ResumableEffectOutcome, resume_accepted_effects,
    },
    effect_session::{
        CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
        EffectSessionPoll, EffectSessionStatus,
    },
    effect_stack::{
        EffectReturnStack, EffectStackLimits, StackedEffectOutcome,
        evaluate_stacked_effects_in_scope, resume_accepted_stacked_effects,
    },
    effect_stack_session::StackedEffectSession,
    flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
    group_reply::{GroupReply, assemble_group_reply},
    ir::{Computation, ComputedBranch, GroupKind},
    parallel_evaluation::*,
    pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits, PureValue},
    pure_typing::{PureType, PureTypeEnvironment},
    source_call::{SourceCallHost, SourceCallLimits},
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, parse};
use std::{borrow::Borrow, cell::Cell, rc::Rc};

struct Private;
struct Domain {
    index: usize,
    maximum: u64,
}
struct Record {
    index: usize,
    count: u64,
}
struct Input {
    record: Rc<Record>,
    buffer: Box<[u8]>,
}
impl Borrow<Record> for Input {
    fn borrow(&self) -> &Record {
        &self.record
    }
}
impl HostResultDomain<Record> for Domain {
    type Error = Private;
    fn matches_type(&self, record: &Record) -> bool {
        self.index == record.index
    }
    fn validate_value(&self, record: &Record) -> Result<(), Private> {
        if self.matches_type(record) && record.count <= self.maximum {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type Node = Computation<u8, &'static str, (), usize>;
type Branch = ComputedBranch<Node, usize>;
type Rows<'a> = GroupReply<'a, Node, usize, (u64, usize), Domain, Input>;
type Schema<'a> = OperationSchema<'a, &'static str, &'static str, ScalarTypeSet, Domain, u8>;
#[derive(Clone)]
enum View<'a> {
    Group(Rc<Rows<'a>>),
    Record(Rc<Record>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Meta {
    Group(u64),
    Record(usize),
}
struct Types;
fn exported(name: &str) -> Option<(usize, &'static str)> {
    match name {
        "left" => Some((0, "counter.left")),
        "right" => Some((1, "counter.right")),
        _ => None,
    }
}
impl PureTypeEnvironment<u8, &'static str> for Types {
    type Result = Meta;
    fn field_type(&self, result: &Meta, field: &u8) -> Option<ScalarType> {
        matches!(result, Meta::Record(0 | 1)).then_some(())?;
        (*field == 0).then_some(ScalarType::Integer)
    }
    fn member_result(&self, result: &Meta, name: &str, operation: &&'static str) -> Option<Meta> {
        if *result != Meta::Group(73) {
            return None;
        }
        let (index, key) = exported(name)?;
        (*operation == key).then_some(Meta::Record(index))
    }
}
impl<'a> CallFlowEnvironment<'a, u8, &'static str, Domain> for Types {
    fn call_result_type(
        &self,
        operation: &&'static str,
        domain: &'a Domain,
    ) -> Option<PureType<Meta>> {
        let key = if domain.index == 0 {
            "counter.left"
        } else {
            "counter.right"
        };
        (*operation == key && domain.index < 2)
            .then_some(PureType::Result(Meta::Record(domain.index)))
    }
}
impl<'source, 'schema> CatalogSourceEnvironment<'source, 'schema, u8, &'static str, Domain>
    for Types
{
    type Error = Private;
    fn source_operation(&self, key: &&'static str) -> Result<&'static str, Private> {
        Ok(*key)
    }
    fn source_field(
        &self,
        result: &Meta,
        name: &'source str,
    ) -> Result<Option<(u8, ScalarType)>, Private> {
        Ok((name == "count")
            .then(|| self.field_type(result, &0).map(|ty| (0, ty)))
            .flatten())
    }
    fn source_member(
        &self,
        result: &Meta,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(&'static str, Meta)>, Private> {
        if group != "bundle" {
            return Ok(None);
        }
        Ok(exported(name)
            .and_then(|(_, key)| self.member_result(result, name, &key).map(|ty| (key, ty))))
    }
}
impl<'expression, 'schema>
    GroupFlowEnvironment<'expression, 'schema, u8, &'static str, Domain, usize> for Types
{
    fn group_branch_type_matches(&self, declared: &usize, inferred: &PureType<Meta>) -> bool {
        matches!(inferred, PureType::Result(Meta::Record(index)) if declared == index)
    }
    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'expression, &'static str, Meta>],
    ) -> Option<Meta> {
        if kind != GroupKind::Parallel || members.len() != 2 {
            return None;
        }
        for (index, member) in members.iter().enumerate() {
            let (selected, key) = exported(member.name)?;
            if selected != index
                || *member.operation != key
                || !matches!(member.inferred, PureType::Result(Meta::Record(i)) if i == index)
            {
                return None;
            }
        }
        Some(Meta::Group(73))
    }
}
impl<'source, 'schema>
    CatalogGroupSourceEnvironment<'source, 'schema, u8, &'static str, Domain, usize> for Types
{
    fn source_group_branch(
        &self,
        _: &'source leselang_syntax::NamedArgument,
        operation: &&'static str,
        declaration: &'schema Domain,
        inferred: &PureType<Meta>,
    ) -> Result<usize, Private> {
        if self.call_result_type(operation, declaration).as_ref() != Some(inferred) {
            return Err(Private);
        }
        Ok(declaration.index)
    }
}
const SOURCE: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 64,
};
const COMPILE: CatalogSourceLimits = CatalogSourceLimits {
    control: ControlSourceLimits {
        source: SOURCE,
        max_bindings: 8,
    },
    max_calls: 16,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
const EFFECT: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PURE,
    max_arguments: 64,
    max_branches: 64,
};
const BATCH: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
fn expression(text: &str) -> Expression {
    let tree = parse(&format!("fn main() = {text}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn schemas() -> [Schema<'static>; 2] {
    [
        OperationSchema {
            key: "counter.left",
            parameters: &[],
            required_capability: 31,
            result: Domain {
                index: 0,
                maximum: 100,
            },
        },
        OperationSchema {
            key: "counter.right",
            parameters: &[],
            required_capability: 32,
            result: Domain {
                index: 1,
                maximum: 100,
            },
        },
    ]
}
fn program(
    catalog: &OperationCatalog<'_, &'static str, &'static str, ScalarTypeSet, Domain, u8>,
) -> Node {
    let source_host = SourceCallHost {
        catalog,
        version: 7,
        granted: &[31, 32],
    };
    let source = expression(
        "bind(bundle: all(left: counter.left(), right: counter.right()), body: bind(left_record: member(value: bundle, name: \"left\"), body: bind(right_record: member(value: bundle, name: \"right\"), body: choose(when: gt(left: field(value: left_record, name: \"count\"), right: 40), then: add(left: field(value: left_record, name: \"count\"), right: field(value: right_record, name: \"count\")), otherwise: 0))))",
    );
    let (program, ty) = compile_catalog_group_source(
        &source,
        &[],
        CatalogGroupSourceLimits {
            expression: COMPILE,
            max_groups: 4,
            max_branches: 64,
        },
        &source_host,
        &Types,
    )
    .unwrap();
    assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
    program
}
struct Host<'a> {
    group: &'a Node,
    schemas: &'a [Schema<'a>],
    live: Cell<bool>,
    calls: Cell<usize>,
    counters: [Cell<u64>; 2],
    reads: Cell<usize>,
    restores: Cell<usize>,
    projections: Cell<usize>,
}
impl<'a> Host<'a> {
    fn new(program: &'a Node, schemas: &'a [Schema<'a>]) -> Self {
        let Node::Bind { value, .. } = program else {
            panic!()
        };
        Self {
            group: value,
            schemas,
            live: Cell::new(true),
            calls: Cell::new(0),
            counters: [Cell::new(40), Cell::new(6)],
            reads: Cell::new(0),
            restores: Cell::new(0),
            projections: Cell::new(0),
        }
    }
    fn row(&self, index: usize) -> Result<&'a Branch, Private> {
        let Node::Group { branches, .. } = self.group else {
            return Err(Private);
        };
        branches.get(index).ok_or(Private)
    }
    fn validate_row(&self, index: usize, row: &Branch) -> Result<(), Private> {
        if !self.live.get() || index >= self.schemas.len() || row.result_type != index {
            return Err(Private);
        }
        let Node::Call {
            operation,
            arguments,
        } = &row.value
        else {
            return Err(Private);
        };
        if *operation != self.schemas[index].key || !arguments.is_empty() {
            return Err(Private);
        }
        Ok(())
    }
    fn validate_group(&self, reply: &Rows<'a>) -> Result<(), Private> {
        if !self.live.get()
            || !std::ptr::eq(reply.group(), self.group)
            || reply.kind() != GroupKind::Parallel
            || reply.members().len() != self.schemas.len()
        {
            return Err(Private);
        }
        for (index, row) in reply.members().iter().enumerate() {
            let (position, original, key, declaration, input) = row.as_parts();
            self.validate_row(index, original)?;
            if position != index
                || *key != (73, index)
                || !std::ptr::eq(original, self.row(index)?)
                || !std::ptr::eq(declaration, &self.schemas[index].result)
                || input.buffer.len() != 13
            {
                return Err(Private);
            }
            declaration.validate_value(&input.record)?;
        }
        Ok(())
    }
    fn invoke(&self, index: usize) -> Input {
        assert!(self.live.get());
        self.calls.set(self.calls.get() + 1);
        let counter = &self.counters[index];
        let count = counter.get().checked_add(1).unwrap();
        counter.set(count);
        Input {
            record: Rc::new(Record { index, count }),
            buffer: vec![index as u8; 13].into_boxed_slice(),
        }
    }
}
impl ReplyAuthority<(u64, usize)> for Host<'_> {
    type Error = Private;
    fn authorize(&self, &(owner, index): &(u64, usize)) -> Result<(), Private> {
        if owner != 73 {
            return Err(Private);
        }
        self.validate_row(index, self.row(index)?)
    }
}
impl ReplyAuthority<u64> for Host<'_> {
    type Error = Private;
    fn authorize(&self, owner: &u64) -> Result<(), Private> {
        if *owner == 73 && self.live.get() {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
impl<'a> HostResultDomain<Rows<'a>> for Host<'a> {
    type Error = Private;
    fn matches_type(&self, reply: &Rows<'a>) -> bool {
        std::ptr::eq(reply.group(), self.group) && reply.kind() == GroupKind::Parallel
    }
    fn validate_value(&self, reply: &Rows<'a>) -> Result<(), Private> {
        self.validate_group(reply)
    }
}
impl<'a> ParallelEvaluationEnvironment<'a, u8, &'static str, (), usize> for Host<'a> {
    type Identity = (u64, usize);
    type Declaration = Domain;
    type Request = usize;
    type Error = Private;
    fn preflight_member(&self, index: usize, row: &'a Branch) -> Result<(), Private> {
        self.validate_row(index, row)
    }
    fn prepare_member(
        &self,
        index: usize,
        row: &'a Branch,
        _: &mut Fuel,
    ) -> ParallelPreparation<'a, (u64, usize), Domain, usize, Private> {
        self.validate_row(index, row)?;
        Ok(PreparedParallelMember {
            identity: (73, index),
            declaration: &self.schemas[index].result,
            request: index,
        })
    }
}
impl<'a> PureEvaluationEnvironment<u8, &'static str> for Host<'a> {
    type Result = View<'a>;
    type Error = Private;
    fn field(&self, result: &View<'a>, field: &u8) -> Result<ScalarValue, Private> {
        let View::Record(record) = result else {
            return Err(Private);
        };
        if !self.live.get() || *field != 0 {
            return Err(Private);
        }
        self.schemas
            .get(record.index)
            .ok_or(Private)?
            .result
            .validate_value(record)?;
        self.reads.set(self.reads.get() + 1);
        Ok(ScalarValue::Integer(record.count))
    }
    fn member(
        &self,
        group: &View<'a>,
        name: &str,
        operation: &&'static str,
    ) -> Result<View<'a>, Private> {
        let View::Group(reply) = group else {
            return Err(Private);
        };
        self.validate_group(reply)?;
        let row = reply
            .lookup(64, name, |row| {
                let (index, original, key, declaration, input) = row.as_parts();
                self.authorize(key)?;
                let Node::Call {
                    operation: original_key,
                    ..
                } = &original.value
                else {
                    return Err(Private);
                };
                if operation != original_key
                    || !std::ptr::eq(declaration, &self.schemas[index].result)
                {
                    return Err(Private);
                }
                declaration.validate_value(&input.record)?;
                Ok(true)
            })
            .map_err(|_| Private)?
            .ok_or(Private)?;
        Ok(View::Record(row.as_parts().4.record.clone()))
    }
}
impl<'a> EffectEvaluationEnvironment<'a, u8, &'static str, (), usize> for Host<'a> {
    type Request = &'a Node;
    type Capture = ();
    fn preflight_effect(&self, node: &'a Node) -> Result<(), Private> {
        if std::ptr::eq(node, self.group) {
            for index in 0..self.schemas.len() {
                self.validate_row(index, self.row(index)?)?;
            }
            return Ok(());
        }
        for index in 0..self.schemas.len() {
            let row = self.row(index)?;
            if std::ptr::eq(node, &row.value) {
                return self.validate_row(index, row);
            }
        }
        Err(Private)
    }
    fn prepare_effect(
        &self,
        node: &'a Node,
        _: &mut ScopeFrame<'_, 'a, PureValue<View<'a>>>,
        _: &mut Fuel,
    ) -> Result<&'a Node, CalculationFailure<Private>> {
        if !std::ptr::eq(node, self.group) {
            return Err(Private.into());
        }
        Ok(node)
    }
    fn capture(
        &self,
        _: &'a str,
        _: &'a Node,
        scope: &ScopeFrame<'_, 'a, PureValue<View<'a>>>,
        _: &mut Fuel,
    ) -> Result<(), Private> {
        if scope.bindings().is_empty() {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
impl<'a> EffectReentryEnvironment<'a, u8, &'static str, (), usize, u64, Host<'a>, Rc<Rows<'a>>>
    for Host<'a>
{
    fn restore_capture(
        &self,
        identity: &u64,
        declaration: &Host<'a>,
        _: (),
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<RestoredEffectBindings<'a, View<'a>>, CalculationFailure<Private>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self, declaration) {
            return Err(Private.into());
        }
        self.restores.set(self.restores.get() + 1);
        Ok(Vec::new())
    }
    fn bind_reply(
        &self,
        identity: &u64,
        declaration: &Host<'a>,
        reply: Rc<Rows<'a>>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<View<'a>>, CalculationFailure<Private>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self, declaration) {
            return Err(Private.into());
        }
        self.validate_group(&reply)?;
        fuel.charge(1).map_err(|_| Private)?;
        self.projections.set(self.projections.get() + 1);
        Ok(PureValue::Result(View::Group(reply)))
    }
}
fn run_batch<'a>(
    program: &'a Node,
    host: &Host<'a>,
) -> (EffectContinuation<'a, Node, ()>, Rc<Rows<'a>>, Fuel) {
    let mut fuel = Fuel::new(100);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let ResumableEffectOutcome::Suspended {
        request,
        continuation,
    } = evaluate_resumable_effects_in_scope(program, &mut scope, host, &mut fuel, EFFECT).unwrap()
    else {
        panic!()
    };
    assert!(std::ptr::eq(request, host.group));
    assert_eq!(host.calls.get(), 0);
    assert_eq!(fuel.remaining(), 98);
    let (reply, fuel) = complete_batch(request, host, fuel);
    assert_eq!(fuel.remaining(), 92);
    (continuation, reply, fuel)
}
fn complete_batch<'a>(request: &'a Node, host: &Host<'a>, fuel: Fuel) -> (Rc<Rows<'a>>, Fuel) {
    let initial_fuel = fuel.remaining();
    // Batch lifecycle has its own explicit root charge; the language root is not skipped/refunded.
    let mut session = ParallelEvaluation::start(request, host, fuel, BATCH).unwrap();
    let ParallelPoll::Requests(batch) = session.poll(host).unwrap() else {
        panic!()
    };
    for item in batch.into_iter().rev() {
        session
            .try_accept::<Record, _>(
                item.index,
                &(73, item.index),
                host.invoke(item.request),
                host,
            )
            .unwrap();
    }
    let ParallelPoll::Results(rows) = session.poll(host).unwrap() else {
        panic!()
    };
    let mut fuel = session.into_completed_fuel().unwrap();
    assert_eq!(fuel.remaining(), initial_fuel - 3);
    let reply = assemble_group_reply(request, rows, BATCH, &mut fuel, |reply, _| {
        host.validate_group(reply)
    })
    .unwrap();
    assert_eq!(fuel.remaining(), initial_fuel - 6);
    (Rc::new(reply), fuel)
}
fn with_program(test: impl FnOnce(&Node, &[Schema<'_>])) {
    let schemas = schemas();
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let program = program(&catalog);
    test(&program, &schemas);
}

#[test]
fn parsed_parallel_results_bind_project_and_reenter_original_enclosing_control_with_one_meter() {
    with_program(|program, schemas| {
        let host = Host::new(program, schemas);
        let (continuation, reply, mut fuel) = run_batch(program, &host);
        let Node::Bind { body, .. } = program else {
            panic!()
        };
        assert_eq!(continuation.name(), "bundle");
        assert!(std::ptr::eq(continuation.body(), body.as_ref()));
        let pointer = Rc::as_ptr(&reply);
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let rejected = pending
            .try_accept::<_, Rows<'_>, _>(&74, reply, &host)
            .unwrap_err();
        assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
        assert_eq!(host.restores.get(), 0);
        let accepted = pending
            .try_accept::<_, Rows<'_>, _>(&73, rejected.reply, &host)
            .unwrap();
        let ResumableEffectOutcome::Value(PureValue::Scalar(value)) =
            resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).unwrap()
        else {
            panic!()
        };
        assert_eq!(value, ScalarValue::Integer(48));
        assert_eq!(host.calls.get(), 2);
        assert_eq!((host.counters[0].get(), host.counters[1].get()), (41, 7));
        assert_eq!(host.reads.get(), 3);
        assert_eq!(host.restores.get(), 1);
        assert_eq!(host.projections.get(), 1);
        assert_eq!(fuel.remaining(), 77);
    });
}

#[test]
fn aggregate_data_does_not_bypass_fresh_outer_acceptance_or_reentry_authority() {
    for after_accept in [false, true] {
        with_program(|program, schemas| {
            let host = Host::new(program, schemas);
            let (continuation, reply, mut fuel) = run_batch(program, &host);
            let mut pending = PendingReply::new(73u64, continuation, &host);
            if after_accept {
                let accepted = pending
                    .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                    .unwrap();
                host.live.set(false);
                assert!(resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).is_err());
            } else {
                host.live.set(false);
                let rejected = pending
                    .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                    .unwrap_err();
                assert!(matches!(rejected.error, ReplyAcceptanceError::Authority(_)));
                assert!(pending.cancel());
            }
            assert_eq!(host.calls.get(), 2);
            assert_eq!(host.restores.get(), 0);
            assert_eq!(host.projections.get(), 0);
            assert_eq!(host.reads.get(), 0);
            assert_eq!(fuel.remaining(), 92);
        });
    }
}

#[test]
fn cancellation_before_complete_group_handoff_never_projects_or_restores_outer_control() {
    with_program(|program, schemas| {
        let host = Host::new(program, schemas);
        let mut fuel = Fuel::new(100);
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let ResumableEffectOutcome::Suspended {
            request,
            continuation,
        } = evaluate_resumable_effects_in_scope(program, &mut scope, &host, &mut fuel, EFFECT)
            .unwrap()
        else {
            panic!()
        };
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let mut session = ParallelEvaluation::start(request, &host, fuel, BATCH).unwrap();
        let ParallelPoll::Requests(mut batch) = session.poll(&host).unwrap() else {
            panic!()
        };
        let first = batch.remove(0);
        session
            .try_accept::<Record, _>(
                first.index,
                &(73, first.index),
                host.invoke(first.request),
                &host,
            )
            .unwrap();
        assert!(session.cancel());
        assert!(pending.cancel());
        let late = Input {
            record: Rc::new(Record { index: 1, count: 7 }),
            buffer: vec![1; 13].into_boxed_slice(),
        };
        let pointer = late.buffer.as_ptr();
        let rejected = session
            .try_accept::<Record, _>(1, &(73, 1), late, &host)
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
        let session = session.into_completed_fuel().unwrap_err();
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(ParallelEnd::Cancelled)
        );
        assert_eq!(session.fuel_remaining(), 95);
        assert_eq!(host.calls.get(), 1);
        assert_eq!(host.restores.get(), 0);
        assert_eq!(host.projections.get(), 0);
        assert_eq!(host.reads.get(), 0);
    });
}

#[test]
fn selected_member_never_falls_back_to_another_operation_or_stale_native_view() {
    with_program(|program, schemas| {
        let host = Host::new(program, schemas);
        let (_, reply, _) = run_batch(program, &host);
        let view = View::Group(reply);
        assert!(host.member(&view, "left", &"counter.right").is_err());
        assert!(host.member(&view, "missing", &"counter.left").is_err());
        let record = host
            .member(&view, "left", &"counter.left")
            .unwrap_or_else(|_| panic!());
        host.live.set(false);
        assert!(host.field(&record, &0).is_err());
        assert!(host.member(&view, "left", &"counter.left").is_err());
        assert_eq!(host.calls.get(), 2);
        assert_eq!(host.reads.get(), 0);
    });
}

fn with_helper_program(test: impl FnOnce(&Node, &[Schema<'_>])) {
    with_helper_profile(HelperProfile::Entry, test);
}

fn with_group_return_program(test: impl FnOnce(&Node, &[Schema<'_>])) {
    with_helper_profile(HelperProfile::GroupReturn, test);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HelperProfile {
    Entry,
    GroupReturn,
    NestedReturn,
    NestedGroupReturn,
}
fn with_helper_profile(profile: HelperProfile, test: impl FnOnce(&Node, &[Schema<'_>])) {
    let schemas = schemas();
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let batch = if matches!(
        profile,
        HelperProfile::GroupReturn | HelperProfile::NestedGroupReturn
    ) {
        "collect()"
    } else {
        "all(left: counter.left(), right: counter.right())"
    };
    let body = format!(
        "bind(bundle: {batch}, body: bind(left_record: member(value: bundle, name: \"left\"), body: bind(right_record: member(value: bundle, name: \"right\"), body: choose(when: gt(left: field(value: left_record, name: \"count\"), right: 40), then: sum(right: field(value: right_record, name: \"count\"), left: field(value: left_record, name: \"count\")), otherwise: 0))))"
    );
    let text = if profile == HelperProfile::NestedReturn {
        format!(
            "fn start() = bind(answer: collect(), body: add(left: answer, right: 1))\nfn collect() = {body}\nfn sum(left: integer, right: integer) = add(left: left, right: right)"
        )
    } else {
        format!(
            "fn start() = {body}\nfn sum(left: integer, right: integer) = add(left: left, right: right){}",
            if profile == HelperProfile::GroupReturn {
                "\nfn collect() = all(left: counter.left(), right: counter.right())"
            } else if profile == HelperProfile::NestedGroupReturn {
                "\nfn collect() = bind(inner: all(left: counter.left(), right: counter.right()), body: inner)"
            } else {
                ""
            },
        )
    };
    let source = parse(&text);
    assert!(source.diagnostics.is_empty());
    let declarations: Vec<_> = source.function.iter().chain(&source.helpers).collect();
    let compiler = if profile != HelperProfile::Entry {
        compile_catalog_group_helper_program
    } else {
        compile_catalog_group_program
    };
    let (program, ty): (Node, _) = compiler(
        &declarations,
        "start",
        CatalogGroupProgramLimits {
            program: CatalogProgramLimits {
                expression: COMPILE,
                max_functions: 32,
                max_parameters: 8,
                max_compilation_nodes: 4096,
                max_expansion_depth: 32,
            },
            max_groups: 4,
            max_branches: 64,
        },
        &SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        },
        &Types,
    )
    .unwrap();
    assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
    test(&program, &schemas);
}

fn stacked_batch<'a>(
    program: &'a Node,
    host: &Host<'a>,
) -> (EffectReturnStack<'a, Node, ()>, Rc<Rows<'a>>, Fuel) {
    let mut fuel = Fuel::new(100);
    let mut bindings = Vec::new();
    let StackedEffectOutcome::Suspended {
        request,
        continuation,
    } = evaluate_stacked_effects_in_scope(
        program,
        &mut ScopeFrame::new(&mut bindings),
        host,
        &mut fuel,
        EffectStackLimits {
            effects: EFFECT,
            max_frames: 4,
        },
    )
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(continuation.frame_count(), 2);
    assert!(std::ptr::eq(request, host.group));
    assert_eq!(fuel.remaining(), 97);
    let (reply, fuel) = complete_batch(request, host, fuel);
    assert_eq!(fuel.remaining(), 91);
    (continuation, reply, fuel)
}

#[test]
fn parsed_helper_internal_group_reply_unwinds_to_original_outer_return_without_new_vm_or_dispatch()
{
    with_helper_profile(HelperProfile::NestedReturn, |program, schemas| {
        let Node::Bind { value: helper, .. } = program else {
            panic!()
        };
        let host = Host::new(helper, schemas);
        let (continuation, reply, mut fuel) = stacked_batch(program, &host);
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let accepted = pending
            .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
            .unwrap();
        let StackedEffectOutcome::Value(PureValue::Scalar(value)) =
            resume_accepted_stacked_effects(
                accepted,
                &host,
                &mut fuel,
                EffectStackLimits {
                    effects: EFFECT,
                    max_frames: 4,
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(value, ScalarValue::Integer(49));
        assert_eq!(host.calls.get(), 2);
        assert_eq!(host.projections.get(), 1);
        assert_eq!(host.restores.get(), 2);
        assert_eq!(host.reads.get(), 3);
        assert_eq!(fuel.remaining(), 69);
    });
}

#[test]
fn nested_group_helper_return_frames_stop_on_cancel_or_fresh_reentry_revocation() {
    for phase in 0..2 {
        with_helper_profile(HelperProfile::NestedReturn, |program, schemas| {
            let Node::Bind { value: helper, .. } = program else {
                panic!()
            };
            let host = Host::new(helper, schemas);
            let (continuation, reply, mut fuel) = stacked_batch(program, &host);
            let mut pending = PendingReply::new(73u64, continuation, &host);
            if phase == 0 {
                assert!(pending.cancel());
            } else {
                let accepted = pending
                    .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                    .unwrap();
                host.live.set(false);
                assert!(
                    resume_accepted_stacked_effects(
                        accepted,
                        &host,
                        &mut fuel,
                        EffectStackLimits {
                            effects: EFFECT,
                            max_frames: 4
                        }
                    )
                    .is_err()
                );
            }
            assert_eq!(host.calls.get(), 2);
            assert_eq!(host.projections.get(), 0);
            assert_eq!(host.restores.get(), 0);
            assert_eq!(host.reads.get(), 0);
            assert_eq!(fuel.remaining(), 91);
        });
    }
}

#[test]
fn parsed_native_group_value_returns_through_outer_frame_without_a_second_reply_projection() {
    with_helper_profile(HelperProfile::NestedGroupReturn, |program, schemas| {
        let Node::Bind { value: helper, .. } = program else {
            panic!()
        };
        let host = Host::new(helper, schemas);
        let (continuation, reply, mut fuel) = stacked_batch(program, &host);
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let accepted = pending
            .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
            .unwrap();
        let StackedEffectOutcome::Value(PureValue::Scalar(ScalarValue::Integer(48))) =
            resume_accepted_stacked_effects(
                accepted,
                &host,
                &mut fuel,
                EffectStackLimits {
                    effects: EFFECT,
                    max_frames: 4,
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(host.calls.get(), 2);
        assert_eq!(host.projections.get(), 1);
        assert_eq!(host.restores.get(), 2);
        assert_eq!(host.reads.get(), 3);
        assert_eq!(fuel.remaining(), 71);
    });
}

struct GroupDispatch<'a> {
    request: &'a Node,
    reserved: Fuel,
}
struct SessionHost<'host, 'a> {
    native: &'host Host<'a>,
}
impl<'a> PureEvaluationEnvironment<u8, &'static str> for SessionHost<'_, 'a> {
    type Result = View<'a>;
    type Error = Private;
    fn field(&self, value: &View<'a>, field: &u8) -> Result<ScalarValue, Private> {
        self.native.field(value, field)
    }
    fn member(&self, value: &View<'a>, name: &str, op: &&'static str) -> Result<View<'a>, Private> {
        self.native.member(value, name, op)
    }
}
impl<'a> EffectEvaluationEnvironment<'a, u8, &'static str, (), usize> for SessionHost<'_, 'a> {
    type Request = &'a Node;
    type Capture = ();
    fn preflight_effect(&self, node: &'a Node) -> Result<(), Private> {
        self.native.preflight_effect(node)
    }
    fn prepare_effect(
        &self,
        node: &'a Node,
        scope: &mut ScopeFrame<'_, 'a, PureValue<View<'a>>>,
        fuel: &mut Fuel,
    ) -> Result<&'a Node, CalculationFailure<Private>> {
        self.native.prepare_effect(node, scope, fuel)
    }
    fn capture(
        &self,
        name: &'a str,
        body: &'a Node,
        scope: &ScopeFrame<'_, 'a, PureValue<View<'a>>>,
        fuel: &mut Fuel,
    ) -> Result<(), Private> {
        self.native.capture(name, body, scope, fuel)
    }
}
impl<'a> EffectReentryEnvironment<'a, u8, &'static str, (), usize, u64, Host<'a>, Rc<Rows<'a>>>
    for SessionHost<'_, 'a>
{
    fn restore_capture(
        &self,
        identity: &u64,
        declaration: &Host<'a>,
        capture: (),
        fuel: &mut Fuel,
        limits: EffectEvaluationLimits,
    ) -> Result<RestoredEffectBindings<'a, View<'a>>, CalculationFailure<Private>> {
        self.native
            .restore_capture(identity, declaration, capture, fuel, limits)
    }
    fn bind_reply(
        &self,
        identity: &u64,
        declaration: &Host<'a>,
        reply: Rc<Rows<'a>>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<View<'a>>, CalculationFailure<Private>> {
        self.native.bind_reply(identity, declaration, reply, fuel)
    }
}
impl<'host, 'a> EffectSessionEnvironment<'a, 'host, u8, &'static str, (), usize>
    for SessionHost<'host, 'a>
{
    type Identity = u64;
    type Declaration = Host<'a>;
    type Reply = Rc<Rows<'a>>;
    type Dispatch = GroupDispatch<'a>;
    fn correlate_request(
        &self,
        request: &'a Node,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'host, u64, Host<'a>, GroupDispatch<'a>, Private> {
        self.native.preflight_effect(request)?;
        if !std::ptr::eq(request, self.native.group) {
            return Err(Private.into());
        }
        // This finite fixture prepays the exact native batch/assembly work once.
        // The submeter spends only reserved units; it cannot refill the root session.
        fuel.charge(6).map_err(|_| Private)?;
        Ok(CorrelatedEffectRequest {
            identity: 73,
            declaration: self.native,
            dispatch: GroupDispatch {
                request,
                reserved: Fuel::new(6),
            },
        })
    }
}

#[test]
fn parsed_group_helpers_use_owned_fuel_stacked_sessions_with_explicit_native_work_reservation() {
    for profile in [
        HelperProfile::NestedReturn,
        HelperProfile::NestedGroupReturn,
    ] {
        with_helper_profile(profile, |program, schemas| {
            let Node::Bind { value: helper, .. } = program else {
                panic!()
            };
            let native = Host::new(helper, schemas);
            let host = SessionHost { native: &native };
            let mut session = StackedEffectSession::new(
                program,
                Vec::new(),
                Fuel::new(100),
                EffectStackLimits {
                    effects: EFFECT,
                    max_frames: 4,
                },
            );
            let EffectSessionPoll::Request(dispatch) = session.poll(&host).unwrap() else {
                panic!()
            };
            assert!(std::ptr::eq(dispatch.request, native.group));
            assert_eq!(session.fuel_remaining(), 91);
            assert_eq!(native.calls.get(), 0);
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            let (reply, reserved) = complete_batch(dispatch.request, &native, dispatch.reserved);
            assert_eq!(reserved.remaining(), 0);
            let pointer = Rc::as_ptr(&reply);
            let rejected = session
                .try_accept::<Rows<'_>, _>(&74, reply, &native)
                .unwrap_err();
            assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
            session
                .try_accept::<Rows<'_>, _>(&73, rejected.reply, &native)
                .unwrap();
            assert_eq!(session.status(), EffectSessionStatus::Ready);
            assert_eq!(session.fuel_remaining(), 91);
            assert_eq!(native.restores.get(), 0);
            assert_eq!(native.projections.get(), 0);
            let EffectSessionPoll::Value(PureValue::Scalar(value)) = session.poll(&host).unwrap()
            else {
                panic!()
            };
            let (expected, fuel) = if profile == HelperProfile::NestedReturn {
                (49, 69)
            } else {
                (48, 71)
            };
            assert_eq!(value, ScalarValue::Integer(expected));
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(native.calls.get(), 2);
            assert_eq!(native.restores.get(), 2);
            assert_eq!(native.projections.get(), 1);
            assert_eq!(native.reads.get(), 3);
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
            ));
            assert!(!session.cancel());
        });
    }
}

#[test]
fn stacked_group_session_cancel_or_revocation_after_acceptance_never_projects_or_restores() {
    for cancel in [false, true] {
        with_helper_profile(HelperProfile::NestedReturn, |program, schemas| {
            let Node::Bind { value: helper, .. } = program else {
                panic!()
            };
            let native = Host::new(helper, schemas);
            let host = SessionHost { native: &native };
            let mut session = StackedEffectSession::new(
                program,
                Vec::new(),
                Fuel::new(100),
                EffectStackLimits {
                    effects: EFFECT,
                    max_frames: 4,
                },
            );
            let EffectSessionPoll::Request(dispatch) = session.poll(&host).unwrap() else {
                panic!()
            };
            let (reply, _) = complete_batch(dispatch.request, &native, dispatch.reserved);
            session
                .try_accept::<Rows<'_>, _>(&73, reply, &native)
                .unwrap();
            if cancel {
                assert!(session.cancel());
            } else {
                native.live.set(false);
                assert!(session.poll(&host).is_err());
            }
            let reason = if cancel {
                EffectSessionEnd::Cancelled
            } else {
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(reason));
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
            assert_eq!(session.fuel_remaining(), 91);
            assert_eq!(native.restores.get(), 0);
            assert_eq!(native.projections.get(), 0);
            assert_eq!(native.reads.get(), 0);
            assert_eq!(native.calls.get(), 2);
        });
    }
}

#[test]
fn stacked_group_session_exhausted_native_reservation_cannot_publish_partial_work() {
    with_helper_profile(HelperProfile::NestedReturn, |program, schemas| {
        let Node::Bind { value: helper, .. } = program else {
            panic!()
        };
        let native = Host::new(helper, schemas);
        let host = SessionHost { native: &native };
        let mut session = StackedEffectSession::new(
            program,
            Vec::new(),
            Fuel::new(8),
            EffectStackLimits {
                effects: EFFECT,
                max_frames: 4,
            },
        );
        assert!(session.poll(&host).is_err());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
        );
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
        ));
        assert_eq!(native.calls.get(), 0);
        assert_eq!(native.restores.get(), 0);
        assert_eq!(native.projections.get(), 0);
        assert!(!session.cancel());
    });
}

#[test]
fn parsed_group_return_helper_reenters_original_control_and_hygienic_tail_with_one_meter() {
    with_group_return_program(|program, schemas| {
        let host = Host::new(program, schemas);
        let (continuation, reply, mut fuel) = run_batch(program, &host);
        let pointer = Rc::as_ptr(&reply);
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let rejected = pending
            .try_accept::<_, Rows<'_>, _>(&74, reply, &host)
            .unwrap_err();
        assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
        let accepted = pending
            .try_accept::<_, Rows<'_>, _>(&73, rejected.reply, &host)
            .unwrap();
        let ResumableEffectOutcome::Value(PureValue::Scalar(value)) =
            resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).unwrap()
        else {
            panic!()
        };
        assert_eq!(value, ScalarValue::Integer(48));
        assert_eq!(host.calls.get(), 2);
        assert_eq!(host.restores.get(), 1);
        assert_eq!(host.projections.get(), 1);
        assert_eq!(host.reads.get(), 3);
        assert_eq!(fuel.remaining(), 73);
    });
}

#[test]
fn group_return_helper_does_not_bypass_outer_reply_revocation_or_cancellation() {
    for phase in 0..3 {
        with_group_return_program(|program, schemas| {
            let host = Host::new(program, schemas);
            let (continuation, reply, mut fuel) = run_batch(program, &host);
            let mut pending = PendingReply::new(73u64, continuation, &host);
            match phase {
                0 => assert!(pending.cancel()),
                1 => {
                    host.live.set(false);
                    let rejected = pending
                        .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                        .unwrap_err();
                    assert!(matches!(rejected.error, ReplyAcceptanceError::Authority(_)));
                    assert!(pending.cancel());
                }
                _ => {
                    let accepted = pending
                        .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                        .unwrap();
                    host.live.set(false);
                    assert!(resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).is_err());
                }
            }
            assert_eq!(host.calls.get(), 2);
            assert_eq!(host.restores.get(), 0);
            assert_eq!(host.projections.get(), 0);
            assert_eq!(host.reads.get(), 0);
            assert_eq!(fuel.remaining(), 92);
        });
    }
}

#[test]
fn parsed_group_entry_program_reenters_hygienic_helper_tail_with_original_reply_and_fuel() {
    with_helper_program(|program, schemas| {
        let host = Host::new(program, schemas);
        let (continuation, reply, mut fuel) = run_batch(program, &host);
        let pointer = Rc::as_ptr(&reply);
        let mut pending = PendingReply::new(73u64, continuation, &host);
        let rejected = pending
            .try_accept::<_, Rows<'_>, _>(&74, reply, &host)
            .unwrap_err();
        assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
        let accepted = pending
            .try_accept::<_, Rows<'_>, _>(&73, rejected.reply, &host)
            .unwrap();
        let ResumableEffectOutcome::Value(PureValue::Scalar(value)) =
            resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).unwrap()
        else {
            panic!()
        };
        assert_eq!(value, ScalarValue::Integer(48));
        assert_eq!(host.calls.get(), 2);
        assert_eq!(host.restores.get(), 1);
        assert_eq!(host.projections.get(), 1);
        assert_eq!(host.reads.get(), 3);
        assert_eq!(fuel.remaining(), 73);
    });
}

#[test]
fn helper_tail_never_runs_after_revocation_or_outer_cancellation() {
    for phase in 0..3 {
        with_helper_program(|program, schemas| {
            let host = Host::new(program, schemas);
            let (continuation, reply, mut fuel) = run_batch(program, &host);
            let mut pending = PendingReply::new(73u64, continuation, &host);
            match phase {
                0 => assert!(pending.cancel()),
                1 => {
                    host.live.set(false);
                    let rejected = pending
                        .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                        .unwrap_err();
                    assert!(matches!(rejected.error, ReplyAcceptanceError::Authority(_)));
                    assert!(pending.cancel());
                }
                _ => {
                    let accepted = pending
                        .try_accept::<_, Rows<'_>, _>(&73, reply, &host)
                        .unwrap();
                    host.live.set(false);
                    assert!(resume_accepted_effects(accepted, &host, &mut fuel, EFFECT).is_err());
                }
            }
            assert_eq!(host.calls.get(), 2);
            assert_eq!(host.restores.get(), 0);
            assert_eq!(host.projections.get(), 0);
            assert_eq!(host.reads.get(), 0);
            assert_eq!(fuel.remaining(), 92);
        });
    }
}
