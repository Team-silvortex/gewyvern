//! Released Leserpent structured-plan vocabulary and shape policy, without VM state.

use crate::GroupResultBinding;
use leselang_runtime_core::Fault;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_MERGE_BRANCHES: usize = 64;
pub const MAX_MERGE_BRANCH_NAME_BYTES: usize = 64;

/// Unchecked saved plan data. Decoding is not admission or recovery authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MergePlan {
    pub branches: Vec<String>,
    #[serde(default, skip_serializing_if = "ExecutionOrder::is_default_parallel")]
    pub order: ExecutionOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_binding: Option<Box<GroupResultBinding>>,
}

/// Released execution-order vocabulary; classifications do not schedule work.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOrder {
    #[default]
    Parallel,
    Sequential,
    ResultChain,
    Dataflow,
    BoundParallel,
    BoundSequential,
    GroupTail,
    GroupCapture,
    ParallelGroupTail,
    ParallelGroupCapture,
    GroupDataflow,
    ParallelGroupDataflow,
    GroupConditional,
    ParallelGroupConditional,
}

impl ExecutionOrder {
    fn is_default_parallel(&self) -> bool {
        *self == Self::Parallel
    }

    pub fn is_parallel(&self) -> bool {
        matches!(
            self,
            Self::Parallel
                | Self::BoundParallel
                | Self::ParallelGroupTail
                | Self::ParallelGroupCapture
                | Self::ParallelGroupDataflow
                | Self::ParallelGroupConditional
        )
    }

    pub fn is_sequential(&self) -> bool {
        !self.is_parallel()
    }

    pub fn has_group_successor(&self) -> bool {
        matches!(
            self,
            Self::GroupTail
                | Self::GroupCapture
                | Self::ParallelGroupTail
                | Self::ParallelGroupCapture
                | Self::GroupDataflow
                | Self::ParallelGroupDataflow
                | Self::GroupConditional
                | Self::ParallelGroupConditional
        )
    }

    pub fn captures_group_successor(&self) -> bool {
        matches!(
            self,
            Self::GroupCapture
                | Self::ParallelGroupCapture
                | Self::GroupDataflow
                | Self::ParallelGroupDataflow
                | Self::GroupConditional
                | Self::ParallelGroupConditional
        )
    }
}

impl MergePlan {
    /// Construct the released unbound parallel plan with its original 64-member/name limits.
    ///
    /// Input conversion completes before validation, preserving legacy iteration/fault order.
    /// This constructor keeps the public Fault return type and LSV2401 compatibility;
    /// shape-only consumers can use explicit limits without VM execution or journal state.
    pub fn new(branches: impl IntoIterator<Item = impl Into<String>>) -> Result<Self, Fault> {
        let plan = Self {
            branches: branches.into_iter().map(Into::into).collect(),
            order: ExecutionOrder::Parallel,
            result_binding: None,
        };
        plan.validate_shape(MergePlanShapeLimits {
            max_branches: MAX_MERGE_BRANCHES,
            max_name_bytes: MAX_MERGE_BRANCH_NAME_BYTES,
        })
        .map_err(|_| Fault {
            code: "LSV2401".to_string(),
            message: "structured merge plan is invalid or exceeds runtime bounds".to_string(),
        })?;
        Ok(plan)
    }
}

/// Explicit shape policy; no scheduling, authority or saved-body validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergePlanShapeLimits {
    pub max_branches: usize,
    pub max_name_bytes: usize,
}

/// Payload-free shape refusal; the reference VM preserves its original fault mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergePlanShapeError {
    GroupBinding,
    Successor,
    Branches,
}

impl std::fmt::Display for MergePlanShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::GroupBinding => "execution order and group binding presence differ",
            Self::Successor => "successor branch declaration differs",
            Self::Branches => "merge branches exceed shape policy",
        })
    }
}

impl std::error::Error for MergePlanShapeError {}

impl MergePlan {
    /// Validate order/binding presence, successor naming, count, then names/duplicates.
    ///
    /// Only outer data is inspected; saved bindings are not walked or authenticated.
    /// Callers bound ingress and policy before entry and retain full binding, authority,
    /// request, fuel and journal validation. Success is not permission to execute.
    pub fn validate_shape(&self, limits: MergePlanShapeLimits) -> Result<(), MergePlanShapeError> {
        if matches!(
            self.order,
            ExecutionOrder::BoundParallel
                | ExecutionOrder::BoundSequential
                | ExecutionOrder::GroupTail
                | ExecutionOrder::GroupCapture
                | ExecutionOrder::ParallelGroupTail
                | ExecutionOrder::ParallelGroupCapture
                | ExecutionOrder::GroupDataflow
                | ExecutionOrder::ParallelGroupDataflow
                | ExecutionOrder::GroupConditional
                | ExecutionOrder::ParallelGroupConditional
        ) != self.result_binding.is_some()
        {
            return Err(MergePlanShapeError::GroupBinding);
        }
        if self.order == ExecutionOrder::ResultChain && self.branches != ["result", "successor"] {
            return Err(MergePlanShapeError::Successor);
        }
        if self.order == ExecutionOrder::Dataflow
            && self
                .branches
                .iter()
                .enumerate()
                .any(|(index, name)| *name != format!("step_{}", index + 1))
        {
            return Err(MergePlanShapeError::Successor);
        }
        let minimum = if matches!(
            self.order,
            ExecutionOrder::Sequential
                | ExecutionOrder::BoundSequential
                | ExecutionOrder::GroupTail
                | ExecutionOrder::GroupCapture
                | ExecutionOrder::GroupDataflow
                | ExecutionOrder::GroupConditional
        ) {
            1
        } else {
            2
        };
        if !(minimum..=limits.max_branches).contains(&self.branches.len()) {
            return Err(MergePlanShapeError::Branches);
        }
        let mut unique = HashSet::with_capacity(self.branches.len());
        if self.branches.iter().any(|branch| {
            !valid_merge_branch_name(branch, limits.max_name_bytes)
                || !unique.insert(branch.as_str())
        }) {
            return Err(MergePlanShapeError::Branches);
        }
        Ok(())
    }
}

/// Released ASCII branch spelling with an explicit byte ceiling; no normalization.
pub fn valid_merge_branch_name(name: &str, max_name_bytes: usize) -> bool {
    !name.is_empty()
        && name.len() <= max_name_bytes
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
