//! Owned control-plane reply projection for the released Leserpent value format.

use leselang_hir::Type;
use leserpent_domain::QueryResult;

use crate::{EffectResult, Value};

impl EffectResult {
    /// Move a control-plane reply into the caller's declared result shape.
    ///
    /// The declared type must come from the original execution, not from the
    /// received reply. A generic command result carries no operation tag.
    /// This conversion does not check correspondence, authority, revisions,
    /// status, resource limits or replay state. Validate those before conversion.
    /// A wrong category/type is consumed and returns None; GUI replies use their
    /// separate From conversion. Inspect retains its released boxed shape; other
    /// payload owners move unchanged, without cloning or serialization.
    pub fn into_control_plane_value(self, result_type: Type) -> Option<Value> {
        Some(match (result_type, self) {
            (Type::RuntimeList, Self::Query(QueryResult::RuntimeList { revision, runtimes })) => {
                Value::RuntimeList { revision, runtimes }
            }
            (
                Type::RuntimeInspect,
                Self::Query(QueryResult::RuntimeInspect { revision, runtime }),
            ) => Value::RuntimeInspect {
                revision,
                runtime: Box::new(runtime),
            },
            (
                Type::RuntimeHistory,
                Self::Query(QueryResult::RuntimeHistory { revision, entries }),
            ) => Value::RuntimeHistory { revision, entries },
            (
                Type::RuntimeLogs,
                Self::Query(QueryResult::RuntimeLogs {
                    revision,
                    runtime_id,
                    runtime_name,
                    entries,
                }),
            ) => Value::RuntimeLogs {
                revision,
                runtime_id,
                runtime_name,
                entries,
            },
            (Type::RuntimeRefresh, Self::Command(result)) => Value::RuntimeRefresh { result },
            (Type::RuntimeCapabilitiesRefresh, Self::Command(result)) => {
                Value::RuntimeCapabilitiesRefresh { result }
            }
            (Type::RuntimeDeploy, Self::Command(result)) => Value::RuntimeDeploy { result },
            (Type::DebuggerCancel, Self::DebuggerCancel(result)) => {
                Value::DebuggerCancel { result }
            }
            _ => return None,
        })
    }
}
