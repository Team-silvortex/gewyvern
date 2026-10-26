use super::*;
use leselang_hir::lower;
use leselang_host_contract::RuntimeId;
use leselang_syntax::parse;
use leserpent_domain::{
    CommandResult, CommandStatus, MAX_RUNTIME_LOG_QUERY_ENTRIES, QueryResult, RefreshStatus,
    RuntimeLogLevel, RuntimeLogRecord, RuntimeProjection,
};

struct Case {
    request: EffectRequest,
    reply: EffectResult,
    value: Value,
}

fn runtime() -> RuntimeProjection {
    RuntimeProjection {
        id: RuntimeId::new("runtime-a").unwrap(),
        name: "runtime-a".into(),
        endpoint: "http://127.0.0.1:8080".into(),
        sidecar_endpoint: None,
        registered_at_unix_ms: None,
        updated_at_unix_ms: None,
        revision: Revision(7),
        refresh_count: 0,
        refresh_status: RefreshStatus::NeverRequested,
        tags: Default::default(),
        status: Default::default(),
        sidecar_status: None,
        capabilities: Default::default(),
        capabilities_observed_for_revision: None,
    }
}

fn log() -> RuntimeLogRecord {
    RuntimeLogRecord {
        sequence: 1,
        level: RuntimeLogLevel::Info,
        message: "ready".into(),
    }
}

pub(super) fn reply_entry_cases() -> Vec<(EffectRequest, EffectResult)> {
    cases()
        .into_iter()
        .map(|case| (case.request, case.reply))
        .collect()
}

fn cases() -> Vec<Case> {
    [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, source)| {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let request = Vm::build_effect_request(
            &program.function.effect,
            program.function.result_type,
            index as u64 + 1,
            Principal::new("operator").unwrap(),
            CapabilitySet::new([
                CAPABILITY_RUNTIME_READ,
                CAPABILITY_RUNTIME_REFRESH,
                CAPABILITY_RUNTIME_DEPLOY,
                CAPABILITY_DEBUGGER_CONTROL,
            ]),
            Some(Revision(7)),
            30_000,
            None,
            21,
        )
        .unwrap();
        let command = CommandResult {
            command_id: CommandId::new(format!("leselang-command-{}", index + 1)).unwrap(),
            status: CommandStatus::Applied,
            runtime: runtime(),
            events: vec![],
        };
        let (reply, value) = match index {
            0 => (
                EffectResult::Query(QueryResult::RuntimeList {
                    revision: Revision(7),
                    runtimes: vec![runtime()],
                }),
                Value::RuntimeList {
                    revision: Revision(7),
                    runtimes: vec![runtime()],
                },
            ),
            1 => (
                EffectResult::Query(QueryResult::RuntimeInspect {
                    revision: Revision(7),
                    runtime: runtime(),
                }),
                Value::RuntimeInspect {
                    revision: Revision(7),
                    runtime: Box::new(runtime()),
                },
            ),
            2 => (
                EffectResult::Query(QueryResult::RuntimeHistory {
                    revision: Revision(7),
                    entries: vec![command.clone()],
                }),
                Value::RuntimeHistory {
                    revision: Revision(7),
                    entries: vec![command],
                },
            ),
            3 => (
                EffectResult::Query(QueryResult::RuntimeLogs {
                    revision: Revision(7),
                    runtime_id: runtime().id,
                    runtime_name: "runtime-a".into(),
                    entries: vec![log()],
                }),
                Value::RuntimeLogs {
                    revision: Revision(7),
                    runtime_id: runtime().id,
                    runtime_name: "runtime-a".into(),
                    entries: vec![log()],
                },
            ),
            4 => (
                EffectResult::Command(Box::new(command.clone())),
                Value::RuntimeRefresh {
                    result: Box::new(command),
                },
            ),
            5 => (
                EffectResult::Command(Box::new(command.clone())),
                Value::RuntimeCapabilitiesRefresh {
                    result: Box::new(command),
                },
            ),
            6 => (
                EffectResult::Command(Box::new(command.clone())),
                Value::RuntimeDeploy {
                    result: Box::new(command),
                },
            ),
            7 => {
                let result = DebuggerCancelResult {
                    command_id: command.command_id,
                    session_id: "session-a".into(),
                    observed_at_ms: 11,
                };
                (
                    EffectResult::DebuggerCancel(result.clone()),
                    Value::DebuggerCancel { result },
                )
            }
            _ => unreachable!(),
        };
        Case {
            request,
            reply,
            value,
        }
    })
    .collect()
}

