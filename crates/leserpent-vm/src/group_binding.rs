pub use leselang_command::GroupResultBinding;
use leselang_runtime_core::Fuel;
#[cfg(test)]
use leserpent_hir::Type;
use leserpent_hir::host_call::HostOperation;
use leserpent_hir::{Effect, HirBranch};

#[cfg(test)]
#[path = "group_protocol_tests.rs"]
mod protocol_tests;

#[cfg(test)]
#[path = "group_successor_contract_tests.rs"]
mod successor_contract_tests;

#[cfg(test)]
#[path = "group_execution_contract_tests.rs"]
mod execution_contract_tests;

#[cfg(test)]
#[path = "group_plan_contract_tests.rs"]
pub(crate) mod plan_contract_tests;

#[cfg(test)]
use crate::MergePlan;
#[cfg(test)]
use crate::result_binding::ReferenceResultBinding;
use crate::{ContinuationToken, EffectRequest, Fault, ResultBinding, Step, Value};

pub(super) use leselang_command::reference_group_execution::CaptureProgress;

pub(super) use leselang_command::reference_merge::invalid_group_binding as invalid;
pub(super) use leselang_command::reference_merge::{
    group_binding_admission_fault as admission_fault, validate_plan_encoding,
};

pub(super) trait ReferenceGroupResultBinding {
    #[cfg(test)]
    fn branches(&self) -> Result<&[HirBranch], Fault>;
    #[cfg(test)]
    fn successor_name_at(&self, position: usize) -> Result<String, Fault>;
    #[cfg(test)]
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
    #[cfg(any(test, feature = "sqlite-journal"))]
    fn validate_saved_capture(
        &self,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault>;
}

impl ReferenceGroupResultBinding for GroupResultBinding {
    #[cfg(test)]
    fn branches(&self) -> Result<&[HirBranch], Fault> {
        self.plan_branches().map_err(|_| invalid())
    }

    #[cfg(test)]
    fn successor_name_at(&self, position: usize) -> Result<String, Fault> {
        self.plan_successor_name_at(position, crate::MAX_MERGE_BRANCHES)
            .map_err(|_| invalid())
    }

    #[cfg(test)]
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
        leselang_command::reference_group_binding::validate_member(self, name, value)
    }

    fn prepare_initial(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        leselang_command::reference_group_execution::prepare_initial(
            self,
            owner,
            first,
            value,
            &mut GroupExecutionHost,
        )
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
        leselang_command::reference_group_successor::build_group_successor(
            self,
            effect,
            binding,
            leselang_command::reference_group_successor::GroupSuccessorContext {
                owner,
                first,
                fuel_remaining: fuel,
                position,
            },
            |input| {
                crate::Vm::build_effect_request(
                    input.effect,
                    input.result_type,
                    input.sequence,
                    input.principal,
                    input.capabilities,
                    input.expected_revision,
                    input.deadline_ms,
                    input.deadline_at_ms,
                    input.fuel_remaining,
                )
            },
        )
    }

    fn advance_capture(
        &self,
        owner: &ContinuationToken,
        requests: &[EffectRequest],
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        leselang_command::reference_group_execution::advance_capture(
            self,
            owner,
            requests,
            value,
            &mut GroupExecutionHost,
        )
    }

    fn finish(&self, value: &Value) -> Step {
        leselang_command::reference_group_execution::finish(self, value, &mut GroupExecutionHost)
    }

    #[cfg(any(test, feature = "sqlite-journal"))]
    fn validate_saved_capture(
        &self,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault> {
        leselang_command::reference_group_binding::validate_saved_capture(
            self, request, fields, operations,
        )
    }
}

struct GroupExecutionHost;

impl leselang_command::reference_group_execution::ReferenceGroupExecutionHost
    for GroupExecutionHost
{
    fn resume_group<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        branches: &'a [HirBranch],
        fuel: &mut Fuel,
    ) -> Result<crate::computation::Outcome<'a>, Fault> {
        crate::computation::resume_group_outcome(binding, value, branches, fuel)
    }

    fn resume_capture<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        operation: HostOperation,
        fuel: &mut Fuel,
    ) -> Result<crate::computation::Outcome<'a>, Fault> {
        crate::computation::resume_outcome(binding, value, operation, fuel)
    }

    fn finish_group(
        &mut self,
        binding: &ResultBinding,
        value: &Value,
        branches: &[HirBranch],
        fuel: &mut Fuel,
    ) -> Result<crate::ScalarValue, Fault> {
        crate::computation::resume_group(binding, value, branches, fuel)
    }

    fn build_successor(
        &mut self,
        group: &GroupResultBinding,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        context: leselang_command::reference_group_successor::GroupSuccessorContext<'_>,
    ) -> Result<EffectRequest, Fault> {
        group.build_successor(
            context.owner,
            context.first,
            effect,
            binding,
            context.fuel_remaining,
            context.position,
        )
    }

    fn validate_successor(
        &mut self,
        parent: &EffectRequest,
        result: &Step,
        next: &EffectRequest,
    ) -> Result<(), Fault> {
        crate::successor::validate_pair(parent, result, next)
    }
}

#[cfg(all(test, feature = "sqlite-journal"))]
mod correspondence_tests {
    use super::*;
    use crate::result_binding::{ReferenceProjectedGroup, group_capture_limits, projection_fault};

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
        let program = leserpent_hir::lower(&leselang_syntax::parse(
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

#[cfg(test)]
pub(super) fn value_type(value: &Value) -> Type {
    value.result_type()
}
