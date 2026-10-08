use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_typing::CallTypeLimits;
use leselang_hir::flow_typing::*;
use leselang_hir::ir::{Computation, ComputedBranch, GroupKind};
use leselang_hir::prepared_typing::{PreparedCallSchemas, SelectedPreparedCall};
use leselang_hir::pure_typing::{PureType, PureTypeEnvironment, TypeInferenceLimits};
use leselang_runtime_core::{
    OperationCatalogError, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
};

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<&'static str>>,
    hosts: RefCell<Vec<usize>>,
    members: RefCell<Vec<(usize, usize, usize)>>,
    clones: Cell<usize>,
    panic_at: Cell<Option<&'static str>>,
}
impl Trace {
    fn record(&self, event: &'static str) {
        self.events.borrow_mut().push(event);
        assert_ne!(self.panic_at.get(), Some(event), "private native graph");
    }
}

// All native slots are move-only, GUI-local and have no Debug/serde/PartialEq.
struct Declaration {
    kind: u8,
    owner: Rc<Trace>,
}
type Row<'s> = OperationSchema<'s, u32, &'s str, ScalarTypeSet, Declaration, u8>;
struct Operation<'s> {
    row: &'s Row<'s>,
    owner: Rc<Trace>,
}
struct NativeBranch<'s> {
    name: String,
    effect: Graph<'s>,
    declaration: &'s Declaration,
}
enum Graph<'s> {
    Atomic {
        operation: Operation<'s>,
        declaration: &'s Declaration,
    },
    Group {
        kind: GroupKind,
        members: Vec<NativeBranch<'s>>,
    },
}
type Ir<'s> = Computation<u8, Operation<'s>, Graph<'s>, ()>;

struct Member<'e, 's> {
    name: &'e str,
    operation: &'e Operation<'s>,
    declaration: &'s Declaration,
}
#[derive(Clone)]
enum Query<'e, 's> {
    Atomic(&'s Declaration),
    Group {
        kind: GroupKind,
        members: Rc<[Member<'e, 's>]>,
    },
}
struct Metadata<'e, 's> {
    query: Query<'e, 's>,
    trace: Rc<Trace>,
}
impl Clone for Metadata<'_, '_> {
    fn clone(&self) -> Self {
        self.trace.clones.set(self.trace.clones.get() + 1);
        Self {
            query: self.query.clone(),
            trace: self.trace.clone(),
        }
    }
}
impl PartialEq for Metadata<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.query, &other.query) {
            (Query::Atomic(left), Query::Atomic(right)) => std::ptr::eq(*left, *right),
            (
                Query::Group {
                    kind: left_kind,
                    members: left,
                },
                Query::Group {
                    kind: right_kind,
                    members: right,
                },
            ) => leselang_hir::native_group_signature::compare_native_group_signatures(
                (*left_kind, left.as_ref()),
                (*right_kind, right.as_ref()),
                4,
                |member| -> Result<_, ()> { Ok(member.name) },
                |member| Ok(member.name),
                |left, right| {
                    Ok(std::ptr::eq(left.operation.row, right.operation.row)
                        && std::ptr::eq(left.declaration, right.declaration))
                },
            )
            .unwrap_or(false),
            _ => false,
        }
    }
}

