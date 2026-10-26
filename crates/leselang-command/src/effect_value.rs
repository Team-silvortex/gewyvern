//! Released atomic reply checks and consuming projection, without VM ownership.

use std::fmt;

use leselang_host_contract::Revision;
use leserpent_domain::{MAX_RUNTIME_LOG_QUERY_ENTRIES, QueryResult};
use leserpent_hir::{Effect, Type, host_call::HostOperation};

use crate::{EffectOperation, EffectResult, Value};

/// Original execution data, not an authenticated receipt or admission certificate.
///
/// The host must obtain all fields from its admitted pending operation, never
/// infer the declaration, expected revision or command identity from the reply.
/// The optional operation retains legacy image-only query/GUI correspondence;
/// mutations still require the original saved command envelope.
#[derive(Clone, Copy)]
pub struct EffectValueContext<'a> {
    pub effect: &'a Effect,
    pub result_type: Type,
    pub operation: Option<&'a EffectOperation>,
    pub expected_revision: Option<Revision>,
    pub max_output_items: usize,
}

impl fmt::Debug for EffectValueContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EffectValueContext")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectValueError {
    Mismatch,
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    OutputLimit {
        count: usize,
        limit: usize,
    },
}

impl fmt::Display for EffectValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mismatch => "effect reply does not match its declaration",
            Self::RevisionConflict { .. } => "effect reply revision conflict",
            Self::OutputLimit { .. } => "effect reply exceeds its item limit",
        })
    }
}

impl std::error::Error for EffectValueError {}

impl EffectResult {
    /// Compose the released atomic reply checks before moving its payload to a value.
    ///
    /// GUI correspondence precedes conversion and the saved result-type check.
    /// Control-plane signature/correspondence precedes query revision, query item
    /// limits and conversion. Logs keep their fixed product ceiling; inspect keeps
    /// its non-list policy. GUI/command replies do not acquire new revision/item
    /// checks here. Empty list/history replies may pass a zero item ceiling.
    ///
    /// This consumes the reply on success and refusal, without cloning, retries,
    /// serialization or new fuel. Numeric errors contain only revision/count
    /// metadata. Full domain/value/byte bounds, source/frame authenticity, live
    /// authority, deadlines, leases, dispatch and durable replay are separate host
    /// gates. Supplied context and successful conversion grant no authority.
    pub fn into_effect_value(
        self,
        context: EffectValueContext<'_>,
    ) -> Result<Value, EffectValueError> {
        if let Self::Presentation(reply) = self {
            if !reply.matches_effect(context.effect, context.operation) {
                return Err(EffectValueError::Mismatch);
            }
            let value = Value::from(reply);
            if value.result_type() != context.result_type {
                return Err(EffectValueError::Mismatch);
            }
            return Ok(value);
        }
        if HostOperation::for_effect(context.effect).map(|operation| operation.result_type())
            != Some(context.result_type)
            || !self.matches_control_plane_effect(context.effect, context.operation)
        {
            return Err(EffectValueError::Mismatch);
        }
        let query_bounds = match &self {
            Self::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                Some((*revision, Some((runtimes.len(), context.max_output_items))))
            }
            Self::Query(QueryResult::RuntimeInspect { revision, .. }) => Some((*revision, None)),
            Self::Query(QueryResult::RuntimeHistory { revision, entries }) => {
                Some((*revision, Some((entries.len(), context.max_output_items))))
            }
            Self::Query(QueryResult::RuntimeLogs {
                revision, entries, ..
            }) => Some((
                *revision,
                Some((
                    entries.len(),
                    context
                        .max_output_items
                        .min(usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES)),
                )),
            )),
            _ => None,
        };
        if let Some((revision, output)) = query_bounds {
            if let Some(expected) = context.expected_revision
                && expected != revision
            {
                return Err(EffectValueError::RevisionConflict {
                    expected,
                    actual: revision,
                });
            }
            if let Some((count, limit)) = output
                && count > limit
            {
                return Err(EffectValueError::OutputLimit { count, limit });
            }
        }
        self.into_control_plane_value(context.result_type)
            .ok_or(EffectValueError::Mismatch)
    }
}
