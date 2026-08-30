use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::project_status::StatusCatalog;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_status,
};
use super::dotnet_proof::run_locked_dotnet_test;
use super::read_bounded_json_file;

const INVENTORY_PATH: &str = "project/release/leserpent-v1-schema-inventory.json";
const COMPATIBILITY_BASELINE_PATH: &str =
    "project/release/leserpent-v1-compatibility-baseline.json";
const SCOPE_FREEZE_PATH: &str = "project/release/leserpent-2-scope-freeze.json";
const PATCH_SEAL_PATH: &str = "project/release/leserpent-2-patch-seal.json";
const EXPECTED_FAMILIES: &[&str] = &["command", "effect", "query", "ui", "wire"];
const EXPECTED_FIXTURE_FAMILIES: &[&str] = &["legacy-wire", "ui", "wire"];
const EXPECTED_COMPATIBILITY_FIXTURES: usize = 11;
const EXPECTED_SCOPE_CAPABILITIES: &[&str] = &[
    "authenticated-remote-control",
    "desktop-hub-workspaces",
    "gewyvern-runtime-integration",
    "multi-daemon-gewyvern-orchestration",
    "packaging-release-evidence",
    "performance-budgets",
    "persistence-recovery",
    "renderer-neutral-gui-automation",
    "reverse-deployment",
    "security-hardening",
];
const EXPECTED_CLOSURE_WORK: &[&str] = &[
    "accidental-complexity-reduction",
    "bug-fixes",
    "cross-language-conformance",
    "documentation",
    "existing-capability-polish",
    "packaging-deployment-recovery",
    "performance-benchmarks",
    "reliability-hardening",
    "security-audits",
    "status-tensor-alignment",
];
const EXPECTED_DEFERRED_CAPABILITIES: &[&str] = &[
    "additional-runtime-languages",
    "automatic-gui-framework-compatibility",
    "etragon-release-authority",
    "expanded-host-device-test-matrix",
    "full-mobile-device-parity",
    "future-hosted-account-production-proof",
    "long-tail-native-speaker-review",
    "production-signing-notarization",
    "second-gui-control-dsl",
    "windows-native-parity",
];
const MANAGED_MIGRATION_PROJECT: &str =
    "apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj";
const MANAGED_MIGRATION_FILTER: &str =
    "FullyQualifiedName~Leserpent.SecurityTests.SqliteOrchestraRunStoreTests";
const MANAGED_MIGRATION_MIN_TESTS: usize = 10;
const MANAGED_MIGRATION_INVARIANTS: &[&str] = &[
    "sqlite-v1-in-place-migration",
    "legacy-json-to-sqlite-migration",
    "request-id-uniqueness-preservation",
    "runtime-delete-cascade",
    "bounded-history-retention",
    "concurrent-json-save-serialization",
    "failed-save-snapshot-preservation",
    "transactional-migration-write-rollback",
    "retained-json-byte-preservation",
    "managed-migration-retry",
    "operator-json-rollback",
];

struct ProofSuite {
    id: &'static str,
    package: &'static str,
    target_args: &'static [&'static str],
    test_filter: Option<&'static str>,
    expected_min_tests: usize,
    invariants: &'static [&'static str],
}

