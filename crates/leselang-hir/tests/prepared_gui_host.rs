//! Headless GUI protocol model, not an Avalonia/native-window adapter.
use leselang_hir::{
    call_evaluation::{CallEvaluationHost, CallEvaluationLimits, PreparedCall},
    catalog_group_source::{
        CatalogGroupProgramLimits, CatalogGroupSourceEnvironment, CatalogGroupSourceLimits,
        compile_catalog_prepared_group_helper_program, compile_catalog_prepared_group_source,
    },
    catalog_program::CatalogProgramLimits,
    catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits},
    control_source::ControlSourceLimits,
    effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits},
    effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings},
    effect_session::{
        CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
        EffectSessionPoll, EffectSessionStatus, HostWorkOutcome,
    },
    effect_stack::EffectStackLimits,
    effect_stack_session::StackedEffectSession,
    flow_typing::{CallFlowEnvironment, GroupFlowEnvironment, GroupMemberType},
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    local_effect_session::LocalStackedEffectSession,
    native_group_preparation::PreparedNativeGroup,
    parallel_evaluation::{
        ParallelAccepted, ParallelEnd, ParallelEvaluationLimits, ParallelPoll, ParallelRequest,
        ParallelStatus, PreparedParallelMember,
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

fn repeat_tree(group: &str) -> SyntaxTree {
    sequential_tree(group, "iteration_3")
}

fn sequential_tree(group: &str, selected: &str) -> SyntaxTree {
    let tree = parse(&format!(
        "fn main() = bind(suffix: \"!\", body: bind(answer: render(prefix: \"ready\"), body: concat(left: answer, right: suffix)))\n\
         fn render(prefix: string) = bind(batch: {group},\
             body: bind(ack: member(value: batch, name: \"{selected}\"), body: field(value: ack, name: \"text\")))\n\
         fn prepare_text(prefix: string) = bind(message: concat(left: prefix, right: \".\"), body: label.set_text(target: \"status\", text: message))\n\
         fn prepare_visible() = widget.set_visible(target: \"status\", visible: true)"
    ));
    assert!(tree.diagnostics.is_empty());
    tree
}
fn compile<'s>(tree: &SyntaxTree, source: &Source<'_, 's>, types: &Types<'s>) -> Node<'s> {
    let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    let (node, ty) =
        compile_catalog_prepared_group_helper_program(&functions, "main", COMPILE, source, types)
            .unwrap();
    assert!(matches!(ty, PureType::Scalar(ScalarType::String)));
    node
}

fn scoped_node<'s>(source: &Source<'_, 's>, types: &Types<'s>) -> Node<'s> {
    let parsed = parse(
        "fn main() = bind(batch: repeat(times: 2, body: seq(\
        label: bind(message: concat(left: prefix, right: \".\"), body: choose(when: enabled,\
            then: label.set_text(target: target, text: message), otherwise: label.set_text(target: target, text: \"\"))),\
        visible: widget.set_visible(target: target, visible: enabled))),\
        body: bind(ack: member(value: batch, name: \"iteration_2__label\"),\
            body: concat(left: field(value: ack, name: \"text\"), right: suffix)))",
    );
    assert!(parsed.diagnostics.is_empty());
    let text = PureType::Scalar(ScalarType::String);
    let boolean = PureType::Scalar(ScalarType::Boolean);
    let prefix = [
        ("target", &text),
        ("prefix", &text),
        ("suffix", &text),
        ("enabled", &boolean),
    ];
    let (node, result) = compile_catalog_prepared_group_source(
        &parsed.function.unwrap().body,
        &prefix,
        CatalogGroupSourceLimits {
            expression: COMPILE.program.expression,
            max_groups: COMPILE.max_groups,
            max_branches: COMPILE.max_branches,
        },
        source,
        types,
    )
    .unwrap();
    assert!(matches!(result, PureType::Scalar(ScalarType::String)));
    node
}

fn scoped_bindings<'ir, 's>(
    prefix: &str,
    suffix: &str,
) -> RestoredEffectBindings<'ir, View<'ir, 's>> {
    vec![
        (
            "target",
            PureValue::Scalar(ScalarValue::String("status".to_owned())),
        ),
        (
            "prefix",
            PureValue::Scalar(ScalarValue::String(prefix.to_owned())),
        ),
        (
            "suffix",
            PureValue::Scalar(ScalarValue::String(suffix.to_owned())),
        ),
        ("enabled", PureValue::Scalar(ScalarValue::Boolean(true))),
    ]
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
    identity: MemberIdentity,
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
// Local receipt provenance, not a serializable token or an authentication protocol.
struct ExecutionIdentity(LocalExecutionLease);
impl PartialEq for ExecutionIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_scope(&other.0)
    }
}
struct MemberIdentity {
    execution: ExecutionIdentity,
    index: usize,
}
impl PartialEq for MemberIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.execution == other.execution && self.index == other.index
    }
}
struct MemberDomain<'ir, 's> {
    identity: MemberIdentity,
    original: &'ir Branch<'s>,
}
impl<'s> HostResultDomain<Ack<'s>> for MemberDomain<'_, 's> {
    type Error = Refusal;
    fn matches_type(&self, ack: &Ack<'s>) -> bool {
        std::ptr::eq(self.original.result_type.0, &ack.schema.result)
            && ack.schema.matches_type(ack)
    }
    fn validate_value(&self, ack: &Ack<'s>) -> Result<(), Refusal> {
        if !self.identity.execution.0.is_open()
            || !self.matches_type(ack)
            || self.identity != ack.identity
        {
            return Err(Refusal);
        }
        ack.schema.validate_value(ack)
    }
}
struct Execution<'ir, 's> {
    identity: ExecutionIdentity,
    group: &'ir Node<'s>,
    members: Vec<MemberDomain<'ir, 's>>,
    correlated: Cell<bool>,
}
impl<'ir, 's> Execution<'ir, 's> {
    fn new(node: &'ir Node<'s>, lease: LocalExecutionLease) -> Self {
        let group = group(node);
        let Node::Group { branches, .. } = group else {
            unreachable!()
        };
        assert!(branches.len() <= COMPILE.max_branches);
        Self {
            identity: ExecutionIdentity(lease.clone()),
            group,
            members: branches
                .iter()
                .enumerate()
                .map(|(index, original)| MemberDomain {
                    identity: MemberIdentity {
                        execution: ExecutionIdentity(lease.clone()),
                        index,
                    },
                    original,
                })
                .collect(),
            correlated: Cell::new(false),
        }
    }
    fn identity(&self) -> ExecutionIdentity {
        ExecutionIdentity(self.identity.0.clone())
    }
    fn member(&self, index: usize) -> MemberIdentity {
        MemberIdentity {
            execution: self.identity(),
            index,
        }
    }
}
struct Invocation<'s> {
    identity: MemberIdentity,
    prepared: Prepared<'s>,
}
type Accepted<'ir, 's> = ParallelAccepted<
    'ir,
    Node<'s>,
    Declaration<'s>,
    MemberIdentity,
    MemberDomain<'ir, 's>,
    Input<'s>,
>;
type Parallel<'ir, 's> = PreparedParallelSession<
    'ir,
    Node<'s>,
    Declaration<'s>,
    MemberIdentity,
    MemberDomain<'ir, 's>,
    Input<'s>,
>;
type Sequence<'ir, 's> = PreparedSequenceSession<
    'ir,
    Node<'s>,
    Declaration<'s>,
    MemberIdentity,
    MemberDomain<'ir, 's>,
    Invocation<'s>,
