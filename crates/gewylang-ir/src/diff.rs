use crate::{
    BindingReport, EvidenceOverrideReport, FragmentParamReport, IrFingerprint, IrModelReport,
    IrReport, IrRuleReport, IrValidationErrors, IrWireError, ParamValueReport, ReasonProfileReport,
    WindowReport, decode_analysis_ir_json, decode_binding_ir_json, validate_binding_analysis_ir,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};

/// Version of the typed field comparison and compatibility policy.
pub const IR_DIFF_CONTRACT_VERSION: u32 = 1;
/// Maximum changes retained from one stage comparison.
pub const MAX_IR_DIFF_CHANGES: usize = 256;
/// Maximum UTF-8 bytes retained for one before/after value preview.
pub const MAX_IR_DIFF_VALUE_BYTES: usize = 256;

/// Stable compatibility classes, ordered from least to most disruptive.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrCompatibility {
    Identical,
    AnalysisOnly,
    ExecutionChange,
    Incompatible,
}

impl IrCompatibility {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::AnalysisOnly => "analysis_only",
            Self::ExecutionChange => "execution_change",
            Self::Incompatible => "incompatible",
        }
    }
}

impl fmt::Display for IrCompatibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// IR stage owning one field-level change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrDiffStage {
    Binding,
    Analysis,
}

impl IrDiffStage {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Binding => "binding_ir",
            Self::Analysis => "analysis_ir",
        }
    }
}

impl fmt::Display for IrDiffStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Shape of one field change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrChangeKind {
    Added,
    Removed,
    Modified,
    Reordered,
}

impl IrChangeKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Reordered => "reordered",
        }
    }
}

impl fmt::Display for IrChangeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Semantic impact of one field change.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrChangeImpact {
    Analysis,
    Execution,
    Identity,
    Structure,
}

impl IrChangeImpact {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::Execution => "execution",
            Self::Identity => "identity",
            Self::Structure => "structure",
        }
    }

    const fn compatibility(self) -> IrCompatibility {
        match self {
            Self::Analysis => IrCompatibility::AnalysisOnly,
            Self::Execution => IrCompatibility::ExecutionChange,
            Self::Identity | Self::Structure => IrCompatibility::Incompatible,
        }
    }
}

impl fmt::Display for IrChangeImpact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Bounded human-readable representation of one changed value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrValuePreview {
    pub text: String,
    pub truncated: bool,
}

impl IrValuePreview {
    fn from_text(text: &str) -> Self {
        if text.len() <= MAX_IR_DIFF_VALUE_BYTES {
            return Self {
                text: text.into(),
                truncated: false,
            };
        }
        let mut end = MAX_IR_DIFF_VALUE_BYTES.saturating_sub(3);
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        let mut preview = String::with_capacity(end + 3);
        preview.push_str(&text[..end]);
        preview.push_str("...");
        Self {
            text: preview,
            truncated: true,
        }
    }

    fn unavailable() -> Self {
        Self {
            text: "<unavailable>".into(),
            truncated: false,
        }
    }
}

/// One deterministic field-level IR change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrChange {
    pub stage: IrDiffStage,
    pub path: String,
    pub kind: IrChangeKind,
    pub impact: IrChangeImpact,
    pub before: Option<IrValuePreview>,
    pub after: Option<IrValuePreview>,
}

/// Bounded diff for one independently fingerprinted IR stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrStageDiff {
    pub stage: IrDiffStage,
    pub before_fingerprint: IrFingerprint,
    pub after_fingerprint: IrFingerprint,
    pub compatibility: IrCompatibility,
    pub changes: Vec<IrChange>,
    pub truncated: bool,
}

impl IrStageDiff {
    pub fn is_identical(&self) -> bool {
        self.compatibility == IrCompatibility::Identical
    }
}

/// Coherent Binding/Analysis comparison for two complete compiler snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerIrDiff {
    pub compatibility: IrCompatibility,
    pub binding: IrStageDiff,
    pub analysis: IrStageDiff,
}

/// Side of a comparison containing malformed or cross-stage-incoherent IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrDiffSide {
    Before,
    After,
}

impl IrDiffSide {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

impl fmt::Display for IrDiffSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Fail-closed error returned when a typed comparison receives invalid IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrDiffError {
    pub side: IrDiffSide,
    pub errors: IrValidationErrors,
}

impl fmt::Display for IrDiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid {} IR snapshot: {}",
            self.side, self.errors
        )
    }
}

impl Error for IrDiffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.errors)
    }
}

/// Decode/validation boundary errors for direct standalone-wire comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrWireDiffError {
    Before(IrWireError),
    After(IrWireError),
    Diff(IrDiffError),
}

impl fmt::Display for IrWireDiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Before(error) => write!(formatter, "invalid before IR wire value: {error}"),
            Self::After(error) => write!(formatter, "invalid after IR wire value: {error}"),
            Self::Diff(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for IrWireDiffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Before(error) | Self::After(error) => Some(error),
            Self::Diff(error) => Some(error),
        }
    }
}

