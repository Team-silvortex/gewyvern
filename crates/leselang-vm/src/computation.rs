pub(super) use leselang_command::execution_outcome::ComputationOutcome as Outcome;
pub(super) use leselang_command::reference_computation::{
    evaluate, resume, resume_group, resume_group_outcome, resume_outcome,
};

#[cfg(test)]
#[path = "computation_contract_tests.rs"]
mod contract_tests;
