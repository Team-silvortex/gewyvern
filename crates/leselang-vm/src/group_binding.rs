pub use leselang_command::GroupResultBinding;
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, HirBranch, Type};
use leselang_runtime_core::{CaptureCorrespondenceLimits, Fuel};

#[cfg(test)]
#[path = "group_protocol_tests.rs"]
mod protocol_tests;

#[cfg(test)]
#[path = "group_plan_contract_tests.rs"]
pub(crate) mod plan_contract_tests;

use crate::result_binding::{ReferenceResultBinding, group_capture_limits};
use crate::{ContinuationToken, EffectRequest, Fault, MergePlan, ResultBinding, Step, Value};

pub(super) enum CaptureProgress {
    Done(crate::ScalarValue),
    Pending(Box<EffectRequest>),
}

pub(super) use leselang_command::reference_merge::invalid_group_binding as invalid;
pub(super) use leselang_command::reference_merge::{
    group_binding_admission_fault as admission_fault, validate_plan_encoding,
};

pub(super) trait ReferenceGroupResultBinding {
    fn branches(&self) -> Result<&[HirBranch], Fault>;
    fn successor_name_at(&self, position: usize) -> Result<String, Fault>;
    fn has_successor(&self, plan: &MergePlan) -> bool;
    #[cfg(any(test, feature = "sqlite-journal"))]
    fn validate(&self, plan: &MergePlan) -> Result<Type, Fault>;
    #[cfg(test)]
    fn validate_requests(
        &self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault>;
    fn validate_member(&self, name: &str, value: &Value) -> Result<(), Fault>;
    fn prepare_initial(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        value: &Value,
    ) -> Result<CaptureProgress, Fault>;
    fn build_successor(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
        position: usize,
    ) -> Result<EffectRequest, Fault>;
    fn advance_capture(
        &self,
        owner: &ContinuationToken,
        requests: &[EffectRequest],
        value: &Value,
    ) -> Result<CaptureProgress, Fault>;
    fn finish(&self, value: &Value) -> Step;
    fn validate_saved_capture(
        &self,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault>;
}

impl ReferenceGroupResultBinding for GroupResultBinding {
    fn branches(&self) -> Result<&[HirBranch], Fault> {
        self.plan_branches().map_err(|_| invalid())
    }

    fn successor_name_at(&self, position: usize) -> Result<String, Fault> {
        self.plan_successor_name_at(position, crate::MAX_MERGE_BRANCHES)
            .map_err(|_| invalid())
    }

    fn has_successor(&self, plan: &MergePlan) -> bool {
        self.successor_sequence.is_some()
            && self
                .branches()
                .is_ok_and(|branches| plan.branches.len() > branches.len())
    }

    #[cfg(any(test, feature = "sqlite-journal"))]
    fn validate(&self, plan: &MergePlan) -> Result<Type, Fault> {
        leselang_command::reference_merge::validate_group_binding(self, plan)
    }

    #[cfg(test)]
    fn validate_requests(
        &self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        let final_type = self.validate(plan)?;
        leselang_command::validate_preflighted_group_requests(
            self,
            leselang_command::GroupRequestContext {
                owner,
                plan,
                result_type: final_type,
                max_successor_names: crate::MAX_MERGE_BRANCHES,
                binding_limits: crate::result_binding::result_binding_limits(),
            },
            requests,
            |request| request.as_view(),
            ContinuationToken::as_str,
            crate::canonical_continuation_sequence,
            crate::valid_merge_branch_name,
        )
        .map_err(|_| invalid())
    }

    fn validate_member(&self, name: &str, value: &Value) -> Result<(), Fault> {
        if let Some(index) = (0..self.reserved_sequences().count()).find(|index| {
            self.successor_name_at(*index)
                .is_ok_and(|candidate| candidate == name)
        }) {
            let accepted = if self.captures_successor() {
                self.preflighted_capture_accepts_type_at(index, value_type(value))
            } else {
                value_type(value) == self.binding.validate(&self.pending)?
            };
            return if accepted { Ok(()) } else { Err(invalid()) };
        }
        let branch = self
            .branches()?
            .iter()
            .find(|branch| branch.name == name)
            .ok_or_else(invalid)?;
        if value_type(value) != branch.result_type {
            return Err(invalid());
        }
        Ok(())
    }

    fn prepare_initial(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        let mut fuel = Fuel::new(self.fuel_remaining);
        let (effect, binding) = match crate::computation::resume_group_outcome(
            &self.binding,
            value,
            self.branches()?,
            &mut fuel,
        )? {
            crate::computation::Outcome::Scalar(value) if self.is_conditional() => {
                return Ok(CaptureProgress::Done(value));
            }
            crate::computation::Outcome::Host(effect) if !self.captures_successor() => {
                (effect, None)
            }
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.captures_successor() =>
            {
                (effect, Some(binding))
            }
            _ => return Err(invalid()),
        };
        if fuel.remaining() == 0 {
            return Err(Fault {
                code: "LSV1001".into(),
                message: "execution fuel exhausted before group successor".into(),
            });
        }
        self.build_successor(owner, first, &effect, binding, fuel.remaining(), 0)
            .map(|request| CaptureProgress::Pending(Box::new(request)))
    }

    fn build_successor(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
        position: usize,
    ) -> Result<EffectRequest, Fault> {
        let operation = HostOperation::for_effect(effect).ok_or_else(invalid)?;
        let (principal, capabilities) = crate::successor::authority(&first.operation);
        let image = &first.continuation;
        let mut request = crate::Vm::build_effect_request(
            effect,
            operation.result_type(),
            self.reserved_sequences()
                .nth(position)
                .ok_or_else(invalid)?,
            principal.clone(),
            capabilities.clone(),
            image.expected_revision,
            image.deadline_ms,
            image.deadline_at_ms,
            fuel,
        )?;
        request.continuation.schema_version = self.continuation_schema();
        request.continuation.group_result = Some(owner.clone());
        request.continuation.result_binding = binding;
        request.continuation.max_output_items = image.max_output_items;
        request.budget.max_output_items = image.max_output_items;
        crate::validate_effect_request(&request)?;
        crate::validate_continuation_size(&request.continuation)?;
        Ok(request)
    }

    fn advance_capture(
        &self,
        owner: &ContinuationToken,
        requests: &[EffectRequest],
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        let Value::Structured { fields } = value else {
            return Err(invalid());
        };
        let (tail, prefix) = fields.split_last().ok_or_else(invalid)?;
        let request = requests.last().ok_or_else(invalid)?;
        let operations = requests
            .iter()
            .take(prefix.len())
            .map(|request| {
                HostOperation::for_effect(&request.continuation.pending_effect).ok_or_else(invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_saved_capture(request, prefix, &operations)?;
        let image = &request.continuation;
        let saved = image.result_binding.as_ref().ok_or_else(invalid)?;
        let operation = HostOperation::for_effect(&image.pending_effect).ok_or_else(invalid)?;
        let mut fuel = Fuel::new(image.fuel_remaining);
        match crate::computation::resume_outcome(saved, &tail.value, operation, &mut fuel)? {
            crate::computation::Outcome::Scalar(value) => Ok(CaptureProgress::Done(value)),
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.is_dataflow() || self.is_conditional() =>
            {
                if fuel.remaining() == 0 {
                    return Err(Fault {
                        code: "LSV1001".into(),
                        message: "execution fuel exhausted before group successor".into(),
                    });
                }
                let next = self.build_successor(
                    owner,
                    requests.first().ok_or_else(invalid)?,
                    &effect,
                    Some(binding),
                    fuel.remaining(),
                    requests.len() - self.branches()?.len(),
                )?;
                crate::successor::validate_pair(
                    request,
                    &Step::Done((*tail.value).clone()),
                    &next,
                )?;
                Ok(CaptureProgress::Pending(Box::new(next)))
            }
            _ => Err(invalid()),
        }
    }

    fn finish(&self, value: &Value) -> Step {
        let mut fuel = Fuel::new(self.fuel_remaining);
        match self.branches().and_then(|branches| {
            crate::computation::resume_group(&self.binding, value, branches, &mut fuel)
        }) {
            Ok(value) => Step::Done(Value::Scalar { value }),
            Err(error) => Step::Fault(error),
        }
    }

    fn validate_saved_capture(
        &self,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault> {
        let branches = self.branches()?;
        if fields.len() < branches.len() || fields.len() != operations.len() {
            return Err(invalid());
        }
        let binding = request
            .continuation
            .result_binding
            .as_ref()
            .ok_or_else(invalid)?;
        leselang_command::validate_group_capture_correspondence(
            binding,
            branches,
            fields,
            operations,
            CaptureCorrespondenceLimits {
                max_saved_bindings: leselang_hir::MAX_EFFECT_NESTING_DEPTH,
                max_original_rows: crate::MAX_MERGE_BRANCHES,
            },
            group_capture_limits(),
            crate::valid_merge_branch_name,
        )
        .map_err(|error| match error {
            leselang_command::CaptureReplayError::InvalidProjection => {
                crate::result_binding::invalid()
            }
            leselang_command::CaptureReplayError::Mismatch => invalid(),
        })
    }
}

#[cfg(all(test, feature = "sqlite-journal"))]
mod correspondence_tests {
    use super::*;
    use crate::result_binding::{ReferenceProjectedGroup, projection_fault};

    struct TempRoot(std::path::PathBuf);
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn saved_capture_correspondence_preserves_frozen_native_fault_order() {
        fn frozen(
            binding: &GroupResultBinding,
            request: &EffectRequest,
            fields: &[crate::StructuredField],
            operations: &[HostOperation],
        ) -> Result<(), Fault> {
            let branches = binding.branches()?;
            if fields.len() < branches.len() || fields.len() != operations.len() {
                return Err(invalid());
            }
            let saved = request
                .continuation
                .result_binding
                .as_ref()
                .ok_or_else(invalid)?;
            for group in &saved.groups {
                if !group
                    .group
                    .matches_members(
                        branches,
                        &fields[..branches.len()],
                        group_capture_limits(),
                        crate::valid_merge_branch_name,
                    )
                    .map_err(projection_fault)?
                {
                    return Err(invalid());
                }
            }
            for result in &saved.results {
                let mut matched = false;
                for (operation, field) in operations.iter().zip(fields) {
                    matched |= result
                        .result
                        .matches_capture(*operation, &field.value)
                        .map_err(projection_fault)?;
                }
                if !matched {
                    return Err(invalid());
                }
            }
            Ok(())
        }
        let directory = std::env::temp_dir().join(format!(
            "leselang-capture-correspondence-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let root = TempRoot(directory);
        let path = root.0.join("capture.sqlite3");
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")),
                body: bind(alias: g, body: bind(state: member(value: alias, name: "a"),
                    body: bind(r: ui.focus(node_id: "a"), body:
                        eq(left: field(value: state, name: "node_id"), right: field(value: r, name: "node_id"))))))"#,
        )).unwrap();
        let mut vm = crate::Vm::open_journal(&path, 500).unwrap();
        let Step::Effect(first) = vm.start(
            &program,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
        ) else {
            panic!()
        };
        let focus = |node: &str| {
            crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                node_id: node.into(),
            })
        };
        let Step::Effect(second) = vm.resume(&first.continuation, focus("a")) else {
            panic!()
        };
        let Step::Effect(tail) = vm.resume(&second.continuation, focus("b")) else {
            panic!()
        };
        drop(vm);
        let connection = rusqlite::Connection::open(&path).unwrap();
        let bytes: Vec<u8> = connection
            .query_row("SELECT plan FROM vm_merge_groups", [], |row| row.get(0))
            .unwrap();
        let plan: MergePlan = serde_json::from_slice(&bytes).unwrap();
        let binding = plan.result_binding.unwrap();
        let fields = [
            crate::StructuredField {
                name: "a".into(),
                value: Box::new(Value::UiFocus {
                    node_id: "a".into(),
                }),
            },
            crate::StructuredField {
                name: "b".into(),
                value: Box::new(Value::UiFocus {
                    node_id: "b".into(),
                }),
            },
        ];
        let operations = [HostOperation::UiFocus, HostOperation::UiFocus];
        assert_eq!(
            tail.continuation
                .result_binding
                .as_ref()
                .unwrap()
                .groups
                .len(),
            2
        );
        assert_eq!(
            tail.continuation
                .result_binding
                .as_ref()
                .unwrap()
                .results
                .len(),
            1
        );
        for case in 0..10 {
            let mut request = (*tail).clone();
            let mut rows = fields.to_vec();
            let mut ops = operations.to_vec();
            match case {
                0 => {}
                1 => {
                    rows.pop();
                    ops.pop();
                }
                2 => {
                    ops.pop();
                }
                3 => request.continuation.result_binding = None,
                4 => {
                    request.continuation.result_binding.as_mut().unwrap().groups[1]
                        .group
                        .members[0]
                        .name = "changed".into()
                }
                5 => {
                    request
                        .continuation
                        .result_binding
                        .as_mut()
                        .unwrap()
                        .results[0]
                        .result
                        .fields[0]
                        .value = crate::ScalarValue::String("unmatched".into())
                }
                6 => rows[1].name = "changed".into(),
                7 => ops[1] = HostOperation::UiNavigateFocus,
                8 => {
                    rows.push(crate::StructuredField {
                        name: "later".into(),
                        value: Box::new(Value::Scalar {
                            value: crate::ScalarValue::Boolean(true),
                        }),
                    });
                    ops.push(HostOperation::UiFocus);
                }
                9 => {
                    request
                        .continuation
                        .result_binding
                        .as_mut()
                        .unwrap()
                        .results[0]
                        .result
                        .projection_version = 99
                }
                _ => unreachable!(),
            }
            let expected = frozen(&binding, &request, &rows, &ops);
            assert_eq!(
                binding.validate_saved_capture(&request, &rows, &ops),
                expected,
                "case {case}"
            );
            assert_eq!(expected.is_ok(), matches!(case, 0 | 7));
            if case == 8 {
                assert_eq!(expected.unwrap_err().code, "LSV1405");
            }
        }
        drop(connection);
    }
}

pub(super) fn value_type(value: &Value) -> Type {
    value.result_type()
}
