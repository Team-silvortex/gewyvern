//! Product group-start assembly with explicit live admission and reservation hooks.

use leselang_hir::{Effect, HirBranch};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, Fuel};

use crate::continuation_schema::{
    GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION, GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION,
    GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION, GROUP_RESULT_CONTINUATION_SCHEMA_VERSION,
    GROUP_TAIL_CONTINUATION_SCHEMA_VERSION, PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION,
};
use crate::reference_merge::validate_merge_plan;
use crate::reference_request_build::{ReservedEffectInput, build_effect_request};
use crate::{
    ContinuationToken, ExecutionOrder, GroupResultBinding, MergePlan, NamedEffectRequest,
    ResultBinding,
};

pub struct GroupStartContext {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub expected_revision: Option<Revision>,
    pub deadline_ms: u64,
    pub deadline_at_ms: Option<u64>,
    pub fuel_remaining: u64,
}

/// Trusted hooks on the original owner, not proof supplied by the adapter.
/// Capacity is observed before any reservation; the eventual journal transaction
/// must recheck it. Each allocation reserves in the live identity namespace.
/// Failure/unwind stops the pass without retry or refund of prior reservations.
pub trait ReferenceGroupStartHost {
    fn check_admission(&mut self, additional: usize) -> Result<(), Fault>;
    fn allocate_sequence(&mut self) -> Result<u64, Fault>;
}

/// Prepared data, not an admitted graph, dispatch grant or receipt. The caller
/// must validate and atomically commit the whole graph before publishing it.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedGroupStart {
    pub merge_token: ContinuationToken,
    pub plan: MergePlan,
    pub branches: Vec<NamedEffectRequest>,
}

/// Assemble the original group plan, prefix requests and successor reservations.
/// Callers must bound/admit HIR and authorize its cold branches before entry; this
/// function preserves preflight and allocation order, not arbitrary-input safety.
/// It neither installs pending state nor commits a graph. Partially allocated
/// identities remain consumed on refusal; partial requests are never published.
pub fn prepare_group_start<Host: ReferenceGroupStartHost + ?Sized>(
    branches: &[HirBranch],
    order: ExecutionOrder,
    context: GroupStartContext,
    result_binding: Option<Box<ResultBinding>>,
    host: &mut Host,
) -> Result<PreparedGroupStart, Fault> {
    let GroupStartContext {
        principal,
        capabilities,
        expected_revision,
        deadline_ms,
        deadline_at_ms,
        fuel_remaining,
    } = context;
    if branches.iter().any(|branch| {
        matches!(
            branch.effect,
            Effect::All { .. } | Effect::Sequence { .. } | Effect::Compute { .. }
        )
    }) {
        return Err(fault(
            "LSV1002",
            "nested structured execution is not yet supported",
        ));
    }
    let (order, result_binding) = if let Some(binding) = result_binding {
        let mut body_fuel = Fuel::new(fuel_remaining);
        if body_fuel.charge(branches.len() as u64).is_err() {
            return Err(fault("LSV1001", "bound group exceeds execution fuel"));
        }
        let (order, pending) = if order.is_sequential() {
            (
                ExecutionOrder::BoundSequential,
                Effect::Sequence {
                    steps: branches.to_vec(),
                },
            )
        } else {
            (
                ExecutionOrder::BoundParallel,
                Effect::All {
                    branches: branches.to_vec(),
                },
            )
        };
        (
            order,
            Some(Box::new(GroupResultBinding {
                pending,
                // Only a shape/size marker here; reserve the real identity after all prefix IDs.
                successor_sequence: if binding.body.is_pure() {
                    None
                } else {
                    Some(1)
                },
                additional_successor_sequences: (2..=binding.body.atomic_flow_bound().unwrap_or(0)
                    as u64)
                    .collect(),
                binding,
                fuel_remaining: body_fuel.remaining(),
            })),
        )
    } else {
        (order, None)
    };
    let order = if result_binding
        .as_ref()
        .is_some_and(|binding| binding.successor_sequence.is_some())
    {
        let captures = result_binding
            .as_ref()
            .is_some_and(|binding| binding.binding.body.is_atomic_capture());
        if result_binding
            .as_ref()
            .is_some_and(|binding| binding.is_conditional())
        {
            if order.is_parallel() {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            }
        } else if result_binding
            .as_ref()
            .is_some_and(|binding| binding.is_dataflow())
        {
            if order.is_parallel() {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            }
        } else {
            match (order.is_parallel(), captures) {
                (false, false) => ExecutionOrder::GroupTail,
                (false, true) => ExecutionOrder::GroupCapture,
                (true, false) => ExecutionOrder::ParallelGroupTail,
                (true, true) => ExecutionOrder::ParallelGroupCapture,
            }
        }
    } else {
        order
    };
    let mut plan = MergePlan {
        branches: branches.iter().map(|branch| branch.name.clone()).collect(),
        order,
        result_binding,
    };
    validate_merge_plan(&plan)?;
    if order.is_sequential() && branches.len() as u64 > fuel_remaining {
        return Err(fault(
            "LSV1001",
            "sequential control flow exceeds execution fuel",
        ));
    }
    host.check_admission(branches.len())?;
    let group_sequence = host.allocate_sequence()?;
    let merge_token = ContinuationToken::from_wire(format!("merge-{group_sequence}"));
    let mut named = Vec::with_capacity(branches.len());
    for (index, branch) in branches.iter().enumerate() {
        let sequence = host.allocate_sequence()?;
        let mut request = build_effect_request(ReservedEffectInput {
            effect: &branch.effect,
            result_type: branch.result_type,
            sequence,
            principal: principal.clone(),
            capabilities: capabilities.clone(),
            expected_revision,
            deadline_ms,
            deadline_at_ms,
            fuel_remaining,
        })?;
        if order.is_sequential() {
            let mut fuel = Fuel::new(request.continuation.fuel_remaining);
            if fuel.charge(index as u64).is_err() {
                return Err(fault(
                    "LSV1001",
                    "sequential control flow exceeds execution fuel",
                ));
            }
            request.continuation.fuel_remaining = fuel.remaining();
            request.budget.fuel_remaining = request.continuation.fuel_remaining;
        }
        if plan.result_binding.is_some() {
            request.continuation.schema_version = match order {
                ExecutionOrder::GroupConditional | ExecutionOrder::ParallelGroupConditional => {
                    GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::GroupDataflow | ExecutionOrder::ParallelGroupDataflow => {
                    GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::ParallelGroupTail | ExecutionOrder::ParallelGroupCapture => {
                    PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::GroupCapture => GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION,
                ExecutionOrder::GroupTail => GROUP_TAIL_CONTINUATION_SCHEMA_VERSION,
                _ => GROUP_RESULT_CONTINUATION_SCHEMA_VERSION,
            };
            request.continuation.group_result = Some(merge_token.clone());
        }
        named.push(NamedEffectRequest {
            branch: branch.name.clone(),
            request,
        });
    }
    if order.has_group_successor() {
        let sequence = host.allocate_sequence()?;
        if let Some(binding) = &mut plan.result_binding {
            binding.successor_sequence = Some(sequence);
            for reserved in &mut binding.additional_successor_sequences {
                *reserved = host.allocate_sequence()?;
            }
        }
    }
    Ok(PreparedGroupStart {
        merge_token,
        plan,
        branches: named,
    })
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
