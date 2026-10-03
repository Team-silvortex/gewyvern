use std::fs;
use std::path::{Path, PathBuf};

const MODULES: &[&str] = &[
    "runtime.md",
    "gewylang.md",
    "leselang.md",
    "protocols.md",
    "operations.md",
    "project.md",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn leselang_boolean_projection_contract_keeps_acknowledgements_and_upgrade_fences_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Boolean GUI Projections")
        .nth(1)
        .unwrap()
        .split("## Text GUI Projections")
        .next()
        .unwrap();
    for invariant in [
        "successful correlated acknowledgement",
        "not an arbitrary UI read",
        "projection_version: 2",
        "unversioned v1 wire shape",
        "never synthesizes absent booleans",
        "both cold conditional paths",
        "committed raw result",
        "LSV1405",
        "LSV3003",
        "journal schema 10 remain unchanged",
        "shared fuel",
    ] {
        assert!(
            section.contains(invariant),
            "missing boolean projection boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(program.function.result_type, leselang_hir::Type::UiFocus);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_text_projection_contract_matches_typed_hir_and_legacy_budget_fences() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Text GUI Projections")
        .nth(1)
        .unwrap()
        .split("## Kind GUI Projections")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "successful correlated acknowledgement",
        "acknowledged expectation",
        "submitted value",
        "live-property query",
        "not exported",
        "256 bytes",
        "128 bytes",
        "1024 bytes",
        "4096 bytes",
        "LSV1404",
        "projection_version: 3",
        "canonical order",
        "v1 and explicit v2 frames remain byte-exact",
        "never synthesizes missing text fields",
        "Cold branches",
        "LSV1405",
        "committed raw receipt",
        "shared fuel",
        "64 KiB",
        "LSV3002",
        "one transaction",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "secret-handling",
        "no public private frames",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing text-projection contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_kind_projection_contract_keeps_token_domains_legacy_and_authority_fences_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Kind GUI Projections")
        .nth(1)
        .unwrap()
        .split("## Optional GUI Projections")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "canonical enum token string",
        "successful correlated acknowledgement",
        "not a live-property query",
        "no case folding",
        "ordinal conversion",
        "own operation domain",
        "LSV1404",
        "projection_version: 4",
        "Legacy v1/v2/v3 frames stay byte-exact",
        "never synthesizes missing kind fields",
        "cold branches and conditional aliases",
        "high-bit",
        "LSV1405",
        "committed raw receipt",
        "shared fuel",
        "64 KiB",
        "absolute deadline",
        "current-effect cancellation",
        "one transaction",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "without exposing private frames",
        "nullable projections",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing kind-projection contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(program.function.result_type, leselang_hir::Type::UiFocus);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_optional_projection_contract_distinguishes_missing_empty_and_cold_defaults() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Optional GUI Projections")
        .nth(1)
        .unwrap()
        .split("## Named Group Result Bindings")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "distinct scalar type",
        "Absent and empty are different",
        "evaluates the fallback only when absent",
        "no implicit lifting or unwrapping",
        "acknowledged nullable expectation",
        "not a live-property query",
        "optional-text host arguments",
        "projection_version: 5",
        "v1-v4 frames remain byte-exact",
        "Missing payload is rejected",
        "committed raw receipt",
        "1024 bytes",
        "4096 bytes",
        "256 bytes",
        "LSV1404",
        "LSV1405",
        "shared fuel",
        "64 KiB",
        "one transaction",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "without exposing private frames",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing optional-projection contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::UiSetFormValue
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_pure_function_contract_keeps_types_hygiene_and_durable_bounds_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Reusable Pure Functions")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "32 declarations",
        "8 parameters",
        "parameter declaration order",
        "hygienic expansion",
        "before cloning",
        "1024-node",
        "16-level",
        "shared fuel",
        "unused helpers",
        "recursion",
        "unchanged",
        "restart needs no source",
        "LSH1504",
    ] {
        assert!(
            section.contains(invariant),
            "missing helper boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::UiSetFormValue
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    let reference =
        fs::read_to_string(repository_root().join("docs/leselang-language.md")).unwrap();
    assert!(reference.contains("helper-call"));
    assert!(reference.contains("parameter     ="));
    assert!(!reference.contains("excludes group-result\nbindings"));
}

#[test]
fn leselang_effectful_function_contract_preserves_existing_flow_and_wire_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Reusable Effectful Functions")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "Every normal return",
        "pure typed data",
        "parameter declaration order",
        "hygienically renamed",
        "cold returns",
        "before\ncloning",
        "1024 nodes",
        "16 levels",
        "256 KiB canonical source",
        "SQL failures roll back",
        "No hidden call stack",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "v1-v5",
        "wrong-node rejection",
        "effectful loops",
        "host-error recovery/cleanup",
    ] {
        assert!(
            section.contains(invariant),
            "missing effectful helper boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap(),
        program
    );
    for path in ["docs/leselang-language.md", "docs/leselang-embedding.md"] {
        let reference = fs::read_to_string(repository_root().join(path)).unwrap();
        assert!(reference.contains("normal-return splicing"));
        assert!(
            reference.contains("no hidden call stack")
                || reference.contains("not a hidden call stack")
        );
        assert!(!reference.contains("excludes effectful helper functions"));
    }
}

#[test]
fn leselang_prepared_group_members_keep_precomputation_and_dispatch_boundaries_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Prepared Atomic Members")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "Every path produces exactly one atomic operation",
        "same `HostOperation` signature",
        "parameter declaration order",
        "before cloning",
        "bounded iterative traversal",
        "A later preparation failure admits no member",
        "resolved atomic requests",
        "all-success barrier",
        "not reused",
        "parallel debugger starts still fail preflight",
        "1024-node",
        "16-level",
        "64-effect",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "result-dependent members",
    ] {
        assert!(
            section.contains(invariant),
            "missing prepared member boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(program.function.result_type, leselang_hir::Type::Structured);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap(),
        program
    );
    for path in ["docs/leselang-language.md", "docs/leselang-embedding.md"] {
        let reference = fs::read_to_string(repository_root().join(path)).unwrap();
        assert!(reference.contains("Prepared atomic members"));
        assert!(reference.contains("HostOperation"));
    }
}

