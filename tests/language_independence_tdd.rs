use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

fn workspace_metadata() -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata should start");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("cargo metadata should be valid JSON")
}

fn workspace_dependency_graph() -> BTreeMap<String, BTreeSet<String>> {
    let metadata = workspace_metadata();
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata should contain packages");
    let workspace_names = packages
        .iter()
        .filter(|package| package["source"].is_null())
        .filter_map(|package| package["name"].as_str())
        .collect::<BTreeSet<_>>();

    packages
        .iter()
        .filter_map(|package| {
            let name = package["name"].as_str()?;
            if !workspace_names.contains(name) {
                return None;
            }
            let dependencies = package["dependencies"]
                .as_array()?
                .iter()
                .filter(|dependency| dependency["kind"] != "dev")
                .filter_map(|dependency| dependency["name"].as_str())
                .filter(|dependency| workspace_names.contains(dependency))
                .map(str::to_string)
                .collect();
            Some((name.to_string(), dependencies))
        })
        .collect()
}

fn dependency_closure(graph: &BTreeMap<String, BTreeSet<String>>, root: &str) -> BTreeSet<String> {
    let mut pending = vec![root.to_string()];
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop() {
        if !visited.insert(package.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&package) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    visited.remove(root);
    visited
}

fn assert_workspace_closure(root: &str, expected: &[&str]) {
    let graph = workspace_dependency_graph();
    assert!(
        graph.contains_key(root),
        "workspace package '{root}' is missing"
    );
    let actual = dependency_closure(&graph, root);
    let expected = expected
        .iter()
        .map(|dependency| (*dependency).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "'{root}' gained a workspace dependency; review the standalone language boundary"
    );
}

#[test]
fn gewylang_frontend_and_compiler_have_no_product_dependency() {
    assert_workspace_closure("gewylang-contract", &[]);
    assert_workspace_closure("gewylang-ir", &["gewylang-contract"]);
    assert_workspace_closure("gewylang-syntax", &["gewylang-contract"]);
    assert_workspace_closure(
        "gewylang-compiler",
        &["gewylang-contract", "gewylang-syntax"],
    );
}

#[test]
fn leselang_frontend_and_host_contract_have_no_product_dependency() {
    assert_workspace_closure("leselang-syntax", &[]);
    assert_workspace_closure("leselang-host-contract", &["silvortex-identity"]);
    assert_workspace_closure(
        "leselang-hir",
        &[
            "leselang-host-contract",
            "leselang-runtime-core",
            "leselang-syntax",
            "silvortex-identity",
        ],
    );
}

#[test]
fn shared_control_ir_has_explicit_native_slots_and_reference_aliases_not_a_second_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ir = std::fs::read_to_string(root.join("crates/leselang-hir/src/ir.rs")).unwrap();
    for product in [
        "HostOperation",
        "ResultField",
        "crate::Type",
        "leserpent",
        "sqlite",
    ] {
        assert!(!ir.contains(product), "shared control IR leaked {product}");
    }
    assert!(ir.contains("pub enum Computation<Field, Operation, HostEffect, ResultType>"));
    assert!(ir.contains("pub struct ComputedArgument<Expression>"));
    assert!(ir.contains("pub struct ComputedBranch<Expression, ResultType>"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains(
        "pub type Computation = crate::ir::Computation<ResultField, HostOperation, Effect, Type>"
    ));
    assert!(
        reference
            .contains("pub type ComputedBranch = crate::ir::ComputedBranch<Computation, Type>")
    );
    assert!(
        reference.contains("pub type ComputedArgument = crate::ir::ComputedArgument<Computation>")
    );
    assert!(!reference.contains("pub enum Computation"));
    assert!(!reference.contains("fn children("));
    assert!(!reference.contains("fn is_pure("));
}

#[test]
fn shared_argument_typing_uses_native_domains_and_the_reference_ir_bridge() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let checker =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/argument_typing.rs"))
            .unwrap();
    for product in [
        "leselang_hir",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !checker.contains(product),
            "shared argument typing leaked {product}"
        );
    }
    assert!(checker.contains("pub trait ScalarArgumentDomain"));
    assert!(checker.contains("pub fn check_argument_types("));
    let lower =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(lower.contains("lower_preflighted_arguments("));
    assert!(lower.contains("Type::Scalar(ty) => Some(ty)"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/source_call.rs")).unwrap();
    assert!(source.contains("check_argument_type(&parameter.domain, facts)"));
    assert!(source.contains("is_pure: true"));
    let domains =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/host_call.rs")).unwrap();
    assert!(domains.contains("impl ScalarArgumentDomain for ArgumentDomain"));
    assert!(domains.contains("check_argument_type(&self, ScalarArgumentType::literal(value))"));
}

#[test]
fn native_source_call_bridge_uses_original_ast_catalog_and_shared_ir_without_product_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/source_call.rs")).unwrap();
    for product in [
        "HostOperation",
        "ResultField",
        "crate::Type",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "source call bridge leaked {product}"
        );
    }
    assert!(source.contains("use leselang_syntax::{Expression, NamedArgument, Span};"));
    assert!(source.contains("use crate::ir::{Computation, ComputedArgument};"));
    assert!(source.contains("pub fn lower_source_call"));
    let public = source.split_once("pub fn lower_source_call").unwrap().1;
    assert!(
        public.find("preflight(expression, limits)?;").unwrap()
            < public
                .find(".authorize(callee.as_str(), host.version, host.granted)")
                .unwrap()
    );
    assert!(
        public.find(".bind_arguments(&names)").unwrap() < public.find("lower(argument)").unwrap()
    );
    assert!(public.contains("preflight_with_budget(&value, 1, &mut budget)"));
    assert!(public.find(".check_pending(0, 1)").unwrap() < public.find("lower(argument)").unwrap());
    assert!(!source.contains(".scalar_argument_type("));
    assert!(!source.contains("pub enum Expression"));
    assert!(!source.contains(".clone()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("return lowered\n            .finish_call("));
    assert!(!reference.contains("let bindings = operation.bind_names(&names"));
}

#[test]
fn prepared_native_calls_delegate_owned_construction_and_require_fresh_whole_call_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/source_call.rs")).unwrap();
    let finish = source
        .split_once("pub fn finish_call")
        .unwrap()
        .1
        .split_once("impl<Key, Domain")
        .unwrap()
        .0;
    assert!(finish.contains("map: impl FnOnce("));
    assert!(finish.contains("admit: impl FnOnce("));
    assert!(
        finish
            .find("preflight_with_budget(&argument.value, 1, &mut budget)")
            .unwrap()
            < finish.find("map(schema)").unwrap()
    );
    assert!(
        finish.find("let call = Computation::Call").unwrap()
            < finish.find("admit(&call, &output, schema)").unwrap()
    );
    assert!(finish.contains("arguments: self.into_arguments()"));
    assert!(finish.contains("Ok((call, output))"));
    for forbidden in [".clone()", "serde", "Fuel", "HostOperation", "crate::Type"] {
        assert!(
            !finish.contains(forbidden),
            "native call completion leaked {forbidden}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("SourceCallFinishLimits"));
    assert!(reference.contains("std::ptr::eq(schema, operation.schema())"));
    assert!(reference.contains("crate::pure_reference::infer_source_call_in_scope("));
    assert!(!reference.contains("arguments: lowered.into_arguments()"));
    let native =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_reference.rs")).unwrap();
    let native = native
        .split_once("pub(crate) fn infer_source_call_in_scope")
        .unwrap()
        .1
        .split_once("fn limits()")
        .unwrap()
        .0;
    assert!(native.contains("scope\n        .bindings()"));
    assert!(native.contains("local.members().map(ReferenceMembers::External)"));
    assert!(native.contains("max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS"));
    assert!(native.contains("check_call_arguments("));
    for recursive in [
        "validate_in_type_scope",
        "canonical_source",
        "lower_expression",
        ".clone()",
    ] {
        assert!(
            !native.contains(recursive),
            "source admission reentered/copied {recursive}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_call_finish.rs"))
            .unwrap();
    for claim in [
        "exact_original_operands_schema_and_move_only_outputs_reach_once_only_admission",
        "complete_cold_forest_uses_one_current_root_node_and_depth_budget",
        "fresh_lexical_admission_rejects_forged_prepared_scalar_facts",
        "original_schema_borrow_does_not_freeze_live_native_domains_version_or_grants",
        "native_admission_unwind_releases_whole_owned_output_without_refunding_external_work",
        "unrelated_numeric_and_panel_hosts_complete_original_calls_and_prepare_actual_values",
        "reference_computed_call_keeps_full_width_group_scope_and_canonical_wire_policy",
    ] {
        assert!(proof.contains(claim), "missing native call proof: {claim}");
    }
}

#[test]
fn pure_type_inference_uses_shared_ir_and_native_queries_not_reference_product_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "pure inference leaked {product}");
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait PureTypeEnvironment<Field, Operation>"));
    assert!(source.contains("pub fn infer_pure_type"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let canonical_gate = reference.find("if roundtrip != *self").unwrap();
    let native_check = reference
        .find("crate::pure_reference::infer(self, scope, groups)")
        .unwrap();
    assert!(canonical_gate < native_check);
    assert!(reference.contains("crate::pure_typing::valid_local_name(name)"));
}

#[test]
fn atomic_call_inference_uses_native_catalogs_and_shared_types_not_product_commands() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/call_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "atomic call inference leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::{Computation, ComputedArgument};"));
    assert!(source.contains("pub fn infer_call_type"));
    assert!(source.contains("pub fn check_call_arguments"));
    assert!(source.contains("OperationCatalog"));
    assert!(source.contains("infer_in_scope"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *self").unwrap()
            < reference.find("crate::pure_reference::infer_call").unwrap()
    );
}

#[test]
fn prepared_call_typing_keeps_one_ir_and_preflights_cold_schemas_before_type_metadata() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/prepared_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "prepared call inference leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub fn infer_prepared_call_type"));
    assert!(source.contains("pub trait PreparedCallSchemas"));
    assert!(source.contains("std::ptr::eq(expected, schema)"));
    assert!(source.find(".select(operation)").unwrap() < source.find("bindings.to_vec()").unwrap());
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let canonical = reference.find("if roundtrip != *self").unwrap();
    let prepared = reference
        .find("crate::pure_reference::infer_prepared")
        .unwrap();
    assert!(canonical < prepared);
    assert!(reference.contains("crate::prepared_typing::call_leaves_only(self)"));
}

#[test]
fn call_result_dataflow_uses_explicit_native_mapping_after_all_cold_schemas() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/flow_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "call flow typing leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait CallFlowEnvironment"));
    assert!(source.contains("pub fn infer_call_flow_type"));
    assert!(source.contains("check_in_scope"));
    assert!(source.contains("infer_in_scope"));
    assert!(source.find(".select(operation)").unwrap() < source.find("bindings.to_vec()").unwrap());
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *self").unwrap()
            < reference.find("crate::pure_reference::infer_flow").unwrap()
    );
    assert!(reference.contains("crate::flow_typing::call_flow_nodes_only(self)"));
    assert!(reference.contains("if !prepared_call"));
}

#[test]
fn group_dataflow_keeps_one_engine_and_checks_cold_rows_before_native_exports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/flow_typing.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "group dataflow leaked {product}");
    }
    assert!(source.contains("pub trait GroupFlowEnvironment"));
    assert!(source.contains("pub fn infer_group_flow_type"));
    assert!(source.contains("pub struct GroupMemberType"));
    assert!(source.contains("preflight_call_leaves_with_budget"));
    assert!(source.contains("std::ptr::eq(row, schema)"));
    assert!(
        source
            .find("for (group_index, group) in physical.groups")
            .unwrap()
            < source.find("bindings.to_vec()").unwrap()
    );
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(
        reference.find("if roundtrip != *self").unwrap()
            < reference
                .find("crate::pure_reference::infer_group_flow")
                .unwrap()
    );
    assert!(reference.contains("crate::flow_typing::group_flow_nodes_only(self)"));
}

#[test]
fn opaque_host_dataflow_uses_the_same_engine_with_explicit_cold_admission_and_borrowed_types() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/flow_typing.rs")).unwrap();
    for marker in [
        "pub trait HostFlowEnvironment<'expression",
        "pub struct HostFlowTypeLimits",
        "pub fn infer_host_flow_type",
        "fn host_result_type(&self, effect: &'expression HostEffect)",
        "fn infer_with_policies",
        "host_limit: Some(limits.max_hosts)",
        "HostAdmission { host_index }",
        "HostResultType { host_index }",
        "host_limit: None",
        "hosts: &NoHosts",
    ] {
        assert!(
            flow.contains(marker),
            "missing opaque host boundary {marker}"
        );
    }
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "serde::",
        "evaluate_",
        "canonical_source(",
    ] {
        assert!(!flow.contains(product), "opaque flow leaked {product}");
    }
    let assembly = flow.split_once("fn infer_with_policies").unwrap().1;
    let physical = assembly.find("let physical = preflight(").unwrap();
    let calls = assembly.find(".select(operation)").unwrap();
    let admission = assembly.find("policies.hosts.admit(effect)").unwrap();
    let clone = assembly.find("bindings.to_vec()").unwrap();
    assert!(physical < calls && calls < admission && admission < clone);
    let product =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(
        product.find("if roundtrip != *self").unwrap()
            < product
                .find("crate::pure_reference::infer_host_flow")
                .unwrap()
    );
    assert!(product.contains("crate::flow_typing::host_flow_nodes_only(self"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_reference.rs")).unwrap();
    assert!(adapter.contains("infer_host_flow_type("));
    let native = adapter
        .split_once("fn admit_host")
        .unwrap()
        .1
        .split_once("impl<'a> PureTypeEnvironment")
        .unwrap()
        .0;
    assert!(native.contains("HostOperation::for_effect(effect)"));
    assert!(!native.contains("validate_in_type_scope") && !native.contains("canonical_source("));
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_flow_typing.rs"))
        .unwrap();
    for boundary in [
        "cold_native_admission_precedes_prefix_clone_and_guard_or_field_queries",
        "all_cold_call_policy_checks_precede_opaque_admission_and_semantic_queries",
        "hidden_native_graph_foreign_owner_schema_version_and_grants_need_explicit_admission",
        "guards_call_arguments_and_projection_loop_fold_recovery_operands_stay_pure_even_cold",
        "native_hook_unwind_drops_temporary_scope_without_prefix_mutation_or_retry",
        "result_metadata_can_borrow_the_original_nonclone_opaque_declaration",
        "unrelated_gui_host_uses_nonclone_slots_and_original_owned_declaration_identity",
        "exact_large_prefix_ceiling_and_active_growth_are_not_residual_small_product_limits",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_flow_reference.rs"))
            .unwrap();
    for boundary in [
        "mixed_native_host_and_computed_call_chains_keep_native_types_wire_and_authority",
        "native_graph_host_wrappers_and_new_groups_keep_their_existing_source_policy",
        "native_raw_receipt_and_effectful_operand_restrictions_do_not_become_typing_permissions",
        "forged_native_payload_and_literal_call_nodes_are_still_rejected_by_canonical_admission",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn leselang_runtime_core_has_no_workspace_or_build_dependency_even_in_its_tests() {
    assert_workspace_closure("leselang-runtime-core", &[]);
    let metadata = workspace_metadata();
    let core = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "leselang-runtime-core")
        .expect("standalone runtime foundation must exist");
    let actual: BTreeSet<_> = core["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|dependency| {
            assert!(
                dependency["path"].is_null(),
                "core must not need workspace paths"
            );
            (
                dependency["name"].as_str().unwrap().to_string(),
                dependency["kind"].as_str().unwrap_or("normal").to_string(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            ("serde".to_string(), "normal".to_string()),
            ("serde_json".to_string(), "dev".to_string())
        ]
        .into()
    );
    assert!(!core["targets"].as_array().unwrap().iter().any(|target| {
        target["kind"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "custom-build")
    }));

    let workflow = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"),
    )
    .unwrap();
    let standalone_job = workflow
        .split_once("  leselang-core:\n")
        .unwrap()
        .1
        .split_once("  rust:\n")
        .unwrap()
        .0;
    assert!(standalone_job.contains("cargo +1.98.0 package -p leselang-runtime-core --locked"));
    assert!(standalone_job.contains("cargo +1.98.0 test --manifest-path target/package/leselang-runtime-core-*/Cargo.toml --locked --offline"));
    assert!(workflow.contains("needs: [rust, product-surfaces, leselang-core]"));
}

#[test]
fn mixed_host_groups_share_atomic_walk_typing_and_exact_native_declaration_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/flow_typing.rs")).unwrap();
    for marker in [
        "pub trait HostGroupFlowEnvironment",
        "pub struct HostGroupFlowTypeLimits",
        "pub fn infer_host_group_flow_type",
        "preflight_atomic_leaves_with_budget",
        "group_hosts: &NoGroupHosts",
        "std::ptr::eq(row, schema)",
    ] {
        assert!(
            flow.contains(marker),
            "missing mixed group boundary {marker}"
        );
    }
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "canonical_source(",
        "validate_in_type_scope",
        "evaluate_",
        "serde::",
    ] {
        assert!(
            !flow.contains(coupling),
            "mixed group typing leaked {coupling}"
        );
    }
    let assembly = flow.split_once("fn infer_with_policies").unwrap().1;
    assert!(
        assembly.find("policies.hosts.admit(effect)").unwrap()
            < assembly
                .find("policies.group_hosts.operation(effect)")
                .unwrap()
    );
    assert!(
        assembly
            .find(".admit(effect, operation, &schema.result)")
            .unwrap()
            < assembly.find("bindings.to_vec()").unwrap()
    );
    assert!(
        assembly.find("std::ptr::eq(row, schema)").unwrap()
            < assembly.find("bindings.to_vec()").unwrap()
    );
    let prepared =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/prepared_typing.rs")).unwrap();
    assert!(prepared.contains(
        "preflight_atomic_leaves_with_budget(expression, depth, budget, max_arguments, false)"
    ));
    assert!(prepared.contains("Computation::Host { .. } if allow_hosts"));
    assert!(prepared.contains("call_index: call_count"));
    let product =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(
        product.find("if roundtrip != *self").unwrap()
            < product
                .find("crate::pure_reference::infer_host_group_flow")
                .unwrap()
    );
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_reference.rs")).unwrap();
    assert!(adapter.contains("infer_host_group_flow_type("));
    assert!(adapter.contains("std::ptr::eq(declaration, &operation.schema().result)"));
    assert!(adapter.contains("std::ptr::eq(operation, &operation.schema().result.operation)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_group_typing.rs"))
            .unwrap();
    for boundary in [
        "mixed_sequence_parallel_exports_borrow_original_names_and_host_call_operations",
        "pure_preparation_and_conditional_host_call_paths_require_one_original_schema_row",
        "native_pair_admission_rejects_equal_lookalike_keys_foreign_results_and_missing_mapping",
        "group_member_pure_initializers_and_cold_call_positions_do_not_count_host_leaves_as_calls",
        "native_mapping_pair_declaration_and_construction_unwind_never_publish_or_retry_members",
        "unrelated_gui_profile_borrows_nonclone_operation_effect_and_result_declarations",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_group_reference.rs"))
            .unwrap();
    assert!(
        proof.contains(
            "mixed_flat_sequence_parallel_groups_keep_product_types_wire_and_capabilities"
        )
    );
    assert!(proof.contains(
        "forged_group_result_operation_and_native_payload_are_rejected_before_shared_type_exports"
    ));
}

#[test]
fn flat_native_graph_types_reuse_shared_protocols_and_original_closed_member_observations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_reference.rs")).unwrap();
    for marker in [
        "pub(crate) fn flat_native_group(",
        "pub(crate) fn supports_host_flow(",
        "operation.schema().result.ty != branch.result_type",
        "branch.name.as_str()",
        "members: Some(ReferenceMembers::Computed { kind, members })",
        "native_group_type_observations_borrow_names_and_exact_canonical_operation_rows",
        "native_and_computed_group_joins_ignore_payloads_but_never_union_exports",
    ] {
        assert!(
            adapter.contains(marker),
            "missing flat native graph boundary {marker}"
        );
    }
    assert!(
        adapter
            .split_whitespace()
            .collect::<String>()
            .contains("&HostOperation::for_effect(&branch.effect)?.schema().result.operation")
    );
    let product =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(product.contains("crate::pure_reference::flat_native_group(effect)"));
    assert_eq!(
        product
            .matches("crate::pure_reference::supports_host_flow")
            .count(),
        2
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_group_flow.rs"))
            .unwrap();
    for marker in [
        "independent_gui_graph_exports_borrow_original_names_operations_and_nonclone_declarations",
        "native_and_computed_gui_groups_join_only_the_same_original_closed_signature",
        "every_cold_native_graph_admission_precedes_prefix_clones_and_member_queries",
        "host_language_limits_do_not_replace_inclusive_private_graph_limits_or_live_policy",
        "changed_native_graph_metadata_needs_fresh_admission_without_a_cached_certificate",
        "nested_opaque_graphs_are_not_accepted_as_atomic_computed_members",
        "native_graph_hook_unwind_does_not_retry_publish_or_mutate_the_borrowed_prefix",
        "call_policy_and_impure_cold_guards_precede_all_opaque_graph_queries",
    ] {
        assert!(proof.contains(marker));
    }
    for coupling in [
        "leserpent",
        "HostOperation",
        "Effect::",
        "canonical_source",
        "serde::",
        "tokio",
    ] {
        assert!(
            !proof.contains(coupling),
            "independent graph proof leaked {coupling}"
        );
    }
    let proof = std::fs::read_to_string(
        root.join("crates/leselang-hir/tests/native_group_flow_reference.rs"),
    )
    .unwrap();
    for marker in [
        "flat_native_group_captures_and_result_driven_successors_keep_wire_and_authority",
        "native_and_computed_group_choices_share_one_closed_ordered_signature",
        "native_group_aliases_pure_selection_and_helpers_keep_closed_exports",
        "mode_order_names_and_operation_mismatches_do_not_union_native_and_computed_exports",
        "cold_forged_native_graph_metadata_and_payloads_fail_canonical_admission",
        "an_opaque_group_cannot_masquerade_as_an_atomic_computed_member",
    ] {
        assert!(proof.contains(marker));
    }
}

#[test]
fn actual_result_validation_is_product_free_and_shared_by_reference_raw_value_binding() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/host_result.rs"))
            .unwrap();
    for product in [
        "leselang_hir",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "host-result validation leaked {product}"
        );
    }
    assert!(source.contains("pub trait HostResultDomain<Reply: ?Sized>"));
    assert!(source.contains("pub fn validate_host_result"));
    assert!(
        source.find("if !domain.matches_type(reply)").unwrap()
            < source.find(".validate_value(reply)").unwrap()
    );
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-vm/src/host_result.rs")).unwrap();
    assert!(reference.contains("impl HostResultDomain<Value> for BoundResultDomain"));
    assert!(reference.contains("PendingReply::new(image.token.as_str(), image, &declaration)"));
    assert!(reference.contains(".try_accept(&image.token.as_str(), value, &ResultObservation)"));
    assert!(
        reference.contains(
            "ReplyAcceptanceError::Result(HostResultError::InvalidValue(error)) => error"
        )
    );
    assert!(reference.contains("crate::group_binding::value_type(reply)"));
    let vm = std::fs::read_to_string(root.join("crates/leselang-vm/src/lib.rs")).unwrap();
    assert!(vm.contains("use host_result::{accept_bound_value, validate_bound_value};"));
    assert!(vm.contains("match accept_bound_value(image, &value)"));
    assert!(!vm.contains("fn validate_bound_value("));
}

