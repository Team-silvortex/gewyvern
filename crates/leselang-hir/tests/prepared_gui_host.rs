//! Headless GUI protocol model, not an Avalonia/native-window adapter.
use leselang_hir::{
    call_evaluation::{CallEvaluationHost, CallEvaluationLimits, PreparedCall},
    catalog_group_source::{
        CatalogGroupProgramLimits, CatalogGroupSourceEnvironment,
        compile_catalog_prepared_group_helper_program,
    },
    catalog_program::CatalogProgramLimits,
    catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits},
    control_source::ControlSourceLimits,
    effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits},
    effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings},
    effect_session::{
        CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
        EffectSessionPoll, EffectSessionStatus,
    },
    effect_stack::EffectStackLimits,
    effect_stack_session::StackedEffectSession,
    flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::PreparedNativeGroup,
    parallel_evaluation::{
        ParallelAccepted, ParallelEvaluationLimits, ParallelPoll, ParallelRequest,
        PreparedParallelMember,
    },
    prepared_call_evaluation::{PreparedGroupEvaluationLimits, prepare_atomic_group_in_scope},
    prepared_parallel::PreparedParallelSession,
    prepared_sequence::{PreparedSequenceError, PreparedSequenceSession},
    pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, scalar_copy_cost,
    },
    pure_typing::{PureType, PureTypeEnvironment},
    sequence_evaluation::{SequenceEnd, SequencePoll, SequenceStatus},
    source_call::{SourceCallHost, SourceCallLimits},
};
use leselang_runtime_core::*;
use leselang_syntax::{NamedArgument, SyntaxTree, parse};
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    rc::Rc,
};

const GENERATION: u64 = 501;
const VERSION: u32 = 29;
#[derive(Debug)]
struct Refusal;
#[derive(Clone, Copy, PartialEq)]
enum AckKind {
    Text,
    Visibility,
}
impl AckKind {
    fn capability(self) -> u8 {
        match self {
            Self::Text => 1,
            Self::Visibility => 2,
        }
    }
    fn scalar(self) -> ScalarType {
        match self {
            Self::Text => ScalarType::String,
            Self::Visibility => ScalarType::Boolean,
        }
    }
}
struct Widget {
    generation: Cell<u64>,
    grants: Cell<u8>,
    text: RefCell<String>,
    visible: Cell<bool>,
    invocations: RefCell<Vec<&'static str>>,
}
impl Widget {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            generation: Cell::new(GENERATION),
            grants: Cell::new(3),
            text: RefCell::new(String::new()),
            visible: Cell::new(false),
            invocations: RefCell::new(Vec::new()),
        })
    }
    fn admit(&self, kind: AckKind) -> Result<(), Refusal> {
        if self.generation.get() == GENERATION && self.grants.get() & kind.capability() != 0 {
            Ok(())
        } else {
            Err(Refusal)
        }
    }
}
// Native keys, fields, declarations and replies deliberately have no Clone/Debug/serde.
struct Key {
    label: &'static str,
    owner: Rc<Widget>,
}
impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.label
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && Rc::ptr_eq(&self.owner, &other.owner)
    }
}
struct Field(AckKind);
struct Opaque;
struct Domain {
    kind: AckKind,
    owner: Rc<Widget>,
}
struct Declaration<'s>(&'s Domain);
type Node<'s> = Computation<Field, Key, Opaque, Declaration<'s>>;
type Branch<'s> = ComputedBranch<Node<'s>, Declaration<'s>>;
type Schema<'s> = OperationSchema<'s, Key, &'s str, ScalarTypeSet, Domain, u8>;
type Source<'h, 's> = SourceCallHost<'h, 's, Key, ScalarTypeSet, Domain, u8>;
type Prepared<'s> = PreparedCall<'s, Key, ScalarTypeSet, Domain, u8>;