// Frozen pre-composition projection, including legacy query/GUI policy.
fn legacy_atomic_step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    if let EffectResult::Presentation(reply) = result {
        if !reply.matches_effect(&image.pending_effect, operation) {
            return fault("LSV2103", "effect result does not match pending effect");
        }
        let value = Value::from(reply);
        if value.result_type() != image.result_type {
            return fault("LSV2103", "effect result does not match pending effect");
        }
        return Step::Done(value);
    }
    if leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .map(|operation| operation.result_type())
        != Some(image.result_type)
        || !result.matches_control_plane_effect(&image.pending_effect, operation)
    {
        return fault("LSV2103", "effect result does not match pending effect");
    }
    let query_bounds = match &result {
        EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
            Some((*revision, Some((runtimes.len(), image.max_output_items))))
        }
        EffectResult::Query(QueryResult::RuntimeInspect { revision, .. }) => {
            Some((*revision, None))
        }
        EffectResult::Query(QueryResult::RuntimeHistory { revision, entries }) => {
            Some((*revision, Some((entries.len(), image.max_output_items))))
        }
        EffectResult::Query(QueryResult::RuntimeLogs {
            revision, entries, ..
        }) => Some((
            *revision,
            Some((
                entries.len(),
                image
                    .max_output_items
                    .min(usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES)),
            )),
        )),
        _ => None,
    };
    if let Some((revision, output)) = query_bounds {
        if let Some(expected) = image.expected_revision
            && expected != revision
        {
            return fault(
                "LSV2101",
                format!(
                    "effect revision conflict: expected {}, actual {}",
                    expected.0, revision.0
                ),
            );
        }
        if let Some((count, limit)) = output
            && count > limit
        {
            return fault(
                "LSV2102",
                format!("effect returned {count} items, limit is {limit}"),
            );
        }
    }
    match result.into_control_plane_value(image.result_type) {
        Some(value) => Step::Done(value),
        None => fault("LSV2103", "effect result does not match pending effect"),
    }
}

fn mismatch() -> Step {
    fault("LSV2103", "effect result does not match pending effect")
}

fn project(case: &Case, reply: EffectResult) -> Step {
    atomic_step_from_effect_result(
        &case.request.continuation,
        Some(&case.request.operation),
        reply,
    )
}

