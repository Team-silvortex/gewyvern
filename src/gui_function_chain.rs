use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const GUI_FUNCTION_CHAIN_SCHEMA_VERSION: u32 = 1;
const MAX_CATALOG_BYTES: u64 = 1024 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiFunctionChainCatalog {
    pub schema_version: u32,
    pub release_line: String,
    pub as_of: String,
    pub stages: Vec<GuiDimension>,
    pub surfaces: Vec<GuiSurface>,
    pub operations: Vec<GuiOperation>,
    pub chains: Vec<GuiFunctionChain>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiDimension {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiSurface {
    pub id: String,
    pub label: String,
    pub lifecycle: GuiSurfaceLifecycle,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuiSurfaceLifecycle {
    Target,
    Bridge,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiOperation {
    pub id: String,
    pub owner: GuiOperationOwner,
    pub audience: GuiOperationAudience,
    pub definition: GuiSourceAnchor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuiOperationOwner {
    DomainCommand,
    DomainQuery,
    Protocol,
    Product,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuiOperationAudience {
    Operator,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiSourceAnchor {
    pub path: String,
    pub contains: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiFunctionChain {
    pub id: String,
    pub label: String,
    pub release_required: bool,
    pub operations: Vec<String>,
    pub required_stages: Vec<String>,
    pub required_surfaces: Vec<String>,
    pub coverage: Vec<GuiCoverage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiCoverage {
    pub surface: String,
    pub state: GuiCoverageState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(default)]
    pub evidence: Vec<GuiStageEvidence>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuiCoverageState {
    Absent,
    ConformanceOnly,
    Partial,
    Closed,
}

impl GuiCoverageState {
    const fn score(self) -> u32 {
        match self {
            Self::Absent => 0,
            Self::ConformanceOnly => 25,
            Self::Partial => 50,
            Self::Closed => 100,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuiStageEvidence {
    pub stage: String,
    pub path: String,
    pub contains: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GuiFunctionChainSummary {
    pub schema_version: u32,
    pub release_line: String,
    pub as_of: String,
    pub operation_count: usize,
    pub chain_count: usize,
    pub target_score: u8,
    pub surfaces: Vec<GuiSurfaceSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GuiSurfaceSummary {
    pub id: String,
    pub lifecycle: GuiSurfaceLifecycle,
    pub score: u8,
    pub required_chain_count: usize,
    pub closed: usize,
    pub partial: usize,
    pub conformance_only: usize,
    pub absent: usize,
    pub gaps: Vec<String>,
}

impl GuiFunctionChainCatalog {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("cannot inspect GUI function-chain catalog: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("GUI function-chain catalog must be a regular file".to_string());
        }
        if metadata.len() > MAX_CATALOG_BYTES {
            return Err("GUI function-chain catalog exceeds the size limit".to_string());
        }
        let payload = fs::read(path)
            .map_err(|error| format!("cannot read GUI function-chain catalog: {error}"))?;
        serde_json::from_slice(&payload)
            .map_err(|error| format!("cannot decode GUI function-chain catalog: {error}"))
    }

    pub fn validate(&self, root: impl AsRef<Path>) -> Result<(), Vec<String>> {
        let root = root.as_ref();
        let mut errors = Vec::new();
        if self.schema_version != GUI_FUNCTION_CHAIN_SCHEMA_VERSION {
            errors.push(format!(
                "GUI function-chain schema must be {}, got {}",
                GUI_FUNCTION_CHAIN_SCHEMA_VERSION, self.schema_version
            ));
        }
        if self.release_line != "2.0" {
            errors.push("GUI function-chain release_line must be '2.0'".to_string());
        }
        if !valid_date(&self.as_of) {
            errors.push("GUI function-chain as_of must use YYYY-MM-DD".to_string());
        }

        let stages = validate_dimensions("stage", &self.stages, &mut errors);
        let surface_dimensions = self
            .surfaces
            .iter()
            .map(|surface| GuiDimension {
                id: surface.id.clone(),
                label: surface.label.clone(),
            })
            .collect::<Vec<_>>();
        let surfaces = validate_dimensions("surface", &surface_dimensions, &mut errors);
        if !self
            .surfaces
            .iter()
            .any(|surface| surface.lifecycle == GuiSurfaceLifecycle::Target)
        {
            errors.push("GUI function-chain catalog must declare a target surface".to_string());
        }

        let mut operations = BTreeSet::new();
        for operation in &self.operations {
            validate_slug("operation id", &operation.id, &mut errors);
            if !operations.insert(operation.id.as_str()) {
                errors.push(format!("duplicate GUI operation '{}'", operation.id));
            }
            validate_source_anchor(root, &operation.definition, "operation", &mut errors);
        }

        let mut chain_ids = BTreeSet::new();
        let mut assigned_operations = BTreeMap::<&str, &str>::new();
        for chain in &self.chains {
            validate_slug("chain id", &chain.id, &mut errors);
            require_text("chain label", &chain.label, &mut errors);
            if !chain_ids.insert(chain.id.as_str()) {
                errors.push(format!("duplicate GUI function chain '{}'", chain.id));
            }
            validate_unique_text(
                &format!("chain '{}' operations", chain.id),
                &chain.operations,
                &mut errors,
            );
            validate_unique_text(
                &format!("chain '{}' required_stages", chain.id),
                &chain.required_stages,
                &mut errors,
            );
            validate_unique_text(
                &format!("chain '{}' required_surfaces", chain.id),
                &chain.required_surfaces,
                &mut errors,
            );
            if chain.operations.is_empty() {
                errors.push(format!(
                    "chain '{}' must own at least one operation",
                    chain.id
                ));
            }
            for operation in &chain.operations {
                if !operations.contains(operation.as_str()) {
                    errors.push(format!(
                        "chain '{}' references unknown operation '{}'",
                        chain.id, operation
                    ));
                }
                if let Some(previous) = assigned_operations.insert(operation, chain.id.as_str()) {
                    errors.push(format!(
                        "operation '{}' is assigned to both '{}' and '{}'",
                        operation, previous, chain.id
                    ));
                }
            }
            if chain.release_required && chain.required_surfaces.is_empty() {
                errors.push(format!(
                    "release-required chain '{}' must name a required surface",
                    chain.id
                ));
            }
            for stage in &chain.required_stages {
                if !stages.contains(stage.as_str()) {
                    errors.push(format!(
                        "chain '{}' references unknown stage '{}'",
                        chain.id, stage
                    ));
                }
            }
            for surface in &chain.required_surfaces {
                if !surfaces.contains(surface.as_str()) {
                    errors.push(format!(
                        "chain '{}' references unknown surface '{}'",
                        chain.id, surface
                    ));
                }
            }

            let mut coverage_surfaces = BTreeSet::new();
            for coverage in &chain.coverage {
                if !surfaces.contains(coverage.surface.as_str()) {
                    errors.push(format!(
                        "chain '{}' coverage references unknown surface '{}'",
                        chain.id, coverage.surface
                    ));
                }
                if !coverage_surfaces.insert(coverage.surface.as_str()) {
                    errors.push(format!(
                        "chain '{}' has duplicate coverage for '{}'",
                        chain.id, coverage.surface
                    ));
                }
                validate_coverage(root, chain, coverage, &stages, &mut errors);
            }
            for surface in &chain.required_surfaces {
                if !coverage_surfaces.contains(surface.as_str()) {
                    errors.push(format!(
                        "chain '{}' has no coverage decision for required surface '{}'",
                        chain.id, surface
                    ));
                }
            }
        }
        for operation in &self.operations {
            if !assigned_operations.contains_key(operation.id.as_str()) {
                errors.push(format!(
                    "GUI operation '{}' is not assigned to a function chain",
                    operation.id
                ));
            } else if operation.audience == GuiOperationAudience::Operator {
                let chain_id = assigned_operations[operation.id.as_str()];
                if self
                    .chains
                    .iter()
                    .find(|chain| chain.id == chain_id)
                    .is_some_and(|chain| !chain.release_required)
                {
                    errors.push(format!(
                        "operator GUI operation '{}' cannot be hidden in non-release chain '{}'",
                        operation.id, chain_id
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn summary(&self) -> GuiFunctionChainSummary {
        let mut surfaces = Vec::with_capacity(self.surfaces.len());
        let mut target_score_sum = 0u32;
        let mut target_score_count = 0u32;
        for surface in &self.surfaces {
            let relevant = self
                .chains
                .iter()
                .filter(|chain| {
                    chain.release_required
                        && chain.required_surfaces.iter().any(|id| id == &surface.id)
                })
                .filter_map(|chain| {
                    chain
                        .coverage
                        .iter()
                        .find(|coverage| coverage.surface == surface.id)
                        .map(|coverage| (chain, coverage))
                })
                .collect::<Vec<_>>();
            let score_sum = relevant
                .iter()
                .map(|(_, coverage)| coverage.state.score())
                .sum::<u32>();
            let score = rounded_score(score_sum, relevant.len());
            if surface.lifecycle == GuiSurfaceLifecycle::Target {
                target_score_sum += score_sum;
                target_score_count += relevant.len() as u32;
            }
            surfaces.push(GuiSurfaceSummary {
                id: surface.id.clone(),
                lifecycle: surface.lifecycle,
                score,
                required_chain_count: relevant.len(),
                closed: count_state(&relevant, GuiCoverageState::Closed),
                partial: count_state(&relevant, GuiCoverageState::Partial),
                conformance_only: count_state(&relevant, GuiCoverageState::ConformanceOnly),
                absent: count_state(&relevant, GuiCoverageState::Absent),
                gaps: relevant
                    .iter()
                    .filter(|(_, coverage)| coverage.state != GuiCoverageState::Closed)
                    .map(|(chain, _)| chain.id.clone())
                    .collect(),
            });
        }
        GuiFunctionChainSummary {
            schema_version: self.schema_version,
            release_line: self.release_line.clone(),
            as_of: self.as_of.clone(),
            operation_count: self.operations.len(),
            chain_count: self.chains.len(),
            target_score: if target_score_count == 0 {
                0
            } else {
                ((target_score_sum + target_score_count / 2) / target_score_count) as u8
            },
            surfaces,
        }
    }
}

pub fn default_gui_function_chain_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref()
        .join("project/release/leserpent-gui-function-chain.json")
}

fn validate_coverage(
    root: &Path,
    chain: &GuiFunctionChain,
    coverage: &GuiCoverage,
    stages: &BTreeSet<&str>,
    errors: &mut Vec<String>,
) {
    let needs_gap = coverage.state != GuiCoverageState::Closed;
    if needs_gap {
        match coverage.gap.as_deref() {
            Some(gap) => require_text(&format!("chain '{}' coverage gap", chain.id), gap, errors),
            None => errors.push(format!(
                "chain '{}' non-closed coverage for '{}' must explain its gap",
                chain.id, coverage.surface
            )),
        }
    } else if coverage.gap.is_some() {
        errors.push(format!(
            "chain '{}' closed coverage for '{}' cannot retain a gap",
            chain.id, coverage.surface
        ));
    }
    if coverage.state == GuiCoverageState::Absent && !coverage.evidence.is_empty() {
        errors.push(format!(
            "chain '{}' absent coverage for '{}' cannot claim evidence",
            chain.id, coverage.surface
        ));
    }
    if coverage.state != GuiCoverageState::Absent && coverage.evidence.is_empty() {
        errors.push(format!(
            "chain '{}' coverage for '{}' must claim evidence",
            chain.id, coverage.surface
        ));
    }

    let mut evidence_keys = BTreeSet::new();
    let mut covered_stages = BTreeSet::new();
    for evidence in &coverage.evidence {
        if !stages.contains(evidence.stage.as_str()) {
            errors.push(format!(
                "chain '{}' evidence references unknown stage '{}'",
                chain.id, evidence.stage
            ));
        }
        covered_stages.insert(evidence.stage.as_str());
        let key = (
            evidence.stage.as_str(),
            evidence.path.as_str(),
            evidence.contains.as_str(),
        );
        if !evidence_keys.insert(key) {
            errors.push(format!(
                "chain '{}' has duplicate evidence for '{}'",
                chain.id, evidence.stage
            ));
        }
        validate_source_anchor(
            root,
            &GuiSourceAnchor {
                path: evidence.path.clone(),
                contains: evidence.contains.clone(),
            },
            &format!("chain '{}' evidence", chain.id),
            errors,
        );
    }
    if coverage.state == GuiCoverageState::Closed {
        for stage in &chain.required_stages {
            if !covered_stages.contains(stage.as_str()) {
                errors.push(format!(
                    "chain '{}' closed coverage for '{}' lacks '{}' evidence",
                    chain.id, coverage.surface, stage
                ));
            }
        }
    }
}

fn validate_source_anchor(
    root: &Path,
    anchor: &GuiSourceAnchor,
    context: &str,
    errors: &mut Vec<String>,
) {
    require_text(&format!("{context} path"), &anchor.path, errors);
    require_text(&format!("{context} contains"), &anchor.contains, errors);
    let path = Path::new(&anchor.path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        errors.push(format!(
            "{context} must stay inside the repository: {}",
            anchor.path
        ));
        return;
    }
    let full_path = root.join(path);
    let Ok(metadata) = fs::symlink_metadata(&full_path) else {
        errors.push(format!("{context} path does not exist: {}", anchor.path));
        return;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        errors.push(format!(
            "{context} path must be a regular file: {}",
            anchor.path
        ));
        return;
    }
    let canonical_root = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(error) => {
            errors.push(format!(
                "{context} repository root cannot be resolved ({}): {error}",
                root.display()
            ));
            return;
        }
    };
    let canonical_path = match fs::canonicalize(&full_path) {
        Ok(path) => path,
        Err(error) => {
            errors.push(format!(
                "{context} path cannot be resolved ({}): {error}",
                anchor.path
            ));
            return;
        }
    };
    if !canonical_path.starts_with(&canonical_root) {
        errors.push(format!(
            "{context} path resolves outside the repository: {}",
            anchor.path
        ));
        return;
    }
    if metadata.len() > MAX_EVIDENCE_BYTES {
        errors.push(format!(
            "{context} path exceeds the evidence size limit: {}",
            anchor.path
        ));
        return;
    }
    match fs::read_to_string(&full_path) {
        Ok(source) if !source.contains(&anchor.contains) => errors.push(format!(
            "{context} anchor is missing from {}: {}",
            anchor.path, anchor.contains
        )),
        Ok(_) => {}
        Err(error) => errors.push(format!(
            "{context} path is not readable UTF-8 ({}): {error}",
            anchor.path
        )),
    }
}

fn validate_dimensions<'a>(
    kind: &str,
    values: &'a [GuiDimension],
    errors: &mut Vec<String>,
) -> BTreeSet<&'a str> {
    let mut ids = BTreeSet::new();
    for value in values {
        validate_slug(&format!("{kind} id"), &value.id, errors);
        require_text(&format!("{kind} label"), &value.label, errors);
        if !ids.insert(value.id.as_str()) {
            errors.push(format!("duplicate GUI {kind} '{}'", value.id));
        }
    }
    if values.is_empty() {
        errors.push(format!("GUI function-chain catalog must declare {kind}s"));
    }
    ids
}

fn validate_unique_text(context: &str, values: &[String], errors: &mut Vec<String>) {
    let mut unique = BTreeSet::new();
    for value in values {
        require_text(context, value, errors);
        if !unique.insert(value) {
            errors.push(format!("{context} contains duplicate '{value}'"));
        }
    }
}

fn validate_slug(context: &str, value: &str, errors: &mut Vec<String>) {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if !valid {
        errors.push(format!(
            "{context} must be a bounded lowercase slug: '{value}'"
        ));
    }
}

fn require_text(context: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        errors.push(format!("{context} must be bounded printable text"));
    }
}

fn valid_date(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn count_state(values: &[(&GuiFunctionChain, &GuiCoverage)], state: GuiCoverageState) -> usize {
    values
        .iter()
        .filter(|(_, coverage)| coverage.state == state)
        .count()
}

fn rounded_score(score_sum: u32, count: usize) -> u8 {
    if count == 0 {
        0
    } else {
        ((score_sum + count as u32 / 2) / count as u32) as u8
    }
}