struct Member<'s> {
    name: String,
    key: &'static str,
    domain: &'s Domain,
}
#[derive(Clone)]
enum Meta<'s> {
    Ack(&'s Domain),
    Group(GroupKind, Rc<Vec<Member<'s>>>),
}
impl PartialEq for Meta<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ack(a), Self::Ack(b)) => std::ptr::eq(*a, *b),
            (Self::Group(a, left), Self::Group(b, right)) => {
                a == b
                    && left.len() == right.len()
                    && left.iter().zip(right.iter()).all(|(a, b)| {
                        a.name == b.name && a.key == b.key && std::ptr::eq(a.domain, b.domain)
                    })
            }
            _ => false,
        }
    }
}
struct Types<'s> {
    owner: Rc<Widget>,
    schema: std::marker::PhantomData<&'s Domain>,
}
impl<'s> PureTypeEnvironment<Field, Key> for Types<'s> {
    type Result = Meta<'s>;
    fn field_type(&self, result: &Meta<'s>, field: &Field) -> Option<ScalarType> {
        match result {
            Meta::Ack(domain)
                if domain.kind == field.0 && Rc::ptr_eq(&domain.owner, &self.owner) =>
            {
                Some(field.0.scalar())
            }
            _ => None,
        }
    }
    fn member_result(&self, result: &Meta<'s>, name: &str, operation: &Key) -> Option<Meta<'s>> {
        let Meta::Group(_, rows) = result else {
            return None;
        };
        if !Rc::ptr_eq(&operation.owner, &self.owner) {
            return None;
        }
        rows.iter()
            .find(|row| row.name == name && row.key == operation.label)
            .map(|row| Meta::Ack(row.domain))
    }
}
impl<'s> CallFlowEnvironment<'s, Field, Key, Domain> for Types<'s> {
    fn call_result_type(&self, key: &Key, domain: &'s Domain) -> Option<PureType<Meta<'s>>> {
        (Rc::ptr_eq(&key.owner, &self.owner) && Rc::ptr_eq(&domain.owner, &self.owner))
            .then_some(PureType::Result(Meta::Ack(domain)))
    }
}
impl<'source, 's> CatalogSourceEnvironment<'source, 's, Field, Key, Domain> for Types<'s> {
    type Error = Refusal;
    fn source_operation(&self, original: &Key) -> Result<Key, Refusal> {
        Ok(Key {
            label: original.label,
            owner: original.owner.clone(),
        })
    }
    fn source_field(
        &self,
        result: &Meta<'s>,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Refusal> {
        let kind = match name {
            "text" => AckKind::Text,
            "visible" => AckKind::Visibility,
            _ => return Ok(None),
        };
        Ok(self
            .field_type(result, &Field(kind))
            .map(|ty| (Field(kind), ty)))
    }
    fn source_member(
        &self,
        result: &Meta<'s>,
        _: &'source str,
        name: &'source str,
    ) -> Result<Option<(Key, Meta<'s>)>, Refusal> {
        let Meta::Group(_, rows) = result else {
            return Ok(None);
        };
        Ok(rows.iter().find(|row| row.name == name).map(|row| {
            (
                Key {
                    label: row.key,
                    owner: self.owner.clone(),
                },
                Meta::Ack(row.domain),
            )
        }))
    }
}
impl<'e, 's> GroupFlowEnvironment<'e, 's, Field, Key, Domain, Declaration<'s>> for Types<'s> {
    fn group_branch_type_matches(
        &self,
        declared: &Declaration<'s>,
        inferred: &PureType<Meta<'s>>,
    ) -> bool {
        matches!(inferred, PureType::Result(Meta::Ack(domain)) if std::ptr::eq(*domain, declared.0))
    }
    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'e, Key, Meta<'s>>],
    ) -> Option<Meta<'s>> {
        let mut rows = Vec::new();
        for member in members {
            let PureType::Result(Meta::Ack(domain)) = member.inferred else {
                return None;
            };
            if !Rc::ptr_eq(&member.operation.owner, &self.owner) {
                return None;
            }
            rows.push(Member {
                name: member.name.into(),
                key: member.operation.label,
                domain,
            });
        }
        Some(Meta::Group(kind, Rc::new(rows)))
    }
}
impl<'source, 's> CatalogGroupSourceEnvironment<'source, 's, Field, Key, Domain, Declaration<'s>>
    for Types<'s>
{
    fn source_group_branch(
        &self,
        _: &'source NamedArgument,
        _: &Key,
        domain: &'s Domain,
        _: &PureType<Meta<'s>>,
    ) -> Result<Declaration<'s>, Refusal> {
        Ok(Declaration(domain))
    }
}