struct Gui<'e, 's> {
    rows: &'s [Row<'s>],
    trace: Rc<Trace>,
    version: Cell<u32>,
    granted: Cell<bool>,
    max_native_nodes: Cell<usize>,
    marker: PhantomData<&'e ()>,
}
impl<'e, 's> Gui<'e, 's> {
    fn new(rows: &'s [Row<'s>], trace: Rc<Trace>) -> Self {
        Self {
            rows,
            trace,
            version: Cell::new(9),
            granted: Cell::new(true),
            max_native_nodes: Cell::new(5),
            marker: PhantomData,
        }
    }
    fn metadata(&self, query: Query<'e, 's>) -> Metadata<'e, 's> {
        Metadata {
            query,
            trace: self.trace.clone(),
        }
    }
    fn operation(&self, index: usize) -> Operation<'s> {
        Operation {
            row: &self.rows[index],
            owner: self.trace.clone(),
        }
    }
    fn atomic(&self, index: usize) -> Graph<'s> {
        Graph::Atomic {
            operation: self.operation(index),
            declaration: &self.rows[index].result,
        }
    }
    fn group(&self, kind: GroupKind, members: &[(&str, usize)]) -> Ir<'s> {
        Ir::Host {
            effect: Box::new(Graph::Group {
                kind,
                members: members
                    .iter()
                    .map(|(name, index)| NativeBranch {
                        name: (*name).into(),
                        effect: self.atomic(*index),
                        declaration: &self.rows[*index].result,
                    })
                    .collect(),
            }),
        }
    }
    fn call(&self, index: usize) -> Ir<'s> {
        Ir::Call {
            operation: self.operation(index),
            arguments: vec![],
        }
    }
    fn atomic_valid(&self, graph: &Graph<'s>) -> bool {
        let Graph::Atomic {
            operation,
            declaration,
        } = graph
        else {
            return false;
        };
        self.rows.iter().any(|row| std::ptr::eq(row, operation.row))
            && Rc::ptr_eq(&operation.owner, &self.trace)
            && Rc::ptr_eq(&declaration.owner, &self.trace)
            && std::ptr::eq(*declaration, &operation.row.result)
    }
}
impl<'e, 's: 'e> PureTypeEnvironment<u8, Operation<'s>> for Gui<'e, 's> {
    type Result = Metadata<'e, 's>;
    fn field_type(&self, result: &Self::Result, field: &u8) -> Option<ScalarType> {
        self.trace.record("field");
        matches!(result.query, Query::Atomic(declaration) if declaration.kind == 1 && *field == 7)
            .then_some(ScalarType::Integer)
    }
    fn member_result(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &Operation<'s>,
    ) -> Option<Self::Result> {
        self.trace.record("member");
        let Query::Group { kind, members } = &group.query else {
            return None;
        };
        let member = leselang_hir::native_group_lookup::lookup_native_group_member(
            *kind,
            members.as_ref(),
            4,
            name,
            |member| -> Result<_, ()> { Ok(member.name) },
            |member| Ok(std::ptr::eq(member.operation.row, operation.row)),
        )
        .ok()??;
        self.trace.members.borrow_mut().push((
            member.name.as_ptr() as usize,
            std::ptr::from_ref(member.operation) as usize,
            std::ptr::from_ref(member.declaration) as usize,
        ));
        Some(self.metadata(Query::Atomic(member.declaration)))
    }
    fn join_results(&self, left: Self::Result, right: Self::Result) -> Option<Self::Result> {
        self.trace.record("join");
        (left == right).then_some(left)
    }
}
impl<'e, 's> PreparedCallSchemas<'s, Operation<'s>> for Gui<'e, 's> {
    type Key = u32;
    type Domain = ScalarTypeSet;
    type Result = Declaration;
    type Capability = u8;
    fn select(
        &self,
        operation: &Operation<'s>,
    ) -> Result<SelectedPreparedCall<'s, Operation<'s>, Self>, OperationCatalogError> {
        self.trace.record("schema");
        if self.version.get() != 9 {
            return Err(OperationCatalogError::UnsupportedVersion);
        }
        if !self.granted.get() {
            return Err(OperationCatalogError::CapabilityDenied);
        }
        self.rows
            .iter()
            .find(|row| {
                std::ptr::eq(*row, operation.row) && Rc::ptr_eq(&operation.owner, &self.trace)
            })
            .ok_or(OperationCatalogError::UnknownOperation)
    }
}
impl<'e, 's: 'e> CallFlowEnvironment<'s, u8, Operation<'s>, Declaration> for Gui<'e, 's> {
    fn call_result_type(
        &self,
        _: &Operation<'s>,
        declaration: &'s Declaration,
    ) -> Option<PureType<Self::Result>> {
        self.trace.record("call_type");
        Some(PureType::Result(self.metadata(Query::Atomic(declaration))))
    }
}
impl<'e, 's: 'e> HostFlowEnvironment<'e, 's, u8, Operation<'s>, Declaration, Graph<'s>>
    for Gui<'e, 's>
{
    fn admit_host(&self, graph: &'e Graph<'s>) -> bool {
        self.trace.record("admit");
        self.trace
            .hosts
            .borrow_mut()
            .push(std::ptr::from_ref(graph) as usize);
        if self.version.get() != 9 || !self.granted.get() {
            return false;
        }
        match graph {
            Graph::Atomic { .. } => self.max_native_nodes.get() >= 1 && self.atomic_valid(graph),
            Graph::Group { kind, members } => {
                members.len() < self.max_native_nodes.get()
                    && leselang_hir::native_group::admit_native_group_members(
                        *kind,
                        members,
                        4,
                        |member| Ok(member.name.as_str()),
                        |member| {
                            if self.atomic_valid(&member.effect)
                                && matches!(&member.effect, Graph::Atomic { declaration, .. } if std::ptr::eq(*declaration, member.declaration))
                            {
                                Ok(())
                            } else {
                                Err(())
                            }
                        },
                    ).is_ok()
            }
        }
    }
    fn host_result_type(&self, graph: &'e Graph<'s>) -> Option<PureType<Self::Result>> {
        self.trace.record("host_type");
        let query = match graph {
            Graph::Atomic { declaration, .. } => Query::Atomic(declaration),
            Graph::Group { kind, members } => {
                let exports = leselang_hir::native_group_exports::observe_native_group_exports(
                    *kind,
                    members,
                    4,
                    |member| Ok(member.name.as_str()),
                    |member| {
                        let Graph::Atomic { operation, .. } = &member.effect else {
                            return Err(());
                        };
                        Ok(Member {
                            name: &member.name,
                            operation,
                            declaration: member.declaration,
                        })
                    },
                    |candidate_kind, original, candidate| {
                        if candidate_kind == *kind
                            && self.version.get() == 9
                            && self.granted.get()
                            && original.len() < self.max_native_nodes.get()
                            && original.iter().zip(candidate).all(|(member, observed)| {
                                let Graph::Atomic { operation, .. } = &member.effect else {
                                    return false;
                                };
                                self.atomic_valid(&member.effect)
                                    && std::ptr::eq(member.name.as_str(), observed.name)
                                    && std::ptr::eq(member.name.as_str(), observed.operation.name)
                                    && std::ptr::eq(operation, observed.operation.operation)
                                    && std::ptr::eq(
                                        member.declaration,
                                        observed.operation.declaration,
                                    )
                                    && std::ptr::eq(member.declaration, &operation.row.result)
                            })
                        {
                            Ok(())
                        } else {
                            Err(())
                        }
                    },
                )
                .ok()?;
                Query::Group {
                    kind: exports.kind,
                    members: exports
                        .members
                        .into_iter()
                        .map(|member| member.operation)
                        .collect(),
                }
            }
        };
        Some(PureType::Result(self.metadata(query)))
    }
}
impl<'e, 's: 'e> GroupFlowEnvironment<'e, 's, u8, Operation<'s>, Declaration, ()> for Gui<'e, 's> {
    fn group_branch_type_matches(&self, _: &(), inferred: &PureType<Self::Result>) -> bool {
        matches!(
            inferred,
            PureType::Result(Metadata {
                query: Query::Atomic(_),
                ..
            })
        )
    }
    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'e, Operation<'s>, Self::Result>],
    ) -> Option<Self::Result> {
        let members = members
            .iter()
            .map(|member| {
                let PureType::Result(Metadata {
                    query: Query::Atomic(declaration),
                    ..
                }) = &member.inferred
                else {
                    return None;
                };
                std::ptr::eq(*declaration, &member.operation.row.result).then_some(Member {
                    name: member.name,
                    operation: member.operation,
                    declaration,
                })
            })
            .collect::<Option<Rc<[_]>>>()?;
        Some(self.metadata(Query::Group { kind, members }))
    }
}
impl<'e, 's: 'e> HostGroupFlowEnvironment<'e, 's, u8, Operation<'s>, Declaration, Graph<'s>, ()>
    for Gui<'e, 's>
{
    fn host_group_operation(&self, graph: &'e Graph<'s>) -> Option<&'e Operation<'s>> {
        let Graph::Atomic { operation, .. } = graph else {
            return None;
        };
        Some(operation)
    }
    fn admit_host_group_operation(
        &self,
        graph: &'e Graph<'s>,
        operation: &'e Operation<'s>,
        declaration: &'s Declaration,
    ) -> bool {
        matches!(graph, Graph::Atomic { operation: original, declaration: original_declaration } if std::ptr::eq(original, operation) && std::ptr::eq(*original_declaration, declaration))
    }
}