#[test]
fn pending_reply_ownership_is_product_free_and_closes_before_native_callbacks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-runtime-core/src/reply.rs")).unwrap();
    for product in [
        "leselang_hir",
        "ContinuationImage",
        "leserpent",
        "sqlite",
        "serde",
    ] {
        // Documentation names serde only to reject the requirement; no import/derive is allowed.
        if product == "serde" {
            assert!(!source.contains("use serde"));
            assert!(!source.contains("Serialize, Deserialize"));
        } else {
            assert!(
                !source.contains(product),
                "reply ownership leaked {product}"
            );
        }
    }
    assert!(source.contains("pub trait ReplyAuthority<Identity>"));
    assert!(source.contains("Reply: Borrow<View>"));
    let attempt = source.split_once("pub fn try_accept").unwrap().1;
    let close = attempt
        .find("State::Terminal(ReplyEnd::HostUncertain)")
        .unwrap();
    let identity = attempt
        .find("&waiting.identity != actual_identity")
        .unwrap();
    let authority = attempt
        .find("authority.authorize(&waiting.identity)")
        .unwrap();
    let value = attempt
        .find("validate_host_result(waiting.declaration, reply.borrow())")
        .unwrap();
    assert!(close < identity && identity < authority && authority < value);
    assert!(attempt.contains("self.state = State::Waiting(waiting)"));
    assert!(
        attempt
            .find("self.state = State::Terminal(ReplyEnd::Accepted)")
            .unwrap()
            < attempt.find("Ok(AcceptedReply {").unwrap()
    );
    let proofs =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
            .unwrap();
    assert!(proofs.contains("PendingReply::new((7, 1), capture"));
    assert!(proofs.contains("PendingReply::new(1u64, capture"));
    assert!(proofs.contains("ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)"));
}

#[test]
fn shared_pure_execution_is_product_free_and_used_by_the_reference_vm() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/pure_evaluation.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "shared pure execution leaked {product}"
        );
    }
    assert!(source.contains("use crate::ir::Computation;"));
    assert!(source.contains("pub trait PureEvaluationEnvironment<Field, Operation>"));
    assert!(source.contains("pub fn evaluate_pure_in_scope"));
    assert!(source.contains("preflight_with_budget(expression, 0, &mut budget)"));
    assert!(source.contains("Err(failure) if failure.is_recoverable()"));
    assert!(!source.contains("pub enum Computation"));
    let vm = std::fs::read_to_string(root.join("crates/leselang-vm/src/computation.rs")).unwrap();
    assert!(vm.contains("type LocalValue<'a> = PureValue<ResultView<'a>>"));
    assert!(vm.contains("evaluate_effects_in_scope("));
    let control =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/effect_evaluation.rs")).unwrap();
    assert!(control.contains("evaluate_preflighted_in_scope("));
    assert!(vm.contains("PureEvaluationFault::Native(fault) => fault"));
    for duplicated in [
        "LoopBudget::new",
        "FoldCursor::new",
        "StringListBuilder::with_capacity",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(
            !vm.contains(duplicated),
            "reference VM duplicated {duplicated}"
        );
    }
}

#[test]
fn shared_call_preparation_connects_native_catalogs_and_values_to_the_reference_vm() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/call_evaluation.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(
            !source.contains(product),
            "call preparation leaked {product}"
        );
    }
    for shared in [
        "use crate::ir::{Computation, ComputedArgument};",
        "pub fn prepare_call_in_scope",
        "pub fn evaluate_call_arguments_in_scope",
        "pub struct PreparedCall",
        "preflight_value_scope",
        "preflight_arguments_with_budget",
        "evaluate_preflighted_in_scope",
        "check_argument_type",
    ] {
        assert!(
            source.contains(shared),
            "missing shared call boundary {shared}"
        );
    }
    assert!(
        source
            .find("let names = preflight(arguments, scope, limits)?;")
            .unwrap()
            < source.find("evaluate_arguments(arguments, &names").unwrap()
    );
    assert!(!source.contains("bindings.to_vec()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/host_call.rs")).unwrap();
    assert!(reference.contains("pub fn evaluate_computed_arguments"));
    assert!(reference.contains("crate::call_evaluation::evaluate_call_arguments_in_scope("));
    let vm = std::fs::read_to_string(root.join("crates/leselang-vm/src/computation.rs")).unwrap();
    assert!(vm.contains(".evaluate_computed_arguments("));
    assert!(!vm.contains("for argument in arguments"));
    assert!(
        vm.contains("CallEvaluationError::Evaluation { failure, .. } => pure_failure(failure)")
    );
    assert!(vm.contains("operation.resolve(&values)"));
}

#[test]
fn shared_effect_control_drives_reference_vm_with_explicit_non_dispatching_capture_hooks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/effect_evaluation.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "effect control leaked {product}");
    }
    for boundary in [
        "pub trait EffectEvaluationEnvironment",
        "pub enum EffectEvaluationOutcome",
        "pub fn evaluate_effects_in_scope",
        "fn preflight_effect(",
        "fn prepare_effect(",
        "fn capture(",
        "let mut local = scope.nested();",
        "evaluate_preflighted_in_scope(",
        "EffectEvaluationFault::NestedSuspension",
    ] {
        assert!(
            source.contains(boundary),
            "missing effect control boundary {boundary}"
        );
    }
    assert!(
        source
            .find("let effects = preflight(expression, scope, limits)?;")
            .unwrap()
            < source.find(".preflight_effect(effect)").unwrap()
    );
    assert!(!source.contains("pub enum Computation"));
    assert!(!source.contains("bindings.to_vec()"));
    let vm = std::fs::read_to_string(root.join("crates/leselang-vm/src/computation.rs")).unwrap();
    assert!(vm.contains("evaluate_effects_in_scope("));
    assert!(vm.contains("type Capture = Box<ResultBinding>;"));
    assert!(vm.contains("charge_group_projection(fuel, &group)?"));
    assert!(!vm.contains("Computation::Bind { name, value, body } =>"));
    assert!(!vm.contains("Err(failure) if failure.is_recoverable()"));
}