const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 512,
    max_depth: 64,
    max_bindings: 16,
};
const STACK: EffectStackLimits = EffectStackLimits {
    effects: EffectEvaluationLimits {
        pure: PURE,
        max_arguments: 4,
        max_branches: 4,
    },
    max_frames: 4,
};
const BATCH: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 512,
    max_depth: 64,
    max_branches: 4,
};
const COMPILE: CatalogGroupProgramLimits = CatalogGroupProgramLimits {
    program: CatalogProgramLimits {
        expression: CatalogSourceLimits {
            control: ControlSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 512,
                    max_source_depth: 64,
                    max_lowered_nodes: 512,
                    max_lowered_depth: 64,
                    max_arguments: 4,
                },
                max_bindings: 16,
            },
            max_calls: 32,
        },
        max_functions: 16,
        max_parameters: 4,
        max_compilation_nodes: 8192,
        max_expansion_depth: 32,
    },
    max_groups: 4,
    max_branches: 4,
};
fn tree(kind: &str) -> SyntaxTree {
    let parsed = parse(&format!(
        "fn main() = bind(suffix: \"!\", body: bind(answer: render(prefix: \"ready\"), body: concat(left: answer, right: suffix)))\n\
         fn render(prefix: string) = bind(batch: {kind}(label: prepare_text(prefix: prefix), visibility: prepare_visible()),\
          body: bind(label: member(value: batch, name: \"label\"), body: bind(state: member(value: batch, name: \"visibility\"),\
          body: choose(when: field(value: state, name: \"visible\"), then: field(value: label, name: \"text\"), otherwise: \"hidden\"))))\n\
         fn prepare_text(prefix: string) = bind(message: concat(left: prefix, right: \".\"),\
          body: choose(when: true, then: label.set_text(target: \"status\", text: message), otherwise: label.set_text(target: \"status\", text: \"\")))\n\
         fn prepare_visible() = bind(flag: true, body: choose(when: flag, then: widget.set_visible(target: \"status\", visible: flag), otherwise: widget.set_visible(target: \"status\", visible: false)))"
    ));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed
}
fn compile<'s>(tree: &SyntaxTree, source: &Source<'_, 's>, types: &Types<'s>) -> Node<'s> {
    let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    let (node, ty) =
        compile_catalog_prepared_group_helper_program(&functions, "main", COMPILE, source, types)
            .unwrap();
    assert!(matches!(ty, PureType::Scalar(ScalarType::String)));
    node
}
fn group<'ir, 's>(node: &'ir Node<'s>) -> &'ir Node<'s> {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        if matches!(node, Node::Group { .. }) {
            return node;
        }
        pending.extend(node.children());
    }
    panic!("typed fixture has no group")
}
fn fixture(test: impl for<'h, 's> FnOnce(&Source<'h, 's>, &Types<'s>, &'s Schema<'s>)) {
    let owner = Widget::new();
    let text = [
        NamedParameter::required("target", ScalarTypeSet::only(ScalarType::String)),
        NamedParameter::required("text", ScalarTypeSet::only(ScalarType::String)),
    ];
    let visible = [
        NamedParameter::required("target", ScalarTypeSet::only(ScalarType::String)),
        NamedParameter::required("visible", ScalarTypeSet::only(ScalarType::Boolean)),
    ];
    let schemas = [
        OperationSchema {
            key: Key {
                label: "label.set_text",
                owner: owner.clone(),
            },
            parameters: &text,
            required_capability: 101,
            result: Domain {
                kind: AckKind::Text,
                owner: owner.clone(),
            },
        },
        OperationSchema {
            key: Key {
                label: "widget.set_visible",
                owner: owner.clone(),
            },
            parameters: &visible,
            required_capability: 102,
            result: Domain {
                kind: AckKind::Visibility,
                owner: owner.clone(),
            },
        },
    ];
    let foreign = OperationSchema {
        key: Key {
            label: "label.set_text",
            owner: Widget::new(),
        },
        parameters: &text,
        required_capability: 101,
        result: Domain {
            kind: AckKind::Text,
            owner: Widget::new(),
        },
    };
    let catalog = OperationCatalog::new(
        VERSION,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 2,
        },
    )
    .unwrap();
    let source = SourceCallHost {
        catalog: &catalog,
        version: VERSION,
        granted: &[101, 102],
    };
    test(
        &source,
        &Types {
            owner,
            schema: std::marker::PhantomData,
        },
        &foreign,
    );
}

struct Ack<'s> {
    schema: &'s Schema<'s>,
    generation: u64,
    value: ScalarValue,
}
struct Input<'s> {
    ack: Rc<Ack<'s>>,
    buffer: Box<[u8]>,
}
impl<'s> Borrow<Ack<'s>> for Input<'s> {
    fn borrow(&self) -> &Ack<'s> {
        &self.ack
    }
}
impl<'s> HostResultDomain<Ack<'s>> for Schema<'s> {
    type Error = Refusal;
    fn matches_type(&self, ack: &Ack<'s>) -> bool {
        std::ptr::eq(self, ack.schema) && ack.value.scalar_type() == self.result.kind.scalar()
    }
    fn validate_value(&self, ack: &Ack<'s>) -> Result<(), Refusal> {
        self.result.owner.admit(self.result.kind)?;
        if !self.matches_type(ack) || ack.generation != GENERATION {
            return Err(Refusal);
        }
        match &ack.value {
            ScalarValue::String(value) if value.len() <= 64 => Ok(()),
            ScalarValue::Boolean(_) => Ok(()),
            _ => Err(Refusal),
        }
    }
}
type Accepted<'ir, 's> =
    ParallelAccepted<'ir, Node<'s>, Declaration<'s>, (u64, usize), Schema<'s>, Input<'s>>;
type Parallel<'ir, 's> =
    PreparedParallelSession<'ir, Node<'s>, Declaration<'s>, (u64, usize), Schema<'s>, Input<'s>>;
type Sequence<'ir, 's> =
    PreparedSequenceSession<'ir, Node<'s>, Declaration<'s>, (u64, usize), Schema<'s>, Prepared<'s>>;
