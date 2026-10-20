//! Closed product envelope decoding, separate from runtime request admission.

use leselang_runtime_core::ResourceBudget;
use leserpent_domain::QueryEnvelope;
use serde::{Deserialize, Deserializer};

use crate::EffectOperation;

/// Decoded, untrusted product data. This is not an admitted execution request.
///
/// The caller owns the continuation representation and must validate the original
/// frame, operation, identity and budgets before use. No token traits beyond
/// deserialization are required; values are moved without cloning or conversion.
pub struct DecodedEffectRequest<Continuation> {
    pub effect_id: String,
    pub required_capability: String,
    pub operation: EffectOperation,
    pub continuation: Continuation,
    pub budget: ResourceBudget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentEffectRequestWire<Continuation> {
    effect_id: String,
    required_capability: String,
    operation: EffectOperation,
    continuation: Continuation,
    budget: ResourceBudget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyQueryEffectRequestWire<Continuation> {
    effect_id: String,
    required_capability: String,
    query: QueryEnvelope,
    continuation: Continuation,
    budget: ResourceBudget,
}

// Keep the released untagged enum name and branch order in serde diagnostics.
#[derive(Deserialize)]
#[serde(untagged)]
enum EffectRequestWire<Continuation> {
    Current(CurrentEffectRequestWire<Continuation>),
    LegacyQuery(LegacyQueryEffectRequestWire<Continuation>),
}

/// Decode the current operation envelope or its legacy query-only form.
///
/// Unknown, missing, duplicate and mixed current/legacy fields are refused by the
/// same closed serde shapes as the released VM. Legacy queries become the current
/// `EffectOperation::Query` without changing their contents.
///
/// This does not bound input bytes, validate semantic budgets/frame provenance,
/// authorize work, allocate identities, restore a session or grant replay rights.
/// Callers must bound ingress and apply their normal full request admission.
/// One value is decoded; a caller supplying a streaming deserializer must also
/// enforce its framing/end-of-input policy before accepting that value.
pub fn decode_effect_request<'de, D, Continuation>(
    deserializer: D,
) -> Result<DecodedEffectRequest<Continuation>, D::Error>
where
    D: Deserializer<'de>,
    Continuation: Deserialize<'de>,
{
    Ok(match EffectRequestWire::deserialize(deserializer)? {
        EffectRequestWire::Current(CurrentEffectRequestWire {
            effect_id,
            required_capability,
            operation,
            continuation,
            budget,
        }) => DecodedEffectRequest {
            effect_id,
            required_capability,
            operation,
            continuation,
            budget,
        },
        EffectRequestWire::LegacyQuery(LegacyQueryEffectRequestWire {
            effect_id,
            required_capability,
            query,
            continuation,
            budget,
        }) => DecodedEffectRequest {
            effect_id,
            required_capability,
            operation: EffectOperation::Query(query),
            continuation,
            budget,
        },
    })
}
