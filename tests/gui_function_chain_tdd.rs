use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gewyvern::gui_function_chain::{
    GUI_FUNCTION_CHAIN_SCHEMA_VERSION, GuiCoverageState, GuiFunctionChainCatalog,
    GuiOperationAudience, GuiOperationOwner, GuiSurfaceLifecycle, default_gui_function_chain_path,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_catalog() -> GuiFunctionChainCatalog {
    GuiFunctionChainCatalog::load(default_gui_function_chain_path(root()))
        .expect("GUI function-chain catalog must decode")
}

fn ids_for_owner(catalog: &GuiFunctionChainCatalog, owner: GuiOperationOwner) -> BTreeSet<&str> {
    catalog
        .operations
        .iter()
        .filter(|operation| operation.owner == owner)
        .map(|operation| operation.id.as_str())
        .collect()
}

fn domain_enum_operation_ids(enum_name: &str) -> BTreeSet<String> {
    let source = fs::read_to_string(root().join("crates/leserpent-domain/src/lib.rs"))
        .expect("Leserpent domain source must be readable");
    let declaration = format!("pub enum {enum_name} {{");
    let start = source
        .find(&declaration)
        .unwrap_or_else(|| panic!("{enum_name} declaration must exist"));
    let mut depth = 0i32;
    let mut variants = BTreeSet::new();
    for line in source[start..].lines() {
        let trimmed = line.trim();
        if depth == 1
            && trimmed
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_uppercase)
        {
            let variant = trimmed
                .split(|character: char| {
                    character == '{'
                        || character == '('
                        || character == ','
                        || character.is_whitespace()
                })
                .next()
                .expect("variant line must have a name");
            variants.insert(camel_to_kebab(variant));
        }
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth == 0 && line.contains('}') {
            break;
        }
    }
    assert!(!variants.is_empty(), "{enum_name} must declare variants");
    variants
}

fn camel_to_kebab(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 8);
    for (index, character) in value.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            result.push('-');
        }
        result.push(character.to_ascii_lowercase());
    }
    result
}