struct Collected<'ir, 's> {
    group: &'ir Node<'s>,
    rows: Vec<Accepted<'ir, 's>>,
    buffer: Box<[u8]>,
}
struct GroupView<'ir, 's> {
    group: &'ir Node<'s>,
    rows: Vec<(&'ir Branch<'s>, Rc<Ack<'s>>)>,
}
#[derive(Clone)]
enum View<'ir, 's> {
    Group(Rc<GroupView<'ir, 's>>),
    Ack(Rc<Ack<'s>>),
}
struct Request<'ir, 's> {
    group: &'ir Node<'s>,
    prepared: PreparedNativeGroup<'ir, Branch<'s>, Prepared<'s>>,
}
enum Dispatch<'ir, 's> {
    Sequence(Sequence<'ir, 's>),
    Parallel {
        barrier: Parallel<'ir, 's>,
        requests: Vec<ParallelRequest<Prepared<'s>>>,
    },
}
struct Runtime<'ir, 'h, 's> {
    group: &'ir Node<'s>,
    source: &'h Source<'h, 's>,
    widget: Rc<Widget>,
    fail_admission: Cell<bool>,
    restores: Cell<usize>,
    projections: Cell<usize>,
}
impl<'ir, 'h, 's> Runtime<'ir, 'h, 's> {
    fn new(node: &'ir Node<'s>, source: &'h Source<'h, 's>, widget: Rc<Widget>) -> Self {
        Self {
            group: group(node),
            source,
            widget,
            fail_admission: Cell::new(false),
            restores: Cell::new(0),
            projections: Cell::new(0),
        }
    }
    fn branches(&self) -> &[Branch<'s>] {
        match self.group {
            Node::Group { branches, .. } => branches,
            _ => panic!("fixture group"),
        }
    }
    fn schema(&self, branch: &Branch<'s>) -> Result<&'s Schema<'s>, Refusal> {
        let mut pending = vec![&branch.value];
        let mut original: Option<&Schema<'s>> = None;
        let mut count = 0;
        // Already compiled/typed IR; fresh native policy still checks every cold call row.
        while let Some(node) = pending.pop() {
            count += 1;
            if count > PURE.max_nodes {
                return Err(Refusal);
            }
            match node {
                Node::Call { operation, .. } => {
                    let schema = self
                        .source
                        .catalog
                        .authorize(operation.label, self.source.version, self.source.granted)
                        .map_err(|_| Refusal)?;
                    self.widget.admit(schema.result.kind)?;
                    if operation != &schema.key
                        || !Rc::ptr_eq(&schema.result.owner, &self.widget)
                        || !std::ptr::eq(branch.result_type.0, &schema.result)
                        || original.is_some_and(|row| !std::ptr::eq(row, schema))
                    {
                        return Err(Refusal);
                    }
                    original = Some(schema);
                }
                Node::Host { .. } | Node::Group { .. } => return Err(Refusal),
                _ => pending.extend(node.children()),
            }
        }
        original.ok_or(Refusal)
    }
    fn admit_prepared(&self, branch: &Branch<'s>, prepared: &Prepared<'s>) -> Result<(), Refusal> {
        if std::ptr::eq(self.schema(branch)?, prepared.schema()) {
            Ok(())
        } else {
            Err(Refusal)
        }
    }
    fn correlate(
        &self,
        index: usize,
        original: &'ir Branch<'s>,
        prepared: Prepared<'s>,
        fuel: &mut Fuel,
    ) -> Result<PreparedParallelMember<'ir, (u64, usize), Schema<'s>, Prepared<'s>>, Refusal> {
        self.admit_prepared(original, &prepared)?;
        fuel.charge(1).map_err(|_| Refusal)?;
        Ok(PreparedParallelMember {
            identity: (GENERATION, index),
            declaration: self.schema(original)?,
            request: prepared,
        })
    }
    fn publish(
        &self,
        sequence: &mut Sequence<'ir, 's>,
    ) -> Result<SequencePoll<Prepared<'s>>, PreparedSequenceError<Refusal>> {
        sequence.poll(|index, original, identity, domain, prepared| {
            self.authorize(identity)?;
            self.admit_prepared(original, prepared)?;
            if *identity != (GENERATION, index) || !std::ptr::eq(domain, self.schema(original)?) {
                return Err(Refusal);
            }
            Ok(())
        })
    }
    fn invoke(&self, request: ParallelRequest<Prepared<'s>>) -> Result<Input<'s>, Refusal> {
        let branch = self.branches().get(request.index).ok_or(Refusal)?;
        self.admit_prepared(branch, &request.request)?;
        let (schema, args) = request.request.into_parts();
        if args.len() != 2 || args[0].value != ScalarValue::String("status".into()) {
            return Err(Refusal);
        }
        let value = match (schema.result.kind, &args[1].value) {
            (AckKind::Text, ScalarValue::String(text)) if text.len() <= 64 => {
                self.widget.text.replace(text.clone());
                ScalarValue::String(text.clone())
            }
            (AckKind::Visibility, ScalarValue::Boolean(visible)) => {
                self.widget.visible.set(*visible);
                ScalarValue::Boolean(*visible)
            }
            _ => return Err(Refusal),
        };
        self.widget.invocations.borrow_mut().push(schema.key.label);
        Ok(Input {
            ack: Rc::new(Ack {
                schema,
                generation: GENERATION,
                value,
            }),
            buffer: vec![17; 9].into_boxed_slice(),
        })
    }
}
impl<'ir, 's> HostResultDomain<Collected<'ir, 's>> for Node<'s> {
    type Error = Refusal;
    fn matches_type(&self, reply: &Collected<'ir, 's>) -> bool {
        std::ptr::eq(self, reply.group)
    }
    fn validate_value(&self, reply: &Collected<'ir, 's>) -> Result<(), Refusal> {
        let Node::Group { branches, .. } = self else {
            return Err(Refusal);
        };
        if !self.matches_type(reply) || reply.rows.len() != branches.len() {
            return Err(Refusal);
        }
        for (i, row) in reply.rows.iter().enumerate() {
            let (index, original, identity, schema, input) = row.as_parts();
            if index != i
                || *identity != (GENERATION, i)
                || !std::ptr::eq(original, &branches[i])
                || !std::ptr::eq(&schema.result, original.result_type.0)
            {
                return Err(Refusal);
            }
            schema.validate_value(&input.ack)?;
        }
        Ok(())
    }
}
impl<'ir, 's> PureEvaluationEnvironment<Field, Key> for Runtime<'ir, '_, 's> {
    type Result = View<'ir, 's>;
    type Error = Refusal;
    fn field(&self, value: &Self::Result, field: &Field) -> Result<ScalarValue, Refusal> {
        let View::Ack(ack) = value else {
            return Err(Refusal);
        };
        ack.schema.validate_value(ack.as_ref())?;
        if !Rc::ptr_eq(&ack.schema.result.owner, &self.widget) || field.0 != ack.schema.result.kind
        {
            return Err(Refusal);
        }
        self.projections.set(self.projections.get() + 1);
        Ok(ack.value.clone())
    }
    fn member(
        &self,
        value: &Self::Result,
        name: &str,
        operation: &Key,
    ) -> Result<Self::Result, Refusal> {
        let View::Group(group) = value else {
            return Err(Refusal);
        };
        self.authorize(&GENERATION)?;
        if !std::ptr::eq(group.group, self.group) {
            return Err(Refusal);
        }
        let (original, ack) = group
            .rows
            .iter()
            .find(|(b, _)| b.name == name)
            .ok_or(Refusal)?;
        if operation != &self.schema(original)?.key {
            return Err(Refusal);
        }
        ack.schema.validate_value(ack.as_ref())?;
        Ok(View::Ack(ack.clone()))
    }
}
impl<'ir, 's> EffectEvaluationEnvironment<'ir, Field, Key, Opaque, Declaration<'s>>
    for Runtime<'ir, '_, 's>
{
    type Request = Request<'ir, 's>;
    type Capture = RestoredEffectBindings<'ir, View<'ir, 's>>;
    fn preflight_effect(&self, node: &'ir Node<'s>) -> Result<(), Refusal> {
        let mut pending = vec![self.group];
        let mut count = 0;
        let mut found = false;
        while let Some(part) = pending.pop() {
            count += 1;
            if count > PURE.max_nodes {
                return Err(Refusal);
            }
            found |= std::ptr::eq(part, node);
            pending.extend(part.children());
        }
        if !found {
            return Err(Refusal);
        }
        for branch in self.branches() {
            self.schema(branch)?;
        }
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node<'s>,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Refusal>> {
        if !std::ptr::eq(node, self.group) {
            return Err(Refusal.into());
        }
        let prepared = prepare_atomic_group_in_scope(
            node,
            scope,
            &CallEvaluationHost {
                catalog: self.source.catalog,
                version: self.source.version,
                granted: self.source.granted,
                environment: self,
            },
            fuel,
            PreparedGroupEvaluationLimits {
                call: CallEvaluationLimits {
                    pure: PURE,
                    max_arguments: 4,
                },
                max_branches: 4,
            },
            |index, original, schema| {
                if (index == 1 && self.fail_admission.get())
                    || !std::ptr::eq(schema, self.schema(original)?)
                {
                    return Err(Refusal);
                }
                Ok(())
            },
        )
        .map_err(|_| Refusal)?;
        Ok(Request {
            group: node,
            prepared,
        })
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node<'s>,
        scope: &ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Refusal> {
        let mut saved = Vec::new();
        for (name, value) in scope.bindings() {
            let extra = match value {
                PureValue::Scalar(scalar) => scalar_copy_cost(scalar),
                _ => 0,
            };
            fuel.charge(1 + extra).map_err(|_| Refusal)?;
            saved.push((*name, value.clone()));
        }
        Ok(saved)
    }
}
impl ReplyAuthority<u64> for Runtime<'_, '_, '_> {
    type Error = Refusal;
    fn authorize(&self, identity: &u64) -> Result<(), Refusal> {
        if *identity != GENERATION {
            return Err(Refusal);
        }
        for branch in self.branches() {
            self.schema(branch)?;
        }
        Ok(())
    }
}
impl ReplyAuthority<(u64, usize)> for Runtime<'_, '_, '_> {
    type Error = Refusal;
    fn authorize(&self, identity: &(u64, usize)) -> Result<(), Refusal> {
        if identity.0 != GENERATION {
            return Err(Refusal);
        }
        self.schema(self.branches().get(identity.1).ok_or(Refusal)?)
            .map(|_| ())
    }
}
impl<'ir, 's>
    EffectReentryEnvironment<
        'ir,
        Field,
        Key,
        Opaque,
        Declaration<'s>,
        u64,
        Node<'s>,
        Collected<'ir, 's>,
    > for Runtime<'ir, '_, 's>
{
    fn restore_capture(
        &self,
        identity: &u64,
        declaration: &Node<'s>,
        saved: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self.group, declaration) {
            return Err(Refusal.into());
        }
        self.restores.set(self.restores.get() + 1);
        Ok(saved)
    }
    fn bind_reply(
        &self,
        identity: &u64,
        declaration: &Node<'s>,
        reply: Collected<'ir, 's>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<Self::Result>, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self.group, declaration) {
            return Err(Refusal.into());
        }
        declaration.validate_value(&reply)?;
        let rows = assemble_group_reply(reply.group, reply.rows, BATCH, fuel, |whole, _| {
            for (i, row) in whole.members().iter().enumerate() {
                let (index, original, identity, schema, input) = row.as_parts();
                if index != i
                    || *identity != (GENERATION, i)
                    || !std::ptr::eq(original, &self.branches()[i])
                    || !std::ptr::eq(schema, self.schema(original)?)
                {
                    return Err(Refusal);
                }
                schema.validate_value(&input.ack)?;
            }
            Ok(())
        })
        .map_err(|_| Refusal)?;
        let group = rows.group();
        let rows = rows
            .into_members()
            .into_iter()
            .map(|row| {
                let (_, original, _, _, input) = row.into_parts();
                (original, input.ack)
            })
            .collect();
        Ok(PureValue::Result(View::Group(Rc::new(GroupView {
            group,
            rows,
        }))))
    }
}
impl<'d, 'ir: 'd, 's> EffectSessionEnvironment<'ir, 'd, Field, Key, Opaque, Declaration<'s>>
    for Runtime<'ir, '_, 's>
{
    type Identity = u64;
    type Declaration = Node<'s>;
    type Reply = Collected<'ir, 's>;
    type Dispatch = Dispatch<'ir, 's>;
    fn correlate_request(
        &self,
        request: Self::Request,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'d, u64, Node<'s>, Self::Dispatch, Refusal> {
        let Node::Group { group_kind, .. } = request.group else {
            return Err(Refusal.into());
        };
        let dispatch = match group_kind {
            GroupKind::Sequence => Dispatch::Sequence(
                Sequence::start(
                    request.group,
                    request.prepared,
                    fuel,
                    BATCH,
                    |_, original, prepared| self.admit_prepared(original, prepared),
                    |i, original, prepared, fuel| self.correlate(i, original, prepared, fuel),
                )
                .map_err(|_| Refusal)?,
            ),
            GroupKind::Parallel => {
                let (barrier, requests) = Parallel::start(
                    request.group,
                    request.prepared,
                    fuel,
                    BATCH,
                    |_, original, prepared| self.admit_prepared(original, prepared),
                    |i, original, prepared, fuel| self.correlate(i, original, prepared, fuel),
                )
                .map_err(|_| Refusal)?;
                Dispatch::Parallel { barrier, requests }
            }
        };
        Ok(CorrelatedEffectRequest {
            identity: GENERATION,
            declaration: self.group,
            dispatch,
        })
    }
}

fn collect<'ir, 's>(
    runtime: &Runtime<'ir, '_, 's>,
    dispatch: Dispatch<'ir, 's>,
) -> Collected<'ir, 's> {
    let rows = match dispatch {
        Dispatch::Sequence(mut sequence) => {
            let mut rows = Vec::new();
            for expected in 0..2 {
                let SequencePoll::Request { index, request } =
                    runtime.publish(&mut sequence).unwrap()
                else {
                    panic!()
                };
                assert_eq!(index, expected);
                assert!(
                    matches!(runtime.publish(&mut sequence).unwrap(), SequencePoll::Awaiting { index: pending } if pending == index)
                );
                let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
                rows.push(
                    sequence
                        .try_accept(&(GENERATION, index), input, runtime)
                        .unwrap(),
                );
            }
            assert_eq!(
                sequence.status(),
                SequenceStatus::Terminal(SequenceEnd::Completed)
            );
            rows
        }
        Dispatch::Parallel {
            mut barrier,
            requests,
        } => {
            for request in requests.into_iter().rev() {
                let index = request.index;
                let input = runtime.invoke(request).unwrap();
                barrier
                    .try_accept(index, &(GENERATION, index), input, runtime)
                    .unwrap();
                if index == 1 {
                    assert!(matches!(
                        barrier.poll().unwrap(),
                        ParallelPoll::Awaiting { remaining: 1 }
                    ));
                }
            }
            let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
                panic!()
            };
            rows
        }
    };
    Collected {
        group: runtime.group,
        rows,
        buffer: vec![23; 11].into_boxed_slice(),
    }
}