>;
struct Collected<'ir, 's> {
    identity: ExecutionIdentity,
    group: &'ir Node<'s>,
    rows: Vec<Accepted<'ir, 's>>,
    buffer: Box<[u8]>,
}
struct GroupView<'ir, 's> {
    identity: ExecutionIdentity,
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
        requests: Vec<ParallelRequest<Invocation<'s>>>,
    },
}
struct Runtime<'ir, 'h, 's> {
    execution: &'ir Execution<'ir, 's>,
    group: &'ir Node<'s>,
    source: &'h Source<'h, 's>,
    widget: Rc<Widget>,
    fail_admission: Cell<bool>,
    panic_admission: Cell<bool>,
    restores: Cell<usize>,
    projections: Cell<usize>,
}
impl<'ir, 'h, 's> Runtime<'ir, 'h, 's> {
    fn new(
        execution: &'ir Execution<'ir, 's>,
        source: &'h Source<'h, 's>,
        widget: Rc<Widget>,
    ) -> Self {
        Self {
            execution,
            group: execution.group,
            source,
            widget,
            fail_admission: Cell::new(false),
            panic_admission: Cell::new(false),
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
    ) -> Result<
        PreparedParallelMember<'ir, MemberIdentity, MemberDomain<'ir, 's>, Invocation<'s>>,
        Refusal,
    > {
        self.admit_prepared(original, &prepared)?;
        fuel.charge(1).map_err(|_| Refusal)?;
        Ok(PreparedParallelMember {
            identity: self.execution.member(index),
            declaration: self.execution.members.get(index).ok_or(Refusal)?,
            request: Invocation {
                identity: self.execution.member(index),
                prepared,
            },
        })
    }
    fn publish(
        &self,
        sequence: &mut Sequence<'ir, 's>,
    ) -> Result<SequencePoll<Invocation<'s>>, PreparedSequenceError<Refusal>> {
        sequence.poll(|index, original, identity, domain, prepared| {
            self.admit_publication(index, original, identity, domain, prepared)
        })
    }
    fn publish_in_scope(
        &self,
        sequence: &mut Sequence<'ir, 's>,
        scope: &LocalExecutionScope,
    ) -> Result<SequencePoll<Invocation<'s>>, PreparedSequenceError<Refusal>> {
        sequence.poll_in_scope(
            |index, original, identity, domain, prepared| {
                self.admit_publication(index, original, identity, domain, prepared)
            },
            scope,
        )
    }
    fn admit_publication(
        &self,
        index: usize,
        original: &'ir Branch<'s>,
        identity: &MemberIdentity,
        domain: &'ir MemberDomain<'ir, 's>,
        prepared: &Invocation<'s>,
    ) -> Result<(), Refusal> {
        self.authorize(identity)?;
        self.admit_prepared(original, &prepared.prepared)?;
        if *identity != self.execution.member(index)
            || *identity != prepared.identity
            || !std::ptr::eq(domain, &self.execution.members[index])
        {
            return Err(Refusal);
        }
        Ok(())
    }
    fn invoke(&self, request: ParallelRequest<Invocation<'s>>) -> Result<Input<'s>, Refusal> {
        self.authorize(&request.request.identity)?;
        if request.request.identity.index != request.index {
            return Err(Refusal);
        }
        let branch = self.branches().get(request.index).ok_or(Refusal)?;
        self.admit_prepared(branch, &request.request.prepared)?;
        let (schema, args) = request.request.prepared.into_parts();
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
                identity: request.request.identity,
                value,
            }),
            buffer: vec![17; 9].into_boxed_slice(),
        })
    }
}
impl<'ir, 's> HostResultDomain<Collected<'ir, 's>> for Execution<'ir, 's> {
    type Error = Refusal;
    fn matches_type(&self, reply: &Collected<'ir, 's>) -> bool {
        std::ptr::eq(self.group, reply.group)
    }
    fn validate_value(&self, reply: &Collected<'ir, 's>) -> Result<(), Refusal> {
        let Node::Group { branches, .. } = self.group else {
            return Err(Refusal);
        };
        if !self.identity.0.is_open()
            || !self.matches_type(reply)
            || self.identity != reply.identity
            || reply.rows.len() != branches.len()
        {
            return Err(Refusal);
        }
        for (i, row) in reply.rows.iter().enumerate() {
            let (index, original, identity, domain, input) = row.as_parts();
            if index != i
                || *identity != self.member(i)
                || !std::ptr::eq(original, &branches[i])
                || !std::ptr::eq(domain, &self.members[i])
            {
                return Err(Refusal);
            }
            domain.validate_value(&input.ack)?;
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
        self.authorize(&ack.identity)?;
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
        self.authorize(&group.identity)?;
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
        self.authorize(&ack.identity)?;
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
                assert!(
                    index != 1 || !self.panic_admission.get(),
                    "fixture admission unwind"
                );
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
impl ReplyAuthority<ExecutionIdentity> for Runtime<'_, '_, '_> {
    type Error = Refusal;
    fn authorize(&self, identity: &ExecutionIdentity) -> Result<(), Refusal> {
        if !self.execution.identity.0.is_open() || *identity != self.execution.identity {
            return Err(Refusal);
        }
        for branch in self.branches() {
            self.schema(branch)?;
        }
        Ok(())
    }
}
impl ReplyAuthority<MemberIdentity> for Runtime<'_, '_, '_> {
    type Error = Refusal;
    fn authorize(&self, identity: &MemberIdentity) -> Result<(), Refusal> {
        if !self.execution.identity.0.is_open() || identity.execution != self.execution.identity {
            return Err(Refusal);
        }
        self.schema(self.branches().get(identity.index).ok_or(Refusal)?)
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
        ExecutionIdentity,
        Execution<'ir, 's>,
        Collected<'ir, 's>,
    > for Runtime<'ir, '_, 's>
{
    fn restore_capture(
        &self,
        identity: &ExecutionIdentity,
        declaration: &Execution<'ir, 's>,
        saved: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self.execution, declaration) {
            return Err(Refusal.into());
        }
        self.restores.set(self.restores.get() + 1);
        Ok(saved)
    }
    fn bind_reply(
        &self,
        identity: &ExecutionIdentity,
        declaration: &Execution<'ir, 's>,
        reply: Collected<'ir, 's>,
        fuel: &mut Fuel,
    ) -> Result<PureValue<Self::Result>, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        if !std::ptr::eq(self.execution, declaration) {
            return Err(Refusal.into());
        }
        declaration.validate_value(&reply)?;
        let rows = assemble_group_reply(reply.group, reply.rows, BATCH, fuel, |whole, _| {
            for (i, row) in whole.members().iter().enumerate() {
                let (index, original, identity, domain, input) = row.as_parts();
                if index != i
                    || *identity != self.execution.member(i)
                    || !std::ptr::eq(original, &self.branches()[i])
                    || !std::ptr::eq(domain, &self.execution.members[i])
                {
                    return Err(Refusal);
                }
                domain.validate_value(&input.ack)?;
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
            identity: reply.identity,
            group,
            rows,
        }))))
    }
}
impl<'d, 'ir: 'd, 's> EffectSessionEnvironment<'ir, 'd, Field, Key, Opaque, Declaration<'s>>
    for Runtime<'ir, '_, 's>
{
    type Identity = ExecutionIdentity;
    type Declaration = Execution<'ir, 's>;
    type Reply = Collected<'ir, 's>;
    type Dispatch = Dispatch<'ir, 's>;
    fn correlate_request(
        &self,
        request: Self::Request,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'d, ExecutionIdentity, Execution<'ir, 's>, Self::Dispatch, Refusal> {
        if self.execution.correlated.replace(true) {
            return Err(Refusal.into());
        }
        self.authorize(&self.execution.identity)?;
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
            identity: self.execution.identity(),
            declaration: self.execution,
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
            for expected in 0..runtime.branches().len() {
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
                        .try_accept(&runtime.execution.member(index), input, runtime)
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
                    .try_accept(index, &runtime.execution.member(index), input, runtime)
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
        identity: runtime.execution.identity(),
        group: runtime.group,
        rows,
        buffer: vec![23; 11].into_boxed_slice(),
    }
}

#[test]
fn local_gui_session_closes_on_completion_but_preserves_recoverable_reply_rejection() {
    for kind in ["seq", "all"] {
        fixture(|source, types, _| {
            let node = compile(&tree(kind), source, types);
            let scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session =
                LocalStackedEffectSession::new(scope, &node, Vec::new(), Fuel::new(1000), STACK);
            let lease = session.lease();
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            assert_eq!(session.fuel_remaining(), 948);
            assert!(lease.is_open());
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            let Ok(Err(dispatch)) = session.with_host_work(dispatch, Err::<(), _>) else {
                panic!("native refusal must stay a normal host output")
            };
            assert!(lease.is_open());
            assert_eq!(session.status(), EffectSessionStatus::Awaiting);
            assert_eq!(session.fuel_remaining(), 948);
            let Ok(reply) =
                session.with_host_work(dispatch, |dispatch| collect(&runtime, dispatch))
            else {
                panic!("awaiting host work must be admitted")
            };
            let pointer = reply.buffer.as_ptr();
            let foreign = ExecutionIdentity(LocalExecutionScope::new().lease());
            let reply = session
                .try_accept(&foreign, reply, &runtime)
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            assert!(lease.is_open());
            assert_eq!(session.status(), EffectSessionStatus::Awaiting);
            let mut reply = reply;
            reply.rows.swap(0, 1);
            let mut reply = session
                .try_accept(&execution.identity, reply, &runtime)
                .unwrap_err()
                .reply;
            assert!(lease.is_open());
            assert_eq!(reply.buffer.as_ptr(), pointer);
            reply.rows.swap(0, 1);
            runtime.widget.grants.set(0);
            let reply = session
                .try_accept(&execution.identity, reply, &runtime)
                .unwrap_err()
                .reply;
            assert!(lease.is_open());
            assert_eq!(reply.buffer.as_ptr(), pointer);
            runtime.widget.grants.set(3);
            session
                .try_accept(&execution.identity, reply, &runtime)
                .unwrap();
            assert!(lease.is_open());
            assert_eq!(session.fuel_remaining(), 948);
            assert_eq!(runtime.projections.get(), 0);
            assert!(
                matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
            );
            assert!(!lease.is_open());
            assert_eq!(session.fuel_remaining(), 925);
            assert!(!session.cancel());
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
            ));
            assert_eq!(types.owner.invocations.borrow().len(), 2);
        });
    }
}