const PROOF_SUITES: &[ProofSuite] = &[
    ProofSuite {
        id: "domain-v1",
        package: "leserpent-domain",
        target_args: &["--lib"],
        test_filter: None,
        expected_min_tests: 14,
        invariants: &[
            "command-v1-roundtrip",
            "query-v1-roundtrip",
            "effect-plan-v1-validation",
            "unknown-version-rejection",
        ],
    },
    ProofSuite {
        id: "ui-v1",
        package: "leselang-ui",
        target_args: &["--lib"],
        test_filter: None,
        expected_min_tests: 19,
        invariants: &[
            "ui-document-v1",
            "ui-patch-v1",
            "bounded-renderer-neutral-ir",
        ],
    },
    ProofSuite {
        id: "wire-v1",
        package: "leserpent-protocol",
        target_args: &["--lib"],
        test_filter: None,
        expected_min_tests: 11,
        invariants: &[
            "wire-envelope-v1",
            "strict-versioned-decode",
            "bounded-message-contract",
            "typed-deployment-receipt-contract",
            "typed-orchestra-atomic-persistence-contract",
            "typed-orchestra-history-pagination-contract",
            "typed-orchestra-transactional-delete-contract",
        ],
    },
    ProofSuite {
        id: "runtime-migration-replay",
        package: "leserpent-runtime",
        target_args: &["--lib"],
        test_filter: Some("migrat"),
        expected_min_tests: 4,
        invariants: &[
            "journal-v1-to-current-replay",
            "snapshot-v3-generation-migration",
            "complete-v6-semantic-migration",
            "invalid-migration-history-rejection",
        ],
    },
    ProofSuite {
        id: "legacy-wire-migration",
        package: "leserpent-protocol",
        target_args: &["--test", "compatibility_v1"],
        test_filter: None,
        expected_min_tests: 7,
        invariants: &[
            "legacy-runtime-list-normalization",
            "legacy-status-refresh-idempotency",
            "legacy-error-preservation",
            "legacy-wire-size-bound",
            "legacy-deployment-pre-effect-contract",
            "legacy-orchestra-atomic-persistence-contract",
            "rust-authoritative-deployment-normalization",
        ],
    },
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SchemaInventory {
    schema_version: u32,
    release_line: String,
    freeze_state: String,
    contracts: Vec<SchemaContract>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SchemaContract {
    id: String,
    family: String,
    version: u32,
    stability: String,
    source: String,
    anchors: Vec<String>,
    proof: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityBaseline {
    schema_version: u32,
    release_line: String,
    baseline_state: String,
    algorithm: String,
    fixtures: Vec<CompatibilityFixture>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityFixture {
    id: String,
    family: String,
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeFreeze {
    schema_version: u32,
    release_line: String,
    freeze_state: String,
    authoritative_sources: Vec<ScopeSource>,
    core_capabilities: Vec<ScopeCapability>,
    accepted_closure_work: Vec<String>,
    deferred_capabilities: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeSource {
    path: String,
    anchor: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScopeCapability {
    id: String,
    status_cells: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatchSeal {
    schema_version: u32,
    release_line: String,
    target_release: String,
    policy: String,
    scope_freeze_manifest: String,
    patch_slots: Vec<PatchSlot>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PatchSlot {
    version: String,
    focus: String,
    closure_work: Vec<String>,
    exit_gate: String,
}

pub fn run_leserpent_schema_freeze_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let root = repo_root();
    let inventory = load_and_validate_inventory(&root.join(INVENTORY_PATH), &root)?;
    let compatibility =
        load_and_validate_compatibility_baseline(&root.join(COMPATIBILITY_BASELINE_PATH), &root)?;
    let scope_freeze = load_and_validate_scope_freeze(&root.join(SCOPE_FREEZE_PATH), &root)?;
    let patch_seal = load_and_validate_patch_seal(&root.join(PATCH_SEAL_PATH), &scope_freeze)?;
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-schema-freeze"));
    fs::create_dir_all(&out_dir)?;
    clear_previous_evidence(&out_dir)?;

    let mut proof_summaries = Vec::new();
    let mut checks = Vec::new();
    let mut files = Vec::new();
    let mut total_test_count = 0usize;
    for proof in PROOF_SUITES {
        let log = format!("{}.log", proof.id);
        let mut args = vec![
            "test".to_string(),
            "-p".to_string(),
            proof.package.to_string(),
        ];
        args.extend(proof.target_args.iter().map(|arg| (*arg).to_string()));
        if let Some(filter) = proof.test_filter {
            args.push(filter.to_string());
        }
        args.extend(["--".to_string(), "--nocapture".to_string()]);
        let log_path = out_dir.join(&log);
        run_cargo_status(&args, &log_path)?;
        let test_count = require_nonzero_test_result(&log_path, proof.id)?;
        if test_count < proof.expected_min_tests {
            return Err(ValidationError::new(format!(
                "{} proof ran {test_count} tests, expected at least {}",
                proof.id, proof.expected_min_tests
            )));
        }
        total_test_count += test_count;
        files.push(log);
        checks.extend(proof.invariants.iter().map(|item| (*item).to_string()));
        proof_summaries.push(json!({
            "id": proof.id,
            "package": proof.package,
            "target_args": proof.target_args,
            "test_filter": proof.test_filter,
            "expected_min_tests": proof.expected_min_tests,
            "invariants": proof.invariants,
            "test_count": test_count,
            "status": "passed",
        }));
    }

    let managed_log = "managed-control-plane-migration.log";
    let dotnet_artifacts = out_dir.join("managed-control-plane-migration-artifacts");
    let managed_test_count = run_locked_dotnet_test(
        MANAGED_MIGRATION_PROJECT,
        Some(MANAGED_MIGRATION_FILTER),
        &dotnet_artifacts,
        &out_dir.join(managed_log),
    )?;
    if managed_test_count < MANAGED_MIGRATION_MIN_TESTS {
        return Err(ValidationError::new(format!(
            "managed control-plane migration proof ran {managed_test_count} tests, expected at least {MANAGED_MIGRATION_MIN_TESTS}"
        )));
    }
    total_test_count += managed_test_count;
    files.push(managed_log.to_string());
    checks.extend(
        MANAGED_MIGRATION_INVARIANTS
            .iter()
            .map(|item| (*item).to_string()),
    );
    proof_summaries.push(json!({
        "id": "managed-control-plane-migration",
        "runner": "dotnet-test",
        "project": MANAGED_MIGRATION_PROJECT,
        "test_filter": MANAGED_MIGRATION_FILTER,
        "restore_locked": true,
        "expected_min_tests": MANAGED_MIGRATION_MIN_TESTS,
        "test_count": managed_test_count,
        "invariants": MANAGED_MIGRATION_INVARIANTS,
        "status": "passed",
    }));

    let freeze_ready = inventory.freeze_state == "frozen"
        && inventory
            .contracts
            .iter()
            .all(|contract| contract.stability == "frozen");
    checks.extend(
        inventory
            .contracts
            .iter()
            .map(|contract| format!("{}-inventory", contract.family)),
    );
    checks.extend(
        compatibility
            .fixtures
            .iter()
            .map(|fixture| format!("{}-compatibility-fingerprint", fixture.id)),
    );
    checks.extend(
        scope_freeze
            .core_capabilities
            .iter()
            .map(|capability| format!("{}-scope-frozen", capability.id)),
    );
    checks.extend(
        scope_freeze
            .deferred_capabilities
            .iter()
            .map(|capability| format!("{capability}-scope-deferred")),
    );
    checks.extend(
        patch_seal
            .patch_slots
            .iter()
            .map(|slot| format!("{}-patch-seal", slot.focus)),
    );
    let current_patch_slot = patch_seal
        .patch_slots
        .iter()
        .find(|slot| slot.version == env!("CARGO_PKG_VERSION"))
        .map(|slot| slot.focus.as_str());
    let summary_name = "schema-freeze-summary.json";
    fs::write(
        out_dir.join(summary_name),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "release_line": inventory.release_line,
            "inventory": INVENTORY_PATH,
            "compatibility_baseline": COMPATIBILITY_BASELINE_PATH,
            "scope_freeze_manifest": SCOPE_FREEZE_PATH,
            "patch_seal_manifest": PATCH_SEAL_PATH,
            "freeze_state": inventory.freeze_state,
            "freeze_ready": freeze_ready,
            "scope_freeze_state": scope_freeze.freeze_state,
            "scope_freeze_ready": true,
            "contract_count": inventory.contracts.len(),
            "compatibility_fixture_count": compatibility.fixtures.len(),
            "scope_capability_count": scope_freeze.core_capabilities.len(),
            "accepted_closure_work_count": scope_freeze.accepted_closure_work.len(),
            "deferred_capability_count": scope_freeze.deferred_capabilities.len(),
            "patch_slot_count": patch_seal.patch_slots.len(),
            "current_patch_slot": current_patch_slot,
            "proof_count": proof_summaries.len(),
            "test_count": total_test_count,
            "contracts": inventory.contracts,
            "compatibility_fixtures": compatibility.fixtures,
            "scope_freeze": scope_freeze,
            "patch_seal": patch_seal,
            "proofs": proof_summaries,
            "remaining_gate": if freeze_ready {
                serde_json::Value::Null
            } else {
                json!("promote candidates only after every Gate 7 release criterion has reproducible evidence")
            },
        }))?,
    )?;
    files.insert(0, summary_name.to_string());
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-schema-freeze",
            "files": files,
        }))?,
    )?;

    Ok(ValidationReport {
        name: "Leserpent v1 schema freeze readiness shelf".to_string(),
        out_dir,
        checks,
    })
}

fn load_and_validate_scope_freeze(
    path: &Path,
    root: &Path,
) -> Result<ScopeFreeze, ValidationError> {
    let value = read_bounded_json_file(path, "Leserpent 2.0 scope freeze", 64 * 1024)?;
    let scope: ScopeFreeze = serde_json::from_value(value).map_err(|error| {
        ValidationError::new(format!("invalid Leserpent 2.0 scope freeze: {error}"))
    })?;
    if scope.schema_version != 1 || scope.release_line != "2.0" || scope.freeze_state != "frozen" {
        return Err(ValidationError::new(
            "Leserpent scope freeze must be frozen schema v1 for release line 2.0",
        ));
    }

    validate_scope_sources(&scope.authoritative_sources, root)?;
    require_exact_set(
        "core capability",
        scope
            .core_capabilities
            .iter()
            .map(|capability| capability.id.as_str()),
        EXPECTED_SCOPE_CAPABILITIES,
    )?;
    require_exact_set(
        "accepted closure work",
        scope.accepted_closure_work.iter().map(String::as_str),
        EXPECTED_CLOSURE_WORK,
    )?;
    require_exact_set(
        "deferred capability",
        scope.deferred_capabilities.iter().map(String::as_str),
        EXPECTED_DEFERRED_CAPABILITIES,
    )?;

    let catalog =
        StatusCatalog::load(root.join("project/status/catalog.json")).map_err(|error| {
            ValidationError::new(format!(
                "failed to load status catalog for scope freeze: {error}"
            ))
        })?;
    let known_cells = catalog
        .cells
        .iter()
        .map(|cell| cell.id.as_str())
        .collect::<BTreeSet<_>>();
    for capability in &scope.core_capabilities {
        if capability.status_cells.is_empty() || capability.status_cells.len() > 8 {
            return Err(ValidationError::new(format!(
                "scope capability {} must reference one to eight status cells",
                capability.id
            )));
        }
        let mut cells = BTreeSet::new();
        for cell in &capability.status_cells {
            if !cells.insert(cell.as_str()) {
                return Err(ValidationError::new(format!(
                    "scope capability {} contains duplicate status cell {cell}",
                    capability.id
                )));
            }
            if !(cell.starts_with("leserpent-2/") || cell.starts_with("gewyvern-core/"))
                || !known_cells.contains(cell.as_str())
            {
                return Err(ValidationError::new(format!(
                    "scope capability {} references unavailable core status cell {cell}",
                    capability.id
                )));
            }
        }
    }

    Ok(scope)
}

fn load_and_validate_patch_seal(
    path: &Path,
    scope: &ScopeFreeze,
) -> Result<PatchSeal, ValidationError> {
    let value = read_bounded_json_file(path, "Leserpent 2.0 patch seal", 64 * 1024)?;
    let seal: PatchSeal = serde_json::from_value(value).map_err(|error| {
        ValidationError::new(format!("invalid Leserpent 2.0 patch seal: {error}"))
    })?;
    if seal.schema_version != 1
        || seal.release_line != "1.20.x"
        || seal.target_release != "2.0.0"
        || seal.policy != "closure-only"
        || seal.scope_freeze_manifest != SCOPE_FREEZE_PATH
    {
        return Err(ValidationError::new(
            "Leserpent patch seal must define the closure-only 1.20.x to 2.0.0 window",
        ));
    }
    if seal.patch_slots.len() != 10 {
        return Err(ValidationError::new(
            "Leserpent patch seal must contain exactly ten patch slots",
        ));
    }

    let allowed_work = scope
        .accepted_closure_work
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut covered_work = BTreeSet::new();
    let mut focuses = BTreeSet::new();
    for (patch, slot) in seal.patch_slots.iter().enumerate() {
        let expected_version = format!("1.20.{patch}");
        if slot.version != expected_version {
            return Err(ValidationError::new(format!(
                "Leserpent patch slot {patch} must be {expected_version}, observed {}",
                slot.version
            )));
        }
        if !is_normalized_identifier(&slot.focus) || !focuses.insert(slot.focus.as_str()) {
            return Err(ValidationError::new(format!(
                "Leserpent patch slot {} must have a unique normalized focus",
                slot.version
            )));
        }
        if slot.closure_work.is_empty() {
            return Err(ValidationError::new(format!(
                "Leserpent patch slot {} must declare closure work",
                slot.version
            )));
        }
        let mut slot_work = BTreeSet::new();
        for item in &slot.closure_work {
            if !slot_work.insert(item.as_str()) {
                return Err(ValidationError::new(format!(
                    "Leserpent patch slot {} repeats closure work {item}",
                    slot.version
                )));
            }
            if !allowed_work.contains(item.as_str()) {
                return Err(ValidationError::new(format!(
                    "Leserpent patch slot {} contains work outside the frozen closure: {item}",
                    slot.version
                )));
            }
            covered_work.insert(item.as_str());
        }
        if slot.exit_gate.trim().is_empty() || slot.exit_gate.len() > 512 {
            return Err(ValidationError::new(format!(
                "Leserpent patch slot {} must have a bounded non-empty exit gate",
                slot.version
            )));
        }
    }
    if covered_work != allowed_work {
        return Err(ValidationError::new(
            "Leserpent patch seal does not cover every accepted closure-work family",
        ));
    }

    let current = env!("CARGO_PKG_VERSION");
    let target_prerelease = format!("{}-", seal.target_release);
    let target_build = format!("{}+", seal.target_release);
    if current != seal.target_release
        && !current.starts_with(&target_prerelease)
        && !current.starts_with(&target_build)
        && !seal.patch_slots.iter().any(|slot| slot.version == current)
    {
        return Err(ValidationError::new(format!(
            "current product version {current} is outside the frozen patch-seal window"
        )));
    }

    Ok(seal)
}

fn is_normalized_identifier(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validate_scope_sources(sources: &[ScopeSource], root: &Path) -> Result<(), ValidationError> {
    let expected = BTreeMap::from([
        ("docs/leserpent-2-architecture.md", "## 2.0 Scope Boundary"),
        ("docs/leserpent-2-roadmap.md", "## 2.0 Scope Freeze"),
    ]);
    if sources.len() != expected.len() {
        return Err(ValidationError::new(
            "Leserpent scope freeze must reference both authoritative scope documents",
        ));
    }
    let mut paths = BTreeSet::new();
    for source in sources {
        let Some(expected_anchor) = expected.get(source.path.as_str()) else {
            return Err(ValidationError::new(format!(
                "Leserpent scope freeze references non-authoritative source {}",
                source.path
            )));
        };
        if source.anchor != *expected_anchor || !paths.insert(source.path.as_str()) {
            return Err(ValidationError::new(format!(
                "Leserpent scope freeze source {} has a duplicate or invalid anchor",
                source.path
            )));
        }
        let relative = Path::new(&source.path);
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(ValidationError::new(
                "Leserpent scope freeze source must be repository-relative and normalized",
            ));
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > 2 * 1024 * 1024
        {
            return Err(ValidationError::new(format!(
                "Leserpent scope freeze source must be a bounded regular file: {}",
                path.display()
            )));
        }
        let body = fs::read_to_string(&path)?;
        if !body.contains(&source.anchor) {
            return Err(ValidationError::new(format!(
                "Leserpent scope freeze source {} is missing its authority anchor",
                source.path
            )));
        }
    }
    Ok(())
}

fn require_exact_set<'a>(
    label: &str,
    actual: impl Iterator<Item = &'a str>,
    expected: &[&str],
) -> Result<(), ValidationError> {
    let values = actual.collect::<Vec<_>>();
    let unique = values.iter().copied().collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != values.len() || unique != expected {
        return Err(ValidationError::new(format!(
            "Leserpent scope freeze {label} set does not match the closed 2.0 boundary"
        )));
    }
    Ok(())
}

fn load_and_validate_inventory(
    path: &Path,
    root: &Path,
) -> Result<SchemaInventory, ValidationError> {
    let value = read_bounded_json_file(path, "Leserpent schema inventory", 64 * 1024)?;
    let inventory: SchemaInventory = serde_json::from_value(value).map_err(|error| {
        ValidationError::new(format!("invalid Leserpent schema inventory: {error}"))
    })?;
    if inventory.schema_version != 1 || inventory.release_line != "2.0" {
        return Err(ValidationError::new(
            "Leserpent schema inventory must target schema version 1 and release line 2.0",
        ));
    }
    if !matches!(inventory.freeze_state.as_str(), "candidate" | "frozen") {
        return Err(ValidationError::new(
            "Leserpent schema inventory freeze_state must be candidate or frozen",
        ));
    }

    let proof_registry = PROOF_SUITES
        .iter()
        .map(|proof| (proof.id, proof))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    for contract in &inventory.contracts {
        if !ids.insert(contract.id.as_str()) {
            return Err(ValidationError::new(format!(
                "Leserpent schema inventory contains duplicate id {}",
                contract.id
            )));
        }
        if !families.insert(contract.family.as_str()) {
            return Err(ValidationError::new(format!(
                "Leserpent schema inventory contains duplicate family {}",
                contract.family
            )));
        }
        if contract.version != 1 {
            return Err(ValidationError::new(format!(
                "Leserpent schema contract {} is not version 1",
                contract.id
            )));
        }
        if contract.stability != inventory.freeze_state {
            return Err(ValidationError::new(format!(
                "Leserpent schema contract {} stability does not match inventory freeze_state",
                contract.id
            )));
        }
        if !proof_registry.contains_key(contract.proof.as_str()) {
            return Err(ValidationError::new(format!(
                "Leserpent schema contract {} references unknown proof {}",
                contract.id, contract.proof
            )));
        }
        validate_source(contract, root)?;
    }
    let expected = EXPECTED_FAMILIES.iter().copied().collect::<BTreeSet<_>>();
    if families != expected {
        return Err(ValidationError::new(format!(
            "Leserpent schema inventory families must be exactly {}",
            EXPECTED_FAMILIES.join(", ")
        )));
    }
    Ok(inventory)
}

fn validate_source(contract: &SchemaContract, root: &Path) -> Result<(), ValidationError> {
    let relative = Path::new(&contract.source);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(ValidationError::new(format!(
            "Leserpent schema contract {} source must be a normalized repository-relative path",
            contract.id
        )));
    }
    if contract.anchors.is_empty() || contract.anchors.len() > 8 {
        return Err(ValidationError::new(format!(
            "Leserpent schema contract {} must provide one to eight anchors",
            contract.id
        )));
    }
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        ValidationError::new(format!("failed to inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 2 * 1024 * 1024
    {
        return Err(ValidationError::new(format!(
            "Leserpent schema source must be a regular non-symlink file no larger than 2 MiB: {}",
            path.display()
        )));
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        ValidationError::new(format!("failed to read {}: {error}", path.display()))
    })?;
    for anchor in &contract.anchors {
        if anchor.is_empty()
            || anchor.len() > 128
            || anchor.chars().any(char::is_control)
            || !source.contains(anchor)
        {
            return Err(ValidationError::new(format!(
                "Leserpent schema contract {} has a missing or invalid source anchor",
                contract.id
            )));
        }
    }
    Ok(())
}

fn load_and_validate_compatibility_baseline(
    path: &Path,
    root: &Path,
) -> Result<CompatibilityBaseline, ValidationError> {
    let value = read_bounded_json_file(path, "Leserpent compatibility baseline", 64 * 1024)?;
    let baseline: CompatibilityBaseline = serde_json::from_value(value).map_err(|error| {
        ValidationError::new(format!("invalid Leserpent compatibility baseline: {error}"))
    })?;
    if baseline.schema_version != 1
        || baseline.release_line != "2.0"
        || baseline.baseline_state != "candidate"
        || baseline.algorithm != "sha256"
    {
        return Err(ValidationError::new(
            "Leserpent compatibility baseline must be candidate schema v1 for release line 2.0 using sha256",
        ));
    }
    if baseline.fixtures.len() != EXPECTED_COMPATIBILITY_FIXTURES {
        return Err(ValidationError::new(format!(
            "Leserpent compatibility baseline must contain exactly {EXPECTED_COMPATIBILITY_FIXTURES} v1 fixtures"
        )));
    }

    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut families = BTreeSet::new();
    for fixture in &baseline.fixtures {
        if !ids.insert(fixture.id.as_str()) || !paths.insert(fixture.path.as_str()) {
            return Err(ValidationError::new(
                "Leserpent compatibility baseline contains a duplicate fixture id or path",
            ));
        }
        families.insert(fixture.family.as_str());
        validate_compatibility_fixture(fixture, root)?;
    }
    let expected = EXPECTED_FIXTURE_FAMILIES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if families != expected {
        return Err(ValidationError::new(format!(
            "Leserpent compatibility fixture families must be exactly {}",
            EXPECTED_FIXTURE_FAMILIES.join(", ")
        )));
    }
    Ok(baseline)
}

fn validate_compatibility_fixture(
    fixture: &CompatibilityFixture,
    root: &Path,
) -> Result<(), ValidationError> {
    const MAX_FIXTURE_BYTES: u64 = 256 * 1024;

    let relative = Path::new(&fixture.path);
    if !fixture.path.ends_with("-v1.json")
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(ValidationError::new(format!(
            "compatibility fixture {} must use a normalized repository-relative v1 JSON path",
            fixture.id
        )));
    }
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        ValidationError::new(format!("failed to inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_FIXTURE_BYTES
        || metadata.len() != fixture.bytes
    {
        return Err(ValidationError::new(format!(
            "compatibility fixture {} must be a bounded regular file with its declared byte length",
            fixture.id
        )));
    }
    if fixture.sha256.len() != 64 || !fixture.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ValidationError::new(format!(
            "compatibility fixture {} has an invalid SHA-256 fingerprint",
            fixture.id
        )));
    }
    let bytes = fs::read(&path)?;
    let actual = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let actual = actual
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != fixture.sha256.to_ascii_lowercase() {
        return Err(ValidationError::new(format!(
            "compatibility fixture {} differs from its reviewed v1 baseline",
            fixture.id
        )));
    }
    Ok(())
}

