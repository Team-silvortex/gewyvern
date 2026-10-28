use leserpent_vm::Vm;

#[path = "support/reference_dispatch_authority.rs"]
mod reference_dispatch_authority;

#[path = "support/reference_request_build_authority.rs"]
mod reference_request_build_authority;

#[test]
fn memory_request_construction_cannot_reserve_or_dispatch_a_live_effect() {
    reference_request_build_authority::exercise(Vm::new(100), |vm| vm);
}

#[test]
fn memory_dispatch_adapter_proposals_cannot_replace_current_leases_or_commit_retries() {
    reference_dispatch_authority::exercise(Vm::new(100), |vm| vm);
}