#[test]
fn parsed_gui_helpers_return_heterogeneous_acks_through_seq_all_and_one_enclosing_meter() {
    for kind in ["seq", "all"] {
        fixture(|source, types, _| {
            let node = compile(&tree(kind), source, types);
            let runtime = Runtime::new(&node, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            let prepared_fuel = session.fuel_remaining();
            assert_eq!(prepared_fuel, 948);
            assert!(runtime.widget.invocations.borrow().is_empty());
            for _ in 0..2 {
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Awaiting
                ));
                assert_eq!(session.fuel_remaining(), prepared_fuel);
            }
            let reply = collect(&runtime, dispatch);
            assert_eq!(session.fuel_remaining(), prepared_fuel);
            let pointer = reply.buffer.as_ptr();
            let mut reply = session
                .try_accept(&(GENERATION + 1), reply, &runtime)
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            reply.rows.swap(0, 1);
            let mut reply = session
                .try_accept(&GENERATION, reply, &runtime)
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            reply.rows.swap(0, 1);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            session.try_accept(&GENERATION, reply, &runtime).unwrap();
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(session.fuel_remaining(), prepared_fuel);
            assert!(
                matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
            );
            assert_eq!(&*runtime.widget.text.borrow(), "ready.");
            assert!(runtime.widget.visible.get());
            assert_eq!(
                *runtime.widget.invocations.borrow(),
                if kind == "seq" {
                    vec!["label.set_text", "widget.set_visible"]
                } else {
                    vec!["widget.set_visible", "label.set_text"]
                }
            );
            assert_eq!(runtime.restores.get(), 2);
            assert_eq!(runtime.projections.get(), 2);
            assert_eq!(weak.strong_count(), 0);
            let final_fuel = session.fuel_remaining();
            assert_eq!(final_fuel, 925);
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
            ));
            assert_eq!(session.fuel_remaining(), final_fuel);
        });
    }
}

