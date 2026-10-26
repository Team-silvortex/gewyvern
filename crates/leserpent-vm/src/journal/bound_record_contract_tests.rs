use super::super::group_recovery_contract_tests::Fixture;
use super::*;
use leselang_command::reference_bound_receipt::validate_bound_group_receipt;
use leselang_runtime_core::{OrderedOutcomeSummary, OutcomeOrder};

fn snapshot(connection: &Connection) -> Vec<Vec<(rusqlite::types::Type, Vec<u8>)>> {
    use rusqlite::types::ValueRef;
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
                        .map(|column| {
                            let value = row.get_ref(column)?;
                            // Preserve corrupt TEXT bytes without decoding them or hiding their storage type.
                            let bytes = match value {
                                ValueRef::Null => Vec::new(),
                                ValueRef::Integer(value) => value.to_le_bytes().to_vec(),
                                ValueRef::Real(value) => value.to_bits().to_le_bytes().to_vec(),
                                ValueRef::Text(bytes) | ValueRef::Blob(bytes) => bytes.to_vec(),
                            };
                            Ok((value.data_type(), bytes))
                        })
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap(),
        );
    }
    values
}

#[test]
fn bound_record_stream_preserves_frozen_sqlite_fault_order_and_original_tables() {
    let mut comparisons = 0;
    for mode in ["seq", "all"] {
        for body in [
            "1",
            "ui.focus(node_id: \"tail\")",
            "bind(r: ui.focus(node_id: \"tail\"), body: true)",
            "bind(r: ui.focus(node_id: \"tail\"), body: bind(s: ui.focus(node_id: \"next\"), body: true))",
            "choose(when: true, then: bind(r: ui.focus(node_id: \"tail\"), body: true), otherwise: false)",
        ] {
            let source = format!(
                "fn main() = bind(g: {mode}(a: ui.focus(node_id: \"a\"), b: ui.focus(node_id: \"b\")), body: {body})"
            );
            for stage in [0, 1, 3, 2] {
                let fixture = Fixture::new(&source, stage);
                let mut connection = Connection::open(&fixture.0).unwrap();
                // Model damaged on-disk records that cannot be produced by normal writes.
                connection
                    .pragma_update(None, "ignore_check_constraints", true)
                    .unwrap();
                for defect in 0..18 {
                    let tx = connection.transaction().unwrap();
                    let (owner, plan, terminal): (String, Vec<u8>, Option<Vec<u8>>) = tx
                        .query_row(
                            "SELECT token, plan, terminal_step FROM vm_merge_groups",
                            [],
                            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                        )
                        .unwrap();
                    let token = ContinuationToken::from_wire(owner);
                    let plan: MergePlan = serde_json::from_slice(&plan).unwrap();
                    let mut terminal: Option<Step> = terminal
                        .as_deref()
                        .map(|bytes| serde_json::from_slice(bytes).unwrap());
                    let tokens: Vec<String> = tx
                        .prepare("SELECT branch_token FROM vm_merge_branches ORDER BY position")
                        .unwrap()
                        .query_map([], |row| row.get(0))
                        .unwrap()
                        .collect::<Result<_, _>>()
                        .unwrap();
                    let first = tokens.first().unwrap();
                    let last = tokens.last().unwrap();
                    match defect {
                        0 => {}
                        1 | 16 | 17 => {
                            tx.execute(
                                "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), first],
                            )
                            .unwrap();
                        }
                        2 => {
                            tx.execute(
                                "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), last],
                            )
                            .unwrap();
                        }
                        3 => {
                            tx.execute(
                                "UPDATE vm_effects SET terminal_step = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), first],
                            )
                            .unwrap();
                        }
                        4 => {
                            tx.execute(
                                "UPDATE vm_effects SET terminal_step = ?1 WHERE token = ?2",
                                params![b"{".as_slice(), last],
                            )
                            .unwrap();
                        }
                        5 => {
                            tx.execute("DELETE FROM vm_dispatches WHERE token = ?1", [last])
                                .unwrap();
                        }
                        6 => {
                            tx.execute("UPDATE vm_merge_branches SET branch_name = 'wrong' WHERE branch_token = ?1", [first]).unwrap();
                        }
                        7 => {
                            tx.execute("UPDATE vm_merge_branches SET branch_name = CAST(x'80' AS TEXT) WHERE branch_token = ?1", [last]).unwrap();
                        }
                        8 => {
                            tx.execute(
                                "UPDATE vm_dispatches SET request = zeroblob(1) WHERE token = ?1",
                                [last],
                            )
                            .unwrap();
                        }
                        9 => {
                            tx.execute(
                                "UPDATE vm_effects SET terminal_step = NULL WHERE token = ?1",
                                [&tokens[tokens.len() - 2]],
                            )
                            .unwrap();
                        }
                        10 => {
                            tx.execute(
                                "UPDATE vm_effects SET terminal_step = ?1 WHERE token = ?2",
                                params![
                                    serde_json::to_vec(&Step::Done(crate::Value::Scalar {
                                        value: crate::ScalarValue::Boolean(true)
                                    }))
                                    .unwrap(),
                                    first
                                ],
                            )
                            .unwrap();
                        }
                        11..=13 => {
                            let bytes: Vec<u8> = tx
                                .query_row(
                                    "SELECT request FROM vm_dispatches WHERE token = ?1",
                                    [last],
                                    |row| row.get(0),
                                )
                                .unwrap();
                            let mut request: EffectRequest =
                                serde_json::from_slice(&bytes).unwrap();
                            match defect {
                                11 => request.continuation.result_binding = None,
                                12 => request.continuation.fuel_remaining += 1,
                                _ => request.budget.max_output_items = 0,
                            }
                            tx.execute(
                                "UPDATE vm_dispatches SET request = ?1 WHERE token = ?2",
                                params![serde_json::to_vec(&request).unwrap(), last],
                            )
                            .unwrap();
                        }
                        14 => {
                            tx.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                                .unwrap();
                        }
                        15 => terminal = Some(Step::Fault(journal_fault("wrong", "not original"))),
                        _ => unreachable!(),
                    }
                    if defect == 16 {
                        tx.execute(
                            "UPDATE vm_dispatches SET request = zeroblob(1) WHERE token = ?1",
                            [last],
                        )
                        .unwrap();
                    }
                    if defect == 17 {
                        tx.execute("UPDATE vm_merge_branches SET branch_name = CAST(x'80' AS TEXT) WHERE branch_token = ?1", [last]).unwrap();
                    }
                    let before = snapshot(&tx);
                    let actual = validate_records(&tx, &token, &plan, terminal.as_ref());
                    assert_eq!(snapshot(&tx), before);
                    let expected = frozen_records(&tx, &token, &plan, terminal.as_ref());
                    assert_eq!(
                        actual, expected,
                        "mode={mode} body={body} stage={stage} defect={defect}"
                    );
                    assert_eq!(snapshot(&tx), before);
                    if defect == 0 {
                        assert_eq!(actual, Ok(()));
                    }
                    tx.rollback().unwrap();
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 720);
}

fn frozen_records(
    connection: &Connection,
    token: &ContinuationToken,
    plan: &MergePlan,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    let mut statement = connection
        .prepare(
            "SELECT b.branch_name, d.request, e.terminal_step
         FROM vm_merge_branches b
         LEFT JOIN vm_dispatches d ON d.token = b.branch_token
         JOIN vm_effects e ON e.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position ASC",
        )
        .map_err(|error| journal_error("LSV4007", "failed to load bound group requests", error))?;
    let rows = statement
        .query_map([token.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .map_err(|error| journal_error("LSV4007", "failed to read bound group requests", error))?;
    let mut requests: Vec<(String, EffectRequest)> = Vec::with_capacity(plan.branches.len());
    let mut outcomes = OrderedOutcomeSummary::new(
        if plan.order.is_sequential() {
            OutcomeOrder::Sequential
        } else {
            OutcomeOrder::Parallel
        },
        crate::MAX_MERGE_BRANCHES,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )
    .map_err(|_| invalid())?;
    let mut last_step = None;
    let mut prefix_fields = Vec::new();
    let mut prefix_operations = Vec::new();
    let mut previous_step = None;
    let mut successful_prefix = 0usize;
    for row in rows {
        let (name, request, step) =
            row.map_err(|error| journal_error("LSV4007", "invalid bound group record", error))?;
        let request: EffectRequest =
            decode_bounded(&request.ok_or_else(invalid)?, MAX_JOURNAL_ENTRY_BYTES)?;
        if binding.captures_successor() && requests.len() >= binding.branches()?.len() {
            binding.validate_saved_capture(&request, &prefix_fields, &prefix_operations)?;
            if requests.len() > binding.branches()?.len() {
                crate::successor::validate_pair(
                    &requests.last().ok_or_else(invalid)?.1,
                    previous_step.as_ref().ok_or_else(invalid)?,
                    &request,
                )?;
            }
        }
        previous_step = None;
        if let Some(bytes) = step {
            let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            previous_step = Some(step.clone());
            if requests.len() + 1 == plan.branches.len() {
                last_step = Some(step.clone());
            }
            if let Step::Done(value) = step {
                binding.validate_member(&name, &value)?;
                if requests.len() < binding.branches()?.len() {
                    successful_prefix += 1;
                }
                outcomes
                    .observe_success(validate_value(&value, 0)?, |_, _| {
                        Step::Fault(journal_fault(
                            "LSV2404",
                            "sequential output exceeds runtime bounds",
                        ))
                    })
                    .map_err(|_| invalid())?;
                if binding.captures_successor() && binding.has_successor(plan) {
                    prefix_operations.push(
                        leselang_hir::host_call::HostOperation::for_effect(
                            &request.continuation.pending_effect,
                        )
                        .ok_or_else(invalid)?,
                    );
                    prefix_fields.push(crate::StructuredField {
                        name: name.clone(),
                        value: Box::new(value),
                    });
                }
            } else {
                outcomes.observe_stop(step).map_err(|_| invalid())?;
            }
        }
        requests.push((name, request));
    }
    let graph = requests
        .iter()
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(token, plan, &graph)?;
    if binding.has_successor(plan) && successful_prefix != binding.branches()?.len() {
        return Err(invalid());
    }
    inspect_group_reservations(plan, &mut GroupReservationSource(connection))?;
    validate_bound_group_receipt(
        plan,
        requests
            .last()
            .and_then(|(_, request)| request.continuation.result_binding.as_deref()),
        last_step.as_ref(),
        outcomes,
        terminal,
    )
}