#[test]
fn shared_scalar_source_construction_is_product_free_and_delegated_by_reference_compiler() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/scalar_source.rs")).unwrap();
    for product in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "leserpent",
        "sqlite",
    ] {
        assert!(!source.contains(product), "scalar source leaked {product}");
    }
    for boundary in [
        "pub fn lower_scalar_source",
        "pub(crate) fn lower_form",
        "pub(crate) fn lower_choose_form",
        "StringListBuilder::with_capacity",
        ".try_push(text)",
        "check_string_label",
        "preflight(expression, limits)",
        "preflight_names(expression)",
        "preflight_with_budget(&value",
        "BinaryOperator::parse",
        "UnaryOperator::parse",
        "ScalarSourceError::InconsistentLiteral",
        "type OperandResult",
        "if depth > self.max_depth",
    ] {
        assert!(
            source.contains(boundary),
            "missing scalar source boundary {boundary}"
        );
    }
    let public = source.split_once("pub fn lower_scalar_source").unwrap().1;
    assert!(
        public.find("preflight(expression, limits)").unwrap()
            < public.find("preflight_names(expression)").unwrap()
    );
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public.find("let output = Builder").unwrap()
    );
    assert!(!source.contains("apply_binary("));
    assert!(!source.contains("apply_unary("));
    assert!(!source.contains("Some(text.clone())"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::scalar_source::primitive(expression)"));
    assert!(reference.contains("crate::scalar_source::lower_form(expression"));
    assert!(reference.contains("crate::choice_source::lower_choice_source("));
    assert!(reference.contains("crate::binding_source::lower_binding_source("));
    assert!(reference.contains("source.construct(value, body)"));
    assert!(!reference.contains("if callee == \"recover\""));
    assert!(!reference.contains("if callee == \"strings\""));
    assert!(!reference.contains("fn unary_type("));
    assert!(!reference.contains("if let Some(operator) = BinaryOperator::parse(callee)"));
    assert!(!reference.contains("Some(text.clone())"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_call.rs")).unwrap();
    assert!(proof.contains("leselang_hir::scalar_source::lower_scalar_source"));
    assert!(!proof.contains("fn node(expression: &Expression)"));
}

#[test]
fn shared_scalar_source_bindings_keep_explicit_policy_and_read_only_native_scope() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/scalar_source.rs")).unwrap();
    for boundary in [
        "pub struct ScalarSourceLimits",
        "pub max_bindings: usize",
        "pub struct ScalarSourceScope",
        "pub fn get(&self, name: &str) -> Option<ScalarType>",
        "max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "pub fn lower_scalar_source_with_scope",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        "let mut scope = self.scope.nested()",
        "source.construct(value.value, body.value)",
        "max_bindings: None",
        "max_bindings: Some(limits.max_bindings)",
    ] {
        assert!(
            source.contains(boundary),
            "missing lexical boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_scalar_source_with_scope")
        .unwrap()
        .1;
    for boundary in [
        "preflight(expression, limits.source)",
        "preflight_names(expression)",
        "PureTypeError::InvalidScope",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
    ] {
        assert!(public.find(boundary).unwrap() < public.find("let output = Builder").unwrap());
    }
    let scope = source
        .split_once("pub struct ScalarSourceScope")
        .unwrap()
        .1
        .split_once("pub(crate) struct BindingSource")
        .unwrap()
        .0;
    assert!(!scope.contains("pub bindings"));
    assert!(!scope.contains("pub fn push"));
    assert!(!scope.contains("pub fn pop"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_binding_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "lower_source_call",
        "evaluate_call_arguments_in_scope",
        "catch_unwind",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_native_binding_source_keeps_explicit_stages_bounds_and_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/binding_source.rs")).unwrap();
    for boundary in [
        "pub struct BindingSourceLimits",
        "pub enum BindingSourcePhase",
        "pub enum BindingSourceError<Error>",
        "pub struct BindingSourceForm",
        "pub trait BindingSourceAdapter",
        "type Value;",
        "type Result;",
        "type Error;",
        "metadata: &mut Self::Value",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        "physical(&value, 1, &mut budget)",
        "budget.check_pending(0, 1)",
        "physical(&body, 1, &mut budget)",
        "physical(&output, 0, &mut budget)",
        "max_lowered_depth == 0",
    ] {
        assert!(
            source.contains(boundary),
            "missing binding boundary {boundary}"
        );
    }
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Default",
        "std::sync",
        "Clone +",
        "Debug +",
    ] {
        assert!(!source.contains(forbidden), "native coupling {forbidden}");
    }
    let public = source.split_once("pub fn lower_binding_source").unwrap().1;
    let sequence = [
        "preflight(expression, limits.source)",
        "binding_source(expression)",
        "preflight_bindings(expression, &mut scope, limits.max_bindings)",
        ".lower_value(source.binding())",
        "physical(&value, 1, &mut budget)",
        ".lower_body(&source, &value, &mut metadata)",
        "physical(&body, 1, &mut budget)",
        ".finish(&source, value, metadata, body, result)",
        "physical(&output, 0, &mut budget)",
    ];
    for pair in sequence.windows(2) {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    for boundary in [
        "crate::binding_source::lower_binding_source(",
        "struct BindingAdapter",
        "let mut local = self.scope.nested()",
        "metadata.members.take()",
        "metadata.group_member_count.is_none_or",
        "source.construct(value, body)",
        "source.span()",
        "crate::function_flow::bind_result(",
        "computation exceeds its node or nesting limit",
    ] {
        assert!(
            reference.contains(boundary),
            "missing reference binding policy {boundary}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    for boundary in [
        "original_source_metadata_native_buffers_and_phase_order_are_preserved",
        "generated_value_budget_reserves_body_and_stops_body_hook",
        "generated_body_uses_same_aggregate_budget_before_finish_hook",
        "rewritten_finish_output_is_rechecked_without_partial_result",
        "shifted_body_and_rewritten_output_depth_are_checked_at_their_own_phases",
        "whole_cold_source_nodes_and_depth_fail_before_value_lowering",
        "native_unwind_releases_owned_parts_and_guarded_frames_once",
        "all_four_move_only_ir_slots_child_boxes_and_vectors_move_without_clone",
        "parsed_native_result_binding_uses_original_schemas_received_values_and_exact_fuel",
        "native_versions_grants_and_undefined_initializer_names_stop_before_later_phases",
        "reference_pure_atomic_group_and_helper_bindings_keep_canonical_wire_and_authority",
        "reference_cold_type_shadow_group_export_and_capture_flow_fail_closed",
        "check_result(&reply)",
        "prepare_call_in_scope",
        "catch_unwind",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary), "missing binding proof {boundary}");
    }
}

#[test]
fn shared_native_choice_source_requires_explicit_queries_after_cold_bounds() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/choice_source.rs")).unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
    ] {
        assert!(!source.contains(forbidden), "choice coupling {forbidden}");
    }
    let public = source.split_once("pub fn lower_choice_source").unwrap().1;
    for pair in [
        "preflight(expression, limits)",
        "cold_names(expression)",
        "child(ChoiceSourcePhase::When",
        "preflight_with_budget(&when",
        "boolean(&when, &when_type)",
        "child(ChoiceSourcePhase::Then",
        "physical(&then, 1, &mut budget)",
        "child(ChoiceSourcePhase::Otherwise",
        "physical(&otherwise, 1, &mut budget)",
        "same(&then_type, &otherwise_type)",
    ]
    .windows(2)
    {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    assert!(source.contains("!matches!(value, ScalarValue::Boolean(_))"));
    assert!(source.contains("drop(when_type)"));
    assert!(source.contains(".check_pending(0, 3)"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::choice_source::lower_choice_source("));
    assert!(!reference.contains("crate::scalar_source::lower_choose_form("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/choice_source.rs")).unwrap();
    for boundary in [
        "all_four_native_slots_and_original_boxes_and_vector_buffers_move_unchanged",
        "every_native_unwind_releases_owned_parts_and_guarded_frames_without_retry",
        "forged_boolean_metadata_cannot_accept_an_integer_literal",
        "std::ptr::eq",
        "catch_unwind",
        "parsed_native_choice_checks_both_schemas_but_prepares_only_selected_call_with_exact_fuel",
        "native_choice_never_confers_future_version_or_cold_branch_grants",
        "evaluate_effects_in_scope",
        "check_result(&ScalarValue::Integer(101))",
    ] {
        assert!(proof.contains(boundary), "missing choice proof {boundary}");
    }
}

#[test]
fn shared_flat_group_source_keeps_cold_output_before_explicit_native_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/group_source.rs")).unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
    ] {
        assert!(
            !source.contains(forbidden),
            "group source coupling {forbidden}"
        );
    }
    let public = source
        .split_once("pub fn lower_flat_group_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, limits.max_branches)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_branches)",
        "lower(index, argument)",
        "physical(&value, 1, &mut budget)",
        "atomic_candidate(&value)",
        "branches.push(ComputedBranch",
        "admit(index, argument, branch)",
    ]
    .windows(2)
    {
        assert!(public.find(pair[0]).unwrap() < public.find(pair[1]).unwrap());
    }
    assert!(source.contains("arguments.len() - index - 1"));
    assert!(source.contains("argument.name.to_owned()"));
    assert!(!source.contains(".clone()"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computed_group.rs")).unwrap();
    for boundary in [
        "lower_flat_group_source(",
        "operation.result_type() == branch.result_type",
        "lower_repeat_sequence_source(",
        "lower_sequence_source(",
        "lower_all_with",
        "branch.name",
        "effect: *effect",
        "GroupSourceError::DuplicateMember { kind, span: at, .. }",
        "phase: GroupSourcePhase::Lower",
        "arguments.iter().skip(index + 1)",
        "only collect bounded sibling diagnostics",
    ] {
        assert!(
            reference.contains(boundary),
            "missing group reference policy {boundary}"
        );
    }
    assert!(!reference.contains("arguments[index]"));
    let diagnostics =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scope_preflight.rs")).unwrap();
    assert!(
        diagnostics
            .contains("failed_parallel_lowering_collects_only_remaining_sibling_diagnostics")
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    for boundary in [
        "original_named_arguments_move_only_results_and_two_stage_order_are_preserved",
        "aggregate_later_output_is_bounded_before_any_admission_hook",
        "matching_result_observations_cannot_replace_uniform_atomic_operation_admission",
        "native_unwind_releases_all_consumed_parts_and_scopes_once_without_retry",
        "reference_nested_cold_error_indices_never_index_the_root_member_array",
        "parsed_native_groups_preserve_original_rows_prepare_real_values_and_exact_fuel_without_dispatch",
        "missing_later_grants_stale_versions_and_wrong_rows_fail_before_value_preparation",
        "evaluate_effects_in_scope",
        "check_result(&ScalarValue::Integer(101))",
        "std::ptr::eq",
        "catch_unwind",
    ] {
        assert!(
            proof.contains(boundary),
            "missing flat group proof {boundary}"
        );
    }
}

#[test]
fn shared_flat_repeat_source_reserves_complete_output_before_explicit_native_factories() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/repeat_source.rs")).unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "PartialEq +",
        "Clone +",
        "Debug +",
        "Send +",
        ".clone()",
    ] {
        assert!(
            !source.contains(forbidden),
            "repeat source coupling {forbidden}"
        );
    }
    let entry = source
        .split_once("pub fn lower_flat_repeat_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, limits.max_repetitions)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_repetitions)",
        ".lower_body(body)",
        "let expected = shape(&value",
        "let cost = measure_source_cost",
        "cost.nodes,",
        "Vec::with_capacity(count)",
        ".materialize(iteration, body, &branches[0])",
        "actual != expected",
        "actual != cost",
        ".admit(index + 1, body, &branches[0], branch)",
    ]
    .windows(2)
    {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for boundary in [
        "checked_mul(count)",
        "checked_add(1)",
        "for iteration in 2..=count",
        "atomic_candidate(&value)",
        "name: \"iteration_1\".into()",
        "format!(\"iteration_{iteration}\")",
        "No native Clone/PartialEq/Debug/serde/Send",
    ] {
        assert!(
            source.contains(boundary),
            "missing repeat boundary {boundary}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computed_group.rs")).unwrap();
    for boundary in [
        "lower_flat_repeat_source(",
        "struct RepeatAdapter",
        "original.value.clone()",
        "candidate.result_type == original.result_type",
        "candidate.value.prepared_atomic_operation() == operation",
        "functions::host_source_extra",
        "finish_flat_group(value, span)",
        "lower_repeat_sequence_source(",
    ] {
        assert!(
            reference.contains(boundary),
            "missing repeat reference policy {boundary}"
        );
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/repeat_source.rs")).unwrap();
    for boundary in [
        "original_body_result_buffers_and_once_factory_admission_order_are_preserved",
        "complete_shifted_physical_reservation_precedes_every_factory",
        "folded_constructor_and_opaque_source_costs_are_reserved_before_factories",
        "equal_node_count_with_changed_depth_cannot_reuse_the_reserved_shape",
        "same_sized_native_result_observations_do_not_become_declarations",
        "native_unwind_drops_all_consumed_instances_and_restores_scopes_once",
        "late_native_cost_failure_and_unwind_drop_every_instance_without_admission",
        "parsed_native_repeat_prepares_original_schema_values_and_exact_fuel_without_dispatch",
        "wrong_copy_declarations_changed_values_and_stale_live_policy_cannot_return_partial_requests",
        "evaluate_effects_in_scope",
        "std::ptr::eq",
        "check_result(&ScalarValue::Integer(101))",
    ] {
        assert!(proof.contains(boundary), "missing repeat proof {boundary}");
    }
}

#[test]
fn shared_owned_sequence_source_moves_native_children_without_effect_round_trips() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/sequence_source.rs")).unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Clone +",
        "PartialEq +",
        "Debug +",
        "Send +",
        ".clone()",
    ] {
        assert!(
            !source.contains(forbidden),
            "sequence source coupling {forbidden}"
        );
    }
    let entry = source.split_once("pub fn lower_sequence_source").unwrap().1;
    for pair in [
        "header(expression, limits.max_branches)",
        "preflight(expression, limits.source)",
        "cold_headers(expression, limits.max_branches)",
        "lower(index, argument)",
        "members.is_empty()",
        "MAX_LOCAL_NAME_BYTES",
        "physical(&branch.value, 1, &mut budget)",
        "atomic_candidate(&branch.value)",
        "admit(index, member, argument, branch)",
    ]
    .windows(2)
    {
        assert!(entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap());
    }
    for boundary in [
        "SequenceSourceMember::Sequence { branches } if nested",
        "branches.push(branch)",
        "origins.push((index, member))",
        "previous.name == branch.name",
        "arguments.len() - index - 1",
        "never formatted or exposed in source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing sequence boundary {boundary}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computed_group.rs")).unwrap();
    let sequence = reference
        .split_once("fn lower_sequence_nested")
        .unwrap()
        .1
        .split_once("fn lower_flat")
        .unwrap()
        .0;
    assert!(sequence.contains("lower_sequence_source("));
    assert!(!sequence.contains("into_effect("));
    assert!(!sequence.contains(".clone()"));
    assert!(!sequence.contains("control_flow::lower_sequence_with"));
    assert!(sequence.contains("Effect::Sequence { steps }"));
    assert!(sequence.contains("operation.result_type() == branch.result_type"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/sequence_source.rs")).unwrap();
    for boundary in [
        "original_rows_order_labels_and_move_only_results_survive_flattening",
        "whole_shifted_output_reserves_future_roots_before_later_lowering",
        "joined_names_are_bounded_before_allocation_and_collisions_before_admission",
        "native_unwind_drops_all_consumed_rows_once_and_never_retries",
        "reference_nested_sequences_keep_canonical_wire_authority_and_diagnostics",
    ] {
        assert!(
            proof.contains(boundary),
            "missing owned sequence proof {boundary}"
        );
    }
    let host =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    assert!(host.contains(
        "parsed_nested_sequence_preserves_two_native_schemas_values_fuel_and_live_rejection"
    ));
    assert!(host.contains("assert_eq!(fuel.remaining(), 93)"));
    assert!(host.contains("compile_sequence(&argument.value, host)"));
}

#[test]
fn shared_repeat_sequence_source_reserves_whole_forests_before_native_factories_and_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/repeat_sequence_source.rs"))
            .unwrap();
    for forbidden in [
        "leselang_host_contract",
        "leserpent",
        "rusqlite",
        "use serde",
        "serde::",
        "Clone +",
        "PartialEq +",
        "Debug +",
        "Send +",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
    ] {
        assert!(
            !source.contains(forbidden),
            "repeat-sequence coupling {forbidden}"
        );
    }
    let entry = source
        .split_once("pub fn lower_repeat_sequence_source")
        .unwrap()
        .1;
    for pair in [
        "header(expression, bounds.max_repetitions)",
        "preflight(expression, bounds.source)",
        "cold_headers(expression, bounds.max_repetitions)",
        "group_headers(expression, limits.max_branches)",
        ".lower_sequence(body)",
        "let expanded_width = width",
        "expected.nodes - 1",
        "MAX_LOCAL_NAME_BYTES",
        "let cost = measure_source_cost",
        "cost.nodes - 1",
        "Vec::with_capacity(count)",
        ".materialize_sequence(iteration, body, original)",
        "actual != expected",
        "branch.name != original.name",
        "actual != cost",
        ".admit_member(index + 1, member, body, original, candidate)",
        "Vec::with_capacity(expanded_width)",
    ]
    .windows(2)
    {
        assert!(
            entry.find(pair[0]).unwrap() < entry.find(pair[1]).unwrap(),
            "incorrect repeat-sequence order {pair:?}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computed_group.rs")).unwrap();
    assert!(reference.contains("lower_repeat_sequence_source("));
    assert!(reference.contains("Ok(original.to_vec())"));
    assert!(reference.contains("sequence_branches(value, body.span)"));
    assert!(!reference.contains("control_flow::lower_repeat_with"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/repeat_sequence_source.rs"))
            .unwrap();
    for boundary in [
        "original_template_rows_move_once_and_factories_precede_all_ordered_admission",
        "all_four_native_slots_boxes_and_operand_buffers_require_no_clone_debug_serde_or_send",
        "flattened_physical_reservation_does_not_recharge_removed_group_roots",
        "opaque_source_reservation_is_complete_before_any_instance_factory",
        "changed_width_labels_shape_cost_or_candidate_stops_later_factories_and_all_admission",
        "native_unwind_releases_consumed_instances_and_restores_frames_without_replay",
        "reference_nested_repeat_names_wire_authority_and_expansion_diagnostics_remain_compatible",
    ] {
        assert!(
            proof.contains(boundary),
            "missing repeat-sequence proof {boundary}"
        );
    }
    let host =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    assert!(host.contains("parsed_repeated_sequence_preserves_native_rows_values_exact_fuel_and_rejects_changed_semantics"));
    assert!(host.contains("assert_eq!(fuel.remaining(), 87)"));
    assert!(host.contains("changed native argument semantics"));
}

#[test]
fn shared_sequence_sessions_gate_each_request_on_the_original_accepted_reply_without_product_state()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/sequence_evaluation.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        "Identity: Clone",
        "Declaration: Clone",
        "Request: Clone",
        "Node: Clone",
        "IrResult: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "standalone sequence session gained {forbidden}"
        );
    }
    for required in [
        "SequenceEvaluationEnvironment",
        "PreparedSequenceMember",
        "PendingReply::new",
        "SequenceSession",
        "SequenceEvaluationLimits",
        "SequenceStatus",
        "SequenceEnd::HostUncertain",
        "SequenceEnd::Cancelled",
        "SequenceEnd::Completed",
        "SequenceEnd::Failed",
        "Reply: Borrow<View>",
        "Declaration: HostResultDomain<View>",
        "Identity: PartialEq",
        "No native Clone/Debug/serde/Send/Sync bound",
        "no retry, fuel refund or automatic restoration",
    ] {
        assert!(
            source.contains(required),
            "sequence session lost {required}"
        );
    }
    let start = source
        .split_once("pub fn start<Environment>")
        .unwrap()
        .1
        .split_once("pub fn poll<Environment>")
        .unwrap()
        .0;
    let physical = start.find("physical(").unwrap();
    let candidates = start.find("atomic_candidate(&branch.value)").unwrap();
    let cold = start.find(".preflight_member(index, branch)").unwrap();
    let charge = start.find("fuel.charge(1)").unwrap();
    assert!(physical < candidates && candidates < cold && cold < charge);
    let poll = source
        .split_once("pub fn poll<Environment>")
        .unwrap()
        .1
        .split_once("pub fn status")
        .unwrap()
        .0;
    let close = poll
        .find("self.state = State::Terminal(SequenceEnd::HostUncertain)")
        .unwrap();
    let charge = poll.find("self.fuel.charge(1)").unwrap();
    let prepare = poll.find("environment.prepare_member(").unwrap();
    let waiting = poll.find("PendingReply::new(").unwrap();
    assert!(close < charge && charge < prepare && prepare < waiting);
    assert!(poll.find("SequencePoll::Awaiting").unwrap() < close);
    let receive = source
        .split_once("pub fn try_accept<")
        .unwrap()
        .1
        .split_once("pub fn cancel")
        .unwrap()
        .0;
    assert!(
        receive.find("std::mem::replace").unwrap() < receive.find("pending.try_accept").unwrap()
    );
    assert!(receive.find("pending.try_accept").unwrap() < receive.find("self.next += 1").unwrap());
    assert!(!receive.contains("prepare_member"));
    let cancel = source.split_once("pub fn cancel").unwrap().1;
    assert!(cancel.find("std::mem::replace").unwrap() < cancel.find("drop(old)").unwrap());
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/sequence_evaluation.rs"))
            .unwrap();
    for proof in [
        "cold_shape_limits_and_candidates_precede_all_native_hooks",
        "move_only_native_rows_replies_and_requests_preserve_pointers_without_eager_work",
        "rejected_identity_authority_type_and_value_preserve_input_and_pending_member",
        "preparation_failure_or_unwind_is_terminal_without_fuel_refund_or_retry",
        "every_reply_callback_unwind_releases_identity_and_closes_without_rearming",
        "unsized_native_domain_and_text_reply_view_need_no_cloning_or_serialization",
    ] {
        assert!(tests.contains(proof), "sequence lifecycle lost {proof}");
    }
    let native =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_source.rs")).unwrap();
    for proof in [
        "parsed_nested_repeat_runs_one_native_request_per_accepted_reply_with_exact_fuel",
        "parsed_sequence_revocation_cancellation_and_stale_replies_never_invoke_a_successor",
        "NativeCounters",
        ".invoke(request)",
        "session.fuel_remaining(), 85",
        "(counters.left, counters.right), (85, 22)",
        "SequenceEnd::Cancelled",
        "SequenceEnd::Failed",
    ] {
        assert!(
            native.contains(proof),
            "parsed native sequence lost {proof}"
        );
    }
}

#[test]
fn shared_accepted_binding_reentry_preserves_original_sites_and_validation_order_without_product_state()
 {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/effect_reentry.rs")).unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        "Identity: Clone",
        "Capture: Clone",
        "Reply: Clone",
        "Node: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "binding reentry gained {forbidden}"
        );
    }
    for required in [
        "EffectContinuation",
        "pub(crate) fn new",
        "AcceptedReply",
        "Declaration: ?Sized",
        "EffectReentryEnvironment",
        "ResumableEffectOutcome",
        "resume_accepted_effects",
        "preflight_value_scope",
    ] {
        assert!(source.contains(required), "binding reentry lost {required}");
    }
    let resume = source
        .split_once("pub fn resume_accepted_effects<")
        .unwrap()
        .1;
    let physical = resume.find("preflight(body,").unwrap();
    let cold = resume.find(".preflight_effect(effect)").unwrap();
    let restore = resume.find(".restore_capture(").unwrap();
    let quota = resume
        .find("scope.len() >= limits.pure.max_bindings")
        .unwrap();
    let charge = resume.find("fuel.charge(scope.len() as u64)").unwrap();
    let prefix = resume.find("preflight_value_scope(&scope,").unwrap();
    let shadow = resume.find("scope.get(name)").unwrap();
    let projection = resume.find(".bind_reply(").unwrap();
    let insert = resume.find(".push(name, value)").unwrap();
    let mapped = resume.rfind("preflight_value_scope(&scope,").unwrap();
    let body = resume.rfind("evaluate(").unwrap();
    assert!(
        physical < cold && cold < restore && restore < quota && quota < charge && charge < prefix
    );
    assert!(
        prefix < shadow
            && shadow < projection
            && projection < insert
            && insert < mapped
            && mapped < body
    );
    let walker =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/effect_evaluation.rs")).unwrap();
    assert!(walker.contains("EffectContinuation::new(name, body, capture)"));
    assert!(walker.contains(".map(ResumableEffectOutcome::into_legacy)"));
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
            .unwrap();
    for proof in [
        "shared_accepted_reentry_preserves_alias_identity_exact_fuel_and_original_binding_sites",
        "native_capture_metadata_cannot_redirect_the_core_owned_reply_binding_or_body",
        "projection_unwind_releases_restored_aliases_and_actual_reply_without_rearming",
        "reentry_bounds_and_current_cold_schemas_stop_before_restore_projection_and_fuel",
        "invalid_restored_prefixes_stop_before_reply_projection_or_body_execution",
        "restoration_and_projection_failure_or_unwind_never_rearm_or_enter_the_body",
        "restoration_prefix_fuel_is_charged_before_projection_without_refund",
        "shared_gui_reentry_accepts_a_move_only_text_payload_and_unsized_native_domain",
        "cancelled_core_owned_continuation_never_restores_or_projects_late_input",
    ] {
        assert!(tests.contains(proof), "binding reentry lost proof {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    for proof in [
        "parsed_binding_invokes_native_calls_accepts_original_domains_and_reenters_with_exact_fuel",
        "execution.invoke(request)",
        "execution.actual.get(), 42",
        "fuel.remaining(), 94",
    ] {
        assert!(parsed.contains(proof), "parsed binding chain lost {proof}");
    }
}

#[test]
fn borrowed_control_session_keeps_reply_ready_and_owned_fuel_without_product_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/effect_session.rs")).unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        ".unwrap(",
        "Identity: Clone",
        "Capture: Clone",
        "Reply: Clone",
        "Node: Clone",
    ] {
        assert!(
            !production.contains(forbidden),
            "control session gained {forbidden}"
        );
    }
    for required in [
        "ControlSession",
        "EffectSessionEnvironment",
        "CorrelatedEffectRequest",
        "PendingReply::new",
        "EffectSessionEnd::HostUncertain",
        "EffectSessionEnd::Cancelled",
        "EffectSessionEnd::Failed",
        "EffectSessionEnd::Completed",
        "AcceptedBinding",
        "AcceptedFinal",
        "Reply: Borrow<View>",
        "Declaration: HostResultDomain<View>",
        "Identity: PartialEq",
        "type Declaration: ?Sized",
    ] {
        assert!(source.contains(required), "control session lost {required}");
    }
    let poll = source.split_once("pub fn poll<Environment>").unwrap().1;
    let close = poll.find("std::mem::replace").unwrap();
    assert!(poll.find("EffectSessionPoll::Awaiting").unwrap() < close);
    assert!(close < poll.find("evaluate_resumable_effects_in_scope(").unwrap());
    assert!(close < poll.find("resume_accepted_effects(").unwrap());
    assert!(
        poll.find("drop(bindings)").unwrap()
            < poll.find("self.state = State::AwaitingFinal").unwrap()
    );
    assert!(
        poll.find("drop(identity)").unwrap() < poll.find("EffectSessionEnd::Completed").unwrap()
    );
    let receive = source
        .split_once("pub fn try_accept<")
        .unwrap()
        .1
        .split_once("impl<Node:")
        .unwrap()
        .0;
    assert!(
        receive.find("std::mem::replace").unwrap() < receive.find("pending.try_accept").unwrap()
    );
    assert!(receive.contains("State::AcceptedBinding(accepted)"));
    assert!(receive.contains("State::AcceptedFinal(accepted)"));
    for forbidden in [
        "resume_accepted_effects(",
        "correlate_request(",
        "bind_reply(",
        "fuel.charge",
    ] {
        assert!(
            !receive.contains(forbidden),
            "reply acceptance implicitly ran {forbidden}"
        );
    }
    let cancel = source
        .split_once("pub fn cancel")
        .unwrap()
        .1
        .split_once("pub fn try_accept")
        .unwrap()
        .0;
    assert!(cancel.find("std::mem::replace").unwrap() < cancel.find("drop(old)").unwrap());
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "accepted_binding_waits_for_explicit_poll_and_returns_requests_and_pure_tail_once",
        "final_native_reply_is_projected_only_on_later_poll_and_handed_off_once",
        "wrong_identity_policy_type_and_domain_return_exact_input_and_keep_waiting",
        "cancel_initial_waiting_or_accepted_binding_and_final_reply_prevents_all_later_work",
        "physical_prefix_limits_and_cold_policy_precede_fuel_and_preparation",
        "native_preparation_capture_and_correlation_failures_or_unwind_are_terminal",
        "every_native_acceptance_callback_unwind_seals_the_session_without_rearming",
        "restored_binding_or_final_projection_failure_unwind_and_bounds_never_replay",
        "owned_initial_prefix_cleanup_unwind_cannot_install_a_waiting_request",
        "cancellation_cleanup_unwind_retains_cancelled_reason_and_drops_once",
        "accepted_identity_or_capture_cleanup_unwind_cannot_publish_completion_or_successor",
        "correlation_error_followed_by_capture_cleanup_unwind_stays_host_uncertain",
        "exhaustion_is_terminal_and_never_refills_while_waiting_or_after_acceptance",
    ] {
        assert!(tests.contains(proof), "control session lost {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    assert!(
        parsed.contains(
            "parsed_binding_session_owns_reply_gated_reentry_final_output_and_cancellation"
        )
    );
    assert!(parsed.contains("cancelled.fuel_remaining(), 98"));
    assert!(parsed.contains("execution.invoke(write)"));
    let gui = std::fs::read_to_string(root.join("crates/leselang-hir/tests/effect_evaluation.rs"))
        .unwrap();
    assert!(gui.contains("gui_local_control_session_owns_borrowed_dispatch_move_only_input_and_cancel_after_acceptance"));
}

#[test]
fn recursive_control_source_is_borrowed_once_admitted_and_product_free() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/control_source.rs")).unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        "unreachable!",
        ".expect(",
        ".unwrap(",
        "Type: Clone",
        "Type: PartialEq",
        "Type: std::fmt::Debug",
    ] {
        assert!(
            !production.contains(forbidden),
            "source entry gained {forbidden}"
        );
    }
    for required in [
        "ControlSourceAdapter",
        "ControlSourceScope",
        "prefix.to_vec()",
        "&ty",
        "future + 1",
        "future + 2",
        "check_pending(0, future)",
        "physical(&value, depth",
        "preflight_with_budget(&when",
        "drop(ty)",
        "drop(otherwise_type)",
        "ControlSourcePhase::Admit",
    ] {
        assert!(source.contains(required), "source entry lost {required}");
    }
    let entry = source.split_once("pub fn lower_control_source").unwrap().1;
    let build = entry.find(".build(expression").unwrap();
    for preflight in [
        "preflight(expression",
        "preflight_names(expression",
        "cold_names(expression",
        "preflight_bindings(",
        "crate::projection_source::preflight_names(expression)",
    ] {
        assert!(entry.find(preflight).unwrap() < build);
    }
    assert!(build < entry.find(".admit(").unwrap());
    assert_eq!(production.matches(".admit(").count(), 1);
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/control_source.rs")).unwrap();
    for proof in [
        "recursive_bind_choose_borrows_original_scope_metadata_and_moves_native_buffers",
        "whole_cold_signatures_and_lexical_errors_stop_every_native_hook",
        "prefix_names_and_active_quota_are_checked_before_native_work",
        "initializer_sees_parent_and_sibling_scopes_do_not_leak",
        "aggregate_leaf_expansion_retains_outer_future_roots_before_sibling_hooks",
        "shifted_native_leaf_depth_is_not_reset_at_each_control_boundary",
        "independent_condition_purity_and_literal_kind_cannot_be_overridden",
        "every_native_failure_or_unwind_releases_owned_parts_without_admission_or_retry",
        "native_observation_cleanup_unwind_cannot_publish_a_complete_tree",
    ] {
        assert!(tests.contains(proof), "source entry lost {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/binding_source.rs")).unwrap();
    assert!(
        parsed.contains("parsed_recursive_control_source_runs_selected_native_calls_and_pure_tail")
    );
    assert!(
        parsed.contains(
            "recursive_control_native_admission_rejects_cold_types_and_unfinished_capture"
        )
    );
    assert!(parsed.contains("session.fuel_remaining(), 84"));
    assert!(parsed.contains("other.fuel_remaining(), 86"));
    assert!(parsed.contains("infer_call_flow_type("));
}

#[test]
fn bound_native_projection_source_has_exact_borrowed_type_queries_without_product_frames() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/bound_projection_source.rs"))
            .unwrap();
    let production = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "leserpent_domain",
        "leserpent_runtime",
        "leselang_vm",
        "rusqlite",
        "serde::",
        ".clone()",
        "panic!",
        ".expect(",
        ".unwrap(",
        "Result: Clone",
        "Result: PartialEq",
        "lower_value(",
        "unsafe",
    ] {
        assert!(
            !production.contains(forbidden),
            "bound projection gained {forbidden}"
        );
    }
    for required in [
        "BoundProjectionSourceEnvironment",
        "BoundProjectionSourceLimits",
        "BoundProjectionSourceError::Unbound",
        "BoundProjectionSourceError::BoundReference",
        "PureType::Result(result)",
        "ProjectionSourceError::NonResult",
        "environment.field(result",
        "environment.member(result",
        "member.construct(operation)",
        "Node::Local",
        "budget.visit(1, 0, 0)",
    ] {
        let compact = source.split_whitespace().collect::<Vec<_>>().join(" ");
        let flat = compact
            .replace("environment .", "environment.")
            .replace("budget .", "budget.");
        assert!(flat.contains(required), "bound projection lost {required}");
    }
    let entry = source
        .split_once("pub fn lower_bound_projection_source")
        .unwrap()
        .1;
    let query = entry.find("environment").unwrap_or(entry.len());
    assert!(query < entry.len());
    for gate in [
        "preflight(expression",
        "preflight_names(expression",
        "prefix.len()",
        "PureType::Result(result)",
    ] {
        assert!(entry.find(gate).unwrap() < entry.find(".member(").unwrap());
        assert!(entry.find(gate).unwrap() < entry.find(".field(").unwrap());
    }
    let tests =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_source.rs"))
            .unwrap();
    for proof in [
        "two_unrelated_schemas_borrow_original_result_metadata_and_export_all_six_scalars",
        "bound_group_member_uses_exact_group_observation_and_moves_returned_slots",
        "missing_scalar_foreign_and_non_group_bindings_cannot_launder_exports_by_name",
        "malformed_cold_projection_metadata_precedes_all_native_queries",
        "prefix_validity_quota_and_all_minimum_source_output_limits_precede_native_queries",
        "inclusive_field_two_node_one_depth_and_member_one_node_zero_depth_bounds_are_exact",
        "native_error_and_unwind_do_not_copy_drop_or_requery_borrowed_metadata",
    ] {
        assert!(tests.contains(proof), "bound projection lost {proof}");
    }
    let parsed =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/bound_projection_session.rs"))
            .unwrap();
    for proof in [
        "parsed_record_reply_fields_drive_native_branch_arguments_with_exact_fuel_and_no_snapshot_copy",
        "foreign_record_domain_and_cancellation_before_field_projection_preserve_native_reply_ownership",
        "whole_native_source_typing_rejects_foreign_fields_unbound_aliases_and_cold_grants_before_calls",
        "post_acceptance_live_policy_and_native_view_identity_fail_before_write_without_retry",
        "weak.strong_count()",
        "session.fuel_remaining()",
        "lower_bound_projection_source",
        "infer_call_flow_type",
    ] {
        assert!(parsed.contains(proof));
    }
}

