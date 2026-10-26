//! Product group-plan correspondence, independent of VM state and durable storage.

use crate::{ExecutionOrder, GroupResultBinding, MergePlan};
use leserpent_hir::{Effect, HirBranch, host_call::HostOperation};
use std::fmt;

/// Explicit caller ceilings; none of these values grant or replenish runtime resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupPlanLimits {
    pub max_branches: usize,
    pub max_effect_sequence: u64,
    pub max_fuel: u64,
}

/// Closed refusal with no saved names, effects, credentials or input payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidGroupPlan;

impl fmt::Display for InvalidGroupPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid group plan correspondence")
    }
}
impl std::error::Error for InvalidGroupPlan {}

impl GroupResultBinding {
    /// Observe an appended successor in the original, already-admitted plan.
    /// Shape correspondence and current request/recovery authority are separate gates.
    pub fn has_successor(&self, plan: &MergePlan) -> bool {
        self.successor_sequence.is_some()
            && self
                .plan_branches()
                .is_ok_and(|branches| plan.branches.len() > branches.len())
    }

    /// Borrow original group members without validating their shape or signatures.
    pub fn plan_branches(&self) -> Result<&[HirBranch], InvalidGroupPlan> {
        match &self.pending {
            Effect::All { branches } | Effect::Sequence { steps: branches } => Ok(branches),
            _ => Err(InvalidGroupPlan),
        }
    }

    /// Select an unused released successor name within an explicit candidate window.
    /// This observes names only; it does not allocate an identity or reserve work.
    pub fn plan_successor_name_at(
        &self,
        position: usize,
        max_candidates: usize,
    ) -> Result<String, InvalidGroupPlan> {
        let branches = self.plan_branches()?;
        let mut available = 0;
        for index in 0..max_candidates {
            let name = if index == 0 {
                "successor".into()
            } else {
                format!("successor_{index}")
            };
            if !branches.iter().any(|branch| branch.name == name) {
                if available == position {
                    return Ok(name);
                }
                available += 1;
            }
        }
        Err(InvalidGroupPlan)
    }

    /// Check ordered member/signature, flow/order and saved reservation correspondence.
    ///
    /// Callers must bound ingress and preflight binding structure before recursive IR
    /// classification. Outer plan shape, aggregate encoding size and full lexical/type
    /// validation remain separate gates. This does not inspect plan.result_binding:
    /// the caller must select the original binding from the admitted plan.
    ///
    /// Success is not authentication or restoration authority. This borrows input and
    /// never executes, mutates fuel, allocates sequences, admits requests or reads storage.
    pub fn validate_preflighted_plan(
        &self,
        plan: &MergePlan,
        limits: GroupPlanLimits,
    ) -> Result<(), InvalidGroupPlan> {
        let branches = self.plan_branches()?;
        let tail = self.successor_sequence.is_some();
        let bound = self
            .binding
            .body
            .atomic_flow_bound()
            .ok_or(InvalidGroupPlan)?;
        if plan.branches.len() < branches.len()
            || plan.branches.len() > branches.len().saturating_add(bound)
            || !self.binding.results.is_empty()
            || !self.binding.groups.is_empty()
            || if tail {
                if plan.order.captures_group_successor() {
                    !self.captures_successor()
                } else {
                    !self.binding.body.is_atomic_tail()
                }
            } else {
                !self.binding.body.is_pure()
            }
            || self.fuel_remaining > limits.max_fuel
            || !matches!(
                (&self.pending, plan.order),
                (Effect::All { .. }, ExecutionOrder::BoundParallel)
                    | (Effect::Sequence { .. }, ExecutionOrder::BoundSequential)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupTail)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupCapture)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupTail)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupCapture)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupDataflow)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupDataflow)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupConditional)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupConditional)
            )
            || matches!(
                plan.order,
                ExecutionOrder::GroupDataflow | ExecutionOrder::ParallelGroupDataflow
            ) != self.is_dataflow()
            || matches!(
                plan.order,
                ExecutionOrder::GroupConditional | ExecutionOrder::ParallelGroupConditional
            ) != self.is_conditional()
            || plan.order.has_group_successor() != tail
            || self.reserved_sequences().count() != bound
            || self
                .reserved_sequences()
                .any(|sequence| sequence == 0 || sequence > limits.max_effect_sequence)
            || self
                .reserved_sequences()
                .zip(self.reserved_sequences().skip(1))
                .any(|(left, right)| left >= right)
            || branches.len().saturating_add(bound) > limits.max_branches
            || plan
                .branches
                .iter()
                .skip(branches.len())
                .enumerate()
                .any(|(index, name)| {
                    self.plan_successor_name_at(index, limits.max_branches)
                        .as_ref()
                        != Ok(name)
                })
            || branches.iter().zip(&plan.branches).any(|(branch, name)| {
                branch.name != *name
                    || HostOperation::for_effect(&branch.effect)
                        .is_none_or(|operation| operation.result_type() != branch.result_type)
            })
        {
            return Err(InvalidGroupPlan);
        }
        Ok(())
    }
}