fn clear_previous_evidence(out_dir: &Path) -> Result<(), ValidationError> {
    let dotnet_artifacts = out_dir.join("managed-control-plane-migration-artifacts");
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(dotnet_artifacts)?;
    }
    for name in [
        "schema-freeze-summary.json",
        "evidence-index.json",
        "domain-v1.log",
        "ui-v1.log",
        "wire-v1.log",
        "runtime-migration-replay.log",
        "legacy-wire-migration.log",
        "managed-control-plane-migration.log",
    ] {
        let path = out_dir.join(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn require_nonzero_test_result(path: &Path, proof_id: &str) -> Result<usize, ValidationError> {
    const MAX_LOG_BYTES: u64 = 1024 * 1024;

    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ValidationError::new(format!("failed to inspect {} proof log: {error}", proof_id))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_LOG_BYTES {
        return Err(ValidationError::new(format!(
            "{proof_id} proof log must be a regular non-symlink file no larger than {MAX_LOG_BYTES} bytes"
        )));
    }
    let body = fs::read_to_string(path).map_err(|error| {
        ValidationError::new(format!("failed to read {proof_id} proof log: {error}"))
    })?;
    let summaries = body
        .lines()
        .filter_map(|line| line.strip_prefix("test result: ok. "))
        .collect::<Vec<_>>();
    if summaries.len() != 1 {
        return Err(ValidationError::new(format!(
            "{proof_id} proof log must contain exactly one successful test summary"
        )));
    }
    let (passed, remainder) = summaries[0].split_once(" passed;").ok_or_else(|| {
        ValidationError::new(format!("{proof_id} proof log has an invalid test summary"))
    })?;
    let passed = passed.parse::<usize>().map_err(|_| {
        ValidationError::new(format!("{proof_id} proof log has an invalid passed count"))
    })?;
    if passed == 0 || !remainder.trim_start().starts_with("0 failed;") {
        return Err(ValidationError::new(format!(
            "{proof_id} proof must execute at least one test with zero failures"
        )));
    }
    Ok(passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_inventory_is_complete_but_not_prematurely_frozen() {
        let root = repo_root();
        let inventory = load_and_validate_inventory(&root.join(INVENTORY_PATH), &root).unwrap();
        assert_eq!(inventory.contracts.len(), 5);
        assert_eq!(inventory.freeze_state, "candidate");
        assert!(
            inventory
                .contracts
                .iter()
                .all(|contract| contract.version == 1 && contract.stability == "candidate")
        );
    }

    #[test]
    fn proof_registry_is_fixed_and_non_executable_from_the_manifest() {
        assert_eq!(PROOF_SUITES.len(), 5);
        assert_eq!(PROOF_SUITES[0].id, "domain-v1");
        assert_eq!(PROOF_SUITES[1].id, "ui-v1");
        assert_eq!(PROOF_SUITES[2].id, "wire-v1");
        assert_eq!(PROOF_SUITES[3].id, "runtime-migration-replay");
        assert_eq!(PROOF_SUITES[3].test_filter, Some("migrat"));
        assert_eq!(PROOF_SUITES[4].id, "legacy-wire-migration");
        assert_eq!(MANAGED_MIGRATION_MIN_TESTS, 10);
        assert!(MANAGED_MIGRATION_FILTER.contains("SqliteOrchestraRunStoreTests"));
    }

    #[test]
    fn production_compatibility_baseline_covers_all_expected_v1_fixtures() {
        let root = repo_root();
        let baseline = load_and_validate_compatibility_baseline(
            &root.join(COMPATIBILITY_BASELINE_PATH),
            &root,
        )
        .unwrap();
        assert_eq!(baseline.fixtures.len(), EXPECTED_COMPATIBILITY_FIXTURES);
        assert_eq!(baseline.algorithm, "sha256");
        assert_eq!(baseline.baseline_state, "candidate");
    }

    #[test]
    fn production_scope_freeze_is_closed_and_status_backed() {
        let root = repo_root();
        let scope = load_and_validate_scope_freeze(&root.join(SCOPE_FREEZE_PATH), &root).unwrap();
        assert_eq!(scope.freeze_state, "frozen");
        assert_eq!(
            scope.core_capabilities.len(),
            EXPECTED_SCOPE_CAPABILITIES.len()
        );
        assert_eq!(
            scope.accepted_closure_work.len(),
            EXPECTED_CLOSURE_WORK.len()
        );
        assert_eq!(
            scope.deferred_capabilities.len(),
            EXPECTED_DEFERRED_CAPABILITIES.len()
        );
    }

    #[test]
    fn production_patch_seal_is_contiguous_and_closure_only() {
        let root = repo_root();
        let scope = load_and_validate_scope_freeze(&root.join(SCOPE_FREEZE_PATH), &root).unwrap();
        let seal = load_and_validate_patch_seal(&root.join(PATCH_SEAL_PATH), &scope).unwrap();
        assert_eq!(seal.patch_slots.len(), 10);
        assert_eq!(seal.patch_slots[0].version, "1.20.0");
        assert_eq!(seal.patch_slots[9].version, "1.20.9");
        assert_eq!(seal.target_release, "2.0.0");
    }

    #[test]
    fn patch_seal_rejects_work_outside_the_scope_freeze() {
        let root = repo_root();
        let scope = load_and_validate_scope_freeze(&root.join(SCOPE_FREEZE_PATH), &root).unwrap();
        let mut seal = load_and_validate_patch_seal(&root.join(PATCH_SEAL_PATH), &scope).unwrap();
        seal.patch_slots[0]
            .closure_work
            .push("additional-runtime-languages".to_string());
        let path = std::env::temp_dir().join(format!(
            "gewyvern-patch-seal-scope-expansion-{}.json",
            std::process::id()
        ));
        fs::write(&path, serde_json::to_vec(&seal).unwrap()).unwrap();

        let error = load_and_validate_patch_seal(&path, &scope)
            .expect_err("patch-seal scope expansion must fail closed")
            .to_string();
        assert!(error.contains("outside the frozen closure"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn scope_freeze_rejects_non_core_status_authority() {
        let root = repo_root();
        let mut scope =
            load_and_validate_scope_freeze(&root.join(SCOPE_FREEZE_PATH), &root).unwrap();
        scope.core_capabilities[0]
            .status_cells
            .push("etragon/learning-sidecar/advisory-learning".to_string());
        let path =
            std::env::temp_dir().join(format!("gewyvern-scope-freeze-{}.json", std::process::id()));
        fs::write(&path, serde_json::to_vec(&scope).unwrap()).unwrap();

        let error = load_and_validate_scope_freeze(&path, &root)
            .expect_err("non-core status authority must fail closed")
            .to_string();
        assert!(error.contains("unavailable core status cell"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn compatibility_fingerprint_rejects_unreviewed_fixture_drift() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-compatibility-drift-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("fixture-v1.json"), "{}").unwrap();
        let fixture = CompatibilityFixture {
            id: "fixture-v1".to_string(),
            family: "wire".to_string(),
            path: "fixture-v1.json".to_string(),
            bytes: 2,
            sha256: "0".repeat(64),
        };

        assert!(validate_compatibility_fixture(&fixture, &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proof_summary_parser_rejects_vacuous_and_ambiguous_success() {
        let path = std::env::temp_dir().join(format!(
            "gewyvern-schema-freeze-proof-{}.log",
            std::process::id()
        ));
        fs::write(
            &path,
            "stdout:\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured\n",
        )
        .unwrap();
        assert_eq!(require_nonzero_test_result(&path, "fixture").unwrap(), 3);

        fs::write(
            &path,
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n",
        )
        .unwrap();
        assert!(require_nonzero_test_result(&path, "fixture").is_err());

        fs::write(
            &path,
            "test result: ok. 1 passed; 0 failed;\ntest result: ok. 1 passed; 0 failed;\n",
        )
        .unwrap();
        assert!(require_nonzero_test_result(&path, "fixture").is_err());
        fs::remove_file(path).unwrap();
    }
}
