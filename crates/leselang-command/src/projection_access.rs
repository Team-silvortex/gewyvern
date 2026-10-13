//! Closed-field observation for product captures, without execution authority.

use leselang_hir::Effect;
use leselang_hir::computation::Computation;
use leselang_hir::host_call::HostOperation;
use leselang_hir::projection_access::{
    ProjectionAccessError, ProjectionAccessLimits, ProjectionAccessPolicy,
    observe_projection_accesses,
};
use leselang_hir::result_field::ResultField;
use leselang_runtime_core::{CaptureScopeError, CapturedBindings, ScopeFrame, with_captured_scope};

use crate::{InvalidProjectedResult, ProjectedGroup, ProjectedResult};

/// Observe restored field accesses using the exact stored field sets.
///
/// Prefer [`crate::validate_result_binding`] for the complete product data/type
/// validation sequence; this lower-level observer deliberately checks only access.
///
/// The caller must bound and validate capture counts, names, scalar/result/group
/// schemas and body structure before entry. Explicit walk limits include cold
/// branches and cannot exceed the shared HIR safety ceilings. Names and saved
/// groups are borrowed; only field masks and temporary scope entries are built.
///
/// This is not complete type validation: unsaved group members use the operation
/// catalog as a fallback, just as live host/call nodes do. The caller must still
/// run the canonical group/type checker and enforce original authority, budgets,
/// input limits and journal provenance. Success is not an execution certificate.
/// No data is repaired, dispatched, serialized, upgraded or retained after return.
pub fn validate_captured_projection_accesses(
    captured: &CapturedBindings<Computation, ProjectedResult, ProjectedGroup>,
    pending: &Effect,
    limits: ProjectionAccessLimits,
) -> Result<(), InvalidProjectedResult> {
    let mut bindings = Vec::with_capacity(
        captured.locals.len() + captured.results.len() + captured.groups.len() + 1,
    );
    let mut scope = ScopeFrame::new(&mut bindings);
    with_captured_scope(
        captured,
        &mut scope,
        // Scalar names occupy slots without granting result fields.
        |_| Ok(None),
        |saved| {
            Ok(Some(AvailableProjection::Fields(
                saved
                    .fields
                    .iter()
                    .fold(0, |mask, field| mask | field_bit(field.field)),
            )))
        },
        |group| Ok(Some(AvailableProjection::Group(group))),
        || Ok(available_fields(pending).map(AvailableProjection::Fields)),
        |body, scope| validate_projected_accesses(body, scope, limits).map(|_| ()),
    )
    .map_err(|error| match error {
        CaptureScopeError::Duplicate { .. } => InvalidProjectedResult,
        CaptureScopeError::Native { error, .. } => error,
    })
}

fn field_bit(field: ResultField) -> u16 {
    match field {
        ResultField::Revision => 1,
        ResultField::Count => 2,
        ResultField::NodeId => 4,
        ResultField::FocusedNodeId => 8,
        ResultField::MaxLength => 16,
        ResultField::Selected => 32,
        ResultField::Required => 64,
        ResultField::Expected => 128,
        ResultField::Field => 256,
        ResultField::Value => 512,
        ResultField::Kind => 1024,
        ResultField::OptionalExpected => 2048,
    }
}

fn operation_fields(operation: HostOperation) -> u16 {
    ResultField::ALL
        .into_iter()
        .filter(|field| field.result_type(operation.result_type()).is_some())
        .fold(0, |mask, field| mask | field_bit(field))
}

fn available_fields(effect: &Effect) -> Option<u16> {
    HostOperation::for_effect(effect).map(operation_fields)
}

#[derive(Clone, Copy)]
enum AvailableProjection<'a> {
    Fields(u16),
    Group(&'a ProjectedGroup),
}

type ProjectionScope<'scope, 'values> =
    ScopeFrame<'scope, 'values, Option<AvailableProjection<'values>>>;

// A legacy frame is a closed field set, even when its operation now exports more.
struct ProductProjectionPolicy;

impl<'a> ProjectionAccessPolicy<'a, ResultField, HostOperation, Effect>
    for ProductProjectionPolicy
{
    type Observation = AvailableProjection<'a>;
    type Error = InvalidProjectedResult;

    fn local(
        &mut self,
        value: Option<&Self::Observation>,
    ) -> Result<Option<Self::Observation>, InvalidProjectedResult> {
        Ok(value.copied())
    }

    fn field(
        &mut self,
        value: Option<&Self::Observation>,
        field: &'a ResultField,
    ) -> Result<(), InvalidProjectedResult> {
        if matches!(value, Some(AvailableProjection::Fields(fields)) if fields & field_bit(*field) != 0)
        {
            Ok(())
        } else {
            Err(InvalidProjectedResult)
        }
    }

    fn host(
        &mut self,
        effect: &'a Effect,
    ) -> Result<Option<Self::Observation>, InvalidProjectedResult> {
        Ok(available_fields(effect).map(AvailableProjection::Fields))
    }

    fn call(
        &mut self,
        operation: &'a HostOperation,
    ) -> Result<Option<Self::Observation>, InvalidProjectedResult> {
        Ok(Some(AvailableProjection::Fields(operation_fields(
            *operation,
        ))))
    }

    fn member(
        &mut self,
        value: Option<&Self::Observation>,
        _: &'a str,
        name: &'a str,
        operation: &'a HostOperation,
    ) -> Result<Option<Self::Observation>, InvalidProjectedResult> {
        if let Some(AvailableProjection::Group(saved)) = value {
            let member = saved
                .members
                .iter()
                .find(|member| member.name == name && member.result.operation == *operation)
                .ok_or(InvalidProjectedResult)?;
            return Ok(Some(AvailableProjection::Fields(
                member
                    .result
                    .fields
                    .iter()
                    .fold(0, |mask, field| mask | field_bit(field.field)),
            )));
        }
        // The full canonical type checker, not this observation, fences group names.
        self.call(operation)
    }

    fn join(
        &mut self,
        left: Option<Self::Observation>,
        right: Option<Self::Observation>,
    ) -> Result<Option<Self::Observation>, InvalidProjectedResult> {
        Ok(match (left, right) {
            (Some(AvailableProjection::Fields(left)), Some(AvailableProjection::Fields(right))) => {
                Some(AvailableProjection::Fields(left & right))
            }
            _ => None,
        })
    }
}

fn validate_projected_accesses<'a>(
    expression: &'a Computation,
    scope: &mut ProjectionScope<'_, 'a>,
    limits: ProjectionAccessLimits,
) -> Result<Option<AvailableProjection<'a>>, InvalidProjectedResult> {
    observe_projection_accesses(expression, scope, limits, &mut ProductProjectionPolicy).map_err(
        |error| match error {
            ProjectionAccessError::Native { error, .. } => error,
            _ => InvalidProjectedResult,
        },
    )
}
