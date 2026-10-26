use super::*;

// Frozen pre-extraction native policies from 49648886. Storage primitives,
// encoding, and group finalization are shared; these are not independent engines.
impl EphemeralJournal {
    fn frozen_cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        let command_id = audit.command_id.as_str();
        if let Some(existing) = self.debugger_audits.get(command_id) {
            if existing.token == image.token
                && existing.idempotency_key == audit.idempotency_key.as_str()
                && existing.principal_id == audit.principal.id
                && audit_matches_record(audit, &existing.record)
            {
                let authoritative = self
                    .dispatches
                    .get(&image.token)
                    .and_then(|dispatch| dispatch.terminal_step.clone())
                    .ok_or_else(|| {
                        journal_fault("LSV4032", "debugger audit has no terminal step")
                    })?;
                return Ok((authoritative, existing.record.clone()));
            }
            return Err(journal_fault("LSV4033", "debugger command audit conflicts"));
        }
        if self.debugger_audits.values().any(|existing| {
            existing.principal_id == audit.principal.id
                && existing.idempotency_key == audit.idempotency_key.as_str()
        }) {
            return Err(journal_fault(
                "LSV4033",
                "debugger idempotency key was reused",
            ));
        }
        let dispatch = self
            .dispatches
            .get(&image.token)
            .ok_or_else(|| journal_fault("LSV4013", "pending continuation is missing"))?;
        if dispatch.request.continuation != *image {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with journal state",
            ));
        }
        // Only an exact replay above may attribute an already completed cancellation.
        if dispatch.acknowledged {
            return Err(journal_fault(
                "LSV4033",
                "completed effect has no matching debugger audit",
            ));
        }
        let authoritative = self.cancel(image, step)?;
        let record = debugger_audit_record(audit, now_ms);
        self.debugger_audits.insert(
            command_id.to_string(),
            EphemeralDebuggerAudit {
                token: image.token.clone(),
                idempotency_key: audit.idempotency_key.as_str().to_string(),
                principal_id: audit.principal.id.clone(),
                record: record.clone(),
            },
        );
        Ok((authoritative, record))
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_cancel_audited(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now_ms: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        validate_image(image)?;
        validate_cancellation_step(image, step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4032", "failed to lock debugger audit journal", error)
            })?;
        if let Some((existing_command, token, idempotency_key, record)) =
            load_debugger_audit(&transaction, audit)?
        {
            if existing_command == audit.command_id.as_str()
                && token == image.token.as_str()
                && idempotency_key == audit.idempotency_key.as_str()
                && audit_matches_record(audit, &record)
            {
                let existing = load_record(&transaction, image.token.as_str())?
                    .ok_or_else(|| journal_fault("LSV4032", "debugger audit effect is missing"))?;
                let authoritative: Step = decode_bounded(
                    &existing.terminal_step.ok_or_else(|| {
                        journal_fault("LSV4032", "debugger audit has no terminal step")
                    })?,
                    MAX_JOURNAL_ENTRY_BYTES,
                )?;
                transaction.commit().map_err(|error| {
                    journal_error("LSV4032", "failed to close debugger audit replay", error)
                })?;
                return Ok((authoritative, record));
            }
            return Err(journal_fault("LSV4033", "debugger command audit conflicts"));
        }
        let existing = load_record(&transaction, image.token.as_str())?
            .ok_or_else(|| journal_fault("LSV4013", "pending continuation is missing"))?;
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            return Err(journal_fault(
                "LSV4033",
                "completed effect has no matching debugger audit",
            ));
        }
        let record = debugger_audit_record(audit, now_ms);
        transaction
            .execute(
                "INSERT INTO vm_debugger_audit(
                    command_id, principal_id, idempotency_key, origin, session_id,
                    expected_revision, observed_at_ms, continuation_token
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    audit.command_id.as_str(),
                    audit.principal.id.as_str(),
                    audit.idempotency_key.as_str(),
                    command_origin_label(audit.origin),
                    audit.session_id.as_str(),
                    i64::try_from(audit.expected_revision.0).map_err(|_| {
                        journal_fault("LSV4033", "debugger audit revision is out of range")
                    })?,
                    i64::try_from(now_ms).map_err(|_| {
                        journal_fault("LSV4033", "debugger audit clock is out of range")
                    })?,
                    image.token.as_str(),
                ],
            )
            .map_err(|error| {
                journal_error(
                    "LSV4033",
                    "failed to store debugger cancellation audit",
                    error,
                )
            })?;
        ensure_growth(&transaction, step_bytes.len())?;
        complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error(
                "LSV4032",
                "failed to commit debugger cancellation audit",
                error,
            )
        })?;
        Ok((step.clone(), record))
    }
}

