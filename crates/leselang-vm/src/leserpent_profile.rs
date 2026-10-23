// Released Leserpent specialization; included at the crate root for API compatibility.

use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(feature = "sqlite-journal")]
use std::path::Path;

#[cfg(test)]
use leselang_hir::CAPABILITY_UI_PRESENTATION;
use leselang_hir::{Effect, HirProgram, Type, authorize, canonical_source};
#[cfg(test)]
use leselang_host_contract::CommandOrigin;
#[cfg(test)]
use leselang_host_contract::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, Confirmation,
};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fuel, SequenceWatermarkRequirement};
#[cfg(test)]
use leselang_runtime_core::{ResourceBudgetError, ResourceBudgetLimits};
use leserpent_domain::{CommandId, IdempotencyKey};
#[cfg(test)]
use leserpent_domain::{DOMAIN_SCHEMA_VERSION, QueryEnvelope};
use serde::Serialize;
#[cfg(test)]
use serde::de::Error as _;
#[cfg(test)]
use serde::{Deserialize, Deserializer};

mod admission;
#[cfg(test)]
mod bounded_decode_tests;
#[cfg(test)]
mod bounded_json_tests;
mod computation;
#[cfg(test)]
mod continuation_codec_contract_tests;
#[cfg(test)]
mod continuation_contract_tests;
#[cfg(test)]
mod continuation_schema_tests;
#[cfg(test)]
mod control_plane_reply_tests;
#[cfg(test)]
mod control_plane_request_tests;
#[cfg(test)]
mod debugger_audit_adapter_tests;
#[cfg(test)]
mod execution_outcome_contract_tests;
mod group_binding;
mod host_result;
#[cfg(test)]
mod identity_outcome_contract_tests;
mod journal;
#[cfg(test)]
mod merge_completion_tests;
#[cfg(test)]
mod merge_plan_contract_tests;
#[cfg(test)]
mod operation_request_tests;
#[cfg(test)]
mod presentation_request_tests;
#[cfg(test)]
mod reference_value_contract_tests;
#[cfg(test)]
mod request_codec_tests;
mod result_binding;
#[cfg(test)]
mod retry_policy_adapter_tests;
mod scheduling;
#[cfg(test)]
mod scheduling_contract_tests;
mod successor;
#[cfg(test)]
mod value_validation_tests;

pub use admission::{
    AdmissionEnd, AdmissionPolicy, AdmissionPoll, AdmissionStatus, AdmissionWait,
    MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_DELAY_MS, RootAdmission,
};
pub use group_binding::GroupResultBinding;
pub use leselang_hir::computation::ScalarValue;
pub use leselang_runtime_core::Fault;
pub use leselang_runtime_core::ResourceBudget;
pub use result_binding::{
    ProjectedBinding, ProjectedField, ProjectedGroup, ProjectedGroupBinding, ProjectedResult,
    ResultBinding, ScalarBinding,
};
pub use scheduling::{DispatchClaim, SchedulerLimits, SchedulerPressure};
use scheduling::{ReferenceSchedulerLimits, ReferenceSchedulerPressure};

use group_binding::ReferenceGroupResultBinding;
use host_result::{accept_bound_value, validate_bound_value};
use journal::{EffectObservation, Journal, MergeProgress};
pub use journal::{
    JOURNAL_SCHEMA_VERSION, MAX_JOURNAL_ENTRY_BYTES, MAX_JOURNAL_RECORDS, MAX_JOURNAL_TOTAL_BYTES,
};
use result_binding::ReferenceResultBinding;

pub use leselang_command::continuation_schema::{
    CONDITIONAL_CONTINUATION_SCHEMA_VERSION, CONTINUATION_SCHEMA_VERSION,
    DATAFLOW_CONTINUATION_SCHEMA_VERSION, GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION,
    GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION, GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION,
    GROUP_RESULT_CONTINUATION_SCHEMA_VERSION, GROUP_TAIL_CONTINUATION_SCHEMA_VERSION,
    PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION, RESULT_BINDING_CONTINUATION_SCHEMA_VERSION,
    SUCCESSOR_CONTINUATION_SCHEMA_VERSION,
};
#[cfg(test)]
use leselang_command::reference_continuation::validate_restorable_image;
pub(crate) use leselang_command::reference_continuation::{
    invalid_continuation_sequence_fault, validate_continuation_encoding_size,
    validate_continuation_size,
};
pub use leselang_command::{MAX_CONTINUATION_BYTES, decode_continuation, encode_continuation};
pub const DEFAULT_FUEL: u64 = 1_000;
pub const DEFAULT_EFFECT_DEADLINE_MS: u64 = 30_000;
pub use leselang_command::ContinuationToken;
pub(crate) use leselang_command::continuation_identity::{
    MAX_EFFECT_SEQUENCE, canonical_continuation_sequence, continuation_age_order,
    valid_continuation_token,
};
pub use leselang_command::reference_dispatch::{
    DEFAULT_DISPATCH_LEASE_MS, MAX_DISPATCH_ATTEMPTS, MAX_DISPATCH_LEASE_MS,
};
pub use leselang_command::reference_value::MAX_STRUCTURED_VALUE_DEPTH;
pub(crate) use leselang_command::reference_value::{
    validate_branch_outcome, validate_value, value_bounds_fault, value_validation_limits,
};
pub use leselang_command::retention_policy::{
    DEFAULT_COMPACTION_BATCH, DEFAULT_COMPLETED_RETENTION, MAX_COMPACTION_BATCH,
};
pub use leselang_command::retry_policy::{
    DEFAULT_MAX_SEMANTIC_RETRIES, DEFAULT_RETRY_BASE_DELAY_MS, DEFAULT_RETRY_MAX_DELAY_MS,
    MAX_RETRY_DELAY_MS, MAX_SEMANTIC_RETRIES,
};

pub use leselang_command::reference_request::{
    DEFAULT_MAX_OUTPUT_ITEMS, MAX_EFFECT_DEADLINE_MS, MAX_EXECUTION_FUEL, validate_effect_request,
};
pub(crate) use leselang_command::reference_request::{
    continuation_admission_limits, request_admission_fault,
    validate_continuation_image as validate_image,
};
pub use leselang_command::{ContinuationImage, EffectRequest};

// Keep released Rust paths as exact aliases, with no wire conversion or duplicate DTOs.
pub use leselang_command::{
    DebuggerAuditContext, DebuggerAuditRecord, DebuggerCancelResult, EffectOperation, EffectResult,
    PresentationEnvelope, PresentationOperation, PresentationResult, StructuredField, Value,
};

pub use leselang_command::{DispatchLease, FailedEffect, RetryDisposition, RetrySchedule};
pub use leselang_command::{EffectError, EffectErrorClass, RetryPolicy};

pub use leselang_command::{CompactionReport, RetentionPolicy};

pub use leselang_command::{
    ExecutionOrder, MAX_MERGE_BRANCH_NAME_BYTES, MAX_MERGE_BRANCHES, MergePlan,
};

pub use leselang_command::{BranchCompletion, BranchOutcome, Cancellation, CancellationReason};

pub use leselang_command::{MergeWait, NamedEffectRequest, Step, StructuredEffectBatch};

/// A serially entered engine that may own multiple independent suspended roots.
/// Separate engines can run on different host threads; shared journals coordinate dispatch.
pub struct Vm {
    next_effect_id: u64,
    fuel_limit: u64,
    pending: BTreeMap<ContinuationToken, ContinuationImage>,
    completed: BTreeMap<ContinuationToken, Step>,
    journal: Journal,
    scheduler_limits: SchedulerLimits,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(DEFAULT_FUEL)
    }
}

