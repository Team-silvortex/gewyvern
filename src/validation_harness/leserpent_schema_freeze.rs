use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_status,
};
use super::dotnet_proof::run_locked_dotnet_test;
use super::read_bounded_json_file;

const INVENTORY_PATH: &str = "project/release/leserpent-v1-schema-inventory.json";
const COMPATIBILITY_BASELINE_PATH: &str =
    "project/release/leserpent-v1-compatibility-baseline.json";
const EXPECTED_FAMILIES: &[&str] = &["command", "effect", "query", "ui", "wire"];
const EXPECTED_FIXTURE_FAMILIES: &[&str] = &["legacy-wire", "ui", "wire"];
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

pub fn run_leserpent_schema_freeze_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let root = repo_root();
    let inventory = load_and_validate_inventory(&root.join(INVENTORY_PATH), &root)?;
    let compatibility =
        load_and_validate_compatibility_baseline(&root.join(COMPATIBILITY_BASELINE_PATH), &root)?;
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
    let summary_name = "schema-freeze-summary.json";
    fs::write(
        out_dir.join(summary_name),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "release_line": inventory.release_line,
            "inventory": INVENTORY_PATH,
            "compatibility_baseline": COMPATIBILITY_BASELINE_PATH,
            "freeze_state": inventory.freeze_state,
            "freeze_ready": freeze_ready,
            "contract_count": inventory.contracts.len(),
            "compatibility_fixture_count": compatibility.fixtures.len(),
            "proof_count": proof_summaries.len(),
            "test_count": total_test_count,
            "contracts": inventory.contracts,
            "compatibility_fixtures": compatibility.fixtures,
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
    if baseline.fixtures.len() != 11 {
        return Err(ValidationError::new(
            "Leserpent compatibility baseline must contain exactly eleven v1 fixtures",
        ));
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
    fn production_compatibility_baseline_covers_all_nine_v1_fixtures() {
        let root = repo_root();
        let baseline = load_and_validate_compatibility_baseline(
            &root.join(COMPATIBILITY_BASELINE_PATH),
            &root,
        )
        .unwrap();
        assert_eq!(baseline.fixtures.len(), 9);
        assert_eq!(baseline.algorithm, "sha256");
        assert_eq!(baseline.baseline_state, "candidate");
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
