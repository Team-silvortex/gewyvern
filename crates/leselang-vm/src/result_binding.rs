use leselang_hir::computation::{Computation, GroupLocalType, ScalarValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::projection_access::{
    ProjectionAccessError, ProjectionAccessLimits, ProjectionAccessPolicy,
    observe_projection_accesses,
};
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, Type};
use leselang_runtime_core::{
    CaptureScopeError, CaptureStructureError, CaptureValidationLimits, GroupCaptureLimits,
    ScalarProjectionField, ScopeFrame, validate_captured_bindings, validate_projected_group,
    validate_scalar_projection, with_captured_scope,
};
use serde::{Deserialize, Serialize};

use crate::{Fault, Value};

pub use leselang_runtime_core::ScalarBinding;

/// Exact reference specializations of the shared owned capture DTOs.
pub type ResultBinding =
    leselang_runtime_core::CapturedBindings<Computation, ProjectedResult, ProjectedGroup>;
pub type ProjectedBinding = leselang_runtime_core::ProjectedBinding<ProjectedResult>;
pub type ProjectedGroupBinding = leselang_runtime_core::ProjectedGroupBinding<ProjectedGroup>;
pub type ProjectedGroup = leselang_runtime_core::ProjectedGroup<ProjectedResult>;

pub(super) trait ReferenceProjectedGroup: Sized {
    fn capture(branches: &[leselang_hir::HirBranch], value: &Value) -> Result<Self, Fault>;
    fn validate(&self) -> Result<(), Fault>;
    fn matches_signature(&self, branches: &[leselang_hir::HirBranch]) -> bool;
    fn matches_members(
        &self,
        branches: &[leselang_hir::HirBranch],
        fields: &[crate::StructuredField],
    ) -> Result<bool, Fault>;
}