fn audit() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: crate::CommandId::new("audit-original").unwrap(),
        idempotency_key: crate::IdempotencyKey::new("key-original").unwrap(),
        principal: crate::Principal::new("operator").unwrap(),
        origin: crate::CommandOrigin::Gui,
        session_id: "session-original".into(),
        expected_revision: crate::Revision(7),
    }
}
fn cancellation(image: &ContinuationImage, time: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::Requested,
        observed_at_ms: time,
    })
}
fn memory_snapshot(journal: &EphemeralJournal) -> Vec<u8> {
    serde_json::to_vec(&(
        journal
            .dispatches
            .iter()
            .map(|(token, d)| {
                (
                    token,
                    &d.request,
                    d.attempt,
                    d.lease_expires_at_ms,
                    d.ready_at_ms,
                    d.retry_count,
                    &d.last_error,
                    d.acknowledged,
                    &d.terminal_step,
                )
            })
            .collect::<Vec<_>>(),
        journal
            .merge_groups
            .iter()
            .map(|(token, g)| (token, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        journal
            .debugger_audits
            .iter()
            .map(|(command, a)| {
                (
                    command,
                    &a.token,
                    &a.idempotency_key,
                    &a.principal_id,
                    &a.record,
                )
            })
            .collect::<Vec<_>>(),
    ))
    .unwrap()
}
fn memory_fixture(
    request: &EffectRequest,
    variant: usize,
) -> (
    EphemeralJournal,
    ContinuationImage,
    Step,
    DebuggerAuditContext,
) {
    let mut journal = EphemeralJournal::default();
    journal
        .record_pending(Some(request), SchedulerLimits::default())
        .unwrap();
    let mut image = request.continuation.clone();
    let mut audit = audit();
    let mut step = cancellation(&image, 200);
    if matches!(variant, 1..=9 | 12 | 13 | 15 | 17) {
        journal
            .frozen_cancel_audited(&image, &cancellation(&image, 30), &audit, 30)
            .unwrap();
    }
    match variant {
        0 | 1 => {}
        2 => audit.idempotency_key = crate::IdempotencyKey::new("key-other").unwrap(),
        3 => audit.principal = crate::Principal::new("other").unwrap(),
        4 => audit.origin = crate::CommandOrigin::Cli,
        5 => audit.session_id = "other".into(),
        6 => audit.expected_revision = crate::Revision(8),
        7 => image.token = ContinuationToken::from_wire("effect-999".into()),
        8 => audit.command_id = crate::CommandId::new("audit-other").unwrap(),
        9 => {
            audit.command_id = crate::CommandId::new("audit-other").unwrap();
            audit.idempotency_key = crate::IdempotencyKey::new("key-other").unwrap();
        }
        10 | 17 => image.fuel_remaining += 1,
        11 | 13 => {
            journal.dispatches.clear();
        }
        12 => {
            journal
                .dispatches
                .get_mut(&image.token)
                .unwrap()
                .terminal_step = None
        }
        14 | 15 => {
            step = Step::Done(crate::Value::UiFocus {
                node_id: "not-cancellation".into(),
            })
        }
        16 => {
            journal.cancel(&image, &cancellation(&image, 30)).unwrap();
        }
        _ => unreachable!(),
    }
    (journal, image, step, audit)
}
#[test]
fn memory_audit_commit_matches_frozen_replay_conflict_and_mutation_policy_for_all_operations() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for variant in 0..18 {
            let (mut actual, image, step, audit) = memory_fixture(&request, variant);
            let (mut expected, ..) = memory_fixture(&request, variant);
            let result = actual.cancel_audited(&image, &step, &audit, 200);
            let original = expected.frozen_cancel_audited(&image, &step, &audit, 200);
            assert_eq!(result, original, "variant {variant}");
            assert_eq!(
                memory_snapshot(&actual),
                memory_snapshot(&expected),
                "variant {variant}"
            );
            assert_eq!(
                result.is_ok(),
                matches!(variant, 0 | 1 | 15 | 17),
                "variant {variant}: {result:?}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 1260);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::super::group_recovery_contract_tests::Fixture;
    use super::*;

    const SOURCES: [&str; 5] = [
        r#"fn main() = ui.focus(node_id: "a")"#,
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    ];
    const ORIGINS: [crate::CommandOrigin; 5] = [
        crate::CommandOrigin::Gui,
        crate::CommandOrigin::Cli,
        crate::CommandOrigin::Leselang,
        crate::CommandOrigin::Model,
        crate::CommandOrigin::CompatibilityAdapter,
    ];

    fn snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let mut tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        tables
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|table| {
                let table = table.unwrap();
                let mut query = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let columns = query.column_count();
                let rows = query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }

    fn mutate(
        journal: &mut SqliteJournal,
        image: &mut ContinuationImage,
        step: &mut Step,
        audit: &mut DebuggerAuditContext,
        variant: usize,
    ) {
        // Deliberate corruption only in disposable fixtures. Restore enforcement
        // before the production method begins its transaction.
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match variant {
            0 | 1 => {},
            2 => audit.idempotency_key = crate::IdempotencyKey::new("key-other").unwrap(),
            3 => audit.principal = crate::Principal::new("other").unwrap(),
            4 => audit.origin = if audit.origin == crate::CommandOrigin::Cli { crate::CommandOrigin::Gui } else { crate::CommandOrigin::Cli },
            5 => audit.session_id = "other".into(),
            6 => audit.expected_revision = crate::Revision(8),
            7 => image.token = ContinuationToken::from_wire("effect-999".into()),
            8 => audit.command_id = crate::CommandId::new("audit-other").unwrap(),
            9 => { audit.command_id = crate::CommandId::new("audit-other").unwrap(); audit.idempotency_key = crate::IdempotencyKey::new("key-other").unwrap(); },
            10 | 17 => image.fuel_remaining += 1,
            11 | 13 => { journal.connection.execute("DELETE FROM vm_effects WHERE token=?1", [image.token.as_str()]).unwrap(); },
            12 => { journal.connection.execute("UPDATE vm_effects SET terminal_step=NULL WHERE token=?1", [image.token.as_str()]).unwrap(); },
            14 | 15 => *step = Step::Done(crate::Value::UiFocus { node_id: "not-cancellation".into() }),
            16 => { journal.cancel(image, &cancellation(image, 30)).unwrap(); },
            18 => audit.expected_revision = crate::Revision(u64::MAX),
            19 => { journal.connection.execute("UPDATE vm_effects SET terminal_step=x'7b' WHERE token=?1", [image.token.as_str()]).unwrap(); },
            20 => journal.connection.execute_batch("CREATE TRIGGER refuse_audit BEFORE INSERT ON vm_debugger_audit BEGIN SELECT RAISE(ABORT, 'audit refused'); END;").unwrap(),
            21 => journal.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT, 'effect refused'); END;").unwrap(),
            22 => journal.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'dispatch refused'); END;").unwrap(),
            23 => journal.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'group refused'); END;").unwrap(),
            24 => journal.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER INSERT ON vm_debugger_audit BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            _ => unreachable!(),
        }
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }

    #[test]
    fn sqlite_audit_commit_matches_frozen_policy_and_rolls_back_all_failed_writes() {
        let mut cases = 0;
        for source in SOURCES {
            for origin in ORIGINS {
                for variant in 0..25 {
                    let fixture = Fixture::new(source, 0);
                    let vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                    let original_image = vm.pending_continuations().remove(0);
                    drop(vm);
                    let mut audit = audit();
                    audit.origin = origin;
                    if matches!(variant, 1..=9 | 12 | 13 | 15 | 17 | 19) {
                        let mut seed = SqliteJournal {
                            connection: Connection::open(&fixture.0).unwrap(),
                        };
                        seed.frozen_cancel_audited(
                            &original_image,
                            &cancellation(&original_image, 30),
                            &audit,
                            30,
                        )
                        .unwrap();
                    }
                    let other = fixture.0.with_extension("reference.sqlite3");
                    std::fs::copy(&fixture.0, &other).unwrap();
                    let mut actual = SqliteJournal {
                        connection: Connection::open(&fixture.0).unwrap(),
                    };
                    let mut original = SqliteJournal {
                        connection: Connection::open(&other).unwrap(),
                    };
                    let (mut image, mut proposed, mut metadata) = (
                        original_image.clone(),
                        cancellation(&original_image, 200),
                        audit.clone(),
                    );
                    mutate(
                        &mut actual,
                        &mut image,
                        &mut proposed,
                        &mut metadata,
                        variant,
                    );
                    mutate(
                        &mut original,
                        &mut original_image.clone(),
                        &mut cancellation(&original_image, 200),
                        &mut audit,
                        variant,
                    );
                    let before = snapshot(&actual.connection);
                    assert_eq!(before, snapshot(&original.connection));
                    let expected =
                        original.frozen_cancel_audited(&image, &proposed, &metadata, 200);
                    let result = actual.cancel_audited(&image, &proposed, &metadata, 200);
                    assert_eq!(result, expected, "{source} {origin:?} {variant}");
                    assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                    if result.is_err() {
                        assert_eq!(snapshot(&actual.connection), before, "variant {variant}");
                    }
                    if matches!(variant, 0 | 1 | 17) {
                        assert!(result.is_ok(), "{result:?}");
                    }
                    if matches!(variant, 20..=22 | 24) {
                        assert!(result.is_err());
                    }
                    assert!(actual.connection.is_autocommit());
                    assert!(original.connection.is_autocommit());
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 625);
    }

    #[test]
    fn committed_audit_reopens_with_original_attribution_and_never_dispatches_its_token() {
        let mut cases = 0;
        for source in SOURCES {
            for origin in ORIGINS {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let image = vm.pending_continuations().remove(0);
                let mut audit = audit();
                audit.origin = origin;
                let record = vm.cancel_effect_audited(&image, 30, &audit).unwrap();
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(
                    reopened.debugger_audit(&audit.command_id).unwrap(),
                    Some(record.clone())
                );
                assert_eq!(
                    reopened.cancel_effect_audited(&image, 200, &audit).unwrap(),
                    record
                );
                assert_eq!(record.observed_at_ms, 30);
                for now in 201..205 {
                    if let Some(lease) = reopened.claim_effect(now, 100).unwrap() {
                        assert_ne!(lease.request.continuation.token, image.token);
                    }
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 25);
    }
}
