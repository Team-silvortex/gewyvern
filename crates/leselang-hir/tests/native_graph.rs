use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::GroupKind;
use leselang_hir::native_graph::*;
use leselang_runtime_core::StructureError;

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<(&'static str, usize)>>,
    fail: Cell<Option<&'static str>>,
    panic: Cell<Option<&'static str>>,
    granted: Cell<bool>,
}
struct PrivateError(String);
struct NativeSlot {
    _owner: Rc<Trace>,
}
struct Branch {
    name: String,
    node: Box<Node>,
    _declaration: NativeSlot,
}
enum Node {
    Leaf {
        payload: Vec<u8>,
        _owner: Rc<Trace>,
    },
    Group {
        kind: GroupKind,
        members: Vec<Branch>,
        _owner: Rc<Trace>,
    },
}
impl Trace {
    fn record(&self, phase: &'static str, address: usize) -> Result<(), PrivateError> {
        self.events.borrow_mut().push((phase, address));
        assert_ne!(self.panic.get(), Some(phase), "private native callback");
        if self.fail.get() == Some(phase) {
            Err(PrivateError("private token".into()))
        } else {
            Ok(())
        }
    }
}
const LIMITS: NativeGraphLimits = NativeGraphLimits {
    max_nodes: 128,
    max_depth: 16,
    max_members: 64,
};
fn leaf(trace: &Rc<Trace>) -> Node {
    Node::Leaf {
        payload: vec![1],
        _owner: trace.clone(),
    }
}
fn group(trace: &Rc<Trace>, kind: GroupKind, children: Vec<Node>) -> Node {
    Node::Group {
        kind,
        members: children
            .into_iter()
            .enumerate()
            .map(|(index, node)| Branch {
                name: format!("slot_{index}"),
                node: Box::new(node),
                _declaration: NativeSlot {
                    _owner: trace.clone(),
                },
            })
            .collect(),
        _owner: trace.clone(),
    }
}
fn sample(trace: &Rc<Trace>) -> Node {
    group(
        trace,
        GroupKind::Parallel,
        vec![
            group(trace, GroupKind::Sequence, vec![leaf(trace), leaf(trace)]),
            leaf(trace),
        ],
    )
}
fn walk(
    trace: &Trace,
    node: &Node,
    limits: NativeGraphLimits,
) -> Result<NativeGraphSummary, NativeGraphError<PrivateError>> {
    inspect_native_graph(
        node,
        limits,
        |node| {
            trace.record("view", std::ptr::from_ref(node) as usize)?;
            Ok(match node {
                Node::Leaf { .. } => NativeGraphShape::Leaf,
                Node::Group { kind, members, .. } => NativeGraphShape::Group {
                    kind: *kind,
                    members,
                },
            })
        },
        |branch: &Branch| {
            trace.record("child", std::ptr::from_ref(branch) as usize)?;
            Ok(branch.node.as_ref())
        },
        |node| {
            trace.record("leaf", std::ptr::from_ref(node) as usize)?;
            let Node::Leaf { payload, .. } = node else {
                return Err(PrivateError("wrong native kind".into()));
            };
            if trace.granted.get() && payload.len() <= 4 {
                Ok(())
            } else {
                Err(PrivateError("private policy".into()))
            }
        },
    )
}
fn trace() -> Rc<Trace> {
    let trace = Rc::new(Trace::default());
    trace.granted.set(true);
    trace
}

#[test]
fn nested_gui_graphs_borrow_original_nonclone_slices_slots_and_declaration_order() {
    let trace = trace();
    let root = sample(&trace);
    let Node::Group { members, .. } = &root else {
        panic!()
    };
    let Node::Group {
        members: nested, ..
    } = members[0].node.as_ref()
    else {
        panic!()
    };
    let summary = walk(&trace, &root, LIMITS).unwrap();
    assert_eq!(
        summary,
        NativeGraphSummary {
            nodes: 5,
            depth: 2,
            groups: 2,
            leaves: 3
        }
    );
    let address = |node: &Node| std::ptr::from_ref(node) as usize;
    let branch = |node: &Branch| std::ptr::from_ref(node) as usize;
    assert_eq!(
        *trace.events.borrow(),
        [
            ("view", address(&root)),
            ("child", branch(&members[0])),
            ("view", address(&members[0].node)),
            ("child", branch(&nested[0])),
            ("view", address(&nested[0].node)),
            ("leaf", address(&nested[0].node)),
            ("child", branch(&nested[1])),
            ("view", address(&nested[1].node)),
            ("leaf", address(&nested[1].node)),
            ("child", branch(&members[1])),
            ("view", address(&members[1].node)),
            ("leaf", address(&members[1].node)),
        ]
    );
    assert_eq!(members[0].name, "slot_0");
}

