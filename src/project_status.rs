use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::machine_error::{ErrorCategory, MachineError};
use serde::{Deserialize, Serialize};

pub const STATUS_SCHEMA_VERSION: u32 = 3;
pub const STATUS_CALIBRATION_MODEL: &str = "priority-weighted-strength-v1";

#[derive(Debug)]
pub enum StatusCatalogLoadError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl StatusCatalogLoadError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Read { .. } => "status_catalog_read_failed",
            Self::Decode { .. } => "status_catalog_decode_failed",
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(self, Self::Read { .. })
    }

    pub const fn category(&self) -> ErrorCategory {
        match self {
            Self::Read { .. } => ErrorCategory::Io,
            Self::Decode { .. } => ErrorCategory::Configuration,
        }
    }
}

impl std::fmt::Display for StatusCatalogLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, source } => write!(
                formatter,
                "failed to read status catalog '{}': {source}",
                path.display()
            ),
            Self::Decode { path, source } => write!(
                formatter,
                "failed to decode status catalog '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for StatusCatalogLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
        }
    }
}

impl From<StatusCatalogLoadError> for MachineError {
    fn from(error: StatusCatalogLoadError) -> Self {
        let code = error.code();
        let category = error.category();
        let retryable = error.retryable();
        Self::new(code, category, error.to_string(), retryable, 1)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusCatalog {
    pub schema_version: u32,
    pub project: String,
    pub checkpoint: String,
    pub calibration: StatusCalibration,
    pub dimensions: StatusDimensions,
    pub coverage_requirements: Vec<StatusCoverageRequirement>,
    pub cells: Vec<StatusCell>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusCalibration {
    pub model: String,
    pub as_of: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusDimensions {
    pub architectures: Vec<DimensionEntry>,
    pub modules: Vec<DimensionEntry>,
    pub features: Vec<DimensionEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DimensionEntry {
    pub id: String,
    pub label: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusCoverageRequirement {
    pub id: String,
    pub architecture: String,
    pub kind: CoverageKind,
    pub summary: String,
    pub source: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusCell {
    pub id: String,
    pub architecture: String,
    pub module: String,
    pub feature: String,
    pub lifecycle: Lifecycle,
    pub priority: Priority,
    pub maturity: Maturity,
    pub completion: u8,
    pub confidence: Confidence,
    pub independence: Independence,
    pub contract: StatusContract,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<StatusBlocker>,
    #[serde(default)]
    pub consumers: Vec<String>,
    pub evidence: Vec<StatusEvidence>,
    pub next_gate: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusContract {
    pub id: String,
    pub version: String,
    pub stability: ContractStability,
    pub surfaces: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusBlocker {
    pub id: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StatusEvidence {
    pub kind: EvidenceKind,
    pub path: String,
    pub state: EvidenceState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    Current,
    Bridge,
    Target,
    Retired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    Critical,
    Active,
    Maintenance,
    Deferred,
}

impl Priority {
    pub const fn delivery_weight(self) -> u8 {
        match self {
            Self::Critical => 4,
            Self::Active => 2,
            Self::Maintenance => 1,
            Self::Deferred => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    Planned,
    Incubating,
    Developing,
    Stabilizing,
    Mature,
    Deprecated,
    Blocked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Independence {
    Internal,
    ReusableLibrary,
    StandaloneTool,
    StandaloneService,
    ReplaceableFrontend,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractStability {
    Draft,
    Evolving,
    Stable,
    Deprecated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    Source,
    Test,
    Documentation,
    Benchmark,
    Release,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceState {
    Present,
    Planned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageKind {
    OwnershipBoundary,
    RoadmapGate,
    ProofShelf,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusSummary {
    pub schema_version: u32,
    pub project: String,
    pub checkpoint: String,
    pub calibration: StatusCalibration,
    pub cell_count: usize,
    pub deferred_cell_count: usize,
    pub coverage: StatusCoverageSummary,
    pub overall_score: u8,
    pub portfolio_score: u8,
    pub delivery_completion: u8,
    pub portfolio_completion: u8,
    pub lifecycles: Vec<StatusGroupSummary>,
    pub architectures: Vec<StatusGroupSummary>,
    pub modules: Vec<StatusGroupSummary>,
    pub weakest: Vec<StatusCellView>,
    pub independently_usable: Vec<StatusCellView>,
    pub in_development: Vec<StatusCellView>,
    pub deferred: Vec<StatusCellView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusCoverageSummary {
    pub requirement_count: usize,
    pub architecture_count: usize,
    pub ownership_boundary_count: usize,
    pub roadmap_gate_count: usize,
    pub proof_shelf_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusGroupSummary {
    pub id: String,
    pub label: String,
    pub score: u8,
    pub completion: u8,
    pub cell_count: usize,
    pub active_cell_count: usize,
    pub weakest_cell: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusCellView {
    pub id: String,
    pub architecture: String,
    pub module: String,
    pub feature: String,
    pub lifecycle: Lifecycle,
    pub priority: Priority,
    pub maturity: Maturity,
    pub completion: u8,
    pub confidence: Confidence,
    pub independence: Independence,
    pub score: u8,
    pub contract_id: String,
    pub contract_version: String,
    pub contract_stability: ContractStability,
    pub blocker_count: usize,
    pub next_gate: String,
}

impl StatusCatalog {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::load_typed(path).map_err(|error| error.to_string())
    }

    pub fn load_typed(path: impl AsRef<Path>) -> Result<Self, StatusCatalogLoadError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|source| StatusCatalogLoadError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&source).map_err(|source| StatusCatalogLoadError::Decode {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn validate(&self, repository_root: impl AsRef<Path>) -> Result<(), Vec<String>> {
        let root = repository_root.as_ref();
        let mut errors = Vec::new();
        if self.schema_version != STATUS_SCHEMA_VERSION {
            errors.push(format!(
                "unsupported schema_version {}, expected {STATUS_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        require_text("project", &self.project, &mut errors);
        require_text("checkpoint", &self.checkpoint, &mut errors);
        require_text("calibration.model", &self.calibration.model, &mut errors);
        require_text("calibration.as_of", &self.calibration.as_of, &mut errors);
        if self.calibration.model != STATUS_CALIBRATION_MODEL {
            errors.push(format!(
                "unsupported calibration model '{}', expected '{STATUS_CALIBRATION_MODEL}'",
                self.calibration.model
            ));
        }
        if !is_iso_date(&self.calibration.as_of) {
            errors.push("calibration.as_of must use YYYY-MM-DD".to_string());
        }

        let architectures =
            validate_dimension("architecture", &self.dimensions.architectures, &mut errors);
        let modules = validate_dimension("module", &self.dimensions.modules, &mut errors);
        let features = validate_dimension("feature", &self.dimensions.features, &mut errors);

        let mut cell_ids = BTreeSet::new();
        let mut used_architectures = BTreeSet::new();
        let mut used_modules = BTreeSet::new();
        let mut used_features = BTreeSet::new();
        let mut delivery_cells = 0usize;
        for cell in &self.cells {
            if !cell_ids.insert(cell.id.as_str()) {
                errors.push(format!("duplicate cell id '{}'", cell.id));
            }
            let expected_id = format!("{}/{}/{}", cell.architecture, cell.module, cell.feature);
            if cell.id != expected_id {
                errors.push(format!(
                    "cell '{}' must use canonical id '{}'",
                    cell.id, expected_id
                ));
            }
            if !architectures.contains(cell.architecture.as_str()) {
                errors.push(format!(
                    "cell '{}' references unknown architecture '{}'",
                    cell.id, cell.architecture
                ));
            }
            used_architectures.insert(cell.architecture.as_str());
            used_modules.insert(cell.module.as_str());
            used_features.insert(cell.feature.as_str());
            if cell.priority.delivery_weight() > 0 {
                delivery_cells += 1;
            }
            if cell.completion > 100 {
                errors.push(format!(
                    "cell '{}' completion must be between 0 and 100",
                    cell.id
                ));
            }
            if cell.maturity == Maturity::Planned && cell.completion > 25 {
                errors.push(format!(
                    "planned cell '{}' cannot report completion above 25",
                    cell.id
                ));
            }
            if cell.maturity == Maturity::Incubating && cell.completion > 75 {
                errors.push(format!(
                    "incubating cell '{}' cannot report completion above 75; recalibrate its maturity",
                    cell.id
                ));
            }
            if cell.maturity == Maturity::Stabilizing && cell.completion < 70 {
                errors.push(format!(
                    "stabilizing cell '{}' must report completion of at least 70",
                    cell.id
                ));
            }
            if !modules.contains(cell.module.as_str()) {
                errors.push(format!(
                    "cell '{}' references unknown module '{}'",
                    cell.id, cell.module
                ));
            }
            if !features.contains(cell.feature.as_str()) {
                errors.push(format!(
                    "cell '{}' references unknown feature '{}'",
                    cell.id, cell.feature
                ));
            }
            require_text(
                &format!("cell '{}' contract.id", cell.id),
                &cell.contract.id,
                &mut errors,
            );
            require_text(
                &format!("cell '{}' contract.version", cell.id),
                &cell.contract.version,
                &mut errors,
            );
            require_text(
                &format!("cell '{}' next_gate", cell.id),
                &cell.next_gate,
                &mut errors,
            );
            if cell.contract.surfaces.is_empty() {
                errors.push(format!(
                    "cell '{}' contract must declare at least one surface",
                    cell.id
                ));
            }
            validate_slug(
                &format!("cell '{}' contract.id", cell.id),
                &cell.contract.id,
                &mut errors,
            );
            validate_unique_text(
                &format!("cell '{}' contract.surfaces", cell.id),
                &cell.contract.surfaces,
                &mut errors,
            );
            validate_unique_text(
                &format!("cell '{}' depends_on", cell.id),
                &cell.depends_on,
                &mut errors,
            );
            validate_unique_text(
                &format!("cell '{}' consumers", cell.id),
                &cell.consumers,
                &mut errors,
            );
            let mut blocker_ids = BTreeSet::new();
            for blocker in &cell.blockers {
                validate_slug(
                    &format!("cell '{}' blocker.id", cell.id),
                    &blocker.id,
                    &mut errors,
                );
                require_text(
                    &format!("cell '{}' blocker.summary", cell.id),
                    &blocker.summary,
                    &mut errors,
                );
                if !blocker_ids.insert(blocker.id.as_str()) {
                    errors.push(format!(
                        "cell '{}' has duplicate blocker '{}'",
                        cell.id, blocker.id
                    ));
                }
            }
            if cell.evidence.is_empty() {
                errors.push(format!("cell '{}' must declare evidence", cell.id));
            }
            for evidence in &cell.evidence {
                require_text(
                    &format!("cell '{}' evidence.path", cell.id),
                    &evidence.path,
                    &mut errors,
                );
                let evidence_path = Path::new(&evidence.path);
                if evidence_path.is_absolute()
                    || evidence_path
                        .components()
                        .any(|component| component == std::path::Component::ParentDir)
                {
                    errors.push(format!(
                        "cell '{}' evidence must stay inside the repository: {}",
                        cell.id, evidence.path
                    ));
                    continue;
                }
                if evidence.state == EvidenceState::Present && !root.join(&evidence.path).exists() {
                    errors.push(format!(
                        "cell '{}' present evidence does not exist: {}",
                        cell.id, evidence.path
                    ));
                }
            }
            if cell.maturity == Maturity::Mature {
                if cell.completion < 85 {
                    errors.push(format!(
                        "mature cell '{}' must have completion >= 85",
                        cell.id
                    ));
                }
                if cell.contract.stability != ContractStability::Stable {
                    errors.push(format!(
                        "mature cell '{}' must have a stable contract",
                        cell.id
                    ));
                }
                if !cell.blockers.is_empty() {
                    errors.push(format!("mature cell '{}' cannot have blockers", cell.id));
                }
                if !cell.evidence.iter().any(|item| {
                    item.kind == EvidenceKind::Test && item.state == EvidenceState::Present
                }) {
                    errors.push(format!(
                        "mature cell '{}' must have present test evidence",
                        cell.id
                    ));
                }
            }
        }
        if delivery_cells == 0 {
            errors.push("status catalog must contain at least one non-deferred cell".to_string());
        }

        let mut requirement_ids = BTreeSet::new();
        let mut coverage_architectures = BTreeSet::new();
        let mut covered_cells = BTreeSet::new();
        for requirement in &self.coverage_requirements {
            validate_slug("coverage requirement id", &requirement.id, &mut errors);
            if !requirement_ids.insert(requirement.id.as_str()) {
                errors.push(format!(
                    "duplicate coverage requirement id '{}'",
                    requirement.id
                ));
            }
            if !architectures.contains(requirement.architecture.as_str()) {
                errors.push(format!(
                    "coverage requirement '{}' references unknown architecture '{}'",
                    requirement.id, requirement.architecture
                ));
            }
            coverage_architectures.insert(requirement.architecture.as_str());
            require_text(
                &format!("coverage requirement '{}'.summary", requirement.id),
                &requirement.summary,
                &mut errors,
            );
            validate_coverage_source(root, requirement, &mut errors);
            if requirement.cells.is_empty() {
                errors.push(format!(
                    "coverage requirement '{}' must map to at least one cell",
                    requirement.id
                ));
            }
            validate_unique_text(
                &format!("coverage requirement '{}'.cells", requirement.id),
                &requirement.cells,
                &mut errors,
            );
            for covered_cell in &requirement.cells {
                covered_cells.insert(covered_cell.as_str());
                match self.cells.iter().find(|cell| cell.id == *covered_cell) {
                    None => errors.push(format!(
                        "coverage requirement '{}' references unknown cell '{}'",
                        requirement.id, covered_cell
                    )),
                    Some(cell) if cell.architecture != requirement.architecture => errors.push(
                        format!(
                            "coverage requirement '{}' cannot map architecture '{}' to cell '{}' in architecture '{}'",
                            requirement.id,
                            requirement.architecture,
                            covered_cell,
                            cell.architecture
                        ),
                    ),
                    Some(_) => {}
                }
            }
        }
        for architecture in &used_architectures {
            if !coverage_architectures.contains(architecture) {
                errors.push(format!(
                    "architecture '{architecture}' has cells but no coverage requirements"
                ));
            }
        }
        for cell in &self.cells {
            if coverage_architectures.contains(cell.architecture.as_str())
                && !covered_cells.contains(cell.id.as_str())
            {
                errors.push(format!(
                    "cell '{}' is missing from the '{}' coverage manifest",
                    cell.id, cell.architecture
                ));
            }
        }

        validate_dimension_usage(
            "architecture",
            &architectures,
            &used_architectures,
            &mut errors,
        );
        validate_dimension_usage("module", &modules, &used_modules, &mut errors);
        validate_dimension_usage("feature", &features, &used_features, &mut errors);

        for cell in &self.cells {
            for dependency in &cell.depends_on {
                if dependency == &cell.id {
                    errors.push(format!("cell '{}' cannot depend on itself", cell.id));
                } else if !cell_ids.contains(dependency.as_str()) {
                    errors.push(format!(
                        "cell '{}' references unknown dependency '{}'",
                        cell.id, dependency
                    ));
                }
            }
        }
        validate_dependency_cycles(&self.cells, &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn summary(&self, limit: usize) -> StatusSummary {
        let architectures = self.summarize_dimension(&self.dimensions.architectures, |cell, id| {
            cell.architecture == id
        });
        let modules =
            self.summarize_dimension(&self.dimensions.modules, |cell, id| cell.module == id);
        let lifecycle_dimensions = [
            (Lifecycle::Current, "current", "Current"),
            (Lifecycle::Bridge, "bridge", "Bridge"),
            (Lifecycle::Target, "target", "Target"),
            (Lifecycle::Retired, "retired", "Retired"),
        ];
        let mut lifecycles = lifecycle_dimensions
            .into_iter()
            .filter_map(|(lifecycle, id, label)| {
                let cells = self
                    .cells
                    .iter()
                    .filter(|cell| cell.lifecycle == lifecycle)
                    .collect::<Vec<_>>();
                self.group_summary(id, label, &cells)
            })
            .collect::<Vec<_>>();
        lifecycles.sort_by_key(|group| std::cmp::Reverse(group.score));

        let delivery_cells = self
            .cells
            .iter()
            .filter(|cell| cell.priority != Priority::Deferred)
            .collect::<Vec<_>>();
        let mut weakest = delivery_cells.clone();
        weakest.sort_by_key(|cell| (self.cell_score(cell), cell.id.as_str()));

        let mut independently_usable = self
            .cells
            .iter()
            .filter(|cell| {
                cell.priority != Priority::Deferred
                    && cell.independence != Independence::Internal
                    && cell.maturity >= Maturity::Stabilizing
                    && !matches!(cell.maturity, Maturity::Blocked | Maturity::Deprecated)
            })
            .collect::<Vec<_>>();
        independently_usable
            .sort_by_key(|cell| (std::cmp::Reverse(self.cell_score(cell)), cell.id.as_str()));

        let mut in_development = self
            .cells
            .iter()
            .filter(|cell| {
                cell.priority != Priority::Deferred
                    && matches!(
                        cell.maturity,
                        Maturity::Incubating
                            | Maturity::Developing
                            | Maturity::Stabilizing
                            | Maturity::Blocked
                    )
            })
            .collect::<Vec<_>>();
        in_development.sort_by_key(|cell| (self.cell_score(cell), cell.id.as_str()));

        let mut deferred = self
            .cells
            .iter()
            .filter(|cell| cell.priority == Priority::Deferred)
            .collect::<Vec<_>>();
        deferred.sort_by_key(|cell| (self.cell_score(cell), cell.id.as_str()));

        StatusSummary {
            schema_version: self.schema_version,
            project: self.project.clone(),
            checkpoint: self.checkpoint.clone(),
            calibration: self.calibration.clone(),
            cell_count: self.cells.len(),
            deferred_cell_count: deferred.len(),
            coverage: StatusCoverageSummary {
                requirement_count: self.coverage_requirements.len(),
                architecture_count: self
                    .coverage_requirements
                    .iter()
                    .map(|item| item.architecture.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
                ownership_boundary_count: self
                    .coverage_requirements
                    .iter()
                    .filter(|item| item.kind == CoverageKind::OwnershipBoundary)
                    .count(),
                roadmap_gate_count: self
                    .coverage_requirements
                    .iter()
                    .filter(|item| item.kind == CoverageKind::RoadmapGate)
                    .count(),
                proof_shelf_count: self
                    .coverage_requirements
                    .iter()
                    .filter(|item| item.kind == CoverageKind::ProofShelf)
                    .count(),
            },
            overall_score: weighted_average(
                delivery_cells
                    .iter()
                    .map(|cell| (self.cell_score(cell), cell.priority.delivery_weight())),
            ),
            portfolio_score: average(self.cells.iter().map(|cell| self.cell_score(cell))),
            delivery_completion: weighted_average(
                delivery_cells
                    .iter()
                    .map(|cell| (cell.completion, cell.priority.delivery_weight())),
            ),
            portfolio_completion: average(self.cells.iter().map(|cell| cell.completion)),
            lifecycles,
            architectures,
            modules,
            weakest: weakest
                .into_iter()
                .take(limit)
                .map(|cell| self.cell_view(cell))
                .collect(),
            independently_usable: independently_usable
                .into_iter()
                .take(limit)
                .map(|cell| self.cell_view(cell))
                .collect(),
            in_development: in_development
                .into_iter()
                .take(limit)
                .map(|cell| self.cell_view(cell))
                .collect(),
            deferred: deferred
                .into_iter()
                .take(limit)
                .map(|cell| self.cell_view(cell))
                .collect(),
        }
    }

    pub fn views(&self) -> Vec<StatusCellView> {
        self.cells.iter().map(|cell| self.cell_view(cell)).collect()
    }

    pub fn cell_score(&self, cell: &StatusCell) -> u8 {
        let maturity = match cell.maturity {
            Maturity::Planned => 5u16,
            Maturity::Incubating => 20,
            Maturity::Developing => 45,
            Maturity::Stabilizing => 75,
            Maturity::Mature => 100,
            Maturity::Deprecated => 35,
            Maturity::Blocked => 10,
        };
        let confidence_penalty = match cell.confidence {
            Confidence::Low => 10,
            Confidence::Medium => 4,
            Confidence::High => 0,
        };
        let blocker_penalty = (cell.blockers.len() as u8).saturating_mul(6).min(24);
        let weighted = ((maturity * 55 + u16::from(cell.completion) * 45) / 100) as u8;
        weighted
            .saturating_sub(confidence_penalty)
            .saturating_sub(blocker_penalty)
    }

    fn cell_view(&self, cell: &StatusCell) -> StatusCellView {
        StatusCellView {
            id: cell.id.clone(),
            architecture: cell.architecture.clone(),
            module: cell.module.clone(),
            feature: cell.feature.clone(),
            lifecycle: cell.lifecycle,
            priority: cell.priority,
            maturity: cell.maturity,
            completion: cell.completion,
            confidence: cell.confidence,
            independence: cell.independence,
            score: self.cell_score(cell),
            contract_id: cell.contract.id.clone(),
            contract_version: cell.contract.version.clone(),
            contract_stability: cell.contract.stability,
            blocker_count: cell.blockers.len(),
            next_gate: cell.next_gate.clone(),
        }
    }

    fn summarize_dimension(
        &self,
        dimensions: &[DimensionEntry],
        belongs: impl Fn(&StatusCell, &str) -> bool,
    ) -> Vec<StatusGroupSummary> {
        let mut groups = dimensions
            .iter()
            .filter_map(|dimension| {
                let cells = self
                    .cells
                    .iter()
                    .filter(|cell| belongs(cell, &dimension.id))
                    .collect::<Vec<_>>();
                self.group_summary(&dimension.id, &dimension.label, &cells)
            })
            .collect::<Vec<_>>();
        groups.sort_by_key(|group| std::cmp::Reverse(group.score));
        groups
    }

    fn group_summary(
        &self,
        id: &str,
        label: &str,
        cells: &[&StatusCell],
    ) -> Option<StatusGroupSummary> {
        let active = cells
            .iter()
            .copied()
            .filter(|cell| cell.priority != Priority::Deferred)
            .collect::<Vec<_>>();
        let ranked = if active.is_empty() {
            cells
        } else {
            active.as_slice()
        };
        let weakest = ranked.iter().min_by_key(|cell| self.cell_score(cell))?;
        let score = if active.is_empty() {
            average(cells.iter().map(|cell| self.cell_score(cell)))
        } else {
            weighted_average(
                active
                    .iter()
                    .map(|cell| (self.cell_score(cell), cell.priority.delivery_weight())),
            )
        };
        let completion = if active.is_empty() {
            average(cells.iter().map(|cell| cell.completion))
        } else {
            weighted_average(
                active
                    .iter()
                    .map(|cell| (cell.completion, cell.priority.delivery_weight())),
            )
        };
        Some(StatusGroupSummary {
            id: id.to_string(),
            label: label.to_string(),
            score,
            completion,
            cell_count: cells.len(),
            active_cell_count: active.len(),
            weakest_cell: weakest.id.clone(),
        })
    }
}

pub fn default_catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("project/status/catalog.json")
}

fn validate_dimension<'a>(
    name: &str,
    values: &'a [DimensionEntry],
    errors: &mut Vec<String>,
) -> BTreeSet<&'a str> {
    let mut ids = BTreeSet::new();
    if values.is_empty() {
        errors.push(format!("{name} dimension cannot be empty"));
    }
    for value in values {
        require_text(&format!("{name}.id"), &value.id, errors);
        require_text(
            &format!("{name} '{}'.label", value.id),
            &value.label,
            errors,
        );
        require_text(
            &format!("{name} '{}'.summary", value.id),
            &value.summary,
            errors,
        );
        if !ids.insert(value.id.as_str()) {
            errors.push(format!("duplicate {name} id '{}'", value.id));
        }
        validate_slug(&format!("{name}.id"), &value.id, errors);
    }
    ids
}

fn validate_slug(field: &str, value: &str, errors: &mut Vec<String>) {
    let valid = !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.as_bytes().windows(2).any(|pair| pair == b"--");
    if !valid {
        errors.push(format!("{field} must be a lowercase kebab-case identifier"));
    }
}

fn validate_unique_text(field: &str, values: &[String], errors: &mut Vec<String>) {
    let mut unique = BTreeSet::new();
    for value in values {
        require_text(field, value, errors);
        if !unique.insert(value.as_str()) {
            errors.push(format!("{field} contains duplicate value '{value}'"));
        }
    }
}

fn validate_dimension_usage(
    name: &str,
    declared: &BTreeSet<&str>,
    used: &BTreeSet<&str>,
    errors: &mut Vec<String>,
) {
    for id in declared.difference(used) {
        errors.push(format!("unused {name} dimension '{id}'"));
    }
}

fn require_text(field: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{field} cannot be empty"));
    }
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| !matches!(index, 4 | 7) && !byte.is_ascii_digit())
    {
        return false;
    }
    let Ok(year) = value[..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u8>() else {
        return false;
    };
    let Ok(day) = value[8..].parse::<u8>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year > 0 && day >= 1 && day <= maximum_day
}

fn validate_coverage_source(
    root: &Path,
    requirement: &StatusCoverageRequirement,
    errors: &mut Vec<String>,
) {
    require_text(
        &format!("coverage requirement '{}'.source", requirement.id),
        &requirement.source,
        errors,
    );
    let source = Path::new(&requirement.source);
    if source.is_absolute()
        || source
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        errors.push(format!(
            "coverage requirement '{}' source must stay inside the repository: {}",
            requirement.id, requirement.source
        ));
    } else if !root.join(source).exists() {
        errors.push(format!(
            "coverage requirement '{}' source does not exist: {}",
            requirement.id, requirement.source
        ));
    }
}

fn validate_dependency_cycles(cells: &[StatusCell], errors: &mut Vec<String>) {
    let graph = cells
        .iter()
        .map(|cell| (cell.id.as_str(), cell.depends_on.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for cell in cells {
        visit_dependency(
            cell.id.as_str(),
            &graph,
            &mut visiting,
            &mut visited,
            errors,
        );
    }
}

fn visit_dependency<'a>(
    id: &'a str,
    graph: &BTreeMap<&'a str, &'a [String]>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
    errors: &mut Vec<String>,
) {
    if visited.contains(id) {
        return;
    }
    if !visiting.insert(id) {
        errors.push(format!("dependency cycle includes '{id}'"));
        return;
    }
    if let Some(dependencies) = graph.get(id) {
        for dependency in *dependencies {
            if graph.contains_key(dependency.as_str()) {
                visit_dependency(dependency, graph, visiting, visited, errors);
            }
        }
    }
    visiting.remove(id);
    visited.insert(id);
}

fn average(values: impl Iterator<Item = u8>) -> u8 {
    let values = values.map(u64::from).collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    (values.iter().sum::<u64>() / values.len() as u64) as u8
}

fn weighted_average(values: impl Iterator<Item = (u8, u8)>) -> u8 {
    let values = values.collect::<Vec<_>>();
    let total_weight = values
        .iter()
        .map(|(_, weight)| u64::from(*weight))
        .sum::<u64>();
    if total_weight == 0 {
        return 0;
    }
    (values
        .iter()
        .map(|(value, weight)| u64::from(*value) * u64::from(*weight))
        .sum::<u64>()
        / total_weight) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid() {
        let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog should decode");
        catalog
            .validate(env!("CARGO_MANIFEST_DIR"))
            .expect("catalog should validate");
    }

    #[test]
    fn summary_orders_weakest_cells_first() {
        let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog should decode");
        let summary = catalog.summary(20);
        assert!(summary.weakest.len() >= 2);
        assert!(
            summary
                .weakest
                .windows(2)
                .all(|pair| pair[0].score <= pair[1].score)
        );
        assert!(
            summary
                .independently_usable
                .iter()
                .all(|cell| cell.independence != Independence::Internal)
        );
        assert!(
            summary
                .weakest
                .iter()
                .chain(&summary.in_development)
                .chain(&summary.independently_usable)
                .all(|cell| cell.priority != Priority::Deferred)
        );
        assert_eq!(summary.deferred_cell_count, summary.deferred.len());
        assert!(
            summary
                .deferred
                .iter()
                .all(|cell| cell.priority == Priority::Deferred)
        );
    }

    #[test]
    fn validation_rejects_invalid_calibration_and_maturity_bands() {
        let mut catalog =
            StatusCatalog::load(default_catalog_path()).expect("catalog should decode");
        catalog.calibration.model = "equal-weight-v0".to_string();
        catalog.calibration.as_of = "2026-02-30".to_string();
        catalog
            .cells
            .iter_mut()
            .find(|cell| cell.maturity == Maturity::Incubating)
            .expect("catalog should have an incubating cell")
            .completion = 76;
        catalog
            .cells
            .iter_mut()
            .find(|cell| cell.maturity == Maturity::Stabilizing)
            .expect("catalog should have a stabilizing cell")
            .completion = 69;
        let planned = catalog
            .cells
            .iter_mut()
            .find(|cell| cell.maturity == Maturity::Mature)
            .expect("catalog should have a mature cell");
        planned.maturity = Maturity::Planned;
        planned.completion = 26;
        for cell in &mut catalog.cells {
            cell.priority = Priority::Deferred;
        }

        let errors = catalog
            .validate(env!("CARGO_MANIFEST_DIR"))
            .expect_err("invalid calibration must be rejected")
            .join("\n");
        assert!(errors.contains("unsupported calibration model"));
        assert!(errors.contains("calibration.as_of must use YYYY-MM-DD"));
        assert!(errors.contains("at least one non-deferred cell"));
        assert!(errors.contains("planned cell"));
        assert!(errors.contains("cannot report completion above 25"));
        assert!(errors.contains("incubating cell"));
        assert!(errors.contains("cannot report completion above 75"));
        assert!(errors.contains("stabilizing cell"));
        assert!(errors.contains("must report completion of at least 70"));
    }

    #[test]
    fn validation_rejects_false_maturity_and_repository_escape() {
        let mut catalog =
            StatusCatalog::load(default_catalog_path()).expect("catalog should decode");
        let mature = catalog
            .cells
            .iter_mut()
            .find(|cell| cell.maturity == Maturity::Mature)
            .expect("catalog should have a mature cell");
        mature.contract.stability = ContractStability::Draft;
        mature.completion = 101;
        mature.evidence.push(StatusEvidence {
            kind: EvidenceKind::Source,
            path: "../outside".to_string(),
            state: EvidenceState::Present,
        });

        let errors = catalog
            .validate(env!("CARGO_MANIFEST_DIR"))
            .expect_err("invalid maturity must be rejected")
            .join("\n");
        assert!(errors.contains("completion must be between 0 and 100"));
        assert!(errors.contains("must have a stable contract"));
        assert!(errors.contains("evidence must stay inside the repository"));
    }

    #[test]
    fn validation_rejects_dependency_cycles() {
        let mut catalog =
            StatusCatalog::load(default_catalog_path()).expect("catalog should decode");
        let runtime_id = "gewyvern-core/runtime-evidence/evidence-reconstruction";
        let linux_id = "gewyvern-core/linux-ebpf/linux-attach";
        catalog
            .cells
            .iter_mut()
            .find(|cell| cell.id == runtime_id)
            .expect("runtime cell should exist")
            .depends_on
            .push(linux_id.to_string());

        let errors = catalog
            .validate(env!("CARGO_MANIFEST_DIR"))
            .expect_err("dependency cycle must be rejected")
            .join("\n");
        assert!(errors.contains("dependency cycle"));
    }
}
