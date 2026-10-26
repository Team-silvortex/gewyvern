//! Borrowed correspondence for released Leserpent control-plane requests.

use leselang_host_contract::{CommandOrigin, Confirmation, Revision};
use leserpent_domain::{Command, CommandEnvelope, MAX_RUNTIME_LOG_QUERY_ENTRIES, Query};
use leserpent_hir::Effect;

/// Match the original query payload, including the language's fixed log policy.
///
/// This is correspondence, not full domain validation or authority. The caller
/// must check the operation category, schema, principal, grants and execution
/// state separately. No allocation, serialization, canonicalization or dispatch
/// occurs here; native values retain their existing matching rules.
pub fn query_matches_effect(query: &Query, effect: &Effect) -> bool {
    match (effect, query) {
        (Effect::RuntimeList { filter: expected }, Query::RuntimeList { filter }) => {
            filter == expected
        }
        (
            Effect::RuntimeInspect {
                runtime_id: expected,
            },
            Query::RuntimeInspect { runtime_id },
        )
        | (
            Effect::RuntimeHistory {
                runtime_id: expected,
            },
            Query::RuntimeHistory { runtime_id },
        ) => runtime_id == expected,
        (
            Effect::RuntimeLogs {
                runtime_id: expected,
            },
            Query::RuntimeLogs {
                runtime_id,
                after_sequence: None,
                limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
            },
        ) => runtime_id == expected,
        _ => false,
    }
}

/// Match a command against its pending effect and the caller's trusted expectations.
///
/// Identity strings and revision must come from the original execution, not from
/// the received envelope. This checks correspondence only: it neither generates
/// identities nor authenticates a principal, checks grants, admits a continuation,
/// or provides replay protection. Schema/authority/canonical and budget checks
/// remain separate. All inputs are borrowed and no execution fuel is consumed.
pub fn command_matches_effect(
    command: &CommandEnvelope,
    effect: &Effect,
    expected_command_id: &str,
    expected_idempotency_key: &str,
    expected_revision: Option<Revision>,
) -> bool {
    command.command_id.as_str() == expected_command_id
        && command.idempotency_key.as_str() == expected_idempotency_key
        && command.expected_revision == expected_revision
        && command.origin == CommandOrigin::Leselang
        && !command.dry_run
        && match (effect, &command.command) {
            (
                Effect::RuntimeRefresh {
                    runtime_id: expected,
                },
                Command::RuntimeRefresh { runtime_id },
            )
            | (
                Effect::RuntimeCapabilitiesRefresh {
                    runtime_id: expected,
                },
                Command::RuntimeCapabilitiesRefresh { runtime_id },
            ) => command.confirmation == Confirmation::NotRequired && runtime_id == expected,
            (
                Effect::RuntimeDeploy {
                    runtime_id: expected_runtime_id,
                    pipeline_kind: expected_pipeline_kind,
                    target: expected_target,
                },
                Command::RuntimeDeploy {
                    runtime_id,
                    pipeline_kind,
                    target,
                },
            ) => {
                command.confirmation == Confirmation::Confirmed
                    && runtime_id == expected_runtime_id
                    && pipeline_kind == expected_pipeline_kind
                    && target == expected_target
            }
            (
                Effect::DebuggerCancel {
                    session_id: expected,
                },
                Command::DebuggerCancel { session_id },
            ) => command.confirmation == Confirmation::Confirmed && session_id == expected,
            _ => false,
        }
}
