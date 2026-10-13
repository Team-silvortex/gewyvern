//! Borrowed correspondence for released Leserpent control-plane replies.

use leselang_hir::Effect;
use leserpent_domain::{CommandStatus, QueryResult};

use crate::{EffectOperation, EffectResult};

impl EffectResult {
    /// Check reply payload correspondence against the original pending operation.
    ///
    /// Queries retain legacy image-only matching. Mutations require the saved
    /// command ID, never an expectation derived from the received reply. The
    /// caller must already have admitted that envelope and authenticated delivery.
    /// This is not type, revision, output-budget, domain-record or replay validation.
    /// All inputs are borrowed; no payload is copied, converted or dispatched.
    pub fn matches_control_plane_effect(
        &self,
        effect: &Effect,
        saved_operation: Option<&EffectOperation>,
    ) -> bool {
        match (effect, self, saved_operation) {
            (Effect::RuntimeList { .. }, Self::Query(QueryResult::RuntimeList { .. }), _) => true,
            (
                Effect::RuntimeInspect { runtime_id },
                Self::Query(QueryResult::RuntimeInspect { runtime, .. }),
                _,
            ) => runtime.id == *runtime_id,
            (
                Effect::RuntimeHistory { runtime_id },
                Self::Query(QueryResult::RuntimeHistory { entries, .. }),
                _,
            ) => entries.iter().all(|entry| entry.runtime.id == *runtime_id),
            (
                Effect::RuntimeLogs { runtime_id },
                Self::Query(QueryResult::RuntimeLogs {
                    runtime_id: actual, ..
                }),
                _,
            ) => actual == runtime_id,
            (
                Effect::RuntimeRefresh { runtime_id }
                | Effect::RuntimeCapabilitiesRefresh { runtime_id }
                | Effect::RuntimeDeploy { runtime_id, .. },
                Self::Command(result),
                Some(EffectOperation::Command(command)),
            ) => {
                result.runtime.id == *runtime_id
                    && result.command_id == command.command_id
                    && result.status == CommandStatus::Applied
            }
            (
                Effect::DebuggerCancel { session_id },
                Self::DebuggerCancel(result),
                Some(EffectOperation::Command(command)),
            ) => {
                result.session_id == *session_id
                    && result.command_id == command.command_id
                    && result.observed_at_ms <= i64::MAX as u64
            }
            _ => false,
        }
    }
}