#[test]
fn shared_scalar_loop_source_reuses_original_ir_scope_limits_and_reference_gates() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/scalar_source.rs")).unwrap();
    for boundary in [
        "pub(crate) struct LoopSource",
        "pub(crate) fn loop_source",
        "pub(crate) fn lower_loop_body",
        "pub(crate) fn scalar_operand",
        "*value > MAX_LOOP_ITERATIONS",
        "source.limit()?",
        "preflight_bindings(source.condition, &mut local, max_bindings)",
        "preflight_bindings(source.next, &mut local, max_bindings)",
        "source.construct(initial, body, limit)",
        "if ty != state_type",
    ] {
        assert!(
            source.contains(boundary),
            "missing loop boundary {boundary}"
        );
    }
    assert!(!source.contains("LoopBudget::new"));
    assert!(!source.contains("0..limit"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let loop_lowerer = reference
        .split_once("fn lower_loop")
        .unwrap()
        .1
        .split_once("fn lower_fold")
        .unwrap()
        .0;
    assert!(loop_lowerer.contains("crate::scalar_source::loop_source(arguments, span)"));
    assert!(loop_lowerer.contains("crate::scalar_source::lower_loop_body("));
    assert!(loop_lowerer.contains("crate::scalar_source::scalar_operand("));
    assert!(!loop_lowerer.contains("Computation::Loop {"));
    assert!(!loop_lowerer.contains("next_type != state_type"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_loop_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "LoopError::IterationLimit",
        "PureEvaluationFault::FuelExhausted",
        "catch_unwind",
        "std::ptr::eq",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_scalar_fold_source_keeps_two_local_policy_without_collection_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/scalar_source.rs")).unwrap();
    for boundary in [
        "pub(crate) use fold::{fold_items, fold_next, fold_source}",
        "preflight_bindings(source.items, scope, max_bindings)",
        "source.check_scope(&self.scope, Some(max_bindings))",
        "fold_items(items, span(source.items))",
        "fold_next(next, state_type, span(source.next))",
        "source.construct(items, initial, next, limit)",
    ] {
        assert!(
            source.contains(boundary),
            "missing fold boundary {boundary}"
        );
    }
    let fold = std::fs::read_to_string(root.join("crates/leselang-hir/src/scalar_source/fold.rs"))
        .unwrap();
    for boundary in [
        "pub(crate) struct FoldSource",
        "pub item: &'source str",
        "pub fn check_scope<Value, Error>",
        "scope.len().checked_add(2)",
        "MAX_STRING_LIST_ITEMS as u64",
        "pub(crate) fn fold_source",
        "pub(crate) fn fold_items",
        "pub(crate) fn fold_next",
        "Computation::Fold {",
        "if ty != ScalarType::StringList",
        "if ty != state_type",
    ] {
        assert!(fold.contains(boundary), "missing fold boundary {boundary}");
    }
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "FoldCursor",
        "Fuel::",
        "0..limit",
    ] {
        assert!(
            !fold.contains(coupling),
            "unexpected fold coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("fn lower_fold")
        .unwrap()
        .1
        .split_once("pub(super) fn valid_local")
        .unwrap()
        .0;
    for boundary in [
        "crate::scalar_source::fold_source(arguments, span)",
        "crate::scalar_source::fold_items(",
        "crate::scalar_source::scalar_operand(",
        "crate::scalar_source::fold_next(",
        "source.construct(items, initial, next, limit)",
    ] {
        assert!(lowerer.contains(boundary));
    }
    assert!(!lowerer.contains("Computation::Fold {"));
    assert!(!reference.contains("fn scalar("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/scalar_fold_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "PureEvaluationFault::Fold(",
        "PureEvaluationFault::FuelExhausted",
        "catch_unwind",
        "std::ptr::eq",
        "Rc::ptr_eq",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn shared_projection_source_keeps_native_export_queries_bounded_and_reference_compatible() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/projection_source.rs")).unwrap();
    for boundary in [
        "pub trait ProjectionSourceEnvironment",
        "pub fn lower_projection_source",
        "preflight(expression, limits)",
        "preflight_names(expression)",
        ".check_pending(0, 1)",
        "preflight_with_budget(&value, 1, budget)",
        "let PureType::Result(result) = input_type",
        "valid_local_name(self.group)",
        "valid_member_name(self.name)",
        "source.construct(operation)",
        "self.construct(value, field)",
    ] {
        assert!(
            source.contains(boundary),
            "missing projection boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_projection_source")
        .unwrap()
        .1;
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public
                .find("environment.member")
                .or_else(|| public.find(".member(source.group"))
                .unwrap()
    );
    let finish = source.split_once("fn finish_with_budget").unwrap().1;
    assert!(
        finish.find("preflight_with_budget(&value").unwrap()
            < finish.find("export(&input, key)").unwrap()
    );
    assert!(
        public.find("source.finish_with_budget(").unwrap()
            < public.find(".field(result, name)").unwrap()
    );
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "OperationCatalog",
        "Field: Clone",
        "type Result: Clone",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected projection coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("pub(super) fn lower_expression_with_functions")
        .unwrap()
        .1
        .split_once("fn lower_loop")
        .unwrap()
        .0;
    assert!(lowerer.contains("crate::projection_source::field_source(arguments, span)"));
    assert!(lowerer.contains("lower_bound_projection_source("));
    assert!(lowerer.contains(".lower_preflighted("));
    assert!(!lowerer.contains("Computation::Field {"));
    assert!(!lowerer.contains("Computation::Member {"));
    assert!(!reference.contains("fn named"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/projection_source.rs"))
            .unwrap();
    for boundary in [
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "evaluate_call_arguments_in_scope",
        "lower_scalar_source",
        "catch_unwind",
        "std::ptr::eq",
        "Rc::ptr_eq",
        "serde_json::to_vec",
        "source_construction_does_not_require_clone_or_debug_for_result_observations",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn reference_field_frontend_uses_shared_consuming_stages_without_changing_public_preflight() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/projection_source.rs")).unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    let staged = production
        .split_once("pub(crate) fn lower_preflighted")
        .unwrap()
        .1
        .split_once("fn finish_with_budget")
        .unwrap()
        .0;
    assert!(
        staged.find(".check_pending(0, 1)").unwrap() < staged.find("lower(self.value)").unwrap()
    );
    assert!(
        staged.find("lower(self.value)").unwrap()
            < staged.find("self.finish_with_budget(").unwrap()
    );
    let finish = production
        .split_once("fn finish_with_budget")
        .unwrap()
        .1
        .split_once("pub fn construct")
        .unwrap()
        .0;
    assert!(
        finish.find("decode(self.literal_name()?)").unwrap()
            < finish.find("preflight_with_budget(&value").unwrap()
    );
    assert!(
        finish.find("preflight_with_budget(&value").unwrap()
            < finish.find("export(&input, key)").unwrap()
    );
    assert!(finish.contains("self.construct(value, field)"));
    for coupling in [
        ".clone()",
        "evaluate_",
        "validate_in_type_scope",
        "ResultField",
        "HostOperation",
        "Field: Clone",
        "Input: Clone",
    ] {
        assert!(
            !production.contains(coupling),
            "unexpected field coupling {coupling}"
        );
    }
    let public = production
        .split_once("pub fn lower_projection_source")
        .unwrap()
        .1;
    assert!(
        public.find("preflight_names(expression)").unwrap()
            < public.find(".lower_value(source.value)").unwrap()
    );
    assert!(public.contains("source.finish_with_budget("));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let field = reference
        .split_once("if callee == \"field\"")
        .unwrap()
        .1
        .split_once("if callee == \"choose\"")
        .unwrap()
        .0;
    for boundary in [
        ".lower_preflighted(",
        "lower_expression_with_functions(child, scope, visited, depth + 1, functions)",
        "ResultField::parse(name)",
        "field.result_type(*input_type)",
    ] {
        assert!(
            field.contains(boundary),
            "missing field boundary {boundary}"
        );
    }
    for duplication in [
        ".literal_name()",
        "source.construct(",
        "value.is_pure()",
        "Computation::Field {",
    ] {
        assert!(!field.contains(duplication));
    }
    assert!(
        reference
            .contains("field_stages_charge_original_child_once_without_lowering_name_metadata")
    );
    for proof in [
        "exact_original_ast_name_input_buffer_and_move_only_slots_pass_once",
        "native_child_failure_precedes_nonliteral_metadata_and_is_redacted",
        "closed_name_failure_precedes_impure_input_without_an_export_query",
        "entire_input_purity_and_one_root_budget_precede_native_export",
        "export_error_or_unwind_drops_original_input_and_key_without_retry",
    ] {
        assert!(source.contains(proof));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/field_source_reference.rs"))
            .unwrap();
    for boundary in [
        "field_metadata_errors_preserve_child_before_name_before_native_export_order",
        "lookalike_native_records_do_not_supply_foreign_field_exports",
        "field_does_not_hide_effects_in_cold_pure_input_branches",
        "helper_local_result_names_never_escape_their_original_lexical_prefix",
        "canonical_source",
        "serde_json::to_vec",
        "authorize",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn native_host_source_owns_once_preparation_admission_and_construction_without_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/host_source.rs")).unwrap();
    let public = source
        .split_once("pub fn lower_host_source")
        .unwrap()
        .1
        .split_once("pub(crate) fn lower_preflighted_host_source")
        .unwrap()
        .0;
    assert!(
        public.find("preflight(expression, limits)").unwrap()
            < public
                .find("lower_preflighted_host_source(expression")
                .unwrap()
    );
    let core = source
        .split_once("pub(crate) fn lower_preflighted_host_source")
        .unwrap()
        .1;
    assert!(core.find(".visit(0, 0, 0)").unwrap() < core.find("prepare(expression)").unwrap());
    assert!(
        core.find("prepare(expression)").unwrap()
            < core.find("admit(expression, &effect, &ty)").unwrap()
    );
    assert!(
        core.find("admit(expression, &effect, &ty)").unwrap()
            < core.find("Computation::Host").unwrap()
    );
    assert!(core.contains("Box::new(effect)"));
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        ".clone()",
        "canonical_source(",
        "validate_in_type_scope",
        "evaluate_",
        "Clone +",
        "Send +",
        "serde::",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected host source coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let lowerer = reference
        .split_once("pub(super) fn lower_expression_with_functions")
        .unwrap()
        .1
        .split_once("fn lower_loop")
        .unwrap()
        .0;
    let host = lowerer
        .split_once("crate::host_source::lower_preflighted_host_source(")
        .unwrap()
        .1;
    for boundary in [
        "lower_effect(source)?",
        "contains_computation(effect)",
        "HostOperation::parse(callee)",
        "HostOperation::for_effect(effect)",
        "operation.map(HostOperation::result_type) == Some(*result)",
        "HostSourceError::Preparation",
        "HostSourceError::Admission",
    ] {
        assert!(
            host.contains(boundary),
            "missing native host policy {boundary}"
        );
    }
    assert!(!lowerer.contains("Computation::Host {"));
    assert!(
        lowerer.find(".finish_call(").unwrap()
            < lowerer.find("lower_preflighted_host_source(").unwrap()
    );
    assert!(reference.contains(
        "native_host_source_charges_one_original_root_and_never_visits_literal_metadata"
    ));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_source.rs")).unwrap();
    for boundary in [
        "original_ast_payload_buffer_and_move_only_type_pass_once_into_one_host_root",
        "whole_cold_source_names_text_arguments_and_physical_limits_precede_native_work",
        "one_language_root_never_certifies_foreign_result_or_unbounded_opaque_graphs",
        "unrelated_numeric_and_panel_hosts_use_the_same_entry_without_product_payloads",
        "native_preparation_or_admission_unwind_drops_owned_parts_without_retry",
        "native_observations_after_handoff_are_not_saved_admission_certificates",
        "Rc::ptr_eq",
        "std::ptr::eq",
    ] {
        assert!(proof.contains(boundary));
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/host_source_reference.rs"))
            .unwrap();
    for boundary in [
        "native_leaf_errors_keep_original_messages_priority_and_shifted_source_spans",
        "helper_native_host_unwrapping_preserves_the_direct_atomic_product_bytes",
        "literal_host_bridge_does_not_replace_the_computed_argument_call_path",
        "native_host_leaves_still_supply_closed_group_members_and_projected_result_bindings",
        "canonical_source",
        "authorize",
        "serde_json::to_vec",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn reference_member_frontend_uses_shared_bounds_and_exact_borrowed_native_group_exports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let member = source
        .split_once("if callee == \"member\"")
        .unwrap()
        .1
        .split_once("if callee == \"field\"")
        .unwrap()
        .0;
    for boundary in [
        "lower_bound_projection_source(",
        "BoundProjectionSourceLimits",
        "PureType::Result(MemberSourceType::Bound(local))",
        "Type::Scalar(ty) => PureType::Scalar(ty)",
        "max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS",
        "PureType::Result(MemberSourceType::Selected(ty))",
        "member is not exported by this bound group",
    ] {
        assert!(
            member.contains(boundary),
            "missing reference member boundary {boundary}"
        );
    }
    for legacy in [
        "member_source(arguments",
        "scope.get(",
        ".clone()",
        "Computation::Member {",
    ] {
        assert!(
            !member.contains(legacy),
            "member frontend retained {legacy}"
        );
    }
    let adapter = source
        .split_once("fn member(\n")
        .unwrap()
        .1
        .split_once("impl From<Type>")
        .unwrap()
        .0;
    assert!(adapter.contains("MemberSourceType::Bound(local)"));
    assert!(adapter.contains("local.members().and_then"));
    assert!(adapter.contains("MemberSourceType::Selected(operation.result_type())"));
    assert!(!adapter.contains(".clone()"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/member_source_reference.rs"))
            .unwrap();
    for claim in [
        "reference_member_source_preserves_exact_native_slot_without_a_synthetic_local",
        "same_named_member_in_a_different_group_does_not_authorize_a_foreign_native_operation",
        "same_result_field_type_does_not_allow_substitution_of_another_native_operation",
        "scalar_record_missing_and_expired_helper_scopes_do_not_supply_member_exports",
        "member_header_diagnostics_keep_legacy_root_spans_and_never_lower_metadata_as_values",
        "full_width_group_last_member_and_source_metadata_limits_preserve_wire_and_authority",
        "field_child_error_precedence_is_unchanged_when_member_lowering_delegates_to_shared_core",
    ] {
        assert!(proof.contains(claim), "missing native member proof {claim}");
    }
}

#[test]
fn shared_owned_helper_hygiene_preserves_native_slots_and_reference_expansion_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_hygiene.rs")).unwrap();
    for boundary in [
        "pub struct HelperHygieneLimits",
        "pub enum HelperHygieneError",
        "pub fn hygienic_helper_body",
        "parameters: &[(&str, &str)]",
        "reserved: &[&str]",
        "StructureBudget::new(limits.max_nodes, limits.max_depth)",
        ".check_pending(pending.len(), 1)",
        "names: &'names HashSet<String>",
        "ScopeFrame",
        "std::mem::replace(name, fresh.clone())",
        "self.names.contains(&name)",
        "!self.used.insert(name.clone())",
        "HelperHygieneError::Capture { group: true }",
        "check_binding(name, scope, 2, max_bindings)",
        "check_binding(item, scope, 2, max_bindings)",
        "Node::Literal { .. } | Node::Host { .. } => {}",
        "Failure/unwind drops the consumed body",
        "native name reservations are not rolled back or refunded",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper boundary {boundary}"
        );
    }
    let public = source.split_once("pub fn hygienic_helper_body").unwrap().1;
    let rename = public.find("renamer.rename(").unwrap();
    for preflight in [
        "collect_names(&body, limits, &mut names)",
        "check_scope(",
        "names.contains(alias)",
    ] {
        assert!(public.find(preflight).unwrap() < rename);
    }
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "infer_pure_type",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "body.clone()",
        "scope.clone()",
        "BTreeMap",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    for boundary in [
        "crate::helper_instance_finish::finish_helper_instance(",
        "body.clone()",
        "fresh_name(self.names, self.next_name)",
        ".bindings()",
        "max_reserved_names: MAX_COMPUTATION_NODES",
        "helper cannot capture caller locals",
        "helper cannot capture a caller group",
    ] {
        assert!(
            lowerer.contains(boundary),
            "missing reference helper boundary {boundary}"
        );
    }
    assert!(
        lowerer.find("lower_preflighted_helper_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    assert!(
        lowerer.find("finish_helper_instance(").unwrap()
            < lowerer.find("struct InstanceFinisher").unwrap()
    );
    assert!(!reference.contains("fn rename("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_hygiene.rs")).unwrap();
    for boundary in [
        "lower_scalar_source_with_scope",
        "parse(",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "exact_execution_fuel",
        "catch_unwind",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "serde_json::to_vec",
        "call_group_and_string_forests_do_not_export_sibling_temporary_bindings",
    ] {
        assert!(proof.contains(boundary), "missing helper proof {boundary}");
    }
}

#[test]
fn shared_helper_signature_and_arguments_keep_source_borrows_native_slots_and_expansion_boundaries()
{
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_source.rs")).unwrap();
    for boundary in [
        "pub fn helper_parameters",
        "pub struct HelperSignature",
        "pub struct HelperSourceLimits",
        "pub fn lower_helper_arguments",
        "max_parameters > MAX_FUNCTION_PARAMETERS",
        "NamedParameter::required(parameter.name.as_str(), ty)",
        "preflight::<Error>(expression, limits.source)",
        "callee != signature.name",
        "!parameter.required || !valid_local_name(parameter.name)",
        "bind_named_arguments(&names, parameters)",
        "bound.iter().enumerate()",
        "let parameter = &parameters[parameter_index]",
        ".check_pending(0, arguments.len() - parameter_index)",
        "preflight_with_budget(&value, 0, &mut budget)",
        "check_argument_type(&ScalarTypeSet::only(parameter.domain), facts)",
        "pub parameter: &'signature HelperParameter<'signature>",
        "pub argument: &'source NamedArgument",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper argument boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_helper_arguments")
        .unwrap()
        .1;
    assert!(
        public.find("preflight::<Error>").unwrap()
            < public
                .find("lower_preflighted_helper_arguments(arguments")
                .unwrap()
    );
    let preparation = source
        .split_once("pub(crate) fn lower_preflighted_helper_arguments")
        .unwrap()
        .1;
    assert!(
        preparation.find("bind_named_arguments(").unwrap()
            < preparation.find("lower(argument)").unwrap()
    );
    assert!(
        preparation.find(".check_pending(").unwrap() < preparation.find("lower(argument)").unwrap()
    );
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Leserpent",
        "sqlite",
        "evaluate_pure",
        "infer_pure_type",
        "OperationCatalog",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "value.clone()",
        "body.clone()",
        "ScopeFrame",
        "Fuel::",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper argument coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let parameter_adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_body_source.rs"))
            .unwrap();
    assert!(
        parameter_adapter
            .contains("helper_parameters(function, limits.body.template.max_parameters)")
    );
    assert!(reference.contains("crate::helper_body_source::lower_preflighted_helper_body("));
    assert!(!reference.contains("fn parameter_types"));
    assert!(!parameter_adapter.contains("parameter.type_name.as_str()"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("crate::helper_source::lower_preflighted_helper_arguments("));
    assert!(!lowerer.contains("value.is_pure()"));
    assert!(
        lowerer.find("lower_preflighted_helper_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    assert!(lowerer.contains("helper arguments must be pure scalars of the declared type"));
    assert!(lowerer.contains("arguments[argument_index].span"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_source.rs")).unwrap();
    for boundary in [
        "helper_parameters",
        "lower_scalar_source_with_scope",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "before.remaining(), after.remaining()",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "successful_dynamic_observations_do_not_replace_complete_cold_inference",
        "native_output_bounds_and_language_names_are_not_certified_by_type_observations",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper argument proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_dependencies_keep_borrowed_bounded_lazy_planning_separate_from_execution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_dependencies.rs"))
            .unwrap();
    for boundary in [
        "pub const MAX_DEPENDENCY_HELPERS: usize = 31",
        "pub struct HelperDependencyLimits",
        "pub enum HelperDependencyError",
        "pub struct HelperDependencyOrder<'source>",
        "pub fn helper_dependency_order<'source>",
        "helpers: &[&'source Function]",
        "helpers: Vec<(usize, &'source Function)>",
        "dependencies: [u32; MAX_DEPENDENCY_HELPERS]",
        "StructureBudget::new(limits.max_source_nodes, limits.max_source_depth)",
        ".check_pending(pending.len(), arguments.len())",
        "helpers.len() > limits.max_helpers",
        "!valid_local_name(entry)",
        "!valid_local_name(&function.name)",
        "helpers[..declaration_index]",
        "preflight_source(function, index, limits)?",
        "helpers.sort_unstable_by",
        "function.name.as_str().cmp(callee.as_str())",
        "dependencies[index] |= 1u32 << dependency",
        "declaration_index: *declaration_index",
        "span: *span",
        "self.dependencies[index] & !self.planned == 0",
        "self.planned |= bit",
        "impl FusedIterator for HelperDependencyOrder<'_>",
        "Early abandonment does not validate the graph",
        "Discard the cursor on lowering failure",
    ] {
        assert!(
            source.contains(boundary),
            "missing dependency boundary {boundary}"
        );
    }
    let preflight = source.split_once("fn preflight_source").unwrap().1;
    assert!(
        preflight.find(".check_pending(").unwrap() < preflight.find("pending.extend(").unwrap()
    );
    let constructor = source
        .split_once("pub fn helper_dependency_order")
        .unwrap()
        .1;
    assert!(
        constructor.find("preflight_source(").unwrap()
            < constructor.find("helpers.sort_unstable_by").unwrap()
    );
    assert!(
        constructor.find("preflight_source(").unwrap()
            < constructor.find(".binary_search_by(").unwrap()
    );
    assert!(!constructor.contains("HelperDependencyError::Cycle"));
    let debug = source
        .split_once("impl fmt::Debug for HelperDependencyOrder")
        .unwrap()
        .1
        .split_once("impl<'source> Iterator")
        .unwrap()
        .0;
    assert!(debug.contains("self.planned.count_ones()"));
    assert!(!debug.contains("function.name"));
    assert!(!debug.contains("function.body"));
    for coupling in [
        ".clone()",
        "to_owned()",
        "BTreeSet",
        "crate::Type",
        "HostOperation",
        "ResultField",
        "OperationCatalog",
        "evaluate_pure",
        "infer_pure_type",
        "canonical_source",
        "Template",
        "Fuel::",
        "sqlite",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected dependency coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_dependencies;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    for boundary in [
        "crate::program_source::assemble_preflighted_program(",
        "max_helpers: MAX_FUNCTIONS - 1",
        "max_source_nodes: MAX_COMPUTATION_NODES",
        "max_source_depth: MAX_EFFECT_NESTING_DEPTH",
        "|function, functions|",
        "computation::is_builtin(name)",
        "crate::helper_body_source::lower_preflighted_helper_body(",
        "main cannot have parameters",
        "crate::helper_instance_finish::finish_helper_instance(",
        "recursive helper functions are not supported",
        "helpers cannot call main",
    ] {
        assert!(
            reference.contains(boundary),
            "missing reference dependency gate {boundary}"
        );
    }
    assert!(!reference.contains("BTreeSet"));
    assert!(!reference.contains("let mut dependencies = BTreeMap"));
    let lowerer = reference
        .split_once("assemble_preflighted_program(")
        .unwrap()
        .1;
    assert!(
        lowerer.find("lower_preflighted_helper_body(").unwrap()
            < lowerer.find("lower_expression_with_functions(").unwrap()
    );
    let assembly =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_registry.rs")).unwrap();
    let program =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/program_source.rs")).unwrap();
    assert!(program.contains(
        "assemble_helper_registry(declarations, limits.helpers, state, prepare, register)"
    ));
    assert!(assembly.contains("helper_dependency_order("));
    assert!(assembly.contains("for function in order"));
    assert!(
        assembly.find("function.map_err(").unwrap()
            < assembly.find("prepare(function, &mut state)").unwrap()
    );
    assert!(
        assembly
            .find("register(function, body, &mut state)")
            .unwrap()
            < assembly.find("Ok(state)").unwrap()
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_dependencies.rs"))
            .unwrap();
    for boundary in [
        "reference_order",
        "all_input_permutations_preserve_ready_order_with_chains_and_shared_dependencies",
        "dense_maximum_graph_uses_safe_bits_and_matches_the_old_order",
        "direct_mutual_and_cold_unused_cycles_are_not_reachability_pruned",
        "ready_helpers_precede_a_single_terminal_cycle_error_including_blocked_dependents",
        "all_physical_preflight_precedes_entry_call_scanning_without_mutating_source",
        "entry_call_priority_is_lexical_helper_then_rightmost_physical_child",
        "dependency_planning_is_not_signature_operand_or_native_operation_validation",
        "reference_ready_helper_lowering_error_keeps_precedence_over_later_cycle_detection",
        "std::ptr::eq",
        "helper_parameters",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing dependency proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_wrappers_keep_shifted_bounds_capture_fences_and_native_slot_ownership() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_bindings.rs")).unwrap();
    for boundary in [
        "pub struct HelperBinding<Node>",
        "pub struct HelperBindingLimits",
        "pub enum HelperBindingError",
        "pub fn bind_helper_arguments",
        "bindings: Vec<HelperBinding<Node<Field, Operation, HostEffect, IrResult>>>",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "bindings.len() > limits.max_parameters",
        "!valid_local_name(&binding.name)",
        "bindings[..index]",
        "StructureBudget::new(limits.max_nodes, limits.max_depth)",
        ".visit(index, 0, 0)",
        ".check_pending(0, bindings.len() + 1)",
        ".check_pending(0, bindings.len() - index + 1)",
        "preflight_with_budget(&binding.value, index + 1, &mut budget)",
        "conflicts(node, &bindings, true)",
        "let mut pending = vec![(&body, bindings.len())]",
        "conflicts(node, &bindings, false)",
        ".check_pending(pending.len(), 1)",
        "for binding in bindings.into_iter().rev()",
        "name: binding.name",
        "value: Box::new(binding.value)",
        "body: Box::new(body)",
        "The caller also reserves the",
        "Body literals/operator/group metadata are not certified by the shape walk",
    ] {
        assert!(
            source.contains(boundary),
            "missing wrapper boundary {boundary}"
        );
    }
    let constructor = source.split_once("pub fn bind_helper_arguments").unwrap().1;
    let wrap = constructor
        .find("for binding in bindings.into_iter().rev()")
        .unwrap();
    for check in [
        "!valid_local_name(",
        "preflight_with_budget(",
        "conflicts(node, &bindings, true)",
        "conflicts(node, &bindings, false)",
        "budget\n            .visit(depth, 0, 0)",
    ] {
        assert!(constructor.find(check).unwrap() < wrap);
    }
    assert!(
        constructor.find("preflight_with_budget(").unwrap()
            < constructor
                .find("conflicts(node, &bindings, true)")
                .unwrap()
    );
    for coupling in [
        ".clone()",
        "to_owned()",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "FnMut",
        "fn fresh_name",
        "canonical_source",
        "ScopeFrame",
        "sqlite",
        "Fuel::",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected wrapper coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_bindings;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    let wrap = lowerer.find("finish_helper_instance(").unwrap();
    for gate in [
        "crate::helper_templates::preflight(",
        "lower_preflighted_helper_arguments(",
        "let values = lowered",
    ] {
        assert!(lowerer.find(gate).unwrap() < wrap);
    }
    assert!(lowerer.contains("max_lowered_nodes: MAX_COMPUTATION_NODES"));
    assert!(lowerer.contains("max_lowered_depth: MAX_EFFECT_NESTING_DEPTH"));
    assert!(lowerer.contains("max_parameters: MAX_FUNCTION_PARAMETERS"));
    assert!(lowerer.contains("HelperInstanceArgument"));
    assert!(!lowerer.contains("body = Computation::Bind"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_bindings.rs")).unwrap();
    for boundary in [
        "eight_parameters_fit_exactly_seventeen_nodes_and_depth_eight",
        "every_alias_is_fenced_from_all_cold_caller_operand_names",
        "cold_body_bind_loop_and_fold_declarations_cannot_shadow_parameters",
        "native_projection_boxes_buffers_and_owned_alias_strings_move_unchanged",
        "opaque_host_call_and_group_body_slots_are_not_rebuilt_or_dispatched",
        "body_frontier_is_checked_before_growth_including_cold_group_children",
        "output_shape_is_not_complete_cold_lexical_or_scalar_type_acceptance",
        "unused_argument_failure_and_native_unwind_are_not_skipped_or_retried",
        "reference_wrapper_wire_authority_and_legacy_expansion_bounds_are_preserved",
        "helper_dependency_order",
        "helper_parameters",
        "lower_helper_arguments",
        "hygienic_helper_body",
        "infer_pure_type",
        "evaluate_pure_in_scope",
        "prepare_call_in_scope",
        "std::ptr::eq",
        "buffer.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "remaining[0], remaining[1]",
    ] {
        assert!(proof.contains(boundary), "missing wrapper proof {boundary}");
    }
}

#[test]
fn shared_source_cost_walk_bounds_cold_trees_before_borrowed_native_observations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/source_cost.rs")).unwrap();
    for boundary in [
        "pub struct SourceCostLimits",
        "pub struct SourceCostExtra",
        "pub enum SourceCostError<Error>",
        "pub fn literal_source_extra",
        "pub fn measure_source_cost<Field, Operation, HostEffect, IrResult, Error>",
        "impl FnMut(&HostEffect) -> Result<SourceCostExtra, Error>",
        "Result<HelperExpansionCost, SourceCostError<Error>>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "ScalarValue::OptionalString(_) => (1, 1)",
        "value.0.len()",
        "StructureBudget::new",
        "node.children()",
        "effect.as_ref()",
        "hosts.into_iter().enumerate()",
        "host_cost(effect)",
        "checked_add(extra.depth)",
        "checked_add(extra.nodes)",
        "host_index",
        "rightmost-child-first DFS",
        "not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing source-cost boundary {boundary}"
        );
    }
    let body = source.split_once("pub fn measure_source_cost").unwrap().1;
    let observer = body.find("host_cost(effect)").unwrap();
    assert!(
        body.find(".visit(depth, extra.nodes, extra.depth)")
            .unwrap()
            < observer
    );
    assert!(
        body.find(".check_pending(pending.len(), 1)").unwrap()
            < body.find("pending.push(").unwrap()
    );
    assert!(body.find("for child in node.children()").unwrap() < observer);
    let append = &body[observer..];
    assert!(append.find("DepthLimit").unwrap() < append.find("NodeLimit").unwrap());
    for coupling in [
        "crate::Type",
        "ResultField",
        "HostOperation",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "OperationCatalog",
        "Fuel::",
        "serde::",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "infer_pure_type(",
        "evaluate_pure(",
        "body.clone()",
        "saturating_add(",
        "Default for",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected source-cost coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod source_cost;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    assert!(reference.contains("budget.check_pending(effects.len(), 1)?"));
    assert!(reference.contains("prepared: admitted"));
    assert!(reference.contains("body: &template.prepared"));
    assert!(!reference.contains("pub(super) fn shape("));
    assert!(!reference.contains("literal_nodes"));
    assert!(!reference.contains("&values[index]"));
    let computation =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(computation.contains("crate::source_cost::literal_source_extra(value)"));
    let flow =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/function_flow.rs")).unwrap();
    let repeat =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computed_group.rs")).unwrap();
    assert!(flow.contains("crate::helper_join::HelperJoin::new("));
    assert!(flow.contains("functions::host_source_extra(effect, source_limits)"));
    assert!(repeat.contains("functions::source_cost(expression)"));
    assert!(!flow.contains("functions::shape("));
    assert!(!repeat.contains("functions::shape("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_cost.rs")).unwrap();
    for boundary in [
        "folded_literal_weights_include_optional_none_but_not_empty_list_children",
        "every_control_call_group_projection_form_uses_original_language_children",
        "complete_cold_language_tree_precedes_native_observation",
        "pending_frontier_is_bounded_before_growing_or_observing_cold_hosts",
        "native_sites_use_rightmost_first_dfs_and_relative_language_root_depth",
        "zero_iteration_loops_folds_and_recovery_still_include_all_cold_children",
        "native_depth_overflow_and_depth_before_node_priority_are_checked",
        "native_errors_move_once_without_formatter_bounds_or_payload_source_chains",
        "native_unwind_does_not_retry_rollback_effects_or_mutate_borrowed_ir",
        "move_only_native_slots_original_boxes_vectors_and_literal_buffers_are_borrowed",
        "measurement_does_not_certify_literal_bytes_lexical_types_or_native_metadata",
        "bounded_adapter_inputs_match_legacy_costs_and_independent_inclusive_limits",
        "parsed_folded_operands_compose_with_reservation_templates_hygiene_and_exact_fuel",
        "reference_helper_returns_wire_capabilities_and_cold_repeat_bounds_are_unchanged",
        "bytes.as_ptr()",
        "std::ptr::eq",
        "catch_unwind",
        "serde_json::to_vec",
        "before.remaining(), after.remaining()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing source-cost proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_normal_returns_admit_all_cold_copies_without_native_clone_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_returns.rs")).unwrap();
    for boundary in [
        "pub struct HelperReturnLimits",
        "pub struct HelperReturnShape",
        "pub enum HelperReturnError<Error>",
        "pub struct HelperReturns<Node>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "budget.check_pending(pending.len(), 1)?",
        "let analysis = analyze(&value, continuation_shape)",
        "checked_mul(returns)",
        "checked_add(nodes)",
        "UnsupportedBoundary",
        "exposed_names(&value, &route)",
        "names_are_valid(&value)",
        "names_are_valid(&continuation)",
        "names_are_valid(&body)",
        "names_conflict(&continuation, &name, &reserved)",
        "pub fn return_sites",
        "pub fn connect<Error>",
        "factory(&self.continuation, current)",
        "actual != self.continuation_shape",
        "names_conflict(&body, &self.name, &self.exposed_names)",
        "Host failures are not returns",
        "Pure",
        "locals do not escape",
        "not formatting or source chains",
        "source costs",
        "No callbacks",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper return boundary {boundary}"
        );
    }
    let admit = source
        .split_once("pub fn new(")
        .unwrap()
        .1
        .split_once("pub fn return_sites")
        .unwrap()
        .0;
    let analysis = admit.find("let analysis = analyze(").unwrap();
    assert!(admit.find("preflight(&value, limits)").unwrap() < analysis);
    assert!(admit.find("preflight(&continuation, limits)").unwrap() < analysis);
    assert!(admit.find("limits.max_nodes").unwrap() < analysis);
    let copy = source.split_once("pub fn connect<Error>").unwrap().1;
    let callback = copy.find("factory(&self.continuation, current)").unwrap();
    assert!(callback < copy.find("preflight(&body, self.limits)").unwrap());
    assert!(
        copy.find("actual != self.continuation_shape").unwrap()
            < copy.find("connect(self.value, self.route").unwrap()
    );
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "serde::",
        "body.clone()",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "Fuel::",
        "saturating_add(",
        "panic!(",
        "unreachable!(",
        ".expect(",
        ".unwrap(",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper return coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_returns;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/function_flow.rs")).unwrap();
    assert!(adapter.contains("crate::helper_join::HelperJoin::new("));
    assert!(!adapter.contains("for (value, level) in plan.return_sites()"));
    assert!(
        adapter.find("if bytes > MAX_SOURCE_BYTES").unwrap()
            < adapter.find("plan.connect(").unwrap()
    );
    assert!(adapter.contains("body.clone()"));
    assert!(adapter.contains(".validate_structure()"));
    assert!(!adapter.contains("fn connect("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_returns.rs")).unwrap();
    for boundary in [
        "every_cold_normal_return_is_admitted_before_left_to_right_factories",
        "whole_pure_control_subtrees_are_single_returns_not_rewritten_inside",
        "unsupported_cold_recover_loop_fold_and_operand_effect_boundaries_are_rejected",
        "complete_cold_input_frontiers_are_bounded_before_return_analysis",
        "all_cold_return_copies_are_reserved_not_only_the_selected_branch",
        "pure_terminal_and_initializer_binders_do_not_capture_the_continuation",
        "cold_lexical_names_are_bounded_before_route_name_copy_or_factory_capture_hashing",
        "value_binders_cannot_capture_caller_locals_groups_or_unused_cold_names",
        "equal_node_count_but_changed_depth_is_not_a_reserved_continuation",
        "move_only_all_four_native_slots_boxes_vectors_and_buffers_move_unchanged",
        "factory_error_moves_private_error_and_drops_original_and_partial_native_values_once",
        "factory_unwind_drops_partial_owned_inputs_without_retry_or_reservation_refund",
        "physical_admission_does_not_certify_folded_source_weights_or_native_semantics",
        "unrelated_device_host_preserves_explicit_bind_typing_value_and_exact_fuel",
        "reference_adapter_preserves_canonical_wire_capabilities_and_cold_return_diagnostics",
        "bounded_normal_return_corpus_matches_legacy_desugaring_and_exact_shape",
        "native_execution_failure_never_enters_a_normal_return_or_scalar_recovery",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
        "catch_unwind",
        "arguments.as_ptr()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper return proof {boundary}"
        );
    }
}

#[test]
fn shared_bounded_helper_join_checks_cold_costs_before_native_copy_and_whole_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_join.rs")).unwrap();
    for boundary in [
        "pub struct HelperJoinLimits",
        "pub enum HelperJoinError<Error>",
        "pub struct HelperJoin<Node>",
        "pub fn new<Error>",
        "pub fn connect<Error>",
        "pub const fn source_cost",
        "HelperReturns::new(name, value, continuation, limits.output)",
        "language_join_cost(&returns, limits.source)?",
        "measure_observed(returns.value(), limits.source, &mut observe)?",
        "observations.insert(node, extra)",
        "std::ptr::from_ref(node)",
        "Zero-sized native slots are distinct occurrences",
        "checked_mul(plan.shape().returns)",
        "cost.depth.max(continuation.depth)",
        "HelperJoinError::MissingObservation",
        "actual != self.language_cost",
        "admit(&expression, self.source_cost)",
        "not a global reservation, type or authority token",
        "Equal physical",
        "not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper join boundary {boundary}"
        );
    }
    let build = source
        .split_once("pub fn new<Error>")
        .unwrap()
        .1
        .split_once("pub fn connect<Error>")
        .unwrap()
        .0;
    let gates = [
        "limits.source.max_nodes",
        "HelperReturns::new(",
        "language_join_cost(",
        "let mut observations",
        "measure_observed(returns.value()",
        "measure_observed(returns.continuation()",
        "let source_cost = joined_cost(",
        "Ok(Self",
    ];
    for pair in gates.windows(2) {
        assert!(build.find(pair[0]).unwrap() < build.find(pair[1]).unwrap());
    }
    let connect = source.split_once("pub fn connect<Error>").unwrap().1;
    assert!(
        connect.find(".connect(factory)").unwrap()
            < connect
                .find("language_cost(&expression, self.limits.source)")
                .unwrap()
    );
    assert!(
        connect.find("actual != self.language_cost").unwrap()
            < connect
                .find("admit(&expression, self.source_cost)")
                .unwrap()
    );
    for coupling in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "Effect::",
        "Leserpent",
        "sqlite",
        "Mutex",
        "serde::",
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "HostEffect: Clone",
        "IrResult: Clone",
        "Fuel::",
        "saturating_add(",
        "panic!(",
        "unreachable!(",
        ".expect(",
        ".unwrap(",
        "Default for",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper join coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_join;"));
    let adapter =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/function_flow.rs")).unwrap();
    assert!(adapter.contains("crate::helper_join::HelperJoin::new("));
    assert!(adapter.contains("functions::host_source_extra(effect, source_limits)"));
    assert!(!adapter.contains("functions::source_cost("));
    assert!(
        adapter.find("if bytes > MAX_SOURCE_BYTES").unwrap()
            < adapter.find("plan.connect(").unwrap()
    );
    assert!(adapter.contains("|expression, _| expression.validate_structure()"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_join.rs")).unwrap();
    for boundary in [
        "exact_leaf_cost_is_admitted_before_once_whole_output_admission",
        "zero_invalid_physical_name_capture_and_boundary_limits_precede_native_queries",
        "both_cold_folded_inputs_are_checked_before_observing_the_first_native_value",
        "combined_cold_continuation_weights_are_bounded_before_native_observation",
        "original_hosts_are_observed_once_and_copies_keep_left_to_right_order",
        "native_extra_overflow_and_shifted_return_depth_are_checked_before_copy",
        "zero_sized_native_slots_do_not_merge_original_occurrence_costs",
        "same_physical_copy_with_changed_folded_node_weights_is_rejected_before_admission",
        "same_physical_copy_with_changed_folded_depth_is_rejected_before_admission",
        "equal_cost_does_not_certify_cold_native_semantics_or_live_authority",
        "move_only_all_four_slots_original_boxes_vectors_and_buffers_move_unchanged",
        "native_observation_error_keeps_private_payload_and_drops_owned_inputs_once",
        "native_observation_unwind_drops_inputs_without_retry_or_materialization",
        "copy_error_and_unwind_release_partial_output_without_admission_or_refund",
        "admission_error_and_unwind_never_publish_or_retry_the_complete_output",
        "unrelated_scalar_host_keeps_full_typing_result_fuel_and_caller_prefix",
        "bounded_return_corpus_costs_match_complete_post_copy_observation",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing helper join proof {boundary}"
        );
    }
}

#[test]
fn shared_helper_expansion_reservation_preserves_source_costs_without_execution_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_expansion.rs")).unwrap();
    for boundary in [
        "pub struct HelperExpansionCost",
        "pub struct HelperExpansionLimits",
        "pub enum HelperExpansionError<Error>",
        "pub fn reserve_helper_expansion<Error>",
        "used_nodes: &mut usize",
        "caller_depth: usize",
        "parameter_count: usize",
        "impl FnMut(usize) -> Result<usize, Error>",
        "MAX_TYPE_INFERENCE_NODES",
        "MAX_TYPE_INFERENCE_DEPTH",
        "MAX_FUNCTION_PARAMETERS",
        "checked_add(body.nodes)",
        "checked_add(parameter_count)",
        "checked_add(body.depth)",
        "checked_add(depth)",
        "BodyDepthLimit",
        "ArgumentDepthLimit",
        "parameter_index",
        "*used_nodes = total;",
        "not HelperTemplateShape",
        "already been",
        "A later downstream failure must not undo",
        "trusted adapter observations",
        "never formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing expansion boundary {boundary}"
        );
    }
    let body = source
        .split_once("pub fn reserve_helper_expansion")
        .unwrap()
        .1;
    let observer = body.find("argument_depth(parameter_index)").unwrap();
    for gate in [
        "InvalidLimits",
        "ParameterLimit",
        "EmptyBody",
        "NodeLimit",
        "BodyDepthLimit",
    ] {
        assert!(body.find(gate).unwrap() < observer);
    }
    assert!(body.find("NodeLimit").unwrap() < body.find("BodyDepthLimit").unwrap());
    assert!(observer < body.find("*used_nodes = total;").unwrap());
    for coupling in [
        "use crate::ir",
        "Computation<",
        "Effect::",
        "HostOperation",
        "ResultField",
        "Leserpent",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "Default for",
        "Clone for",
        "OperationCatalog",
        "infer_pure_type(",
        "evaluate_pure(",
        "body.clone()",
        "saturating_add(",
        "Vec::",
        "HashMap",
        "BTreeMap",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected expansion coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_expansion;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    for boundary in [
        "crate::helper_instance_finish::finish_helper_instance(",
        "body: &template.prepared",
        "caller_depth: depth",
        "max_source_nodes: MAX_COMPUTATION_NODES",
        "max_source_depth: MAX_EFFECT_NESTING_DEPTH",
        "max_parameters: MAX_FUNCTION_PARAMETERS",
        ".get(index)",
        "source_cost(value)",
        "LSH1405",
    ] {
        assert!(
            lowerer.contains(boundary),
            "missing expansion adapter {boundary}"
        );
    }
    assert!(!lowerer.contains("*visited += template.nodes"));
    assert!(!lowerer.contains(".saturating_add(template.nodes)"));
    assert!(
        lowerer.find("lower_preflighted_helper_arguments(").unwrap()
            < lowerer.find("finish_helper_instance(").unwrap()
    );
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_expansion.rs"))
        .unwrap();
    for boundary in [
        "append_body_and_wrappers_without_recharging_call_or_operands",
        "node_rejection_precedes_shifted_depth_and_observation",
        "body_depth_overflow_rejects_before_native_argument_work",
        "observers_run_once_in_declaration_order_and_stop_at_first_bad_depth",
        "native_errors_move_once_and_never_enter_formatting_or_source_chains",
        "native_unwind_preserves_counter_without_retry_or_side_effect_rollback",
        "committed_reservations_survive_later_factory_failure_and_unwind",
        "committed_reservation_is_not_refunded_by_wrapper_rejection",
        "move_only_native_operands_are_borrowed_not_cloned_or_owned_by_reservation",
        "folded_and_opaque_source_weights_are_not_physical_template_shape",
        "supplied_metadata_cannot_certify_cold_types_or_actual_output_shape",
        "valid_adapter_inputs_match_legacy_short_circuit_arithmetic_and_observer_order",
        "parsed_declarations_to_reserved_templates_hygiene_and_exact_interpreter_fuel",
        "reference_wire_fresh_names_authority_and_expansion_diagnostics_are_unchanged",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "serde_json::to_vec",
        "before.remaining(), after.remaining()",
    ] {
        assert!(
            proof.contains(boundary),
            "missing expansion proof {boundary}"
        );
    }
}

#[test]
fn shared_closed_group_exports_preflight_cold_routes_before_explicit_native_identity_hooks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/group_exports.rs")).unwrap();
    for boundary in [
        "pub struct GroupExportLimits",
        "pub struct GroupExport<'name, Operation>",
        "pub struct GroupExports<'name, Operation>",
        "pub enum GroupExportPhase",
        "pub enum GroupExportError<Error>",
        "MAX_GROUP_EXPORT_MEMBERS: usize = 64",
        "arguments.len() <= MAX_CALL_ARGUMENTS",
        "valid_argument_name(&argument.name)",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_groups > MAX_TYPE_INFERENCE_NODES",
        "limits.max_members > MAX_GROUP_EXPORT_MEMBERS",
        "check_pending(pending.len(), 1)",
        "let mut known: Option<&Node",
        "!atomic_candidate(&branch.value)",
        "left.name != right.name",
        "signature = Some(current)",
        "name: &member.name",
        "branch(member)",
        "same(&expected.operation, &current.operation)",
        "No native Clone/Debug/serde/Send/PartialEq bound",
        "not full source lowering, flow typing",
        "Native errors are matchable but redacted",
    ] {
        assert!(
            source.contains(boundary),
            "missing group export boundary {boundary}"
        );
    }
    let observe = source
        .split_once("pub fn observe_group_exports<")
        .unwrap()
        .1;
    let gates = [
        "return Err(GroupExportError::InvalidLimits)",
        "physical(expression, limits)?",
        "let leaves = routes(expression, limits)?",
        "for (group_index, node)",
        "host(effect)",
        "if !member_shape(",
        "if let Some(expected) = &signature",
        "same(&expected.operation, &current.operation)",
        "signature = Some(current)",
    ];
    for pair in gates.windows(2) {
        assert!(observe.find(pair[0]).unwrap() < observe.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        "Operation: Clone",
        "Export: Clone",
        "Export: PartialEq",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "canonical_source",
        "catch_unwind",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected group export coupling {coupling}"
        );
    }
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    let adapter = reference
        .split_once("fn group_signature(")
        .unwrap()
        .1
        .split_once("impl Computation")
        .unwrap()
        .0;
    assert!(
        adapter
            .find("expression.validate_structure().ok()?")
            .unwrap()
            < adapter
                .find("crate::group_exports::observe_group_exports(")
                .unwrap()
    );
    assert!(adapter.contains("branch.value.prepared_atomic_operation().ok_or(())"));
    assert!(adapter.contains("HostOperation::for_effect(&branch.effect).ok_or(())?"));
    assert!(adapter.contains("|left, right| Ok(left == right)"));
    assert!(!adapter.contains("let mut pending"));
    assert!(reference.contains("let group_member_count = members.as_ref().map(Vec::len);"));
    assert!(reference.contains("group_member_count.is_none_or(|count|"));
    assert_eq!(reference.matches("group_members(&value,").count(), 1);
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod group_exports;"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/group_exports.rs")).unwrap();
    for boundary in [
        "all_known_cold_group_kinds_names_and_order_match_before_native_queries",
        "cold_call_argument_names_and_count_are_bounded_before_native_operation_queries",
        "zero_loop_and_fold_do_not_hide_effectful_preparation",
        "opaque_observers_borrow_original_graphs_once_and_join_rightmost_first",
        "native_operation_schema_identity_is_explicit_not_same_spelling_or_result_tag",
        "original_branch_return_declaration_is_available_for_native_corroboration",
        "move_only_observations_keep_buffers_without_clone_debug_serde_send_or_equality",
        "all_four_move_only_ir_slots_remain_original_borrowed_objects",
        "branch_error_releases_partial_owned_observations_without_retry_or_consuming_ir",
        "native_unwind_stops_later_hooks_and_drops_observations_without_partial_output",
        "distinct_embedded_host_formats_can_preserve_original_native_schema_identity",
        "parsed_native_source_calls_feed_closed_exports_and_prepare_values_with_exact_schema_and_fuel",
        "parsed_reference_helper_groups_preserve_canonical_wire_and_all_cold_authority",
        "reference_cold_group_reordering_result_forgery_and_nonflat_helpers_are_rejected",
    ] {
        assert!(
            proof.contains(boundary),
            "missing group export proof {boundary}"
        );
    }
}

#[test]
fn shared_selected_helper_instances_reserve_before_copy_and_admit_complete_hygienic_output() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_instance.rs")).unwrap();
    let finish =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_instance_finish.rs"))
            .unwrap();
    for contract in [
        "pub fn lower_helper_instance",
        "pub trait HelperInstanceAdapter",
        "pub struct SelectedHelper",
        "preflight_template(template.parameters(), template.body(), template_limits)",
        "lower_helper_arguments(",
        "collect_names(&argument.value, hygiene, &mut reserved)",
        "reserve_helper_expansion(",
        "hygienic_helper_body(body, &pairs, &reserved, hygiene",
        "bind_helper_arguments(",
        "Ok((value, template.result_type()))",
        "A committed reservation is never refunded",
        "Matching physical shape alone is not semantic identity",
    ] {
        assert!(
            source.contains(contract) || finish.contains(contract),
            "missing shared instance contract {contract}"
        );
    }
    let public = source.split_once("pub fn lower_helper_instance").unwrap().1;
    for forbidden in [
        "Clone",
        "PartialEq",
        "Serialize",
        "Deserialize",
        "Send",
        "unsafe",
        "serde_json",
        "sqlite",
        "leserpent",
        "crate::lower(",
        "catch_unwind",
        "rollback",
    ] {
        assert!(
            !public.contains(forbidden),
            "unexpected instance dependency {forbidden}"
        );
    }
    let stages = [
        "preflight_instance(",
        "lower_helper_arguments(",
        "finish_preflighted_instance(",
    ];
    let mut previous = 0;
    for stage in stages {
        let position = public.find(stage).unwrap();
        assert!(position >= previous, "out-of-order helper stage {stage}");
        previous = position;
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_instance;"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_instance.rs")).unwrap();
    for contract in [
        "parsed_reordered_arguments_run_once_in_declaration_order_with_original_result_and_hygienic_scope",
        "all_six_scalar_results_support_zero_parameter_instances_without_native_metadata_copy",
        "exact_combined_output_and_source_reservations_precede_factory_work",
        "caller_unused_names_and_cold_operand_names_cannot_be_captured_by_parameter_aliases",
        "successful_reservations_survive_later_errors_and_unwind_without_retries_or_partial_publication",
        "factory_shape_changes_and_same_shape_cold_type_corruption_never_publish_an_instance",
        "native_operand_and_materialized_effect_buffers_move_once_while_cached_body_stays_owned",
        "cached_body_under_current_limits_is_checked_before_any_native_argument_work",
        "duplicate_parameter_aliases_and_bad_local_names_stop_before_final_admission",
        "unrelated_host_requires_exact_declared_projection_instead_of_an_unknown_source_fallback",
        "field.buffer.as_ptr(), adapter.argument_buffers[0]",
        "PureValue::Scalar(ScalarValue::Integer(13))",
    ] {
        assert!(
            proof.contains(contract),
            "missing instance proof {contract}"
        );
    }
    assert!(!proof.contains("leselang_hir::lower("));
}

#[test]
fn staged_helper_completion_is_shared_and_the_reference_keeps_one_recursive_counter() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_instance_finish.rs"))
            .unwrap();
    for contract in [
        "pub struct HelperInstanceArgument<Node>",
        "pub trait HelperInstanceFinisher",
        "pub fn finish_helper_instance",
        "fn preflight_instance",
        "fn finish_preflighted_instance",
        "admit_argument",
        "arguments.len() != parameters.len()",
        "argument.scalar_type",
        "ScalarTypeSet::only(*expected)",
        "selected.body.source_cost()",
        "phase: HelperInstancePhase::Argument { index }",
        "used_source_nodes",
        "reserved_names",
        "original borrowed result",
    ] {
        assert!(
            source.contains(contract),
            "missing staged helper contract {contract}"
        );
    }
    let completion = source
        .split_once("pub(crate) fn finish_preflighted_instance")
        .unwrap()
        .1;
    let gates = [
        "arguments.len() != parameters.len()",
        "let hygiene",
        "physical(&argument.value",
        "check_argument_type(",
        ".admit_argument(",
        "reserve_helper_expansion(",
        ".materialize(",
        "let mut aliases",
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        ".admit(",
    ];
    for pair in gates.windows(2) {
        assert!(completion.find(pair[0]).unwrap() < completion.find(pair[1]).unwrap());
    }
    for forbidden in [
        "crate::Type",
        "HostOperation",
        "ResultField",
        "serde::",
        "sqlite",
        "Mutex",
        ".clone()",
        ".unwrap(",
        ".expect(",
        "panic!(",
        "catch_unwind",
        "unsafe",
        "Fuel::",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected completion dependency {forbidden}"
        );
    }
    let original =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_instance.rs")).unwrap();
    assert!(original.contains("finish_preflighted_instance("));
    assert!(!original.contains("reserve_helper_expansion("));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let call = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(call.contains("crate::helper_instance_finish::finish_helper_instance("));
    assert!(!call.contains("prepared: admitted"));
    assert!(reference.contains("prepared: crate::helper_body::HelperBody<Computation, Type>"));
    for obsolete in [
        "nodes: cost.nodes",
        "depth: cost.depth",
        "reserve_helper_expansion(",
        "hygienic_helper_body(",
        "bind_helper_arguments(",
        "let mut copied_counter",
    ] {
        assert!(
            !reference.contains(obsolete),
            "obsolete helper adapter stage {obsolete}"
        );
    }
    assert!(
        call.find("crate::helper_templates::preflight(").unwrap()
            < call.find("lower_preflighted_helper_arguments(").unwrap()
    );
    assert!(
        call.find("lower_preflighted_helper_arguments(").unwrap()
            < call.find("finish_helper_instance(").unwrap()
    );
    assert!(call.contains("visited,\n"));
    assert!(call.contains("value.validate_in_type_scope(self.scope)"));
    let computation =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(computation.contains("fn validate_in_type_scope("));
    assert!(computation.contains("fn validate_scoped("));
    assert!(computation.contains("scope_len > max_bindings"));
    assert!(computation.contains("group.members.len() > max_group_members"));
    assert!(computation.contains("&mut visited, root_depth"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_instance_finish.rs"))
            .unwrap();
    for evidence in [
        "already_compiled_operands_share_the_original_counter_and_once_finish_order",
        "cached_and_combined_limits_precede_operand_native_hooks_or_counter_changes",
        "argument_count_and_all_cold_scalar_facts_precede_every_native_hook",
        "a_complete_cold_argument_forest_is_bounded_before_any_native_type_observation",
        "forged_unused_native_argument_type_is_rejected_before_depth_copy_or_commit",
        "original_move_only_operand_buffer_and_cached_result_observation_are_retained",
        "native_argument_error_and_unwind_release_operands_without_copy_or_commit",
        "all_native_operand_types_are_admitted_before_any_depth_query_or_copy",
        "source_depth_overflow_and_insufficient_budget_leave_the_same_counter_unchanged",
        "later_copy_alias_and_admission_error_or_unwind_never_refund_committed_expansion",
        "zero_parameter_completion_still_copies_and_admits_once_without_operand_hooks",
        "reference_nested_same_helper_operands_groups_and_unused_types_keep_legacy_contracts",
        "reference_deep_fold_prefix_uses_binding_quota_not_expression_root_depth",
        "reference_full_width_caller_group_keeps_exact_last_member_signature",
    ] {
        assert!(
            proof.contains(evidence),
            "missing staged helper proof {evidence}"
        );
    }
}

#[test]
fn shared_helper_body_source_owns_closed_signature_and_original_counter_before_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_body_source.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperBodySourceLimits",
        "pub struct HelperBodyObservation<Node, ResultType>",
        "pub enum HelperBodySourceError<Lowering, Validation, Cost>",
        "pub fn lower_helper_body<",
        "pub(crate) fn lower_preflighted_helper_body<",
        "lower: impl FnOnce(",
        "&[HelperParameter<'source>]",
        "used_source_nodes: &mut usize",
        "valid_limits(source_limits(limits))",
        "limits.body.template.max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "limits.body.template.max_parameters > MAX_FUNCTION_PARAMETERS",
        "limits.body.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.body.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "helper_parameters(function, limits.body.template.max_parameters)",
        "parameters.len() > limits.body.template.max_bindings",
        "used_source_nodes > limits.max_source_nodes",
        "*used_source_nodes > limits.max_source_nodes",
        "SourceCounterLimit",
        "SourceCallError::Shape { span, error }",
        "parameter.name.to_owned(), parameter.domain",
        ".prepare(limits.body, validate, host_cost)",
        "Native accounting is trusted; counter bounds do not prove fidelity",
        "No copied meter, precharge, rollback or retry",
        "not a generic recursive compiler",
    ] {
        assert!(
            source.contains(boundary),
            "missing helper body source boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn lower_helper_body<")
        .unwrap()
        .1
        .split_once("pub(crate) fn lower_preflighted_helper_body<")
        .unwrap()
        .0;
    assert!(
        public
            .find("prepare_header(function, limits, *used_source_nodes)")
            .unwrap()
            < public.find("preflight::<Lowering>").unwrap()
    );
    assert!(public.find("preflight::<Lowering>").unwrap() < public.find("finish(").unwrap());
    let completion = source.split_once("fn finish<").unwrap().1;
    let gates = [
        "lower(source.function, &source.parameters, used_source_nodes)",
        "*used_source_nodes > limits.max_source_nodes",
        "LoweredHelperBody {",
        ".prepare(limits.body, validate, host_cost)",
    ];
    for pair in gates.windows(2) {
        assert!(completion.find(pair[0]).unwrap() < completion.find(pair[1]).unwrap());
    }
    let observation = source
        .split_once("pub struct HelperBodyObservation<Node, ResultType>")
        .unwrap()
        .1
        .split_once("pub enum HelperBodySourceError")
        .unwrap()
        .0;
    assert!(!observation.contains("parameters"));
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "ResultType: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "let mut visited",
        "*used_source_nodes =",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected helper source coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_body_source;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let body = reference
        .split_once("assemble_preflighted_program(")
        .unwrap()
        .1
        .split_once("|main, functions|")
        .unwrap()
        .0;
    for boundary in [
        "lower_preflighted_helper_body(",
        "|function, parameters, visited|",
        "LocalType::from(Type::Scalar(parameter.domain))",
        "&function.body",
        "visited,",
        "let checked = body.validate_in_scope(",
        "if checked != *result_type",
        "host_source_extra(effect, source_limits)",
        "HelperBodySourceError::Lowering(error)",
        "prepared: admitted",
    ] {
        assert!(
            body.contains(boundary),
            "missing product body delegation {boundary}"
        );
    }
    assert!(!body.contains("crate::helper_body::LoweredHelperBody"));
    assert!(!body.contains("parameter_types(function)"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_body_source.rs"))
            .unwrap();
    for evidence in [
        "six_closed_scalar_tokens_and_original_declaration_order_own_the_signature",
        "every_source_output_prefix_and_cost_ceiling_precedes_native_lowering",
        "complete_cold_source_names_text_arity_nodes_and_depth_precede_hooks",
        "lowerer_cannot_replace_signature_or_capture_undeclared_parameters",
        "original_counter_is_borrowed_once_and_successful_overcharge_drops_output_without_refund",
        "once_lowering_validation_and_cost_keep_original_native_buffers_and_owned_policy",
        "validation_cost_errors_and_unwind_release_outputs_without_partial_body_or_counter_refund",
        "lowering_error_and_unwind_preserve_counter_and_never_enter_admission",
        "parsed_scalar_source_preparation_instance_and_value_keep_exact_counter_fuel_and_scope",
        "reference_body_pipeline_keeps_wire_cold_authority_nested_helpers_and_error_priority",
        "returned_scalar_category_still_requires_native_metadata_corroboration",
        "assert_eq!(body_counter, 13)",
        "assert_eq!(caller_counter, 6)",
        "assert_eq!(fuel.remaining(), 95)",
    ] {
        assert!(
            proof.contains(evidence),
            "missing helper body source proof {evidence}"
        );
    }
}

#[test]
fn shared_lowered_helper_body_admission_corroborates_cold_types_before_native_cost_and_storage() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_body.rs")).unwrap();
    for boundary in [
        "pub struct LoweredHelperBody<Node, ResultType>",
        "pub struct HelperBody<Node, ResultType>",
        "pub struct HelperBodyLimits",
        "pub enum HelperBodyError<Validation, Cost>",
        "pub fn into_parts(self)",
        "validate: impl FnOnce(",
        "host_cost: impl FnMut(&HostEffect)",
        "limits.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "PureResultMismatch",
        "struct ClosedScalarParameters;",
        "type Result = ();",
        "PureType::Scalar(*ty)",
        "max_bindings: limits.template.max_bindings",
        "template.result_type()",
        "self.scalar_result",
        "No inferred type is trusted solely",
        "No native Clone/Debug/serde/Send bound",
        "Cost limits/failure do not undo earlier validation work",
        "Native errors remain matchable, not formatting or source chains",
    ] {
        assert!(
            source.contains(boundary),
            "missing body admission boundary {boundary}"
        );
    }
    let prepare = source
        .split_once("pub fn prepare<Validation, Cost>")
        .unwrap()
        .1;
    let gates = [
        "limits.source_cost.max_nodes >",
        "let template = HelperTemplate::new(",
        "if template.body().is_pure()",
        "let inferred = infer_pure_type(",
        "if inferred != declared",
        "validate(",
        "let source_cost = measure_source_cost(",
        "Ok(HelperBody {",
    ];
    for pair in gates.windows(2) {
        assert!(prepare.find(pair[0]).unwrap() < prepare.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "ResultType: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "canonical_source",
        "catch_unwind",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected body admission coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_body;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let gates = [
        "crate::helper_body_source::lower_preflighted_helper_body(",
        "computation::lower_expression_with_functions(",
        "crate::helper_body_source::HelperBodyObservation",
        "let checked = body.validate_in_scope(",
        "if checked != *result_type",
        ".insert(function.name.clone(), Template",
        "prepared: admitted",
    ];
    for pair in gates.windows(2) {
        assert!(reference.find(pair[0]).unwrap() < reference.find(pair[1]).unwrap());
    }
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_body.rs")).unwrap();
    for boundary in [
        "every_scalar_domain_is_independently_inferred_before_native_validation",
        "forged_pure_return_observation_is_denied_before_any_native_callback",
        "zero_loop_and_fold_next_bodies_are_still_typed",
        "cold_physical_and_lexical_preflight_precedes_purity_and_callbacks",
        "pure_native_projection_cannot_invent_a_result_from_scalar_parameters",
        "native_validation_corroborates_result_metadata_after_shared_scalar_inference",
        "all_move_only_native_slots_and_owned_buffers_are_handed_off_unchanged",
        "complete_cold_host_validation_precedes_rightmost_first_cost_observations",
        "validation_error_prevents_cost_and_partial_registry_entry_and_drops_inputs_once",
        "cost_error_retains_private_error_without_retry_refund_or_partial_output",
        "validation_and_cost_unwind_drop_owned_inputs_once_without_catching_or_retrying",
        "parsed_scalar_helper_admission_materialization_binding_and_returns_keep_value_and_exact_fuel",
        "reference_helpers_preserve_canonical_wire_authority_and_unused_cold_rejection",
    ] {
        assert!(
            proof.contains(boundary),
            "missing body admission proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_templates_preflight_once_factories_without_native_copy_or_authority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_templates.rs")).unwrap();
    for boundary in [
        "pub struct HelperTemplate<Node, ResultType>",
        "parameters: Vec<(String, ScalarType)>",
        "body: Node,",
        "result_type: ResultType,",
        "pub struct HelperTemplateLimits",
        "pub struct HelperTemplateShape",
        "pub enum HelperTemplateError<Error>",
        "Body(HelperHygieneError<Infallible>)",
        "limits.max_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_depth > MAX_TYPE_INFERENCE_DEPTH",
        "limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "parameters.len() > limits.max_parameters",
        "parameters.len() > limits.max_bindings",
        "!valid_local_name(name) || !names.insert(name.to_owned())",
        "pub fn parameters(&self) -> &[(String, ScalarType)]",
        "pub fn body(&self) -> &Node",
        "pub fn result_type(&self) -> &ResultType",
        "pub fn into_parts(self) -> (Vec<(String, ScalarType)>, Node, ResultType)",
        "(self.parameters, self.body, self.result_type)",
        "factory: impl FnOnce(",
        "No operands, aliases, result receipt",
        "needs fresh caller-owned admission and expansion accounting",
    ] {
        assert!(
            source.contains(boundary),
            "missing template boundary {boundary}"
        );
    }
    let preflight = source.split_once("fn preflight<").unwrap().1;
    assert!(preflight.find("collect_names(").unwrap() < preflight.find("check_scope(").unwrap());
    let materialize = source.split_once("pub fn materialize<Error>").unwrap().1;
    let gates = [
        "preflight(&self.parameters, &self.body, limits)",
        "factory(&self.body).map_err(HelperTemplateError::Factory)",
        "preflight(&self.parameters, &body, limits)",
        "if expected != actual",
        "Ok(body)",
    ];
    for pair in gates.windows(2) {
        assert!(materialize.find(pair[0]).unwrap() < materialize.find(pair[1]).unwrap());
    }
    let debug = source
        .split_once("fmt::Debug for HelperTemplate<Node, ResultType>")
        .unwrap()
        .1
        .split_once("impl<Node, ResultType> HelperTemplate<Node, ResultType>")
        .unwrap()
        .0;
    assert!(debug.contains("&self.parameters.len()"));
    assert!(debug.contains("&self.shape"));
    assert!(!debug.contains("&self.body"));
    assert!(!debug.contains("&self.result_type"));
    for coupling in [
        ".clone()",
        "Field: Clone",
        "Operation: Clone",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "FnMut",
        "canonical_source",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected template coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_templates;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    assert!(reference.contains("prepared: crate::helper_body::HelperBody<Computation, Type>"));
    let checked = reference.find("body.validate_in_scope(").unwrap();
    let stored = reference.find("prepared: admitted").unwrap();
    assert!(
        reference
            .find("crate::helper_body_source::HelperBodyObservation")
            .unwrap()
            < checked
    );
    assert!(checked < reference.find("if checked != *result_type").unwrap());
    assert!(checked < stored);
    assert!(
        reference
            .find(".insert(function.name.clone(), Template")
            .unwrap()
            < stored
    );
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("cached.parameters().to_vec()"));
    assert!(lowerer.contains("Ok(body.clone())"));
    assert!(lowerer.contains("Ok((body, *result_type))"));
    let materialize = lowerer.find("finish_helper_instance(").unwrap();
    assert!(lowerer.find("lower_preflighted_helper_arguments(").unwrap() < materialize);
    assert!(
        materialize
            < lowerer
                .find("fresh_name(self.names, self.next_name)")
                .unwrap()
    );
    assert!(materialize < lowerer.find("fn admit(&mut self").unwrap());
    assert!(!lowerer.contains("crate::helper_bindings::bind_helper_arguments("));
    let proof = std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_templates.rs"))
        .unwrap();
    for boundary in [
        "owned_signature_body_and_return_metadata_move_unchanged_and_can_be_consumed",
        "zero_loop_and_fold_paths_still_have_complete_lexical_preflight",
        "physical_frontier_and_depth_bounds_precede_lexical_scope_walks",
        "debug_and_factory_error_never_dump_private_names_literals_or_native_metadata",
        "narrower_call_limits_reject_before_the_factory_even_for_cold_bindings",
        "factory_receives_exact_original_borrow_once_and_can_move_gui_local_slots",
        "returned_free_locals_and_parameter_shadowing_fail_without_partial_output",
        "smaller_or_differently_nested_output_cannot_change_reserved_physical_shape",
        "expanded_factory_output_is_bounded_and_its_native_slots_drop_once",
        "factory_error_and_unwind_preserve_cache_without_retry_or_native_side_effect_rollback",
        "cached_opaque_host_call_group_and_result_vectors_are_owned_without_rebuilding",
        "equal_shape_does_not_certify_literal_operator_or_declared_return_type_identity",
        "repeated_explicit_materialization_matches_original_value_and_exact_interpreter_fuel",
        "parsed_helper_pipeline_owns_materializes_and_prepares_an_unrelated_native_call",
        "body_parameter_types_and_source_expansion_weights_are_not_template_shape_certificates",
        "reference_repeated_nested_templates_keep_wire_diagnostics_and_cold_authority",
        "std::ptr::eq",
        "bytes.as_ptr()",
        "catch_unwind",
        "prepare_call_in_scope",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing template proof {boundary}"
        );
    }
}

#[test]
fn shared_owned_helper_registry_keeps_lazy_preparation_and_complete_native_state_handoff() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_registry.rs")).unwrap();
    for boundary in [
        "pub struct HelperRegistryLimits",
        "pub enum HelperRegistryError<Lowering, Registration>",
        "pub type HelperRegistryResult<State, Lowering, Registration>",
        "pub fn assemble_helper_registry<",
        "declarations: &HelperDeclarations<'source>",
        "mut state: State",
        "mut prepare: impl FnMut(",
        "mut register: impl FnMut(",
        "&'source Function",
        "&mut State",
        "declarations.helpers()",
        "&declarations.entry().name",
        "for function in order",
        "helper_parameters(function, limits.template.max_parameters)",
        "parameters.len() > limits.template.max_bindings",
        "parameters.len() != template.parameters().len()",
        "declared.name != name || declared.domain != *ty",
        "preflight(template.parameters(), template.body(), limits.template)",
        "not transactional rollback",
        "not complete generic recursive source compilation",
        "callbacks must not dispatch or publish execution",
        "prepare must",
        "copied",
        "instances and dispatch need fresh validation",
    ] {
        assert!(
            source.contains(boundary),
            "missing registry boundary {boundary}"
        );
    }
    let assembly = source
        .split_once("pub fn assemble_helper_registry<")
        .unwrap()
        .1;
    let order = assembly
        .find("let order = helper_dependency_order(")
        .unwrap();
    for ceiling in [
        "limits.dependencies.max_helpers >",
        "limits.dependencies.max_source_nodes >",
        "limits.dependencies.max_source_depth >",
        "limits.template.max_nodes >",
        "limits.template.max_depth >",
        "limits.template.max_bindings >",
        "limits.template.max_parameters >",
        "limits.source_cost.max_nodes >",
        "limits.source_cost.max_depth >",
    ] {
        assert!(assembly.find(ceiling).unwrap() < order);
    }
    let gates = [
        "function.map_err(HelperRegistryError::Dependency)",
        "helper_parameters(function",
        "parameters.len() > limits.template.max_bindings",
        "prepare(function, &mut state)",
        "parameters.len() != template.parameters().len()",
        "preflight(template.parameters()",
        "cost.depth > limits.source_cost.max_depth",
        "cost.nodes > limits.source_cost.max_nodes",
        "register(function, body, &mut state)",
        "Ok(state)",
    ];
    for pair in gates.windows(2) {
        assert!(assembly.find(pair[0]).unwrap() < assembly.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        ".collect",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "RwLock",
        "Arc<",
        "BTreeMap",
        "fn fresh_name",
        "std::mem::take",
        "State: Clone",
        "State: Send",
        "Lowering: std::fmt",
        "Registration: std::fmt",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected registry coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_registry;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let assembly = reference
        .split_once("crate::program_source::assemble_preflighted_program(")
        .unwrap()
        .1
        .split_once("let (main, lowered, _functions)")
        .unwrap()
        .0;
    for boundary in [
        "|function, functions|",
        "crate::helper_body_source::lower_preflighted_helper_body(",
        "|function, parameters, visited|",
        "computation::lower_expression_with_functions(",
        "|function, admitted, functions|",
        ".insert(function.name.clone(), Template { prepared: admitted })",
        "HelperRegistryError::Dependency(error) => dependency_error(error, span)",
        "HelperRegistryError::Lowering { error, .. } => error",
        "HelperRegistryError::Registration { error, .. } => match error {}",
    ] {
        assert!(
            assembly.contains(boundary),
            "missing product registry delegation {boundary}"
        );
    }
    assert!(
        reference.contains("let assembled = crate::program_source::assemble_preflighted_program(")
    );
    assert!(
        reference.contains("computation::lower_computation_with_functions(&main.body, functions)")
    );
    assert!(!reference.contains("for function in order"));
    assert!(!reference.contains("helper_dependency_order("));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_registry.rs")).unwrap();
    for evidence in [
        "borrowed_ready_order_registers_each_helper_before_the_next_without_compiling_the_entry",
        "every_dependency_template_and_cost_ceiling_precedes_native_callbacks",
        "whole_helper_source_and_cold_entry_calls_fail_before_any_preparation",
        "current_parameter_and_unused_prefix_limits_precede_the_selected_native_prepare",
        "prepared_signatures_cannot_substitute_reorder_rename_or_change_domains_before_registration",
        "current_template_and_cached_cost_bounds_precede_native_storage_with_depth_first_priority",
        "original_body_boxes_signature_result_buffers_and_cost_move_into_native_storage_once",
        "ready_native_failure_precedes_a_later_cycle_without_eager_order_collection",
        "late_cycle_drops_already_registered_owned_state_without_returning_a_partial_registry",
        "later_native_errors_and_unwind_drop_state_and_inputs_without_retry_refund_or_payload_formatting",
        "empty_registry_returns_original_state_under_explicit_zero_policies_without_callbacks",
        "helper_count_limits_and_unknown_calls_do_not_grant_native_lookup_or_skip_unused_helpers",
        "unrelated_scalar_registry_composes_declarations_bodies_instances_and_entry_with_exact_fuel",
        "reference_registry_delegation_preserves_nested_helper_wire_authority_and_ready_error_precedence",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "fuel.remaining(), 97",
    ] {
        assert!(
            proof.contains(evidence),
            "missing registry proof {evidence}"
        );
    }
}

#[test]
fn shared_program_assembly_owns_complete_state_entry_and_mandatory_final_admission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/program_source.rs")).unwrap();
    for boundary in [
        "pub struct ProgramSourceLimits",
        "pub enum ProgramSourceError<Lowering, Registration, Entry, Admission>",
        "pub struct ProgramSource<'source, Output, State>",
        "entry: &'source Function",
        "output: Output",
        "state: State",
        "pub fn into_parts(self) -> (&'source Function, Output, State)",
        "pub fn assemble_program<",
        "pub(crate) fn assemble_preflighted_program<",
        "declarations: &HelperDeclarations<'source>",
        "lower_entry: impl FnOnce",
        "admit: impl FnOnce",
        "&Output, &mut State",
        "max_source_nodes: limits.max_entry_source_nodes",
        "max_source_depth: limits.max_entry_source_depth",
        "max_arguments: limits.max_entry_arguments",
        "preflight::<Entry>(&declarations.entry().body, source)",
        "Native output is opaque: the host MUST",
        "not transactional rollback of external storage",
        "State may move between assembly stages",
        "no stable-address or pinning promise",
        "not a complete generic recursive compiler or independent VM",
    ] {
        assert!(
            source.contains(boundary),
            "missing program assembly boundary {boundary}"
        );
    }
    let public = source
        .split_once("pub fn assemble_program<")
        .unwrap()
        .1
        .split_once("pub(crate) fn assemble_preflighted_program<")
        .unwrap()
        .0;
    assert!(
        public.find("!valid_limits(source)").unwrap() < public.find("preflight::<Entry>").unwrap()
    );
    assert!(
        public.find("preflight::<Entry>").unwrap()
            < public.find("assemble_preflighted_program(").unwrap()
    );
    let assembly = source
        .split_once("pub(crate) fn assemble_preflighted_program<")
        .unwrap()
        .1;
    let gates = [
        "!valid_limits(entry_limits(limits))",
        "assemble_helper_registry(declarations, limits.helpers, state, prepare, register)",
        ".map_err(ProgramSourceError::Helpers)",
        "let entry = declarations.entry()",
        "lower_entry(entry, &mut state)",
        "admit(entry, &output, &mut state)",
        "Ok(ProgramSource",
    ];
    for pair in gates.windows(2) {
        assert!(assembly.find(pair[0]).unwrap() < assembly.find(pair[1]).unwrap());
    }
    for coupling in [
        ".clone()",
        ".collect",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "serde::",
        "sqlite",
        "Fuel::",
        "catch_unwind",
        "RefCell",
        "Mutex",
        "RwLock",
        "Arc<",
        "BTreeMap",
        "State: Clone",
        "Output: Clone",
        "Output: Send",
        "std::mem::take",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected program assembly coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod program_source;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    let assembly = reference
        .split_once("let assembled = crate::program_source::assemble_preflighted_program(")
        .unwrap()
        .1
        .split_once("let (main, lowered, _functions) = assembled.into_parts()")
        .unwrap()
        .0;
    for boundary in [
        "helpers: crate::helper_registry::HelperRegistryLimits",
        "|function, functions|",
        "|function, admitted, functions|",
        "|main, functions|",
        "computation::lower_computation_with_functions(&main.body, functions)",
        "|main, lowered, _|",
        "canonical_source(&lowered.effect)",
        "ProgramSourceError::Helpers(error)",
        "ProgramSourceError::Entry { error, .. }",
        "ProgramSourceError::Admission { error, .. }",
    ] {
        assert!(
            assembly.contains(boundary),
            "missing reference program assembly {boundary}"
        );
    }
    assert!(
        assembly.find("|function, admitted, functions|").unwrap()
            < assembly.find("|main, functions|").unwrap()
    );
    assert!(
        assembly.find("|main, functions|").unwrap() < assembly.find("|main, lowered, _|").unwrap()
    );
    assert_eq!(
        assembly
            .matches("canonical_source(&lowered.effect)")
            .count(),
        1
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/program_source.rs")).unwrap();
    for evidence in [
        "original_custom_entry_runs_after_every_ready_helper_then_whole_output_admission_once",
        "all_entry_source_ceilings_precede_helper_entry_and_admission_callbacks",
        "cold_entry_names_text_arity_frontier_and_depth_fail_before_any_native_helper_work",
        "current_helper_ceilings_and_dependency_errors_never_reach_the_entry",
        "late_cycle_drops_completed_helper_state_without_entry_lowering_or_partial_program",
        "ready_helper_native_errors_keep_original_payload_span_and_precede_later_cycle",
        "entry_error_and_unwind_drop_owned_registry_without_admission_retry_or_counter_refund",
        "final_admission_error_and_unwind_drop_original_output_and_state_without_partial_handoff",
        "once_entry_and_admission_closures_move_private_buffers_into_and_out_of_original_state",
        "empty_helper_zero_policies_still_lower_and_admit_a_parameterless_leaf_entry",
        "zero_entry_nodes_reject_before_helpers_even_when_the_helper_policy_is_empty",
        "unknown_entry_calls_and_stale_native_output_need_explicit_fresh_host_policy",
        "unrelated_scalar_program_composes_registry_entry_admission_and_value_with_exact_fuel",
        "reference_program_assembly_keeps_canonical_wire_capabilities_and_helper_before_entry_errors",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "fuel.remaining(), 97",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(evidence),
            "missing program assembly proof {evidence}"
        );
    }
}

#[test]
fn shared_helper_declaration_admission_borrows_complete_forests_without_product_policy() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/helper_declarations.rs"))
            .unwrap();
    for boundary in [
        "pub struct HelperDeclarationLimits",
        "pub enum HelperDeclarationError<Error>",
        "pub struct HelperDeclarations<'source>",
        "entry: &'source Function",
        "helpers: Vec<&'source Function>",
        "reserved_names: HashSet<&'source str>",
        "pub fn accept_helper_declarations<'source, Error>",
        "mut reserved: impl FnMut(&str) -> Result<bool, Error>",
        "limits.max_functions > MAX_FUNCTIONS",
        "limits.max_parameters > MAX_FUNCTION_PARAMETERS",
        "limits.max_source_nodes > MAX_TYPE_INFERENCE_NODES",
        "limits.max_source_depth > MAX_TYPE_INFERENCE_DEPTH",
        "declarations.len() > limits.max_functions",
        "!valid_local_name(entry)",
        "helper_parameters(function, limits.max_parameters)",
        "budget.visit(depth, 0, 0)",
        "arguments.len()",
        "callee == \"fold\"",
        "argument.name == \"item\"",
        "names.insert(value)",
        "names.insert(name)",
        "!entry.parameters.is_empty()",
        "Self::Policy { .. } => \"native declaration name policy failed\"",
        "No names, AST nodes or literal buffers are copied",
    ] {
        assert!(
            source.contains(boundary),
            "missing declaration boundary {boundary}"
        );
    }
    let admission = source
        .split_once("pub fn accept_helper_declarations")
        .unwrap()
        .1;
    let policy = admission.find("reserved(&function.name)").unwrap();
    for check in [
        "limits.max_functions",
        "declarations.len()",
        "!valid_local_name(entry)",
        "!valid_local_name(&function.name)",
    ] {
        assert!(admission.find(check).unwrap() < policy);
    }
    let gates = [
        "reserved(&function.name)",
        "declarations[..declaration_index]",
        "helper_parameters(function",
        "collect_source_names(function",
        "let entry_index",
        "!entry.parameters.is_empty()",
        "Ok(HelperDeclarations",
    ];
    for pair in gates.windows(2) {
        assert!(admission.find(pair[0]).unwrap() < admission.find(pair[1]).unwrap());
    }
    let scan = source
        .split_once("fn collect_source_names")
        .unwrap()
        .1
        .split_once("/// Admit headers")
        .unwrap()
        .0;
    assert!(scan.find(".check_pending(").unwrap() < scan.find("names.extend(").unwrap());
    assert!(scan.find(".check_pending(").unwrap() < scan.find("pending.extend(").unwrap());
    for coupling in [
        ".clone()",
        "to_owned()",
        "crate::Type",
        "ResultField",
        "HostOperation",
        "OperationCatalog",
        "infer_pure_type",
        "evaluate_pure",
        "canonical_source",
        "serde::",
        "sqlite",
        "Fuel::",
        "Mutex",
        "fn fresh_name",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected declaration coupling {coupling}"
        );
    }
    let modules = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(modules.contains("pub mod helper_declarations;"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/functions.rs")).unwrap();
    for boundary in [
        "crate::helper_declarations::accept_helper_declarations(",
        "computation::is_builtin(name)",
        "accepted.reserved_names().map(str::to_owned)",
        "let (main, lowered, _functions) = assembled.into_parts()",
        "crate::program_source::assemble_preflighted_program(",
        "parameter_diagnostics(error, span)",
    ] {
        assert!(
            reference.contains(boundary),
            "missing declaration adapter {boundary}"
        );
    }
    assert!(!reference.contains("let mut declared = BTreeMap"));
    assert!(!reference.contains("let mut pending = vec![(&function.body"));
    assert!(
        reference.find("accept_helper_declarations(").unwrap()
            < reference.find("assemble_preflighted_program(").unwrap()
    );
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/helper_declarations.rs"))
            .unwrap();
    for boundary in [
        "custom_entry_retains_original_declarations_source_order_and_borrowed_buffers",
        "invalid_ceilings_count_and_entry_reject_before_any_native_policy",
        "reserved_policy_is_explicit_once_per_valid_header_and_precedes_duplicate_check",
        "signature_errors_keep_original_parameter_spans_and_precede_physical_body_errors",
        "missing_and_parameterized_entry_checks_follow_the_complete_cold_forest",
        "cold_frontier_is_bounded_before_name_inventory_or_child_stack_growth",
        "reservation_inventory_includes_cold_fold_labels_but_not_callees_or_literals",
        "native_policy_failure_and_unwind_return_no_partial_view_or_retry",
        "admission_does_not_certify_native_calls_entry_cycles_literals_or_cold_types",
        "accepted_borrowed_headers_compose_with_shared_templates_hygiene_and_exact_fuel",
        "reference_declaration_diagnostics_wire_names_and_cold_authority_are_unchanged",
        "std::ptr::eq",
        "as_ptr()",
        "catch_unwind",
        "HelperTemplate::new",
        "hygienic_helper_body",
        "bind_helper_arguments",
        "before.remaining(), after.remaining()",
        "serde_json::to_vec",
    ] {
        assert!(
            proof.contains(boundary),
            "missing declaration proof {boundary}"
        );
    }
}

#[test]
fn native_graph_inspection_borrows_original_slots_and_stays_separate_from_native_typing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/native_graph.rs")).unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for boundary in [
        "pub struct NativeGraphLimits",
        "pub enum NativeGraphShape<'graph, Branch>",
        "members: &'graph [Branch]",
        "pub struct NativeGraphSummary",
        "pub enum NativeGraphPhase",
        "pub enum NativeGraphError<Error>",
        "Node: ?Sized + 'graph",
        "limits.max_nodes > MAX_NATIVE_GRAPH_NODES",
        "limits.max_depth > MAX_NATIVE_GRAPH_DEPTH",
        "limits.max_members > MAX_NATIVE_GRAPH_MEMBERS",
        "let node_index = budget.visited()",
        "frames.last_mut()",
        "frame.members.get(frame.next)",
        "NativeGraphPhase::View",
        "NativeGraphPhase::Child",
        "NativeGraphPhase::Leaf",
        "No quota silently expands",
        "one borrowed sibling cursor per open group",
        "this entry does not promise that all physical errors precede inline leaf work",
        "without retries or partial summary",
    ] {
        assert!(
            source.contains(boundary),
            "missing graph boundary {boundary}"
        );
    }
    assert!(
        compact
            .contains("pubfninspect_native_graph<'graph,Node:?Sized+'graph,Branch:'graph,Error>")
    );
    assert!(compact.contains("letminimum=ifkind==GroupKind::Sequence{1}else{2}"));
    let inspection = source
        .split_once("pub fn inspect_native_graph")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    assert!(inspection.find("budget.visit(").unwrap() < inspection.find("view(node)").unwrap());
    assert!(
        inspection.find("limits.max_members).contains").unwrap()
            < inspection.find("child(member)").unwrap()
    );
    assert!(inspection.find("leaf(node)").unwrap() < inspection.find("child(member)").unwrap());
    for coupling in [
        ".clone()",
        "pending.extend",
        "serde::",
        "use serde",
        "crate::Type",
        "HostOperation",
        "OperationCatalog",
        "ResultField",
        "infer_pure_type",
        "evaluate_pure",
        "canonical_source",
        "Fuel::",
        "Send +",
        "PartialEq +",
    ] {
        assert!(
            !source.contains(coupling),
            "unexpected graph coupling {coupling}"
        );
    }
    let reference = std::fs::read_to_string(root.join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(reference.contains("pub mod native_graph;"));
    let adapter = reference
        .split_once("fn validate_canonical_effect_shape")
        .unwrap()
        .1
        .split_once("fn canonical_effect_source")
        .unwrap()
        .0;
    for boundary in [
        "inspect_native_graph(",
        "max_nodes: MAX_CANONICAL_EFFECT_NODES",
        "max_depth: MAX_EFFECT_NESTING_DEPTH - 1",
        "max_members: MAX_ALL_BRANCHES",
        "ir::GroupKind::Parallel",
        "ir::GroupKind::Sequence",
        "|branch: &HirBranch| Ok(&branch.effect)",
        "computation::validate_shape(expression)",
        "\"LSH1201\"",
        "\"LSH1204\"",
        "\"LSH1205\"",
        "NativeGraphError::Native { error, .. } => error",
        "span: None",
    ] {
        assert!(
            adapter.contains(boundary),
            "missing product graph adapter {boundary}"
        );
    }
    assert!(!adapter.contains("pending"));
    assert!(!adapter.contains("StructureBudget::new"));
    assert!(reference.contains(
        "shared_native_walk_preserves_inline_computation_error_before_later_group_errors"
    ));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_graph.rs")).unwrap();
    for boundary in [
        "nested_gui_graphs_borrow_original_nonclone_slices_slots_and_declaration_order",
        "node_depth_and_member_limits_are_inclusive_and_zero_has_no_expanding_default",
        "invalid_group_arity_stops_before_any_original_child_mapping",
        "repeated_original_edges_count_per_occurrence_without_deduplication",
        "cyclic_native_views_are_bounded_without_recursive_calls_or_cycle_certificates",
        "private_native_errors_keep_positions_and_phase_but_never_format_the_payload",
        "native_callback_unwind_never_retries_consumes_graphs_or_publishes_partial_counts",
        "inline_leaf_checks_preserve_first_failure_without_claiming_whole_cold_admission",
        "successful_counts_do_not_certify_changed_graphs_leaf_payloads_or_live_grants",
        "depth_before_nodes_and_original_payload_free_debug_are_not_native_formatters",
        "unsized_gui_local_nodes_and_nonclone_branch_slots_need_no_box_conversion",
    ] {
        assert!(proof.contains(boundary), "missing graph proof {boundary}");
    }
    let product_proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/native_graph_reference.rs"))
            .unwrap();
    for boundary in [
        "existing_nested_native_parallel_profiles_keep_canonical_wire_order_and_capability_checks",
        "physical_native_graph_counts_do_not_certify_names_types_or_private_payload_domains",
        "shared_structure_inspection_does_not_enable_nested_exports_or_forbidden_sequence_profiles",
        "compute_leaf_structure_stays_separate_from_its_semantic_type_and_native_graph_budget",
        "graph_arity_and_depth_keep_legacy_diagnostics_before_source_formatting",
    ] {
        assert!(
            product_proof.contains(boundary),
            "missing product graph proof {boundary}"
        );
    }
}
