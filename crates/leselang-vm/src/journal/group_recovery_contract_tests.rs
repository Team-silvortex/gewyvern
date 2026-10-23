use super::*;
use leselang_runtime_core::SequenceReservations;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn frozen_batch(connection: &Connection) -> Result<u64, Fault> {
    type RawMergeGroup = (String, Vec<u8>, String, Option<Vec<u8>>, String);
    let groups = {
        let mut statement = connection
            .prepare(
                "SELECT token, plan, state, terminal_step, execution_order
                 FROM vm_merge_groups ORDER BY token ASC",
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to prepare merge graph load", error)
            })?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|error| journal_error("LSV4005", "failed to load merge graphs", error))?
            .collect::<Result<Vec<RawMergeGroup>, _>>()
            .map_err(|error| journal_error("LSV4005", "failed to read merge graph", error))?
    };

    let mut successor_reservations = SequenceReservations::new(
        MAX_JOURNAL_RECORDS * crate::MAX_MERGE_BRANCHES,
        crate::MAX_EFFECT_SEQUENCE,
    );
    let mut highest_sequence = 0;
    for (token, plan_bytes, state, terminal_bytes, order) in groups {
        let token = ContinuationToken::from_wire(token);
        if !valid_continuation_token(&token) {
            return Err(journal_fault("LSV4007", "merge group token is invalid"));
        }
        let token_collision: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                [token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4005", "failed to validate merge group token", error)
            })?;
        if token_collision {
            return Err(journal_fault(
                "LSV4007",
                "merge group token conflicts with an effect token",
            ));
        }
        let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
        validate_merge_plan(&plan)
            .map_err(|_| journal_fault("LSV4007", "merge group plan is invalid"))?;
        highest_sequence = highest_sequence.max(merge_reserved_sequence(&token, &plan));
        if let Some(binding) = &plan.result_binding {
            for sequence in binding.reserved_sequences() {
                successor_reservations
                    .insert(sequence)
                    .map_err(|_| crate::group_binding::invalid())?;
            }
        }
        if execution_order_label(plan.order) != order {
            return Err(journal_fault(
                "LSV4007",
                "execution order conflicts with group plan",
            ));
        }
        let branches: Vec<RecoveredMergeBranch> = {
            let mut statement = connection
                .prepare(
                    "SELECT b.branch_name, b.position, e.state, e.terminal_step, b.branch_token
                     FROM vm_merge_branches b
                     JOIN vm_effects e ON e.token = b.branch_token
                     WHERE b.group_token = ?1
                     ORDER BY b.position ASC",
                )
                .map_err(|error| {
                    journal_error("LSV4005", "failed to prepare merge branches", error)
                })?;
            statement
                .query_map([token.as_str()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<Vec<u8>>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(|error| journal_error("LSV4005", "failed to load merge branches", error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| journal_error("LSV4005", "failed to read merge branches", error))?
        };
        frozen(connection, &token, &plan, &branches, &state, terminal_bytes)?;
    }
    let reservations = successor_reservations
        .finish()
        .map_err(|_| crate::group_binding::invalid())?;
    Ok(highest_sequence.max(reservations.highest_sequence))
}

fn frozen(
    connection: &Connection,
    owner: &ContinuationToken,
    plan: &MergePlan,
    rows: &[RecoveredMergeBranch],
    state: &str,
    encoded: Option<Vec<u8>>,
) -> Result<(), Fault> {
    let mismatch = || journal_fault("LSV4007", "merge branches do not match their declared plan");
    if rows.len() != plan.branches.len() || rows.len() > crate::MAX_MERGE_BRANCHES {
        return Err(mismatch());
    }
    let mut identities = std::collections::HashSet::new();
    for (index, row) in rows.iter().enumerate() {
        if usize::try_from(row.1).ok() != Some(index)
            || row.0 != plan.branches[index]
            || row.4 == owner.as_str()
            || !identities.insert(&row.4)
        {
            return Err(mismatch());
        }
    }
    let complete = rows.iter().all(|row| row.2 == "completed");
    if matches!(
        plan.order,
        ExecutionOrder::ResultChain | ExecutionOrder::Dataflow
    ) {
        successor::validate_chain(connection, owner)?;
    }
    if plan.order.is_sequential() && state == "pending" {
        validate_pending_sequence(connection, owner, rows)?;
    }
    let invalid = || {
        journal_fault(
            "LSV4007",
            "merge group state conflicts with its branch graph",
        )
    };
    let terminal = match (state, complete, encoded) {
        ("pending", false, None) => None,
        ("completed", true, Some(bytes)) => {
            let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&step)?;
            validate_unbound_group_receipt(plan, rows, &step)?;
            Some(step)
        }
        _ => return Err(invalid()),
    };
    group_binding::validate_records(connection, owner, plan, terminal.as_ref())
}

pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new(source: &str, stage: usize) -> Self {
        Self::with_groups(source, stage, 1)
    }
    fn with_groups(source: &str, stage: usize, count: usize) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "leselang-group-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let fixture = Self(directory.join("journal.sqlite3"));
        let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
        for _ in 0..count {
            assert!(matches!(
                vm.start(
                    &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
                    crate::Principal::new("operator").unwrap(),
                    crate::CapabilitySet::new(["ui.presentation"]),
                    None,
                ),
                Step::Effect(_) | Step::Effects(_)
            ));
            for index in 0..if stage == 2 { 8 } else { stage } {
                let Some(lease) = vm.claim_effect(index as u64 * 2 + 1, 100).unwrap() else {
                    break;
                };
                let crate::EffectOperation::Presentation(envelope) = &lease.request.operation
                else {
                    panic!()
                };
                let crate::PresentationOperation::Focus { node_id } = &envelope.operation else {
                    panic!()
                };
                let reply = crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                    node_id: node_id.clone(),
                });
                vm.acknowledge_effect(&lease, index as u64 * 2 + 2, reply);
            }
            if stage == 2 {
                assert_eq!(vm.pending_count(), 0);
            }
        }
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

pub(super) fn snapshot(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut values = Vec::new();
    for table in [
        "vm_metadata",
        "vm_effects",
        "vm_dispatches",
        "vm_merge_groups",
        "vm_merge_branches",
    ] {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let columns = statement.column_count();
        values.extend(
            statement
                .query_map([], |row| {
                    (0..columns)
                        .map(|column| row.get(column))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
        );
    }
    values
}

#[test]
fn recovered_group_pipeline_preserves_frozen_fault_order_and_every_original_table() {
    let sources = [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: a, name: "node_id")))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: bind(b: ui.focus(node_id: field(value: a, name: "node_id")), body: ui.focus(node_id: field(value: b, name: "node_id"))))"#,
    ];
    let mut comparisons = 0;
    for source in sources {
        for stage in 0..3 {
            let fixture = Fixture::new(source, stage);
            let mut connection = Connection::open(&fixture.0).unwrap();
            let groups: i64 = connection
                .query_row("SELECT COUNT(*) FROM vm_merge_groups", [], |row| row.get(0))
                .unwrap();
            if groups == 0 {
                continue;
            } // Atomic chains have no group before the first reply.
            for defect in 0..17 {
                let tx = connection.transaction().unwrap();
                let (owner, bytes, mut state, mut encoded): (
                    String,
                    Vec<u8>,
                    String,
                    Option<Vec<u8>>,
                ) = tx
                    .query_row(
                        "SELECT token, plan, state, terminal_step FROM vm_merge_groups",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .unwrap();
                let owner = ContinuationToken::from_wire(owner);
                let plan: MergePlan = serde_json::from_slice(&bytes).unwrap();
                let mut rows = tx.prepare("SELECT b.branch_name, b.position, e.state, e.terminal_step, b.branch_token FROM vm_merge_branches b JOIN vm_effects e ON e.token = b.branch_token WHERE b.group_token = ?1 ORDER BY b.position").unwrap().query_map([owner.as_str()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))).unwrap().collect::<Result<Vec<RecoveredMergeBranch>, _>>().unwrap();
                match defect {
                    0 => {}
                    1 => rows[0].0 = "wrong".into(),
                    2 => rows[0].1 = -1,
                    3 => rows[1].4 = rows[0].4.clone(),
                    4 => rows[0].4 = owner.as_str().into(),
                    5 => {
                        rows.pop();
                    }
                    6 => {
                        for row in &mut rows {
                            row.2 = "completed".into();
                        }
                        state = "pending".into();
                        encoded = None;
                    }
                    7 => state = "invalid".into(),
                    8 => {
                        state = "completed".into();
                        encoded = Some(b"{".to_vec());
                    }
                    9 => {
                        for row in &mut rows {
                            row.2 = "completed".into();
                        }
                        state = "completed".into();
                        encoded = Some(b"{".to_vec());
                    }
                    10 => {
                        state = "completed".into();
                        encoded = None;
                    }
                    11 => {
                        state = "pending".into();
                        encoded = Some(
                            serde_json::to_vec(&Step::Done(crate::Value::Scalar {
                                value: crate::ScalarValue::Integer(1),
                            }))
                            .unwrap(),
                        );
                    }
                    12 => {
                        state = "pending".into();
                        rows[0].2 = "pending".into();
                        rows[1].2 = "completed".into();
                    }
                    13 => {
                        state = "pending".into();
                        rows[0].2 = "completed".into();
                        rows[0].3 = Some(
                            serde_json::to_vec(&Step::Fault(journal_fault(
                                "LSV1001",
                                "original stop",
                            )))
                            .unwrap(),
                        );
                        rows[1].2 = "pending".into();
                    }
                    14 => {
                        tx.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                            .unwrap();
                    }
                    15 => {
                        state = "completed".into();
                        encoded = Some(
                            serde_json::to_vec(&Step::Done(crate::Value::Scalar {
                                value: crate::ScalarValue::Integer(99),
                            }))
                            .unwrap(),
                        );
                    }
                    16 => {
                        tx.execute("UPDATE vm_dispatches SET request = ?1", [b"{".as_slice()])
                            .unwrap();
                        state = "invalid".into();
                    }
                    _ => unreachable!(),
                }
                let before = snapshot(&tx);
                let expected = frozen(&tx, &owner, &plan, &rows, &state, encoded.clone());
                if defect == 0 {
                    assert_eq!(expected, Ok(()), "{source}, stage {stage}");
                }
                assert_eq!(
                    validate_recovered_group(&tx, &owner, &plan, &rows, &state, encoded),
                    expected,
                    "{source}, stage {stage}, defect {defect}"
                );
                assert_eq!(snapshot(&tx), before);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 374);
}

#[test]
fn recovered_group_batch_preserves_frozen_order_and_late_failure_without_table_changes() {
    let sources = [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 1)"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: a, name: "node_id")))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: bind(b: ui.focus(node_id: field(value: a, name: "node_id")), body: ui.focus(node_id: field(value: b, name: "node_id"))))"#,
    ];
    let mut comparisons = 0;
    for source in sources {
        // Exercise both pending and fully settled multi-group journals.
        for stage in [0, 2] {
            let fixture = Fixture::with_groups(source, stage, 2);
            let mut connection = Connection::open(&fixture.0).unwrap();
            // Model torn stored rows without the test writer repairing their links.
            connection
                .pragma_update(None, "foreign_keys", false)
                .unwrap();
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            let owners = connection
                .prepare("SELECT token FROM vm_merge_groups ORDER BY token")
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            if owners.is_empty() {
                assert_eq!(validate_merge_graph_records(&connection), Ok(0));
                continue;
            }
            assert_eq!(owners.len(), 2);
            for owner in &owners {
                for defect in 0..13 {
                    let tx = connection.transaction().unwrap();
                    match defect {
                        0 => {}
                        1 => {
                            tx.execute(
                                "UPDATE vm_merge_groups SET plan = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), owner],
                            )
                            .unwrap();
                        }
                        2 => {
                            tx.execute(
                                "UPDATE vm_merge_groups SET token = '' WHERE token = ?1",
                                [owner],
                            )
                            .unwrap();
                        }
                        3 => {
                            tx.execute("UPDATE vm_merge_groups SET token = (SELECT token FROM vm_effects LIMIT 1) WHERE token = ?1", [owner]).unwrap();
                        }
                        4 => {
                            tx.execute("UPDATE vm_merge_groups SET execution_order = 'wrong' WHERE token = ?1", [owner]).unwrap();
                        }
                        5 => {
                            tx.execute(
                                "UPDATE vm_merge_groups SET state = 'wrong' WHERE token = ?1",
                                [owner],
                            )
                            .unwrap();
                        }
                        6 => {
                            tx.execute(
                                "UPDATE vm_merge_groups SET terminal_step = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), owner],
                            )
                            .unwrap();
                        }
                        7 => {
                            tx.execute("UPDATE vm_merge_branches SET position = -1 WHERE group_token = ?1 AND position = 0", [owner]).unwrap();
                        }
                        8 => {
                            tx.execute("UPDATE vm_merge_branches SET branch_name = 'wrong' WHERE group_token = ?1 AND position = 0", [owner]).unwrap();
                        }
                        9 => {
                            tx.execute("DELETE FROM vm_merge_branches WHERE group_token = ?1 AND position = 0", [owner]).unwrap();
                        }
                        10 => {
                            tx.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                                .unwrap();
                        }
                        11 => {
                            tx.execute("UPDATE vm_merge_groups SET execution_order = 'wrong', state = 'wrong' WHERE token = ?1", [owner]).unwrap();
                            tx.execute("UPDATE vm_merge_branches SET branch_name = 'wrong' WHERE group_token = ?1 AND position = 0", [owner]).unwrap();
                        }
                        12 => {
                            tx.execute("UPDATE vm_merge_groups SET plan = ?1, execution_order = 'wrong' WHERE token = ?2", params![b"{".as_slice(), owner]).unwrap();
                        }
                        _ => unreachable!(),
                    }
                    let before = snapshot(&tx);
                    let expected = frozen_batch(&tx);
                    if defect == 0 {
                        assert!(expected.is_ok(), "{source}, {stage}");
                    }
                    assert_eq!(
                        validate_merge_graph_records(&tx),
                        expected,
                        "{source}, {stage}, {owner}, {defect}"
                    );
                    assert_eq!(snapshot(&tx), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 364);
}