#[test]
fn local_gui_session_normal_failure_and_preclosed_scope_never_leave_live_authority() {
    for phase in 0..3 {
        fixture(|source, types, _| {
            let node = compile(&tree("seq"), source, types);
            let scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            if phase == 0 {
                runtime.fail_admission.set(true);
            }
            if phase == 2 {
                assert!(scope.close());
                runtime.panic_admission.set(true);
            }
            let budget = if phase == 1 { 0 } else { 1000 };
            let mut session =
                LocalStackedEffectSession::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
            if phase == 2 {
                let reply = Collected {
                    identity: execution.identity(),
                    group: runtime.group,
                    rows: Vec::new(),
                    buffer: vec![29; 3].into_boxed_slice(),
                };
                let pointer = reply.buffer.as_ptr();
                let rejected = session
                    .try_accept(&execution.identity, reply, &runtime)
                    .unwrap_err();
                assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                assert_eq!(session.fuel_remaining(), budget);
            } else {
                assert!(session.poll(&runtime).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
            }
            assert!(!session.lease().is_open());
            let terminal = session.status();
            let remaining = session.fuel_remaining();
            let Err(original) = session.with_host_work::<_, ()>(17, |_| panic!("terminal work"))
            else {
                panic!()
            };
            assert_eq!(original, 17);
            assert!(!session.cancel());
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
            assert_eq!(session.status(), terminal);
            assert_eq!(session.fuel_remaining(), remaining);
            assert!(types.owner.invocations.borrow().is_empty());
        });
    }
}

#[test]
fn local_gui_session_caught_poll_and_accept_unwind_revoke_a_retained_owner() {
    struct PanickingAuthority;
    impl ReplyAuthority<ExecutionIdentity> for PanickingAuthority {
        type Error = Refusal;
        fn authorize(&self, _: &ExecutionIdentity) -> Result<(), Refusal> {
            panic!("fixture authority unwind");
        }
    }
    for acceptance in [false, true] {
        fixture(|source, types, _| {
            let node = compile(&tree("all"), source, types);
            let scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session =
                LocalStackedEffectSession::new(scope, &node, Vec::new(), Fuel::new(1000), STACK);
            if acceptance {
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let reply = collect(&runtime, dispatch);
                let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        session.try_accept(&execution.identity, reply, &PanickingAuthority)
                    }))
                    .is_err()
                );
                assert_eq!(weak.strong_count(), 0);
                assert_eq!(types.owner.invocations.borrow().len(), 2);
            } else {
                runtime.panic_admission.set(true);
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || session.poll(&runtime)
                    ))
                    .is_err()
                );
                assert!(types.owner.invocations.borrow().is_empty());
            }
            assert!(!session.lease().is_open());
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
            );
            let remaining = session.fuel_remaining();
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::HostUncertain)
            ));
            assert!(!session.cancel());
            assert_eq!(session.fuel_remaining(), remaining);
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
        });
    }
}

#[test]
fn local_gui_session_cancel_and_drop_fence_already_published_native_requests() {
    for cancel in [false, true] {
        fixture(|source, types, _| {
            let node = compile(&tree("all"), source, types);
            let scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session =
                LocalStackedEffectSession::new(scope, &node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(Dispatch::Parallel {
                mut barrier,
                requests,
            }) = session.poll(&runtime).unwrap()
            else {
                panic!()
            };
            if cancel {
                assert!(session.cancel());
                assert!(!session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
            } else {
                drop(session);
            }
            assert!(!execution.identity.0.is_open());
            for request in requests {
                assert!(runtime.invoke(request).is_err());
            }
            assert!(barrier.cancel());
            assert!(types.owner.invocations.borrow().is_empty());
        });
    }
}

#[test]
fn local_gui_host_work_unwind_seals_retained_session_and_blocks_escaped_siblings() {
    for kind in ["seq", "all"] {
        for apply_first in [false, true] {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut session = LocalStackedEffectSession::new(
                    scope,
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                );
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let neighbour_scope = LocalExecutionScope::new();
                let neighbour_execution = Execution::new(&node, neighbour_scope.lease());
                let neighbour_runtime =
                    Runtime::new(&neighbour_execution, source, types.owner.clone());
                let mut neighbour = LocalStackedEffectSession::new(
                    neighbour_scope,
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                );
                let EffectSessionPoll::Request(neighbour_dispatch) =
                    neighbour.poll(&neighbour_runtime).unwrap()
                else {
                    panic!()
                };
                let mut escaped = Vec::new();
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    session.with_host_work::<_, ()>(dispatch, |dispatch| {
                        match dispatch {
                            Dispatch::Sequence(mut sequence) => {
                                let SequencePoll::Request { index, request } =
                                    runtime.publish(&mut sequence).unwrap()
                                else {
                                    panic!()
                                };
                                escaped.push(ParallelRequest { index, request });
                            }
                            Dispatch::Parallel { requests, .. } => escaped = requests,
                        }
                        if apply_first {
                            let input = runtime.invoke(escaped.remove(0)).unwrap();
                            drop(input);
                        }
                        panic!("uncertain native GUI work");
                    })
                }));
                assert!(caught.is_err());
                assert!(!session.lease().is_open());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert_eq!(session.fuel_remaining(), 948);
                assert!(!session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::HostUncertain)
                ));
                for request in escaped {
                    assert!(runtime.invoke(request).is_err());
                }
                assert_eq!(
                    types.owner.invocations.borrow().len(),
                    usize::from(apply_first)
                );
                assert_eq!(runtime.restores.get(), 0);
                assert_eq!(runtime.projections.get(), 0);
                let late = Collected {
                    identity: execution.identity(),
                    group: runtime.group,
                    rows: Vec::new(),
                    buffer: vec![31; 7].into_boxed_slice(),
                };
                let pointer = late.buffer.as_ptr();
                let rejected = session
                    .try_accept(&execution.identity, late, &runtime)
                    .unwrap_err();
                assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                let token = Box::new(13);
                let pointer = &*token as *const i32;
                let Err(token) =
                    session.with_host_work::<_, ()>(token, |_| panic!("must not replay"))
                else {
                    panic!()
                };
                assert_eq!(&*token as *const i32, pointer);
                assert!(neighbour.lease().is_open());
                assert_eq!(neighbour.fuel_remaining(), 948);
                let Ok(reply) = neighbour.with_host_work(neighbour_dispatch, |dispatch| {
                    collect(&neighbour_runtime, dispatch)
                }) else {
                    panic!()
                };
                neighbour
                    .try_accept(&neighbour_execution.identity, reply, &neighbour_runtime)
                    .unwrap();
                assert!(matches!(
                    neighbour.poll(&neighbour_runtime).unwrap(),
                    EffectSessionPoll::Value(_)
                ));
                assert_eq!(neighbour.fuel_remaining(), 925);
                assert_eq!(
                    types.owner.invocations.borrow().len(),
                    usize::from(apply_first) + 2
                );
            });
        }
    }
}

#[test]
fn local_gui_host_work_cannot_run_before_dispatch_after_acceptance_or_after_completion() {
    fixture(|source, types, _| {
        let node = compile(&tree("all"), source, types);
        let scope = LocalExecutionScope::new();
        let execution = Execution::new(&node, scope.lease());
        let runtime = Runtime::new(&execution, source, types.owner.clone());
        let mut session =
            LocalStackedEffectSession::new(scope, &node, Vec::new(), Fuel::new(1000), STACK);
        let token = vec![41; 17].into_boxed_slice();
        let pointer = token.as_ptr();
        let Err(token) =
            session.with_host_work::<_, ()>(token, |_| panic!("initial state is not admitted"))
        else {
            panic!()
        };
        assert_eq!(token.as_ptr(), pointer);
        assert_eq!(session.fuel_remaining(), 1000);
        assert!(session.lease().is_open());
        let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
            panic!()
        };
        let Ok(reply) = session.with_host_work(dispatch, |dispatch| collect(&runtime, dispatch))
        else {
            panic!()
        };
        session
            .try_accept(&execution.identity, reply, &runtime)
            .unwrap();
        let Err(token) =
            session.with_host_work::<_, ()>(token, |_| panic!("accepted state is not awaiting"))
        else {
            panic!()
        };
        assert_eq!(token.as_ptr(), pointer);
        assert!(session.lease().is_open());
        assert_eq!(session.fuel_remaining(), 948);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(_)
        ));
        let Err(token) =
            session.with_host_work::<_, ()>(token, |_| panic!("completed state is sealed"))
        else {
            panic!()
        };
        assert_eq!(token.as_ptr(), pointer);
        assert_eq!(session.fuel_remaining(), 925);
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Completed)
        );
        assert_eq!(types.owner.invocations.borrow().len(), 2);
    });
}