/// Compares two structurally valid Binding IR values.
pub fn diff_binding_ir(
    before: &BindingReport,
    after: &BindingReport,
) -> Result<IrStageDiff, IrDiffError> {
    validate_stage(
        IrDiffSide::Before,
        before.validate_invariants().into_result(),
    )?;
    validate_stage(IrDiffSide::After, after.validate_invariants().into_result())?;
    Ok(diff_binding_ir_validated(before, after))
}

/// Compares two structurally valid Analysis IR values.
pub fn diff_analysis_ir(before: &IrReport, after: &IrReport) -> Result<IrStageDiff, IrDiffError> {
    validate_stage(
        IrDiffSide::Before,
        before.validate_invariants().into_result(),
    )?;
    validate_stage(IrDiffSide::After, after.validate_invariants().into_result())?;
    Ok(diff_analysis_ir_validated(before, after))
}

/// Compares two coherent Binding/Analysis snapshots.
pub fn diff_compiler_ir(
    before_binding: &BindingReport,
    before_analysis: &IrReport,
    after_binding: &BindingReport,
    after_analysis: &IrReport,
) -> Result<CompilerIrDiff, IrDiffError> {
    validate_stage(
        IrDiffSide::Before,
        validate_binding_analysis_ir(before_binding, before_analysis).into_result(),
    )?;
    validate_stage(
        IrDiffSide::After,
        validate_binding_analysis_ir(after_binding, after_analysis).into_result(),
    )?;
    let binding = diff_binding_ir_validated(before_binding, after_binding);
    let analysis = diff_analysis_ir_validated(before_analysis, after_analysis);
    Ok(CompilerIrDiff {
        compatibility: binding.compatibility.max(analysis.compatibility),
        binding,
        analysis,
    })
}

/// Strictly decodes and compares two standalone Binding IR wire documents.
pub fn diff_binding_ir_json(before: &[u8], after: &[u8]) -> Result<IrStageDiff, IrWireDiffError> {
    let before = decode_binding_ir_json(before).map_err(IrWireDiffError::Before)?;
    let after = decode_binding_ir_json(after).map_err(IrWireDiffError::After)?;
    diff_binding_ir(&before, &after).map_err(IrWireDiffError::Diff)
}

/// Strictly decodes and compares two standalone Analysis IR wire documents.
pub fn diff_analysis_ir_json(before: &[u8], after: &[u8]) -> Result<IrStageDiff, IrWireDiffError> {
    let before = decode_analysis_ir_json(before).map_err(IrWireDiffError::Before)?;
    let after = decode_analysis_ir_json(after).map_err(IrWireDiffError::After)?;
    diff_analysis_ir(&before, &after).map_err(IrWireDiffError::Diff)
}

fn validate_stage(
    side: IrDiffSide,
    validation: Result<(), IrValidationErrors>,
) -> Result<(), IrDiffError> {
    validation.map_err(|errors| IrDiffError { side, errors })
}

fn diff_binding_ir_validated(before: &BindingReport, after: &BindingReport) -> IrStageDiff {
    let before_fingerprint = before.fingerprint();
    let after_fingerprint = after.fingerprint();
    if before == after {
        return identical_stage_diff(IrDiffStage::Binding, before_fingerprint, after_fingerprint);
    }
    let mut diff = DiffCollector::new(IrDiffStage::Binding);
    compare_text(
        &mut diff,
        "template_id",
        IrChangeImpact::Identity,
        &before.template_id,
        &after.template_id,
    );
    compare_unique_text_list(
        &mut diff,
        "fragments",
        IrChangeImpact::Execution,
        &before.fragments,
        &after.fragments,
    );
    compare_window(&mut diff, before.window.as_ref(), after.window.as_ref());
    compare_reason_profile(
        &mut diff,
        before.reason_profile.as_ref(),
        after.reason_profile.as_ref(),
    );
    compare_program_model(
        &mut diff,
        before.program_model.as_ref(),
        after.program_model.as_ref(),
    );
    compare_fragment_params(&mut diff, &before.fragment_params, &after.fragment_params);
    compare_evidence_overrides(
        &mut diff,
        &before.evidence_overrides,
        &after.evidence_overrides,
    );
    diff.ensure_exhaustive(true);
    diff.finish(before_fingerprint, after_fingerprint)
}

fn diff_analysis_ir_validated(before: &IrReport, after: &IrReport) -> IrStageDiff {
    let before_fingerprint = before.fingerprint();
    let after_fingerprint = after.fingerprint();
    if before == after {
        return identical_stage_diff(IrDiffStage::Analysis, before_fingerprint, after_fingerprint);
    }
    let mut diff = DiffCollector::new(IrDiffStage::Analysis);
    compare_text(
        &mut diff,
        "template_id",
        IrChangeImpact::Identity,
        &before.template_id,
        &after.template_id,
    );
    compare_analysis_model(
        &mut diff,
        "program_model",
        before.program_model.as_ref(),
        after.program_model.as_ref(),
        IrChangeImpact::Execution,
    );
    compare_analysis_model(
        &mut diff,
        "reason_model",
        before.reason_model.as_ref(),
        after.reason_model.as_ref(),
        IrChangeImpact::Analysis,
    );
    diff.ensure_exhaustive(true);
    diff.finish(before_fingerprint, after_fingerprint)
}

