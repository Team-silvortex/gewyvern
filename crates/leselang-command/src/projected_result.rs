//! Released product result captures; no execution or journal authority.

use leselang_hir::computation::ScalarValue;
use leselang_hir::host_call::HostOperation;
use leselang_hir::result_field::ResultField;
use leselang_runtime_core::{ScalarProjectionField, validate_scalar_projection};
use serde::{Deserialize, Serialize};

use crate::Value;

/// Payload-free rejection of an unsupported or malformed product capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidProjectedResult;

impl std::fmt::Display for InvalidProjectedResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid product result projection")
    }
}

impl std::error::Error for InvalidProjectedResult {}

/// Mutable product data, not an authenticated receipt or executable continuation.
///
/// Serde preserves released bytes but does not validate the capture. Hosts must
/// bound ingress and validate original operation, authority, budgets and provenance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedResult {
    #[serde(
        default = "legacy_projection_version",
        skip_serializing_if = "is_legacy_projection"
    )]
    pub projection_version: u32,
    pub operation: HostOperation,
    pub fields: Vec<ProjectedField>,
}

fn legacy_projection_version() -> u32 {
    1
}

fn is_legacy_projection(version: &u32) -> bool {
    *version == 1
}

/// Legacy adapter specialization of the shared scalar field DTO, without conversion.
pub type ProjectedField = ScalarProjectionField<ResultField>;

impl ProjectedResult {
    /// Capture the smallest released vocabulary exporting this operation's fields.
    /// The caller must accept/type-check and bound the raw value before copying.
    /// Matching projected fields alone does not prove the raw value's operation.
    pub fn capture(
        operation: HostOperation,
        value: &Value,
    ) -> Result<Self, InvalidProjectedResult> {
        let version = if ResultField::OptionalExpected
            .result_type(operation.result_type())
            .is_some()
        {
            5
        } else if ResultField::Kind
            .result_type(operation.result_type())
            .is_some()
        {
            4
        } else if [
            ResultField::Expected,
            ResultField::Field,
            ResultField::Value,
        ]
        .into_iter()
        .any(|field| field.result_type(operation.result_type()).is_some())
        {
            3
        } else if [ResultField::Selected, ResultField::Required]
            .into_iter()
            .any(|field| field.result_type(operation.result_type()).is_some())
        {
            2
        } else {
            1
        };
        Self::capture_version(operation, value, version)
    }

    /// Materialize a known legacy vocabulary without upgrading its field set.
    /// This selects representation only, not trusted journal/schema provenance.
    pub fn capture_version(
        operation: HostOperation,
        value: &Value,
        version: u32,
    ) -> Result<Self, InvalidProjectedResult> {
        let fields = projection_fields(version)?
            .iter()
            .copied()
            .filter(|field| field.result_type(operation.result_type()).is_some())
            .map(|field| {
                Ok(ProjectedField {
                    field,
                    value: value.project_scalar(field).ok_or(InvalidProjectedResult)?,
                })
            })
            .collect::<Result<Vec<_>, InvalidProjectedResult>>()?;
        let result = Self {
            projection_version: version,
            operation,
            fields,
        };
        result.validate()?;
        Ok(result)
    }

    /// Validate version, exact ordered fields, scalar bounds and product tokens.
    /// Does not establish original authority, raw-reply identity or capture origin.
    pub fn validate(&self) -> Result<(), InvalidProjectedResult> {
        let expected = projection_fields(self.projection_version)?
            .iter()
            .filter_map(|field| {
                field
                    .result_type(self.operation.result_type())
                    .map(|ty| (field, ty))
            });
        validate_scalar_projection(&self.fields, expected).map_err(|_| InvalidProjectedResult)?;
        // Shape is shared; operation-specific value domains remain adapter policy.
        if self.fields.iter().any(|stored| {
            (stored.field == ResultField::Kind && !valid_kind_token(self.operation, &stored.value))
                || (stored.field == ResultField::OptionalExpected
                    && stored
                        .value
                        .text()
                        .is_some_and(|text| !leselang_hir::validate_ui_expected_text(text)))
        }) {
            return Err(InvalidProjectedResult);
        }
        Ok(())
    }

    /// Validate the saved frame first, then compare only its stored projections.
    /// Raw-result admission and the original trusted operation remain caller-owned.
    pub fn matches_capture(
        &self,
        operation: HostOperation,
        value: &Value,
    ) -> Result<bool, InvalidProjectedResult> {
        self.validate()?;
        if self.operation != operation {
            return Ok(false);
        }
        for stored in &self.fields {
            if stored.value
                != value
                    .project_scalar(stored.field)
                    .ok_or(InvalidProjectedResult)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Copy a stored field after caller validation; never fill missing legacy fields.
    /// This lookup does not implicitly validate the frame or charge execution fuel.
    pub fn field(&self, field: ResultField) -> Result<ScalarValue, InvalidProjectedResult> {
        self.fields
            .iter()
            .find(|stored| stored.field == field)
            .map(|stored| stored.value.clone())
            .ok_or(InvalidProjectedResult)
    }
}

fn projection_fields(version: u32) -> Result<&'static [ResultField], InvalidProjectedResult> {
    match version {
        1 => Ok(&ResultField::V1),
        2 => Ok(&ResultField::V2),
        3 => Ok(&ResultField::V3),
        4 => Ok(&ResultField::V4),
        5 => Ok(&ResultField::V5),
        _ => Err(InvalidProjectedResult),
    }
}

fn valid_kind_token(operation: HostOperation, value: &ScalarValue) -> bool {
    let ScalarValue::String(token) = value else {
        return false;
    };
    match operation {
        HostOperation::UiAssertNodeKind | HostOperation::UiWaitNodeKind => {
            leselang_hir::UiSemanticNodeKind::from_token(token).is_some()
        }
        HostOperation::UiAssertActionKind | HostOperation::UiWaitActionKind => {
            leselang_hir::UiSemanticActionKind::from_token(token).is_some()
        }
        HostOperation::UiAssertFormFieldInputKind | HostOperation::UiWaitFormFieldInputKind => {
            leselang_hir::UiFormInputKind::from_token(token).is_some()
        }
        _ => false,
    }
}