#[test]
fn classified_gui_failure_and_uncertainty_close_without_rollback_or_sibling_revocation() {
    for kind in ["seq", "all"] {
        for uncertain in [false, true] {
            for apply_first in [false, true] {
                fixture(|source, types, _| {
                    let node = compile(&tree(kind), source, types);
                    let scope = LocalExecutionScope::new();
                    let execution = Execution::new(&node, scope.lease());
                    let runtime = Runtime::new(&execution, source, types.owner.clone());
                    let mut session = LocalStackedEffectSession::new(
                        scope,
                        &node,
                        Vec::new(),
                        Fuel::new(1000),
                        STACK,
                    );
                    let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap()
                    else {
                        panic!()
                    };
                    let other_scope = LocalExecutionScope::new();
                    let other_execution = Execution::new(&node, other_scope.lease());
                    let other_runtime = Runtime::new(&other_execution, source, types.owner.clone());
                    let mut other = LocalStackedEffectSession::new(
                        other_scope,
                        &node,
                        Vec::new(),
                        Fuel::new(1000),
                        STACK,
                    );
                    let EffectSessionPoll::Request(other_dispatch) =
                        other.poll(&other_runtime).unwrap()
                    else {
                        panic!()
                    };
                    let mut escaped = Vec::new();
                    let failure = vec![47; 13].into_boxed_slice();
                    let pointer = failure.as_ptr();
                    let Ok(failure) = session.with_host_outcome(dispatch, |dispatch| {
                        match dispatch {
                            Dispatch::Sequence(mut sequence) => {
                                let SequencePoll::Request { index, request } =
                                    runtime.publish(&mut sequence).unwrap()
                                else {
                                    panic!()
                                };
                                escaped.push(ParallelRequest { index, request });
                            }
                            Dispatch::Parallel { requests, .. } => escaped = requests,
                        }
                        if apply_first {
                            drop(runtime.invoke(escaped.remove(0)).unwrap());
                        }
                        if uncertain {
                            HostWorkOutcome::Uncertain(failure)
                        } else {
                            HostWorkOutcome::Failed(failure)
                        }
                    }) else {
                        panic!()
                    };
                    assert_eq!(failure.as_ptr(), pointer);
                    assert!(!session.lease().is_open());
                    let end = if uncertain {
                        EffectSessionEnd::HostUncertain
                    } else {
                        EffectSessionEnd::Failed
                    };
                    assert_eq!(session.status(), EffectSessionStatus::Terminal(end));
                    assert_eq!(session.fuel_remaining(), 948);
                    assert!(!session.cancel());
                    assert!(
                        matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Terminal(actual) if actual == end)
                    );
                    let Err(original) =
                        session.with_host_work::<_, ()>(failure, |_| panic!("terminal dispatch"))
                    else {
                        panic!()
                    };
                    assert_eq!(original.as_ptr(), pointer);
                    for request in escaped {
                        assert!(runtime.invoke(request).is_err());
                    }
                    assert_eq!(
                        types.owner.invocations.borrow().len(),
                        usize::from(apply_first)
                    );
                    assert_eq!(runtime.restores.get(), 0);
                    assert_eq!(runtime.projections.get(), 0);
                    assert_eq!(
                        &*types.owner.text.borrow(),
                        if apply_first { "ready." } else { "" }
                    );
                    assert!(other.lease().is_open());
                    let Ok(reply) = other.with_host_work(other_dispatch, |dispatch| {
                        collect(&other_runtime, dispatch)
                    }) else {
                        panic!()
                    };
                    other
                        .try_accept(&other_execution.identity, reply, &other_runtime)
                        .unwrap();
                    assert!(matches!(
                        other.poll(&other_runtime).unwrap(),
                        EffectSessionPoll::Value(_)
                    ));
                    assert_eq!(other.fuel_remaining(), 925);
                    assert_eq!(
                        types.owner.invocations.borrow().len(),
                        usize::from(apply_first) + 2
                    );
                });
            }
        }
    }
}

#[test]
fn classified_gui_cancellation_hands_back_partial_collectors_after_revocation() {
    for kind in ["seq", "all"] {
        for completed in 0..=2 {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut session = LocalStackedEffectSession::new(
                    scope,
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                );
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let other_scope = LocalExecutionScope::new();
                let other_execution = Execution::new(&node, other_scope.lease());
                let other_runtime = Runtime::new(&other_execution, source, types.owner.clone());
                let mut other = LocalStackedEffectSession::new(
                    other_scope,
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                );
                let EffectSessionPoll::Request(other_dispatch) =
                    other.poll(&other_runtime).unwrap()
                else {
                    panic!()
                };
                let mut receipts = Vec::new();
                let Ok((mut dispatch, rows, escaped)) =
                    session.with_host_outcome(dispatch, |mut dispatch| {
                        let mut rows = Vec::new();
                        let mut escaped = Vec::new();
                        match &mut dispatch {
                            Dispatch::Sequence(sequence) => {
                                for index in 0..2 {
                                    let SequencePoll::Request {
                                        index: actual,
                                        request,
                                    } = runtime.publish(sequence).unwrap()
                                    else {
                                        panic!()
                                    };
                                    assert_eq!(index, actual);
                                    let request = ParallelRequest { index, request };
                                    if index == completed {
                                        escaped.push(request);
                                        break;
                                    }
                                    let input = runtime.invoke(request).unwrap();
                                    receipts.push(Rc::downgrade(&input.ack));
                                    rows.push(
                                        sequence
                                            .try_accept(&execution.member(index), input, &runtime)
                                            .unwrap(),
                                    );
                                }
                            }
                            Dispatch::Parallel { barrier, requests } => {
                                for _ in 0..completed {
                                    let request = requests.remove(0);
                                    let index = request.index;
                                    let input = runtime.invoke(request).unwrap();
                                    receipts.push(Rc::downgrade(&input.ack));
                                    barrier
                                        .try_accept(
                                            index,
                                            &execution.member(index),
                                            input,
                                            &runtime,
                                        )
                                        .unwrap();
                                }
                                escaped = std::mem::take(requests);
                            }
                        }
                        // Keep collectors/results owned until the VM revokes its lease.
                        HostWorkOutcome::Cancelled((dispatch, rows, escaped))
                    })
                else {
                    panic!()
                };
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                );
                assert!(!execution.identity.0.is_open());
                assert!(other.lease().is_open());
                assert_eq!(session.fuel_remaining(), 948);
                assert_eq!(runtime.restores.get(), 0);
                assert_eq!(runtime.projections.get(), 0);
                assert_eq!(receipts.len(), completed);
                assert!(receipts.iter().all(|receipt| receipt.strong_count() == 1));
                assert_eq!(types.owner.invocations.borrow().len(), completed);
                assert_eq!(
                    &*types.owner.text.borrow(),
                    if completed == 0 { "" } else { "ready." }
                );

                // Terminal classification did not silently cancel a host-owned collector.
                match &mut dispatch {
                    Dispatch::Sequence(sequence) => {
                        assert_eq!(rows.len(), completed);
                        if completed == 2 {
                            assert_eq!(
                                sequence.status(),
                                SequenceStatus::Terminal(SequenceEnd::Completed)
                            );
                            assert!(!sequence.cancel());
                        } else {
                            assert_eq!(
                                sequence.status(),
                                SequenceStatus::Awaiting { index: completed }
                            );
                            assert!(sequence.cancel());
                            assert!(matches!(
                                runtime.publish(sequence).unwrap(),
                                SequencePoll::Terminal(SequenceEnd::Cancelled)
                            ));
                        }
                    }
                    Dispatch::Parallel { barrier, .. } => {
                        assert!(rows.is_empty());
                        assert_eq!(
                            barrier.status(),
                            if completed == 2 {
                                ParallelStatus::ReadyResults
                            } else {
                                ParallelStatus::Awaiting {
                                    remaining: 2 - completed,
                                }
                            }
                        );
                        assert!(barrier.cancel());
                        assert!(matches!(
                            barrier.poll().unwrap(),
                            ParallelPoll::Terminal(ParallelEnd::Cancelled)
                        ));
                        assert!(receipts.iter().all(|receipt| receipt.upgrade().is_none()));
                    }
                }
                drop((dispatch, rows));
                assert!(receipts.iter().all(|receipt| receipt.upgrade().is_none()));
                for request in escaped {
                    assert!(runtime.invoke(request).is_err());
                }
                let late = Collected {
                    identity: execution.identity(),
                    group: runtime.group,
                    rows: Vec::new(),
                    buffer: vec![53; 17].into_boxed_slice(),
                };
                let pointer = late.buffer.as_ptr();
                let rejected = session
                    .try_accept(&execution.identity, late, &runtime)
                    .unwrap_err();
                assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                assert!(!session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                assert_eq!(session.fuel_remaining(), 948);
                assert_eq!(types.owner.invocations.borrow().len(), completed);

                let Ok(reply) = other
                    .with_host_work(other_dispatch, |dispatch| collect(&other_runtime, dispatch))
                else {
                    panic!()
                };
                other
                    .try_accept(&other_execution.identity, reply, &other_runtime)
                    .unwrap();
                assert!(matches!(
                    other.poll(&other_runtime).unwrap(),
                    EffectSessionPoll::Value(_)
                ));
                assert_eq!(other.fuel_remaining(), 925);
                assert_eq!(types.owner.invocations.borrow().len(), completed + 2);
            });
        }
    }
}

