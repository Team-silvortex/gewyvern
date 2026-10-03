use leselang_hir::computation::{
    Computation, GroupLocalType, MAX_SCALAR_STRING_BYTES, ScalarValue,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, Type};
use serde::{Deserialize, Serialize};

use crate::{Fault, Value};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarBinding {
    pub name: String,
    pub value: ScalarValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultBinding {
    pub name: String,
    pub locals: Vec<ScalarBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<ProjectedBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ProjectedGroupBinding>,
    pub body: Computation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedBinding {
    pub name: String,
    pub result: ProjectedResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedGroupBinding {
    pub name: String,
    pub group: ProjectedGroup,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedGroup {
    pub members: Vec<ProjectedBinding>,
}

impl ProjectedGroup {
    pub(super) fn capture(
        branches: &[leselang_hir::HirBranch],
        value: &Value,
    ) -> Result<Self, Fault> {
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

    pub(super) fn validate(&self) -> Result<(), Fault> {
        let mut names = std::collections::HashSet::new();
        if self.members.is_empty()
            || self.members.len() >= crate::MAX_MERGE_BRANCHES
            || self.members.iter().any(|member| {
                !crate::valid_merge_branch_name(&member.name) || !names.insert(&member.name)
            })
        {
            return Err(invalid());
        }
        for member in &self.members {
            member.result.validate()?;
        }
        Ok(())
    }

    pub(super) fn matches_signature(&self, branches: &[leselang_hir::HirBranch]) -> bool {
        self.members.len() == branches.len()
            && self.members.iter().zip(branches).all(|(member, branch)| {
                member.name == branch.name
                    && HostOperation::for_effect(&branch.effect) == Some(member.result.operation)
            })
    }

    pub(super) fn matches_members(
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedField {
    pub field: ResultField,
    pub value: ScalarValue,
}

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
            .copied()
            .filter_map(|field| {
                field
                    .result_type(self.operation.result_type())
                    .map(|ty| (field, ty))
            });
        if self.fields.len() != expected.clone().count()
            || self
                .fields
                .iter()
                .zip(expected)
                .any(|(stored, (field, ty))| {
                    stored.field != field
                        || stored.value.scalar_type() != ty
                        || stored
                            .value
                            .text()
                            .is_some_and(|value| value.len() > MAX_SCALAR_STRING_BYTES)
                        || (field == ResultField::Kind
                            && !valid_kind_token(self.operation, &stored.value))
                        || (field == ResultField::OptionalExpected
                            && stored
                                .value
                                .text()
                                .is_some_and(|text| !leselang_hir::validate_ui_expected_text(text)))
                })
        {
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

impl ResultBinding {
    pub(super) fn schema_version(&self) -> u32 {
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

    pub(super) fn validate_structure(&self) -> Result<(), Fault> {
        if self
            .locals
            .len()
            .saturating_add(self.results.len())
            .saturating_add(self.groups.len())
            > MAX_EFFECT_NESTING_DEPTH
            || self.name.len() > MAX_BRANCH_NAME_BYTES
            || self
                .locals
                .iter()
                .any(|local| local.name.len() > MAX_BRANCH_NAME_BYTES || !local.value.is_bounded())
        {
            return Err(invalid());
        }
        for result in &self.results {
            if result.name.len() > MAX_BRANCH_NAME_BYTES {
                return Err(invalid());
            }
            result.result.validate()?;
        }
        for group in &self.groups {
            if group.name.len() > MAX_BRANCH_NAME_BYTES {
                return Err(invalid());
            }
            group.group.validate()?;
        }
        self.body.validate_structure().map_err(|_| invalid())?;
        if !self.body.is_result_flow() {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn validate(&self, pending: &Effect) -> Result<Type, Fault> {
        self.validate_structure()?;
        if !self.results.is_empty() || !self.groups.is_empty() {
            let mut scope = self
                .results
                .iter()
                .map(|saved| {
                    (
                        saved.name.as_str(),
                        Some(AvailableProjection::Fields(
                            saved
                                .result
                                .fields
                                .iter()
                                .fold(0, |mask, field| mask | field_bit(field.field)),
                        )),
                    )
                })
                .collect::<Vec<_>>();
            scope.extend(self.groups.iter().map(|group| {
                (
                    group.name.as_str(),
                    Some(AvailableProjection::Group(&group.group)),
                )
            }));
            scope.push((
                self.name.as_str(),
                available_fields(pending).map(AvailableProjection::Fields),
            ));
            validate_projected_accesses(&self.body, &mut scope)?;
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

type ProjectionScope<'a> = Vec<(&'a str, Option<AvailableProjection<'a>>)>;

// A legacy frame is a closed field set, even when its operation now exports more.
// Follow aliases and both conditional paths without evaluating or replaying host work.
fn validate_projected_accesses<'a>(
    expression: &'a Computation,
    scope: &mut ProjectionScope<'a>,
) -> Result<Option<AvailableProjection<'a>>, Fault> {
    match expression {
        Computation::Local { name } => Ok(scope
            .iter()
            .find(|(bound, _)| *bound == name)
            .and_then(|(_, fields)| *fields)),
        Computation::Field { value, field } => {
            if !matches!(validate_projected_accesses(value, scope)?, Some(AvailableProjection::Fields(fields)) if fields & field_bit(*field) != 0)
            {
                return Err(invalid());
            }
            Ok(None)
        }
        Computation::Bind { name, value, body } => {
            let fields = validate_projected_accesses(value, scope)?;
            scope.push((name.as_str(), fields));
            let result = validate_projected_accesses(body, scope);
            scope.pop();
            result
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            validate_projected_accesses(when, scope)?;
            let left = validate_projected_accesses(then, scope)?;
            let right = validate_projected_accesses(otherwise, scope)?;
            Ok(match (left, right) {
                (
                    Some(AvailableProjection::Fields(left)),
                    Some(AvailableProjection::Fields(right)),
                ) => Some(AvailableProjection::Fields(left & right)),
                _ => None,
            })
        }
        Computation::Strings { items } => {
            for item in items {
                validate_projected_accesses(item, scope)?;
            }
            Ok(None)
        }
        Computation::Fold {
            name,
            item,
            items,
            initial,
            next,
            ..
        } => {
            validate_projected_accesses(items, scope)?;
            validate_projected_accesses(initial, scope)?;
            scope.push((name.as_str(), None));
            scope.push((item.as_str(), None));
            let result = validate_projected_accesses(next, scope);
            scope.pop();
            scope.pop();
            result.map(|_| None)
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            ..
        } => {
            validate_projected_accesses(initial, scope)?;
            scope.push((name.as_str(), None));
            let result = validate_projected_accesses(condition, scope)
                .and_then(|_| validate_projected_accesses(next, scope));
            scope.pop();
            result.map(|_| None)
        }
        Computation::Binary { left, right, .. } => {
            validate_projected_accesses(left, scope)?;
            validate_projected_accesses(right, scope)?;
            Ok(None)
        }
        Computation::Recover { value, fallback } => {
            validate_projected_accesses(value, scope)?;
            validate_projected_accesses(fallback, scope)?;
            Ok(None)
        }
        Computation::Unary { value, .. } => {
            validate_projected_accesses(value, scope)?;
            Ok(None)
        }
        Computation::Host { effect } => {
            Ok(available_fields(effect).map(AvailableProjection::Fields))
        }
        Computation::Call {
            operation,
            arguments,
        } => {
            for argument in arguments {
                validate_projected_accesses(&argument.value, scope)?;
            }
            Ok(Some(AvailableProjection::Fields(operation_fields(
                *operation,
            ))))
        }
        Computation::Group { branches, .. } => {
            for branch in branches {
                validate_projected_accesses(&branch.value, scope)?;
            }
            Ok(None)
        }
        Computation::Member {
            group,
            name,
            operation,
        } => {
            if let Some((_, Some(AvailableProjection::Group(saved)))) =
                scope.iter().find(|(bound, _)| *bound == group)
            {
                let member = saved
                    .members
                    .iter()
                    .find(|member| member.name == *name && member.result.operation == *operation)
                    .ok_or_else(invalid)?;
                return Ok(Some(AvailableProjection::Fields(
                    member
                        .result
                        .fields
                        .iter()
                        .fold(0, |mask, field| mask | field_bit(field.field)),
                )));
            }
            Ok(Some(AvailableProjection::Fields(operation_fields(
                *operation,
            ))))
        }
        Computation::Literal { .. } => Ok(None),
    }
}

pub(super) fn project(value: &Value, field: ResultField) -> Result<ScalarValue, Fault> {
    use ResultField::{
        Count, Expected, Field, FocusedNodeId, Kind, MaxLength, NodeId, Required, Revision,
        Selected,
    };
    match (field, value) {
        (
            ResultField::OptionalExpected,
            Value::UiAssertFormFieldPlaceholder { expected, .. }
            | Value::UiWaitFormFieldPlaceholder { expected, .. }
            | Value::UiAssertActionUnavailableReason { expected, .. }
            | Value::UiWaitActionUnavailableReason { expected, .. },
        ) => Ok(ScalarValue::OptionalString(
            leselang_hir::computation::OptionalStringValue(expected.clone()),
        )),
        (
            Kind,
            Value::UiAssertNodeKind { expected_kind, .. }
            | Value::UiWaitNodeKind { expected_kind, .. },
        ) => Ok(ScalarValue::String(expected_kind.as_str().into())),
        (
            Kind,
            Value::UiAssertActionKind { expected_kind, .. }
            | Value::UiWaitActionKind { expected_kind, .. },
        ) => Ok(ScalarValue::String(expected_kind.as_str().into())),
        (
            Kind,
            Value::UiAssertFormFieldInputKind { input_kind, .. }
            | Value::UiWaitFormFieldInputKind { input_kind, .. },
        ) => Ok(ScalarValue::String(input_kind.as_str().into())),
        (
            Expected,
            Value::UiAssertText { expected, .. }
            | Value::UiWaitText { expected, .. }
            | Value::UiAssertAutomationId { expected, .. }
            | Value::UiWaitAutomationId { expected, .. }
            | Value::UiAssertActionLabel { expected, .. }
            | Value::UiWaitActionLabel { expected, .. }
            | Value::UiAssertFormValue { expected, .. }
            | Value::UiWaitFormValue { expected, .. }
            | Value::UiAssertFormField { expected, .. }
            | Value::UiWaitFormField { expected, .. }
            | Value::UiAssertAccessibleName { expected, .. }
            | Value::UiWaitAccessibleName { expected, .. }
            | Value::UiAssertAccessibleDescription { expected, .. }
            | Value::UiWaitAccessibleDescription { expected, .. },
        ) => Ok(ScalarValue::String(expected.clone())),
        (
            Field,
            Value::UiSetFormValue { field, .. }
            | Value::UiAssertFormValue { field, .. }
            | Value::UiWaitFormValue { field, .. }
            | Value::UiAssertFormField { field, .. }
            | Value::UiWaitFormField { field, .. }
            | Value::UiAssertFormFieldInputKind { field, .. }
            | Value::UiWaitFormFieldInputKind { field, .. }
            | Value::UiAssertFormFieldRequired { field, .. }
            | Value::UiWaitFormFieldRequired { field, .. }
            | Value::UiAssertFormFieldMaxLength { field, .. }
            | Value::UiWaitFormFieldMaxLength { field, .. }
            | Value::UiAssertFormFieldPlaceholder { field, .. }
            | Value::UiWaitFormFieldPlaceholder { field, .. },
        ) => Ok(ScalarValue::String(field.clone())),
        (ResultField::Value, Value::UiSetFormValue { value, .. }) => {
            Ok(ScalarValue::String(value.clone()))
        }
        (
            Selected,
            Value::UiSetSelection { state, .. }
            | Value::UiAssertSelection { state, .. }
            | Value::UiWaitSelection { state, .. },
        ) => Ok(ScalarValue::Boolean(
            *state == leselang_hir::UiSelectionState::Selected,
        )),
        (
            Required,
            Value::UiAssertFormFieldRequired { state, .. }
            | Value::UiWaitFormFieldRequired { state, .. },
        ) => Ok(ScalarValue::Boolean(
            *state == leselang_hir::UiFormRequirementState::Required,
        )),
        (
            Revision,
            Value::RuntimeList { revision, .. }
            | Value::RuntimeInspect { revision, .. }
            | Value::RuntimeHistory { revision, .. }
            | Value::RuntimeLogs { revision, .. },
        ) => Ok(ScalarValue::Integer(revision.0)),
        (Count, Value::RuntimeList { runtimes, .. }) => count(runtimes.len()),
        (Count, Value::RuntimeHistory { entries, .. }) => count(entries.len()),
        (Count, Value::RuntimeLogs { entries, .. }) => count(entries.len()),
        (
            Count,
            Value::UiAssertChildCount { count: value, .. }
            | Value::UiWaitChildCount { count: value, .. },
        )
        | (
            MaxLength,
            Value::UiAssertFormFieldMaxLength {
                max_length: value, ..
            }
            | Value::UiWaitFormFieldMaxLength {
                max_length: value, ..
            },
        ) => count(*value),
        (
            FocusedNodeId,
            Value::UiNavigateFocus {
                focused_node_id, ..
            },
        ) => Ok(ScalarValue::String(focused_node_id.clone())),
        (
            NodeId,
            Value::UiActivate { node_id }
            | Value::UiFocus { node_id }
            | Value::UiNavigateFocus { node_id, .. }
            | Value::UiScrollIntoView { node_id }
            | Value::UiAssertVisible { node_id }
            | Value::UiAssertHidden { node_id }
            | Value::UiWaitHidden { node_id }
            | Value::UiAssertRealized { node_id }
            | Value::UiWaitRealized { node_id }
            | Value::UiWaitVisible { node_id }
            | Value::UiWaitEnabled { node_id }
            | Value::UiWaitDisabled { node_id }
            | Value::UiOpenWindow { node_id }
            | Value::UiCloseWindow { node_id }
            | Value::UiAssertWindowOpen { node_id }
            | Value::UiWaitWindowOpen { node_id }
            | Value::UiAssertWindowClosed { node_id }
            | Value::UiWaitWindowClosed { node_id }
            | Value::UiWaitFocused { node_id }
            | Value::UiAssertFocused { node_id }
            | Value::UiWaitUnfocused { node_id }
            | Value::UiAssertUnfocused { node_id }
            | Value::UiAssertEnabled { node_id }
            | Value::UiAssertDisabled { node_id }
            | Value::UiAssertChildCount { node_id, .. }
            | Value::UiWaitChildCount { node_id, .. }
            | Value::UiSetSelection { node_id, .. }
            | Value::UiAssertSelection { node_id, .. }
            | Value::UiWaitSelection { node_id, .. }
            | Value::UiAssertText { node_id, .. }
            | Value::UiWaitText { node_id, .. }
            | Value::UiAssertAutomationId { node_id, .. }
            | Value::UiWaitAutomationId { node_id, .. }
            | Value::UiAssertNodeKind { node_id, .. }
            | Value::UiWaitNodeKind { node_id, .. }
            | Value::UiAssertActionKind { node_id, .. }
            | Value::UiWaitActionKind { node_id, .. }
            | Value::UiAssertActionLabel { node_id, .. }
            | Value::UiWaitActionLabel { node_id, .. }
            | Value::UiAssertActionAvailable { node_id }
            | Value::UiWaitActionAvailable { node_id }
            | Value::UiAssertActionUnavailableReason { node_id, .. }
            | Value::UiWaitActionUnavailableReason { node_id, .. }
            | Value::UiSubmitForm { node_id }
            | Value::UiCancelForm { node_id }
            | Value::UiSetFormValue { node_id, .. }
            | Value::UiAssertFormValue { node_id, .. }
            | Value::UiWaitFormValue { node_id, .. }
            | Value::UiAssertFormField { node_id, .. }
            | Value::UiWaitFormField { node_id, .. }
            | Value::UiAssertFormFieldInputKind { node_id, .. }
            | Value::UiWaitFormFieldInputKind { node_id, .. }
            | Value::UiAssertFormFieldRequired { node_id, .. }
            | Value::UiWaitFormFieldRequired { node_id, .. }
            | Value::UiAssertFormFieldMaxLength { node_id, .. }
            | Value::UiWaitFormFieldMaxLength { node_id, .. }
            | Value::UiAssertFormFieldPlaceholder { node_id, .. }
            | Value::UiWaitFormFieldPlaceholder { node_id, .. }
            | Value::UiAssertAccessibleName { node_id, .. }
            | Value::UiWaitAccessibleName { node_id, .. }
            | Value::UiAssertAccessibleDescription { node_id, .. }
            | Value::UiWaitAccessibleDescription { node_id, .. },
        ) => Ok(ScalarValue::String(node_id.clone())),
        _ => Err(invalid()),
    }
}

fn count(value: usize) -> Result<ScalarValue, Fault> {
    u64::try_from(value)
        .map(ScalarValue::Integer)
        .map_err(|_| invalid())
}