#[test]
fn leselang_selected_group_contract_keeps_closed_exports_and_one_time_selection() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Selected Named Groups")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "same group mode",
        "same ordered\nmember names",
        "same `HostOperation` signatures",
        "closed signature, not a union",
        "selected group",
        "Restart does not reselect",
        "resolved requests only",
        "all-success barrier",
        "SQL failures roll back",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "Native parallel starts still fail preflight before session journals",
        "effectful loops",
        "not dynamic topology",
    ] {
        assert!(
            section.contains(invariant),
            "missing selected group boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::String)
    );
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap(),
        program
    );
    for path in ["docs/leselang-language.md", "docs/leselang-embedding.md"] {
        let reference = fs::read_to_string(repository_root().join(path)).unwrap();
        assert!(reference.contains("Selected named groups"));
        assert!(reference.contains("restart does not reselect"));
    }
}

#[test]
fn leselang_prepared_result_binding_contract_keeps_capture_and_recovery_fences() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Prepared Atomic Result Bindings")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "Every cold\npath produces exactly one atomic operation",
        "same `HostOperation`\nsignature",
        "do not leak into the continuation",
        "One capture reserves one atomic slot",
        "selected request is resolved before suspension",
        "uncommitted",
        "legacy fields are not synthesized",
        "Simultaneous workers commit one successor",
        "SQL failures",
        "all-success barrier",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "No effectful loops or host-error cleanup",
    ] {
        assert!(
            section.contains(invariant),
            "missing prepared binding boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap(),
        program
    );
    for path in ["docs/leselang-language.md", "docs/leselang-embedding.md"] {
        let reference = fs::read_to_string(repository_root().join(path)).unwrap();
        assert!(reference.contains("Prepared atomic result bindings"));
        assert!(reference.contains("HostOperation"));
    }
}

#[test]
fn leselang_selected_function_contract_keeps_typed_joins_and_recovery_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Selected Data-Returning Functions")
        .nth(1)
        .unwrap()
        .split("## Explicit Scalar Conversions")
        .next()
        .unwrap();
    for invariant in [
        "only returned typed data",
        "`choose` or direct helper\ncalls",
        "pure data fallbacks",
        "Only the selected call's arguments run",
        "every cold return before cloning",
        "no hidden call stack",
        "uncommitted",
        "Simultaneous workers commit one successor",
        "all-success barrier",
        "Continuation schemas 1-11 and journal schema 10 remain unchanged",
        "parallel helper still fails preflight",
    ] {
        assert!(
            section.contains(invariant),
            "missing selected function boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap(),
        program
    );
    for path in ["docs/leselang-language.md", "docs/leselang-embedding.md"] {
        assert!(
            fs::read_to_string(repository_root().join(path))
                .unwrap()
                .contains("Selected data-returning functions")
        );
    }
}

#[test]
fn leselang_reference_separates_current_contract_from_roadmap_design() {
    let root = repository_root();
    let reference = fs::read_to_string(root.join("docs/leselang-language.md"))
        .expect("Leselang language reference must exist");
    let module = fs::read_to_string(root.join("docs/modules/leselang.md"))
        .expect("Leselang documentation module must exist");
    let roadmap = fs::read_to_string(root.join("docs/leserpent-2-roadmap.md"))
        .expect("Leserpent 2.0 roadmap must exist");

    for invariant in [
        "fn main() = runtime.list(",
        "runtime.read",
        "protocolized GUI and control automation",
        "independent embeddable control language",
        "Leserpent is the first reference host",
        "Zero product dependencies do not yet mean host-neutral",
        "hostable Rust crate",
        "narrow FFI boundary",
        "no GUI framework is automatically compatible",
        "developer-owned adapter",
        "generated framework binding",
        "UiAdapterManifest",
        "bind-call",
        "loop-call",
        "choose-call",
        "recover-call",
        "field-call",
        "result bindings",
        "bounded result chains",
        "computed groups",
        "128 UTF-8",
        "durable local",
        "Effect",
        "64 KiB",
        "durable continuation guarantee",
        "SQLite effect journal",
        "LSE",
        "LSH",
        "LSV",
    ] {
        assert!(
            reference.contains(invariant),
            "Leselang reference must preserve current invariant: {invariant}"
        );
    }

    assert!(reference.contains("do not expose\n`async`/`await`"));
    assert!(module.contains("(../leselang-language.md)"));
    assert!(roadmap.contains("(leselang-language.md)"));
}