#[test]
fn guarded_gui_owner_drop_or_unwind_revokes_escaped_work_without_poisoning_reuse() {
    for kind in ["seq", "all"] {
        for unwind in [false, true] {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut guarded = scope.bind(StackedEffectSession::new(
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                ));
                let EffectSessionPoll::Request(dispatch) =
                    guarded.value_mut().poll(&runtime).unwrap()
                else {
                    panic!()
                };
                let escaped = match dispatch {
                    Dispatch::Sequence(mut sequence) => {
                        let SequencePoll::Request { index, request } =
                            runtime.publish(&mut sequence).unwrap()
                        else {
                            panic!()
                        };
                        vec![ParallelRequest { index, request }]
                    }
                    Dispatch::Parallel { requests, .. } => requests,
                };
                if unwind {
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                            let _owned = guarded;
                            panic!("native host boundary failed");
                        }));
                    assert!(result.is_err());
                } else {
                    drop(guarded);
                }
                assert!(!execution.identity.0.is_open());
                for request in escaped {
                    assert!(runtime.invoke(request).is_err());
                }
                assert!(types.owner.invocations.borrow().is_empty());
                let fresh_scope = LocalExecutionScope::new();
                let fresh_execution = Execution::new(&node, fresh_scope.lease());
                assert!(execution.identity != fresh_execution.identity);
                let fresh_runtime = Runtime::new(&fresh_execution, source, types.owner.clone());
                let mut fresh = fresh_scope.bind(StackedEffectSession::new(
                    &node,
                    Vec::new(),
                    Fuel::new(1000),
                    STACK,
                ));
                let EffectSessionPoll::Request(dispatch) =
                    fresh.value_mut().poll(&fresh_runtime).unwrap()
                else {
                    panic!()
                };
                let reply = collect(&fresh_runtime, dispatch);
                fresh
                    .value_mut()
                    .try_accept(&fresh_execution.identity, reply, &fresh_runtime)
                    .unwrap();
                assert!(
                    matches!(fresh.value_mut().poll(&fresh_runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
                );
                assert_eq!(fresh.value().fuel_remaining(), 925);
                assert!(fresh.close());
                assert_eq!(types.owner.invocations.borrow().len(), 2);
            });
        }
    }
}

#[test]
fn guarded_gui_owner_drop_rejects_late_receipt_without_consuming_pending_member() {
    fixture(|source, types, _| {
        let node = compile(&tree("seq"), source, types);
        let scope = LocalExecutionScope::new();
        let execution = Execution::new(&node, scope.lease());
        let runtime = Runtime::new(&execution, source, types.owner.clone());
        let mut guarded = scope.bind(StackedEffectSession::new(
            &node,
            Vec::new(),
            Fuel::new(1000),
            STACK,
        ));
        let EffectSessionPoll::Request(Dispatch::Sequence(mut sequence)) =
            guarded.value_mut().poll(&runtime).unwrap()
        else {
            panic!()
        };
        let SequencePoll::Request { index, request } = runtime.publish(&mut sequence).unwrap()
        else {
            panic!()
        };
        let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
        let pointer = input.buffer.as_ptr();
        let weak = Rc::downgrade(&input.ack);
        drop(guarded);
        assert!(
            execution.members[index]
                .validate_value(input.ack.as_ref())
                .is_err()
        );
        let refused = sequence
            .try_accept(&execution.member(index), input, &runtime)
            .unwrap_err();
        assert_eq!(refused.reply.buffer.as_ptr(), pointer);
        assert_eq!(weak.strong_count(), 1);
        assert!(
            matches!(runtime.publish(&mut sequence).unwrap(), SequencePoll::Awaiting { index: i } if i == index)
        );
        assert!(sequence.cancel());
        drop(refused);
        assert_eq!(weak.strong_count(), 0);
        assert_eq!(runtime.restores.get(), 0);
        assert_eq!(runtime.projections.get(), 0);
        assert_eq!(types.owner.invocations.borrow().len(), 1);
        assert_eq!(&*types.owner.text.borrow(), "ready.");
    });
}

#[test]
fn guarded_gui_explicit_close_fences_accepted_reentry_without_resetting_core_state() {
    for kind in ["seq", "all"] {
        fixture(|source, types, _| {
            let node = compile(&tree(kind), source, types);
            let scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut guarded = scope.bind(StackedEffectSession::new(
                &node,
                Vec::new(),
                Fuel::new(1000),
                STACK,
            ));
            let EffectSessionPoll::Request(dispatch) = guarded.value_mut().poll(&runtime).unwrap()
            else {
                panic!()
            };
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            guarded
                .value_mut()
                .try_accept(&execution.identity, reply, &runtime)
                .unwrap();
            let fuel = guarded.value().fuel_remaining();
            assert!(guarded.close());
            assert!(!guarded.close());
            assert!(guarded.value_mut().poll(&runtime).is_err());
            assert_eq!(
                guarded.value().status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            );
            assert_eq!(guarded.value().fuel_remaining(), fuel);
            assert_eq!(weak.strong_count(), 0);
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(types.owner.invocations.borrow().len(), 2);
        });
    }
}

