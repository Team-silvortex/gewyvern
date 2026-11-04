use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[path = "support/leselang_upstream.rs"]
pub mod upstream;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenSourceCorePolicy {
    schema_version: u64,
    policy_id: String,
    license_spdx: String,
    official_price: String,
    account_required: bool,
    subscription_eligible: bool,
    existing_capability_reclassification_allowed: bool,
    components: Vec<OpenSourceComponent>,
    operator_capabilities: Vec<String>,
    subscription_boundary: SubscriptionBoundary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenSourceComponent {
    id: String,
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionBoundary {
    scope: String,
    may_include_existing_core_capabilities: bool,
    account_required: bool,
    entitlement_required: bool,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(repository_root().join(path)).expect("repository file must be readable")
}

fn normalized_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn component_source(component: &OpenSourceComponent) -> Result<PathBuf, String> {
    if upstream::NATIVE_CORES.contains(&component.id.as_str()) {
        let recorded = upstream::source_inventory();
        let expected = format!(
            "{}/tree/{}/crates/{}",
            recorded.repository, recorded.revision, component.id
        );
        if component.source != expected {
            return Err(format!(
                "{} must name its pinned public source, not a local pointer or mutable URL",
                component.id
            ));
        }
        Ok(upstream::checkout().join("crates").join(&component.id))
    } else {
        let relative = Path::new(&component.source);
        if !relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err("product sources must stay inside the repository".into());
        }
        Ok(repository_root().join(relative))
    }
}

#[test]
fn external_core_policy_cannot_use_navigation_directories_or_mutable_sources() {
    for id in upstream::NATIVE_CORES {
        for source in [
            format!("crates/{id}"),
            format!("https://github.com/Team-silvortex/leselang/tree/main/crates/{id}"),
            format!(
                "https://example.invalid/leselang/tree/{}/crates/{id}",
                upstream::source_inventory().revision
            ),
        ] {
            assert!(
                component_source(&OpenSourceComponent {
                    id: (*id).into(),
                    source
                })
                .is_err()
            );
        }
    }
}

#[test]
fn every_current_primary_capability_is_mit_open_source_and_free() {
    let policy: OpenSourceCorePolicy =
        serde_json::from_str(&read("project/product/open-source-core.json"))
            .expect("open-source core policy must be strict valid JSON");

    assert_eq!(policy.schema_version, 1);
    assert_eq!(policy.policy_id, "gewyvern-open-source-core");
    assert_eq!(policy.license_spdx, "MIT");
    assert_eq!(policy.official_price, "free");
    assert!(!policy.account_required);
    assert!(!policy.subscription_eligible);
    assert!(!policy.existing_capability_reclassification_allowed);
    assert_eq!(
        policy.subscription_boundary.scope,
        "future-hosted-service-extensions-only"
    );
    assert!(
        !policy
            .subscription_boundary
            .may_include_existing_core_capabilities
    );
    assert!(policy.subscription_boundary.account_required);
    assert!(policy.subscription_boundary.entitlement_required);

    let expected_capabilities = BTreeSet::from([
        "daemon-connection",
        "daemon-retirement",
        "diagnostic-export",
        "fleet-topology",
        "gewyvern-provisioning",
        "gewyvern-retirement",
        "language-management",
        "learning-center",
        "leselang-automation",
        "local-orchestra",
        "reverse-deployment",
        "runtime-debugger",
        "runtime-mutation",
        "runtime-workspace",
    ]);
    let actual_capabilities = policy
        .operator_capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_capabilities, expected_capabilities);
    assert_eq!(
        policy.operator_capabilities.len(),
        actual_capabilities.len(),
        "open-source core capability ids must be unique"
    );