const LIMITS: HostGroupFlowTypeLimits = HostGroupFlowTypeLimits {
    group: GroupFlowTypeLimits {
        flow: CallFlowTypeLimits {
            call: CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 128,
                    max_depth: 16,
                    max_bindings: 16,
                },
                max_arguments: 1,
            },
            max_calls: 16,
        },
        max_groups: 16,
        max_branches: 4,
    },
    max_hosts: 16,
};
fn rows(trace: &Rc<Trace>) -> [Row<'static>; 3] {
    [17, 18, 19].map(|key| OperationSchema {
        key,
        parameters: &[],
        result: Declaration {
            kind: 1,
            owner: trace.clone(),
        },
        required_capability: 31,
    })
}
fn boolean<'s>() -> Ir<'s> {
    Ir::Literal {
        value: ScalarValue::Boolean(false),
    }
}
fn choose<'s>(then: Ir<'s>, otherwise: Ir<'s>) -> Ir<'s> {
    Ir::Choose {
        when: Box::new(boolean()),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn projection<'e, 's>(gui: &Gui<'e, 's>, value: Ir<'s>, name: &str, index: usize) -> Ir<'s> {
    Ir::Bind {
        name: "group".into(),
        value: Box::new(value),
        body: Box::new(Ir::Field {
            value: Box::new(Ir::Member {
                group: "group".into(),
                name: name.into(),
                operation: gui.operation(index),
            }),
            field: 7,
        }),
    }
}
fn infer<'e, 's: 'e>(
    gui: &Gui<'e, 's>,
    value: &'e Ir<'s>,
    prefix: &[(&'e str, PureType<Metadata<'e, 's>>)],
) -> Result<PureType<Metadata<'e, 's>>, CallFlowTypeError> {
    infer_host_group_flow_type(value, prefix, gui, gui, LIMITS)
}
fn assert_integer(result: Result<PureType<Metadata<'_, '_>>, CallFlowTypeError>) {
    assert!(matches!(result, Ok(PureType::Scalar(ScalarType::Integer))));
}

#[test]
fn independent_gui_graph_exports_borrow_original_names_operations_and_nonclone_declarations() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let value = projection(
        &gui,
        gui.group(GroupKind::Parallel, &[("move", 0), ("read", 1)]),
        "move",
        0,
    );
    let Ir::Bind { value: native, .. } = &value else {
        panic!()
    };
    let Ir::Host { effect } = native.as_ref() else {
        panic!()
    };
    let Graph::Group { members, .. } = effect.as_ref() else {
        panic!()
    };
    let Graph::Atomic { operation, .. } = &members[0].effect else {
        panic!()
    };
    assert_integer(infer(&gui, &value, &[]));
    assert_eq!(
        *trace.hosts.borrow(),
        [std::ptr::from_ref(effect.as_ref()) as usize]
    );
    assert_eq!(
        *trace.members.borrow(),
        [(
            members[0].name.as_ptr() as usize,
            std::ptr::from_ref(operation) as usize,
            std::ptr::from_ref(&rows[0].result) as usize
        )]
    );
}

#[test]
fn native_and_computed_gui_groups_join_only_the_same_original_closed_signature() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace);
    for kind in [GroupKind::Sequence, GroupKind::Parallel] {
        let computed = Ir::Group {
            group_kind: kind,
            branches: [("move", 0), ("read", 1)]
                .map(|(name, index)| ComputedBranch {
                    name: name.into(),
                    value: gui.call(index),
                    result_type: (),
                })
                .into(),
        };
        let value = projection(
            &gui,
            choose(gui.group(kind, &[("move", 0), ("read", 1)]), computed),
            "move",
            0,
        );
        assert_integer(infer(&gui, &value, &[]));
    }
    for (kind, members) in [
        (GroupKind::Parallel, vec![("move", 0), ("read", 1)]),
        (GroupKind::Sequence, vec![("read", 1), ("move", 0)]),
        (GroupKind::Sequence, vec![("private", 0), ("read", 1)]),
        (GroupKind::Sequence, vec![("move", 2), ("read", 1)]),
    ] {
        let value = projection(
            &gui,
            choose(
                gui.group(GroupKind::Sequence, &[("move", 0), ("read", 1)]),
                gui.group(kind, &members),
            ),
            "move",
            0,
        );
        assert!(matches!(
            infer(&gui, &value, &[]),
            Err(CallFlowTypeError::BranchTypes)
        ));
    }
}

#[test]
fn opaque_native_groups_use_the_existing_host_only_entry_not_a_new_graph_abi() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let value = projection(
        &gui,
        gui.group(GroupKind::Sequence, &[("move", 0)]),
        "move",
        0,
    );
    assert_integer(infer_host_flow_type(
        &value,
        &[],
        &gui,
        &gui,
        HostFlowTypeLimits {
            flow: LIMITS.group.flow,
            max_hosts: 1,
        },
    ));
    trace.events.borrow_mut().clear();
    assert!(matches!(
        infer_call_flow_type(&value, &[], &gui, &gui, LIMITS.group.flow),
        Err(CallFlowTypeError::UnsupportedFlow)
    ));
    assert!(matches!(
        infer_group_flow_type(&value, &[], &gui, &gui, LIMITS.group),
        Err(CallFlowTypeError::UnsupportedFlow)
    ));
    assert!(trace.events.borrow().is_empty());
}

