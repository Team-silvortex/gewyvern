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
    assert!(reference.contains("lowered.into_arguments()"));
    assert!(!reference.contains("let bindings = operation.bind_names(&names"));
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
    assert!(source.contains("std::ptr::eq(first.schema, call.schema)"));
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
    assert!(reference.contains("crate::scalar_source::lower_choose_form("));
    assert!(reference.contains("crate::scalar_source::binding_source(expression)"));
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
        "preflight_with_budget(&value, 1, &mut budget)",
        "let PureType::Result(result) = input_type",
        "valid_local_name(self.group)",
        "valid_member_name(self.name)",
        "source.construct(operation)",
        "source.construct(value, field)",
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
    assert!(
        public.find("preflight_with_budget(&value").unwrap()
            < public.find(".field(&result").unwrap()
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
    assert!(lowerer.contains("crate::projection_source::member_source(arguments, span)"));
    assert!(lowerer.contains(".literal_name()"));
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
        "crate::helper_hygiene::hygienic_helper_body(",
        ".materialize(template_limits(), |body|",
        "functions.fresh_name()",
        "crate::helper_bindings::bind_helper_arguments(",
        "max_reserved_names: 0",
        "helper cannot capture caller locals",
        "helper cannot capture a caller group",
    ] {
        assert!(
            lowerer.contains(boundary),
            "missing reference helper boundary {boundary}"
        );
    }
    assert!(
        lowerer
            .find("crate::helper_expansion::reserve_helper_expansion(")
            .unwrap()
            < lowerer.find(".materialize(template_limits()").unwrap()
    );
    assert!(
        lowerer.find(".materialize(template_limits()").unwrap()
            < lowerer
                .find("crate::helper_hygiene::hygienic_helper_body(")
                .unwrap()
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
    let parameter_adapter = reference
        .split_once("fn parameter_types")
        .unwrap()
        .1
        .split_once("pub(super) fn lower_program")
        .unwrap()
        .0;
    assert!(
        parameter_adapter
            .contains("crate::helper_source::helper_parameters(function, MAX_FUNCTION_PARAMETERS)")
    );
    assert!(!parameter_adapter.contains("parameter.type_name.as_str()"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("crate::helper_source::lower_preflighted_helper_arguments("));
    assert!(!lowerer.contains("value.is_pure()"));
    assert!(
        lowerer.find("lower_preflighted_helper_arguments(").unwrap()
            < lowerer.find(".materialize(template_limits()").unwrap()
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
        "crate::helper_dependencies::helper_dependency_order(",
        "max_helpers: MAX_FUNCTIONS - 1",
        "max_source_nodes: MAX_COMPUTATION_NODES",
        "max_source_depth: MAX_EFFECT_NESTING_DEPTH",
        "for function in order",
        "computation::is_builtin(name)",
        "parameter_types(function)?",
        "main cannot have parameters",
        ".materialize(template_limits(), |body|",
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
    let lowerer = reference.split_once("for function in order").unwrap().1;
    assert!(
        lowerer.find("function.map_err(").unwrap()
            < lowerer.find("lower_expression_with_functions(").unwrap()
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
    let wrap = lowerer
        .find("crate::helper_bindings::bind_helper_arguments(")
        .unwrap();
    for gate in [
        "crate::helper_expansion::reserve_helper_expansion(",
        "source_cost(value).map(|cost| cost.depth)",
        ".materialize(template_limits(), |body|",
        "crate::helper_hygiene::hygienic_helper_body(",
    ] {
        assert!(lowerer.find(gate).unwrap() < wrap);
    }
    assert!(lowerer.contains("max_nodes: MAX_COMPUTATION_NODES"));
    assert!(lowerer.contains("max_depth: MAX_EFFECT_NESTING_DEPTH"));
    assert!(lowerer.contains("max_parameters: MAX_FUNCTION_PARAMETERS"));
    assert!(lowerer.contains(".zip(values)"));
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
    assert!(reference.contains("nodes: cost.nodes"));
    assert!(reference.contains("depth: cost.depth"));
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
    assert!(flow.contains("functions::source_cost(&value).map_err(oversized)?"));
    assert!(flow.contains("functions::source_cost(&body).map_err(oversized)?"));
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
        "crate::helper_expansion::reserve_helper_expansion(",
        "nodes: template.nodes",
        "depth: template.depth",
        "parameters.len()",
        "max_nodes: MAX_COMPUTATION_NODES",
        "max_depth: MAX_EFFECT_NESTING_DEPTH",
        "max_parameters: MAX_FUNCTION_PARAMETERS",
        ".get(index)",
        "source_cost(value).map(|cost| cost.depth)",
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
        lowerer.find("reserve_helper_expansion(").unwrap()
            < lowerer.find(".materialize(template_limits()").unwrap()
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
    assert!(
        reference.contains("prepared: crate::helper_templates::HelperTemplate<Computation, Type>")
    );
    let checked = reference.find("body.validate_in_scope(").unwrap();
    let stored = reference
        .find("crate::helper_templates::HelperTemplate::new(")
        .unwrap();
    assert!(checked < stored);
    assert!(stored < reference.find("functions.templates.insert(").unwrap());
    assert!(
        reference.contains("crate::source_cost::measure_source_cost(expression, limits, |effect|")
    );
    assert!(reference.contains("let mut effects = vec![(effect, 1)];"));
    let lowerer = reference.split_once("pub(super) fn lower_call").unwrap().1;
    assert!(lowerer.contains("template.prepared.parameters().to_vec()"));
    assert!(lowerer.contains("Ok::<_, std::convert::Infallible>(body.clone())"));
    assert!(lowerer.contains("*template.prepared.result_type()"));
    let materialize = lowerer
        .find(".materialize(template_limits(), |body|")
        .unwrap();
    assert!(
        lowerer
            .find("crate::helper_expansion::reserve_helper_expansion(")
            .unwrap()
            < materialize
    );
    assert!(materialize < lowerer.find("functions.fresh_name()").unwrap());
    assert!(
        materialize
            < lowerer
                .find("crate::helper_hygiene::hygienic_helper_body(")
                .unwrap()
    );
    assert!(
        materialize
            < lowerer
                .find("crate::helper_bindings::bind_helper_arguments(")
                .unwrap()
    );
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
        "let main = accepted.entry()",
        "accepted.helpers()",
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
            < reference.find("helper_dependency_order(").unwrap()
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