#[test]
fn gui_member_reply_rejections_preserve_original_payload_and_pending_sequence() {
    fixture(|source, types, foreign| {
        let node = compile(&tree("seq"), source, types);
        let runtime = Runtime::new(&node, source, types.owner.clone());
        let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(Dispatch::Sequence(mut sequence)) =
            session.poll(&runtime).unwrap()
        else {
            panic!()
        };
        let SequencePoll::Request { index, request } = runtime.publish(&mut sequence).unwrap()
        else {
            panic!()
        };
        let valid = runtime.invoke(ParallelRequest { index, request }).unwrap();
        let original = valid.ack.schema;
        for (schema, generation, value) in [
            (foreign, GENERATION, ScalarValue::String("ready.".into())),
            (original, GENERATION, ScalarValue::Boolean(true)),
            (
                original,
                GENERATION + 1,
                ScalarValue::String("ready.".into()),
            ),
            (original, GENERATION, ScalarValue::String("x".repeat(65))),
        ] {
            let input = Input {
                ack: Rc::new(Ack {
                    schema,
                    generation,
                    value,
                }),
                buffer: vec![19; 7].into_boxed_slice(),
            };
            let pointer = input.buffer.as_ptr();
            let weak = Rc::downgrade(&input.ack);
            let rejected = sequence
                .try_accept(&(GENERATION, index), input, &runtime)
                .unwrap_err();
            assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
            assert_eq!(weak.strong_count(), 1);
            assert!(
                matches!(runtime.publish(&mut sequence).unwrap(), SequencePoll::Awaiting { index: pending } if pending == index)
            );
            drop(rejected);
            assert_eq!(weak.strong_count(), 0);
        }
        let pointer = valid.buffer.as_ptr();
        let rejected = sequence
            .try_accept(&(GENERATION + 1, index), valid, &runtime)
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
        drop(
            sequence
                .try_accept(&(GENERATION, index), rejected.reply, &runtime)
                .unwrap(),
        );
        assert_eq!(runtime.widget.invocations.borrow().len(), 1);
        assert_eq!(runtime.projections.get(), 0);
        assert!(sequence.cancel());
        assert!(session.cancel());
    });
}