#[test]
fn scoped_gui_poll_cancellation_blocks_next_publication_and_completed_barrier_handoff() {
    for kind in ["seq", "all"] {
        for completed in 0..=2 {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut session =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let other_scope = LocalExecutionScope::new();
                let other_execution = Execution::new(&node, other_scope.lease());
                let other_runtime = Runtime::new(&other_execution, source, types.owner.clone());
                let mut other =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
                let EffectSessionPoll::Request(other_dispatch) =
                    other.poll(&other_runtime).unwrap()
                else {
                    panic!()
                };
                let mut rows = Vec::new();
                let mut receipts = Vec::new();
                match dispatch {
                    Dispatch::Sequence(mut sequence) => {
                        for index in 0..completed {
                            let SequencePoll::Request { request, .. } =
                                runtime.publish_in_scope(&mut sequence, &scope).unwrap()
                            else {
                                panic!()
                            };
                            let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
                            receipts.push(Rc::downgrade(&input.ack));
                            rows.push(
                                sequence
                                    .try_accept_in_scope(
                                        &execution.member(index),
                                        input,
                                        &runtime,
                                        &scope,
                                    )
                                    .unwrap(),
                            );
                        }
                        scope.close();
                        let expected = if completed == 2 {
                            SequenceEnd::Completed
                        } else {
                            SequenceEnd::Cancelled
                        };
                        assert!(
                            matches!(runtime.publish_in_scope(&mut sequence, &scope).unwrap(), SequencePoll::Terminal(end) if end == expected)
                        );
                        assert!(receipts.iter().all(|ack| ack.strong_count() == 1));
                    }
                    Dispatch::Parallel {
                        mut barrier,
                        mut requests,
                    } => {
                        for index in 0..completed {
                            let input = runtime.invoke(requests.remove(0)).unwrap();
                            receipts.push(Rc::downgrade(&input.ack));
                            barrier
                                .try_accept_in_scope(
                                    index,
                                    &execution.member(index),
                                    input,
                                    &runtime,
                                    &scope,
                                )
                                .unwrap();
                        }
                        if completed == 2 {
                            assert_eq!(barrier.status(), ParallelStatus::ReadyResults);
                        }
                        scope.close();
                        assert!(matches!(
                            barrier.poll_in_scope(&scope).unwrap(),
                            ParallelPoll::Terminal(ParallelEnd::Cancelled)
                        ));
                        assert!(receipts.iter().all(|ack| ack.strong_count() == 0));
                        for request in requests {
                            assert!(runtime.invoke(request).is_err());
                        }
                    }
                }
                assert_eq!(session.status(), EffectSessionStatus::Awaiting);
                assert!(session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                assert_eq!(session.fuel_remaining(), 948);
                assert_eq!((runtime.restores.get(), runtime.projections.get()), (0, 0));
                assert_eq!(types.owner.invocations.borrow().len(), completed);
                drop(rows);
                assert!(receipts.iter().all(|ack| ack.strong_count() == 0));
                assert!(other_scope.is_open());
                other
                    .try_accept(
                        &other_execution.identity,
                        collect(&other_runtime, other_dispatch),
                        &other_runtime,
                    )
                    .unwrap();
                assert!(
                    matches!(other.poll(&other_runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
                );
                assert_eq!(other.fuel_remaining(), 925);
                assert_eq!(types.owner.invocations.borrow().len(), completed + 2);
            });
        }
    }
}

#[test]
fn scoped_gui_collector_revocation_stops_group_delivery_without_cancelling_a_sibling() {
    struct Revoking<'a, Authority> {
        scope: &'a LocalExecutionScope,
        authority: &'a Authority,
    }
    impl<Authority: ReplyAuthority<MemberIdentity>> ReplyAuthority<MemberIdentity>
        for Revoking<'_, Authority>
    {
        type Error = Authority::Error;
        fn authorize(&self, identity: &MemberIdentity) -> Result<(), Self::Error> {
            self.scope.close();
            self.authority.authorize(identity)
        }
    }
    for kind in ["seq", "all"] {
        for completed in 0..2 {
            fixture(|source, types, _| {
                let node = compile(&tree(kind), source, types);
                let scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut session =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let other_scope = LocalExecutionScope::new();
                let other_execution = Execution::new(&node, other_scope.lease());
                let other_runtime = Runtime::new(&other_execution, source, types.owner.clone());
                let mut other =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
                let EffectSessionPoll::Request(other_dispatch) =
                    other.poll(&other_runtime).unwrap()
                else {
                    panic!()
                };
                let authority = Revoking {
                    scope: &scope,
                    authority: &runtime,
                };
                let mut handed_off = Vec::new();
                let mut retained = Vec::new();
                match dispatch {
                    Dispatch::Sequence(mut sequence) => {
                        for index in 0..=completed {
                            let SequencePoll::Request { request, .. } =
                                runtime.publish(&mut sequence).unwrap()
                            else {
                                panic!()
                            };
                            let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
                            retained.push(Rc::downgrade(&input.ack));
                            if index < completed {
                                handed_off.push(
                                    sequence
                                        .try_accept_in_scope(
                                            &execution.member(index),
                                            input,
                                            &runtime,
                                            &scope,
                                        )
                                        .unwrap(),
                                );
                            } else {
                                let pointer = input.buffer.as_ptr();
                                let rejected = sequence
                                    .try_accept_in_scope(
                                        &execution.member(index),
                                        input,
                                        &authority,
                                        &scope,
                                    )
                                    .unwrap_err();
                                assert!(matches!(
                                    rejected.error,
                                    leselang_hir::sequence_evaluation::SequenceReplyError::Reply(
                                        ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                                    )
                                ));
                                assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                            }
                        }
                        assert_eq!(
                            sequence.status(),
                            SequenceStatus::Terminal(SequenceEnd::Cancelled)
                        );
                        assert!(matches!(
                            runtime.publish(&mut sequence).unwrap(),
                            SequencePoll::Terminal(SequenceEnd::Cancelled)
                        ));
                        if completed == 1 {
                            assert_eq!(retained[0].strong_count(), 1);
                        }
                    }
                    Dispatch::Parallel {
                        mut barrier,
                        mut requests,
                    } => {
                        for index in 0..=completed {
                            let input = runtime.invoke(requests.remove(0)).unwrap();
                            retained.push(Rc::downgrade(&input.ack));
                            if index < completed {
                                barrier
                                    .try_accept_in_scope(
                                        index,
                                        &execution.member(index),
                                        input,
                                        &runtime,
                                        &scope,
                                    )
                                    .unwrap();
                            } else {
                                let pointer = input.buffer.as_ptr();
                                let rejected = barrier
                                    .try_accept_in_scope(
                                        index,
                                        &execution.member(index),
                                        input,
                                        &authority,
                                        &scope,
                                    )
                                    .unwrap_err();
                                assert!(matches!(
                                    rejected.error,
                                    leselang_hir::parallel_evaluation::ParallelReplyError::Reply(
                                        ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                                    )
                                ));
                                assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                            }
                        }
                        assert_eq!(
                            barrier.status(),
                            ParallelStatus::Terminal(ParallelEnd::Cancelled)
                        );
                        assert!(matches!(
                            barrier.poll().unwrap(),
                            ParallelPoll::Terminal(ParallelEnd::Cancelled)
                        ));
                        assert!(retained.iter().all(|ack| ack.strong_count() == 0));
                        for request in requests {
                            assert!(runtime.invoke(request).is_err());
                        }
                    }
                }
                assert!(!scope.is_open());
                assert_eq!(session.fuel_remaining(), 948);
                assert_eq!((runtime.restores.get(), runtime.projections.get()), (0, 0));
                // A collector cannot mutate the unwrapped enclosing session.
                assert!(session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                assert_eq!(types.owner.invocations.borrow().len(), completed + 1);
                assert_eq!(&*types.owner.text.borrow(), "ready.");
                drop(handed_off);
                assert!(retained.iter().all(|ack| ack.strong_count() == 0));
                assert!(other_scope.is_open());
                let collected = collect(&other_runtime, other_dispatch);
                other
                    .try_accept(&other_execution.identity, collected, &other_runtime)
                    .unwrap();
                assert!(
                    matches!(other.poll(&other_runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
                );
                assert_eq!(other.fuel_remaining(), 925);
                assert_eq!(types.owner.invocations.borrow().len(), completed + 3);
            });
        }
    }
}

#[test]
fn shared_gui_ir_interleaves_scoped_executions_without_cross_accepting_receipts() {
    fixture(|source, types, _| {
        let node = scoped_node(source, types);
        let left_scope = LocalExecutionScope::new();
        let left = Execution::new(&node, left_scope.lease());
        let right_scope = LocalExecutionScope::new();
        let right = Execution::new(&node, right_scope.lease());
        let a = Runtime::new(&left, source, types.owner.clone());
        let b = Runtime::new(&right, source, types.owner.clone());
        assert!(std::ptr::eq(a.group, b.group));
        assert!(left.identity != right.identity);
        let mut first =
            StackedEffectSession::new(&node, scoped_bindings("alpha", "!"), Fuel::new(1000), STACK);
        let mut second =
            StackedEffectSession::new(&node, scoped_bindings("bravo", "?"), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(Dispatch::Sequence(mut one)) = first.poll(&a).unwrap()
        else {
            panic!()
        };
        let EffectSessionPoll::Request(Dispatch::Sequence(mut two)) = second.poll(&b).unwrap()
        else {
            panic!()
        };
        let prepared = (first.fuel_remaining(), second.fuel_remaining());
        let mut rows_a = Vec::new();
        let mut rows_b = Vec::new();
        let mut earlier_ack = None;
        for index in 0..4 {
            let SequencePoll::Request { index: i, request } = a.publish(&mut one).unwrap() else {
                panic!()
            };
            assert_eq!(i, index);
            let input_a = a.invoke(ParallelRequest { index, request }).unwrap();
            let SequencePoll::Request { index: i, request } = b.publish(&mut two).unwrap() else {
                panic!()
            };
            assert_eq!(i, index);
            let input_b = b.invoke(ParallelRequest { index, request }).unwrap();
            let pointer = input_a.buffer.as_ptr();
            let foreign = two
                .try_accept(&left.member(index), input_a, &b)
                .unwrap_err()
                .reply;
            assert_eq!(foreign.buffer.as_ptr(), pointer);
            // A correct outer label cannot launder a foreign native receipt.
            let foreign = two
                .try_accept(&right.member(index), foreign, &b)
                .unwrap_err()
                .reply;
            assert_eq!(foreign.buffer.as_ptr(), pointer);
            assert!(
                matches!(b.publish(&mut two).unwrap(), SequencePoll::Awaiting { index: i } if i == index)
            );
            if index == 0 {
                earlier_ack = Some(foreign.ack.clone());
            }
            if index == 2 {
                let replay = Input {
                    ack: earlier_ack.take().unwrap(),
                    buffer: vec![31; 3].into_boxed_slice(),
                };
                let pointer = replay.buffer.as_ptr();
                let replay = one
                    .try_accept(&left.member(index), replay, &a)
                    .unwrap_err()
                    .reply;
                assert_eq!(replay.buffer.as_ptr(), pointer);
            }
            rows_b.push(two.try_accept(&right.member(index), input_b, &b).unwrap());
            rows_a.push(one.try_accept(&left.member(index), foreign, &a).unwrap());
        }
        let mut reply_a = Collected {
            identity: left.identity(),
            group: a.group,
            rows: rows_a,
            buffer: vec![41; 5].into_boxed_slice(),
        };
        let mut reply_b = Collected {
            identity: right.identity(),
            group: b.group,
            rows: rows_b,
            buffer: vec![43; 5].into_boxed_slice(),
        };
        let pointer = reply_a.buffer.as_ptr();
        let weak_a = Rc::downgrade(&reply_a.rows[0].as_parts().4.ack);
        let weak_b = Rc::downgrade(&reply_b.rows[0].as_parts().4.ack);
        reply_a = second
            .try_accept(&left.identity, reply_a, &b)
            .unwrap_err()
            .reply;
        reply_a = second
            .try_accept(&right.identity, reply_a, &b)
            .unwrap_err()
            .reply;
        assert_eq!(reply_a.buffer.as_ptr(), pointer);
        std::mem::swap(&mut reply_a.rows[0], &mut reply_b.rows[0]);
        reply_b = second
            .try_accept(&right.identity, reply_b, &b)
            .unwrap_err()
            .reply;
        std::mem::swap(&mut reply_a.rows[0], &mut reply_b.rows[0]);
        assert_eq!((first.fuel_remaining(), second.fuel_remaining()), prepared);
        assert_eq!((a.restores.get(), b.restores.get()), (0, 0));
        first.try_accept(&left.identity, reply_a, &a).unwrap();
        second.try_accept(&right.identity, reply_b, &b).unwrap();
        assert!(
            matches!(second.poll(&b).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "bravo.?")
        );
        assert!(
            matches!(first.poll(&a).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "alpha.!")
        );
        assert_eq!(first.fuel_remaining(), second.fuel_remaining());
        assert_eq!((a.restores.get(), b.restores.get()), (1, 1));
        assert_eq!((a.projections.get(), b.projections.get()), (1, 1));
        assert_eq!((weak_a.strong_count(), weak_b.strong_count()), (0, 0));
        assert_eq!(types.owner.invocations.borrow().len(), 8);
        // The actual widget is shared; receipt isolation does not serialize GUI mutations.
        assert_eq!(&*types.owner.text.borrow(), "bravo.");
    });
}

#[test]
fn shared_gui_parallel_barriers_reject_foreign_receipts_before_consuming_pending_members() {
    fixture(|source, types, _| {
        let node = compile(&tree("all"), source, types);
        let left_scope = LocalExecutionScope::new();
        let left = Execution::new(&node, left_scope.lease());
        let right_scope = LocalExecutionScope::new();
        let right = Execution::new(&node, right_scope.lease());
        let a = Runtime::new(&left, source, types.owner.clone());
        let b = Runtime::new(&right, source, types.owner.clone());
        let mut first = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let mut second = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(Dispatch::Parallel {
            barrier: mut one,
            requests: requests_a,
        }) = first.poll(&a).unwrap()
        else {
            panic!()
        };
        let EffectSessionPoll::Request(Dispatch::Parallel {
            barrier: mut two,
            requests: requests_b,
        }) = second.poll(&b).unwrap()
        else {
            panic!()
        };
        for (request_a, request_b) in requests_a.into_iter().zip(requests_b).rev() {
            let index = request_a.index;
            let input_a = a.invoke(request_a).unwrap();
            let input_b = b.invoke(request_b).unwrap();
            let pointer = input_a.buffer.as_ptr();
            let rejected = two
                .try_accept(index, &left.member(index), input_a, &b)
                .unwrap_err()
                .reply;
            let rejected = two
                .try_accept(index, &right.member(index), rejected, &b)
                .unwrap_err()
                .reply;
            assert_eq!(rejected.buffer.as_ptr(), pointer);
            one.try_accept(index, &left.member(index), rejected, &a)
                .unwrap();
            two.try_accept(index, &right.member(index), input_b, &b)
                .unwrap();
            if index == 1 {
                assert!(matches!(
                    two.poll().unwrap(),
                    ParallelPoll::Awaiting { remaining: 1 }
                ));
            }
        }
        let ParallelPoll::Results(rows_a) = one.poll().unwrap() else {
            panic!()
        };
        let ParallelPoll::Results(rows_b) = two.poll().unwrap() else {
            panic!()
        };
        let reply_a = Collected {
            identity: left.identity(),
            group: a.group,
            rows: rows_a,
            buffer: Box::new([]),
        };
        let reply_b = Collected {
            identity: right.identity(),
            group: b.group,
            rows: rows_b,
            buffer: Box::new([]),
        };
        let reply_a = second
            .try_accept(&right.identity, reply_a, &b)
            .unwrap_err()
            .reply;
        first.try_accept(&left.identity, reply_a, &a).unwrap();
        second.try_accept(&right.identity, reply_b, &b).unwrap();
        assert!(
            matches!(first.poll(&a).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
        );
        assert!(
            matches!(second.poll(&b).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
        );
        assert_eq!(
            (first.fuel_remaining(), second.fuel_remaining()),
            (925, 925)
        );
        assert_eq!(types.owner.invocations.borrow().len(), 4);
    });
}

#[test]
fn gui_execution_cancellation_fences_late_dispatch_without_revoking_other_sessions() {
    for kind in ["seq", "all"] {
        fixture(|source, types, _| {
            let node = compile(&tree(kind), source, types);
            let left_scope = LocalExecutionScope::new();
            let left = Execution::new(&node, left_scope.lease());
            let right_scope = LocalExecutionScope::new();
            let right = Execution::new(&node, right_scope.lease());
            let a = Runtime::new(&left, source, types.owner.clone());
            let b = Runtime::new(&right, source, types.owner.clone());
            let mut first = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let mut second = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch_a) = first.poll(&a).unwrap() else {
                panic!()
            };
            let EffectSessionPoll::Request(dispatch_b) = second.poll(&b).unwrap() else {
                panic!()
            };
            let late = match dispatch_a {
                Dispatch::Sequence(mut sequence) => {
                    let SequencePoll::Request { index, request } =
                        a.publish(&mut sequence).unwrap()
                    else {
                        panic!()
                    };
                    assert!(sequence.cancel());
                    vec![ParallelRequest { index, request }]
                }
                Dispatch::Parallel {
                    mut barrier,
                    requests,
                } => {
                    assert!(barrier.cancel());
                    requests
                }
            };
            // The host must deliver cancellation to native work as well as the core session.
            assert!(left_scope.close());
            assert!(first.cancel());
            for request in late {
                assert!(a.invoke(request).is_err());
            }
            assert!(types.owner.invocations.borrow().is_empty());
            let reply = collect(&b, dispatch_b);
            second.try_accept(&right.identity, reply, &b).unwrap();
            assert!(
                matches!(second.poll(&b).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
            );
            assert!(matches!(
                first.poll(&a).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!((a.restores.get(), a.projections.get()), (0, 0));
            assert_eq!(types.owner.invocations.borrow().len(), 2);
            assert_eq!(types.owner.grants.get(), 3);
        });
    }
}

#[test]
fn gui_execution_domains_are_single_use_and_foreign_dispatch_cannot_invoke() {
    fixture(|source, types, _| {
        let node = compile(&tree("all"), source, types);
        let left_scope = LocalExecutionScope::new();
        let left = Execution::new(&node, left_scope.lease());
        let right_scope = LocalExecutionScope::new();
        let right = Execution::new(&node, right_scope.lease());
        let a = Runtime::new(&left, source, types.owner.clone());
        let b = Runtime::new(&right, source, types.owner.clone());
        let mut first = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(dispatch) = first.poll(&a).unwrap() else {
            panic!()
        };
        let mut alias = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        assert!(alias.poll(&a).is_err());
        assert!(types.owner.invocations.borrow().is_empty());
        let reply = collect(&a, dispatch);
        first.try_accept(&left.identity, reply, &a).unwrap();
        assert!(matches!(
            first.poll(&a).unwrap(),
            EffectSessionPoll::Value(_)
        ));
        let mut replay = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        assert!(replay.poll(&a).is_err());
        let mut fresh = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(Dispatch::Parallel {
            mut barrier,
            requests,
        }) = fresh.poll(&b).unwrap()
        else {
            panic!()
        };
        for request in requests {
            assert!(a.invoke(request).is_err());
        }
        assert!(barrier.cancel());
        assert!(fresh.cancel());
        assert_eq!(types.owner.invocations.borrow().len(), 2);
    });
}

#[test]
fn scoped_gui_expression_reuses_one_compiled_tree_with_fresh_inputs_and_original_reply_sessions() {
    fixture(|source, types, _| {
        let node = scoped_node(source, types);
        let mut budgets = Vec::new();
        for (prefix, suffix, enabled) in [
            ("alpha", "!", true),
            ("bravo", "?", true),
            ("delta", ".", false),
        ] {
            types.owner.invocations.borrow_mut().clear();
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut bindings = scoped_bindings(prefix, suffix);
            bindings[3].1 = PureValue::Scalar(ScalarValue::Boolean(enabled));
            let mut session = StackedEffectSession::new(&node, bindings, Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!();
            };
            let prepared = session.fuel_remaining();
            assert!(runtime.widget.invocations.borrow().is_empty());
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert_eq!(session.fuel_remaining(), prepared);
            let reply = collect(&runtime, dispatch);
            let pointer = reply.buffer.as_ptr();
            let reply = session
                .try_accept(
                    &ExecutionIdentity(LocalExecutionScope::new().lease()),
                    reply,
                    &runtime,
                )
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            let weak = Rc::downgrade(&reply.rows[2].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(session.fuel_remaining(), prepared);
            let expected_text = if enabled {
                format!("{prefix}.")
            } else {
                String::new()
            };
            assert!(
                matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == format!("{expected_text}{suffix}"))
            );
            assert_eq!(&*runtime.widget.text.borrow(), &expected_text);
            assert_eq!(runtime.widget.visible.get(), enabled);
            assert_eq!(
                *runtime.widget.invocations.borrow(),
                [
                    "label.set_text",
                    "widget.set_visible",
                    "label.set_text",
                    "widget.set_visible"
                ]
            );
            assert_eq!(runtime.restores.get(), 1);
            assert_eq!(runtime.projections.get(), 1);
            assert_eq!(weak.strong_count(), 0);
            budgets.push((prepared, session.fuel_remaining()));
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
            ));
        }
        assert_eq!(budgets[0], budgets[1]);
    });
}

#[test]
fn scoped_gui_expression_runtime_inputs_are_not_authorized_by_compile_time_types() {
    for case in 0..5 {
        fixture(|source, types, _| {
            let node = scoped_node(source, types);
            let mut bindings = scoped_bindings("alpha", "!");
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut budget = 1000;
            match case {
                0 => {
                    bindings.remove(1);
                }
                1 => bindings[1].1 = PureValue::Scalar(ScalarValue::Integer(9)),
                2 => bindings[3].1 = PureValue::Scalar(ScalarValue::String("true".to_owned())),
                3 => runtime.widget.grants.set(0),
                _ => budget = 0,
            }
            let mut session = StackedEffectSession::new(&node, bindings, Fuel::new(budget), STACK);
            assert!(session.poll(&runtime).is_err(), "case {case}");
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            );
            assert!(runtime.widget.invocations.borrow().is_empty());
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
        });
    }
}

#[test]
fn scoped_gui_expression_cancellation_and_revocation_fence_captured_external_inputs() {
    for case in 0..3 {
        fixture(|source, types, _| {
            let node = scoped_node(source, types);
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session = StackedEffectSession::new(
                &node,
                scoped_bindings("alpha", "!"),
                Fuel::new(1000),
                STACK,
            );
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!();
            };
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
            if case == 0 {
                assert!(session.cancel());
                assert!(matches!(
                    session.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
            } else {
                if case == 1 {
                    runtime.widget.generation.set(GENERATION + 1);
                } else {
                    runtime.widget.grants.set(0);
                }
                assert!(session.poll(&runtime).is_err());
            }
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(runtime.widget.invocations.borrow().len(), 4);
            assert_eq!(weak.strong_count(), 0);
        });
    }
}

#[test]
fn parsed_gui_helpers_return_heterogeneous_acks_through_seq_all_and_one_enclosing_meter() {
    for kind in ["seq", "all"] {
        fixture(|source, types, _| {
            let node = compile(&tree(kind), source, types);
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
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
                .try_accept(
                    &ExecutionIdentity(LocalExecutionScope::new().lease()),
                    reply,
                    &runtime,
                )
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            reply.rows.swap(0, 1);
            let mut reply = session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap_err()
                .reply;
            assert_eq!(reply.buffer.as_ptr(), pointer);
            reply.rows.swap(0, 1);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
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
fn parsed_gui_repeat_returns_original_ack_with_explicit_sequence_fuel_parity() {
    let mut budgets = Vec::new();
    for group in [
        "repeat(times: 3, body: prepare_text(prefix: prefix))",
        "seq(iteration_1: prepare_text(prefix: prefix), iteration_2: prepare_text(prefix: prefix), iteration_3: prepare_text(prefix: prefix))",
    ] {
        fixture(|source, types, _| {
            let node = compile(&repeat_tree(group), source, types);
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            let prepared = session.fuel_remaining();
            assert!(runtime.widget.invocations.borrow().is_empty());
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[2].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
            assert_eq!(session.fuel_remaining(), prepared);
            assert_eq!(runtime.projections.get(), 0);
            assert!(
                matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
            );
            assert_eq!(*runtime.widget.invocations.borrow(), ["label.set_text"; 3]);
            assert_eq!(&*runtime.widget.text.borrow(), "ready.");
            assert_eq!(weak.strong_count(), 0);
            assert_eq!(runtime.restores.get(), 2);
            assert_eq!(runtime.projections.get(), 1);
            budgets.push((prepared, session.fuel_remaining()));
        });
    }
    assert_eq!(budgets[0], budgets[1]);
    assert!(budgets[0].1 < budgets[0].0 && budgets[0].0 < 1000);
}

#[test]
fn gui_repeat_cancel_generation_or_grant_revocation_fences_remaining_iterations() {
    for phase in 0..3 {
        fixture(|source, types, _| {
            let node = compile(
                &repeat_tree("repeat(times: 3, body: prepare_text(prefix: prefix))"),
                source,
                types,
            );
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
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
            let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
            drop(
                sequence
                    .try_accept(&runtime.execution.member(index), input, &runtime)
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
                    runtime.widget.grants.set(0);
                    assert!(runtime.publish(&mut sequence).is_err());
                }
            }
            assert!(session.cancel());
            assert!(matches!(
                runtime.publish(&mut sequence).unwrap(),
                SequencePoll::Terminal(_)
            ));
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(*runtime.widget.invocations.borrow(), ["label.set_text"]);
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(session.fuel_remaining(), fuel);
        });
    }
}

const COMPOSED_GUI: &str =
    "repeat(times: 2, body: seq(label: prepare_text(prefix: prefix), visible: prepare_visible()))";

#[test]
fn composed_gui_sequences_preserve_heterogeneous_order_projected_reply_and_flat_fuel() {
    let mut budgets = Vec::new();
    for (group, selected) in [
        (COMPOSED_GUI, "iteration_2__label"),
        (
            "seq(first: seq(label: prepare_text(prefix: prefix), visible: prepare_visible()), second: seq(label: prepare_text(prefix: prefix), visible: prepare_visible()))",
            "second__label",
        ),
        (
            "seq(a: prepare_text(prefix: prefix), b: prepare_visible(), c: prepare_text(prefix: prefix), d: prepare_visible())",
            "c",
        ),
    ] {
        fixture(|source, types, _| {
            let node = compile(&sequential_tree(group, selected), source, types);
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            let prepared = session.fuel_remaining();
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[2].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
            assert_eq!(session.fuel_remaining(), prepared);
            assert!(
                matches!(session.poll(&runtime).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready.!")
            );
            assert_eq!(
                *runtime.widget.invocations.borrow(),
                [
                    "label.set_text",
                    "widget.set_visible",
                    "label.set_text",
                    "widget.set_visible"
                ]
            );
            assert!(runtime.widget.visible.get());
            assert_eq!(weak.strong_count(), 0);
            assert_eq!(runtime.restores.get(), 2);
            assert_eq!(runtime.projections.get(), 1);
            budgets.push((prepared, session.fuel_remaining()));
        });
    }
    assert!(budgets.iter().all(|budget| *budget == budgets[0]));
    assert!(budgets[0].1 < budgets[0].0 && budgets[0].0 < 1000);
}

#[test]
fn composed_gui_cross_iteration_cancel_or_revocation_never_publishes_remaining_member() {
    for phase in 0..3 {
        fixture(|source, types, _| {
            let node = compile(
                &sequential_tree(COMPOSED_GUI, "iteration_2__label"),
                source,
                types,
            );
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(Dispatch::Sequence(mut sequence)) =
                session.poll(&runtime).unwrap()
            else {
                panic!()
            };
            for expected in 0..3 {
                let SequencePoll::Request { index, request } =
                    runtime.publish(&mut sequence).unwrap()
                else {
                    panic!()
                };
                assert_eq!(index, expected);
                let input = runtime.invoke(ParallelRequest { index, request }).unwrap();
                let pointer = input.buffer.as_ptr();
                let refused = sequence
                    .try_accept(&runtime.execution.member((index + 1) % 4), input, &runtime)
                    .unwrap_err();
                assert_eq!(refused.reply.buffer.as_ptr(), pointer);
                drop(
                    sequence
                        .try_accept(&runtime.execution.member(index), refused.reply, &runtime)
                        .unwrap(),
                );
            }
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
                    runtime.widget.grants.set(0);
                    assert!(runtime.publish(&mut sequence).is_err());
                }
            }
            assert!(session.cancel());
            assert!(matches!(
                runtime.publish(&mut sequence).unwrap(),
                SequencePoll::Terminal(_)
            ));
            assert!(matches!(
                session.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(
                *runtime.widget.invocations.borrow(),
                ["label.set_text", "widget.set_visible", "label.set_text"]
            );
            assert_eq!(runtime.restores.get(), 0);
            assert_eq!(runtime.projections.get(), 0);
            assert_eq!(session.fuel_remaining(), fuel);
        });
    }
}

#[test]
fn gui_member_reply_rejections_preserve_original_payload_and_pending_sequence() {
    fixture(|source, types, foreign| {
        let node = compile(&tree("seq"), source, types);
        let execution_scope = LocalExecutionScope::new();
        let execution = Execution::new(&node, execution_scope.lease());
        let runtime = Runtime::new(&execution, source, types.owner.clone());
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
                    identity: runtime.execution.member(index),
                    value,
                }),
                buffer: vec![19; 7].into_boxed_slice(),
            };
            let pointer = input.buffer.as_ptr();
            let weak = Rc::downgrade(&input.ack);
            let rejected = sequence
                .try_accept(&runtime.execution.member(index), input, &runtime)
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
            .try_accept(
                &MemberIdentity {
                    execution: ExecutionIdentity(LocalExecutionScope::new().lease()),
                    index,
                },
                valid,
                &runtime,
            )
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
        drop(
            sequence
                .try_accept(&runtime.execution.member(index), rejected.reply, &runtime)
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
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
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
                        &runtime.execution.member(index),
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
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
            let mut session = StackedEffectSession::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                panic!()
            };
            let reply = collect(&runtime, dispatch);
            let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
            session
                .try_accept(&runtime.execution.identity, reply, &runtime)
                .unwrap();
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
            let execution_scope = LocalExecutionScope::new();
            let execution = Execution::new(&node, execution_scope.lease());
            let runtime = Runtime::new(&execution, source, types.owner.clone());
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
                let execution_scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, execution_scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
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
        let execution_scope = LocalExecutionScope::new();
        let execution = Execution::new(&node, execution_scope.lease());
        let runtime = Runtime::new(&execution, source, types.owner.clone());
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
                let execution_scope = LocalExecutionScope::new();
                let execution = Execution::new(&node, execution_scope.lease());
                let runtime = Runtime::new(&execution, source, types.owner.clone());
                let mut session =
                    StackedEffectSession::new(&node, Vec::new(), Fuel::new(fuel), STACK);
                let EffectSessionPoll::Request(dispatch) = session.poll(&runtime).unwrap() else {
                    panic!()
                };
                let reply = collect(&runtime, dispatch);
                let weak = Rc::downgrade(&reply.rows[0].as_parts().4.ack);
                session
                    .try_accept(&runtime.execution.identity, reply, &runtime)
                    .unwrap();
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