#[test]
fn node_depth_and_member_limits_are_inclusive_and_zero_has_no_expanding_default() {
    let trace = trace();
    let root = sample(&trace);
    let exact = NativeGraphLimits {
        max_nodes: 5,
        max_depth: 2,
        max_members: 2,
    };
    assert!(walk(&trace, &root, exact).is_ok());
    assert!(matches!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_nodes: 4,
                ..exact
            }
        ),
        Err(NativeGraphError::Structure {
            node_index: 4,
            error: StructureError::NodeLimit
        })
    ));
    assert!(matches!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_depth: 1,
                ..exact
            }
        ),
        Err(NativeGraphError::Structure {
            node_index: 2,
            error: StructureError::DepthLimit
        })
    ));
    assert!(matches!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_members: 1,
                ..exact
            }
        ),
        Err(NativeGraphError::Arity { node_index: 0 })
    ));
    trace.events.borrow_mut().clear();
    assert!(matches!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_nodes: 0,
                ..exact
            }
        ),
        Err(NativeGraphError::Structure { node_index: 0, .. })
    ));
    assert!(trace.events.borrow().is_empty());
    let root = leaf(&trace);
    assert_eq!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_nodes: 1,
                max_depth: 0,
                max_members: 0
            }
        )
        .unwrap()
        .nodes,
        1
    );
    for limits in [
        NativeGraphLimits {
            max_nodes: MAX_NATIVE_GRAPH_NODES + 1,
            ..LIMITS
        },
        NativeGraphLimits {
            max_depth: MAX_NATIVE_GRAPH_DEPTH + 1,
            ..LIMITS
        },
        NativeGraphLimits {
            max_members: MAX_NATIVE_GRAPH_MEMBERS + 1,
            ..LIMITS
        },
    ] {
        trace.events.borrow_mut().clear();
        assert!(matches!(
            walk(&trace, &root, limits),
            Err(NativeGraphError::InvalidLimits)
        ));
        assert!(trace.events.borrow().is_empty());
    }
}

#[test]
fn invalid_group_arity_stops_before_any_original_child_mapping() {
    let trace = trace();
    for (kind, count) in [
        (GroupKind::Sequence, 0),
        (GroupKind::Parallel, 0),
        (GroupKind::Parallel, 1),
        (GroupKind::Sequence, 65),
    ] {
        let root = group(&trace, kind, (0..count).map(|_| leaf(&trace)).collect());
        trace.events.borrow_mut().clear();
        assert!(matches!(
            walk(&trace, &root, LIMITS),
            Err(NativeGraphError::Arity { node_index: 0 })
        ));
        assert_eq!(trace.events.borrow().len(), 1);
        assert_eq!(trace.events.borrow()[0].0, "view");
    }
}

