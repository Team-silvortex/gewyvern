use std::rc::Rc;

use leselang_runtime_core::{StructureBudget, StructureError};

#[test]
fn physical_visits_cost_one_and_limits_are_inclusive_even_at_zero() {
    let mut empty = StructureBudget::new(0, 0);
    assert_eq!(empty.visit(0, 0, 0), Err(StructureError::NodeLimit));
    assert_eq!(empty.visited(), 0);
    let mut root = StructureBudget::new(1, 0);
    root.visit(0, 0, 0).unwrap();
    assert_eq!(root.visited(), 1);
    assert_eq!(root.visit(0, 0, 0), Err(StructureError::NodeLimit));
    assert_eq!(root.visited(), 1);
    assert_eq!(root.visit(1, 0, 0), Err(StructureError::DepthLimit));
}

#[test]
fn folded_weights_spend_nodes_and_depth_without_erasing_the_accepted_prefix() {
    let mut budget = StructureBudget::new(5, 4);
    budget.visit(0, 0, 0).unwrap();
    budget.visit(3, 2, 1).unwrap();
    assert_eq!(budget.visited(), 4);
    assert_eq!(budget.visit(4, 0, 1), Err(StructureError::DepthLimit));
    assert_eq!(budget.visit(0, 1, 0), Err(StructureError::NodeLimit));
    assert_eq!(budget.visited(), 4);
    // The adapter must reject the failed graph, not silently skip its bad branch.
    budget.visit(0, 0, 0).unwrap();
    assert_eq!(budget.visited(), 5);
}

#[test]
fn full_width_node_arithmetic_never_wraps_or_saturates_into_acceptance() {
    let mut budget = StructureBudget::new(usize::MAX, usize::MAX);
    assert_eq!(
        budget.visit(0, usize::MAX, 0),
        Err(StructureError::NodeLimit)
    );
    assert_eq!(budget.visited(), 0);
    budget.visit(0, usize::MAX - 1, 0).unwrap();
    assert_eq!(budget.visited(), usize::MAX);
    assert_eq!(budget.visit(0, 0, 0), Err(StructureError::NodeLimit));
    assert_eq!(budget.visited(), usize::MAX);
    budget.check_pending(0, 0).unwrap();
    assert_eq!(budget.check_pending(0, 1), Err(StructureError::NodeLimit));
}

#[test]
fn full_width_depth_is_checked_before_nodes_and_rejection_is_atomic() {
    let mut budget = StructureBudget::new(usize::MAX, usize::MAX);
    budget.visit(usize::MAX, 0, 0).unwrap();
    assert_eq!(
        budget.visit(usize::MAX, usize::MAX, 1),
        Err(StructureError::DepthLimit)
    );
    assert_eq!(budget.visited(), 1);
    budget.visit(usize::MAX - 1, 0, 1).unwrap();
    assert_eq!(budget.visited(), 2);
    let mut empty = StructureBudget::new(0, 0);
    assert_eq!(empty.visit(1, 0, 0), Err(StructureError::DepthLimit));
    assert_eq!(empty.visited(), 0);
}

#[test]
fn pending_capacity_checks_are_readonly_not_reservations_or_source_certificates() {
    let mut budget = StructureBudget::new(4, 0);
    budget.visit(0, 0, 0).unwrap();
    for _ in 0..3 {
        budget.check_pending(2, 1).unwrap();
        assert_eq!(budget.visited(), 1);
    }
    assert_eq!(budget.check_pending(2, 2), Err(StructureError::NodeLimit));
    budget.check_pending(0, 1).unwrap();
    assert_eq!(budget.visit(1, 0, 0), Err(StructureError::DepthLimit));
    assert_eq!(budget.visit(0, 4, 0), Err(StructureError::NodeLimit));
    assert_eq!(budget.visited(), 1);
}