fn identical_stage_diff(
    stage: IrDiffStage,
    before_fingerprint: IrFingerprint,
    after_fingerprint: IrFingerprint,
) -> IrStageDiff {
    IrStageDiff {
        stage,
        before_fingerprint,
        after_fingerprint,
        compatibility: IrCompatibility::Identical,
        changes: Vec::new(),
        truncated: false,
    }
}

struct DiffCollector {
    stage: IrDiffStage,
    compatibility: IrCompatibility,
    changes: Vec<IrChange>,
    truncated: bool,
}

impl DiffCollector {
    fn new(stage: IrDiffStage) -> Self {
        Self {
            stage,
            compatibility: IrCompatibility::Identical,
            changes: Vec::new(),
            truncated: false,
        }
    }

    fn record(
        &mut self,
        path: impl Into<String>,
        kind: IrChangeKind,
        impact: IrChangeImpact,
        before: Option<IrValuePreview>,
        after: Option<IrValuePreview>,
    ) {
        self.compatibility = self.compatibility.max(impact.compatibility());
        if self.changes.len() >= MAX_IR_DIFF_CHANGES {
            self.truncated = true;
            return;
        }
        self.changes.push(IrChange {
            stage: self.stage,
            path: path.into(),
            kind,
            impact,
            before,
            after,
        });
    }

    fn modified(
        &mut self,
        path: impl Into<String>,
        impact: IrChangeImpact,
        before: IrValuePreview,
        after: IrValuePreview,
    ) {
        self.record(
            path,
            IrChangeKind::Modified,
            impact,
            Some(before),
            Some(after),
        );
    }

    fn ensure_exhaustive(&mut self, values_differ: bool) {
        if values_differ && self.compatibility == IrCompatibility::Identical {
            self.record(
                "$",
                IrChangeKind::Modified,
                IrChangeImpact::Structure,
                None,
                None,
            );
        }
    }

    fn finish(
        self,
        before_fingerprint: IrFingerprint,
        after_fingerprint: IrFingerprint,
    ) -> IrStageDiff {
        IrStageDiff {
            stage: self.stage,
            before_fingerprint,
            after_fingerprint,
            compatibility: self.compatibility,
            changes: self.changes,
            truncated: self.truncated,
        }
    }
}

fn compare_text(
    diff: &mut DiffCollector,
    path: &str,
    impact: IrChangeImpact,
    before: &str,
    after: &str,
) {
    if before != after {
        diff.modified(
            path,
            impact,
            IrValuePreview::from_text(before),
            IrValuePreview::from_text(after),
        );
    }
}

fn compare_serialized<T: Eq + Serialize>(
    diff: &mut DiffCollector,
    path: &str,
    impact: IrChangeImpact,
    before: &T,
    after: &T,
) {
    if before != after {
        diff.modified(path, impact, preview_json(before), preview_json(after));
    }
}

fn compare_unique_text_list(
    diff: &mut DiffCollector,
    path: &str,
    impact: IrChangeImpact,
    before: &[String],
    after: &[String],
) {
    if before == after {
        return;
    }
    let before_set = before.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let after_set = after.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if before_set == after_set {
        diff.record(
            path,
            IrChangeKind::Reordered,
            impact,
            Some(preview_json(before)),
            Some(preview_json(after)),
        );
        return;
    }
    for (index, value) in before.iter().enumerate() {
        if !after_set.contains(value.as_str()) {
            diff.record(
                format!("{path}[{index}]"),
                IrChangeKind::Removed,
                impact,
                Some(IrValuePreview::from_text(value)),
                None,
            );
        }
    }
    for (index, value) in after.iter().enumerate() {
        if !before_set.contains(value.as_str()) {
            diff.record(
                format!("{path}[{index}]"),
                IrChangeKind::Added,
                impact,
                None,
                Some(IrValuePreview::from_text(value)),
            );
        }
    }
}

fn compare_window(
    diff: &mut DiffCollector,
    before: Option<&WindowReport>,
    after: Option<&WindowReport>,
) {
    match (before, after) {
        (None, None) => {}
        (Some(before), Some(after)) => {
            compare_text(
                diff,
                "window.id",
                IrChangeImpact::Execution,
                &before.id,
                &after.id,
            );
            compare_serialized(
                diff,
                "window.duration_ms",
                IrChangeImpact::Execution,
                &before.duration_ms,
                &after.duration_ms,
            );
            compare_serialized(
                diff,
                "window.lateness_ms",
                IrChangeImpact::Execution,
                &before.lateness_ms,
                &after.lateness_ms,
            );
        }
        (Some(before), None) => diff.record(
            "window",
            IrChangeKind::Removed,
            IrChangeImpact::Structure,
            Some(preview_json(before)),
            None,
        ),
        (None, Some(after)) => diff.record(
            "window",
            IrChangeKind::Added,
            IrChangeImpact::Structure,
            None,
            Some(preview_json(after)),
        ),
    }
}

