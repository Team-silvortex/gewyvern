#![cfg(feature = "leserpent-profile")]
#![forbid(unsafe_code)]
//! Test-only historical boundary oracles. These compile original source in one
//! test crate so frozen coordinators retain private kernel access. Production
//! uses leselang-hir-core through the facade; no private testing API is exported.

#[path = "../../leselang-hir-core/src/binding_source.rs"]
pub mod binding_source;
#[path = "../../leselang-hir-core/src/bound_projection_source.rs"]
pub mod bound_projection_source;
#[path = "../../leselang-hir-core/src/call_evaluation.rs"]
pub mod call_evaluation;
#[path = "../../leselang-hir-core/src/call_typing.rs"]
pub mod call_typing;
#[path = "../../leselang-hir-core/src/catalog_atomic_source.rs"]
mod catalog_atomic_source;
#[path = "../../leselang-hir-core/src/catalog_group_source.rs"]
pub mod catalog_group_source;
#[path = "../../leselang-hir-core/src/catalog_program.rs"]
pub mod catalog_program;
#[path = "../../leselang-hir-core/src/catalog_source.rs"]
pub mod catalog_source;
#[path = "../../leselang-hir-core/src/choice_source.rs"]
pub mod choice_source;
#[path = "../../leselang-hir-core/src/computation_profile.rs"]
pub mod computation_profile;
#[path = "../../leselang-hir-core/src/control_source.rs"]
pub mod control_source;
#[path = "../../leselang-hir-core/src/effect_evaluation.rs"]
pub mod effect_evaluation;
#[path = "../../leselang-hir-core/src/effect_reentry.rs"]
pub mod effect_reentry;
#[path = "../../leselang-hir-core/src/effect_session.rs"]
pub mod effect_session;
#[path = "../../leselang-hir-core/src/effect_stack.rs"]
pub mod effect_stack;
#[path = "../../leselang-hir-core/src/effect_stack_session.rs"]
pub mod effect_stack_session;
#[path = "../../leselang-hir-core/src/flow_typing.rs"]
pub mod flow_typing;
#[path = "../../leselang-hir-core/src/fold_source.rs"]
pub mod fold_source;
#[path = "../../leselang-hir-core/src/group_exports.rs"]
pub mod group_exports;
#[path = "../../leselang-hir-core/src/group_reply.rs"]
pub mod group_reply;
#[path = "../../leselang-hir-core/src/group_source.rs"]
pub mod group_source;
#[path = "../../leselang-hir-core/src/helper_bindings.rs"]
pub mod helper_bindings;
#[path = "../../leselang-hir-core/src/helper_body.rs"]
pub mod helper_body;
#[path = "../../leselang-hir-core/src/helper_body_source.rs"]
pub mod helper_body_source;
#[path = "../../leselang-hir-core/src/helper_declarations.rs"]
pub mod helper_declarations;
#[path = "../../leselang-hir-core/src/helper_dependencies.rs"]
pub mod helper_dependencies;
#[path = "../../leselang-hir-core/src/helper_expansion.rs"]
pub mod helper_expansion;
#[path = "../../leselang-hir-core/src/helper_hygiene.rs"]
pub mod helper_hygiene;
#[path = "../../leselang-hir-core/src/helper_instance.rs"]
pub mod helper_instance;
#[path = "../../leselang-hir-core/src/helper_instance_finish.rs"]
pub mod helper_instance_finish;
#[path = "../../leselang-hir-core/src/helper_join.rs"]
pub mod helper_join;
#[path = "../../leselang-hir-core/src/helper_registry.rs"]
pub mod helper_registry;
#[path = "../../leselang-hir-core/src/helper_returns.rs"]
pub mod helper_returns;
#[path = "../../leselang-hir-core/src/helper_source.rs"]
pub mod helper_source;
#[path = "../../leselang-hir-core/src/helper_templates.rs"]
pub mod helper_templates;
#[path = "../../leselang-hir-core/src/host_source.rs"]
pub mod host_source;
#[path = "../../leselang-hir-core/src/ir.rs"]
pub mod ir;
#[path = "../../leselang-hir-core/src/local_effect_session.rs"]
pub mod local_effect_session;
#[path = "../../leselang-hir-core/src/local_scope.rs"]
mod local_scope;
#[path = "../../leselang-hir-core/src/loop_source.rs"]
pub mod loop_source;
#[path = "../../leselang-hir-core/src/native_graph.rs"]
pub mod native_graph;
#[path = "../../leselang-hir-core/src/native_group.rs"]
pub mod native_group;
#[path = "../../leselang-hir-core/src/native_group_exports.rs"]
pub mod native_group_exports;
#[path = "../../leselang-hir-core/src/native_group_lookup.rs"]
pub mod native_group_lookup;
#[path = "../../leselang-hir-core/src/native_group_preparation.rs"]
pub mod native_group_preparation;
#[path = "../../leselang-hir-core/src/native_group_signature.rs"]
pub mod native_group_signature;
#[path = "../../leselang-hir-core/src/parallel_evaluation.rs"]
pub mod parallel_evaluation;
#[path = "../../leselang-hir-core/src/prepared_call_evaluation.rs"]
pub mod prepared_call_evaluation;
#[path = "../../leselang-hir-core/src/prepared_parallel.rs"]
pub mod prepared_parallel;
#[path = "../../leselang-hir-core/src/prepared_sequence.rs"]
pub mod prepared_sequence;
#[path = "../../leselang-hir-core/src/prepared_typing.rs"]
pub mod prepared_typing;
#[path = "../../leselang-hir-core/src/program_source.rs"]
pub mod program_source;
#[path = "../../leselang-hir-core/src/projection_access.rs"]
pub mod projection_access;
#[path = "../../leselang-hir-core/src/projection_source.rs"]
pub mod projection_source;
#[path = "../../leselang-hir-core/src/pure_evaluation.rs"]
pub mod pure_evaluation;
#[path = "../../leselang-hir-core/src/pure_typing.rs"]
pub mod pure_typing;
#[path = "../../leselang-hir-core/src/repeat_sequence_source.rs"]
pub mod repeat_sequence_source;
#[path = "../../leselang-hir-core/src/repeat_source.rs"]
pub mod repeat_source;
#[path = "../../leselang-hir-core/src/scalar_form.rs"]
pub mod scalar_form;
#[path = "../../leselang-hir-core/src/scalar_source.rs"]
pub mod scalar_source;
#[path = "../../leselang-hir-core/src/sequence_evaluation.rs"]
pub mod sequence_evaluation;
#[path = "../../leselang-hir-core/src/sequence_source.rs"]
pub mod sequence_source;
#[path = "../../leselang-hir-core/src/source_call.rs"]
pub mod source_call;
#[path = "../../leselang-hir-core/src/source_cost.rs"]
pub mod source_cost;

#[path = "support/migration_computation.rs"]
pub mod computation;
#[path = "../../leserpent-hir/src/computed_group.rs"]
mod computed_group;
#[path = "../../leserpent-hir/src/control_flow.rs"]
mod control_flow;
#[path = "../../leserpent-hir/src/function_flow.rs"]
mod function_flow;
#[path = "../../leserpent-hir/src/functions.rs"]
mod functions;
#[path = "../../leserpent-hir/src/host_call.rs"]
pub mod host_call;
#[path = "../../leserpent-hir/src/pure_reference.rs"]
mod pure_reference;
#[path = "../../leserpent-hir/src/result_field.rs"]
pub mod result_field;

include!("../../leserpent-hir/src/leserpent_profile.rs");
