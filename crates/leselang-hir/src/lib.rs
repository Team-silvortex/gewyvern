#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "leserpent-profile"),
    doc = r#"
The native profile does not export product operations or legacy entry points:

```compile_fail
use leselang_hir::{Effect, lower};
```

```compile_fail
use leselang_hir::host_call::HostOperation;
```

```compile_fail
use leselang_hir::computation::Computation;
```

```compile_fail
use leselang_hir::result_field::ResultField;
```
"#
)]

//! Host-defined compilation, typed control IR and bounded in-memory execution.
//!
//! Default features preserve the Leserpent reference profile and its legacy APIs.
//! Independent hosts use `default-features = false` to omit that profile and its
//! product operation contract. Native adapter work and authority remain explicit.

pub mod binding_source;
pub mod bound_projection_source;
pub mod call_evaluation;
pub mod call_typing;
mod catalog_atomic_source;
pub mod catalog_group_source;
pub mod catalog_program;
pub mod catalog_source;
pub mod choice_source;
pub mod computation_profile;
pub mod control_source;
pub mod effect_evaluation;
pub mod effect_reentry;
pub mod effect_session;
pub mod effect_stack;
pub mod effect_stack_session;
pub mod flow_typing;
pub mod group_exports;
pub mod group_reply;
pub mod group_source;
pub mod helper_bindings;
pub mod helper_body;
pub mod helper_body_source;
pub mod helper_declarations;
pub mod helper_dependencies;
pub mod helper_expansion;
pub mod helper_hygiene;
pub mod helper_instance;
pub mod helper_instance_finish;
pub mod helper_join;
pub mod helper_registry;
pub mod helper_returns;
pub mod helper_source;
pub mod helper_templates;
pub mod host_source;
pub mod ir;
pub mod local_effect_session;
mod local_scope;
pub mod native_graph;
pub mod native_group;
pub mod native_group_exports;
pub mod native_group_lookup;
pub mod native_group_preparation;
pub mod native_group_signature;
pub mod parallel_evaluation;
pub mod prepared_call_evaluation;
pub mod prepared_parallel;
pub mod prepared_sequence;
pub mod prepared_typing;
pub mod program_source;
pub mod projection_access;
pub mod projection_source;
pub mod pure_evaluation;
pub mod pure_typing;
pub mod repeat_sequence_source;
pub mod repeat_source;
pub mod scalar_source;
pub mod sequence_evaluation;
pub mod sequence_source;
pub mod source_call;
pub mod source_cost;

// Root inclusion avoids remapping the released reference API or private adapters.
#[cfg(feature = "leserpent-profile")]
include!("leserpent_profile.rs");

#[cfg(test)]
mod argument_binding_contract_tests;

#[cfg(test)]
mod source_preparation_contract_tests;
