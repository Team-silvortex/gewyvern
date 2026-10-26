use super::*;

pub(super) fn cases() -> Vec<(EffectRequest, EffectResult)> {
    let mut cases = crate::control_plane_reply_tests::reply_entry_cases();
    for request in crate::continuation_contract_tests::requests() {
        let EffectOperation::Presentation(envelope) = &request.operation else {
            continue;
        };
        let mut wire = serde_json::to_value(&envelope.operation).unwrap();
        if wire["kind"] == "navigate_focus" {
            wire["focused_node_id"] = serde_json::json!("node-b");
        }
        let reply = EffectResult::Presentation(serde_json::from_value(wire).unwrap());
        assert!(matches!(
            atomic_step_from_effect_result(
                &request.continuation,
                Some(&request.operation),
                reply.clone()
            ),
            Step::Done(_)
        ));
        cases.push((request, reply));
    }
    assert_eq!(cases.len(), 70);
    cases
}

fn invoke(
    vm: &mut Vm,
    mode: usize,
    lease: &DispatchLease,
    now: u64,
    reply: EffectResult,
    frozen: bool,
) -> Step {
    match (mode, frozen) {
        (0, true) => frozen_resume(vm, &lease.request.continuation, reply),
        (0, false) => vm.resume(&lease.request.continuation, reply),
        (1, true) => frozen_resume_at(vm, &lease.request.continuation, now, reply),
        (1, false) => vm.resume_at(&lease.request.continuation, now, reply),
        (_, true) => frozen_acknowledge_effect(vm, lease, now, reply),
        (_, false) => vm.acknowledge_effect(lease, now, reply),
    }
}

fn assert_state(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(12),
        expected.scheduler_pressure(12)
    );
}

