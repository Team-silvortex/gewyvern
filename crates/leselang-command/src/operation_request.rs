//! Product request correspondence, independent of continuation and journal ownership.

use std::fmt;

use leselang_host_contract::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Principal, Revision,
};
use leserpent_domain::DOMAIN_SCHEMA_VERSION;
use leserpent_hir::{CAPABILITY_UI_PRESENTATION, Effect};

use crate::{EffectOperation, command_matches_effect, query_matches_effect};

/// Borrowed expectations from a separately admitted original continuation.
/// This view does not validate their provenance or manufacture identities.
pub struct OperationRequestContext<'a> {
    pub effect: &'a Effect,
    pub required_capability: &'a str,
    pub expected_command_id: &'a str,
    pub expected_idempotency_key: &'a str,
    pub expected_revision: Option<Revision>,
}

/// Payload-free product refusals. The VM retains its released fault-code mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationRequestError {
    Capability,
    Envelope,
    Mismatch,
}

impl fmt::Display for OperationRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Capability => "effect request capability is inconsistent",
            Self::Envelope => "effect request envelope is invalid",
            Self::Mismatch => "effect request does not match its continuation",
        })
    }
}
impl std::error::Error for OperationRequestError {}

impl EffectOperation {
    /// Borrow declared authority, not an authenticated or currently valid grant.
    pub fn authority(&self) -> (&Principal, &CapabilitySet) {
        match self {
            Self::Query(envelope) => (&envelope.principal, &envelope.capabilities),
            Self::Command(envelope) => (&envelope.principal, &envelope.capabilities),
            Self::Presentation(envelope) => (&envelope.principal, &envelope.capabilities),
        }
    }

    /// Check category, declared capability, envelope identity, then payload.
    ///
    /// Cross-category requests fail as mismatches without reading their envelope
    /// policy. This is borrowed correspondence only: no cloning, normalization,
    /// lowering, dispatch, fuel debit or journal access. Success does not validate
    /// source/frame structure, live authority, successor grants or resource budgets.
    /// Callers must bound ingress and admit the original effect before this phase.
    pub fn validate_request(
        &self,
        context: OperationRequestContext<'_>,
    ) -> Result<(), OperationRequestError> {
        let operation_valid = match (context.effect, self) {
            (
                effect @ (Effect::RuntimeList { .. }
                | Effect::RuntimeInspect { .. }
                | Effect::RuntimeHistory { .. }
                | Effect::RuntimeLogs { .. }),
                EffectOperation::Query(query),
            ) => {
                validate_effect_identity(
                    query.schema_version,
                    &query.principal,
                    &query.capabilities,
                    context.required_capability,
                    CAPABILITY_RUNTIME_READ,
                )?;
                query_matches_effect(&query.query, effect)
            }
            (
                effect
                @ (Effect::RuntimeRefresh { .. } | Effect::RuntimeCapabilitiesRefresh { .. }),
                EffectOperation::Command(command),
            ) => {
                validate_effect_identity(
                    command.schema_version,
                    &command.principal,
                    &command.capabilities,
                    context.required_capability,
                    CAPABILITY_RUNTIME_REFRESH,
                )?;
                command_matches_effect(
                    command,
                    effect,
                    context.expected_command_id,
                    context.expected_idempotency_key,
                    context.expected_revision,
                )
            }
            (effect @ Effect::RuntimeDeploy { .. }, EffectOperation::Command(command)) => {
                validate_effect_identity(
                    command.schema_version,
                    &command.principal,
                    &command.capabilities,
                    context.required_capability,
                    CAPABILITY_RUNTIME_DEPLOY,
                )?;
                command_matches_effect(
                    command,
                    effect,
                    context.expected_command_id,
                    context.expected_idempotency_key,
                    context.expected_revision,
                )
            }
            (effect @ Effect::DebuggerCancel { .. }, EffectOperation::Command(command)) => {
                validate_effect_identity(
                    command.schema_version,
                    &command.principal,
                    &command.capabilities,
                    context.required_capability,
                    CAPABILITY_DEBUGGER_CONTROL,
                )?;
                command_matches_effect(
                    command,
                    effect,
                    context.expected_command_id,
                    context.expected_idempotency_key,
                    context.expected_revision,
                )
            }
            (effect, EffectOperation::Presentation(presentation))
                if leserpent_hir::host_call::HostOperation::for_effect(effect).is_some_and(
                    |operation| operation.required_capability() == CAPABILITY_UI_PRESENTATION,
                ) =>
            {
                validate_effect_identity(
                    presentation.schema_version,
                    &presentation.principal,
                    &presentation.capabilities,
                    context.required_capability,
                    CAPABILITY_UI_PRESENTATION,
                )?;
                presentation.operation.matches_effect(effect)
            }
            _ => false,
        };
        if operation_valid {
            Ok(())
        } else {
            Err(OperationRequestError::Mismatch)
        }
    }
}

fn validate_effect_identity(
    schema_version: u32,
    principal: &Principal,
    capabilities: &CapabilitySet,
    actual_capability: &str,
    expected_capability: &str,
) -> Result<(), OperationRequestError> {
    if actual_capability != expected_capability || !capabilities.contains(actual_capability) {
        return Err(OperationRequestError::Capability);
    }
    if schema_version != DOMAIN_SCHEMA_VERSION
        || principal.id.is_empty()
        || principal.id.len() > 128
        || !principal
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(OperationRequestError::Envelope);
    }
    Ok(())
}
