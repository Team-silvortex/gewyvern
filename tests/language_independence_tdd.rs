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
    assert!(flow.contains("functions::source_cost(plan.value()).map_err(oversized)?"));
    assert!(flow.contains("functions::source_cost(plan.continuation()).map_err(oversized)?"));
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
    assert!(adapter.contains("crate::helper_returns::HelperReturns::new("));
    assert!(adapter.contains("for (value, level) in plan.return_sites()"));
    assert!(
        adapter.find("if bytes > MAX_SOURCE_BYTES").unwrap()
            < adapter.find(".connect(|body, _|").unwrap()
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
        "computation::lower_expression_with_functions(",
        "crate::helper_body::LoweredHelperBody",
        "let checked = body.validate_in_scope(",
        "if checked != *result_type",
        "let (prepared, cost) = admitted.into_parts();",
        "functions.templates.insert(",
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
    assert!(
        reference.contains("prepared: crate::helper_templates::HelperTemplate<Computation, Type>")
    );
    let checked = reference.find("body.validate_in_scope(").unwrap();
    let stored = reference
        .find("let (prepared, cost) = admitted.into_parts();")
        .unwrap();
    assert!(
        reference
            .find("crate::helper_body::LoweredHelperBody")
            .unwrap()
            < checked
    );
    assert!(checked < reference.find("if checked != *result_type").unwrap());
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