impl Vm {
    /// Creates an in-memory reference VM without opening or restoring a journal.
    #[cfg_attr(
        not(feature = "sqlite-journal"),
        doc = r#"
The durable API is absent when `sqlite-journal` is disabled:
```compile_fail
use leselang_vm::Vm;
let _ = Vm::open_journal("must-not-fall-back.sqlite3", 100);
```
"#
    )]
    pub fn new(fuel_limit: u64) -> Self {
        Self {
            next_effect_id: 1,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
            journal: Journal::ephemeral(),
            scheduler_limits: SchedulerLimits::default(),
        }
    }

    /// Opens the durable SQLite backend; available with the default `sqlite-journal` feature.
    /// Disabling the feature removes this API rather than silently falling back to memory.
    #[cfg(feature = "sqlite-journal")]
    pub fn open_journal(path: impl AsRef<Path>, fuel_limit: u64) -> Result<Self, Fault> {
        Self::open_journal_with_limits(path, fuel_limit, SchedulerLimits::default())
    }

    pub fn new_with_limits(fuel_limit: u64, limits: SchedulerLimits) -> Result<Self, Fault> {
        limits.validate()?;
        let mut vm = Self::new(fuel_limit);
        vm.scheduler_limits = limits;
        Ok(vm)
    }

    /// The host must reapply the same policy to all trusted workers after restart.
    #[cfg(feature = "sqlite-journal")]
    pub fn open_journal_with_limits(
        path: impl AsRef<Path>,
        fuel_limit: u64,
        limits: SchedulerLimits,
    ) -> Result<Self, Fault> {
        limits.validate()?;
        let (journal, snapshot) = Journal::open(path.as_ref())?;
        Ok(Self {
            next_effect_id: snapshot.next_sequence,
            fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
            pending: snapshot.pending,
            completed: snapshot.completed,
            journal,
            scheduler_limits: limits,
        })
    }

    /// Lowering capacity does not cancel existing work; it only gates new admission/leases.
    pub fn set_scheduler_limits(&mut self, limits: SchedulerLimits) -> Result<(), Fault> {
        limits.validate()?;
        self.scheduler_limits = limits;
        Ok(())
    }

    /// Read-only observation over the full clock range; does not reap due work.
    /// Unlike a claim, it does not need a representable future lease expiration.
    pub fn scheduler_pressure(&self, now_ms: u64) -> Result<SchedulerPressure, Fault> {
        self.journal
            .scheduler_pressure(now_ms, self.scheduler_limits)
    }

    pub fn start(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
    ) -> Step {
        self.start_inner(
            program,
            principal,
            capabilities,
            expected_revision,
            DEFAULT_EFFECT_DEADLINE_MS,
            None,
        )
    }

    pub fn start_timed(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Step {
        let deadline_at_ms = match validate_effect_clock(now_ms, timeout_ms) {
            Ok(deadline_at_ms) => deadline_at_ms,
            Err(error) => return Step::Fault(error),
        };
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        self.start_inner(
            program,
            principal,
            capabilities,
            expected_revision,
            timeout_ms,
            Some(deadline_at_ms),
        )
    }

    fn start_inner(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
    ) -> Step {
        if self.fuel_limit == 0 {
            return fault("LSV1001", "execution fuel exhausted");
        }
        if let Err(error) = authorize(program, &capabilities) {
            return fault(&error.code, error.message);
        }
        let mut fuel = Fuel::new(self.fuel_limit);
        let (effect, result_binding) = if let Effect::Compute { expression } =
            &program.function.effect
        {
            if !principal.is_valid() {
                return fault("LSV1402", "invalid computation principal");
            }
            match computation::evaluate(expression, &mut fuel) {
                Ok(computation::Outcome::Scalar(value)) => {
                    return Step::Done(Value::Scalar { value });
                }
                Ok(computation::Outcome::Host(effect)) => (effect, None),
                Ok(computation::Outcome::BoundHost { effect, binding }) => (effect, Some(binding)),
                Ok(computation::Outcome::Result) => {
                    return fault("LSV1402", "unexpected unprojected result");
                }
                Err(error) => return Step::Fault(error),
            }
        } else {
            (std::borrow::Cow::Borrowed(&program.function.effect), None)
        };
        let fuel_remaining = fuel.remaining();
        let effect = effect.as_ref();
        let effect_result_type = if let Some(binding) = &result_binding {
            if let Err(error) = binding.validate(effect) {
                return Step::Fault(error);
            }
            match leselang_hir::host_call::HostOperation::for_effect(effect) {
                Some(operation) => operation.result_type(),
                None if matches!(effect, Effect::All { .. } | Effect::Sequence { .. }) => {
                    Type::Structured
                }
                None => return Step::Fault(result_binding::invalid()),
            }
        } else {
            program.function.result_type
        };
        if fuel_remaining == 0 {
            return fault("LSV1001", "execution fuel exhausted before host operation");
        }
        if let Effect::Sequence { .. } = effect
            && canonical_source(effect).is_err()
        {
            return fault("LSV1002", "invalid sequential control flow");
        }
        if let Effect::All { branches } | Effect::Sequence { steps: branches } = effect {
            return self.start_group(
                branches,
                if matches!(effect, Effect::Sequence { .. }) {
                    ExecutionOrder::Sequential
                } else {
                    ExecutionOrder::Parallel
                },
                principal,
                capabilities,
                expected_revision,
                deadline_ms,
                deadline_at_ms,
                fuel_remaining,
                result_binding,
            );
        }

        if let Err(error) = self.journal.check_admission(1, self.scheduler_limits) {
            return Step::Fault(error);
        }
        let sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let mut request = match Self::build_effect_request(
            effect,
            effect_result_type,
            sequence,
            principal,
            capabilities,
            expected_revision,
            deadline_ms,
            deadline_at_ms,
            fuel_remaining,
        ) {
            Ok(request) => request,
            Err(error) => return Step::Fault(error),
        };
        if let Some(binding) = result_binding {
            request.continuation.schema_version = binding.schema_version();
            request.continuation.result_binding = Some(binding);
            if let Err(error) = validate_effect_request(&request) {
                return Step::Fault(error);
            }
        }
        if let Err(error) = self.journal.record_pending(
            &request.continuation,
            Some(&request),
            self.scheduler_limits,
        ) {
            return Step::Fault(error);
        }
        self.pending.insert(
            request.continuation.token.clone(),
            request.continuation.clone(),
        );
        Step::Effect(Box::new(request))
    }

    #[allow(clippy::too_many_arguments)]
    fn start_group(
        &mut self,
        branches: &[leselang_hir::HirBranch],
        order: ExecutionOrder,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
        fuel_remaining: u64,
        result_binding: Option<Box<ResultBinding>>,
    ) -> Step {
        if branches.iter().any(|branch| {
            matches!(
                branch.effect,
                Effect::All { .. } | Effect::Sequence { .. } | Effect::Compute { .. }
            )
        }) {
            return fault(
                "LSV1002",
                "nested structured execution is not yet supported",
            );
        }
        let (order, result_binding) = if let Some(binding) = result_binding {
            let mut body_fuel = Fuel::new(fuel_remaining);
            if body_fuel.charge(branches.len() as u64).is_err() {
                return fault("LSV1001", "bound group exceeds execution fuel");
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
                    additional_successor_sequences: (2..=binding
                        .body
                        .atomic_flow_bound()
                        .unwrap_or(0)
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
        if let Err(error) = validate_merge_plan(&plan) {
            return Step::Fault(error);
        }
        if order.is_sequential() && branches.len() as u64 > fuel_remaining {
            return fault("LSV1001", "sequential control flow exceeds execution fuel");
        }
        if let Err(error) = self
            .journal
            .check_admission(branches.len(), self.scheduler_limits)
        {
            return Step::Fault(error);
        }
        let group_sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let merge_token = ContinuationToken::from_wire(format!("merge-{group_sequence}"));
        let mut named = Vec::with_capacity(branches.len());
        for (index, branch) in branches.iter().enumerate() {
            let sequence = match self.allocate_sequence() {
                Ok(sequence) => sequence,
                Err(error) => return Step::Fault(error),
            };
            let mut request = match Self::build_effect_request(
                &branch.effect,
                branch.result_type,
                sequence,
                principal.clone(),
                capabilities.clone(),
                expected_revision,
                deadline_ms,
                deadline_at_ms,
                fuel_remaining,
            ) {
                Ok(request) => request,
                Err(error) => return Step::Fault(error),
            };
            if order.is_sequential() {
                let mut fuel = Fuel::new(request.continuation.fuel_remaining);
                if fuel.charge(index as u64).is_err() {
                    return fault("LSV1001", "sequential control flow exceeds execution fuel");
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
            let sequence = match self.allocate_sequence() {
                Ok(sequence) => sequence,
                Err(error) => return Step::Fault(error),
            };
            if let Some(binding) = &mut plan.result_binding {
                binding.successor_sequence = Some(sequence);
                for reserved in &mut binding.additional_successor_sequences {
                    *reserved = match self.allocate_sequence() {
                        Ok(sequence) => sequence,
                        Err(error) => return Step::Fault(error),
                    };
                }
            }
        }
        let graph = named
            .iter()
            .map(|branch| (branch.branch.as_str(), &branch.request))
            .collect::<Vec<_>>();
        if let Err(error) = self.journal.record_merge_graph_with_limits(
            &merge_token,
            &plan,
            &graph,
            self.scheduler_limits,
        ) {
            return Step::Fault(error);
        }
        for branch in &named {
            self.pending.insert(
                branch.request.continuation.token.clone(),
                branch.request.continuation.clone(),
            );
        }
        if order.is_sequential() {
            return Step::Effect(Box::new(named.remove(0).request));
        }
        Step::Effects(Box::new(StructuredEffectBatch {
            merge_token,
            branches: named,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_effect_request(
        effect: &Effect,
        result_type: Type,
        sequence: u64,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        deadline_ms: u64,
        deadline_at_ms: Option<u64>,
        fuel_remaining: u64,
    ) -> Result<EffectRequest, Fault> {
        let mut fuel = Fuel::new(fuel_remaining);
        fuel.charge(1).map_err(|_| Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before host operation".into(),
        })?;
        let image = ContinuationImage {
            schema_version: CONTINUATION_SCHEMA_VERSION,
            token: ContinuationToken::from_wire(format!("continuation-{sequence}")),
            program_counter: 1,
            expected_revision,
            result_type,
            pending_effect: effect.clone(),
            fuel_remaining: fuel.remaining(),
            deadline_ms,
            deadline_at_ms,
            max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
            result_binding: None,
            group_result: None,
        };
        let (required_capability, operation) = leselang_command::lower_effect_operation(
            effect,
            principal,
            capabilities,
            expected_revision,
            || {
                Some((
                    CommandId::new(format!("leselang-command-{sequence}")).ok()?,
                    IdempotencyKey::new(format!("leselang-effect-{sequence}")).ok()?,
                ))
            },
        )
        .map_err(|error| match error {
            leselang_command::OperationLoweringError::Identity => generated_effect_identity_fault(),
            leselang_command::OperationLoweringError::Lowering(error) => Fault {
                code: "LSV1002".to_string(),
                message: format!("structured branch lowering failed: {error:?}"),
            },
        })?;
        let budget = image.resource_budget();
        let request = EffectRequest {
            effect_id: format!("effect-{sequence}"),
            required_capability,
            operation,
            continuation: image,
            budget,
        };
        validate_effect_request(&request)?;
        Ok(request)
    }

    pub fn restore(&mut self, image: ContinuationImage) -> Result<(), Fault> {
        self.restore_inner(image, None)
    }

    /// Restores a continuation together with its original host-owned authority envelope.
    pub fn restore_request(&mut self, request: EffectRequest) -> Result<(), Fault> {
        validate_effect_request(&request)?;
        self.restore_inner(request.continuation.clone(), Some(&request))
    }

    fn restore_inner(
        &mut self,
        image: ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        validate_continuation_size(&image)?;
        if image.group_result.is_some() {
            return Err(group_binding::invalid());
        }
        if matches!(
            image.schema_version,
            SUCCESSOR_CONTINUATION_SCHEMA_VERSION
                | DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ) && request.is_none()
        {
            return Err(successor::invalid());
        }
        if let Some(current) = self.pending.get(&image.token) {
            return if current == &image {
                self.journal
                    .record_pending(&image, request, self.scheduler_limits)?;
                Ok(())
            } else {
                Err(Fault {
                    code: "LSV2002".to_string(),
                    message: "continuation token conflicts with pending state".to_string(),
                })
            };
        }
        if self.completed.contains_key(&image.token) {
            return Err(Fault {
                code: "LSV2003".to_string(),
                message: "continuation was already consumed".to_string(),
            });
        }
        let sequence = canonical_continuation_sequence(&image.token)
            .ok_or_else(invalid_continuation_sequence_fault)?;
        let next_sequence = SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
            .map_err(|_| effect_sequence_exhausted_fault())?
            .required_watermark();
        self.journal
            .record_pending(&image, request, self.scheduler_limits)?;
        self.next_effect_id = self.next_effect_id.max(next_sequence);
        self.pending.insert(image.token.clone(), image);
        Ok(())
    }

    pub fn resume(&mut self, image: &ContinuationImage, result: EffectResult) -> Step {
        if image.deadline_at_ms.is_some() {
            return fault(
                "LSV2111",
                "timed effects require resume_at with trusted scheduler time",
            );
        }
        self.resume_inner(image, result)
    }

    pub fn resume_at(
        &mut self,
        image: &ContinuationImage,
        now_ms: u64,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = validate_scheduler_time(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        self.resume_inner(image, result)
    }

    fn resume_inner(&mut self, image: &ContinuationImage, result: EffectResult) -> Step {
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        if let Err(error) = validate_image(image) {
            return Step::Fault(error);
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        if matches!(
            image.pending_effect,
            Effect::RuntimeRefresh { .. }
                | Effect::RuntimeCapabilitiesRefresh { .. }
                | Effect::RuntimeDeploy { .. }
                | Effect::DebuggerCancel { .. }
        ) {
            return fault(
                "LSV2110",
                "mutating effects must be completed through dispatch acknowledgement",
            );
        }

        self.complete_result(image, None, result)
    }

    /// Lease an eligible effect with the fewest delivery attempts, breaking ties
    /// by numeric admission order. Not-before clocks and sequence barriers still apply.
    pub fn claim_effect(
        &mut self,
        now_ms: u64,
        lease_ms: u64,
    ) -> Result<Option<DispatchLease>, Fault> {
        match self.try_claim_effect(now_ms, lease_ms)? {
            DispatchClaim::Leased(lease) => Ok(Some(*lease)),
            DispatchClaim::Idle => Ok(None),
            DispatchClaim::Backpressured(pressure) => Err(pressure.lease_fault()),
        }
    }

    /// Distinguishes an idle outbox from eligible work waiting for a host lease slot.
    /// Invalid clock/lease parameters fail before deadline cleanup or journal entry.
    pub fn try_claim_effect(&mut self, now_ms: u64, lease_ms: u64) -> Result<DispatchClaim, Fault> {
        validate_scheduler_time(now_ms)?;
        journal::validate_lease_clock(now_ms, lease_ms)?;
        self.expire_due(now_ms)?;
        let claim = self
            .journal
            .claim_dispatch(now_ms, lease_ms, self.scheduler_limits)?;
        if let DispatchClaim::Leased(lease) = &claim {
            let image = &lease.request.continuation;
            match self.pending.entry(image.token.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => {
                    if entry.get() != image {
                        return Err(Fault {
                            code: "LSV2005".into(),
                            message: "claimed continuation conflicts with local pending state"
                                .into(),
                        });
                    }
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    // Another engine may have admitted this request after our startup snapshot.
                    entry.insert(image.clone());
                }
            }
        }
        Ok(claim)
    }

    pub fn acknowledge_effect(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = validate_effect_request(&lease.request) {
            return Step::Fault(error);
        }
        let image = &lease.request.continuation;
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        self.complete_result(image, Some((lease, now_ms)), result)
    }

    pub fn report_effect_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        error: EffectError,
        policy: &RetryPolicy,
    ) -> Result<RetryDisposition, Fault> {
        validate_scheduler_time(now_ms)?;
        self.expire_due(now_ms)?;
        if let Some(completed) = self.replay_completed(&lease.request.continuation.token)? {
            return Ok(RetryDisposition::Terminal(
                self.visible_step(&lease.request.continuation.token, completed),
            ));
        }
        let schedule = leselang_command::retry_policy::plan_effect_retry(
            lease.retry_count,
            now_ms,
            &error,
            policy,
        )
        .map_err(effect_retry_fault)?;
        let disposition = match schedule {
            None => RetryDisposition::Terminal(Step::Failed(FailedEffect {
                error,
                retry_count: lease.retry_count,
            })),
            Some(point) => RetryDisposition::Scheduled(RetrySchedule {
                retry_count: point.retry_count,
                ready_at_ms: point.ready_at_ms,
                error,
            }),
        };
        let authoritative = self.journal.report_error(lease, now_ms, &disposition)?;
        if let RetryDisposition::Terminal(step) = &authoritative {
            let image = &lease.request.continuation;
            self.pending.remove(&image.token);
            self.completed.insert(image.token.clone(), step.clone());
            return Ok(RetryDisposition::Terminal(
                self.visible_step(&image.token, step.clone()),
            ));
        }
        Ok(authoritative)
    }

    pub fn cancel_effect(&mut self, image: &ContinuationImage, now_ms: u64) -> Step {
        if let Err(error) = validate_scheduler_time(now_ms) {
            return Step::Fault(error);
        }
        if let Err(error) = self.expire_due(now_ms) {
            return Step::Fault(error);
        }
        match self.replay_completed(&image.token) {
            Ok(Some(completed)) => return self.visible_step(&image.token, completed),
            Ok(None) => {}
            Err(error) => return Step::Fault(error),
        }
        let Some(stored) = self.pending.get(&image.token) else {
            return fault("LSV2004", "unknown continuation token");
        };
        if stored != image {
            return fault("LSV2005", "continuation image does not match pending state");
        }
        self.finish_cancellation(image, CancellationReason::Requested, now_ms)
    }

    pub fn cancel_effect_audited(
        &mut self,
        image: &ContinuationImage,
        now_ms: u64,
        audit: &DebuggerAuditContext,
    ) -> Result<DebuggerAuditRecord, Fault> {
        validate_debugger_audit(audit)?;
        validate_scheduler_time(now_ms)?;
        self.expire_due(now_ms)?;
        // Reconcile peer completion/retention just like ordinary cancellation.
        if let Some(completed) = self.reconcile_completed(&image.token)? {
            if !matches!(
                completed,
                Step::Cancelled(Cancellation {
                    reason: CancellationReason::Requested,
                    ..
                })
            ) {
                return Err(Fault {
                    code: "LSV2014".to_string(),
                    message: "completed effect cannot accept debugger cancellation audit"
                        .to_string(),
                });
            }
        } else {
            let Some(stored) = self.pending.get(&image.token) else {
                return Err(Fault {
                    code: "LSV2004".to_string(),
                    message: "unknown continuation token".to_string(),
                });
            };
            if stored != image {
                return Err(Fault {
                    code: "LSV2005".to_string(),
                    message: "continuation image does not match pending state".to_string(),
                });
            }
        }
        let step = Step::Cancelled(Cancellation {
            continuation: image.token.clone(),
            reason: CancellationReason::Requested,
            observed_at_ms: now_ms,
        });
        let (authoritative, record) = self.journal.cancel_audited(image, &step, audit, now_ms)?;
        if matches!(authoritative, Step::Cancelled(_)) {
            self.pending.remove(&image.token);
            self.completed
                .insert(image.token.clone(), authoritative.clone());
            if let Step::Fault(error) = self.visible_step(&image.token, authoritative) {
                return Err(error);
            }
        }
        Ok(record)
    }

    pub fn debugger_audit(
        &self,
        command_id: &CommandId,
    ) -> Result<Option<DebuggerAuditRecord>, Fault> {
        self.journal.debugger_audit(command_id.as_str())
    }

    /// Local pending cache size, not a shared-journal-wide scheduler statistic.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn pending_continuations(&self) -> Vec<ContinuationImage> {
        self.pending.values().cloned().collect()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn merge_result(&self, merge_token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        self.journal.merge_result(merge_token)
    }

    pub fn compact_journal(&mut self, policy: &RetentionPolicy) -> Result<CompactionReport, Fault> {
        validate_retention_policy(policy)?;
        let mut local_completed = self
            .completed
            .iter()
            .map(|(token, step)| (token.clone(), step.clone()))
            .collect::<Vec<_>>();
        local_completed.sort_by(|(left, _), (right, _)| continuation_age_order(left, right));
        let compacted = self.journal.compact_completed(&local_completed, policy)?;
        for token in &compacted.pruned_tokens {
            self.completed.remove(token);
        }
        Ok(CompactionReport {
            removed_records: compacted.removed_records,
            remaining_completed: compacted.remaining_completed,
            reclaimed_logical_bytes: compacted.reclaimed_logical_bytes,
        })
    }

    fn replay_completed(&mut self, token: &ContinuationToken) -> Result<Option<Step>, Fault> {
        Ok(self.reconcile_completed(token)?.cloned())
    }

    fn reconcile_completed(&mut self, token: &ContinuationToken) -> Result<Option<&Step>, Fault> {
        if self.completed.contains_key(token) {
            if !self.journal.completed_exists(token)? {
                self.completed.remove(token);
            }
        } else {
            match self.journal.observe_effect(token)? {
                #[cfg(feature = "sqlite-journal")]
                EffectObservation::Missing => {
                    self.pending.remove(token);
                }
                EffectObservation::Untracked | EffectObservation::Pending => {}
                EffectObservation::Completed(step) => {
                    self.pending.remove(token);
                    self.completed.insert(token.clone(), *step);
                }
            }
        }
        Ok(self.completed.get(token))
    }

    fn visible_step(&mut self, branch_token: &ContinuationToken, branch_step: Step) -> Step {
        match self.journal.merge_progress(branch_token) {
            Ok(MergeProgress::Standalone) => branch_step,
            Ok(MergeProgress::Next {
                request,
                completed_tokens,
            }) => {
                for token in completed_tokens {
                    self.pending.remove(&token);
                }
                self.pending.insert(
                    request.continuation.token.clone(),
                    request.continuation.clone(),
                );
                Step::Effect(request)
            }
            Ok(MergeProgress::Pending {
                merge_token,
                completed_branches,
                total_branches,
            }) => Step::Waiting(MergeWait {
                merge_token,
                completed_branches,
                total_branches,
            }),
            Ok(MergeProgress::Completed {
                merge_token,
                step,
                branch_tokens,
            }) => {
                let step = *step;
                for token in branch_tokens {
                    if self.pending.remove(&token).is_some() {
                        self.completed.entry(token).or_insert_with(|| step.clone());
                    }
                }
                debug_assert!(
                    self.journal
                        .merge_result(&merge_token)
                        .ok()
                        .flatten()
                        .is_some()
                );
                step
            }
            Err(error) => Step::Fault(error),
        }
    }

    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        loop {
            let sequence = self.journal.allocate_sequence(self.next_effect_id)?;
            if sequence == 0 || sequence > MAX_EFFECT_SEQUENCE {
                return Err(effect_sequence_exhausted_fault());
            }
            self.next_effect_id =
                SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
                    .map_err(|_| effect_sequence_exhausted_fault())?
                    .required_watermark();
            let token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
            if !self.pending.contains_key(&token) && !self.completed.contains_key(&token) {
                return Ok(sequence);
            }
        }
    }

    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault> {
        validate_scheduler_time(now_ms)?;
        let expired = self.journal.expire_due(now_ms)?;
        for (image, step) in expired {
            self.pending.remove(&image.token);
            self.completed.insert(image.token, step);
        }
        Ok(())
    }

    fn finish_cancellation(
        &mut self,
        image: &ContinuationImage,
        reason: CancellationReason,
        observed_at_ms: u64,
    ) -> Step {
        let step = Step::Cancelled(Cancellation {
            continuation: image.token.clone(),
            reason,
            observed_at_ms,
        });
        let authoritative = match self.journal.cancel(image, &step) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        self.visible_step(&image.token, authoritative)
    }
}

fn encode_json_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<Vec<u8>, Fault> {
    leselang_command::bounded_json::encode(value, limit)
        .map_err(|error| bounded_json_fault(error, label))
}

fn validate_json_size_capped<T: Serialize>(
    value: &T,
    limit: usize,
    label: &str,
) -> Result<(), Fault> {
    measure_json_capped(value, limit, label).map(|_| ())
}

fn measure_json_capped<T: Serialize>(value: &T, limit: usize, label: &str) -> Result<usize, Fault> {
    leselang_command::bounded_json::measure(value, limit)
        .map_err(|error| bounded_json_fault(error, label))
}

fn bounded_json_fault(
    error: leselang_command::bounded_json::BoundedJsonError,
    label: &str,
) -> Fault {
    match error {
        leselang_command::bounded_json::BoundedJsonError::Limit { limit, .. } => Fault {
            code: "LSV3002".to_string(),
            message: format!("{label} exceeds {limit} bytes"),
        },
        leselang_command::bounded_json::BoundedJsonError::Serialize(error) => Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode {label}: {error}"),
        },
    }
}

fn generated_effect_identity_fault() -> Fault {
    Fault {
        code: "LSV1002".to_string(),
        message: "generated effect identity is invalid".to_string(),
    }
}

fn validate_scheduler_time(now_ms: u64) -> Result<(), Fault> {
    leselang_runtime_core::validate_clock(now_ms)
}

fn validate_debugger_audit(audit: &DebuggerAuditContext) -> Result<(), Fault> {
    leselang_command::debugger_audit::validate_debugger_audit(audit).map_err(|error| Fault {
        code: "LSV2015".to_string(),
        message: error.to_string(),
    })
}

fn validate_effect_clock(now_ms: u64, timeout_ms: u64) -> Result<u64, Fault> {
    leselang_runtime_core::validate_execution_deadline(now_ms, timeout_ms)
}

#[cfg(any(test, feature = "sqlite-journal"))]
fn validate_effect_error(error: &EffectError) -> Result<(), Fault> {
    leselang_command::retry_policy::validate_effect_error(error).map_err(effect_retry_fault)
}

fn effect_retry_fault(error: leselang_command::retry_policy::EffectRetryError) -> Fault {
    use leselang_command::retry_policy::EffectRetryError;
    Fault {
        code: match error {
            EffectRetryError::Metadata => "LSV2202",
            EffectRetryError::Policy => "LSV2201",
            EffectRetryError::Clock => "LSV2203",
        }
        .into(),
        message: error.to_string(),
    }
}

pub fn merge_declared(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
    max_output_items: usize,
) -> Result<Step, Fault> {
    if plan.order.captures_group_successor()
        || (plan.order.has_group_successor()
            && plan
                .result_binding
                .as_ref()
                .is_none_or(|binding| !binding.has_successor(plan)))
    {
        return Err(group_binding::invalid());
    }
    merge_declared_inner(plan, completions, max_output_items, true)
}

pub(crate) fn merge_group_prefix(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
) -> Result<Step, Fault> {
    merge_declared_inner(plan, completions, DEFAULT_MAX_OUTPUT_ITEMS, false)
}

fn merge_declared_inner(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
    max_output_items: usize,
    finish_binding: bool,
) -> Result<Step, Fault> {
    validate_merge_plan(plan)?;
    let merged = leselang_command::merge_completion::merge_preflighted_completions(
        plan,
        completions,
        max_output_items,
        DEFAULT_MAX_OUTPUT_ITEMS,
        finish_binding,
        &ReferenceMergeCompletionHost,
    )
    .map_err(merge_completion_fault)?;
    let step = match merged {
        leselang_command::merge_completion::MergedCompletion::Value(value) => Step::Done(value),
        leselang_command::merge_completion::MergedCompletion::Terminal(step) => step,
    };
    if let Step::Done(value) = &step {
        validate_value(value, 1)?;
    }
    encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "structured merge output")?;
    if finish_binding
        && !plan.order.has_group_successor()
        && let (Some(binding), Step::Done(value)) = (&plan.result_binding, &step)
    {
        return Ok(binding.finish(value));
    }
    Ok(step)
}

struct ReferenceMergeCompletionHost;

impl leselang_command::merge_completion::MergeCompletionHost for ReferenceMergeCompletionHost {
    type Completion = BranchCompletion;
    type Outcome = BranchOutcome;
    type Terminal = Step;
    type Error = Fault;

    fn into_parts(completion: BranchCompletion) -> (String, BranchOutcome) {
        (completion.branch, completion.outcome)
    }

    fn validate_outcome(&self, outcome: &BranchOutcome) -> Result<usize, Fault> {
        validate_branch_outcome(outcome)
    }

    fn validate_member(
        &self,
        binding: &GroupResultBinding,
        branch: &str,
        outcome: &BranchOutcome,
    ) -> Result<(), Fault> {
        if let BranchOutcome::Value(value) = outcome {
            binding.validate_member(branch, value)?;
        }
        Ok(())
    }

    fn into_value_or_terminal(
        outcome: BranchOutcome,
    ) -> leselang_command::merge_completion::MergedCompletion<Step> {
        use leselang_command::merge_completion::MergedCompletion;
        match outcome {
            BranchOutcome::Value(value) => MergedCompletion::Value(value),
            BranchOutcome::Cancelled(cancellation) => {
                MergedCompletion::Terminal(Step::Cancelled(cancellation))
            }
            BranchOutcome::Failed(failure) => MergedCompletion::Terminal(Step::Failed(failure)),
            BranchOutcome::Fault(fault) => MergedCompletion::Terminal(Step::Fault(fault)),
        }
    }
}

fn merge_completion_fault(
    error: leselang_command::merge_completion::MergeCompletionError<Fault>,
) -> Fault {
    use leselang_command::merge_completion::MergeCompletionError;
    let (code, message) = match error {
        MergeCompletionError::Native(error) => return error,
        MergeCompletionError::OutputItems { items, limit } => {
            return merge_output_fault(items, limit);
        }
        MergeCompletionError::MissingFinalValue => return successor::invalid(),
        MergeCompletionError::OutputBudget => (
            "LSV2404",
            "structured merge output budget exceeds runtime bounds",
        ),
        MergeCompletionError::Incomplete => {
            ("LSV2402", "structured merge completion set is incomplete")
        }
        MergeCompletionError::Mismatch => (
            "LSV2402",
            "structured merge completion set does not match its plan",
        ),
    };
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

fn merge_output_fault(items: usize, limit: usize) -> Fault {
    Fault {
        code: "LSV2404".into(),
        message: format!("structured merge returned {items} items, limit is {limit}"),
    }
}

pub(crate) use leselang_command::reference_merge::validate_merge_plan;
use leselang_command::reference_merge::{merge_plan_admission_fault, merge_plan_admission_limits};

fn valid_merge_branch_name(name: &str) -> bool {
    leselang_command::valid_merge_branch_name(name, MAX_MERGE_BRANCH_NAME_BYTES)
}

fn effect_sequence_exhausted_fault() -> Fault {
    Fault {
        code: "LSV1003".to_string(),
        message: "continuation sequence is exhausted".to_string(),
    }
}

fn validate_retention_policy(policy: &RetentionPolicy) -> Result<(), Fault> {
    leselang_command::retention_policy::validate_retention_policy(policy, MAX_JOURNAL_RECORDS)
        .map_err(|error| Fault {
            code: "LSV2301".to_string(),
            message: error.to_string(),
        })
}

fn step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    let step = atomic_step_from_effect_result(image, operation, result);
    let Some(binding) = &image.result_binding else {
        return step;
    };
    let Step::Done(value) = step else {
        return step;
    };
    let (image, value) = match accept_bound_value(image, &value) {
        Ok(accepted) => accepted,
        Err(error) => return Step::Fault(error),
    };
    let mut fuel = Fuel::new(image.fuel_remaining);
    let Some(operation) = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
    else {
        return Step::Fault(result_binding::invalid());
    };
    match computation::resume(binding, value, operation, &mut fuel) {
        Ok(value) => Step::Done(Value::Scalar { value }),
        Err(error) => Step::Fault(error),
    }
}

fn atomic_step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    match result.into_effect_value(leselang_command::EffectValueContext {
        effect: &image.pending_effect,
        result_type: image.result_type,
        operation,
        expected_revision: image.expected_revision,
        max_output_items: image.max_output_items,
    }) {
        Ok(value) => Step::Done(value),
        Err(leselang_command::EffectValueError::Mismatch) => {
            fault("LSV2103", "effect result does not match pending effect")
        }
        Err(leselang_command::EffectValueError::RevisionConflict { expected, actual }) => fault(
            "LSV2101",
            format!(
                "effect revision conflict: expected {}, actual {}",
                expected.0, actual.0
            ),
        ),
        Err(leselang_command::EffectValueError::OutputLimit { count, limit }) => fault(
            "LSV2102",
            format!("effect returned {count} items, limit is {limit}"),
        ),
    }
}

fn fault(code: &str, message: impl Into<String>) -> Step {
    Step::Fault(Fault {
        code: code.to_string(),
        message: message.into(),
    })
}

#[cfg(test)]
#[cfg(feature = "sqlite-journal")]
mod tests {
    use leselang_hir::{
        UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS, UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
        UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS, UI_WAIT_ACTION_KIND_TIMEOUT_MS,
        UI_WAIT_ACTION_LABEL_TIMEOUT_MS, UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
        UI_WAIT_AUTOMATION_ID_TIMEOUT_MS, UI_WAIT_CHILD_COUNT_TIMEOUT_MS,
        UI_WAIT_ENABLED_TIMEOUT_MS, UI_WAIT_FOCUSED_TIMEOUT_MS,
        UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS, UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
        UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS, UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
        UI_WAIT_FORM_FIELD_TIMEOUT_MS, UI_WAIT_FORM_VALUE_TIMEOUT_MS, UI_WAIT_NODE_KIND_TIMEOUT_MS,
        UI_WAIT_REALIZED_TIMEOUT_MS, UI_WAIT_SELECTION_TIMEOUT_MS, UI_WAIT_TEXT_TIMEOUT_MS,
        UI_WAIT_UNFOCUSED_TIMEOUT_MS, UI_WAIT_VISIBLE_TIMEOUT_MS, UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS,
        UI_WAIT_WINDOW_OPEN_TIMEOUT_MS, UiFocusNavigationDirection, UiFormInputKind,
        UiFormRequirementState, UiSelectionState, UiSemanticActionKind, UiSemanticNodeKind,
    };
    use leserpent_domain::{
        Command, CommandResult, CommandStatus, MAX_RUNTIME_LOG_QUERY_ENTRIES, Query, QueryResult,
        RuntimeLogRecord,
    };
    #[test]
    fn string_list_value_validation_counts_entries_and_fences_in_memory_payloads() {
        use super::{ScalarValue, Value, validate_value};
        use leselang_hir::computation::StringListValue;
        for (items, count) in [
            (vec![], 1),
            (vec![String::new(); 64], 64),
            (vec!["x".repeat(4096)], 1),
        ] {
            assert_eq!(
                validate_value(
                    &Value::Scalar {
                        value: ScalarValue::StringList(StringListValue(items))
                    },
                    0
                )
                .unwrap(),
                count
            );
        }
        for items in [
            vec![String::new(); 65],
            vec!["x".repeat(4097)],
            vec!["x".repeat(2049); 2],
        ] {
            assert_eq!(
                validate_value(
                    &Value::Scalar {
                        value: ScalarValue::StringList(StringListValue(items))
                    },
                    0
                )
                .unwrap_err()
                .code,
                "LSV2403"
            );
        }
    }
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, InMemoryControlPlane, RuntimeId,
        RuntimeListFilter,
    };

    use super::*;

    static NEXT_TEMP_JOURNAL: AtomicU64 = AtomicU64::new(1);

    struct TempJournal {
        root: PathBuf,
        path: PathBuf,
    }

    impl TempJournal {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP_JOURNAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "leselang-vm-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("effects.sqlite3");
            Self { root, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempJournal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn program() -> HirProgram {
        lower(&parse(
            "fn main() = runtime.list(environment: \"production\", role: none)",
        ))
        .unwrap()
    }

    fn start(vm: &mut Vm, expected_revision: Option<Revision>) -> EffectRequest {
        let Step::Effect(request) = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision,
        ) else {
            panic!("expected effect");
        };
        *request
    }

    fn start_timed(vm: &mut Vm, now_ms: u64, timeout_ms: u64) -> EffectRequest {
        let Step::Effect(request) = vm.start_timed(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
            now_ms,
            timeout_ms,
        ) else {
            panic!("expected timed effect");
        };
        *request
    }

    fn runtime_list_result(revision: u64) -> EffectResult {
        EffectResult::Query(QueryResult::RuntimeList {
            revision: Revision(revision),
            runtimes: Vec::new(),
        })
    }

    fn runtime_list_value(revision: u64) -> Value {
        Value::RuntimeList {
            revision: Revision(revision),
            runtimes: Vec::new(),
        }
    }

    fn completed_branch(branch: &str, revision: u64) -> BranchCompletion {
        BranchCompletion {
            branch: branch.to_string(),
            outcome: BranchOutcome::Value(runtime_list_value(revision)),
        }
    }

    fn transient_error() -> EffectError {
        EffectError {
            class: EffectErrorClass::Transient,
            code: "upstream.unavailable".to_string(),
            message: "runtime endpoint is temporarily unavailable".to_string(),
        }
    }

    fn start_refresh(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            Some(expected_revision),
        ) else {
            panic!("expected refresh effect");
        };
        *request
    }

    fn start_capabilities_refresh(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            Some(expected_revision),
        ) else {
            panic!("expected capabilities refresh effect");
        };
        *request
    }

    fn start_deploy(vm: &mut Vm, expected_revision: Revision) -> EffectRequest {
        let program = lower(&parse(
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \"pid:42\")",
        ))
        .unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
            Some(expected_revision),
        ) else {
            panic!("expected deployment effect");
        };
        *request
    }

    #[test]
    fn stackless_start_emits_typed_runtime_list_effect() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        assert_eq!(request.required_capability, CAPABILITY_RUNTIME_READ);
        assert_eq!(request.continuation.program_counter, 1);
        assert_eq!(vm.pending_count(), 1);
        let EffectOperation::Query(query) = request.operation else {
            panic!("expected query operation");
        };
        let Query::RuntimeList { filter } = query.query else {
            panic!("runtime list must produce a list query");
        };
        assert_eq!(
            filter,
            RuntimeListFilter {
                environment: Some("production".to_string()),
                cluster: None,
                role: None,
            }
        );
    }

    #[test]
    fn ui_focus_uses_a_local_presentation_envelope_and_reenters_typed() {
        let program = lower(&parse(
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus effect");
        };
        assert_eq!(request.required_capability, CAPABILITY_UI_PRESENTATION);
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focus must not become a query or command");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::Focus { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiFocus {
                node_id: "runtime-a:refresh".into(),
            })
        );
    }

    #[test]
    fn ui_activate_uses_a_local_presentation_envelope_and_reenters_typed() {
        let program = lower(&parse(
            "fn main() = ui.activate(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI activate effect");
        };
        assert_eq!(request.required_capability, CAPABILITY_UI_PRESENTATION);
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI activate must not become a query or command");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::Activate { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::Activate {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiActivate {
                node_id: "runtime-a:refresh".into(),
            })
        );
    }

    #[test]
    fn ui_activate_rejects_missing_capability_and_torn_operations() {
        let program = lower(&parse(
            "fn main() = ui.activate(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        assert!(matches!(
            vm.start(
                &program,
                Principal {
                    id: "desktop-operator".to_string(),
                },
                CapabilitySet::default(),
                None,
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSH2001"
        ));

        let Step::Effect(mut request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI activate effect");
        };
        let EffectOperation::Presentation(presentation) = &mut request.operation else {
            panic!("UI activate must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::Activate {
            node_id: "other-action".into(),
        };
        assert!(validate_effect_request(&request).is_err());
    }

    #[test]
    fn forged_hir_and_continuations_fail_before_persistence() {
        let mut malformed = lower(&parse(
            "fn main() = ui.activate(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let Effect::UiActivate { node_id } = &mut malformed.function.effect else {
            panic!("expected UI activate effect");
        };
        *node_id = "x".repeat(leselang_hir::MAX_UI_NODE_ID_BYTES + 1);

        let mut vm = Vm::default();
        assert!(matches!(
            vm.start(
                &malformed,
                Principal {
                    id: "desktop-operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                None,
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2013"
        ));
        assert_eq!(vm.pending_count(), 0);

        let mut source_vm = Vm::default();
        let valid = lower(&parse(
            "fn main() = ui.activate(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let Step::Effect(request) = source_vm.start(
            &valid,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI activate effect");
        };
        let mut forged_image = request.continuation.clone();
        forged_image.pending_effect = malformed.function.effect;
        let mut restored = Vm::default();
        assert_eq!(restored.restore(forged_image).unwrap_err().code, "LSV2017");
        assert_eq!(restored.pending_count(), 0);
    }

    #[test]
    fn ui_focus_requires_presentation_capability_and_rejects_torn_operations() {
        let program = lower(&parse(
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        assert!(matches!(
            vm.start(
                &program,
                Principal {
                    id: "desktop-operator".to_string(),
                },
                CapabilitySet::default(),
                None,
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSH2001"
        ));

        let Step::Effect(mut request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus effect");
        };
        let EffectOperation::Presentation(presentation) = &mut request.operation else {
            panic!("UI focus must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::Focus {
            node_id: "other-action".into(),
        };
        assert!(validate_effect_request(&request).is_err());
    }

    #[test]
    fn ui_focus_navigation_binds_start_direction_and_actual_destination() {
        let program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"next\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus navigation effect");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focus navigation must use a presentation envelope");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::NavigateFocus {
                node_id,
                direction: UiFocusNavigationDirection::Next,
            } if node_id == "runtime-a:inspect"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::NavigateFocus {
                    node_id: "runtime-a:inspect".into(),
                    direction: UiFocusNavigationDirection::Next,
                    focused_node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiNavigateFocus {
                node_id: "runtime-a:inspect".into(),
                direction: UiFocusNavigationDirection::Next,
                focused_node_id: "runtime-a:refresh".into(),
            })
        );

        let program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"last\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus navigation effect");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focus navigation must use a presentation envelope");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::NavigateFocus {
                node_id,
                direction: UiFocusNavigationDirection::Last,
            } if node_id == "runtime-a:inspect"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::NavigateFocus {
                    node_id: "runtime-a:inspect".into(),
                    direction: UiFocusNavigationDirection::Last,
                    focused_node_id: "runtime-a:deploy".into(),
                }),
            ),
            Step::Done(Value::UiNavigateFocus {
                node_id: "runtime-a:inspect".into(),
                direction: UiFocusNavigationDirection::Last,
                focused_node_id: "runtime-a:deploy".into(),
            })
        );

        let program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:deploy\", direction: \"first\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus navigation effect");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focus navigation must use a presentation envelope");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::NavigateFocus {
                node_id,
                direction: UiFocusNavigationDirection::First,
            } if node_id == "runtime-a:deploy"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::NavigateFocus {
                    node_id: "runtime-a:deploy".into(),
                    direction: UiFocusNavigationDirection::First,
                    focused_node_id: "runtime-a:inspect".into(),
                }),
            ),
            Step::Done(Value::UiNavigateFocus {
                node_id: "runtime-a:deploy".into(),
                direction: UiFocusNavigationDirection::First,
                focused_node_id: "runtime-a:inspect".into(),
            })
        );
    }

    #[test]
    fn ui_focus_navigation_rejects_torn_requests_and_results() {
        let program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"next\")",
        ))
        .unwrap();
        let start = |vm: &mut Vm| {
            let Step::Effect(request) = vm.start(
                &program,
                Principal {
                    id: "desktop-operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                None,
            ) else {
                panic!("expected UI focus navigation effect");
            };
            *request
        };

        let mut vm = Vm::default();
        let mut torn = start(&mut vm);
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI focus navigation must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::NavigateFocus {
            node_id: "runtime-a:inspect".into(),
            direction: UiFocusNavigationDirection::Previous,
        };
        assert!(validate_effect_request(&torn).is_err());

        let mut vm = Vm::default();
        let request = start(&mut vm);
        assert!(matches!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::NavigateFocus {
                    node_id: "runtime-a:inspect".into(),
                    direction: UiFocusNavigationDirection::Previous,
                    focused_node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2103"
        ));

        let mut vm = Vm::default();
        let request = start(&mut vm);
        assert!(matches!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::NavigateFocus {
                    node_id: "runtime-a:inspect".into(),
                    direction: UiFocusNavigationDirection::Next,
                    focused_node_id: "runtime-a:inspect".into(),
                }),
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2103"
        ));
    }

    #[test]
    fn ui_scroll_into_view_uses_a_distinct_typed_presentation_round_trip() {
        let program = lower(&parse(
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI scroll effect");
        };
        assert_eq!(request.required_capability, CAPABILITY_UI_PRESENTATION);
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI scroll must not become a query or command");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::ScrollIntoView { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::ScrollIntoView {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiScrollIntoView {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI scroll must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::Focus {
            node_id: "runtime-a:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_visible_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI visibility assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI visibility assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertVisible { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertVisible {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiAssertVisible {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI visibility assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::ScrollIntoView {
            node_id: "runtime-a:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_hidden_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_hidden(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI hidden assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI hidden assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertHidden { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertHidden {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiAssertHidden {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI hidden assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertVisible {
            node_id: "runtime-a:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_hidden_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_hidden(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI hidden wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI hidden wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitHidden {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitHidden {
                    node_id: "runtime-a:card".into(),
                    timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitHidden {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI hidden wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitHidden {
            node_id: "runtime-a:card".into(),
            timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_realized_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_realized(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI realization assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI realization assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertRealized { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertRealized {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiAssertRealized {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI realization assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertVisible {
            node_id: "runtime-a:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_realized_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_realized(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI realization wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI realization wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitRealized {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_REALIZED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitRealized {
                    node_id: "runtime-a:card".into(),
                    timeout_ms: UI_WAIT_REALIZED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitRealized {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI realization wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitRealized {
            node_id: "runtime-a:card".into(),
            timeout_ms: UI_WAIT_REALIZED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_visible_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_visible(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI visibility wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI visibility wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitVisible {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitVisible {
                    node_id: "runtime-a:card".into(),
                    timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitVisible {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI visibility wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitVisible {
            node_id: "runtime-a:card".into(),
            timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_enabled_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_enabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI enabled wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI enabled wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitEnabled {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitEnabled {
                    node_id: "runtime-a:refresh".into(),
                    timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitEnabled {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI enabled wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitEnabled {
            node_id: "runtime-a:refresh".into(),
            timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_disabled_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_disabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI disabled wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI disabled wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitDisabled {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitDisabled {
                    node_id: "runtime-a:refresh".into(),
                    timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitDisabled {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI disabled wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitDisabled {
            node_id: "runtime-a:refresh".into(),
            timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_window_open_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_window_open(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI window-open assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI window-open assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertWindowOpen { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertWindowOpen {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiAssertWindowOpen {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI window-open assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertWindowOpen {
            node_id: "runtime-b:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_window_lifecycle_reenters_only_from_matching_frontend_local_results() {
        for (source, expected_operation, result, expected_value) in [
            (
                "fn main() = ui.open_window(node_id: \"runtime-a:card\")",
                PresentationOperation::OpenWindow {
                    node_id: "runtime-a:card".into(),
                },
                PresentationResult::OpenWindow {
                    node_id: "runtime-a:card".into(),
                },
                Value::UiOpenWindow {
                    node_id: "runtime-a:card".into(),
                },
            ),
            (
                "fn main() = ui.close_window(node_id: \"runtime-a:card\")",
                PresentationOperation::CloseWindow {
                    node_id: "runtime-a:card".into(),
                },
                PresentationResult::CloseWindow {
                    node_id: "runtime-a:card".into(),
                },
                Value::UiCloseWindow {
                    node_id: "runtime-a:card".into(),
                },
            ),
        ] {
            let program = lower(&parse(source)).unwrap();
            let mut vm = Vm::default();
            let Step::Effect(request) = vm.start(
                &program,
                Principal {
                    id: "desktop-operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                None,
            ) else {
                panic!("expected UI window lifecycle effect");
            };
            let EffectOperation::Presentation(presentation) = &request.operation else {
                panic!("UI window lifecycle must remain frontend-local");
            };
            assert_eq!(presentation.operation, expected_operation);
            validate_effect_request(&request).unwrap();
            assert_eq!(
                vm.resume(&request.continuation, EffectResult::Presentation(result),),
                Step::Done(expected_value)
            );
        }
    }

    #[test]
    fn ui_wait_window_open_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_window_open(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI window-open wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI window-open wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitWindowOpen {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_WINDOW_OPEN_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitWindowOpen {
                    node_id: "runtime-a:card".into(),
                    timeout_ms: UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitWindowOpen {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI window-open wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitWindowOpen {
            node_id: "runtime-a:card".into(),
            timeout_ms: UI_WAIT_WINDOW_OPEN_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_window_closed_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_window_closed(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI window-closed assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI window-closed assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertWindowClosed { node_id }
                if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertWindowClosed {
                    node_id: "runtime-a:card".into(),
                }),
            ),
            Step::Done(Value::UiAssertWindowClosed {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI window-closed assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertWindowClosed {
            node_id: "runtime-b:card".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_window_closed_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_window_closed(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI window-closed wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI window-closed wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitWindowClosed {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitWindowClosed {
                    node_id: "runtime-a:card".into(),
                    timeout_ms: UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitWindowClosed {
                node_id: "runtime-a:card".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI window-closed wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitWindowClosed {
            node_id: "runtime-a:card".into(),
            timeout_ms: UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_focused_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_focused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focused wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focused wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitFocused {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && *timeout_ms == UI_WAIT_FOCUSED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFocused {
                    node_id: "runtime-a:refresh".into(),
                    timeout_ms: UI_WAIT_FOCUSED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFocused {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI focused wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFocused {
            node_id: "runtime-a:refresh".into(),
            timeout_ms: UI_WAIT_FOCUSED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_focused_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI focus assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI focus assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFocused { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFocused {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiAssertFocused {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI focus assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertVisible {
            node_id: "runtime-a:refresh".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_unfocused_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_unfocused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI unfocused wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI unfocused wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitUnfocused {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && *timeout_ms == UI_WAIT_UNFOCUSED_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitUnfocused {
                    node_id: "runtime-a:refresh".into(),
                    timeout_ms: UI_WAIT_UNFOCUSED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitUnfocused {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI unfocused wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitUnfocused {
            node_id: "runtime-a:refresh".into(),
            timeout_ms: UI_WAIT_UNFOCUSED_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_unfocused_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_unfocused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI unfocused assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI unfocused assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertUnfocused { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertUnfocused {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiAssertUnfocused {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI unfocused assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertVisible {
            node_id: "runtime-a:refresh".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_enabled_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_enabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI enabled assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI enabled assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertEnabled { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertEnabled {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiAssertEnabled {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI enabled assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFocused {
            node_id: "runtime-a:refresh".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_disabled_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_disabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI disabled assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI disabled assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertDisabled { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertDisabled {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiAssertDisabled {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI disabled assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertEnabled {
            node_id: "runtime-a:refresh".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_child_count_binds_node_count_timeout_and_reentry_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_child_count(node_id: \"fleet-root\", count: \"4\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI child-count wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI child-count wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitChildCount {
                node_id,
                count: 4,
                timeout_ms,
            } if node_id == "fleet-root"
                && *timeout_ms == UI_WAIT_CHILD_COUNT_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();

        let mut torn = request.clone();
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI child-count wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitChildCount {
            node_id: "fleet-root".into(),
            count: 3,
            timeout_ms: UI_WAIT_CHILD_COUNT_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitChildCount {
                    node_id: "fleet-root".into(),
                    count: 4,
                    timeout_ms: UI_WAIT_CHILD_COUNT_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitChildCount {
                node_id: "fleet-root".into(),
                count: 4,
            })
        );

        let assert_program = lower(&parse(
            "fn main() = ui.assert_child_count(node_id: \"fleet-root\", count: \"3\")",
        ))
        .unwrap();
        let Step::Effect(assert_request) = vm.start(
            &assert_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI child-count assertion");
        };
        assert_eq!(
            vm.resume(
                &assert_request.continuation,
                EffectResult::Presentation(PresentationResult::AssertChildCount {
                    node_id: "fleet-root".into(),
                    count: 3,
                }),
            ),
            Step::Done(Value::UiAssertChildCount {
                node_id: "fleet-root".into(),
                count: 3,
            })
        );
    }

    #[test]
    fn ui_selection_mutation_binds_node_state_and_reentry_result() {
        let program = lower(&parse(
            "fn main() = ui.set_selection(node_id: \"runtime-a:card\", state: \"unselected\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection mutation");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI selection mutation must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::SetSelection {
                node_id,
                state: UiSelectionState::Unselected,
            } if node_id == "runtime-a:card"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::SetSelection {
                    node_id: "runtime-a:card".into(),
                    state: UiSelectionState::Unselected,
                }),
            ),
            Step::Done(Value::UiSetSelection {
                node_id: "runtime-a:card".into(),
                state: UiSelectionState::Unselected,
            })
        );

        let mut vm = Vm::default();
        let Step::Effect(mut torn) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection mutation");
        };
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI selection mutation must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::SetSelection {
            node_id: "runtime-a:card".into(),
            state: UiSelectionState::Selected,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_form_lifecycle_mutations_bind_kind_node_and_reentry_result() {
        let submit_program = lower(&parse(
            "fn main() = ui.submit_form(node_id: \"runtime-a:deploy\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(submit_request) = vm.start(
            &submit_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form submission mutation");
        };
        assert!(matches!(
            &submit_request.operation,
            EffectOperation::Presentation(PresentationEnvelope {
                operation: PresentationOperation::SubmitForm { node_id },
                ..
            }) if node_id == "runtime-a:deploy"
        ));
        validate_effect_request(&submit_request).unwrap();
        assert_eq!(
            vm.resume(
                &submit_request.continuation,
                EffectResult::Presentation(PresentationResult::SubmitForm {
                    node_id: "runtime-a:deploy".into(),
                }),
            ),
            Step::Done(Value::UiSubmitForm {
                node_id: "runtime-a:deploy".into(),
            })
        );

        let mut torn = submit_request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form submission must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::CancelForm {
            node_id: "runtime-a:deploy".into(),
        };
        assert!(validate_effect_request(&torn).is_err());

        let cancel_program = lower(&parse(
            "fn main() = ui.cancel_form(node_id: \"runtime-b:deploy\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(cancel_request) = vm.start(
            &cancel_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form cancellation mutation");
        };
        validate_effect_request(&cancel_request).unwrap();
        assert_eq!(
            vm.resume(
                &cancel_request.continuation,
                EffectResult::Presentation(PresentationResult::CancelForm {
                    node_id: "runtime-b:deploy".into(),
                }),
            ),
            Step::Done(Value::UiCancelForm {
                node_id: "runtime-b:deploy".into(),
            })
        );
    }

    #[test]
    fn ui_form_value_effects_bind_node_field_value_timeout_and_reentry() {
        let set_program = lower(&parse(
            "fn main() = ui.set_form_value(node_id: \"runtime-a:deploy\", field: \"pipeline_kind\", value: \"capture\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &set_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form value mutation");
        };
        assert!(matches!(
            &request.operation,
            EffectOperation::Presentation(PresentationEnvelope {
                operation: PresentationOperation::SetFormValue {
                    node_id,
                    field,
                    value,
                },
                ..
            }) if node_id == "runtime-a:deploy"
                && field == "pipeline_kind"
                && value == "capture"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::SetFormValue {
                    node_id: "runtime-a:deploy".into(),
                    field: "pipeline_kind".into(),
                    value: "capture".into(),
                }),
            ),
            Step::Done(Value::UiSetFormValue {
                node_id: "runtime-a:deploy".into(),
                field: "pipeline_kind".into(),
                value: "capture".into(),
            })
        );

        let assert_program = lower(&parse(
            "fn main() = ui.assert_form_value(node_id: \"runtime-a:deploy\", field: \"target\", expected: \"\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &assert_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form value assertion");
        };
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormValue {
                    node_id: "runtime-a:deploy".into(),
                    field: "target".into(),
                    expected: String::new(),
                }),
            ),
            Step::Done(Value::UiAssertFormValue {
                node_id: "runtime-a:deploy".into(),
                field: "target".into(),
                expected: String::new(),
            })
        );

        let wait_program = lower(&parse(
            "fn main() = ui.wait_form_value(node_id: \"runtime-a:deploy\", field: \"pipeline_kind\", expected: \"capture\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &wait_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form value wait");
        };
        assert!(matches!(
            &request.operation,
            EffectOperation::Presentation(PresentationEnvelope {
                operation: PresentationOperation::WaitFormValue {
                    node_id,
                    field,
                    expected,
                    timeout_ms: UI_WAIT_FORM_VALUE_TIMEOUT_MS,
                },
                ..
            }) if node_id == "runtime-a:deploy"
                && field == "pipeline_kind"
                && expected == "capture"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormValue {
                    node_id: "runtime-a:deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: "capture".into(),
                    timeout_ms: UI_WAIT_FORM_VALUE_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormValue {
                node_id: "runtime-a:deploy".into(),
                field: "pipeline_kind".into(),
                expected: "capture".into(),
            })
        );

        let mut vm = Vm::default();
        let Step::Effect(mut torn) = vm.start(
            &set_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form value mutation");
        };
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form value mutation must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::SetFormValue {
            node_id: "runtime-a:deploy".into(),
            field: "target".into(),
            value: "capture".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_selection_wait_binds_node_state_timeout_and_reentry_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\", state: \"selected\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI selection wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitSelection {
                node_id,
                state: UiSelectionState::Selected,
                timeout_ms,
            } if node_id == "runtime-a:card"
                && *timeout_ms == UI_WAIT_SELECTION_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitSelection {
                    node_id: "runtime-a:card".into(),
                    state: UiSelectionState::Selected,
                    timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitSelection {
                node_id: "runtime-a:card".into(),
                state: UiSelectionState::Selected,
            })
        );

        let mut vm = Vm::default();
        let Step::Effect(mut torn) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection wait");
        };
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI selection wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitSelection {
            node_id: "runtime-a:card".into(),
            state: UiSelectionState::Unselected,
            timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection wait");
        };
        assert!(matches!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitSelection {
                    node_id: "runtime-a:card".into(),
                    state: UiSelectionState::Unselected,
                    timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS,
                }),
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2103"
        ));

        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI selection wait");
        };
        assert!(matches!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitSelection {
                    node_id: "runtime-a:card".into(),
                    state: UiSelectionState::Selected,
                    timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS + 1,
                }),
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2103"
        ));
    }

    #[test]
    fn ui_assert_text_binds_expected_text_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI text assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI text assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertText { node_id, expected }
                if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertText {
                    node_id: "fleet-title".into(),
                    expected: "Runtime fleet".into(),
                }),
            ),
            Step::Done(Value::UiAssertText {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI text assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertText {
            node_id: "fleet-title".into(),
            expected: "Forged text".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_text_binds_expected_text_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI text wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI text wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitText {
                node_id,
                expected,
                timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS,
            } if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitText {
                    node_id: "fleet-title".into(),
                    expected: "Runtime fleet".into(),
                    timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitText {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI text wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitText {
                node_id: "fleet-title".into(),
                expected: "Forged text".into(),
                timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI text wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitText {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
                timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_automation_id_binds_expected_id_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"fleet-title\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI automation id assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI automation id assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertAutomationId { node_id, expected }
                if node_id == "fleet-title" && expected == "fleet-title"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertAutomationId {
                    node_id: "fleet-title".into(),
                    expected: "fleet-title".into(),
                }),
            ),
            Step::Done(Value::UiAssertAutomationId {
                node_id: "fleet-title".into(),
                expected: "fleet-title".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI automation id assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertAutomationId {
            node_id: "fleet-title".into(),
            expected: "forged-id".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_automation_id_binds_expected_id_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_automation_id(node_id: \"fleet-title\", expected: \"fleet-title\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI automation id wait");
        };
        assert!(matches!(
            &request.operation,
            EffectOperation::Presentation(PresentationEnvelope {
                operation: PresentationOperation::WaitAutomationId {
                    node_id,
                    expected,
                    timeout_ms: UI_WAIT_AUTOMATION_ID_TIMEOUT_MS,
                },
                ..
            }) if node_id == "fleet-title" && expected == "fleet-title"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitAutomationId {
                    node_id: "fleet-title".into(),
                    expected: "fleet-title".into(),
                    timeout_ms: UI_WAIT_AUTOMATION_ID_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitAutomationId {
                node_id: "fleet-title".into(),
                expected: "fleet-title".into(),
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI automation id wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitAutomationId {
                node_id: "fleet-title".into(),
                expected: "forged-id".into(),
                timeout_ms: UI_WAIT_AUTOMATION_ID_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI automation id wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitAutomationId {
                node_id: "fleet-title".into(),
                expected: "fleet-title".into(),
                timeout_ms: UI_WAIT_AUTOMATION_ID_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_node_kind_binds_expected_kind_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI node kind assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI node kind assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertNodeKind {
                node_id,
                expected_kind: UiSemanticNodeKind::Heading,
            } if node_id == "fleet-title"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertNodeKind {
                    node_id: "fleet-title".into(),
                    expected_kind: UiSemanticNodeKind::Heading,
                }),
            ),
            Step::Done(Value::UiAssertNodeKind {
                node_id: "fleet-title".into(),
                expected_kind: UiSemanticNodeKind::Heading,
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI node kind assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertNodeKind {
            node_id: "fleet-title".into(),
            expected_kind: UiSemanticNodeKind::Text,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_node_kind_binds_expected_kind_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI node kind wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI node kind wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitNodeKind {
                node_id,
                expected_kind: UiSemanticNodeKind::Heading,
                timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS,
            } if node_id == "fleet-title"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitNodeKind {
                    node_id: "fleet-title".into(),
                    expected_kind: UiSemanticNodeKind::Heading,
                    timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitNodeKind {
                node_id: "fleet-title".into(),
                expected_kind: UiSemanticNodeKind::Heading,
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI node kind wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitNodeKind {
                node_id: "fleet-title".into(),
                expected_kind: UiSemanticNodeKind::Text,
                timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI node kind wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitNodeKind {
                node_id: "fleet-title".into(),
                expected_kind: UiSemanticNodeKind::Heading,
                timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_action_kind_binds_expected_kind_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action kind assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action kind assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertActionKind {
                node_id,
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
            } if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertActionKind {
                    node_id: "runtime-a:refresh".into(),
                    expected_kind: UiSemanticActionKind::RuntimeRefresh,
                }),
            ),
            Step::Done(Value::UiAssertActionKind {
                node_id: "runtime-a:refresh".into(),
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action kind assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertActionKind {
            node_id: "runtime-a:refresh".into(),
            expected_kind: UiSemanticActionKind::RuntimeDeploy,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_action_kind_binds_expected_kind_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action kind wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action kind wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitActionKind {
                node_id,
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
                timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS,
            } if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitActionKind {
                    node_id: "runtime-a:refresh".into(),
                    expected_kind: UiSemanticActionKind::RuntimeRefresh,
                    timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitActionKind {
                node_id: "runtime-a:refresh".into(),
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI action kind wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitActionKind {
                node_id: "runtime-a:refresh".into(),
                expected_kind: UiSemanticActionKind::RuntimeDeploy,
                timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI action kind wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitActionKind {
                node_id: "runtime-a:refresh".into(),
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
                timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_action_available_reenters_only_from_its_matching_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_available(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action available assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action available assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertActionAvailable { node_id }
                if node_id == "runtime-a:refresh"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertActionAvailable {
                    node_id: "runtime-a:refresh".into(),
                }),
            ),
            Step::Done(Value::UiAssertActionAvailable {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action available assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertActionAvailable {
            node_id: "runtime-b:refresh".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_action_label_binds_expected_label_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_label(node_id: \"runtime-a:refresh\", expected: \"Refresh runtime\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action label assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action label assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertActionLabel {
                node_id,
                expected,
            } if node_id == "runtime-a:refresh"
                && expected == "Refresh runtime"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertActionLabel {
                    node_id: "runtime-a:refresh".into(),
                    expected: "Refresh runtime".into(),
                }),
            ),
            Step::Done(Value::UiAssertActionLabel {
                node_id: "runtime-a:refresh".into(),
                expected: "Refresh runtime".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action label assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertActionLabel {
            node_id: "runtime-a:refresh".into(),
            expected: "Inspect runtime".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_action_label_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_action_label(node_id: \"runtime-a:refresh\", expected: \"Refresh runtime\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action label wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action label wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitActionLabel {
                node_id,
                expected,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && expected == "Refresh runtime"
                && *timeout_ms == UI_WAIT_ACTION_LABEL_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitActionLabel {
                    node_id: "runtime-a:refresh".into(),
                    expected: "Refresh runtime".into(),
                    timeout_ms: UI_WAIT_ACTION_LABEL_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitActionLabel {
                node_id: "runtime-a:refresh".into(),
                expected: "Refresh runtime".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action label wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitActionLabel {
            node_id: "runtime-a:refresh".into(),
            expected: "Refresh runtime".into(),
            timeout_ms: UI_WAIT_ACTION_LABEL_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_action_available_reenters_only_from_its_fixed_bounded_presentation_result() {
        let program = lower(&parse(
            "fn main() = ui.wait_action_available(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action available wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action available wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitActionAvailable {
                node_id,
                timeout_ms,
            } if node_id == "runtime-a:refresh"
                && *timeout_ms == UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitActionAvailable {
                    node_id: "runtime-a:refresh".into(),
                    timeout_ms: UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitActionAvailable {
                node_id: "runtime-a:refresh".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action available wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitActionAvailable {
            node_id: "runtime-a:refresh".into(),
            timeout_ms: UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_action_unavailable_reason_binds_expected_reason_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action unavailable reason assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action unavailable reason assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertActionUnavailableReason {
                node_id,
                expected,
            } if node_id == "runtime-a:refresh"
                && expected.as_deref() == Some("Verification action is temporarily unavailable")
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertActionUnavailableReason {
                    node_id: "runtime-a:refresh".into(),
                    expected: Some("Verification action is temporarily unavailable".into()),
                }),
            ),
            Step::Done(Value::UiAssertActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: Some("Verification action is temporarily unavailable".into()),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI action unavailable reason assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertActionUnavailableReason {
            node_id: "runtime-a:refresh".into(),
            expected: Some("mismatch".into()),
        };
        assert!(validate_effect_request(&torn).is_err());

        let none_program = lower(&parse(
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: none)",
        ))
        .unwrap();
        let Step::Effect(none_request) = vm.start(
            &none_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected optional UI action unavailable reason assertion");
        };
        let EffectOperation::Presentation(none_presentation) = &none_request.operation else {
            panic!("optional UI action unavailable reason assertion must remain frontend-local");
        };
        assert!(matches!(
            &none_presentation.operation,
            PresentationOperation::AssertActionUnavailableReason {
                node_id,
                expected: None,
            } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            vm.resume(
                &none_request.continuation,
                EffectResult::Presentation(PresentationResult::AssertActionUnavailableReason {
                    node_id: "runtime-a:refresh".into(),
                    expected: None,
                }),
            ),
            Step::Done(Value::UiAssertActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: None,
            })
        );
    }

    #[test]
    fn ui_wait_action_unavailable_reason_binds_expected_reason_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI action unavailable reason wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI action unavailable reason wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitActionUnavailableReason {
                node_id,
                expected,
                timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
            } if node_id == "runtime-a:refresh"
                && expected.as_deref() == Some("Verification action is temporarily unavailable")
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitActionUnavailableReason {
                    node_id: "runtime-a:refresh".into(),
                    expected: Some("Verification action is temporarily unavailable".into()),
                    timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: Some("Verification action is temporarily unavailable".into()),
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI action unavailable reason wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: Some("mismatch".into()),
                timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI action unavailable reason wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: Some("Verification action is temporarily unavailable".into()),
                timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());

        let none_program = lower(&parse(
            "fn main() = ui.wait_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: none)",
        ))
        .unwrap();
        let Step::Effect(none_request) = vm.start(
            &none_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected optional UI action unavailable reason wait");
        };
        let EffectOperation::Presentation(none_presentation) = &none_request.operation else {
            panic!("optional UI action unavailable reason wait must remain frontend-local");
        };
        assert!(matches!(
            &none_presentation.operation,
            PresentationOperation::WaitActionUnavailableReason {
                node_id,
                expected: None,
                timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
            } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            vm.resume(
                &none_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitActionUnavailableReason {
                    node_id: "runtime-a:refresh".into(),
                    expected: None,
                    timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: None,
            })
        );
    }

    #[test]
    fn ui_assert_form_field_binds_node_field_expected_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFormField {
                node_id,
                field,
                expected,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && expected == "Pipeline kind"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormField {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: "Pipeline kind".into(),
                }),
            ),
            Step::Done(Value::UiAssertFormField {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: "Pipeline kind".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFormField {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "target".into(),
            expected: "Pipeline kind".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_form_field_input_kind_binds_node_field_kind_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field input kind assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field input kind assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFormFieldInputKind {
                node_id,
                field,
                input_kind: UiFormInputKind::PathToken,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormFieldInputKind {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    input_kind: UiFormInputKind::PathToken,
                }),
            ),
            Step::Done(Value::UiAssertFormFieldInputKind {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                input_kind: UiFormInputKind::PathToken,
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field input kind assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFormFieldInputKind {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "target".into(),
            input_kind: UiFormInputKind::PathToken,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_form_field_required_binds_node_field_state_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field required assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field required assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFormFieldRequired {
                node_id,
                field,
                state: UiFormRequirementState::Required,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormFieldRequired {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    state: UiFormRequirementState::Required,
                }),
            ),
            Step::Done(Value::UiAssertFormFieldRequired {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                state: UiFormRequirementState::Required,
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field required assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFormFieldRequired {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "target".into(),
            state: UiFormRequirementState::Required,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_form_field_max_length_binds_node_field_length_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field max length assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field max length assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFormFieldMaxLength {
                node_id,
                field,
                max_length: 128,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormFieldMaxLength {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    max_length: 128,
                }),
            ),
            Step::Done(Value::UiAssertFormFieldMaxLength {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                max_length: 128,
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field max length assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFormFieldMaxLength {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            max_length: 129,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_form_field_metadata_binds_expected_timeout_and_reentry() {
        let mut vm = Vm::default();
        let principal = Principal {
            id: "desktop-operator".to_string(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_UI_PRESENTATION]);

        let label_program = lower(&parse(
            "fn main() = ui.wait_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
        ))
        .unwrap();
        let Step::Effect(label_request) = vm.start(
            &label_program,
            principal.clone(),
            capabilities.clone(),
            None,
        ) else {
            panic!("expected UI form field wait");
        };
        let EffectOperation::Presentation(label_presentation) = &label_request.operation else {
            panic!("UI form field wait must remain frontend-local");
        };
        assert!(matches!(
            &label_presentation.operation,
            PresentationOperation::WaitFormField {
                node_id,
                field,
                expected,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && expected == "Pipeline kind"
                && *timeout_ms == UI_WAIT_FORM_FIELD_TIMEOUT_MS
        ));
        validate_effect_request(&label_request).unwrap();
        assert_eq!(
            vm.resume(
                &label_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormField {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: "Pipeline kind".into(),
                    timeout_ms: UI_WAIT_FORM_FIELD_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormField {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: "Pipeline kind".into(),
            })
        );
        let mut torn = label_request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormField {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            expected: "Pipeline kind".into(),
            timeout_ms: UI_WAIT_FORM_FIELD_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());

        let input_kind_program = lower(&parse(
            "fn main() = ui.wait_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
        ))
        .unwrap();
        let Step::Effect(input_kind_request) = vm.start(
            &input_kind_program,
            principal.clone(),
            capabilities.clone(),
            None,
        ) else {
            panic!("expected UI form field input kind wait");
        };
        let EffectOperation::Presentation(input_kind_presentation) = &input_kind_request.operation
        else {
            panic!("UI form field input kind wait must remain frontend-local");
        };
        assert!(matches!(
            &input_kind_presentation.operation,
            PresentationOperation::WaitFormFieldInputKind {
                node_id,
                field,
                input_kind: UiFormInputKind::PathToken,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && *timeout_ms == UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS
        ));
        validate_effect_request(&input_kind_request).unwrap();
        assert_eq!(
            vm.resume(
                &input_kind_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormFieldInputKind {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    input_kind: UiFormInputKind::PathToken,
                    timeout_ms: UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormFieldInputKind {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                input_kind: UiFormInputKind::PathToken,
            })
        );
        let mut torn = input_kind_request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field input kind wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormFieldInputKind {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "target".into(),
            input_kind: UiFormInputKind::PathToken,
            timeout_ms: UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        let required_program = lower(&parse(
            "fn main() = ui.wait_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
        ))
        .unwrap();
        let Step::Effect(required_request) = vm.start(
            &required_program,
            principal.clone(),
            capabilities.clone(),
            None,
        ) else {
            panic!("expected UI form field required wait");
        };
        let EffectOperation::Presentation(required_presentation) = &required_request.operation
        else {
            panic!("UI form field required wait must remain frontend-local");
        };
        assert!(matches!(
            &required_presentation.operation,
            PresentationOperation::WaitFormFieldRequired {
                node_id,
                field,
                state: UiFormRequirementState::Required,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && *timeout_ms == UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS
        ));
        validate_effect_request(&required_request).unwrap();
        assert_eq!(
            vm.resume(
                &required_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormFieldRequired {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    state: UiFormRequirementState::Required,
                    timeout_ms: UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormFieldRequired {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                state: UiFormRequirementState::Required,
            })
        );
        let mut torn = required_request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field required wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormFieldRequired {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            state: UiFormRequirementState::Optional,
            timeout_ms: UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        let max_length_program = lower(&parse(
            "fn main() = ui.wait_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
        ))
        .unwrap();
        let Step::Effect(max_length_request) =
            vm.start(&max_length_program, principal, capabilities, None)
        else {
            panic!("expected UI form field max length wait");
        };
        let EffectOperation::Presentation(max_length_presentation) = &max_length_request.operation
        else {
            panic!("UI form field max length wait must remain frontend-local");
        };
        assert!(matches!(
            &max_length_presentation.operation,
            PresentationOperation::WaitFormFieldMaxLength {
                node_id,
                field,
                max_length: 128,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && *timeout_ms == UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS
        ));
        validate_effect_request(&max_length_request).unwrap();
        assert_eq!(
            vm.resume(
                &max_length_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormFieldMaxLength {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    max_length: 128,
                    timeout_ms: UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormFieldMaxLength {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                max_length: 128,
            })
        );
        let mut torn = max_length_request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field max length wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormFieldMaxLength {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            max_length: 129,
            timeout_ms: UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_form_field_placeholder_binds_node_field_expected_across_request_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field placeholder assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field placeholder assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && expected.as_deref() == Some("http/request")
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormFieldPlaceholder {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: Some("http/request".into()),
                }),
            ),
            Step::Done(Value::UiAssertFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: Some("http/request".into()),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field placeholder assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertFormFieldPlaceholder {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            expected: Some("mismatch".into()),
        };
        assert!(validate_effect_request(&torn).is_err());

        let none_program = lower(&parse(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: none)",
        ))
        .unwrap();
        let Step::Effect(none_request) = vm.start(
            &none_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected optional UI form field placeholder assertion");
        };
        let EffectOperation::Presentation(none_presentation) = &none_request.operation else {
            panic!("optional UI form field placeholder assertion must remain frontend-local");
        };
        assert!(matches!(
            &none_presentation.operation,
            PresentationOperation::AssertFormFieldPlaceholder {
                node_id,
                field,
                expected: None,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        assert_eq!(
            vm.resume(
                &none_request.continuation,
                EffectResult::Presentation(PresentationResult::AssertFormFieldPlaceholder {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: None,
                }),
            ),
            Step::Done(Value::UiAssertFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: None,
            })
        );
    }

    #[test]
    fn ui_wait_form_field_placeholder_binds_node_field_expected_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI form field placeholder wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI form field placeholder wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && expected.as_deref() == Some("http/request")
                && *timeout_ms == UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormFieldPlaceholder {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: Some("http/request".into()),
                    timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: Some("http/request".into()),
            })
        );

        let mut torn = request.clone();
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field placeholder wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormFieldPlaceholder {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            expected: Some("mismatch".into()),
            timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI form field placeholder wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitFormFieldPlaceholder {
            node_id: "workspace-runtime-a-deploy".into(),
            field: "pipeline_kind".into(),
            expected: Some("http/request".into()),
            timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());

        let none_program = lower(&parse(
            "fn main() = ui.wait_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: none)",
        ))
        .unwrap();
        let Step::Effect(none_request) = vm.start(
            &none_program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected optional UI form field placeholder wait");
        };
        let EffectOperation::Presentation(none_presentation) = &none_request.operation else {
            panic!("optional UI form field placeholder wait must remain frontend-local");
        };
        assert!(matches!(
            &none_presentation.operation,
            PresentationOperation::WaitFormFieldPlaceholder {
                node_id,
                field,
                expected: None,
                timeout_ms,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && *timeout_ms == UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS
        ));
        assert_eq!(
            vm.resume(
                &none_request.continuation,
                EffectResult::Presentation(PresentationResult::WaitFormFieldPlaceholder {
                    node_id: "workspace-runtime-a-deploy".into(),
                    field: "pipeline_kind".into(),
                    expected: None,
                    timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: None,
            })
        );
    }

    #[test]
    fn ui_assert_accessible_name_binds_expected_name_across_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI accessible name assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI accessible name assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertAccessibleName { node_id, expected }
                if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertAccessibleName {
                    node_id: "fleet-title".into(),
                    expected: "Runtime fleet".into(),
                }),
            ),
            Step::Done(Value::UiAssertAccessibleName {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI accessible name assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertAccessibleName {
            node_id: "fleet-title".into(),
            expected: "Forged name".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_accessible_name_binds_expected_name_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI accessible name wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI accessible name wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitAccessibleName {
                node_id,
                expected,
                timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
            } if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitAccessibleName {
                    node_id: "fleet-title".into(),
                    expected: "Runtime fleet".into(),
                    timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitAccessibleName {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
            })
        );

        let mut torn = request;
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI accessible name wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitAccessibleName {
                node_id: "fleet-title".into(),
                expected: "Forged name".into(),
                timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
        {
            let EffectOperation::Presentation(presentation) = &mut torn.operation else {
                panic!("UI accessible name wait must use a presentation envelope");
            };
            presentation.operation = PresentationOperation::WaitAccessibleName {
                node_id: "fleet-title".into(),
                expected: "Runtime fleet".into(),
                timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS + 1,
            };
        }
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_assert_accessible_description_binds_expected_description_across_reentry() {
        let program = lower(&parse(
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI accessible description assertion");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI accessible description assertion must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::AssertAccessibleDescription { node_id, expected }
                if node_id == "runtime-runtime-a-inspect"
                    && expected == "Open the read-only runtime workspace"
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::AssertAccessibleDescription {
                    node_id: "runtime-runtime-a-inspect".into(),
                    expected: "Open the read-only runtime workspace".into(),
                }),
            ),
            Step::Done(Value::UiAssertAccessibleDescription {
                node_id: "runtime-runtime-a-inspect".into(),
                expected: "Open the read-only runtime workspace".into(),
            })
        );

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI accessible description assertion must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::AssertAccessibleDescription {
            node_id: "runtime-runtime-a-inspect".into(),
            expected: "Forged description".into(),
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn ui_wait_accessible_description_binds_expected_description_timeout_and_reentry() {
        let program = lower(&parse(
            "fn main() = ui.wait_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "desktop-operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            None,
        ) else {
            panic!("expected UI accessible description wait");
        };
        let EffectOperation::Presentation(presentation) = &request.operation else {
            panic!("UI accessible description wait must remain frontend-local");
        };
        assert!(matches!(
            &presentation.operation,
            PresentationOperation::WaitAccessibleDescription {
                node_id,
                expected,
                timeout_ms,
            } if node_id == "runtime-runtime-a-inspect"
                && expected == "Open the read-only runtime workspace"
                && *timeout_ms == UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS
        ));
        validate_effect_request(&request).unwrap();
        assert_eq!(
            vm.resume(
                &request.continuation,
                EffectResult::Presentation(PresentationResult::WaitAccessibleDescription {
                    node_id: "runtime-runtime-a-inspect".into(),
                    expected: "Open the read-only runtime workspace".into(),
                    timeout_ms: UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS,
                }),
            ),
            Step::Done(Value::UiWaitAccessibleDescription {
                node_id: "runtime-runtime-a-inspect".into(),
                expected: "Open the read-only runtime workspace".into(),
            })
        );

        let mut torn = request.clone();
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI accessible description wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitAccessibleDescription {
            node_id: "runtime-runtime-a-inspect".into(),
            expected: "Forged description".into(),
            timeout_ms: UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS,
        };
        assert!(validate_effect_request(&torn).is_err());

        let mut torn = request;
        let EffectOperation::Presentation(presentation) = &mut torn.operation else {
            panic!("UI accessible description wait must use a presentation envelope");
        };
        presentation.operation = PresentationOperation::WaitAccessibleDescription {
            node_id: "runtime-runtime-a-inspect".into(),
            expected: "Open the read-only runtime workspace".into(),
            timeout_ms: UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS + 1,
        };
        assert!(validate_effect_request(&torn).is_err());
    }

    #[test]
    fn runtime_inspect_reenters_with_one_typed_projection() {
        let program = lower(&parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        ) else {
            panic!("expected inspect effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("inspect must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeInspect { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");
        let result = control.query(query.clone()).unwrap();
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeInspect {
                revision: Revision(1),
                runtime,
            }) if runtime.id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_history_reenters_with_a_bounded_typed_sequence() {
        let program = lower(&parse(
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        ) else {
            panic!("expected history effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("history must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeHistory { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");
        let result = control.query(query.clone()).unwrap();
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeHistory {
                revision: Revision(1),
                entries,
            }) if entries.is_empty()
        ));
    }

    #[test]
    fn runtime_logs_reenters_with_a_bounded_typed_sequence() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(3)),
        ) else {
            panic!("expected logs effect");
        };
        let EffectOperation::Query(query) = &request.operation else {
            panic!("logs must produce a query");
        };
        assert!(matches!(
            &query.query,
            Query::RuntimeLogs {
                runtime_id,
                after_sequence: None,
                limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
            } if runtime_id.as_str() == "runtime-a"
        ));
        let result = QueryResult::RuntimeLogs {
            revision: Revision(3),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: "Runtime A".to_string(),
            entries: vec![RuntimeLogRecord {
                sequence: 1,
                level: leserpent_domain::RuntimeLogLevel::Warning,
                message: "packet loss detected".to_string(),
            }],
        };
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Done(Value::RuntimeLogs {
                revision: Revision(3),
                runtime_id,
                runtime_name,
                entries,
            }) if runtime_id.as_str() == "runtime-a"
                && runtime_name == "Runtime A"
                && entries.len() == 1
        ));
    }

    #[test]
    fn runtime_logs_rejects_an_overproducing_host() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        ) else {
            panic!("expected logs effect");
        };
        let entry = RuntimeLogRecord {
            sequence: 1,
            level: leserpent_domain::RuntimeLogLevel::Info,
            message: "bounded".to_string(),
        };
        let result = QueryResult::RuntimeLogs {
            revision: Revision(1),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: "Runtime A".to_string(),
            entries: vec![entry; usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES) + 1],
        };
        assert!(matches!(
            vm.resume(&request.continuation, EffectResult::Query(result)),
            Step::Fault(Fault { code, .. }) if code == "LSV2102"
        ));
    }

    #[test]
    fn structured_all_starts_as_an_ordered_atomic_effect_batch() {
        let structured = lower(&parse(
            "fn main() = all(read: runtime.list(), refresh: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let unauthorized = vm.start(
            &structured,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(1)),
        );
        assert!(matches!(
            unauthorized,
            Step::Fault(Fault { ref code, .. }) if code == "LSH2001"
        ));

        let executable = lower(&parse(
            "fn main() = all(left: runtime.list(), right: runtime.list())",
        ))
        .unwrap();
        let started = vm.start(
            &executable,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        let Step::Effects(batch) = started else {
            panic!("expected structured effect batch");
        };
        assert_eq!(batch.merge_token.as_str(), "merge-1");
        assert_eq!(
            batch
                .branches
                .iter()
                .map(|branch| branch.branch.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert_eq!(
            batch
                .branches
                .iter()
                .map(|branch| branch.request.continuation.token.as_str())
                .collect::<Vec<_>>(),
            ["continuation-2", "continuation-3"]
        );
        assert_eq!(vm.pending_count(), 2);
        assert_eq!(vm.completed_count(), 0);

        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(MergeWait {
                completed_branches: 1,
                total_branches: 2,
                ..
            })
        ));
        let second = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&second, 11, runtime_list_result(2)),
            Step::Done(Value::Structured { .. })
        ));
        assert!(matches!(
            vm.merge_result(&batch.merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_structured_start_resumes_remaining_branches_after_restart() {
        let journal = TempJournal::new("structured-start-restart");
        let program = lower(&parse(
            "fn main() = all(left: runtime.list(), right: runtime.list())",
        ))
        .unwrap();
        let merge_token = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let Step::Effects(batch) = vm.start(
                &program,
                Principal {
                    id: "operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                None,
            ) else {
                panic!("expected durable structured batch");
            };
            let first = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
                Step::Waiting(MergeWait {
                    completed_branches: 1,
                    total_branches: 2,
                    ..
                })
            ));
            batch.merge_token.clone()
        };

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_count(), 1);
        assert_eq!(recovered.completed_count(), 1);
        let second = recovered.claim_effect(20, 100).unwrap().unwrap();
        let Step::Done(Value::Structured { fields }) =
            recovered.acknowledge_effect(&second, 21, runtime_list_result(2))
        else {
            panic!("expected restart-safe structured result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert!(matches!(
            recovered.merge_result(&merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn nested_all_fails_before_sequence_allocation() {
        let nested = lower(&parse(
            "fn main() = all(outer: all(left: runtime.list(), right: runtime.list()), tail: runtime.list())",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let unsupported = vm.start(
            &nested,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        assert!(matches!(
            unsupported,
            Step::Fault(Fault { ref code, .. }) if code == "LSV1002"
        ));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            start(&mut vm, None).continuation.token.as_str(),
            "continuation-1"
        );
    }

    fn merge_branch_requests() -> (EffectRequest, EffectRequest, MergePlan) {
        let mut source = Vm::default();
        let left = start(&mut source, None);
        let right = start(&mut source, None);
        let plan = MergePlan::new(["left", "right"]).unwrap();
        (left, right, plan)
    }

    #[test]
    fn durable_merge_graph_creation_recovers_every_branch_in_declared_order() {
        let journal = TempJournal::new("merge-graph-create");
        let (left, right, plan) = merge_branch_requests();
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(
                    &ContinuationToken::from_wire("merge-1".to_string()),
                    &plan,
                    &[("left", &left), ("right", &right)],
                )
                .unwrap();
        }

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_count(), 2);
        let first = recovered.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(first.request, left);
        let second = recovered.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(second.request, right);
        assert!(recovered.claim_effect(10, 100).unwrap().is_none());
    }

    #[test]
    fn ephemeral_merge_graph_creation_uses_the_same_declared_order() {
        let (left, right, plan) = merge_branch_requests();
        let mut vm = Vm::default();
        vm.journal
            .record_merge_graph(
                &ContinuationToken::from_wire("merge-1".to_string()),
                &plan,
                &[("left", &left), ("right", &right)],
            )
            .unwrap();

        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(first.request, left);
        vm.journal
            .acknowledge_dispatch(&first, 11, &Step::Done(runtime_list_value(1)))
            .unwrap();
        let second = vm.claim_effect(10, 100).unwrap().unwrap();
        assert_eq!(second.request, right);
        vm.journal
            .acknowledge_dispatch(&second, 11, &Step::Done(runtime_list_value(2)))
            .unwrap();
        assert!(vm.claim_effect(10, 100).unwrap().is_none());
        let merged = vm
            .journal
            .merge_result(&ContinuationToken::from_wire("merge-1".to_string()))
            .unwrap()
            .unwrap();
        let Step::Done(Value::Structured { fields }) = merged else {
            panic!("expected structured merge result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
    }

    #[test]
    fn ephemeral_compaction_deletes_completed_merge_graphs_as_logical_units() {
        let mut source = Vm::default();
        let requests = (0..4).map(|_| start(&mut source, None)).collect::<Vec<_>>();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let first_token = ContinuationToken::from_wire("merge-1".to_string());
        let second_token = ContinuationToken::from_wire("merge-2".to_string());
        let mut vm = Vm::default();
        vm.journal
            .record_merge_graph(
                &first_token,
                &plan,
                &[("left", &requests[0]), ("right", &requests[1])],
            )
            .unwrap();
        vm.journal
            .record_merge_graph(
                &second_token,
                &plan,
                &[("left", &requests[2]), ("right", &requests[3])],
            )
            .unwrap();

        let mut local_completed = Vec::new();
        for revision in 1..=4 {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = Step::Done(runtime_list_value(revision));
            vm.journal.acknowledge_dispatch(&lease, 11, &step).unwrap();
            local_completed.push((lease.request.continuation.token, step));
        }
        let compacted = vm
            .journal
            .compact_completed(
                &local_completed,
                &RetentionPolicy {
                    max_completed_records: 1,
                    max_delete_per_run: 1,
                },
            )
            .unwrap();
        assert_eq!(compacted.removed_records, 1);
        assert_eq!(compacted.remaining_completed, 1);
        assert_eq!(compacted.pruned_tokens.len(), 2);
        assert!(vm.journal.merge_result(&first_token).unwrap().is_none());
        assert!(vm.journal.merge_result(&second_token).unwrap().is_some());
    }

    #[test]
    fn durable_final_branch_atomically_commits_restart_safe_merge_result() {
        let journal = TempJournal::new("merge-final-completion");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken::from_wire("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let first = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
                Step::Waiting(_)
            ));
            assert!(vm.journal.merge_result(&merge_token).unwrap().is_none());

            let second = vm.claim_effect(10, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&second, 11, runtime_list_result(2)),
                Step::Done(Value::Structured { .. })
            ));
            assert!(matches!(
                vm.journal.merge_result(&merge_token).unwrap(),
                Some(Step::Done(Value::Structured { .. }))
            ));
        }

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let Some(Step::Done(Value::Structured { fields })) =
            recovered.journal.merge_result(&merge_token).unwrap()
        else {
            panic!("expected recovered structured merge result");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert!(matches!(
            fields[0].value.as_ref(),
            Value::RuntimeList {
                revision: Revision(1),
                ..
            }
        ));
        assert!(matches!(
            fields[1].value.as_ref(),
            Value::RuntimeList {
                revision: Revision(2),
                ..
            }
        ));
    }

    #[test]
    fn durable_merge_failure_rolls_back_the_final_branch_completion() {
        let journal = TempJournal::new("merge-final-rollback");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken::from_wire("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(_)
        ));
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_merge_terminal
                 BEFORE UPDATE OF state ON vm_merge_groups
                 WHEN NEW.state = 'completed'
                 BEGIN
                   SELECT RAISE(ABORT, 'injected merge completion failure');
                 END;",
            )
            .unwrap();
        let second = vm.claim_effect(20, 100).unwrap().unwrap();
        let failed = vm.acknowledge_effect(&second, 21, runtime_list_result(2));
        assert!(matches!(failed, Step::Fault(Fault { ref code, .. }) if code == "LSV4033"));

        let group_state: String = connection
            .query_row(
                "SELECT state FROM vm_merge_groups WHERE token = 'merge-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let second_state: String = connection
            .query_row(
                "SELECT state FROM vm_effects WHERE token = 'continuation-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let dispatch_state: String = connection
            .query_row(
                "SELECT state FROM vm_dispatches WHERE token = 'continuation-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_state, "pending");
        assert_eq!(second_state, "pending");
        assert_eq!(dispatch_state, "leased");
    }

    #[test]
    fn durable_merge_commits_declared_terminal_failure() {
        let journal = TempJournal::new("merge-terminal-failure");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken::from_wire("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }

        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let first = vm.claim_effect(10, 100).unwrap().unwrap();
        assert!(matches!(
            vm.acknowledge_effect(&first, 11, runtime_list_result(1)),
            Step::Waiting(_)
        ));
        let second = vm.claim_effect(20, 100).unwrap().unwrap();
        let disposition = vm
            .report_effect_error(
                &second,
                21,
                EffectError {
                    class: EffectErrorClass::Permanent,
                    code: "runtime.rejected".to_string(),
                    message: "runtime rejected the branch".to_string(),
                },
                &RetryPolicy::default(),
            )
            .unwrap();
        assert!(matches!(
            disposition,
            RetryDisposition::Terminal(Step::Failed(_))
        ));
        assert!(matches!(
            vm.journal.merge_result(&merge_token).unwrap(),
            Some(Step::Failed(FailedEffect {
                error: EffectError { ref code, .. },
                ..
            })) if code == "runtime.rejected"
        ));
    }

    #[test]
    fn retention_keeps_a_completed_merge_graph_within_the_configured_limit() {
        let journal = TempJournal::new("merge-compaction-protection");
        let (left, right, plan) = merge_branch_requests();
        let merge_token = ContinuationToken::from_wire("merge-1".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(&merge_token, &plan, &[("left", &left), ("right", &right)])
                .unwrap();
        }
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        for revision in [1, 2] {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = vm.acknowledge_effect(&lease, 11, runtime_list_result(revision));
            assert!(if revision == 1 {
                matches!(step, Step::Waiting(_))
            } else {
                matches!(step, Step::Done(Value::Structured { .. }))
            });
        }
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 100,
            })
            .unwrap();
        assert_eq!(report.removed_records, 0);
        drop(vm);

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(matches!(
            recovered.journal.merge_result(&merge_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_compaction_deletes_the_oldest_completed_merge_graph_as_one_unit() {
        let journal = TempJournal::new("merge-group-compaction");
        let mut source = Vm::default();
        let requests = (0..4).map(|_| start(&mut source, None)).collect::<Vec<_>>();
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let first_token = ContinuationToken::from_wire("merge-1".to_string());
        let second_token = ContinuationToken::from_wire("merge-2".to_string());
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            vm.journal
                .record_merge_graph(
                    &first_token,
                    &plan,
                    &[("left", &requests[0]), ("right", &requests[1])],
                )
                .unwrap();
            vm.journal
                .record_merge_graph(
                    &second_token,
                    &plan,
                    &[("left", &requests[2]), ("right", &requests[3])],
                )
                .unwrap();
        }

        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        for revision in 1..=4 {
            let lease = vm.claim_effect(10, 100).unwrap().unwrap();
            let step = vm.acknowledge_effect(&lease, 11, runtime_list_result(revision));
            assert!(if revision % 2 == 1 {
                matches!(step, Step::Waiting(_))
            } else {
                matches!(step, Step::Done(Value::Structured { .. }))
            });
        }
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 1,
            })
            .unwrap();
        assert_eq!(report.removed_records, 1);
        assert_eq!(report.remaining_completed, 1);
        assert!(report.reclaimed_logical_bytes > 0);
        assert_eq!(vm.completed_count(), 2);
        assert!(vm.journal.merge_result(&first_token).unwrap().is_none());
        assert!(matches!(
            vm.journal.merge_result(&second_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
        drop(vm);

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_merge_groups),
                   (SELECT COUNT(*) FROM vm_merge_branches),
                   (SELECT COUNT(*) FROM vm_effects),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 2, 2, 2));
        drop(connection);

        let recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.completed_count(), 2);
        assert!(matches!(
            recovered.journal.merge_result(&second_token).unwrap(),
            Some(Step::Done(Value::Structured { .. }))
        ));
    }

    #[test]
    fn durable_merge_graph_creation_rolls_back_every_row_on_branch_failure() {
        let journal = TempJournal::new("merge-graph-rollback");
        let (left, right, plan) = merge_branch_requests();
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_second_merge_branch
                 BEFORE INSERT ON vm_effects
                 WHEN NEW.token = 'continuation-2'
                 BEGIN
                   SELECT RAISE(ABORT, 'injected branch failure');
                 END;",
            )
            .unwrap();

        let error = vm
            .journal
            .record_merge_graph(
                &ContinuationToken::from_wire("merge-1".to_string()),
                &plan,
                &[("left", &left), ("right", &right)],
            )
            .unwrap_err();
        assert_eq!(error.code, "LSV4032");
        for table in [
            "vm_merge_groups",
            "vm_merge_branches",
            "vm_effects",
            "vm_dispatches",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} must roll back with the graph");
        }
    }

    #[test]
    fn structured_merge_uses_declaration_order_not_arrival_order() {
        let plan = MergePlan::new(["inventory", "health", "history"]).unwrap();
        let merged = merge_declared(
            &plan,
            vec![
                completed_branch("history", 3),
                completed_branch("inventory", 1),
                completed_branch("health", 2),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        let Step::Done(Value::Structured { fields }) = merged else {
            panic!("expected structured value");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["inventory", "health", "history"]
        );
        for (field, expected_revision) in fields.iter().zip(1..=3) {
            assert!(matches!(
                field.value.as_ref(),
                Value::RuntimeList { revision, .. } if *revision == Revision(expected_revision)
            ));
        }
    }

    #[test]
    fn structured_merge_terminal_winner_is_deterministic() {
        let plan = MergePlan::new(["primary", "secondary", "fallback"]).unwrap();
        let merged = merge_declared(
            &plan,
            vec![
                BranchCompletion {
                    branch: "fallback".to_string(),
                    outcome: BranchOutcome::Cancelled(Cancellation {
                        continuation: ContinuationToken::from_wire("continuation-3".to_string()),
                        reason: CancellationReason::Requested,
                        observed_at_ms: 30,
                    }),
                },
                BranchCompletion {
                    branch: "secondary".to_string(),
                    outcome: BranchOutcome::Fault(Fault {
                        code: "SECOND".to_string(),
                        message: "secondary failed first in declared order".to_string(),
                    }),
                },
                completed_branch("primary", 1),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        assert!(matches!(merged, Step::Fault(Fault { ref code, .. }) if code == "SECOND"));
    }

    #[test]
    fn structured_merge_rejects_invalid_sets_and_resource_budgets() {
        assert_eq!(MergePlan::new(["only"]).unwrap_err().code, "LSV2401");
        assert_eq!(
            MergePlan::new(["same", "same"]).unwrap_err().code,
            "LSV2401"
        );
        let plan = MergePlan::new(["left", "right"]).unwrap();
        assert_eq!(
            merge_declared(
                &plan,
                vec![completed_branch("left", 1), completed_branch("left", 2)],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2402"
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![completed_branch("left", 1)],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2402"
        );

        let mut control = InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "left".to_string(),
                        outcome: BranchOutcome::Value(Value::RuntimeList {
                            revision: Revision(1),
                            runtimes: vec![runtime],
                        }),
                    },
                    completed_branch("right", 2),
                ],
                0,
            )
            .unwrap_err()
            .code,
            "LSV2404"
        );
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "left".to_string(),
                        outcome: BranchOutcome::Fault(Fault {
                            code: "TOO_LARGE".to_string(),
                            message: "x".repeat(4_097),
                        }),
                    },
                    completed_branch("right", 2),
                ],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2403"
        );
    }

    #[test]
    fn structured_merge_rejects_excessive_nesting() {
        let mut nested = runtime_list_value(1);
        for depth in 0..MAX_STRUCTURED_VALUE_DEPTH {
            nested = Value::Structured {
                fields: vec![
                    StructuredField {
                        name: "leaf".to_string(),
                        value: Box::new(runtime_list_value(depth as u64)),
                    },
                    StructuredField {
                        name: "nested".to_string(),
                        value: Box::new(nested),
                    },
                ],
            };
        }
        let plan = MergePlan::new(["deep", "peer"]).unwrap();
        assert_eq!(
            merge_declared(
                &plan,
                vec![
                    BranchCompletion {
                        branch: "deep".to_string(),
                        outcome: BranchOutcome::Value(nested),
                    },
                    completed_branch("peer", 2),
                ],
                DEFAULT_MAX_OUTPUT_ITEMS,
            )
            .unwrap_err()
            .code,
            "LSV2403"
        );
    }

    #[test]
    fn legacy_query_effect_request_decodes_to_current_operation() {
        let mut vm = Vm::default();
        let request = start(&mut vm, None);
        let EffectOperation::Query(query) = request.operation.clone() else {
            panic!("expected query operation");
        };
        let legacy = serde_json::json!({
            "effect_id": request.effect_id,
            "required_capability": request.required_capability,
            "query": query,
            "continuation": request.continuation,
            "budget": request.budget,
        });
        let decoded: EffectRequest = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded, request);
    }

    #[test]
    fn effect_request_rejects_long_non_numeric_identity_without_panicking() {
        let mut vm = Vm::default();
        let mut request = start_refresh(&mut vm, Revision(1));
        let suffix = "x".repeat(115);
        request.continuation.token = ContinuationToken::from_wire(format!("continuation-{suffix}"));
        request.effect_id = format!("effect-{suffix}");
        let encoded = serde_json::to_vec(&request).unwrap();
        let error = serde_json::from_slice::<EffectRequest>(&encoded)
            .expect_err("non-canonical external identities must fail during decoding");
        assert!(error.to_string().contains("LSV2010"));
    }

    #[test]
    fn mutating_effect_cannot_bypass_leased_command_envelope() {
        let mut vm = Vm::default();
        let request = start_refresh(&mut vm, Revision(1));
        let result = CommandResult {
            command_id: CommandId::new("leselang-command-1").unwrap(),
            status: CommandStatus::Applied,
            runtime: InMemoryControlPlane::default().register_runtime(
                RuntimeId::new("runtime-a").unwrap(),
                "Alpha",
                "http://runtime-a",
            ),
            events: Vec::new(),
        };
        let step = vm.resume(
            &request.continuation,
            EffectResult::Command(Box::new(result)),
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSV2110"));
    }

    #[test]
    fn capabilities_refresh_reenters_only_through_dispatch_acknowledgement() {
        let mut vm = Vm::default();
        start_capabilities_refresh(&mut vm, Revision(1));
        let lease = vm.claim_effect(1_000, 50).unwrap().unwrap();
        let EffectOperation::Command(command) = lease.request.operation.clone() else {
            panic!("expected command operation");
        };
        assert!(matches!(
            command.command,
            Command::RuntimeCapabilitiesRefresh { .. }
        ));

        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        let result = control.execute(command).unwrap();
        let completed = vm.acknowledge_effect(
            &lease,
            1_001,
            EffectResult::Command(Box::new(result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeCapabilitiesRefresh { result: ref value })
                if **value == result
        ));
    }

    #[test]
    fn deployment_is_confirmed_and_reenters_only_through_dispatch_acknowledgement() {
        let mut vm = Vm::default();
        start_deploy(&mut vm, Revision(1));
        let lease = vm.claim_effect(1_000, 50).unwrap().unwrap();
        let EffectOperation::Command(command) = lease.request.operation.clone() else {
            panic!("expected deployment command");
        };
        assert_eq!(command.confirmation, Confirmation::Confirmed);
        assert!(matches!(
            command.command,
            Command::RuntimeDeploy {
                ref pipeline_kind,
                target: Some(ref target),
                ..
            } if pipeline_kind == "http/request" && target == "pid:42"
        ));

        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );
        let result = control.execute(command).unwrap();
        let completed = vm.acknowledge_effect(
            &lease,
            1_001,
            EffectResult::Command(Box::new(result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeDeploy { result: ref value }) if **value == result
        ));
    }

    #[test]
    fn debugger_cancel_is_confirmed_correlated_and_restart_safe() {
        let journal = TempJournal::new("debugger-cancel-language");
        let program = lower(&parse(
            "fn main() = debugger.cancel(session_id: \"session-a\")",
        ))
        .unwrap();
        let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let Step::Effect(request) = vm.start(
            &program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
            Some(Revision(7)),
        ) else {
            panic!("expected debugger cancellation effect");
        };
        let EffectOperation::Command(command) = &request.operation else {
            panic!("expected debugger cancellation command");
        };
        assert_eq!(command.confirmation, Confirmation::Confirmed);
        assert!(matches!(
            command.command,
            Command::DebuggerCancel { ref session_id } if session_id == "session-a"
        ));
        drop(vm);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let lease = recovered.claim_effect(1_000, 50).unwrap().unwrap();
        let result = DebuggerCancelResult {
            command_id: command.command_id.clone(),
            session_id: "session-a".into(),
            observed_at_ms: 1_001,
        };
        let completed = recovered.acknowledge_effect(
            &lease,
            1_001,
            EffectResult::DebuggerCancel(result.clone()),
        );
        assert_eq!(completed, Step::Done(Value::DebuggerCancel { result }));
    }

    #[test]
    fn completion_consumes_once_and_duplicate_delivery_is_idempotent() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(7)));
        let result = runtime_list_result(7);
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Done(Value::RuntimeList { .. })));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn ephemeral_compaction_is_bounded_and_orders_numeric_continuations() {
        let mut vm = Vm::default();
        let mut requests = Vec::new();
        for revision in 1..=11 {
            let request = start(&mut vm, None);
            assert!(matches!(
                vm.resume(&request.continuation, runtime_list_result(revision)),
                Step::Done(_)
            ));
            requests.push(request);
        }

        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 2,
                max_delete_per_run: 3,
            })
            .unwrap();
        assert_eq!(report.removed_records, 3);
        assert_eq!(report.remaining_completed, 8);
        assert!(report.reclaimed_logical_bytes > 0);
        assert_eq!(vm.completed_count(), 8);
        for request in requests.iter().take(3) {
            assert!(matches!(
                vm.resume(&request.continuation, runtime_list_result(99)),
                Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
            ));
        }
        assert!(matches!(
            vm.resume(&requests[9].continuation, runtime_list_result(99)),
            Step::Done(Value::RuntimeList {
                revision: Revision(10),
                ..
            })
        ));
    }

    #[test]
    fn retention_policy_is_bounded() {
        let mut vm = Vm::default();
        for policy in [
            RetentionPolicy {
                max_completed_records: 0,
                max_delete_per_run: 1,
            },
            RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: MAX_COMPACTION_BATCH + 1,
            },
        ] {
            assert_eq!(vm.compact_journal(&policy).unwrap_err().code, "LSV2301");
        }
    }

    #[test]
    fn continuation_survives_serialization_and_process_restart() {
        let mut first_vm = Vm::default();
        let request = start(&mut first_vm, None);
        let encoded = encode_continuation(&request.continuation).unwrap();
        let restored_image = decode_continuation(&encoded).unwrap();

        let mut restarted_vm = Vm::default();
        restarted_vm.restore(restored_image.clone()).unwrap();
        let step = restarted_vm.resume(&restored_image, runtime_list_result(11));
        assert!(matches!(
            step,
            Step::Done(Value::RuntimeList {
                revision: Revision(11),
                ..
            })
        ));
    }

    #[test]
    fn revision_conflict_is_typed_and_idempotently_replayed() {
        let mut vm = Vm::default();
        let request = start(&mut vm, Some(Revision(3)));
        let result = runtime_list_result(4);
        let first = vm.resume(&request.continuation, result.clone());
        let duplicate = vm.resume(&request.continuation, result);
        assert_eq!(first, duplicate);
        assert!(matches!(first, Step::Fault(Fault { ref code, .. }) if code == "LSV2101"));
    }

    #[test]
    fn missing_capability_and_invalid_images_fail_without_effects() {
        let mut vm = Vm::default();
        let step = vm.start(
            &program(),
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::default(),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSH2001"));
        assert_eq!(vm.pending_count(), 0);

        let mut image = start(&mut vm, None).continuation;
        image.schema_version = 99;
        assert_eq!(encode_continuation(&image).unwrap_err().code, "LSV2001");
        assert_eq!(
            decode_continuation(&vec![b' '; MAX_CONTINUATION_BYTES + 1])
                .unwrap_err()
                .code,
            "LSV3002"
        );

        let mut forged_budget = start(&mut vm, None).continuation;
        forged_budget.deadline_ms = 0;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2008"
        );
        forged_budget.deadline_ms = DEFAULT_EFFECT_DEADLINE_MS;
        forged_budget.max_output_items = DEFAULT_MAX_OUTPUT_ITEMS + 1;
        assert_eq!(
            encode_continuation(&forged_budget).unwrap_err().code,
            "LSV2009"
        );
    }

    #[test]
    fn continuation_decode_rejects_ambiguous_or_incoherent_state() {
        let mut vm = Vm::default();
        let valid = start(&mut vm, None).continuation;

        let mut unknown_root = serde_json::to_value(&valid).unwrap();
        unknown_root["future_field"] = serde_json::json!(true);
        assert_eq!(
            decode_continuation(&serde_json::to_vec(&unknown_root).unwrap())
                .unwrap_err()
                .code,
            "LSV3003"
        );

        let mut unknown_effect = serde_json::to_value(&valid).unwrap();
        unknown_effect["pending_effect"]["future_field"] = serde_json::json!(true);
        assert_eq!(
            decode_continuation(&serde_json::to_vec(&unknown_effect).unwrap())
                .unwrap_err()
                .code,
            "LSV3003"
        );

        let mut unsupported_pc = valid.clone();
        unsupported_pc.program_counter = 2;
        assert_eq!(
            encode_continuation(&unsupported_pc).unwrap_err().code,
            "LSV2014"
        );

        let mut mismatched_type = valid.clone();
        mismatched_type.result_type = Type::RuntimeInspect;
        assert_eq!(
            encode_continuation(&mismatched_type).unwrap_err().code,
            "LSV2016"
        );

        let mut structured = valid;
        structured.pending_effect = Effect::All {
            branches: Vec::new(),
        };
        structured.result_type = Type::Structured;
        assert_eq!(
            encode_continuation(&structured).unwrap_err().code,
            "LSV2015"
        );
    }

    #[test]
    fn effect_request_decode_is_strict_and_semantically_validated() {
        let mut vm = Vm::default();
        let request = start(&mut vm, None);

        let mut unknown = serde_json::to_value(&request).unwrap();
        unknown["future_field"] = serde_json::json!(true);
        assert!(serde_json::from_value::<EffectRequest>(unknown).is_err());

        let mut incoherent = serde_json::to_value(&request).unwrap();
        incoherent["budget"]["fuel_remaining"] = serde_json::json!(0);
        assert!(serde_json::from_value::<EffectRequest>(incoherent).is_err());
    }

    #[test]
    fn source_to_hir_to_effect_to_domain_to_value_is_one_vertical_slice() {
        let mut vm = Vm::default();
        let unfiltered = lower(&parse("fn main() = runtime.list()")).unwrap();
        let Step::Effect(request) = vm.start(
            &unfiltered,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        ) else {
            panic!("expected effect");
        };
        let request = *request;
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-b").unwrap(), "Bravo", "http://b");
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");

        let EffectOperation::Query(query) = request.operation else {
            panic!("expected query operation");
        };
        let result = control.query(query).unwrap();
        let step = vm.resume(&request.continuation, EffectResult::Query(result));
        let Step::Done(Value::RuntimeList { runtimes, .. }) = step else {
            panic!("vertical slice should complete");
        };
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn restored_tokens_do_not_collide_with_new_effects() {
        let mut first = Vm::default();
        let restored = start(&mut first, None).continuation;
        let mut restarted = Vm::default();
        restarted.restore(restored).unwrap();
        let next = start(&mut restarted, None);
        assert_eq!(next.continuation.token.as_str(), "continuation-2");
    }

    #[test]
    fn exhausted_restored_sequence_fails_without_spinning() {
        let mut source = Vm::default();
        let mut image = start(&mut source, None).continuation;
        image.token = ContinuationToken::from_wire(format!("continuation-{MAX_EFFECT_SEQUENCE}"));

        let mut restored = Vm::default();
        restored.restore(image.clone()).unwrap();
        assert!(matches!(
            restored.start(
                &program(),
                Principal {
                    id: "operator".to_string(),
                },
                CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                None,
            ),
            Step::Fault(Fault { ref code, .. }) if code == "LSV1003"
        ));

        image.token =
            ContinuationToken::from_wire(format!("continuation-{}", MAX_EFFECT_SEQUENCE + 1));
        let mut rejected = Vm::default();
        assert_eq!(rejected.restore(image).unwrap_err().code, "LSV2006");
    }

    #[test]
    fn output_and_serialized_continuation_limits_are_enforced() {
        let mut first = Vm::default();
        let mut image = start(&mut first, None).continuation;
        image.max_output_items = 0;
        let mut restarted = Vm::default();
        restarted.restore(image.clone()).unwrap();
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "Alpha", "http://a");
        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            })
            .unwrap();
        assert!(matches!(
            restarted.resume(&image, EffectResult::Query(result)),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2102"
        ));

        let large_value = "x".repeat(MAX_CONTINUATION_BYTES);
        // Untrusted HIR must still be bounded even though source rejects oversized filters.
        let mut large_program = program();
        large_program.function.effect = Effect::RuntimeList {
            filter: RuntimeListFilter {
                environment: Some(large_value.clone()),
                cluster: None,
                role: None,
            },
        };
        let step = Vm::default().start(
            &large_program,
            Principal {
                id: "operator".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            None,
        );
        assert!(matches!(step, Step::Fault(Fault { ref code, .. }) if code == "LSV3002"));

        let mut source_vm = Vm::default();
        let mut oversized_image = start(&mut source_vm, None).continuation;
        oversized_image.pending_effect = Effect::RuntimeList {
            filter: RuntimeListFilter {
                environment: Some(large_value),
                cluster: None,
                role: None,
            },
        };
        assert_eq!(
            Vm::default().restore(oversized_image).unwrap_err().code,
            "LSV3002"
        );
    }

    #[test]
    fn durable_journal_recovers_pending_and_replays_completion_after_restart() {
        let journal = TempJournal::new("restart");
        let request = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start(&mut first, None);
            assert_eq!(first.pending_count(), 1);
            request
        };

        let first_step = {
            let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            assert_eq!(restarted.pending_count(), 1);
            assert_eq!(
                restarted.pending_continuations(),
                vec![request.continuation.clone()]
            );
            restarted.resume(&request.continuation, runtime_list_result(21))
        };
        assert!(matches!(
            first_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(21),
                ..
            })
        ));

        let mut restarted_again = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted_again.pending_count(), 0);
        assert_eq!(restarted_again.completed_count(), 1);
        let replay = restarted_again.resume(&request.continuation, runtime_list_result(99));
        assert_eq!(replay, first_step);
    }

    #[test]
    fn durable_debugger_audit_survives_restart_and_enforces_idempotency() {
        let journal = TempJournal::new("debugger-audit-restart");
        let request;
        let audit = DebuggerAuditContext {
            command_id: CommandId::new("debugger-command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("debugger-idempotency-a").unwrap(),
            principal: Principal {
                id: "debugger-operator".to_string(),
            },
            origin: CommandOrigin::Gui,
            session_id: "session-a".to_string(),
            expected_revision: Revision(7),
        };
        let first_record = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            request = start(&mut vm, Some(Revision(7)));
            vm.cancel_effect_audited(&request.continuation, 1_250, &audit)
                .unwrap()
        };
        assert_eq!(first_record.observed_at_ms, 1_250);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(
            restarted.debugger_audit(&audit.command_id).unwrap(),
            Some(first_record.clone())
        );
        assert_eq!(
            restarted
                .cancel_effect_audited(&request.continuation, 1_500, &audit)
                .unwrap(),
            first_record
        );

        let conflicting = DebuggerAuditContext {
            command_id: CommandId::new("debugger-command-b").unwrap(),
            ..audit.clone()
        };
        let error = restarted
            .cancel_effect_audited(&request.continuation, 1_750, &conflicting)
            .unwrap_err();
        assert_eq!(error.code, "LSV4033");

        let encoded = serde_json::to_string(&first_record).unwrap();
        assert!(!encoded.contains(request.continuation.token.as_str()));
        assert!(!encoded.contains(audit.idempotency_key.as_str()));
    }

    #[test]
    fn durable_journal_rejects_orphaned_debugger_audit() {
        let journal = TempJournal::new("debugger-audit-orphan");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_debugger_audit(
                    command_id, principal_id, idempotency_key, origin, session_id,
                    expected_revision, observed_at_ms, continuation_token
                 ) VALUES ('debugger-command-a', 'debugger-operator',
                           'debugger-idempotency-a', 'gui', 'session-a', 7, 1250,
                           'missing-continuation')",
                [],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_replays_structured_merge_output() {
        let journal = TempJournal::new("structured-merge");
        let request;
        let merged = merge_declared(
            &MergePlan::new(["inventory", "health"]).unwrap(),
            vec![
                completed_branch("health", 2),
                completed_branch("inventory", 1),
            ],
            DEFAULT_MAX_OUTPUT_ITEMS,
        )
        .unwrap();
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            request = start(&mut vm, None);
            vm.journal
                .record_completed(&request.continuation, &merged)
                .unwrap();
        }

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted.completed_count(), 1);
        assert_eq!(
            restarted.resume(&request.continuation, runtime_list_result(99)),
            merged
        );
    }

    #[test]
    fn durable_compaction_deletes_only_oldest_completed_records() {
        let journal = TempJournal::new("compaction");
        let pending;
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let mut completed = Vec::new();
            for revision in 1..=3 {
                let request = start(&mut vm, None);
                assert!(matches!(
                    vm.resume(&request.continuation, runtime_list_result(revision)),
                    Step::Done(_)
                ));
                completed.push(request);
            }
            pending = start(&mut vm, None);

            let report = vm
                .compact_journal(&RetentionPolicy {
                    max_completed_records: 1,
                    max_delete_per_run: 1,
                })
                .unwrap();
            assert_eq!(report.removed_records, 1);
            assert_eq!(report.remaining_completed, 2);
            assert!(report.reclaimed_logical_bytes > 0);
            assert_eq!(vm.pending_count(), 1);
            assert_eq!(vm.completed_count(), 2);
            assert!(matches!(
                vm.resume(&completed[0].continuation, runtime_list_result(99)),
                Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
            ));
        }

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'),
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 2, 3));
        drop(connection);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(restarted.pending_count(), 1);
        assert_eq!(restarted.completed_count(), 2);
        let report = restarted
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 10,
            })
            .unwrap();
        assert_eq!(report.removed_records, 1);
        assert_eq!(report.remaining_completed, 1);
        assert!(
            restarted
                .pending_continuations()
                .contains(&pending.continuation)
        );

        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'pending'),
                   (SELECT COUNT(*) FROM vm_effects WHERE state = 'completed'),
                   (SELECT COUNT(*) FROM vm_dispatches)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 2));
    }

    #[test]
    fn durable_compaction_invalidates_stale_live_vm_replay() {
        let journal = TempJournal::new("compaction-live-vms");
        let oldest;
        {
            let mut creator = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            oldest = start(&mut creator, None);
            creator.resume(&oldest.continuation, runtime_list_result(1));
            let newest = start(&mut creator, None);
            creator.resume(&newest.continuation, runtime_list_result(2));
        }

        let mut stale = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let mut compactor = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        };
        assert_eq!(
            compactor.compact_journal(&policy).unwrap().removed_records,
            1
        );
        assert_eq!(stale.completed_count(), 2);
        assert!(matches!(
            stale.resume(&oldest.continuation, runtime_list_result(99)),
            Step::Fault(Fault { ref code, .. }) if code == "LSV2004"
        ));
        assert_eq!(stale.completed_count(), 1);

        let reconciled = stale.compact_journal(&policy).unwrap();
        assert_eq!(reconciled.removed_records, 0);
        assert_eq!(reconciled.remaining_completed, 1);
    }

    #[test]
    fn durable_sequence_allocation_does_not_collide_between_live_vms() {
        let journal = TempJournal::new("sequences");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_request = start(&mut first, None);
        let second_request = start(&mut second, None);

        assert_eq!(first_request.continuation.token.as_str(), "continuation-1");
        assert_eq!(second_request.continuation.token.as_str(), "continuation-2");
        assert_ne!(first_request.effect_id, second_request.effect_id);
    }

    #[test]
    fn concurrent_completion_uses_the_first_durable_result() {
        let journal = TempJournal::new("completion-race");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let request = start(&mut first, None);
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        let first_step = first.resume(&request.continuation, runtime_list_result(31));
        let competing_step = second.resume(&request.continuation, runtime_list_result(32));

        assert_eq!(competing_step, first_step);
        assert!(matches!(
            competing_step,
            Step::Done(Value::RuntimeList {
                revision: Revision(31),
                ..
            })
        ));
    }

    #[test]
    fn dispatch_lease_is_fenced_and_acknowledged_in_ephemeral_mode() {
        let mut vm = Vm::default();
        let request = start(&mut vm, None);
        let lease = vm
            .claim_effect(1_000, DEFAULT_DISPATCH_LEASE_MS)
            .unwrap()
            .unwrap();
        assert_eq!(lease.request, request);
        assert_eq!(lease.attempt, 1);
        assert!(
            vm.claim_effect(1_001, DEFAULT_DISPATCH_LEASE_MS)
                .unwrap()
                .is_none()
        );

        let direct = vm.resume(&request.continuation, runtime_list_result(1));
        assert!(matches!(direct, Step::Fault(Fault { ref code, .. }) if code == "LSV4024"));

        let completed = vm.acknowledge_effect(&lease, 1_001, runtime_list_result(2));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(2),
                ..
            })
        ));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn timed_effect_requires_trusted_clock_and_completes_before_deadline() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);
        assert_eq!(request.continuation.deadline_ms, 50);
        assert_eq!(request.continuation.deadline_at_ms, Some(1_050));
        assert_eq!(request.budget.deadline_at_ms, Some(1_050));

        let untrusted = vm.resume(&request.continuation, runtime_list_result(1));
        assert!(matches!(untrusted, Step::Fault(Fault { ref code, .. }) if code == "LSV2111"));
        let completed = vm.resume_at(&request.continuation, 1_049, runtime_list_result(2));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(2),
                ..
            })
        ));
    }

    #[test]
    fn timed_effect_rejects_invalid_scheduler_budgets() {
        let mut vm = Vm::default();
        let principal = Principal {
            id: "operator".to_string(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_READ]);
        let zero = vm.start_timed(
            &program(),
            principal.clone(),
            capabilities.clone(),
            None,
            1_000,
            0,
        );
        assert!(matches!(zero, Step::Fault(Fault { ref code, .. }) if code == "LSV2012"));
        let overflow = vm.start_timed(
            &program(),
            principal,
            capabilities,
            None,
            i64::MAX as u64,
            1,
        );
        assert!(matches!(overflow, Step::Fault(Fault { ref code, .. }) if code == "LSV2011"));
        assert_eq!(vm.pending_count(), 0);
    }

    #[test]
    fn trusted_deadline_atomically_cancels_ready_effect() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);

        assert!(vm.claim_effect(1_050, 100).unwrap().is_none());
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
        let replay = vm.cancel_effect(&request.continuation, 1_051);
        assert!(matches!(
            replay,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));
    }

    #[test]
    fn deadline_precedes_requested_cancellation_at_same_clock_tick() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 50);

        let cancelled = vm.cancel_effect(&request.continuation, 1_050);
        assert!(matches!(
            cancelled,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));
    }

    #[test]
    fn requested_cancellation_fences_active_lease() {
        let mut vm = Vm::default();
        let request = start_timed(&mut vm, 1_000, 100);
        let lease = vm.claim_effect(1_001, 90).unwrap().unwrap();

        let cancelled = vm.cancel_effect(&request.continuation, 1_010);
        assert!(matches!(
            cancelled,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::Requested,
                observed_at_ms: 1_010,
                ..
            })
        ));
        assert_eq!(
            vm.acknowledge_effect(&lease, 1_011, runtime_list_result(3)),
            cancelled
        );
        assert!(vm.claim_effect(1_200, 50).unwrap().is_none());
    }

    #[test]
    fn durable_timeout_survives_restart_and_fences_late_worker() {
        let journal = TempJournal::new("timeout-restart");
        let (request, lease) = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start_timed(&mut first, 1_000, 50);
            let lease = first.claim_effect(1_001, 100).unwrap().unwrap();
            (request, lease)
        };

        let timeout = {
            let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            assert!(restarted.claim_effect(1_050, 100).unwrap().is_none());
            assert_eq!(restarted.pending_count(), 0);
            restarted.cancel_effect(&request.continuation, 1_051)
        };
        assert!(matches!(
            timeout,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            })
        ));

        let mut late_worker = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(
            late_worker.acknowledge_effect(&lease, 1_051, runtime_list_result(4)),
            timeout
        );
        assert!(late_worker.claim_effect(2_000, 100).unwrap().is_none());
    }

    #[test]
    fn semantic_retry_waits_for_deterministic_backoff_and_exhausts_typed() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 10_000);
        let first = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let policy = RetryPolicy {
            max_retries: 1,
            base_delay_ms: 250,
            max_delay_ms: 1_000,
        };

        let scheduled = vm
            .report_effect_error(&first, 1_001, transient_error(), &policy)
            .unwrap();
        assert_eq!(
            scheduled,
            RetryDisposition::Scheduled(RetrySchedule {
                retry_count: 1,
                ready_at_ms: 1_251,
                error: transient_error(),
            })
        );
        assert!(vm.claim_effect(1_250, 100).unwrap().is_none());
        let second = vm.claim_effect(1_251, 100).unwrap().unwrap();
        assert_eq!(second.attempt, 2);
        assert_eq!(second.retry_count, 1);

        let exhausted = vm
            .report_effect_error(&second, 1_252, transient_error(), &policy)
            .unwrap();
        assert!(matches!(
            exhausted,
            RetryDisposition::Terminal(Step::Failed(FailedEffect { retry_count: 1, .. }))
        ));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
        assert!(vm.claim_effect(2_000, 100).unwrap().is_none());
    }

    #[test]
    fn permanent_effect_error_fails_without_retry() {
        let mut vm = Vm::default();
        start(&mut vm, None);
        let lease = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let error = EffectError {
            class: EffectErrorClass::Permanent,
            code: "request.rejected".to_string(),
            message: "upstream rejected the request".to_string(),
        };
        let failed = vm
            .report_effect_error(&lease, 1_001, error.clone(), &RetryPolicy::default())
            .unwrap();
        assert_eq!(
            failed,
            RetryDisposition::Terminal(Step::Failed(FailedEffect {
                error,
                retry_count: 0,
            }))
        );
    }

    #[test]
    fn retry_schedule_fences_old_lease_and_deadline_still_wins() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 100);
        let lease = vm.claim_effect(1_001, 90).unwrap().unwrap();
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay_ms: 250,
            max_delay_ms: 250,
        };
        vm.report_effect_error(&lease, 1_002, transient_error(), &policy)
            .unwrap();
        let stale = vm.report_effect_error(&lease, 1_003, transient_error(), &policy);
        assert!(matches!(stale, Err(Fault { ref code, .. }) if code == "LSV4022"));

        assert!(vm.claim_effect(1_100, 100).unwrap().is_none());
        let terminal = vm.cancel_effect(&lease.request.continuation, 1_101);
        assert!(matches!(
            terminal,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_100,
                ..
            })
        ));
    }

    #[test]
    fn deadline_precedes_invalid_effect_error_report() {
        let mut vm = Vm::default();
        start_timed(&mut vm, 1_000, 50);
        let lease = vm.claim_effect(1_001, 100).unwrap().unwrap();
        let invalid = EffectError {
            class: EffectErrorClass::Transient,
            code: "bad code".to_string(),
            message: String::new(),
        };

        let disposition = vm
            .report_effect_error(&lease, 1_050, invalid, &RetryPolicy::default())
            .unwrap();
        assert!(matches!(
            disposition,
            RetryDisposition::Terminal(Step::Cancelled(Cancellation {
                reason: CancellationReason::DeadlineExceeded,
                observed_at_ms: 1_050,
                ..
            }))
        ));
    }

    #[test]
    fn durable_retry_schedule_survives_restart() {
        let journal = TempJournal::new("retry-restart");
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay_ms: 50,
            max_delay_ms: 100,
        };
        {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut first, None);
            let lease = first.claim_effect(1_000, 100).unwrap().unwrap();
            first
                .report_effect_error(&lease, 1_001, transient_error(), &policy)
                .unwrap();
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let persisted_error: Vec<u8> = connection
            .query_row("SELECT last_error FROM vm_dispatches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<EffectError>(&persisted_error).unwrap(),
            transient_error()
        );
        drop(connection);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(restarted.claim_effect(1_050, 100).unwrap().is_none());
        let lease = restarted.claim_effect(1_051, 100).unwrap().unwrap();
        assert_eq!(lease.retry_count, 1);
        assert_eq!(lease.attempt, 2);
        let completed = restarted.acknowledge_effect(&lease, 1_052, runtime_list_result(8));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(8),
                ..
            })
        ));
    }

    #[test]
    fn mutating_semantic_retry_preserves_command_envelope() {
        let journal = TempJournal::new("mutation-semantic-retry");
        let first_lease = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start_refresh(&mut first, Revision(1));
            let lease = first.claim_effect(1_000, 100).unwrap().unwrap();
            first
                .report_effect_error(
                    &lease,
                    1_001,
                    transient_error(),
                    &RetryPolicy {
                        max_retries: 1,
                        base_delay_ms: 50,
                        max_delay_ms: 50,
                    },
                )
                .unwrap();
            lease
        };

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let second_lease = restarted.claim_effect(1_051, 100).unwrap().unwrap();
        assert_eq!(second_lease.request, first_lease.request);
        assert_eq!(second_lease.retry_count, 1);
        let EffectOperation::Command(command) = &second_lease.request.operation else {
            panic!("expected command operation");
        };
        assert_eq!(command.idempotency_key.as_str(), "leselang-effect-1");
        assert_eq!(
            command.command_id,
            CommandId::new("leselang-command-1").unwrap()
        );
    }

    #[test]
    fn retry_policy_and_error_metadata_are_bounded() {
        let mut vm = Vm::default();
        start(&mut vm, None);
        let lease = vm.claim_effect(1_000, 100).unwrap().unwrap();
        let invalid_policy = RetryPolicy {
            max_retries: MAX_SEMANTIC_RETRIES + 1,
            ..RetryPolicy::default()
        };
        assert_eq!(
            vm.report_effect_error(&lease, 1_001, transient_error(), &invalid_policy)
                .unwrap_err()
                .code,
            "LSV2201"
        );
        let invalid_error = EffectError {
            class: EffectErrorClass::Transient,
            code: "bad code".to_string(),
            message: "invalid code shape".to_string(),
        };
        assert_eq!(
            vm.report_effect_error(&lease, 1_001, invalid_error, &RetryPolicy::default())
                .unwrap_err()
                .code,
            "LSV2202"
        );
    }

    #[test]
    fn durable_dispatch_reclaims_expired_lease_and_rejects_stale_worker() {
        let journal = TempJournal::new("dispatch-recovery");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let request = start(&mut first, None);
        let first_lease = first.claim_effect(1_000, 50).unwrap().unwrap();

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(restarted.claim_effect(1_049, 50).unwrap().is_none());
        let expired = first.acknowledge_effect(&first_lease, 1_051, runtime_list_result(40));
        assert!(matches!(expired, Step::Fault(Fault { ref code, .. }) if code == "LSV4023"));

        let second_lease = restarted.claim_effect(1_051, 50).unwrap().unwrap();
        assert_eq!(second_lease.request, request);
        assert_eq!(second_lease.attempt, 2);
        let stale = first.acknowledge_effect(&first_lease, 1_052, runtime_list_result(41));
        assert!(matches!(stale, Step::Fault(Fault { ref code, .. }) if code == "LSV4022"));

        let completed = restarted.acknowledge_effect(&second_lease, 1_052, runtime_list_result(42));
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeList {
                revision: Revision(42),
                ..
            })
        ));
        drop(restarted);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(recovered.claim_effect(2_000, 50).unwrap().is_none());
        assert_eq!(recovered.completed_count(), 1);
    }

    #[test]
    fn durable_dispatch_has_single_claimant_across_live_vms() {
        let journal = TempJournal::new("dispatch-race");
        let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        start(&mut first, None);
        let mut second = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();

        assert!(first.claim_effect(10, 100).unwrap().is_some());
        assert!(second.claim_effect(10, 100).unwrap().is_none());
    }

    #[test]
    fn mutating_effect_redelivery_reuses_domain_idempotency_key() {
        let journal = TempJournal::new("mutation-redelivery");
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(
            RuntimeId::new("runtime-a").unwrap(),
            "Alpha",
            "http://runtime-a",
        );

        let first_lease = {
            let mut first = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            let request = start_refresh(&mut first, Revision(1));
            let EffectOperation::Command(command) = &request.operation else {
                panic!("expected command operation");
            };
            assert_eq!(command.idempotency_key.as_str(), "leselang-effect-1");
            first.claim_effect(1_000, 50).unwrap().unwrap()
        };
        let EffectOperation::Command(first_command) = first_lease.request.operation.clone() else {
            panic!("expected command operation");
        };
        let first_result = control.execute(first_command).unwrap();
        assert_eq!(first_result.runtime.refresh_count, 1);

        let mut restarted = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let second_lease = restarted.claim_effect(1_051, 50).unwrap().unwrap();
        assert_eq!(second_lease.attempt, 2);
        assert_eq!(second_lease.request, first_lease.request);
        let EffectOperation::Command(second_command) = second_lease.request.operation.clone()
        else {
            panic!("expected command operation");
        };
        let replayed_result = control.execute(second_command).unwrap();
        assert_eq!(replayed_result, first_result);
        assert_eq!(replayed_result.runtime.refresh_count, 1);

        let completed = restarted.acknowledge_effect(
            &second_lease,
            1_052,
            EffectResult::Command(Box::new(replayed_result.clone())),
        );
        assert!(matches!(
            completed,
            Step::Done(Value::RuntimeRefresh { ref result }) if **result == replayed_result
        ));
        drop(restarted);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert!(recovered.claim_effect(2_000, 50).unwrap().is_none());
        assert_eq!(recovered.completed_count(), 1);
    }

    #[test]
    fn journal_v1_migrates_without_forging_missing_dispatch_context() {
        let journal = TempJournal::new("v1-migration");
        let request = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None)
        };
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 DROP TABLE vm_dispatches;
                 ALTER TABLE vm_effects RENAME TO vm_effects_v3;
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 INSERT INTO vm_effects(token, state, image, terminal_step)
                   SELECT token, state, image, terminal_step FROM vm_effects_v3;
                 DROP TABLE vm_effects_v3;
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let mut migrated = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(migrated.pending_continuations(), vec![request.continuation]);
        assert!(migrated.claim_effect(10, 100).unwrap().is_none());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
    }

    #[test]
    fn journal_v2_migrates_absolute_deadline_column() {
        let journal = TempJournal::new("v2-migration");
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vm_metadata (
                   singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                   next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                 ) STRICT;
                 INSERT INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_dispatches (
                   token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                   request BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                   attempt INTEGER NOT NULL CHECK (attempt >= 0),
                   lease_expires_at_ms INTEGER,
                   CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                          (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                          (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                 ) STRICT;
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        drop(connection);

        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let deadline_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_effects') WHERE name = 'deadline_at_ms'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deadline_columns, 1);
        let retry_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_dispatches')
                 WHERE name IN ('ready_at_ms', 'retry_count', 'last_error')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retry_columns, 3);
    }

    #[test]
    fn journal_v3_migrates_semantic_retry_state() {
        let journal = TempJournal::new("v3-migration");
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE vm_metadata (
                   singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                   next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                 ) STRICT;
                 INSERT INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                 CREATE TABLE vm_effects (
                   token TEXT PRIMARY KEY,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   image BLOB NOT NULL,
                   deadline_at_ms INTEGER CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0),
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_dispatches (
                   token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                   request BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                   attempt INTEGER NOT NULL CHECK (attempt >= 0),
                   lease_expires_at_ms INTEGER,
                   CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                          (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                          (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                 ) STRICT;
                 CREATE INDEX vm_effect_deadline_idx ON vm_effects(state, deadline_at_ms);
                 PRAGMA user_version = 3;",
            )
            .unwrap();
        drop(connection);

        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let retry_columns: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vm_dispatches')
                 WHERE name IN ('ready_at_ms', 'retry_count', 'last_error')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retry_columns, 3);
    }

    #[test]
    fn journal_v4_migrates_merge_graph_schema_without_losing_effects() {
        let journal = TempJournal::new("v4-migration");
        let request = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None)
        };
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 PRAGMA user_version = 4;",
            )
            .unwrap();
        drop(connection);

        let migrated = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        assert_eq!(migrated.pending_continuations(), vec![request.continuation]);
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            JOURNAL_SCHEMA_VERSION
        );
        let merge_tables: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('vm_merge_groups', 'vm_merge_branches')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(merge_tables, 2);
    }

    #[test]
    fn journal_v4_rejects_preexisting_merge_table_names() {
        let journal = TempJournal::new("v4-merge-table-conflict");
        {
            let _vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE vm_debugger_audit;
                 DROP TABLE vm_merge_branches;
                 DROP TABLE vm_merge_groups;
                 CREATE TABLE vm_merge_groups (untrusted BLOB);
                 PRAGMA user_version = 4;",
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4003");
    }

    #[test]
    fn durable_journal_rejects_merge_graph_order_tampering() {
        let journal = TempJournal::new("merge-order-tamper");
        let (left, right) = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            (start(&mut vm, None), start(&mut vm, None))
        };
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step)
                 VALUES (?1, ?2, 'pending', NULL)",
                rusqlite::params!["merge-1", serde_json::to_vec(&plan).unwrap()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position)
                 VALUES (?1, ?2, 'right', 0), (?1, ?3, 'left', 1)",
                rusqlite::params![
                    "merge-1",
                    left.continuation.token.as_str(),
                    right.continuation.token.as_str()
                ],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_rejects_merge_group_effect_token_collision() {
        let journal = TempJournal::new("merge-token-collision");
        let (left, right) = {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            (start(&mut vm, None), start(&mut vm, None))
        };
        let plan = MergePlan::new(["left", "right"]).unwrap();
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step)
                 VALUES (?1, ?2, 'pending', NULL)",
                rusqlite::params![
                    left.continuation.token.as_str(),
                    serde_json::to_vec(&plan).unwrap()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position)
                 VALUES (?1, ?2, 'left', 0), (?1, ?3, 'right', 1)",
                rusqlite::params![
                    left.continuation.token.as_str(),
                    left.continuation.token.as_str(),
                    right.continuation.token.as_str()
                ],
            )
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4007");
    }

    #[test]
    fn durable_journal_accepts_legacy_query_request_encoding() {
        let journal = TempJournal::new("legacy-request");
        {
            let mut vm = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
            start(&mut vm, None);
        }
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        let request: Vec<u8> = connection
            .query_row("SELECT request FROM vm_dispatches", [], |row| row.get(0))
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&request).unwrap();
        let operation = value.as_object_mut().unwrap().remove("operation").unwrap();
        assert_eq!(operation["kind"], "query");
        value
            .as_object_mut()
            .unwrap()
            .insert("query".to_string(), operation["payload"].clone());
        connection
            .execute(
                "UPDATE vm_dispatches SET request = ?1",
                [serde_json::to_vec(&value).unwrap()],
            )
            .unwrap();
        drop(connection);

        let mut recovered = Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap();
        let lease = recovered.claim_effect(10, 100).unwrap().unwrap();
        let completed = recovered.acknowledge_effect(&lease, 11, runtime_list_result(1));
        assert!(matches!(completed, Step::Done(Value::RuntimeList { .. })));
    }

    #[test]
    fn effect_request_construction_rejects_zero_fuel_without_underflow() {
        let effect = Effect::UiFocus {
            node_id: "budget-test".into(),
        };
        for fuel in 0..=3 {
            let request = Vm::build_effect_request(
                &effect,
                Type::UiFocus,
                1,
                Principal::new("operator").unwrap(),
                CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                None,
                DEFAULT_EFFECT_DEADLINE_MS,
                None,
                fuel,
            );
            if fuel == 0 {
                let error = request.unwrap_err();
                assert_eq!(error.code, "LSV1001");
                assert_eq!(
                    error.message,
                    "execution fuel exhausted before host operation"
                );
            } else {
                let request = request.unwrap();
                assert_eq!(request.continuation.fuel_remaining, fuel - 1);
                assert_eq!(request.budget.fuel_remaining, fuel - 1);
                let wire = serde_json::to_value(&request).unwrap();
                assert_eq!(wire["continuation"]["fuel_remaining"], fuel - 1);
                assert_eq!(wire["budget"]["fuel_remaining"], fuel - 1);
            }
        }
    }

    #[test]
    fn durable_journal_rejects_unknown_schema_versions() {
        let journal = TempJournal::new("version");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        let connection = rusqlite::Connection::open(journal.path()).unwrap();
        connection
            .execute_batch("PRAGMA user_version = 99;")
            .unwrap();
        drop(connection);

        let error = Vm::open_journal(journal.path(), DEFAULT_FUEL)
            .err()
            .unwrap();
        assert_eq!(error.code, "LSV4004");
    }

    #[cfg(unix)]
    #[test]
    fn durable_journal_is_private_and_rejects_symbolic_links() {
        use std::os::unix::fs::{MetadataExt, symlink};

        let journal = TempJournal::new("permissions");
        drop(Vm::open_journal(journal.path(), DEFAULT_FUEL).unwrap());
        assert_eq!(fs::metadata(journal.path()).unwrap().mode() & 0o777, 0o600);

        let link = journal.root.join("journal-link.sqlite3");
        symlink(journal.path(), &link).unwrap();
        let error = Vm::open_journal(&link, DEFAULT_FUEL).err().unwrap();
        assert_eq!(error.code, "LSV4001");
    }
}
