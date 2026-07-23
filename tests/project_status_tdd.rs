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
    assert!(catalog.coverage_requirements.len() >= 28);
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
fn retained_runtime_deletion_crash_evidence_is_non_vacuous() {
    assert_runtime_deletion_crash_evidence(
        "docs/fixtures/leserpent_runtime_deletion_crash_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_crash_evidence(
        "docs/fixtures/leserpent_runtime_deletion_crash_linux_x86_64_20260723.json",
        "X64",
    );
}

fn assert_runtime_deletion_crash_evidence(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion crash evidence must exist"),
    )
    .expect("runtime deletion crash evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert!(
        evidence["observed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        evidence["platform"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(evidence["architecture"], expected_architecture);
    for check in [
        "real_leserpentd",
        "daemon_unregistration_committed",
        "host_process_force_killed",
        "durable_intent_restored",
        "protected_runtime_restored",
        "background_recovery_converged",
        "daemon_and_compatibility_state_absent",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained crash proof {check}"
        );
    }
}

#[test]
fn etragon_stays_downweighted_until_the_deep_learning_stack_is_proven() {
    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let etragon = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "etragon/learning-sidecar/advisory-learning")
        .expect("Etragon advisory-learning cell must exist");

    assert_eq!(etragon.maturity, Maturity::Incubating);
    assert!(etragon.completion <= 45);
    assert!(etragon.blockers.iter().any(|blocker| {
        blocker.id == "deep-learning-stack-not-integrated"
            && blocker.summary.contains("inference evidence")
    }));
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
    assert_eq!(cli.contract.version, "1.6.0");
    for surface in [
        "runtime-provision-command",
        "runtime-retire-command",
        "authenticated-retirement-ipc-https",
        "stable-retirement-identity-replay",
        "credential-free-retirement-progress",
        "retirement-terminal-exit-code",
    ] {
        assert!(
            cli.contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing CLI retirement surface {surface}"
        );
    }
    assert!(cli.blockers.is_empty());

    let runtime = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/control-runtime/durable-authority")
        .expect("Leserpent Gate 5 runtime cell must exist");
    assert_eq!(runtime.maturity, Maturity::Mature);
    assert_eq!(runtime.completion, 100);
    assert_eq!(runtime.contract.stability, ContractStability::Stable);
    assert_eq!(runtime.contract.version, "1.10.0");
    for surface in [
        "durable-sidecar-endpoint",
        "atomic-sidecar-registration-update",
        "secret-free-sidecar-projection",
        "journal-authority-timestamps",
        "snapshot-persisted-authority-timestamps",
        "replay-stable-authority-timestamps",
        "idempotent-timestamp-preservation",
        "legacy-optional-authority-timestamps",
        "timestamp-free-command-outcomes",
        "typed-sidecar-status-observation",
        "sanitized-sidecar-failure-posture",
        "durable-sidecar-status",
        "sidecar-status-replay",
        "validated-runtime-status-posture",
        "sanitized-runtime-status-failure",
        "shared-status-observation-validation",
        "sqlite-v14-runtime-unregistration",
        "atomic-unregistration-orchestra-cleanup",
        "durable-unregistration-idempotency",
        "restart-safe-unregistration-replay",
    ] {
        assert!(
            runtime
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing durable sidecar authority surface {surface}"
        );
    }
    assert!(runtime.blockers.is_empty());

    let compatibility_control = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-1x/control-plane/orchestration-persistence")
        .expect("Leserpent compatibility control-plane cell must exist");
    assert_eq!(compatibility_control.contract.version, "1.9.0");
    for surface in [
        "daemon-authoritative-sidecar-endpoint",
        "daemon-authoritative-runtime-timestamps",
        "legacy-timestamp-field-fallback",
        "reversed-timestamp-rejection",
        "daemon-authoritative-sidecar-status",
        "legacy-sidecar-status-fallback",
        "sanitized-sidecar-failure-intake",
        "daemon-first-sidecar-refresh",
        "composed-daemon-discovery-intake",
        "sanitized-runtime-status-failure-intake",
        "daemon-first-runtime-refresh",
        "daemon-first-fleet-refresh",
        "daemon-first-orchestra-refresh",
        "daemon-first-runtime-unregistration",
        "runtime-deletion-reservation",
        "reserved-session-and-orchestra-rejection",
        "durable-runtime-deletion-intent",
        "strict-deletion-intent-persistence",
        "schema-v2-control-state",
        "deletion-intent-restart-recovery",
        "background-delete-convergence",
        "real-forced-termination-recovery-proof",
        "retained-crash-boundary-evidence",
        "physical-linux-x86-64-crash-recovery-proof",
    ] {
        assert!(
            compatibility_control
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing compatibility authority surface {surface}"
        );
    }
    assert!(
        compatibility_control
            .next_gate
            .contains("every durable runtime-deletion state transition")
    );

    let bootstrap = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/deployment-bootstrap/reverse-bootstrap")
        .expect("reverse deployment bootstrap must be tracked independently");
    assert_eq!(bootstrap.maturity, Maturity::Developing);
    assert!((80..=98).contains(&bootstrap.completion));
    assert_eq!(bootstrap.contract.stability, ContractStability::Draft);
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "native-rust-ssh-transport")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "strict-bootstrap-origin-config-v1")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "independent-authenticated-bootstrap-submission")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "avalonia-phase-gated-session-binding")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "session-bound-add-to-hub-gate")
    );
    assert!(bootstrap.blockers.iter().any(|blocker| {
        blocker.id == "cross-platform-bootstrap-installation-incomplete"
            && blocker
                .summary
                .contains("real Linux SSH/systemd-user authority proof")
    }));
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id == "bootstrap-production-entry-missing")
    );
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id.contains("gewyvern-retirement"))
    );
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id == "post-bind-gewyvern-ui-missing")
    );
    assert!(bootstrap.blockers.iter().all(|blocker| {
        !blocker
            .summary
            .contains("still needs service activation, registration proof")
    }));

    let provisioning = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/deployment-bootstrap/gewyvern-provisioning")
        .expect("Gewyvern provisioning must be tracked independently from pipeline deployment");
    assert_eq!(provisioning.maturity, Maturity::Mature);
    assert_eq!(provisioning.completion, 100);
    assert_eq!(provisioning.contract.stability, ContractStability::Stable);
    assert_eq!(provisioning.contract.version, "1.0.0");
    assert!(
        provisioning.contract.surfaces.iter().any(|surface| {
            surface == "planned-installing-service-ready-runtime-registered-state"
        })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "raw-provisioning-credential-field-rejection" })
    );
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| { blocker.id == "gewyvern-registration-handoff-missing" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "schema-13-shared-authority-checkpoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "authenticated-provisioning-https-route" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "daemon-provisioning-identity-fence" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "independent-gewyvern-installer-wire-v1" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "installer-request-ready-response-binding" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-install-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-activate-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-service-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "installer-installed-only-preparation" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "host-key-pinned-gewyvern-ssh" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "ready-before-trust-before-receipt" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "tls-token-health-before-ready" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "activation-rollback-preserves-prior-service" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "daemon-derived-runtime-registration-proof" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "atomic-effect-registration-checkpoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "legacy-service-ready-promotion" })
    );
    for surface in [
        "native-cli-confirmed-runtime-provision",
        "explicit-provisioning-id-replay",
        "authenticated-cli-provisioning-ipc-https",
        "bounded-cli-provisioning-progress",
        "provisioning-terminal-exit-codes",
        "avalonia-confirmed-runtime-provision",
        "authority-scoped-avalonia-provisioning",
        "identity-locked-avalonia-progress",
        "bounded-avalonia-provisioning-poll",
        "explicit-new-attempt-retry-guidance",
        "optional-local-gewyvern-provisioning-origin",
        "independent-runtime-retirement-domain",
        "runtime-retire-capability",
        "service-retired-before-runtime-unregister",
        "retirement-failure-preserves-registration",
        "opaque-ssh-retirement-handle",
        "strict-retirement-wire-v1",
        "retirement-identity-binding",
        "restart-safe-retirement-checkpoint",
        "durable-retirement-authority",
        "atomic-retirement-effect-unregistration",
        "replayable-runtime-unregistration",
        "schema-12-to-13-authority-migration",
        "lost-retirement-lease-rollback",
        "opaque-retirement-secret-resolution",
        "adapter-gated-daemon-retirement-submission",
        "retirement-predispatch-registration-fence",
        "terminal-retirement-outcome-validation",
        "atomic-daemon-retirement-settlement",
        "forged-retirement-receipt-rejection",
        "strict-gewyvern-retirement-wire-v1",
        "native-gewyvern-retire-v1-entrypoint",
        "manifest-bound-target-retirement",
        "two-phase-retirement-recovery-marker",
        "runtime-scoped-retirement-deletion",
        "private-retirement-authority-permissions",
        "host-key-pinned-retirement-ssh",
        "shared-gewyvern-origin-retirement-policy",
        "production-retirement-adapter-registration",
        "explicit-retirement-ipc-route",
        "authenticated-retirement-https-route",
        "bounded-retirement-route-payload",
        "adapter-registration-retirement-route-gate",
        "native-cli-confirmed-runtime-retire",
        "explicit-retirement-id-replay",
        "authenticated-cli-retirement-ipc-https",
        "bounded-cli-retirement-progress",
        "retirement-terminal-exit-codes",
        "avalonia-confirmed-runtime-retire",
        "authority-scoped-avalonia-retirement",
        "provisioning-bound-avalonia-retirement",
        "identity-locked-avalonia-retirement-progress",
        "bounded-avalonia-retirement-poll",
        "credential-free-avalonia-retirement-status",
        "retirement-failure-registration-guidance",
        "physical-linux-native-ssh-retirement",
        "forged-provisioning-retirement-rejection",
        "idempotent-physical-retirement-replay",
        "zero-residue-systemd-user-retirement",
    ] {
        assert!(
            provisioning
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing provisioning client surface {surface}"
        );
    }
    assert!(provisioning.blockers.is_empty());
    assert!(provisioning.evidence.iter().any(|evidence| {
        evidence.path == "docs/fixtures/leserpent_real_ssh_retirement_20260723.json"
            && evidence.state == EvidenceState::Present
    }));
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| { blocker.id == "gewyvern-provisioning-client-controls-incomplete" })
    );
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| blocker.id == "gewyvern-provisioning-client-controls-missing")
    );

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
        "boundary-leserpent-bootstrap",
        "boundary-gewyvern-runtime-evidence",
        "boundary-gewyvern-linux-ebpf",
        "boundary-gewylang-compiler",
        "boundary-gewylang-protocol-packages",
        "boundary-leserpent-1x-control-plane",
        "boundary-leserpent-1x-web-console",
        "boundary-etragon-learning-sidecar",
        "boundary-project-status-governance",
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

    for architecture in catalog
        .dimensions
        .architectures
        .iter()
        .map(|entry| entry.id.as_str())
    {
        for cell in catalog
            .cells
            .iter()
            .filter(|cell| cell.architecture == architecture)
        {
            assert!(
                catalog.coverage_requirements.iter().any(|requirement| {
                    requirement.architecture == architecture && requirement.cells.contains(&cell.id)
                }),
                "cell '{}' is missing authoritative coverage",
                cell.id
            );
        }
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
fn expanded_coverage_manifest_rejects_gewyvern_core_orphans() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog
        .coverage_requirements
        .retain(|item| item.id != "boundary-gewyvern-linux-ebpf");

    let errors = catalog
        .validate(repository_root())
        .expect_err("unmapped Gewyvern Core cells must be rejected")
        .join("\n");
    assert!(errors.contains(
        "cell 'gewyvern-core/linux-ebpf/linux-attach' is missing from the 'gewyvern-core' coverage manifest"
    ));
}

#[test]
fn coverage_manifest_is_required_for_every_architecture_with_cells() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog
        .coverage_requirements
        .retain(|item| item.architecture != "etragon");

    let errors = catalog
        .validate(repository_root())
        .expect_err("architectures without coverage requirements must be rejected")
        .join("\n");
    assert!(errors.contains("architecture 'etragon' has cells but no coverage requirements"));
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
    assert_eq!(payload["coverage"]["requirement_count"], 29);
    assert_eq!(payload["coverage"]["architecture_count"], 6);
    assert_eq!(payload["coverage"]["ownership_boundary_count"], 21);
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
