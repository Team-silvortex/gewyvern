use std::path::PathBuf;
use std::process::Command;

use gewyvern::project_status::{
    ContractStability, EvidenceKind, EvidenceState, Independence, Maturity, STATUS_SCHEMA_VERSION,
    StatusCatalog, default_catalog_path,
};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn project_status_catalog_is_protocolized_and_valid() {
    let root = repository_root();
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("project/status/schema.json"))
            .expect("status schema must exist"),
    )
    .expect("status schema must be JSON");
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        STATUS_SCHEMA_VERSION
    );

    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog.validate(&root).expect("catalog must validate");
    assert!(catalog.dimensions.architectures.len() >= 6);
    assert!(catalog.dimensions.modules.len() >= 21);
    assert!(catalog.dimensions.features.len() >= 23);
    assert!(catalog.coverage_requirements.len() >= 19);
    assert!(catalog.cells.len() >= 23);

    for cell in &catalog.cells {
        assert!(!cell.contract.id.is_empty());
        assert!(!cell.contract.version.is_empty());
        assert!(!cell.contract.surfaces.is_empty());
        assert!(!cell.evidence.is_empty());
        assert!(!cell.next_gate.is_empty());
    }
}

#[test]
fn tensor_tracks_reuse_development_and_leserpent_two_gates() {
    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let summary = catalog.summary(20);

    assert!(
        summary
            .independently_usable
            .iter()
            .any(|cell| cell.independence == Independence::StandaloneTool)
    );
    assert!(
        summary
            .in_development
            .iter()
            .any(|cell| cell.architecture == "leserpent-2")
    );

    let domain = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/domain-model/command-query-kernel")
        .expect("Leserpent Gate 1 cell must exist");
    assert_eq!(domain.maturity, Maturity::Mature);
    assert_eq!(domain.completion, 100);
    assert_eq!(domain.independence, Independence::ReusableLibrary);
    assert_eq!(domain.contract.stability, ContractStability::Stable);
    assert!(
        domain
            .evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Source && item.state == EvidenceState::Present)
    );

    let language = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-vm/effect-reentry")
        .expect("Leserpent Gate 2 cell must exist");
    assert_eq!(language.maturity, Maturity::Mature);
    assert_eq!(language.completion, 100);
    assert_eq!(language.contract.stability, ContractStability::Stable);
    assert!(language.blockers.is_empty());

    let linux_attach = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "gewyvern-core/linux-ebpf/linux-attach")
        .expect("Gewyvern Linux attach cell must exist");
    assert_eq!(linux_attach.completion, 98);
    assert_eq!(linux_attach.blockers.len(), 1);
    assert!(linux_attach.evidence.iter().any(|item| {
        item.path == "docs/fixtures/linux_attach_pinned_source_root.json"
            && item.state == EvidenceState::Present
    }));

    let gewylang = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "gewylang/compiler/parser-lowering")
        .expect("GewyLang compiler cell must exist");
    assert_eq!(gewylang.maturity, Maturity::Mature);
    assert_eq!(gewylang.completion, 100);
    assert_eq!(gewylang.contract.stability, ContractStability::Stable);
    assert!(gewylang.blockers.is_empty());
    assert!(
        language
            .evidence
            .iter()
            .filter(|item| item.kind == EvidenceKind::Source)
            .all(|item| item.state == EvidenceState::Present)
    );

    let avalonia = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/ui-renderers/avalonia-renderer")
        .expect("Leserpent Gate 4 renderer cell must exist");
    assert_eq!(avalonia.maturity, Maturity::Mature);
    assert_eq!(avalonia.completion, 100);
    assert_eq!(avalonia.contract.stability, ContractStability::Stable);
    assert!(avalonia.blockers.is_empty());

    let transport = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/transport-protocol/wire-compatibility")
        .expect("Leserpent Gate 6 transport cell must exist");
    assert_eq!(transport.maturity, Maturity::Mature);
    assert_eq!(transport.completion, 100);
    assert_eq!(transport.contract.stability, ContractStability::Stable);
    assert!(transport.blockers.is_empty());

    let cli = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/native-cli/cli-parity")
        .expect("Leserpent native CLI cell must exist");
    assert_eq!(cli.maturity, Maturity::Mature);
    assert_eq!(cli.completion, 100);
    assert_eq!(cli.contract.stability, ContractStability::Stable);
    assert!(cli.blockers.is_empty());

    let runtime = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/control-runtime/durable-authority")
        .expect("Leserpent Gate 5 runtime cell must exist");
    assert_eq!(runtime.maturity, Maturity::Mature);
    assert_eq!(runtime.completion, 100);
    assert_eq!(runtime.contract.stability, ContractStability::Stable);
    assert!(runtime.blockers.is_empty());

    let ui = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/ui-language/ui-ir-lowering")
        .expect("Leserpent Gate 4 UI language cell must exist");
    assert_eq!(ui.maturity, Maturity::Mature);
    assert_eq!(ui.completion, 100);
    assert_eq!(ui.contract.stability, ContractStability::Stable);
    assert!(ui.blockers.is_empty());

    let syntax = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-syntax/lossless-frontend")
        .expect("Leserpent language syntax cell must exist");
    assert_eq!(syntax.maturity, Maturity::Mature);
    assert_eq!(syntax.completion, 100);
    assert_eq!(syntax.contract.stability, ContractStability::Stable);
    assert!(syntax.blockers.is_empty());

    let required_boundaries = [
        "boundary-leselang-syntax",
        "boundary-leselang-hir",
        "boundary-leselang-vm",
        "boundary-leselang-command",
        "boundary-leselang-ui",
        "boundary-leserpent-domain",
        "boundary-leserpent-runtime",
        "boundary-leserpent-protocol",
        "boundary-leserpent-adapters",
        "boundary-leserpent-cli",
        "boundary-leserpentd",
    ];
    for requirement in required_boundaries {
        assert!(
            catalog
                .coverage_requirements
                .iter()
                .any(|item| item.id == requirement),
            "missing architecture boundary {requirement}"
        );
    }
    for gate in 1..=7 {
        let gate_name = ["one", "two", "three", "four", "five", "six", "seven"][gate - 1];
        assert!(
            catalog
                .coverage_requirements
                .iter()
                .any(|item| item.id.starts_with(&format!("gate-{gate_name}-"))),
            "missing roadmap gate {gate}"
        );
    }
}