impl ReferenceProjectedGroup for ProjectedGroup {
    fn capture(branches: &[leselang_hir::HirBranch], value: &Value) -> Result<Self, Fault> {
        let Value::Structured { fields } = value else {
            return Err(invalid());
        };
        if branches.len() != fields.len() {
            return Err(invalid());
        }
        let members = branches
            .iter()
            .zip(fields)
            .map(|(branch, field)| {
                if branch.name != field.name {
                    return Err(invalid());
                }
                let operation = HostOperation::for_effect(&branch.effect).ok_or_else(invalid)?;
                Ok(ProjectedBinding {
                    name: branch.name.clone(),
                    result: ProjectedResult::capture(operation, &field.value)?,
                })
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        let group = Self { members };
        group.validate()?;
        Ok(group)
    }

    fn validate(&self) -> Result<(), Fault> {
        validate_projected_group(
            self,
            GroupCaptureLimits {
                min_members: 1,
                max_members: crate::MAX_MERGE_BRANCHES - 1,
            },
            crate::valid_merge_branch_name,
            ProjectedResult::validate,
        )
        .map_err(structure_fault)
    }

    fn matches_signature(&self, branches: &[leselang_hir::HirBranch]) -> bool {
        self.members.len() == branches.len()
            && self.members.iter().zip(branches).all(|(member, branch)| {
                member.name == branch.name
                    && HostOperation::for_effect(&branch.effect) == Some(member.result.operation)
            })
    }

    fn matches_members(
        &self,
        branches: &[leselang_hir::HirBranch],
        fields: &[crate::StructuredField],
    ) -> Result<bool, Fault> {
        self.validate()?;
        if !self.matches_signature(branches) || fields.len() != branches.len() {
            return Ok(false);
        }
        for ((member, branch), field) in self.members.iter().zip(branches).zip(fields) {
            if field.name != branch.name
                || !member
                    .result
                    .matches_capture(member.result.operation, &field.value)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

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
    pub(super) fn capture(operation: HostOperation, value: &Value) -> Result<Self, Fault> {
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

    fn capture_version(
        operation: HostOperation,
        value: &Value,
        version: u32,
    ) -> Result<Self, Fault> {
        let fields = projection_fields(version)?
            .iter()
            .copied()
            .filter(|field| field.result_type(operation.result_type()).is_some())
            .map(|field| {
                Ok(ProjectedField {
                    field,
                    value: project(value, field)?,
                })
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        let result = Self {
            projection_version: version,
            operation,
            fields,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<(), Fault> {
        let expected = projection_fields(self.projection_version)?
            .iter()
            .filter_map(|field| {
                field
                    .result_type(self.operation.result_type())
                    .map(|ty| (field, ty))
            });
        validate_scalar_projection(&self.fields, expected).map_err(|_| invalid())?;
        // Shape is shared; operation-specific value domains remain adapter policy.
        if self.fields.iter().any(|stored| {
            (stored.field == ResultField::Kind && !valid_kind_token(self.operation, &stored.value))
                || (stored.field == ResultField::OptionalExpected
                    && stored
                        .value
                        .text()
                        .is_some_and(|text| !leselang_hir::validate_ui_expected_text(text)))
        }) {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn matches_capture(
        &self,
        operation: HostOperation,
        value: &Value,
    ) -> Result<bool, Fault> {
        self.validate()?;
        if self.operation != operation {
            return Ok(false);
        }
        for stored in &self.fields {
            if stored.value != project(value, stored.field)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn field(&self, field: ResultField) -> Result<ScalarValue, Fault> {
        self.fields
            .iter()
            .find(|stored| stored.field == field)
            .map(|stored| stored.value.clone())
            .ok_or_else(invalid)
    }
}

fn projection_fields(version: u32) -> Result<&'static [ResultField], Fault> {
    match version {
        1 => Ok(&ResultField::V1),
        2 => Ok(&ResultField::V2),
        3 => Ok(&ResultField::V3),
        4 => Ok(&ResultField::V4),
        5 => Ok(&ResultField::V5),
        _ => Err(invalid()),
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

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1405".into(),
        message: "invalid or oversized result-binding continuation".into(),
    }
}

fn structure_fault(error: CaptureStructureError<Fault>) -> Fault {
    match error {
        CaptureStructureError::Native { error, .. } => error,
        _ => invalid(),
    }
}

pub(super) trait ReferenceResultBinding {
    fn schema_version(&self) -> u32;
    fn validate_structure(&self) -> Result<(), Fault>;
    fn validate(&self, pending: &Effect) -> Result<Type, Fault>;
}

impl ReferenceResultBinding for ResultBinding {
    fn schema_version(&self) -> u32 {
        if !self.groups.is_empty() {
            if self.body.is_pure() {
                crate::GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
            } else if self.body.is_result_chain() {
                crate::GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
            } else {
                crate::GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
            }
        } else if !self.body.is_pure() && !self.body.is_result_chain() {
            crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        } else if !self.results.is_empty() || (!self.body.is_pure() && !self.body.is_atomic_tail())
        {
            crate::DATAFLOW_CONTINUATION_SCHEMA_VERSION
        } else if self.body.is_pure() {
            crate::RESULT_BINDING_CONTINUATION_SCHEMA_VERSION
        } else {
            crate::SUCCESSOR_CONTINUATION_SCHEMA_VERSION
        }
    }

    fn validate_structure(&self) -> Result<(), Fault> {
        validate_captured_bindings(
            self,
            CaptureValidationLimits {
                max_saved_bindings: MAX_EFFECT_NESTING_DEPTH,
                max_name_bytes: MAX_BRANCH_NAME_BYTES,
            },
            ProjectedResult::validate,
            ReferenceProjectedGroup::validate,
            |body| {
                body.validate_structure().map_err(|_| invalid())?;
                if !body.is_result_flow() {
                    return Err(invalid());
                }
                Ok(())
            },
        )
        .map_err(structure_fault)
    }

    fn validate(&self, pending: &Effect) -> Result<Type, Fault> {
        self.validate_structure()?;
        if !self.results.is_empty() || !self.groups.is_empty() {
            let mut bindings =
                Vec::with_capacity(self.locals.len() + self.results.len() + self.groups.len() + 1);
            let mut scope = ScopeFrame::new(&mut bindings);
            with_captured_scope(
                self,
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
                |body, scope| validate_projected_accesses(body, scope).map(|_| ()),
            )
            .map_err(|error| match error {
                CaptureScopeError::Duplicate { .. } => invalid(),
                CaptureScopeError::Native { error, .. } => error,
            })?;
        }
        // Reconstruct the lexical scope for the normal type/canonical validator.
        // The normal type checker also fences groups and effectful operands.
        let mut expression = Computation::Bind {
            name: self.name.clone(),
            value: Box::new(Computation::Host {
                effect: Box::new(pending.clone()),
            }),
            body: Box::new(self.body.clone()),
        };
        if !self.results.is_empty() || !self.groups.is_empty() {
            let scope = self
                .locals
                .iter()
                .map(|local| (local.name.clone(), Type::Scalar(local.value.scalar_type())))
                .chain(
                    self.results
                        .iter()
                        .map(|result| (result.name.clone(), result.result.operation.result_type())),
                )
                .collect::<Vec<_>>();
            let groups = self
                .groups
                .iter()
                .map(|saved| GroupLocalType {
                    name: saved.name.clone(),
                    members: saved
                        .group
                        .members
                        .iter()
                        .map(|member| (member.name.clone(), member.result.operation))
                        .collect(),
                })
                .collect::<Vec<_>>();
            return expression
                .validate_in_group_scope(&scope, &groups)
                .map_err(|_| invalid());
        }
        for local in self.locals.iter().rev() {
            expression = Computation::Bind {
                name: local.name.clone(),
                value: Box::new(Computation::Literal {
                    value: local.value.clone(),
                }),
                body: Box::new(expression),
            };
        }
        expression.validate_in_scope(&[]).map_err(|_| invalid())
    }
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
struct ReferenceProjectionPolicy;

impl<'a> ProjectionAccessPolicy<'a, ResultField, HostOperation, Effect>
    for ReferenceProjectionPolicy
{
    type Observation = AvailableProjection<'a>;
    type Error = Fault;

    fn local(
        &mut self,
        value: Option<&Self::Observation>,
    ) -> Result<Option<Self::Observation>, Fault> {
        Ok(value.copied())
    }

    fn field(
        &mut self,
        value: Option<&Self::Observation>,
        field: &'a ResultField,
    ) -> Result<(), Fault> {
        if matches!(value, Some(AvailableProjection::Fields(fields)) if fields & field_bit(*field) != 0)
        {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    fn host(&mut self, effect: &'a Effect) -> Result<Option<Self::Observation>, Fault> {
        Ok(available_fields(effect).map(AvailableProjection::Fields))
    }

    fn call(&mut self, operation: &'a HostOperation) -> Result<Option<Self::Observation>, Fault> {
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
    ) -> Result<Option<Self::Observation>, Fault> {
        if let Some(AvailableProjection::Group(saved)) = value {
            let member = saved
                .members
                .iter()
                .find(|member| member.name == name && member.result.operation == *operation)
                .ok_or_else(invalid)?;
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
    ) -> Result<Option<Self::Observation>, Fault> {
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
) -> Result<Option<AvailableProjection<'a>>, Fault> {
    observe_projection_accesses(
        expression,
        scope,
        ProjectionAccessLimits {
            max_nodes: leselang_hir::computation::MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
        },
        &mut ReferenceProjectionPolicy,
    )
    .map_err(|error| match error {
        ProjectionAccessError::Native { error, .. } => error,
        _ => invalid(),
    })
}

pub(super) fn project(value: &Value, field: ResultField) -> Result<ScalarValue, Fault> {
    value.project_scalar(field).ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leselang_hir::{UiFormInputKind, UiSelectionState};
    use serde_json::json;

    fn frames() -> Vec<(HostOperation, Value, u32, serde_json::Value)> {
        vec![
            (
                HostOperation::UiFocus,
                Value::UiFocus {
                    node_id: "a".into(),
                },
                1,
                json!([{"field":"node_id","value":{"kind":"string","value":"a"}}]),
            ),
            (
                HostOperation::UiSetSelection,
                Value::UiSetSelection {
                    node_id: "a".into(),
                    state: UiSelectionState::Unselected,
                },
                2,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"selected","value":{"kind":"boolean","value":false}}
                ]),
            ),
            (
                HostOperation::UiSetFormValue,
                Value::UiSetFormValue {
                    node_id: "a".into(),
                    field: "name".into(),
                    value: "ready".into(),
                },
                3,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"value","value":{"kind":"string","value":"ready"}}
                ]),
            ),
            (
                HostOperation::UiAssertFormFieldInputKind,
                Value::UiAssertFormFieldInputKind {
                    node_id: "a".into(),
                    field: "name".into(),
                    input_kind: UiFormInputKind::TrimmedText,
                },
                4,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"kind","value":{"kind":"string","value":"trimmed_text"}}
                ]),
            ),
            (
                HostOperation::UiAssertFormFieldPlaceholder,
                Value::UiAssertFormFieldPlaceholder {
                    node_id: "a".into(),
                    field: "name".into(),
                    expected: None,
                },
                5,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"optional_expected","value":{"kind":"optional_string","value":null}}
                ]),
            ),
        ]
    }

    #[test]
    fn scalar_projection_keeps_five_capture_versions_and_frozen_field_wire() {
        for (operation, value, version, fields) in frames() {
            let current = ProjectedResult::capture(operation, &value).unwrap();
            assert_eq!(current.projection_version, version);
            assert_eq!(serde_json::to_value(&current.fields).unwrap(), fields);
            for saved_version in 1..=5 {
                let frame =
                    ProjectedResult::capture_version(operation, &value, saved_version).unwrap();
                let wire = serde_json::to_value(&frame).unwrap();
                assert_eq!(
                    wire.get("projection_version"),
                    (saved_version != 1).then_some(&json!(saved_version))
                );
                let restored: ProjectedResult = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(&restored).unwrap(), wire);
                assert!(restored.matches_capture(operation, &value).unwrap());
                assert!(
                    !restored
                        .matches_capture(HostOperation::RuntimeList, &value)
                        .unwrap()
                );
                for field in ResultField::ALL {
                    let expected = projection_fields(saved_version).unwrap().contains(&field)
                        && field.result_type(operation.result_type()).is_some();
                    assert_eq!(restored.field(field).is_ok(), expected);
                }
            }
        }
    }

    #[test]
    fn scalar_projection_keeps_invalid_frame_fault_and_capture_correspondence() {
        for (operation, value, _, _) in frames() {
            let frame = ProjectedResult::capture(operation, &value).unwrap();
            for mutation in 0..4 {
                let mut changed = frame.clone();
                match mutation {
                    0 => changed.projection_version = 6,
                    1 => {
                        changed.fields.remove(0);
                    }
                    2 => changed.fields.push(changed.fields[0].clone()),
                    3 => changed.fields[0].value = ScalarValue::Integer(7),
                    _ => unreachable!(),
                }
                assert_eq!(changed.validate(), Err(invalid()));
                assert_eq!(changed.matches_capture(operation, &value), Err(invalid()));
            }
            let mut changed = frame.clone();
            changed.fields[0].value = ScalarValue::String("another-node".into());
            assert!(changed.validate().is_ok());
            assert!(!changed.matches_capture(operation, &value).unwrap());
            assert_eq!(project(&value, ResultField::Revision), Err(invalid()));
            assert_eq!(
                ProjectedResult::capture_version(operation, &value, 0),
                Err(invalid())
            );
        }
    }
}