fn compare_reason_profile(
    diff: &mut DiffCollector,
    before: Option<&ReasonProfileReport>,
    after: Option<&ReasonProfileReport>,
) {
    match (before, after) {
        (None, None) => {}
        (Some(before), Some(after)) => match (before, after) {
            (
                ReasonProfileReport::Builtin { id: before_id },
                ReasonProfileReport::Builtin { id: after_id },
            ) => compare_text(
                diff,
                "reason_profile.id",
                IrChangeImpact::Identity,
                before_id,
                after_id,
            ),
            (
                ReasonProfileReport::Declarative {
                    id: before_id,
                    rules: before_rules,
                },
                ReasonProfileReport::Declarative {
                    id: after_id,
                    rules: after_rules,
                },
            ) => {
                compare_text(
                    diff,
                    "reason_profile.id",
                    IrChangeImpact::Identity,
                    before_id,
                    after_id,
                );
                compare_serialized(
                    diff,
                    "reason_profile.rules",
                    IrChangeImpact::Analysis,
                    before_rules,
                    after_rules,
                );
            }
            _ => diff.modified(
                "reason_profile.kind",
                IrChangeImpact::Structure,
                IrValuePreview::from_text(reason_profile_kind(before)),
                IrValuePreview::from_text(reason_profile_kind(after)),
            ),
        },
        (Some(before), None) => diff.record(
            "reason_profile",
            IrChangeKind::Removed,
            IrChangeImpact::Structure,
            Some(preview_json(before)),
            None,
        ),
        (None, Some(after)) => diff.record(
            "reason_profile",
            IrChangeKind::Added,
            IrChangeImpact::Structure,
            None,
            Some(preview_json(after)),
        ),
    }
}

fn reason_profile_kind(reason: &ReasonProfileReport) -> &'static str {
    match reason {
        ReasonProfileReport::Builtin { .. } => "builtin",
        ReasonProfileReport::Declarative { .. } => "declarative",
    }
}

fn compare_program_model(
    diff: &mut DiffCollector,
    before: Option<&crate::ProgramModelReport>,
    after: Option<&crate::ProgramModelReport>,
) {
    match (before, after) {
        (None, None) => {}
        (Some(before), Some(after)) => {
            compare_text(
                diff,
                "program_model.id",
                IrChangeImpact::Identity,
                &before.id,
                &after.id,
            );
            compare_text(
                diff,
                "program_model.operation",
                IrChangeImpact::Execution,
                &before.operation,
                &after.operation,
            );
            compare_serialized(
                diff,
                "program_model.rules",
                IrChangeImpact::Execution,
                &before.rules,
                &after.rules,
            );
        }
        (Some(before), None) => diff.record(
            "program_model",
            IrChangeKind::Removed,
            IrChangeImpact::Structure,
            Some(preview_json(before)),
            None,
        ),
        (None, Some(after)) => diff.record(
            "program_model",
            IrChangeKind::Added,
            IrChangeImpact::Structure,
            None,
            Some(preview_json(after)),
        ),
    }
}