#[test]
fn gui_generation_revocation_or_cancel_blocks_next_publication_without_undoing_first_mutation() {
    for phase in 0..3 {
        fixture(|source, types, _| {
            let node = compile(&tree("seq"), source, types);
            let runtime = Runtime::new(&node, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(Dispatch::Sequence(mut sequence)) =
                session.poll(&runtime).unwrap()
            else {
                panic!()
            };
            let SequencePoll::Request { index, request } = runtime.publish(&mut sequence).unwrap()
            else {
                panic!()
            };
            drop(
                sequence
                    .try_accept(
                        &(GENERATION, index),
                        runtime.invoke(ParallelRequest { index, request }).unwrap(),
                        &runtime,
                    )
                    .unwrap(),
            );
            let fuel = session.fuel_remaining();
            match phase {
                0 => {
                    assert!(sequence.cancel());
                }
                1 => {
                    runtime.widget.generation.set(GENERATION + 1);
                    assert!(runtime.publish(&mut sequence).is_err());
                }
                _ => {
                    runtime.widget.grants.set(1);
                    assert!(runtime.publish(&mut sequence).is_err());
                }
            }
            assert!(matches!(
                runtime.publish(&mut sequence).unwrap(),
                SequencePoll::Terminal(_)
            ));
            assert!(session.cancel());
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(&*runtime.widget.text.borrow(), "ready.");
            assert!(!runtime.widget.visible.get());
            assert_eq!(*runtime.widget.invocations.borrow(), ["label.set_text"]);
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
        });
    }
}

#[test]
fn gui_accepted_group_cancel_or_stale_generation_never_projects_or_returns_to_caller() {
    for cancel in [false, true] {
        fixture(|source, types, _| {
            let node = compile(&tree("all"), source, types);
            let runtime = Runtime::new(&node, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            session.try_accept(&GENERATION, reply, &runtime).unwrap();
            let fuel = session.fuel_remaining();
            if cancel {
                assert!(session.cancel());
            } else {
                runtime.widget.generation.set(GENERATION + 1);
                assert!(session.poll(&runtime).is_err());
            }
            assert_eq!(weak.strong_count(), 0);
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(runtime.widget.invocations.borrow().len(), 2);
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
        });
    }
}

#[test]
fn gui_cold_types_missing_grants_preparation_refusal_and_exhaustion_never_invoke() {
    fixture(|source, types, _| {
        for kind in ["seq", "all"] {
            let valid = tree(kind);
            for source_text in [
                "fn main() = all(label: label.set_text(target: \"status\", text: true))",
                "fn main() = bind(batch: all(label: label.set_text(target: \"status\", text: \"ok\")), body: bind(ack: member(value: batch, name: \"label\"), body: field(value: ack, name: \"visible\")))",
                "fn main() = all(label: label.unknown(target: \"status\", text: \"ok\"))",
            ] {
                let parsed = parse(source_text);
                let functions: Vec<_> = parsed.function.iter().chain(&parsed.helpers).collect();
                let result: Result<(Node<'_>, PureType<Meta<'_>>), _> =
                    compile_catalog_prepared_group_helper_program(
                        &functions, "main", COMPILE, source, types,
                    );
                assert!(result.is_err());
            }
            let functions: Vec<_> = valid.function.iter().chain(&valid.helpers).collect();
            for (version, granted) in [(VERSION, &[101][..]), (VERSION + 1, &[101, 102][..])] {
                let denied = SourceCallHost {
                    catalog: source.catalog,
                    version,
                    granted,
                };
                let result: Result<(Node<'_>, PureType<Meta<'_>>), _> =
                    compile_catalog_prepared_group_helper_program(
                        &functions, "main", COMPILE, &denied, types,
                    );
                assert!(result.is_err());
            }
            let node = compile(&valid, source, types);
            let runtime = Runtime::new(&node, source, types.owner.clone());
            runtime.fail_admission.set(true);
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            assert!(session.poll(&runtime).is_err());
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            );
            runtime.fail_admission.set(false);
            let mut reference =
                StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            assert!(matches!(
                reference.poll(&runtime).unwrap(),
                EffectSessionPoll::Request(_)
            ));
            let required = 1000 - reference.fuel_remaining();
            assert_eq!(required, 52);
            for fuel in 0..required {
                let mut session =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(fuel), STACK);
                assert!(session.poll(&runtime).is_err());
                let remaining = session.fuel_remaining();
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                ));
                assert_eq!(session.fuel_remaining(), remaining);
            }
            assert!(runtime.widget.invocations.borrow().is_empty());
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
        }
    });
}