#[test]
fn composed_reply_conversion_matches_frozen_native_wire_and_refusal_order() {
    let mut cases = cases();
    for (source, reply) in [
        (
            r#"ui.focus(node_id: "a")"#,
            PresentationResult::Focus {
                node_id: "a".into(),
            },
        ),
        (
            r#"ui.wait_text(node_id: "a", expected: "ready")"#,
            PresentationResult::WaitText {
                node_id: "a".into(),
                expected: "ready".into(),
                timeout_ms: leselang_hir::UI_WAIT_TEXT_TIMEOUT_MS,
            },
        ),
        (
            r#"ui.navigate_focus(node_id: "a", direction: "next")"#,
            PresentationResult::NavigateFocus {
                node_id: "a".into(),
                focused_node_id: "b".into(),
                direction: leselang_hir::UiFocusNavigationDirection::Next,
            },
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let request = Vm::build_effect_request(
            &program.function.effect,
            program.function.result_type,
            99,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7)),
            30_000,
            None,
            21,
        )
        .unwrap();
        cases.push(Case {
            request,
            value: Value::from(reply.clone()),
            reply: EffectResult::Presentation(reply),
        });
    }
    for case in &cases {
        for candidate in &cases {
            for saved in [
                None,
                Some(&case.request.operation),
                Some(&candidate.request.operation),
            ] {
                for expected in [None, Some(Revision(7)), Some(Revision(8))] {
                    for limit in [0, 1, usize::MAX] {
                        for result_type in [
                            case.request.continuation.result_type,
                            Type::Structured,
                            Type::UiFocus,
                        ] {
                            let mut image = case.request.continuation.clone();
                            image.expected_revision = expected;
                            image.max_output_items = limit;
                            image.result_type = result_type;
                            let before = serde_json::to_vec(&image).unwrap();
                            let original = legacy_atomic_step_from_effect_result(
                                &image,
                                saved,
                                candidate.reply.clone(),
                            );
                            let current = atomic_step_from_effect_result(
                                &image,
                                saved,
                                candidate.reply.clone(),
                            );
                            assert_eq!(current, original);
                            assert_eq!(
                                serde_json::to_vec(&current).unwrap(),
                                serde_json::to_vec(&original).unwrap()
                            );
                            assert_eq!(serde_json::to_vec(&image).unwrap(), before);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn control_plane_replies_freeze_cross_category_type_and_legacy_image_only_matrix() {
    let cases = cases();
    for (index, case) in cases.iter().enumerate() {
        for (other_index, other) in cases.iter().enumerate() {
            assert_eq!(
                project(case, other.reply.clone()),
                if index == other_index {
                    Step::Done(case.value.clone())
                } else {
                    mismatch()
                }
            );
        }
        assert_eq!(
            atomic_step_from_effect_result(&case.request.continuation, None, case.reply.clone(),),
            if index < 4 {
                Step::Done(case.value.clone())
            } else {
                mismatch()
            }
        );
        for other in &cases {
            if other.request.continuation.result_type == case.request.continuation.result_type {
                continue;
            }
            let mut image = case.request.continuation.clone();
            image.result_type = other.request.continuation.result_type;
            assert_eq!(
                atomic_step_from_effect_result(
                    &image,
                    Some(&case.request.operation),
                    case.reply.clone(),
                ),
                mismatch()
            );
        }
        assert_eq!(
            project(
                case,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "node-a".into()
                })
            ),
            mismatch()
        );
        let wire = serde_json::to_vec(&case.reply).unwrap();
        let reply = serde_json::from_slice(&wire).unwrap();
        assert_eq!(
            serde_json::to_vec(&project(case, reply)).unwrap(),
            serde_json::to_vec(&Step::Done(case.value.clone())).unwrap()
        );
    }
}

#[test]
fn control_plane_reply_fault_order_is_correspondence_then_revision_then_item_limit() {
    for mut case in cases().into_iter().take(4) {
        case.request.continuation.max_output_items = 1;
        let mut bad_revision = case.reply.clone();
        match &mut bad_revision {
            EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                *revision = Revision(8);
                runtimes.push(runtime());
            }
            EffectResult::Query(QueryResult::RuntimeInspect { revision, .. }) => {
                *revision = Revision(8)
            }
            EffectResult::Query(QueryResult::RuntimeHistory { revision, entries }) => {
                *revision = Revision(8);
                entries.push(entries[0].clone());
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                revision, entries, ..
            }) => {
                *revision = Revision(8);
                entries.push(log());
            }
            _ => unreachable!(),
        }
        assert_eq!(
            project(&case, bad_revision.clone()),
            fault("LSV2101", "effect revision conflict: expected 7, actual 8",)
        );
        let mut wrong_id = bad_revision.clone();
        match &mut wrong_id {
            EffectResult::Query(QueryResult::RuntimeList { .. }) => continue,
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => {
                runtime.id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries[1].runtime.id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Query(QueryResult::RuntimeLogs { runtime_id, .. }) => {
                *runtime_id = RuntimeId::new("other").unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(project(&case, wrong_id), mismatch());
    }
    for index in [0, 2, 3] {
        let mut case = cases().remove(index);
        case.request.continuation.expected_revision = None;
        for (limit, count) in [(1, 0), (1, 1), (1, 2), (1000, 256), (1000, 257)] {
            case.request.continuation.max_output_items = limit;
            let mut reply = case.reply.clone();
            match &mut reply {
                EffectResult::Query(QueryResult::RuntimeList { runtimes, .. }) => {
                    runtimes.resize(count, runtime())
                }
                EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                    entries.resize(count, entries[0].clone())
                }
                EffectResult::Query(QueryResult::RuntimeLogs { entries, .. }) => {
                    entries.resize(count, log())
                }
                _ => unreachable!(),
            }
            let result = project(&case, reply);
            let limit = if index == 3 { limit.min(256) } else { limit };
            if count > limit {
                assert_eq!(
                    result,
                    fault(
                        "LSV2102",
                        format!("effect returned {count} items, limit is {limit}")
                    )
                );
            } else {
                assert!(matches!(result, Step::Done(_)));
            }
        }
    }
}

#[test]
fn control_plane_reply_identity_status_and_timestamp_rules_are_preserved() {
    for case in cases().into_iter().skip(4) {
        let mut wrong_id = case.reply.clone();
        match &mut wrong_id {
            EffectResult::Command(result) => result.command_id = CommandId::new("other").unwrap(),
            EffectResult::DebuggerCancel(result) => {
                result.command_id = CommandId::new("other").unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(project(&case, wrong_id), mismatch());
        let mut torn = case.reply.clone();
        match &mut torn {
            EffectResult::Command(result) => {
                result.runtime.id = RuntimeId::new("other").unwrap();
                assert_eq!(project(&case, torn.clone()), mismatch());
                let EffectResult::Command(result) = &mut torn else {
                    unreachable!()
                };
                result.runtime.id = runtime().id;
                result.status = CommandStatus::Planned;
                assert_eq!(project(&case, torn), mismatch());
            }
            EffectResult::DebuggerCancel(result) => {
                result.session_id = "other".into();
                assert_eq!(project(&case, torn.clone()), mismatch());
                let EffectResult::DebuggerCancel(result) = &mut torn else {
                    unreachable!()
                };
                result.session_id = "session-a".into();
                result.observed_at_ms = i64::MAX as u64;
                assert!(matches!(project(&case, torn.clone()), Step::Done(_)));
                let EffectResult::DebuggerCancel(result) = &mut torn else {
                    unreachable!()
                };
                result.observed_at_ms += 1;
                assert_eq!(project(&case, torn), mismatch());
            }
            _ => unreachable!(),
        }
        let mut image = case.request.continuation.clone();
        image.expected_revision = Some(Revision(99));
        assert_eq!(
            atomic_step_from_effect_result(
                &image,
                Some(&case.request.operation),
                case.reply.clone()
            ),
            Step::Done(case.value.clone())
        );
    }
}

#[test]
fn control_plane_projection_keeps_query_revision_and_cardinality_policy_matrix() {
    for (index, case) in cases().into_iter().take(4).enumerate() {
        for expected in [None, Some(Revision(7))] {
            for revision in [Revision(0), Revision(7), Revision(8)] {
                for limit in [1, 256, 1000] {
                    for count in [0, 1, 2, 256, 257] {
                        let mut image = case.request.continuation.clone();
                        image.expected_revision = expected;
                        image.max_output_items = limit;
                        let mut reply = case.reply.clone();
                        match &mut reply {
                            EffectResult::Query(QueryResult::RuntimeList {
                                revision: actual,
                                runtimes,
                            }) => {
                                *actual = revision;
                                runtimes.resize(count, runtime());
                            }
                            EffectResult::Query(QueryResult::RuntimeInspect {
                                revision: actual,
                                ..
                            }) => *actual = revision,
                            EffectResult::Query(QueryResult::RuntimeHistory {
                                revision: actual,
                                entries,
                            }) => {
                                *actual = revision;
                                entries.resize(count, entries[0].clone());
                            }
                            EffectResult::Query(QueryResult::RuntimeLogs {
                                revision: actual,
                                entries,
                                ..
                            }) => {
                                *actual = revision;
                                entries.resize(count, log());
                            }
                            _ => unreachable!(),
                        }
                        let step = atomic_step_from_effect_result(
                            &image,
                            Some(&case.request.operation),
                            reply,
                        );
                        let effective_limit = if index == 3 { limit.min(256) } else { limit };
                        if expected.is_some_and(|expected| expected != revision) {
                            assert_eq!(
                                step,
                                fault(
                                    "LSV2101",
                                    format!(
                                        "effect revision conflict: expected 7, actual {}",
                                        revision.0,
                                    )
                                )
                            );
                        } else if index != 1 && count > effective_limit {
                            assert_eq!(
                                step,
                                fault(
                                    "LSV2102",
                                    format!(
                                        "effect returned {count} items, limit is {effective_limit}",
                                    )
                                )
                            );
                        } else {
                            let Step::Done(value) = step else {
                                panic!("query policy rejected {step:?}");
                            };
                            assert_eq!(value.result_type(), image.result_type);
                            let (actual, actual_count) = match value {
                                Value::RuntimeList { revision, runtimes } => {
                                    (revision, runtimes.len())
                                }
                                Value::RuntimeInspect { revision, .. } => (revision, 1),
                                Value::RuntimeHistory { revision, entries } => {
                                    (revision, entries.len())
                                }
                                Value::RuntimeLogs {
                                    revision, entries, ..
                                } => (revision, entries.len()),
                                _ => unreachable!(),
                            };
                            assert_eq!(actual, revision);
                            assert_eq!(actual_count, if index == 1 { 1 } else { count });
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn control_plane_projection_moves_original_collections_strings_and_command_boxes() {
    for case in cases() {
        let before: Vec<usize> = match &case.reply {
            EffectResult::Query(QueryResult::RuntimeList { runtimes, .. }) => {
                vec![runtimes.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                vec![entries.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                runtime_name,
                entries,
                ..
            }) => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            EffectResult::Command(result) => vec![&**result as *const CommandResult as usize],
            EffectResult::DebuggerCancel(result) => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        let Step::Done(value) = atomic_step_from_effect_result(
            &case.request.continuation,
            Some(&case.request.operation),
            case.reply,
        ) else {
            panic!("valid reply projection failed");
        };
        let after: Vec<usize> = match &value {
            Value::RuntimeList { runtimes, .. } => vec![runtimes.as_ptr() as usize],
            Value::RuntimeInspect { runtime, .. } => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            Value::RuntimeHistory { entries, .. } => vec![entries.as_ptr() as usize],
            Value::RuntimeLogs {
                runtime_name,
                entries,
                ..
            } => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            Value::RuntimeRefresh { result }
            | Value::RuntimeCapabilitiesRefresh { result }
            | Value::RuntimeDeploy { result } => vec![&**result as *const CommandResult as usize],
            Value::DebuggerCancel { result } => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        assert_eq!(after, before);
        assert_eq!(value, case.value);
    }
}

fn dispatch_round_trip(vm: &mut Vm, case: &Case) -> (DispatchLease, Step) {
    vm.restore_request(case.request.clone()).unwrap();
    let lease = vm.claim_effect(10, 100).unwrap().unwrap();
    assert_eq!(lease.request, case.request);
    let done = vm.acknowledge_effect(&lease, 11, case.reply.clone());
    assert_eq!(done, Step::Done(case.value.clone()));
    assert_eq!(
        vm.acknowledge_effect(
            &lease,
            12,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "wrong".into()
            })
        ),
        done
    );
    (lease, done)
}

#[test]
fn control_plane_replies_keep_dispatch_only_mutations_and_once_terminal_replay() {
    let mut vm = Vm::default();
    for (index, case) in cases().iter().enumerate() {
        if index >= 4 {
            vm.restore_request(case.request.clone()).unwrap();
            assert_eq!(
                vm.resume(&case.request.continuation, case.reply.clone()),
                fault(
                    "LSV2110",
                    "mutating effects must be completed through dispatch acknowledgement",
                )
            );
            assert_eq!(vm.pending_count(), 1);
        }
        dispatch_round_trip(&mut vm, case);
    }
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 8);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn control_plane_replies_keep_journal_restart_and_changed_receipt_replay() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Temp(std::env::temp_dir().join(format!(
        "leselang-control-replies-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    )));
    std::fs::create_dir(&dir.0).unwrap();
    let path = dir.0.join("effects.sqlite3");
    let mut vm = Vm::open_journal(&path, DEFAULT_FUEL).unwrap();
    let mut completed = vec![];
    for case in cases() {
        completed.push(dispatch_round_trip(&mut vm, &case));
    }
    drop(vm);
    let mut reopened = Vm::open_journal(&path, DEFAULT_FUEL).unwrap();
    assert_eq!(reopened.pending_count(), 0);
    assert_eq!(reopened.completed_count(), 8);
    for (lease, done) in completed {
        assert_eq!(
            reopened.acknowledge_effect(
                &lease,
                13,
                EffectResult::Query(QueryResult::RuntimeList {
                    revision: Revision(999),
                    runtimes: vec![]
                })
            ),
            done
        );
    }
}