fn compare_fragment_params(
    diff: &mut DiffCollector,
    before: &[FragmentParamReport],
    after: &[FragmentParamReport],
) {
    let before_map = before
        .iter()
        .enumerate()
        .map(|(index, param)| {
            (
                (param.fragment.as_str(), param.key.as_str()),
                (index, param),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .iter()
        .enumerate()
        .map(|(index, param)| {
            (
                (param.fragment.as_str(), param.key.as_str()),
                (index, param),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let before_keys = before_map.keys().copied().collect::<BTreeSet<_>>();
    let after_keys = after_map.keys().copied().collect::<BTreeSet<_>>();
    if before_keys == after_keys
        && before
            .iter()
            .map(|param| (param.fragment.as_str(), param.key.as_str()))
            .ne(after
                .iter()
                .map(|param| (param.fragment.as_str(), param.key.as_str())))
    {
        diff.record(
            "fragment_params",
            IrChangeKind::Reordered,
            IrChangeImpact::Execution,
            Some(preview_json(before)),
            Some(preview_json(after)),
        );
    }
    for (key, (index, param)) in &before_map {
        if !after_map.contains_key(key) {
            diff.record(
                format!("fragment_params[{index}]"),
                IrChangeKind::Removed,
                IrChangeImpact::Execution,
                Some(preview_json(param)),
                None,
            );
        }
    }
    for (key, (index, param)) in &after_map {
        match before_map.get(key) {
            None => diff.record(
                format!("fragment_params[{index}]"),
                IrChangeKind::Added,
                IrChangeImpact::Execution,
                None,
                Some(preview_json(param)),
            ),
            Some((_, before_param)) => compare_param_value(
                diff,
                &format!("fragment_params[{index}].value"),
                &before_param.value,
                &param.value,
            ),
        }
    }
}

fn compare_param_value(
    diff: &mut DiffCollector,
    path: &str,
    before: &ParamValueReport,
    after: &ParamValueReport,
) {
    compare_serialized(diff, path, IrChangeImpact::Execution, before, after);
}

fn compare_evidence_overrides(
    diff: &mut DiffCollector,
    before: &[EvidenceOverrideReport],
    after: &[EvidenceOverrideReport],
) {
    let before_map = before
        .iter()
        .enumerate()
        .map(|(index, evidence)| (evidence.fact_kind.as_str(), (index, evidence)))
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .iter()
        .enumerate()
        .map(|(index, evidence)| (evidence.fact_kind.as_str(), (index, evidence)))
        .collect::<BTreeMap<_, _>>();
    let before_keys = before_map.keys().copied().collect::<BTreeSet<_>>();
    let after_keys = after_map.keys().copied().collect::<BTreeSet<_>>();
    if before_keys == after_keys
        && before
            .iter()
            .map(|evidence| evidence.fact_kind.as_str())
            .ne(after.iter().map(|evidence| evidence.fact_kind.as_str()))
    {
        diff.record(
            "evidence_overrides",
            IrChangeKind::Reordered,
            IrChangeImpact::Analysis,
            Some(preview_json(before)),
            Some(preview_json(after)),
        );
    }
    for (key, (index, evidence)) in &before_map {
        if !after_map.contains_key(key) {
            diff.record(
                format!("evidence_overrides[{index}]"),
                IrChangeKind::Removed,
                IrChangeImpact::Analysis,
                Some(preview_json(evidence)),
                None,
            );
        }
    }
    for (key, (index, evidence)) in &after_map {
        match before_map.get(key) {
            None => diff.record(
                format!("evidence_overrides[{index}]"),
                IrChangeKind::Added,
                IrChangeImpact::Analysis,
                None,
                Some(preview_json(evidence)),
            ),
            Some((_, before_evidence)) => compare_text(
                diff,
                &format!("evidence_overrides[{index}].tier"),
                IrChangeImpact::Analysis,
                &before_evidence.tier,
                &evidence.tier,
            ),
        }
    }
}

fn compare_analysis_model(
    diff: &mut DiffCollector,
    label: &str,
    before: Option<&IrModelReport>,
    after: Option<&IrModelReport>,
    semantic_impact: IrChangeImpact,
) {
    match (before, after) {
        (None, None) => {}
        (Some(before), Some(after)) => {
            compare_text(
                diff,
                &format!("{label}.kind"),
                IrChangeImpact::Structure,
                &before.kind,
                &after.kind,
            );
            compare_text(
                diff,
                &format!("{label}.id"),
                IrChangeImpact::Identity,
                &before.id,
                &after.id,
            );
            compare_serialized(
                diff,
                &format!("{label}.operation"),
                semantic_impact,
                &before.operation,
                &after.operation,
            );
            compare_analysis_rules(diff, label, &before.rules, &after.rules, semantic_impact);
        }
        (Some(before), None) => diff.record(
            label,
            IrChangeKind::Removed,
            IrChangeImpact::Structure,
            Some(preview_json(before)),
            None,
        ),
        (None, Some(after)) => diff.record(
            label,
            IrChangeKind::Added,
            IrChangeImpact::Structure,
            None,
            Some(preview_json(after)),
        ),
    }
}

fn compare_analysis_rules(
    diff: &mut DiffCollector,
    label: &str,
    before: &[IrRuleReport],
    after: &[IrRuleReport],
    semantic_impact: IrChangeImpact,
) {
    for position in 0..before.len().min(after.len()) {
        compare_analysis_rule(
            diff,
            &format!("{label}.rules[{position}]"),
            &before[position],
            &after[position],
            semantic_impact,
        );
    }
    for (position, rule) in before.iter().enumerate().skip(after.len()) {
        diff.record(
            format!("{label}.rules[{position}]"),
            IrChangeKind::Removed,
            semantic_impact,
            Some(preview_json(rule)),
            None,
        );
    }
    for (position, rule) in after.iter().enumerate().skip(before.len()) {
        diff.record(
            format!("{label}.rules[{position}]"),
            IrChangeKind::Added,
            semantic_impact,
            None,
            Some(preview_json(rule)),
        );
    }
}

fn compare_analysis_rule(
    diff: &mut DiffCollector,
    path: &str,
    before: &IrRuleReport,
    after: &IrRuleReport,
    semantic_impact: IrChangeImpact,
) {
    compare_text(
        diff,
        &format!("{path}.predicate"),
        semantic_impact,
        &before.predicate,
        &after.predicate,
    );
    compare_serialized(
        diff,
        &format!("{path}.signal"),
        semantic_impact,
        &before.signal,
        &after.signal,
    );
    compare_text(
        diff,
        &format!("{path}.narrative"),
        IrChangeImpact::Analysis,
        &before.narrative,
        &after.narrative,
    );
    compare_serialized(
        diff,
        &format!("{path}.dedupe"),
        semantic_impact,
        &before.dedupe,
        &after.dedupe,
    );
    compare_serialized(
        diff,
        &format!("{path}.module"),
        semantic_impact,
        &before.module,
        &after.module,
    );
    compare_serialized(
        diff,
        &format!("{path}.phase"),
        semantic_impact,
        &before.phase,
        &after.phase,
    );
    compare_serialized(
        diff,
        &format!("{path}.phase_kind"),
        semantic_impact,
        &before.phase_kind,
        &after.phase_kind,
    );
    compare_serialized(
        diff,
        &format!("{path}.required_facts"),
        IrChangeImpact::Analysis,
        &before.required_facts,
        &after.required_facts,
    );
    compare_serialized(
        diff,
        &format!("{path}.supporting_fragments"),
        IrChangeImpact::Analysis,
        &before.supporting_fragments,
        &after.supporting_fragments,
    );
    compare_serialized(
        diff,
        &format!("{path}.missing_facts"),
        IrChangeImpact::Analysis,
        &before.missing_facts,
        &after.missing_facts,
    );
    compare_serialized(
        diff,
        &format!("{path}.unsupported_payload_offsets"),
        IrChangeImpact::Analysis,
        &before.unsupported_payload_offsets,
        &after.unsupported_payload_offsets,
    );
    compare_serialized(
        diff,
        &format!("{path}.supported"),
        IrChangeImpact::Analysis,
        &before.supported,
        &after.supported,
    );
}

fn preview_json<T: ?Sized + Serialize>(value: &T) -> IrValuePreview {
    let mut writer = PreviewWriter::default();
    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => writer.finish(false),
        Err(_) if writer.truncated => writer.finish(true),
        Err(_) => IrValuePreview::unavailable(),
    }
}

#[derive(Default)]
struct PreviewWriter {
    bytes: Vec<u8>,
    truncated: bool,
}

impl PreviewWriter {
    fn finish(mut self, truncated: bool) -> IrValuePreview {
        let valid_bytes =
            std::str::from_utf8(&self.bytes).map_or_else(|error| error.valid_up_to(), str::len);
        self.bytes.truncate(valid_bytes);
        let mut text = String::from_utf8(self.bytes).unwrap_or_default();
        if !truncated {
            return IrValuePreview {
                text,
                truncated: false,
            };
        }
        let mut end = text.len().min(MAX_IR_DIFF_VALUE_BYTES.saturating_sub(3));
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        text.truncate(end);
        text.push_str("...");
        IrValuePreview {
            text,
            truncated: true,
        }
    }
}

impl Write for PreviewWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }
        let remaining = MAX_IR_DIFF_VALUE_BYTES.saturating_sub(self.bytes.len());
        if remaining == 0 {
            self.truncated = true;
            return Err(io::Error::other("IR diff preview limit reached"));
        }
        let written = remaining.min(input.len());
        self.bytes.extend_from_slice(&input[..written]);
        if written < input.len() {
            self.truncated = true;
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvidenceOverrideReport, FragmentParamReport, ProgramModelReport, encode_analysis_ir_json,
        encode_binding_ir_json,
    };

    fn binding_report() -> BindingReport {
        BindingReport {
            template_id: "dns_probe".into(),
            fragments: vec!["packet".into(), "socket".into()],
            window: Some(WindowReport {
                id: "inline".into(),
                duration_ms: 5_000,
                lateness_ms: 200,
            }),
            reason_profile: Some(ReasonProfileReport::Declarative {
                id: "dns_reason".into(),
                rules: 1,
            }),
            program_model: Some(ProgramModelReport {
                id: "dns_program".into(),
                operation: "dns_query".into(),
                rules: 1,
            }),
            fragment_params: vec![FragmentParamReport {
                fragment: "packet".into(),
                key: "capture".into(),
                value: ParamValueReport::Bool(true),
            }],
            evidence_overrides: vec![EvidenceOverrideReport {
                fact_kind: "packet_meta".into(),
                tier: "core_requirement".into(),
            }],
        }
    }

    fn rule(predicate: &str, signal: &str, narrative: &str) -> IrRuleReport {
        IrRuleReport {
            rule_index: 0,
            predicate: predicate.into(),
            signal: Some(signal.into()),
            narrative: narrative.into(),
            dedupe: true,
            module: Some("dns".into()),
            phase: Some("query".into()),
            phase_kind: Some("send".into()),
            required_facts: vec!["packet_meta".into()],
            supporting_fragments: vec!["packet".into()],
            missing_facts: Vec::new(),
            unsupported_payload_offsets: Vec::new(),
            supported: true,
        }
    }

    fn analysis_report() -> IrReport {
        IrReport {
            template_id: "dns_probe".into(),
            program_model: Some(IrModelReport {
                kind: "program_model".into(),
                id: "dns_program".into(),
                operation: Some("dns_query".into()),
                rules: vec![rule("packet_observed", "query_sent", "query observed")],
            }),
            reason_model: Some(IrModelReport {
                kind: "declarative_reason_model".into(),
                id: "dns_reason".into(),
                operation: None,
                rules: vec![rule("response_observed", "reply_seen", "reply observed")],
            }),
        }
    }

    #[test]
    fn diff_contract_ids_are_stable() {
        assert_eq!(IR_DIFF_CONTRACT_VERSION, 1);
        assert_eq!(IrCompatibility::Identical.id(), "identical");
        assert_eq!(IrCompatibility::AnalysisOnly.id(), "analysis_only");
        assert_eq!(IrCompatibility::ExecutionChange.id(), "execution_change");
        assert_eq!(IrCompatibility::Incompatible.id(), "incompatible");
        assert_eq!(IrDiffStage::Binding.id(), "binding_ir");
        assert_eq!(IrDiffStage::Analysis.id(), "analysis_ir");
        assert_eq!(IrChangeKind::Reordered.id(), "reordered");
        assert_eq!(IrChangeImpact::Identity.id(), "identity");
    }

    #[test]
    fn identical_compiler_snapshots_have_no_changes() {
        let binding = binding_report();
        let analysis = analysis_report();
        let diff = diff_compiler_ir(&binding, &analysis, &binding, &analysis).unwrap();

        assert_eq!(diff.compatibility, IrCompatibility::Identical);
        assert!(diff.binding.is_identical());
        assert!(diff.analysis.is_identical());
        assert!(diff.binding.changes.is_empty());
        assert!(diff.analysis.changes.is_empty());
        assert_eq!(
            diff.binding.before_fingerprint,
            diff.binding.after_fingerprint
        );
    }

    #[test]
    fn narrative_and_reason_rule_changes_are_analysis_only() {
        let binding = binding_report();
        let before = analysis_report();
        let mut after = before.clone();
        after.program_model.as_mut().unwrap().rules[0].narrative = "new narrative".into();
        after.reason_model.as_mut().unwrap().rules[0].predicate = "timeout_observed".into();

        let diff = diff_compiler_ir(&binding, &before, &binding, &after).unwrap();

        assert_eq!(diff.compatibility, IrCompatibility::AnalysisOnly);
        assert_eq!(diff.binding.compatibility, IrCompatibility::Identical);
        assert_eq!(diff.analysis.changes.len(), 2);
        assert!(diff.analysis.changes.iter().all(|change| {
            change.stage == IrDiffStage::Analysis && change.impact == IrChangeImpact::Analysis
        }));
    }

    #[test]
    fn executable_changes_require_execution_replacement() {
        let before_binding = binding_report();
        let before_analysis = analysis_report();
        let mut after_binding = before_binding.clone();
        let mut after_analysis = before_analysis.clone();
        after_binding.program_model.as_mut().unwrap().operation = "dns_response".into();
        after_analysis.program_model.as_mut().unwrap().operation = Some("dns_response".into());
        after_analysis.program_model.as_mut().unwrap().rules[0].predicate =
            "response_observed".into();

        let diff = diff_compiler_ir(
            &before_binding,
            &before_analysis,
            &after_binding,
            &after_analysis,
        )
        .unwrap();

        assert_eq!(diff.compatibility, IrCompatibility::ExecutionChange);
        assert_eq!(diff.binding.changes[0].path, "program_model.operation");
        assert!(diff.analysis.changes.iter().any(|change| {
            change.path == "program_model.rules[0].predicate"
                && change.impact == IrChangeImpact::Execution
        }));
    }

    #[test]
    fn identity_changes_are_incompatible_even_when_each_pair_is_coherent() {
        let before_binding = binding_report();
        let before_analysis = analysis_report();
        let mut after_binding = before_binding.clone();
        let mut after_analysis = before_analysis.clone();
        after_binding.template_id = "http_probe".into();
        after_analysis.template_id = "http_probe".into();

        let diff = diff_compiler_ir(
            &before_binding,
            &before_analysis,
            &after_binding,
            &after_analysis,
        )
        .unwrap();

        assert_eq!(diff.compatibility, IrCompatibility::Incompatible);
        assert_eq!(diff.binding.changes[0].impact, IrChangeImpact::Identity);
        assert_eq!(diff.analysis.changes[0].impact, IrChangeImpact::Identity);
    }

    #[test]
    fn optional_top_level_shape_changes_are_incompatible() {
        let before = binding_report();
        let mut after = before.clone();
        after.window = None;

        let diff = diff_binding_ir(&before, &after).unwrap();

        assert_eq!(diff.compatibility, IrCompatibility::Incompatible);
        assert_eq!(diff.changes[0].path, "window");
        assert_eq!(diff.changes[0].kind, IrChangeKind::Removed);
        assert_eq!(diff.changes[0].impact, IrChangeImpact::Structure);
    }

    #[test]
    fn ordered_set_and_evidence_changes_keep_their_distinct_impact() {
        let before = binding_report();
        let mut reordered = before.clone();
        reordered.fragments.reverse();
        let reorder_diff = diff_binding_ir(&before, &reordered).unwrap();
        assert_eq!(reorder_diff.compatibility, IrCompatibility::ExecutionChange);
        assert_eq!(reorder_diff.changes[0].kind, IrChangeKind::Reordered);

        let mut evidence = before.clone();
        evidence.evidence_overrides[0].tier = "optional_enhancement".into();
        let evidence_diff = diff_binding_ir(&before, &evidence).unwrap();
        assert_eq!(evidence_diff.compatibility, IrCompatibility::AnalysisOnly);
        assert_eq!(evidence_diff.changes[0].path, "evidence_overrides[0].tier");
    }

    #[test]
    fn invalid_and_cross_stage_incoherent_inputs_fail_closed() {
        let mut invalid = binding_report();
        invalid.template_id = " ".into();
        let error = diff_binding_ir(&invalid, &binding_report()).unwrap_err();
        assert_eq!(error.side, IrDiffSide::Before);
        assert_eq!(
            error.errors.first().unwrap().code,
            crate::IrInvariantCode::EmptyValue
        );

        let binding = binding_report();
        let mut drifted = analysis_report();
        drifted.template_id = "other".into();
        let error = diff_compiler_ir(&binding, &drifted, &binding_report(), &analysis_report())
            .unwrap_err();
        assert_eq!(error.side, IrDiffSide::Before);
        assert_eq!(
            error.errors.first().unwrap().code,
            crate::IrInvariantCode::StageIdentityMismatch
        );
    }

    #[test]
    fn direct_wire_diff_reverifies_fingerprints_before_comparison() {
        let before = binding_report();
        let mut after = before.clone();
        after.evidence_overrides[0].tier = "optional_enhancement".into();
        let before_wire = encode_binding_ir_json(&before).unwrap();
        let after_wire = encode_binding_ir_json(&after).unwrap();

        let diff = diff_binding_ir_json(&before_wire, &after_wire).unwrap();
        assert_eq!(diff.compatibility, IrCompatibility::AnalysisOnly);

        let mut tampered = encode_analysis_ir_json(&analysis_report()).unwrap();
        let location = tampered
            .windows("dns_probe".len())
            .position(|window| window == b"dns_probe")
            .unwrap();
        tampered[location] = b'x';
        let valid = encode_analysis_ir_json(&analysis_report()).unwrap();
        let error = diff_analysis_ir_json(&valid, &tampered).unwrap_err();
        assert!(matches!(
            error,
            IrWireDiffError::After(IrWireError::FingerprintMismatch { .. })
        ));
    }

    #[test]
    fn change_reports_and_multibyte_previews_are_bounded() {
        let mut before = binding_report();
        let mut after = binding_report();
        before.fragments = (0..300).map(|index| format!("before_{index}")).collect();
        after.fragments = (0..300).map(|index| format!("after_{index}")).collect();
        before.fragment_params.clear();
        after.fragment_params.clear();

        let diff = diff_binding_ir(&before, &after).unwrap();
        assert_eq!(diff.changes.len(), MAX_IR_DIFF_CHANGES);
        assert!(diff.truncated);
        assert_eq!(diff.compatibility, IrCompatibility::ExecutionChange);

        let before = binding_report();
        let mut after = before.clone();
        after.program_model.as_mut().unwrap().operation = "界".repeat(200);
        let diff = diff_binding_ir(&before, &after).unwrap();
        let preview = diff.changes[0].after.as_ref().unwrap();
        assert!(preview.truncated);
        assert!(preview.text.is_char_boundary(preview.text.len()));
        assert!(preview.text.len() <= MAX_IR_DIFF_VALUE_BYTES);
        assert!(preview.text.ends_with("..."));

        let serialized = preview_json(&vec!["界".repeat(200), "tail".into()]);
        assert!(serialized.truncated);
        assert!(serialized.text.is_char_boundary(serialized.text.len()));
        assert!(serialized.text.len() <= MAX_IR_DIFF_VALUE_BYTES);
        assert!(serialized.text.ends_with("..."));
    }

    #[test]
    fn omitted_late_changes_still_raise_the_complete_compatibility_class() {
        let mut before = binding_report();
        let mut after = binding_report();
        before.fragments = (0..300).map(|index| format!("before_{index}")).collect();
        after.fragments = (0..300).map(|index| format!("after_{index}")).collect();
        before.fragment_params.clear();
        after.fragment_params.clear();
        after.program_model.as_mut().unwrap().id = "replacement_program".into();

        let diff = diff_binding_ir(&before, &after).unwrap();

        assert_eq!(diff.changes.len(), MAX_IR_DIFF_CHANGES);
        assert!(diff.truncated);
        assert_eq!(diff.compatibility, IrCompatibility::Incompatible);
        assert!(
            diff.changes
                .iter()
                .all(|change| change.impact == IrChangeImpact::Execution)
        );
    }
}