#[test]
fn leselang_successor_documentation_preserves_atomicity_and_authority_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    for invariant in [
        "## Result-Driven Successor",
        "schema-3 representation",
        "schema 3",
        "Journal schema 8",
        "one\ntransaction",
        "original authority envelope",
        "Vm::restore_request",
        "without reevaluating",
        "third suspension",
        "as one unit",
    ] {
        assert!(
            source.contains(invariant),
            "missing successor boundary: {invariant}"
        );
    }
    let mut examples = 0;
    for block in source.split("```leselang\n").skip(1) {
        leselang_hir::lower(&leselang_syntax::parse(block.split("```").next().unwrap())).unwrap();
        examples += 1;
    }
    assert!(examples >= 8);
}

#[test]
fn leselang_dataflow_documentation_keeps_typed_frames_and_finite_boundaries_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    for invariant in [
        "## Durable Result Chains",
        "schema 4",
        "typed result\nprojections",
        "Journal schema 9",
        "64 KiB",
        "16-level scope limit",
        "64 steps",
        "mixed exits use",
        "does not replenish",
        "before admitting another effect",
        "as one unit",
    ] {
        assert!(
            source.contains(invariant),
            "missing dataflow contract: {invariant}"
        );
    }
    let section = source.split("## Durable Result Chains").nth(1).unwrap();
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
}

#[test]
fn leselang_conditional_exit_contract_keeps_recovery_and_budget_boundaries_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    for invariant in [
        "## Conditional Exits",
        "schema 5",
        "Journal schema 10",
        "same scalar",
        "cannot have a child",
        "never reevaluates its predicate",
        "before scalar projection",
        "LSV2404",
        "type, scope and capability preflight",
        "Image-only restore",
        "without rewriting old images",
        "private result frames",
    ] {
        assert!(
            source.contains(invariant),
            "missing conditional-exit contract: {invariant}"
        );
    }
    let example = source
        .split("## Conditional Exits")
        .nth(1)
        .unwrap()
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
}

