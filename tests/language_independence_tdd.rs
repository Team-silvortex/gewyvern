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
        "preflight(expression, limits)",
        "preflight_names(expression)",
        "preflight_with_budget(&value",
        "BinaryOperator::parse",
        "UnaryOperator::parse",
        "ScalarSourceError::InconsistentLiteral",
        "type OperandResult",
        "if depth > max_depth",
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
            < public.find("let output = build(").unwrap()
    );
    assert!(!source.contains("apply_binary("));
    assert!(!source.contains("apply_unary("));
    assert!(!source.contains("Some(text.clone())"));
    assert!(!source.contains("pub enum Computation"));
    let reference =
        std::fs::read_to_string(root.join("crates/leselang-hir/src/computation.rs")).unwrap();
    assert!(reference.contains("crate::scalar_source::primitive(expression)"));
    assert!(reference.contains("crate::scalar_source::lower_form(expression"));
    assert!(!reference.contains("fn unary_type("));
    assert!(!reference.contains("if let Some(operator) = BinaryOperator::parse(callee)"));
    assert!(!reference.contains("Some(text.clone())"));
    let proof =
        std::fs::read_to_string(root.join("crates/leselang-hir/tests/source_call.rs")).unwrap();
    assert!(proof.contains("leselang_hir::scalar_source::lower_scalar_source"));
    assert!(!proof.contains("fn node(expression: &Expression)"));
}