#[test]
fn every_cold_native_graph_admission_precedes_prefix_clones_and_member_queries() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let mut invalid = gui.group(GroupKind::Sequence, &[("move", 0)]);
    let Ir::Host { effect } = &mut invalid else {
        panic!()
    };
    let Graph::Group { members, .. } = effect.as_mut() else {
        panic!()
    };
    members[0].declaration = &rows[2].result;
    let value = projection(
        &gui,
        choose(gui.group(GroupKind::Sequence, &[("move", 0)]), invalid),
        "move",
        0,
    );
    let prefix = [(
        "parent",
        PureType::Result(gui.metadata(Query::Atomic(&rows[0].result))),
    )];
    assert!(matches!(
        infer(&gui, &value, &prefix),
        Err(CallFlowTypeError::HostAdmission { host_index: 1 })
    ));
    assert_eq!(trace.clones.get(), 0);
    assert_eq!(*trace.events.borrow(), ["admit", "admit"]);
}

#[test]
fn host_language_limits_do_not_replace_inclusive_private_graph_limits_or_live_policy() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let value = gui.group(
        GroupKind::Sequence,
        &[("a", 0), ("b", 1), ("c", 0), ("d", 1)],
    );
    let mut limits = LIMITS;
    limits.group.flow.call.pure.max_nodes = 1;
    limits.group.flow.call.pure.max_depth = 0;
    limits.group.max_groups = 0;
    limits.group.max_branches = 0;
    limits.max_hosts = 1;
    assert!(infer_host_group_flow_type(&value, &[], &gui, &gui, limits).is_ok());
    gui.max_native_nodes.set(4);
    assert!(matches!(
        infer_host_group_flow_type(&value, &[], &gui, &gui, limits),
        Err(CallFlowTypeError::HostAdmission { host_index: 0 })
    ));
    gui.max_native_nodes.set(5);
    gui.version.set(10);
    assert!(matches!(
        infer_host_group_flow_type(&value, &[], &gui, &gui, limits),
        Err(CallFlowTypeError::HostAdmission { .. })
    ));
    gui.version.set(9);
    gui.granted.set(false);
    assert!(matches!(
        infer_host_group_flow_type(&value, &[], &gui, &gui, limits),
        Err(CallFlowTypeError::HostAdmission { .. })
    ));
    gui.granted.set(true);
    trace.events.borrow_mut().clear();
    limits.max_hosts = 0;
    assert!(matches!(
        infer_host_group_flow_type(&value, &[], &gui, &gui, limits),
        Err(CallFlowTypeError::HostLimit)
    ));
    assert!(trace.events.borrow().is_empty());
}