#[test]
fn named_group_result_contract_keeps_static_members_and_whole_journal_recovery_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Named Group Result Bindings")
        .nth(1)
        .unwrap()
        .split("## Result-Driven Successor")
        .next()
        .unwrap();
    for invariant in [
        "literal step",
        "pure scalar body",
        "schema 6",
        "bound_parallel",
        "bound_sequential",
        "64 KiB",
        "complete original journal",
        "Vm::restore_request(request)",
        "LSV1409",
        "one transaction",
        "before projection",
        "single-member sequences",
        "schema 10",
    ] {
        assert!(
            section.contains(invariant),
            "missing group-result boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn sequential_group_tail_contract_matches_typed_hir_and_transactional_limits() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Sequential Group Tails")
        .nth(1)
        .unwrap()
        .split("## Result-Driven Successor")
        .next()
        .unwrap();
    for invariant in [
        "63 members",
        "schema 7",
        "group_tail",
        "successor_sequence",
        "reservation is not a dispatch",
        "one transaction",
        "complete journal",
        "LSV1409",
        "original principal",
        "shared fuel",
        "before projection",
        "LSV1404",
        "schema 10",
        "Parallel `all` tails",
        "further result capture",
        "one logical record",
    ] {
        assert!(
            section.contains(invariant),
            "missing group-tail boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(program.function.result_type, leselang_hir::Type::UiFocus);
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn captured_sequential_successor_contract_preserves_raw_receipts_and_closed_member_frames() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Captured Sequential Successors")
        .nth(1)
        .unwrap()
        .split("## Parallel Group Successors")
        .next()
        .unwrap();
    for invariant in [
        "one captured atomic successor",
        "pure scalar body",
        "schema 8",
        "group_capture",
        "63-member prefix",
        "`groups`",
        "projection version",
        "cold branches",
        "shared fuel",
        "64 KiB",
        "LSV3002",
        "raw receipt",
        "before scalar projection",
        "LSV2404",
        "one transaction",
        "complete journal",
        "LSV1409",
        "schema 10",
        "second",
        "parallel `all`",
    ] {
        assert!(
            section.contains(invariant),
            "missing captured-group boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn parallel_group_successor_contract_keeps_barrier_budget_and_native_batch_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Parallel Group Successors")
        .nth(1)
        .unwrap()
        .split("## Group Result Chains")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "one atomic tail",
        "pure scalar body",
        "2 to 63",
        "schema 9",
        "parallel_group_tail",
        "parallel_group_capture",
        "all-success",
        "out of order",
        "declared member order",
        "one transaction",
        "exactly one request",
        "one journal snapshot",
        "original authority",
        "shared fuel",
        "64 KiB",
        "LSV3002",
        "before scalar projection",
        "durable `LSV2404`",
        "8 MiB",
        "every individual receipt",
        "complete owned",
        "committed successful prefix",
        "LSV1409",
        "Journal schema 10",
        "Rust batch API",
        "single-presentation channel",
        "debugger_session_not_suspended",
        "before creating a session",
        "schema-10 chain contract",
        "Multi-request",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing parallel-tail contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Integer)
    );
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
}

#[test]
fn group_result_chain_contract_matches_typed_hir_and_owned_transaction_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Group Result Chains")
        .nth(1)
        .unwrap()
        .split("## Group Conditional Exits")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "multiple captured atomic successors",
        "longest cold chain",
        "64 graph slots",
        "62 prefix members",
        "additional_successor_sequences",
        "Unused reservations",
        "one successor is pending",
        "schema 10",
        "group_dataflow",
        "parallel_group_dataflow",
        "journal schema 10 is unchanged",
        "versioned",
        "legacy v1",
        "raw receipt and next request commit in one transaction",
        "original authority",
        "shared fuel",
        "before every admission",
        "before scalar projection",
        "LSV2404",
        "LSV3002",
        "8 MiB",
        "64 KiB",
        "complete owned journal",
        "frame continuity",
        "LSV1409",
        "retention unit",
        "native single-presentation debugger",
        "Rust batch API",
        "mixed scalar early exits",
        "effectful loops",
        "cold branches",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing group-chain contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn group_conditional_exit_contract_keeps_cold_paths_and_atomic_replay_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Group Conditional Exits")
        .nth(1)
        .unwrap()
        .split("## Result-Driven Successor")
        .next()
        .unwrap();
    let normalized = section.replace('\n', " ");
    for invariant in [
        "before the first capture or between captures",
        "same scalar type",
        "schema 11",
        "group_conditional",
        "parallel_group_conditional",
        "Journal schema 10 is unchanged",
        "schemas 1-10",
        "longest cold path",
        "64 graph slots",
        "type and capability preflight",
        "Unused reservations",
        "all-success barrier",
        "pending or failed member",
        "raw receipt and scalar exit commit in one transaction",
        "first committed exit or successor",
        "original authority",
        "shared fuel",
        "before any scalar exit or admission",
        "LSV2404",
        "LSV3002",
        "8 MiB",
        "64 KiB",
        "complete owned journal",
        "without reevaluating source",
        "can return without another suspension",
        "mandatory capture",
        "schema downgrades",
        "LSV1409",
        "retention unit",
        "native single-presentation debugger",
        "Rust batch API",
        "before creating a session",
        "effectful guards/operands",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing group-conditional contract: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    let leselang_hir::Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    let leselang_hir::computation::Computation::Bind { body, .. } = *expression else {
        panic!()
    };
    assert!(body.is_result_flow());
    assert!(!body.is_result_chain());
    assert_eq!(body.atomic_flow_bound(), Some(2));
}

#[test]
fn leselang_conversion_contract_connects_computation_to_validated_host_text() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Explicit Scalar Conversions")
        .nth(1)
        .unwrap()
        .split("## Bounded Pure Loops")
        .next()
        .unwrap();
    for invariant in [
        "to_string",
        "parse_integer",
        "parse_boolean",
        "non-empty ASCII decimal",
        "Leading zeros in text",
        "LSV1408",
        "never the input",
        "4096-byte",
        "shared fuel",
        "original host validators",
        "all-or-nothing",
        "unknown operators",
        "journal schema 10",
        "no storage-layout migration",
    ] {
        assert!(
            section.contains(invariant),
            "missing conversion boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::UiSetFormValue
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_recovery_documentation_keeps_data_errors_separate_from_host_failures() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Pure Calculation Recovery")
        .nth(1)
        .unwrap()
        .split("## Result Bindings")
        .next()
        .unwrap();
    for invariant in [
        "same scalar type",
        "enclosing scope",
        "even when cold",
        "LSH1411",
        "set is closed",
        "LSV1401",
        "LSV1408",
        "never refunded",
        "not recoverable",
        "host failure",
        "journal schema 10",
        "duplicate results replay",
        "not implement recovery around host effects",
    ] {
        assert!(
            section.contains(invariant),
            "missing recovery boundary: {invariant}"
        );
    }
    let examples = section
        .split("```leselang\n")
        .skip(1)
        .map(|block| {
            leselang_hir::lower(&leselang_syntax::parse(block.split("```").next().unwrap()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(examples.len(), 2);
    assert_eq!(
        examples[0].function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::Integer)
    );
    assert!(examples[0].function.required_capabilities.is_empty());
    assert_eq!(
        examples[1].function.result_type,
        leselang_hir::Type::UiSetFormValue
    );
    assert_eq!(
        examples[1].function.required_capabilities,
        ["ui.presentation"]
    );
}

#[test]
fn leselang_loop_documentation_is_executable_and_keeps_the_pure_boundary_explicit() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    for invariant in [
        "## Bounded Pure Loops",
        "0 through 1024",
        "state transitions, not condition checks",
        "LSV1406",
        "does not receive fresh fuel",
        "no mid-loop checkpoint or host effect",
        "This does not add effectful loops",
    ] {
        assert!(
            source.contains(invariant),
            "missing loop boundary: {invariant}"
        );
    }
    let mut examples = 0;
    for block in source.split("```leselang\n").skip(1) {
        let program = block.split("```").next().unwrap();
        if program.contains("loop(") {
            leselang_hir::lower(&leselang_syntax::parse(program)).unwrap();
            examples += 1;
        }
    }
    assert!(
        examples >= 2,
        "pure and result-bound loop examples must compile"
    );
}

#[test]
fn leselang_concurrency_contract_separates_engines_roots_batches_and_host_namespaces() {
    let source = fs::read_to_string(repository_root().join("docs/leselang-embedding.md")).unwrap();
    let section = source
        .split("## Concurrency Model")
        .nth(1)
        .unwrap()
        .split("## Current Boundary And Next Proof")
        .next()
        .unwrap();
    for invariant in [
        "multiple isolated execution contexts with bounded host workers",
        "One mutable engine is entered serially",
        "one VM can suspend multiple roots",
        "all-success barrier",
        "No process-global interpreter lock",
        "same journal namespace",
        "does not rerun source or refill fuel",
        "attempt-fenced leases",
        "local cache, not a global\nqueue count",
        "journal-local, not globally unique",
        "persisted, validated\nadapter-level identity mapping",
        "resource lane",
        "fewest delivery attempts",
        "numeric admission order",
        "fixed eligible cohort",
        "without changing wire formats",
        "reports `LSV4017`",
        "not a global FIFO or tenant-fairness guarantee",
        "Host Admission And Backpressure",
        "trusted host policy",
        "inside the admission write transaction",
        "DispatchClaim::Backpressured",
        "not\n`None`",
        "does not issue a lease or increment attempts",
        "Already admitted chains and group tails",
        "the same limits to all workers in one journal namespace",
        "outbox bounds, not\na total memory or CPU ceiling",
        "not persisted in continuation or journal wire formats",
        "native\nparallel batches still fail preflight",
        "Scheduler faults use `LSV2500` through `LSV2503`",
        "`LSV2401` through `LSV2404` retain their meanings",
        "Restore And Allocator Recovery",
        "high-water mark in the same transaction",
        "one consistent write-locked snapshot",
        "unused cold group\nreservations",
        "original watermark before repair",
        "a high imported identity cannot legitimize a forged reservation",
        "Compaction never lowers the watermark",
        "Repair cannot reconstruct identities",
        "does not scan\nthe journal on every allocation",
        "Bounded Host Ingress",
        "host-owned, single-use pre-admission handle",
        "total attempts including the first",
        "VM-free and journal-free",
        "pinned at submission",
        "bounded pure preparation may repeat",
        "no accepted root receives fresh fuel through this API",
        "not cloneable or deserializable",
        "observations, not checkpoint/replay authority",
        "no hidden queue, timer,\nworker pool or global lock",
        "Backoff supplies neither jitter nor fairness",
        "Native debugger starts do not automatically opt into deferred admission",
    ] {
        assert!(
            section.contains(invariant),
            "missing concurrency boundary: {invariant}"
        );
    }
    let reference =
        fs::read_to_string(repository_root().join("docs/leselang-language.md")).unwrap();
    assert!(reference.contains("(leselang-embedding.md#concurrency-model)"));
    assert!(reference.contains("(leselang-embedding.md#dispatch-selection)"));
    assert!(reference.contains("(leselang-embedding.md#host-admission-and-backpressure)"));
    assert!(reference.contains("(leselang-embedding.md#bounded-host-ingress)"));
}

#[test]
fn leselang_embedding_separates_language_hosts_and_future_shell() {
    let root = repository_root();
    let embedding = fs::read_to_string(root.join("docs/leselang-embedding.md"))
        .expect("Leselang embedding architecture must exist");
    for invariant in [
        "fully independent of Gewyvern and Leserpent",
        "GUI automation is one host profile",
        "## Agent-First Syntax",
        "capability references, not syntax or",
        "generation, validation, composition and recovery",
        "edited source must not silently reuse an old continuation",
        "Untrusted GUI text and host results remain data",
        "this section introduces no new syntax",
        "shell for nuis OS and sirius kernel",
        "does not require an OS implementation now",
        "host/profile -> language contracts and core",
        "explicit developer-owned adapters or schema-generated bindings",
        "VM fuel cannot preempt arbitrary",
        "never host stack frames or native object pointers",
        "not full semantic independence",
        "current VM still links SQLite and product types",
        "Two independent host schemas",
        "leselang/language-vm/host-neutral-embedding",
        "leselang-runtime-core",
        "AdmissionAdapter<Input>",
        "standalone package",
        "not a second scheduler implementation",
        "typed language-operation schema or complete evaluator acceptance gate",
        "read-only admission snapshot",
        "debug output contains scheduling metadata only",
        "Snapshots can become stale",
        "Cleanup panics are not swallowed",
        "payload-free terminal reason",
        "AdmissionEnd",
        "legacy `Finished { attempts }`",
        "Acceptance is not output delivery or execution completion",
        "not a\nrejection or automatic replay permission",
        "explicit pending-or-terminal ownership state",
        "before they enter the return slot",
        "Only explicit pre-publication pressure can rearm input",
        "pure policy preflight",
        "bounded exhaustion diagnostic",
        "Direct permanent host rejections remain verbatim",
        "must-use ownership diagnostics",
        "conditional thread ownership",
        "Attempts, the observed clock",
        "These tests cover\nadmission only",
        "### Shared Fuel Accounting",
        "host-granted fuel accounting",
        "validated saved\nremaining fuel",
        "fuel wire fields remain unchanged",
        "not a sandbox or callback preemption",
        "### Shared Scheduler Clock",
        "portable scheduler-clock arithmetic",
        "parameter preflight before expiry cleanup",
        "do not wait for a SQLite writer lock",
        "overflow-to-pinned-deadline clamp",
        "No absolute-time wire field or journal schema changes",
        "Current-time validation is distinct from future-time construction",
        "one-millisecond future lease",
        "Pressure observation never\nreaps execution deadlines",
        "A due execution deadline takes precedence",
        "Retry overflow remains `LSV2203`",
        "### Shared Retry Delay Arithmetic",
        "pure capped exponential delay arithmetic",
        "without allocation or exponent-sized loops",
        "does not weaken policy validation",
        "Admission attempts, delivery attempts and semantic retries remain distinct budgets",
        "restarting with zero default fuel between retries",
        "adds no policy or wire fields and is not evaluator independence",
        "OS integration is a deferred direction",
    ] {
        assert!(
            embedding.contains(invariant),
            "Leselang embedding architecture lacks boundary: {invariant}"
        );
    }
    for path in [
        "docs/leselang-language.md",
        "docs/leserpent-2-architecture.md",
        "docs/architecture-blueprint.md",
        "docs/architecture-evolution.md",
    ] {
        let source = fs::read_to_string(root.join(path)).expect("architecture page must exist");
        assert!(source.contains("(leselang-embedding.md)"), "{path}");
        assert!(!source.contains("the \"JavaScript\" of Leserpent automation only"));
    }
    let module = fs::read_to_string(root.join("docs/modules/leselang.md")).unwrap();
    assert!(module.contains("(../leselang-embedding.md)"));
}

#[test]
fn documentation_index_routes_to_each_small_domain_module() {
    let root = repository_root();
    let index = fs::read_to_string(root.join("docs/index.md")).expect("docs index must exist");

    for module in MODULES {
        assert!(
            index.contains(&format!("(modules/{module})")),
            "docs index must route to {module}"
        );

        let path = root.join("docs/modules").join(module);
        let source = fs::read_to_string(&path).expect("documentation module must exist");
        assert!(
            source.lines().count() <= 60,
            "module must stay compact: {module}"
        );
        assert!(
            source.contains("## Start"),
            "module needs a start path: {module}"
        );
        assert!(
            markdown_link_targets(&source).len() >= 5,
            "module needs useful routing: {module}"
        );
    }
}

#[test]
fn architecture_shelf_defines_one_protocolized_debugging_fabric() {
    let root = repository_root();
    let blueprint = fs::read_to_string(root.join("docs/architecture-blueprint.md"))
        .expect("canonical architecture blueprint must exist");
    let coordination = fs::read_to_string(root.join("docs/architecture-coordination.md"))
        .expect("architecture coordination contract must exist");
    let evolution = fs::read_to_string(root.join("docs/architecture-evolution.md"))
        .expect("architecture evolution contract must exist");
    let monorepo = fs::read_to_string(root.join("docs/monorepo-stack.md"))
        .expect("monorepo stack guide must exist");
    let bridge = fs::read_to_string(root.join("apps/leserpent/README.md"))
        .expect("Web compatibility bridge documentation must exist");

    for invariant in [
        "replayable, protocolized network debugging fabric",
        "## Four Planes",
        "### Evidence Plane",
        "### Authority Plane",
        "### Intent Plane",
        "### Presentation Plane",
        "### Advisory Sideplane",
        "one kernel/container boundary -> one Gewyvern service",
        "one leserpentd authority      -> many Gewyvern services",
        "one Leserpent client          -> many independent leserpentd authorities",
        "## The Advantage Zone",
        "## Scope Guardrails",
        "silvortex-bounded-io",
        "silvortex-identity",
        "gewyvern-install-contract",
    ] {
        assert!(
            blueprint.contains(invariant),
            "canonical blueprint lacks invariant: {invariant}"
        );
    }

    let (minor_line, _) = env!("CARGO_PKG_VERSION")
        .rsplit_once('.')
        .expect("workspace version must be semantic");
    let priorities_heading = format!("## Current {minor_line}.x Priorities");
    for invariant in [
        "## Compatibility Bridge Rule",
        priorities_heading.as_str(),
        "Horizon 2: Remove Boundary Debt",
        "A shared checkout does not imply shared\nauthority",
        "Managed persistence must not become a second authority",
    ] {
        assert!(
            coordination.contains(invariant)
                || evolution.contains(invariant)
                || monorepo.contains(invariant),
            "architecture shelf lacks invariant: {invariant}"
        );
    }

    assert!(bridge.contains("ASP.NET/TypeScript compatibility bridge"));
    assert!(bridge.contains("不再新增只存在于 managed runtime 的 control-plane 语义"));
    for stale in [
        "2.0 目标",
        "目标 2.0",
        "即时 gRPC",
        "最小 ASP.NET Core control-plane 骨架",
    ] {
        assert!(
            !bridge.contains(stale),
            "Web bridge documentation restored stale architecture: {stale}"
        );
    }
}

#[test]
fn root_product_navigation_exposes_leserpent_as_a_first_class_entry() {
    let root = repository_root();
    let gewyvern = fs::read_to_string(root.join("README.md")).expect("root README must exist");
    let leserpent =
        fs::read_to_string(root.join("LESERPENT.md")).expect("Leserpent product page must exist");
    let implementation = fs::read_to_string(root.join("apps/leserpent/README.md"))
        .expect("Leserpent implementation README must exist");

    assert!(gewyvern.contains("href=\"LESERPENT.md\""));
    assert!(leserpent.contains("href=\"README.md\""));
    assert!(implementation.contains("(../../LESERPENT.md)"));
    let (minor_line, _) = env!("CARGO_PKG_VERSION")
        .rsplit_once('.')
        .expect("workspace version must be semantic");
    assert!(leserpent.contains(&format!("# Leserpent v{minor_line}.x")));
    for invariant in [
        "one or more leserpentd authorities",
        "One Leserpent client can manage multiple independent `leserpentd` authorities",
        "Credentials protect infrastructure authority",
        "does not require a remote connection before local Orchestra",
        "cargo dev package desktop",
        "cargo dev package control",
        "not Apple-notarized",
    ] {
        assert!(
            leserpent.contains(invariant),
            "Leserpent product page lacks invariant: {invariant}"
        );
    }
}

#[test]
fn tutorial_shelf_covers_cli_desktop_languages_and_remote_lifecycle() {
    let root = repository_root();
    let shelf =
        fs::read_to_string(root.join("docs/book/tutorials.md")).expect("tutorial shelf must exist");
    let contracts: &[(&str, &[&str])] = &[
        (
            "tutorial-first-run.md",
            &[
                "--list-protocols",
                "--list-entries quic",
                "--protocol postgres --entry query",
                "--scan-all",
            ],
        ),
        (
            "tutorial-leserpent-desktop.md",
            &[
                "Local Orchestra",
                "+ Add daemon",
                "Workspace Leselang",
                "--verify-desktop-tutorial",
            ],
        ),
        (
            "tutorial-gewylang-package.md",
            &["gewyc -- init", "gewy.pkg", "frontend", "use(...)"],
        ),
        (
            "tutorial-leselang-gui-automation.md",
            &[
                "--export-leselang",
                "--export-plan",
                "opens no socket",
                "ui.presentation",
                "Run live",
            ],
        ),
        (
            "tutorial-remote-deployment-lab.md",
            &[
                "vault:ssh:*",
                "bootstrap deploy",
                "bootstrap bind",
                "runtime provision",
                "runtime retire",
                "bootstrap retire",
            ],
        ),
    ];

    for (file, markers) in contracts {
        assert!(
            shelf.contains(&format!("({file})")),
            "tutorial shelf must route to {file}"
        );
        let source = fs::read_to_string(root.join("docs/book").join(file))
            .expect("tutorial page must exist");
        assert!(source.starts_with("# Tutorial:"), "invalid title in {file}");
        assert!(
            source.contains("## Prerequisites"),
            "tutorial must name prerequisites: {file}"
        );
        assert!(
            source.contains("## Completion Checkpoint"),
            "tutorial must name its observed finish: {file}"
        );
        assert!(
            !source.contains("](docs/"),
            "book tutorial must use local relative links: {file}"
        );
        for marker in *markers {
            assert!(
                source.contains(marker),
                "{file} lacks contract marker {marker}"
            );
        }
    }

    let root_usage = fs::read_to_string(root.join("src/main/ui_locale/catalog.rs"))
        .expect("Gewyvern usage catalog must exist");
    for option in ["--list-protocols", "--list-entries", "--scan-all"] {
        assert!(root_usage.contains(option), "Gewyvern CLI lacks {option}");
    }

    let gewyc_usage = fs::read_to_string(root.join("crates/gewyc/src/main.rs"))
        .expect("gewyc CLI source must exist");
    for command in ["gewyc init", "explain|frontend", "stages|envelope"] {
        assert!(gewyc_usage.contains(command), "gewyc CLI lacks {command}");
    }

    let leserpent_usage = fs::read_to_string(root.join("crates/leserpent-cli/src/lib.rs"))
        .expect("Leserpent CLI source must exist");
    for command in [
        "bootstrap deploy",
        "bootstrap inspect",
        "bootstrap bind",
        "bootstrap retire",
        "runtime provision",
        "runtime inspect",
        "runtime logs",
        "runtime retire",
        "--export-leselang",
        "--export-plan",
    ] {
        assert!(
            leserpent_usage.contains(command),
            "Leserpent CLI lacks tutorial command {command}"
        );
    }

    let remote = fs::read_to_string(root.join("docs/book/tutorial-remote-deployment-lab.md"))
        .expect("remote tutorial must exist");
    let runtime_retirement = remote
        .find("runtime retire \"$RUNTIME_ID\"")
        .expect("runtime retirement step must exist");
    let daemon_retirement = remote
        .find("bootstrap retire \"$BOOTSTRAP_ID\"")
        .expect("daemon retirement step must exist");
    assert!(
        runtime_retirement < daemon_retirement,
        "remote tutorial must retire the runtime before its daemon"
    );
    for forbidden in ["--password", "--private-key", "--sudo-password", "sshpass"] {
        assert!(
            !remote.contains(forbidden),
            "remote tutorial must not introduce raw secret input {forbidden}"
        );
    }
}

#[test]
fn documentation_tree_has_no_dangling_local_links() {
    let root = repository_root();
    let mut documents = vec![root.join("README.md"), root.join("LESERPENT.md")];
    collect_markdown(&root.join("docs"), &mut documents);

    let mut checked = 0usize;
    for document in documents {
        let source = fs::read_to_string(&document).expect("markdown document must be readable");
        for target in markdown_link_targets(&source) {
            let target = target.trim().trim_start_matches('<').trim_end_matches('>');
            if target.starts_with('#')
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }

            let path = target.split('#').next().unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            checked += 1;

            let relative = document.parent().unwrap_or(&root).join(path);
            let repository_relative = root.join(path);
            assert!(
                relative.exists() || repository_relative.exists(),
                "broken local link in {}: {target}",
                document.strip_prefix(&root).unwrap_or(&document).display()
            );
        }
    }

    assert!(
        checked >= 2_500,
        "expected to validate the full documentation tree"
    );
}

#[test]
fn leserpent_next_major_has_one_architecture_and_one_delivery_roadmap() {
    let root = repository_root();
    let architecture = fs::read_to_string(root.join("docs/leserpent-2-architecture.md"))
        .expect("Leserpent 2.0 architecture must exist");
    let roadmap = fs::read_to_string(root.join("docs/leserpent-2-roadmap.md"))
        .expect("Leserpent 2.0 roadmap must exist");
    let project = fs::read_to_string(root.join("docs/modules/project.md"))
        .expect("project module must exist");
    let root_roadmap =
        fs::read_to_string(root.join("ROADMAP.md")).expect("root roadmap must exist");

    for invariant in [
        "Non-Negotiable Invariants",
        "GUI, CLI, and Leselang",
        "Leserpent is the first reference host of Leselang",
        "Full VM independence is not yet implemented",
        "renderer-neutral UI semantics belong to an optional host profile",
        "Rust crate",
        "FFI boundary",
        "No GUI framework becomes compatible automatically",
        "developer-owned adapter",
        "generated binding",
        "UiAdapterManifest",
        "2.0 Scope Boundary",
        "The 2.0 scope is frozen",
        "released architecture",
        "without silently adding a new core\ncapability family",
        "Etragon advisory",
        "Windows native parity",
        "automatic GUI framework compatibility",
        "This closes the 2.0 reverse-bootstrap scope",
        "optional post-2.0 work",
        "mobile retains its minimum entry/lifecycle conformance contract",
        "synchronous source semantics",
        "CommandEnvelope",
        "EffectRequest",
        "UiDocument",
        "atomic replaceability",
    ] {
        assert!(
            architecture.contains(invariant),
            "2.0 architecture must preserve invariant: {invariant}"
        );
    }

    for gate in 1..=7 {
        assert!(
            roadmap.contains(&format!("## Gate {gate}:")),
            "2.0 roadmap must preserve delivery gate {gate}"
        );
    }
    let normalized_roadmap = roadmap.split_whitespace().collect::<Vec<_>>().join(" ");
    for freeze_rule in [
        "## 2.0 Scope Freeze",
        "The core 2.0 capability set is closed",
        "No new core capability family may enter",
        "Remaining minor versions are allowed to finish only the already-declared",
        "Accepted work after the freeze is closure work",
        "Rejected work is scope expansion",
        "moving Etragon into the release gate",
        "claiming Windows native parity",
        "making GUI frameworks automatically compatible",
        "WinRM is explicitly outside the 2.0 evidence gate",
        "physical device release parity is deferred",
        "full mobile device release parity beyond the declared entry/lifecycle contract",
        "Every capability inside this frozen scope is part of the MIT open-source free core",
        "Future commercial work is limited to newly introduced hosted service extensions",
    ] {
        assert!(
            normalized_roadmap.contains(freeze_rule),
            "2.0 roadmap must preserve scope-freeze rule: {freeze_rule}"
        );
    }

    for retired_gate in [
        "desktop and one mobile target pass release tests",
        "desktop and one mobile target pass the same semantic conformance suite",
        "WinRM is the remaining deferred evidence gate",
    ] {
        assert!(
            !architecture.contains(retired_gate) && !roadmap.contains(retired_gate),
            "2.0 documentation must not restore retired release gate: {retired_gate}"
        );
    }

    assert!(project.contains("(../leserpent-2-architecture.md)"));
    assert!(project.contains("(../leserpent-2-roadmap.md)"));
    assert!(root_roadmap.contains("(docs/leserpent-2-architecture.md)"));
    assert!(root_roadmap.contains("(docs/leserpent-2-roadmap.md)"));
}

#[test]
fn leselang_text_inspection_contract_preserves_unicode_fuel_and_durable_boundaries() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Bounded Text Inspection")
        .nth(1)
        .unwrap()
        .split("## Bounded String Collections")
        .next()
        .unwrap();
    let normalized = section.split_whitespace().collect::<Vec<_>>().join(" ");
    for invariant in [
        "contains",
        "starts_with",
        "ends_with",
        "char_at",
        "exact and case-sensitive",
        "Unicode normalization",
        "zero-based Unicode scalar-value",
        "grapheme clusters",
        "u64::MAX",
        "present-empty",
        "left-to-right",
        "operators are eager",
        "shared fuel",
        "complete UTF-8 byte length",
        "materialization block",
        "4096-byte",
        "original host validators",
        "all-or-nothing",
        "v1-v5",
        "1 through 11",
        "journal schema 10",
        "Unknown operators",
        "without source/helper tables",
        "64 KiB",
        "rollback",
        "first-commit replay",
        "private frames",
        "not a new desktop scalar inspector",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing text inspection boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::UiSetFormValue
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn leselang_collection_contract_has_compilable_examples_and_closed_resource_semantics() {
    let source =
        fs::read_to_string(repository_root().join("docs/leselang-control-flow.md")).unwrap();
    let section = source
        .split("## Bounded String Collections")
        .nth(1)
        .unwrap()
        .split("## Bounded Pure Loops")
        .next()
        .unwrap();
    let normalized = section.split_whitespace().collect::<Vec<_>>().join(" ");
    for invariant in [
        "string_list",
        "source order",
        "64 entries",
        "4096 bytes",
        "LSV1403",
        "LSV1406",
        "before the first iteration",
        "0 through",
        "Unicode scalar boundaries",
        "present-empty",
        "hygienically",
        "items` first",
        "shared fuel",
        "mid-loop suspension",
        "not caught",
        "array is required",
        "untrusted length hint",
        "1 through 11",
        "v1-v5",
        "64 KiB",
        "first-commit replay",
        "original host validators",
        "not a live Avalonia",
        "private frames",
    ] {
        assert!(
            normalized.contains(invariant),
            "missing collection contract boundary: {invariant}"
        );
    }
    let example = section
        .split("```leselang\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let program = leselang_hir::lower(&leselang_syntax::parse(example)).unwrap();
    assert_eq!(
        program.function.result_type,
        leselang_hir::Type::Scalar(leselang_hir::computation::ScalarType::StringList)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("documentation directory must be readable") {
        let path = entry.expect("documentation entry must be readable").path();
        if path.is_dir() {
            collect_markdown(&path, output);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            output.push(path);
        }
    }
}

fn markdown_link_targets(source: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut remaining = source;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        let raw = remaining[..end].trim();
        let target = raw.split_whitespace().next().unwrap_or_default();
        if !target.is_empty() {
            targets.push(target);
        }
        remaining = &remaining[end + 1..];
    }
    targets
}