#[test]
fn reply_entries_match_frozen_whole_vm_for_seventy_operations_and_competing_faults() {
    let mut comparisons = 0;
    let mut completed_routes = 0;
    for (original, original_reply) in cases() {
        for mode in 0..3 {
            for defect in 0..16 {
                let mut request = original.clone();
                if defect == 7 {
                    request.continuation.deadline_at_ms = Some(10);
                    request.budget = request.continuation.resource_budget();
                }
                let mut actual = Vm::new(1);
                let mut expected = Vm::new(1);
                actual.restore_request(request.clone()).unwrap();
                expected.restore_request(request.clone()).unwrap();
                let mut lease = if mode == 2 || matches!(defect, 4 | 10 | 14) {
                    let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                    assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                    lease
                } else {
                    // Only the image is used by ordinary reply entries; no lease is issued.
                    DispatchLease {
                        request: request.clone(),
                        attempt: 0,
                        retry_count: 0,
                        lease_expires_at_ms: 0,
                    }
                };
                if matches!(defect, 4 | 14) {
                    assert_eq!(
                        frozen_acknowledge_effect(&mut actual, &lease, 12, original_reply.clone()),
                        frozen_acknowledge_effect(
                            &mut expected,
                            &lease,
                            12,
                            original_reply.clone()
                        )
                    );
                }
                let mut reply = original_reply.clone();
                let mut now = 12;
                match defect {
                    1 | 4 => lease.request.continuation.schema_version = 0,
                    2 => {
                        actual.pending.clear();
                        expected.pending.clear();
                    }
                    3 => {
                        for vm in [&mut actual, &mut expected] {
                            vm.pending
                                .get_mut(&request.continuation.token)
                                .unwrap()
                                .fuel_remaining -= 1;
                        }
                    }
                    5 | 14 => {
                        reply = EffectResult::Presentation(PresentationResult::Focus {
                            node_id: "wrong-node".into(),
                        })
                    }
                    6 => lease.request.continuation.expected_revision = Some(Revision(99)),
                    8 => now = u64::MAX,
                    9 => now = 102,
                    10 => lease.attempt += 1,
                    11 => lease.retry_count += 1,
                    12 => lease.request.effect_id.clear(),
                    13 => {
                        reply = EffectResult::Presentation(PresentationResult::Focus {
                            node_id: "x".repeat(MAX_CONTINUATION_BYTES),
                        })
                    }
                    15 => lease.lease_expires_at_ms = 0,
                    _ => {}
                }
                let before = serde_json::to_vec(&lease).unwrap();
                let want = invoke(&mut expected, mode, &lease, now, reply.clone(), true);
                let got = invoke(&mut actual, mode, &lease, now, reply, false);
                assert_eq!(
                    got, want,
                    "mode={mode}, defect={defect}, type={:?}",
                    original.continuation.result_type
                );
                assert_eq!(
                    serde_json::to_vec(&got).unwrap(),
                    serde_json::to_vec(&want).unwrap()
                );
                if defect == 10
                    && mode < 2
                    && !matches!(
                        original.continuation.pending_effect,
                        Effect::RuntimeRefresh { .. }
                            | Effect::RuntimeCapabilitiesRefresh { .. }
                            | Effect::RuntimeDeploy { .. }
                            | Effect::DebuggerCancel { .. }
                    )
                {
                    assert!(matches!(got, Step::Fault(ref e) if e.code == "LSV4024"));
                }
                if defect == 0 {
                    if mode < 2
                        && matches!(
                            original.continuation.pending_effect,
                            Effect::RuntimeRefresh { .. }
                                | Effect::RuntimeCapabilitiesRefresh { .. }
                                | Effect::RuntimeDeploy { .. }
                                | Effect::DebuggerCancel { .. }
                        )
                    {
                        assert!(matches!(got, Step::Fault(ref e) if e.code == "LSV2110"));
                    } else {
                        assert!(matches!(got, Step::Done(_)), "{got:?}");
                        completed_routes += 1;
                    }
                }
                assert_state(&actual, &expected);
                assert_eq!(
                    actual.claim_effect(200, 100),
                    expected.claim_effect(200, 100)
                );
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(serde_json::to_vec(&lease).unwrap(), before);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 3360);
    assert_eq!(completed_routes, 202);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn reply_entries_sqlite_match_frozen_failures_peer_replay_restart_and_successor_drain() {
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &std::path::Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        tables
            .into_iter()
            .map(|table| {
                let mut stmt = db
                    .prepare(&format!(
                        "SELECT * FROM \"{}\" ORDER BY rowid",
                        table.replace('"', "\"\"")
                    ))
                    .unwrap();
                let columns = stmt.column_count();
                let rows = stmt
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<Result<Vec<SqlValue>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }
    fn reply(lease: &DispatchLease) -> EffectResult {
        let EffectOperation::Presentation(envelope) = &lease.request.operation else {
            panic!()
        };
        EffectResult::Presentation(
            serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
        )
    }
    let mut comparisons = 0;
    for source in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: 7)"#,
        r#"bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: "b"), body: 7))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(s: ui.focus(node_id: "c"), body: 7))"#,
    ] {
        let program =
            leserpent_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}")))
                .unwrap();
        for mode in [1, 2] {
            for defect in 0..4 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-reply-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual = Vm::open_journal(&actual_path, 100).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 100).unwrap();
                let start = |vm: &mut Vm| {
                    vm.start(
                        &program,
                        Principal::new("operator").unwrap(),
                        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                        None,
                    )
                };
                let started = start(&mut actual);
                assert_eq!(started, start(&mut expected));
                let mut peer_actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut peer_expected = Vm::open_journal(&expected_path, 1).unwrap();
                let lease = if mode == 2 {
                    let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                    assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                    lease
                } else {
                    let request = match started {
                        Step::Effect(request) => *request,
                        Step::Effects(mut batch) => batch.branches.remove(0).request,
                        _ => panic!(),
                    };
                    DispatchLease {
                        request,
                        attempt: 0,
                        retry_count: 0,
                        lease_expires_at_ms: 0,
                    }
                };
                if defect > 0 {
                    let (table, action) = match defect {
                        1 => ("vm_effects", "UPDATE"),
                        2 => ("vm_dispatches", "UPDATE"),
                        _ => ("vm_effects", "INSERT"),
                    };
                    for path in [&actual_path, &expected_path] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_reply BEFORE {action} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                let want = invoke(&mut expected, mode, &lease, 2, reply(&lease), true);
                let got = invoke(&mut actual, mode, &lease, 2, reply(&lease), false);
                assert_eq!(got, want, "{source}, mode={mode}, defect={defect}");
                if defect == 0 {
                    assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                if defect == 0 {
                    assert_eq!(
                        invoke(&mut peer_actual, mode, &lease, 3, reply(&lease), false),
                        invoke(&mut peer_expected, mode, &lease, 3, reply(&lease), true)
                    );
                    assert_state(&peer_actual, &peer_expected);
                }
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_reply")
                        .unwrap();
                }
                drop((actual, expected, peer_actual, peer_expected));
                let mut actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 1).unwrap();
                assert_state(&actual, &expected);
                let mut drained = 0;
                for now in [200, 202, 204, 206, 208] {
                    let lease = actual.claim_effect(now, 100).unwrap();
                    assert_eq!(lease, expected.claim_effect(now, 100).unwrap());
                    let Some(lease) = lease else { break };
                    let got = invoke(&mut actual, 2, &lease, now + 1, reply(&lease), false);
                    let want = invoke(&mut expected, 2, &lease, now + 1, reply(&lease), true);
                    assert_eq!(got, want);
                    assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                    drained += 1;
                }
                assert!(drained < 5);
                assert_eq!(actual.pending_count(), 0);
                assert_state(&actual, &expected);
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 64);
}

// Frozen complete reply entries before their product coordination moved.
fn frozen_resume(vm: &mut Vm, image: &ContinuationImage, result: EffectResult) -> Step {
    if image.deadline_at_ms.is_some() {
        return fault(
            "LSV2111",
            "timed effects require resume_at with trusted scheduler time",
        );
    }
    frozen_resume_inner(vm, image, result)
}

fn frozen_resume_at(
    vm: &mut Vm,
    image: &ContinuationImage,
    now_ms: u64,
    result: EffectResult,
) -> Step {
    if let Err(error) = validate_scheduler_time(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = vm.expire_due(now_ms) {
        return Step::Fault(error);
    }
    frozen_resume_inner(vm, image, result)
}

fn frozen_resume_inner(vm: &mut Vm, image: &ContinuationImage, result: EffectResult) -> Step {
    match vm.replay_completed(&image.token) {
        Ok(Some(completed)) => return vm.visible_step(&image.token, completed),
        Ok(None) => {}
        Err(error) => return Step::Fault(error),
    }
    if let Err(error) = validate_image(image) {
        return Step::Fault(error);
    }
    let Some(stored) = vm.pending.get(&image.token) else {
        return fault("LSV2004", "unknown continuation token");
    };
    if stored != image {
        return fault("LSV2005", "continuation image does not match pending state");
    }
    if matches!(
        image.pending_effect,
        Effect::RuntimeRefresh { .. }
            | Effect::RuntimeCapabilitiesRefresh { .. }
            | Effect::RuntimeDeploy { .. }
            | Effect::DebuggerCancel { .. }
    ) {
        return fault(
            "LSV2110",
            "mutating effects must be completed through dispatch acknowledgement",
        );
    }

    vm.complete_result(image, None, result)
}

fn frozen_acknowledge_effect(
    vm: &mut Vm,
    lease: &DispatchLease,
    now_ms: u64,
    result: EffectResult,
) -> Step {
    if let Err(error) = vm.expire_due(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = validate_effect_request(&lease.request) {
        return Step::Fault(error);
    }
    let image = &lease.request.continuation;
    match vm.replay_completed(&image.token) {
        Ok(Some(completed)) => return vm.visible_step(&image.token, completed),
        Ok(None) => {}
        Err(error) => return Step::Fault(error),
    }
    let Some(stored) = vm.pending.get(&image.token) else {
        return fault("LSV2004", "unknown continuation token");
    };
    if stored != image {
        return fault("LSV2005", "continuation image does not match pending state");
    }
    vm.complete_result(image, Some((lease, now_ms)), result)
}