#[test]
fn coverage_manifest_rejects_unknown_mappings_and_orphan_cells() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let proof = catalog
        .coverage_requirements
        .iter_mut()
        .find(|item| item.id == "proof-continuous-shelves")
        .expect("continuous proof requirement must exist");
    proof.cells = vec!["leserpent-2/release-assurance/missing".into()];

    let errors = catalog
        .validate(repository_root())
        .expect_err("invalid coverage mapping must be rejected")
        .join("\n");
    assert!(errors.contains("references unknown cell"));
    assert!(errors.contains("continuous-proof' is missing"));
}

#[test]
fn native_status_cli_exposes_human_and_machine_views() {
    let binary = env!("CARGO_BIN_EXE_gewyvern_status");
    let validate = Command::new(binary)
        .arg("validate")
        .output()
        .expect("status validate must run");
    assert!(validate.status.success());
    assert!(String::from_utf8_lossy(&validate.stdout).contains("status catalog valid"));

    let summary = Command::new(binary)
        .args(["summary", "--json", "--limit", "3"])
        .output()
        .expect("status summary must run");
    assert!(summary.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&summary.stdout).expect("summary must be JSON");
    assert_eq!(payload["schema_version"], STATUS_SCHEMA_VERSION);
    assert_eq!(payload["coverage"]["ownership_boundary_count"], 12);
    assert_eq!(payload["coverage"]["roadmap_gate_count"], 7);
    assert_eq!(payload["coverage"]["proof_shelf_count"], 1);
    assert_eq!(payload["weakest"].as_array().unwrap().len(), 3);
    assert!(payload["lifecycles"].as_array().unwrap().len() >= 3);
    assert!(payload["architectures"].as_array().unwrap().len() >= 6);
    assert!(payload["modules"].as_array().unwrap().len() >= 12);

    let developing = Command::new(binary)
        .args(["developing", "--architecture", "leserpent-2", "--json"])
        .output()
        .expect("status developing query must run");
    assert!(developing.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&developing.stdout).expect("developing view must be JSON");
    assert!(
        payload
            .as_array()
            .unwrap()
            .iter()
            .all(|cell| cell["architecture"] == "leserpent-2")
    );

    let help = Command::new(binary)
        .arg("--help")
        .output()
        .expect("status help must run");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("usage: gewyvern_status"));

    let tensor_slice = Command::new(binary)
        .args([
            "weakest",
            "--architecture",
            "leserpent-2",
            "--module",
            "language-vm",
            "--feature",
            "effect-reentry",
            "--lifecycle",
            "target",
            "--maturity",
            "mature",
            "--json",
        ])
        .output()
        .expect("three-dimensional status slice must run");
    assert!(tensor_slice.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&tensor_slice.stdout).expect("tensor slice must be JSON");
    assert_eq!(payload.as_array().unwrap().len(), 1);
    assert_eq!(payload[0]["feature"], "effect-reentry");
}