#[test]
fn gui_function_chain_catalog_is_complete_and_source_anchored() {
    let catalog = load_catalog();
    catalog.validate(root()).unwrap_or_else(|errors| {
        panic!(
            "GUI function-chain catalog is invalid:\n{}",
            errors.join("\n")
        )
    });

    assert_eq!(catalog.schema_version, GUI_FUNCTION_CHAIN_SCHEMA_VERSION);
    assert_eq!(catalog.release_line, "2.0");
    assert_eq!(catalog.as_of, "2026-08-26");
    assert_eq!(catalog.operations.len(), 35);
    assert_eq!(catalog.chains.len(), 11);

    assert_eq!(
        ids_for_owner(&catalog, GuiOperationOwner::DomainCommand),
        domain_enum_operation_ids("Command")
            .iter()
            .map(String::as_str)
            .collect()
    );
    assert_eq!(
        ids_for_owner(&catalog, GuiOperationOwner::DomainQuery),
        domain_enum_operation_ids("Query")
            .iter()
            .map(String::as_str)
            .collect()
    );
    assert_eq!(
        ids_for_owner(&catalog, GuiOperationOwner::Protocol),
        BTreeSet::from([
            "authority-writer-claim",
            "bootstrap-handoff",
            "bootstrap-session-bind",
            "deployment-receipt",
            "health",
            "orchestra-cancel",
            "orchestra-delete",
            "orchestra-delete-command",
            "orchestra-history",
            "orchestra-persist",
            "orchestra-plan-catalog",
            "orchestra-replay-checkpoint",
            "orchestra-replay-horizon",
            "orchestra-retry",
            "orchestra-run",
            "runtime-unregister",
            "runtime-unregistration-receipt",
        ])
    );
    assert_eq!(
        ids_for_owner(&catalog, GuiOperationOwner::Product),
        BTreeSet::from([
            "daemon-retire",
            "gewyvern-provision",
            "gewyvern-retire",
            "leselang-export",
            "reverse-daemon-bootstrap",
            "rust-web-console",
            "ui-presentation",
        ])
    );

    let release_operations = catalog
        .chains
        .iter()
        .filter(|chain| chain.release_required)
        .flat_map(|chain| chain.operations.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    assert!(catalog.operations.iter().all(|operation| {
        operation.audience != GuiOperationAudience::Operator
            || release_operations.contains(operation.id.as_str())
    }));
}

#[test]
fn gui_summary_separates_product_closure_from_renderer_conformance() {
    let summary = load_catalog().summary();
    assert_eq!(summary.operation_count, 35);
    assert_eq!(summary.chain_count, 11);
    assert_eq!(summary.target_score, 95);

    let avalonia = summary
        .surfaces
        .iter()
        .find(|surface| surface.id == "avalonia-desktop")
        .expect("Avalonia target summary must exist");
    assert_eq!(avalonia.lifecycle, GuiSurfaceLifecycle::Target);
    assert_eq!(avalonia.score, 100);
    assert_eq!(avalonia.required_chain_count, 9);
    assert_eq!(avalonia.closed, 9);
    assert_eq!(avalonia.partial, 0);
    assert_eq!(avalonia.conformance_only, 0);
    assert_eq!(avalonia.absent, 0);
    assert!(avalonia.gaps.is_empty());

    let rust_web = summary
        .surfaces
        .iter()
        .find(|surface| surface.id == "rust-web")
        .expect("Rust Web target summary must exist");
    assert_eq!(rust_web.lifecycle, GuiSurfaceLifecycle::Target);
    assert_eq!(rust_web.score, 50);
    assert_eq!(rust_web.partial, 1);
    assert_eq!(rust_web.absent, 0);
    assert_eq!(rust_web.gaps, ["rust-web-self-host"]);

    let bridge = summary
        .surfaces
        .iter()
        .find(|surface| surface.id == "web-1x")
        .expect("Web bridge summary must exist");
    assert_eq!(bridge.lifecycle, GuiSurfaceLifecycle::Bridge);
    assert_eq!(bridge.score, 100);
    assert_eq!(bridge.closed, 5);
    assert!(bridge.gaps.is_empty());
}

#[test]
fn closed_claims_require_every_stage_and_nonclosed_claims_require_a_gap() {
    let mut catalog = load_catalog();
    let fleet = catalog
        .chains
        .iter_mut()
        .find(|chain| chain.id == "fleet-observation")
        .unwrap();
    let avalonia = fleet
        .coverage
        .iter_mut()
        .find(|coverage| coverage.surface == "avalonia-desktop")
        .unwrap();
    avalonia
        .evidence
        .retain(|evidence| evidence.stage != "transport");
    let errors = catalog
        .validate(root())
        .expect_err("a closed chain with a missing stage must be rejected")
        .join("\n");
    assert!(errors.contains("lacks 'transport' evidence"));

    let mut catalog = load_catalog();
    let rust_web = catalog
        .chains
        .iter_mut()
        .find(|chain| chain.id == "rust-web-self-host")
        .unwrap();
    let coverage = rust_web.coverage.first_mut().unwrap();
    assert_eq!(coverage.state, GuiCoverageState::Partial);
    coverage.gap = None;
    let errors = catalog
        .validate(root())
        .expect_err("a non-closed chain without a gap must be rejected")
        .join("\n");
    assert!(errors.contains("must explain its gap"));
}

#[test]
fn evidence_anchors_cannot_escape_or_drift() {
    let mut catalog = load_catalog();
    catalog.operations[0].definition.path = "../outside".to_string();
    let errors = catalog
        .validate(root())
        .expect_err("repository escape must be rejected")
        .join("\n");
    assert!(errors.contains("must stay inside the repository"));

    let mut catalog = load_catalog();
    catalog.operations[0].definition.contains = "definitely-not-a-real-variant".to_string();
    let errors = catalog
        .validate(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect_err("a drifted source anchor must be rejected")
        .join("\n");
    assert!(errors.contains("anchor is missing"));
}

#[cfg(unix)]
#[test]
fn evidence_anchors_cannot_escape_through_an_intermediate_symlink() {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must follow the Unix epoch")
        .as_nanos();
    let sandbox = std::env::temp_dir().join(format!(
        "gewyvern-gui-chain-symlink-{}-{nonce}",
        std::process::id()
    ));
    let repository = sandbox.join("repository");
    let outside = sandbox.join("outside");
    fs::create_dir_all(&repository).expect("temporary repository must be created");
    fs::create_dir_all(&outside).expect("temporary outside directory must be created");
    fs::write(outside.join("evidence.txt"), "trusted anchor")
        .expect("outside evidence must be written");
    symlink(&outside, repository.join("evidence-link"))
        .expect("intermediate symlink must be created");

    let mut catalog = load_catalog();
    catalog.operations[0].definition.path = "evidence-link/evidence.txt".to_string();
    catalog.operations[0].definition.contains = "trusted anchor".to_string();
    let errors = catalog
        .validate(&repository)
        .expect_err("an intermediate symlink escape must be rejected")
        .join("\n");
    assert!(errors.contains("resolves outside the repository"));

    fs::remove_dir_all(&sandbox).expect("temporary symlink sandbox must be removed");
}

#[test]
fn native_status_cli_reports_gui_closure_without_hiding_gaps() {
    let binary = env!("CARGO_BIN_EXE_gewyvern_status");
    let output = Command::new(binary)
        .args(["gui", "--json"])
        .output()
        .expect("GUI status view must run");
    assert!(output.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("GUI status view must be JSON");
    assert_eq!(payload["target_score"], 95);
    assert_eq!(payload["operation_count"], 35);
    assert_eq!(payload["chain_count"], 11);
    assert_eq!(payload["surfaces"][0]["id"], "avalonia-desktop");
    assert_eq!(payload["surfaces"][0]["score"], 100);
    assert_eq!(payload["surfaces"][0]["gaps"], serde_json::json!([]));
    assert_eq!(payload["surfaces"][1]["id"], "rust-web");
    assert_eq!(payload["surfaces"][1]["score"], 50);
    assert_eq!(
        payload["surfaces"][1]["gaps"],
        serde_json::json!(["rust-web-self-host"])
    );

    let validation = Command::new(binary)
        .args(["validate", "--json"])
        .output()
        .expect("combined status validation must run");
    assert!(validation.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&validation.stdout).expect("combined validation must be JSON");
    assert_eq!(payload["gui_target_score"], 95);
    assert_eq!(payload["gui_operations"], 35);
}