#[test]
fn changed_native_graph_metadata_needs_fresh_admission_without_a_cached_certificate() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let mut value = gui.group(GroupKind::Sequence, &[("move", 0)]);
    assert!(infer(&gui, &value, &[]).is_ok());
    let Ir::Host { effect } = &mut value else {
        panic!()
    };
    let Graph::Group { members, .. } = effect.as_mut() else {
        panic!()
    };
    members[0].name = "bad name".into();
    let error = infer(&gui, &value, &[]).err().unwrap();
    assert!(matches!(error, CallFlowTypeError::HostAdmission { .. }));
    assert!(!format!("{error:?} {error}").contains("bad name"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(
        trace
            .events
            .borrow()
            .iter()
            .filter(|event| **event == "admit")
            .count(),
        2
    );
}

#[test]
fn nested_opaque_graphs_are_not_accepted_as_atomic_computed_members() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace);
    let value = Ir::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "nested".into(),
            value: gui.group(GroupKind::Sequence, &[("move", 0)]),
            result_type: (),
        }],
    };
    assert!(matches!(
        infer(&gui, &value, &[]),
        Err(CallFlowTypeError::Group {
            error: GroupTypeError::MissingOperation { .. },
            ..
        })
    ));
    let mut value = gui.group(GroupKind::Sequence, &[("nested", 0)]);
    let Ir::Host { effect } = &mut value else {
        panic!()
    };
    let Graph::Group { members, .. } = effect.as_mut() else {
        panic!()
    };
    let Ir::Host { effect: nested } = gui.group(GroupKind::Sequence, &[("move", 0)]) else {
        panic!()
    };
    members[0].effect = *nested;
    assert!(matches!(
        infer(&gui, &value, &[]),
        Err(CallFlowTypeError::HostAdmission { .. })
    ));
}