#[test]
fn gui_prepared_parallel_data_is_not_a_grant_to_invoke_after_revocation() {
    fixture(|source, types, _| {
        let node = compile(&tree("all"), source, types);
        let runtime = Runtime::new(&node, source, types.owner.clone());
        let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(Dispatch::Parallel {
            mut barrier,
            requests,
        }) = session.poll(&runtime).unwrap()
        else {
            panic!()
        };
        runtime.widget.grants.set(0);
        for request in requests {
            assert!(runtime.invoke(request).is_err());
        }
        assert!(barrier.cancel());
        assert!(session.cancel());
        assert!(runtime.widget.invocations.borrow().is_empty());
        assert!(runtime.widget.text.borrow().is_empty());
        assert!(!runtime.widget.visible.get());
    });
}

#[test]
fn gui_exhaustion_at_every_enclosing_reentry_debit_seals_without_replaying_mutations() {
    for kind in ["seq", "all"] {
        for fuel in 52..=75 {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let runtime = Runtime::new(&node, source, types.owner.clone());
                let mut session =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(fuel), STACK);
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let reply = collect(&runtime, dispatch);
                let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
                session.try_accept(&GENERATION, reply, &runtime).unwrap();
                let end = if fuel == 75 {
                    assert!(
                        matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
                    );
                    assert_eq!(session.fuel_remaining(), 0);
                    EffectSessionEnd::Completed
                } else {
                    assert!(session.poll(&runtime).is_err());
                    EffectSessionEnd::Failed
                };
                assert_eq!(session.status(), EffectSessionStatus::Terminal(end));
                assert_eq!(weak.strong_count(), 0);
                let remaining = session.fuel_remaining();
                for _ in 0..2 {
                    assert!(matches!(
                        session.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(actual) if actual == end
                    ));
                    assert_eq!(session.fuel_remaining(), remaining);
                    assert_eq!(runtime.widget.invocations.borrow().len(), 2);
                }
                assert_eq!(&*runtime.widget.text.borrow(), "ready.");
                assert!(runtime.widget.visible.get());
            });
        }
    }
}
