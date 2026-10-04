use serde::{Deserialize, Serialize};

use crate::{ScalarContractError, ScalarType, ScalarTypeSet, ScalarValue};

/// One scalar projection with an opaque, developer-defined field key.
///
/// This is mutable data, not an operation receipt, capability or validation
/// certificate. Constructors and serde do not validate its schema; scalar
/// decoding retains its existing bounds. `Debug`/serialization contain payloads
/// and are not redacted. Hosts must bound ingress bytes and field counts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarProjectionField<Field> {
    pub field: Field,
    pub value: ScalarValue,
}

/// Fixed, payload-free projection diagnostics; hosts own their error-code mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    FieldCount,
    FieldKey,
    FieldType,
    UnboundedValue,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::FieldCount => "projection field count does not match schema",
            Self::FieldKey => "projection field key does not match schema",
            Self::FieldType => "projection scalar type does not match schema",
            Self::UnboundedValue => "projection scalar exceeds language bounds",
        })
    }
}

impl std::error::Error for ProjectionError {}

/// Validate exact ordered keys, scalar types and per-value language bounds.
///
/// The schema is trusted adapter metadata: the host selects its operation/version,
/// defines distinct keys and applies domain/authority/replay checks separately.
/// Keys are compared without cloning, formatting or serializing; values are only
/// borrowed. A schema iterator need not be cloneable or exact-sized. At most
/// `fields.len() + 1` calls to `next` are made, without reading its size hint or
/// collecting a second schema. Its callbacks/key comparisons are trusted native
/// code, not sandboxed or preempted. Mismatches are reported left to right.
/// The validator does not mutate fields; native callback/interior-mutability
/// side effects are not rolled back, including on unwind.
///
/// Success describes these data at this instant, not later mutations, total
/// memory/CPU bounds, raw-host-result authenticity or permission to execute.
/// No values are normalized, reordered, truncated or filled in on either path.
///
/// ```
/// use leselang_runtime_core::{ScalarProjectionField, ScalarType, ScalarValue,
///     validate_scalar_projection};
/// #[derive(PartialEq)]
/// enum Field { Caption, Enabled }
/// let schema = [(Field::Caption, ScalarType::String), (Field::Enabled, ScalarType::Boolean)];
/// let fields = [
///     ScalarProjectionField { field: Field::Caption, value: ScalarValue::String("ready".into()) },
///     ScalarProjectionField { field: Field::Enabled, value: ScalarValue::Boolean(true) },
/// ];
/// validate_scalar_projection(&fields, schema.iter().map(|(key, ty)| (key, *ty))).unwrap();
/// ```
///
/// ```
/// use leselang_runtime_core::{ProjectionError, ScalarProjectionField,
///     ScalarType, ScalarValue, validate_scalar_projection};
/// let schema = [("caption", ScalarType::String)];
/// let mut fields = [ScalarProjectionField { field: "caption", value: ScalarValue::String("ok".into()) }];
/// validate_scalar_projection(&fields, schema.iter().map(|(key, ty)| (key, *ty))).unwrap();
/// fields[0].value = ScalarValue::String("x".repeat(4097));
/// assert_eq!(validate_scalar_projection(&fields, schema.iter().map(|(key, ty)| (key, *ty))),
///     Err(ProjectionError::UnboundedValue));
/// ```
pub fn validate_scalar_projection<'schema, Field: PartialEq + 'schema>(
    fields: &[ScalarProjectionField<Field>],
    schema: impl IntoIterator<Item = (&'schema Field, ScalarType)>,
) -> Result<(), ProjectionError> {
    let mut expected = schema.into_iter();
    for stored in fields {
        let (key, ty) = expected.next().ok_or(ProjectionError::FieldCount)?;
        if &stored.field != key {
            return Err(ProjectionError::FieldKey);
        }
        ScalarTypeSet::only(ty)
            .validate(&stored.value)
            .map_err(|error| match error {
                ScalarContractError::TypeMismatch => ProjectionError::FieldType,
                ScalarContractError::UnboundedValue => ProjectionError::UnboundedValue,
            })?;
    }
    if expected.next().is_some() {
        return Err(ProjectionError::FieldCount);
    }
    Ok(())
}