#[test]
fn native_graph_hook_unwind_does_not_retry_publish_or_mutate_the_borrowed_prefix() {
    for phase in ["admit", "host_type", "member", "field", "join"] {
        let trace = Rc::new(Trace::default());
        let rows = rows(&trace);
        let gui = Gui::new(&rows, trace.clone());
        let value = projection(
            &gui,
            choose(
                gui.group(GroupKind::Sequence, &[("move", 0)]),
                gui.group(GroupKind::Sequence, &[("move", 0)]),
            ),
            "move",
            0,
        );
        let prefix = [(
            "parent",
            PureType::Result(gui.metadata(Query::Atomic(&rows[0].result))),
        )];
        trace.panic_at.set(Some(phase));
        assert!(catch_unwind(AssertUnwindSafe(|| infer(&gui, &value, &prefix))).is_err());
        assert_eq!(prefix.len(), 1);
        assert_eq!(prefix[0].0, "parent");
        assert_eq!(
            trace
                .events
                .borrow()
                .iter()
                .filter(|event| **event == phase)
                .count(),
            1
        );
        trace.panic_at.set(None);
        assert_integer(infer(&gui, &value, &prefix));
    }
}

#[test]
fn call_policy_and_impure_cold_guards_precede_all_opaque_graph_queries() {
    let trace = Rc::new(Trace::default());
    let rows = rows(&trace);
    let gui = Gui::new(&rows, trace.clone());
    let value = choose(gui.group(GroupKind::Sequence, &[("move", 0)]), gui.call(0));
    gui.granted.set(false);
    assert!(matches!(
        infer(&gui, &value, &[]),
        Err(CallFlowTypeError::Call { .. })
    ));
    assert_eq!(*trace.events.borrow(), ["schema"]);
    gui.granted.set(true);
    trace.events.borrow_mut().clear();
    let value = Ir::Choose {
        when: Box::new(gui.group(GroupKind::Sequence, &[("move", 0)])),
        then: Box::new(boolean()),
        otherwise: Box::new(boolean()),
    };
    assert!(infer(&gui, &value, &[]).is_err());
    assert!(trace.events.borrow().is_empty());
}