#[test]
fn pending_arithmetic_handles_both_overflow_sites_without_native_panics() {
    let budget = StructureBudget::new(usize::MAX, 0);
    budget.check_pending(usize::MAX, 0).unwrap();
    assert_eq!(
        budget.check_pending(usize::MAX, 1),
        Err(StructureError::NodeLimit)
    );
    let mut used = StructureBudget::new(usize::MAX, 0);
    used.visit(0, 0, 0).unwrap();
    assert_eq!(
        used.check_pending(usize::MAX, 0),
        Err(StructureError::NodeLimit)
    );
    used.check_pending(usize::MAX - 1, 0).unwrap();
    assert_eq!(used.visited(), 1);
}

#[test]
fn caller_owned_walks_support_unrelated_layouts_and_repeated_cyclic_edges() {
    struct GuiNode {
        _private: Rc<String>,
        children: Vec<GuiNode>,
    }
    let gui = GuiNode {
        _private: Rc::new("host-only".into()),
        children: vec![GuiNode {
            _private: Rc::new("child-only".into()),
            children: vec![],
        }],
    };
    let mut budget = StructureBudget::new(2, 1);
    let mut pending = vec![(&gui, 0)];
    while let Some((node, depth)) = pending.pop() {
        budget.visit(depth, 0, 0).unwrap();
        budget
            .check_pending(pending.len(), node.children.len())
            .unwrap();
        pending.extend(node.children.iter().map(|child| (child, depth + 1)));
    }
    assert_eq!(budget.visited(), 2);

    let edges = [0usize];
    let mut cyclic = StructureBudget::new(8, usize::MAX);
    let mut pending = vec![(0, 0)];
    while let Some((index, depth)) = pending.pop() {
        cyclic.visit(depth, 0, 0).unwrap();
        if let Err(error) = cyclic.check_pending(pending.len(), 1) {
            assert_eq!(error, StructureError::NodeLimit);
            break;
        }
        pending.push((edges[index], depth + 1));
    }
    assert_eq!(cyclic.visited(), 8);
    assert_eq!(pending.len(), 0);
    // The meter bounded this compliant walk; it did not recognize or traverse a cycle.
}

#[test]
fn small_limit_model_preserves_success_counts_and_depth_first_error_order() {
    for node_limit in 0..=12 {
        for depth_limit in 0..=3 {
            let mut budget = StructureBudget::new(node_limit, depth_limit);
            let mut count = 0usize;
            for depth in 0..=4 {
                for extra_nodes in 0..=4 {
                    for extra_depth in 0..=2 {
                        let expected = if depth + extra_depth > depth_limit {
                            Err(StructureError::DepthLimit)
                        } else if count + 1 + extra_nodes > node_limit {
                            Err(StructureError::NodeLimit)
                        } else {
                            count += 1 + extra_nodes;
                            Ok(())
                        };
                        assert_eq!(budget.visit(depth, extra_nodes, extra_depth), expected);
                        assert_eq!(budget.visited(), count);
                        for pending in 0..=4 {
                            let expected = if count + pending + 2 <= node_limit {
                                Ok(())
                            } else {
                                Err(StructureError::NodeLimit)
                            };
                            assert_eq!(budget.check_pending(pending, 2), expected);
                            assert_eq!(budget.visited(), count);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn errors_are_fixed_payload_free_metadata_and_meters_move_between_workers() {
    assert_eq!(
        StructureError::NodeLimit.to_string(),
        "structure node limit exceeded"
    );
    assert_eq!(
        StructureError::DepthLimit.to_string(),
        "structure depth limit exceeded"
    );
    let mut budget = StructureBudget::new(5, 2);
    budget.visit(0, 0, 0).unwrap();
    let budget = std::thread::spawn(move || {
        budget.visit(1, 1, 0).unwrap();
        budget
    })
    .join()
    .unwrap();
    assert_eq!(budget.visited(), 3);
    assert_eq!(
        format!("{budget:?}"),
        "StructureBudget { max_nodes: 5, max_depth: 2, visited: 3 }"
    );
}