#[test]
fn repeated_original_edges_count_per_occurrence_without_deduplication() {
    struct Shared<'a> {
        children: &'a [usize],
    }
    let shared = Shared { children: &[] };
    let root = Shared { children: &[0, 1] };
    let calls = Cell::new(0);
    let summary = inspect_native_graph(
        &root,
        NativeGraphLimits {
            max_nodes: 3,
            max_depth: 1,
            max_members: 2,
        },
        |node| -> Result<_, ()> {
            Ok(if node.children.is_empty() {
                NativeGraphShape::Leaf
            } else {
                NativeGraphShape::Group {
                    kind: GroupKind::Parallel,
                    members: node.children,
                }
            })
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok(&shared)
        },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        summary,
        NativeGraphSummary {
            nodes: 3,
            depth: 1,
            groups: 1,
            leaves: 2
        }
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn cyclic_native_views_are_bounded_without_recursive_calls_or_cycle_certificates() {
    struct Cycle {
        edges: [u8; 1],
    }
    let root = Cycle { edges: [0] };
    let views = Cell::new(0);
    let children = Cell::new(0);
    let run = |limits| {
        inspect_native_graph(
            &root,
            limits,
            |node| -> Result<_, ()> {
                views.set(views.get() + 1);
                Ok(NativeGraphShape::Group {
                    kind: GroupKind::Sequence,
                    members: &node.edges,
                })
            },
            |_| {
                children.set(children.get() + 1);
                Ok(&root)
            },
            |_| Ok(()),
        )
    };
    assert!(matches!(
        run(NativeGraphLimits {
            max_nodes: 3,
            max_depth: 16,
            max_members: 1
        }),
        Err(NativeGraphError::Structure {
            node_index: 3,
            error: StructureError::NodeLimit
        })
    ));
    assert_eq!((views.get(), children.get()), (3, 3));
    views.set(0);
    children.set(0);
    assert!(matches!(
        run(NativeGraphLimits {
            max_nodes: 128,
            max_depth: 2,
            max_members: 1
        }),
        Err(NativeGraphError::Structure {
            node_index: 3,
            error: StructureError::DepthLimit
        })
    ));
    assert_eq!((views.get(), children.get()), (3, 3));
}

#[test]
fn private_native_errors_keep_positions_and_phase_but_never_format_the_payload() {
    for phase in ["view", "child", "leaf"] {
        let trace = trace();
        let root = sample(&trace);
        trace.fail.set(Some(phase));
        let error = walk(&trace, &root, LIMITS).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private token"));
        assert!(std::error::Error::source(&error).is_none());
        let NativeGraphError::Native {
            node_index,
            phase: position,
            error: PrivateError(payload),
        } = error
        else {
            panic!()
        };
        assert_eq!(payload, "private token");
        assert_eq!(
            (node_index, position),
            match phase {
                "view" => (0, NativeGraphPhase::View),
                "child" => (0, NativeGraphPhase::Child { member_index: 0 }),
                _ => (2, NativeGraphPhase::Leaf),
            }
        );
        assert_eq!(
            trace
                .events
                .borrow()
                .iter()
                .filter(|(event, _)| *event == phase)
                .count(),
            1
        );
    }
}

#[test]
fn native_callback_unwind_never_retries_consumes_graphs_or_publishes_partial_counts() {
    for phase in ["view", "child", "leaf"] {
        let trace = trace();
        let root = sample(&trace);
        let owners = Rc::strong_count(&trace);
        trace.panic.set(Some(phase));
        assert!(catch_unwind(AssertUnwindSafe(|| walk(&trace, &root, LIMITS))).is_err());
        assert_eq!(Rc::strong_count(&trace), owners);
        assert_eq!(
            trace
                .events
                .borrow()
                .iter()
                .filter(|(event, _)| *event == phase)
                .count(),
            1
        );
        trace.panic.set(None);
        assert_eq!(walk(&trace, &root, LIMITS).unwrap().nodes, 5);
    }
}

#[test]
fn inline_leaf_checks_preserve_first_failure_without_claiming_whole_cold_admission() {
    let trace = trace();
    trace.granted.set(false);
    let root = group(
        &trace,
        GroupKind::Parallel,
        vec![leaf(&trace), group(&trace, GroupKind::Sequence, vec![])],
    );
    assert!(matches!(
        walk(&trace, &root, LIMITS),
        Err(NativeGraphError::Native {
            node_index: 1,
            phase: NativeGraphPhase::Leaf,
            ..
        })
    ));
    trace.granted.set(true);
    assert!(matches!(
        walk(&trace, &root, LIMITS),
        Err(NativeGraphError::Arity { node_index: 2 })
    ));
}

#[test]
fn successful_counts_do_not_certify_changed_graphs_leaf_payloads_or_live_grants() {
    let trace = trace();
    let mut root = sample(&trace);
    let previous = walk(&trace, &root, LIMITS).unwrap();
    trace.granted.set(false);
    assert!(walk(&trace, &root, LIMITS).is_err());
    trace.granted.set(true);
    let Node::Group { members, .. } = &mut root else {
        panic!()
    };
    let Node::Leaf { payload, .. } = members[1].node.as_mut() else {
        panic!()
    };
    payload.resize(5, 0);
    assert!(walk(&trace, &root, LIMITS).is_err());
    let Node::Group { members, .. } = &mut root else {
        panic!()
    };
    members.clear();
    assert!(matches!(
        walk(&trace, &root, LIMITS),
        Err(NativeGraphError::Arity { .. })
    ));
    assert_eq!(previous.nodes, 5);
}

#[test]
fn depth_before_nodes_and_original_payload_free_debug_are_not_native_formatters() {
    let trace = trace();
    let root = sample(&trace);
    assert!(matches!(
        walk(
            &trace,
            &root,
            NativeGraphLimits {
                max_nodes: 1,
                max_depth: 0,
                max_members: 2
            }
        ),
        Err(NativeGraphError::Structure {
            node_index: 1,
            error: StructureError::DepthLimit
        })
    ));
    let Node::Group { members, .. } = &root else {
        panic!()
    };
    let view = NativeGraphShape::Group {
        kind: GroupKind::Parallel,
        members,
    };
    let text = format!("{view:?}");
    assert!(text.contains("members: 2"));
    assert!(!text.contains("slot_0"));
}

#[test]
fn unsized_gui_local_nodes_and_nonclone_branch_slots_need_no_box_conversion() {
    trait LocalNode {
        fn shape(&self) -> NativeGraphShape<'_, LocalBranch>;
    }
    struct LocalBranch {
        node: Box<dyn LocalNode>,
        _slot: Rc<()>,
    }
    struct LocalLeaf {
        _private: Rc<()>,
    }
    struct LocalGroup {
        members: Vec<LocalBranch>,
    }
    impl LocalNode for LocalLeaf {
        fn shape(&self) -> NativeGraphShape<'_, LocalBranch> {
            NativeGraphShape::Leaf
        }
    }
    impl LocalNode for LocalGroup {
        fn shape(&self) -> NativeGraphShape<'_, LocalBranch> {
            NativeGraphShape::Group {
                kind: GroupKind::Sequence,
                members: &self.members,
            }
        }
    }
    let owner = Rc::new(());
    let root: Box<dyn LocalNode> = Box::new(LocalGroup {
        members: vec![LocalBranch {
            node: Box::new(LocalLeaf {
                _private: owner.clone(),
            }),
            _slot: owner,
        }],
    });
    let summary = inspect_native_graph(
        root.as_ref(),
        LIMITS,
        |node| -> Result<_, ()> { Ok(node.shape()) },
        |branch| Ok(branch.node.as_ref()),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(
        summary,
        NativeGraphSummary {
            nodes: 2,
            depth: 1,
            groups: 1,
            leaves: 1
        }
    );
}