    let expected_components = BTreeSet::from([
        "gewyvern-runtime",
        "gewyvern-install-contract",
        "gewyc-cli",
        "gewylang-compiler",
        "gewylang-contract",
        "gewylang-ir",
        "gewylang-syntax",
        "protocol-standard-library",
        "silvortex-bounded-io",
        "silvortex-identity",
        "leselang-command",
        "leselang-hir",
        "leselang-hir-core",
        "leserpent-hir",
        "leselang-host-contract",
        "leselang-observe",
        "leselang-runtime-core",
        "leselang-syntax",
        "leselang-ui",
        "leselang-vm",
        "leserpent-adapters",
        "leserpent-avalonia-client",
        "leserpent-cli",
        "leserpent-domain",
        "leserpent-mobile-clients",
        "leserpent-protocol",
        "leserpent-runtime",
        "leserpent-vm",
        "leserpent-web-console",
        "leserpentd",
    ]);
    let actual_components = policy
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<BTreeSet<_>>();
    let component_sources = policy
        .components
        .iter()
        .map(|component| component.source.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_components, expected_components);
    assert_eq!(policy.components.len(), actual_components.len());
    assert_eq!(policy.components.len(), component_sources.len());
    for component in &policy.components {
        let path = component_source(component).unwrap();
        assert!(
            path.is_dir(),
            "open-source component {} has no source directory {}",
            component.id,
            component.source
        );
        if upstream::NATIVE_CORES.contains(&component.id.as_str()) {
            assert!(path.join("src/lib.rs").is_file());
            let package = upstream::product_metadata()["packages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|package| package["name"] == component.id)
                .unwrap();
            assert_eq!(package["license"], "MIT");
        }
    }
    assert!(upstream::read("LICENSE").starts_with("MIT License\n"));

    let supporting_tooling =
        BTreeSet::from(["crates/gewyvern-dev", "crates/leserpent-frontend-package"]);
    for entry in
        fs::read_dir(repository_root().join("crates")).expect("crate directory must be readable")
    {
        let path = entry.expect("crate entry must be readable").path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(repository_root())
            .expect("crate path must remain inside the repository")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            component_sources.contains(relative.as_str())
                || supporting_tooling.contains(relative.as_str()),
            "product crate {relative} is missing from the open-source core manifest"
        );
    }

    let root_license = read("LICENSE");
    assert!(root_license.starts_with("MIT License\n"));
    assert_eq!(read("apps/leserpent/LICENSE"), root_license);
    assert_eq!(read("apps/etragon/LICENSE"), root_license);
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("[workspace.package]"));
    assert!(cargo.contains("license = \"MIT\""));
    for entry in
        fs::read_dir(repository_root().join("crates")).expect("crate directory must be readable")
    {
        let manifest = entry
            .expect("crate entry must be readable")
            .path()
            .join("Cargo.toml");
        if manifest.is_file() {
            let contents = fs::read_to_string(&manifest).expect("crate manifest must be readable");
            assert!(
                contents.contains("license.workspace = true"),
                "{} must inherit the MIT workspace license",
                manifest.display()
            );
        }
    }
    let dotnet = read("Directory.Build.props");
    assert!(dotnet.contains("<PackageLicenseExpression>MIT</PackageLicenseExpression>"));
    assert!(dotnet.contains("<PackageRequireLicenseAcceptance>false"));
    let frontend: serde_json::Value = serde_json::from_str(&read("apps/leserpent/package.json"))
        .expect("frontend package manifest must be valid JSON");
    assert_eq!(frontend["license"], "MIT");
    let frontend_lock: serde_json::Value =
        serde_json::from_str(&read("apps/leserpent/package-lock.json"))
            .expect("frontend package lock must be valid JSON");
    assert_eq!(frontend_lock["packages"][""]["license"], "MIT");

    let access = read("apps/leserpent-avalonia/src/Leserpent.RemoteClient/ProductAccessPolicy.cs");
    assert!(access.contains("OpenSourceLicenseSpdx = \"MIT\""));
    assert!(access.contains("OpenSourceCoreRequiresPayment => false"));
    assert!(access.contains("OpenSourceCoreMayBeSubscriptionGated => false"));
    assert!(access.contains("ReservedHostedSubscriptionService"));
    assert!(!access.contains("AnonymousCore"));
    for capability in [
        "FleetTopology",
        "LocalOrchestra",
        "DaemonConnection",
        "ReverseDeployment",
        "DaemonRetirement",
        "GewyvernProvisioning",
        "GewyvernRetirement",
        "RuntimeWorkspace",
        "RuntimeMutation",
        "RuntimeDebugger",
        "LeselangAutomation",
        "DiagnosticExport",
        "LanguageManagement",
        "LearningCenter",
    ] {
        assert!(
            access.contains(&format!("ProductCapability.{capability}")),
            "shared product policy is missing {capability}"
        );
    }

    let readme = read("README.md");
    assert!(readme.contains("## Open Source Core Guarantee"));
    assert!(readme.contains("No Team Silvortex account or subscription may gate"));
    assert!(readme.contains("project/product/open-source-core.json"));
    let architecture = normalized_whitespace(&read("docs/leserpent-2-architecture.md"));
    assert!(architecture.contains("Every current Gewyvern, GewyLang, Leserpent, and Leselang"));
    assert!(architecture.contains("may not be reclassified, metered, or moved behind"));
    let roadmap = read("docs/leserpent-2-roadmap.md");
    assert!(roadmap.contains("Every capability inside this frozen scope"));
    assert!(roadmap.contains("Future commercial work is limited"));
}
