use gewylang_contract::{GewyLangContractStamp, GewyLangStage};
use serde::{Deserialize, Serialize};

/// Diagnostics-enriched, product-independent Analysis IR projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrReport {
    pub template_id: String,
    pub program_model: Option<IrModelReport>,
    pub reason_model: Option<IrModelReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrModelReport {
    pub kind: String,
    pub id: String,
    pub operation: Option<String>,
    pub rules: Vec<IrRuleReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrRuleReport {
    pub rule_index: usize,
    pub predicate: String,
    pub signal: Option<String>,
    pub narrative: String,
    pub dedupe: bool,
    pub module: Option<String>,
    pub phase: Option<String>,
    pub phase_kind: Option<String>,
    pub required_facts: Vec<String>,
    pub supporting_fragments: Vec<String>,
    pub missing_facts: Vec<String>,
    pub unsupported_payload_offsets: Vec<u16>,
    pub supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrRuleSupportShape<'a> {
    pub required_facts: &'a [String],
    pub supporting_fragments: &'a [String],
    pub missing_facts: &'a [String],
    pub unsupported_payload_offsets: &'a [u16],
    pub supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrLoweringDelta {
    pub frontend_function_count: usize,
    pub frontend_include_source_count: usize,
    pub frontend_use_edge_count: usize,
    pub frontend_graph_node_count: usize,
    pub frontend_graph_edge_count: usize,
    pub lowered_program_rule_count: usize,
    pub lowered_reason_rule_count: usize,
    pub lowered_supported_rule_count: usize,
    pub lowered_unsupported_rule_count: usize,
    pub lowered_modules: Vec<String>,
    pub lowered_phases: Vec<String>,
    pub lowered_phase_kinds: Vec<String>,
    pub lowered_models: Vec<IrModelShapeSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrModelShapeSummary {
    pub label: String,
    pub id: String,
    pub kind: String,
    pub rule_count: usize,
    pub supported_rule_count: usize,
    pub unsupported_rule_count: usize,
    pub modules: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrModelCompareSummary {
    pub program_rule_count: usize,
    pub reason_rule_count: usize,
    pub rule_count_delta: isize,
    pub program_supported_rule_count: usize,
    pub reason_supported_rule_count: usize,
    pub supported_rule_count_delta: isize,
    pub shared_modules: Vec<String>,
    pub program_only_modules: Vec<String>,
    pub reason_only_modules: Vec<String>,
    pub shared_phases: Vec<String>,
    pub program_only_phases: Vec<String>,
    pub reason_only_phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistorySnapshot {
    pub template_id: String,
    pub operation: Option<String>,
    pub program_model: Option<IrHistoryModelSnapshot>,
    pub reason_model: Option<IrHistoryModelSnapshot>,
    pub model_compare: Option<IrHistoryCompareSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistoryModelSnapshot {
    pub id: String,
    pub kind: String,
    pub rule_count: usize,
    pub supported_rule_count: usize,
    pub unsupported_rule_count: usize,
    pub modules: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistoryCompareSnapshot {
    pub rule_count_delta: isize,
    pub supported_rule_count_delta: isize,
    pub shared_modules: Vec<String>,
    pub program_only_modules: Vec<String>,
    pub reason_only_modules: Vec<String>,
    pub shared_phases: Vec<String>,
    pub program_only_phases: Vec<String>,
    pub reason_only_phases: Vec<String>,
}

impl IrReport {
    /// Contract identity carried by serialized forms of this value.
    pub const fn contract_stamp() -> GewyLangContractStamp {
        GewyLangContractStamp::for_stage(GewyLangStage::AnalysisIr)
    }

    pub fn model_entries(&self) -> Vec<(&'static str, &IrModelReport)> {
        let mut entries = Vec::with_capacity(2);
        if let Some(model) = self.program_model.as_ref() {
            entries.push(("program_model", model));
        }
        if let Some(model) = self.reason_model.as_ref() {
            entries.push(("reason_model", model));
        }
        entries
    }

    pub fn compare_models(&self) -> Option<IrModelCompareSummary> {
        let program = self.program_model.as_ref()?;
        let reason = self.reason_model.as_ref()?;
        let program_modules = program.modules();
        let reason_modules = reason.modules();
        let program_phases = program.phases();
        let reason_phases = reason.phases();
        Some(IrModelCompareSummary {
            program_rule_count: program.rules.len(),
            reason_rule_count: reason.rules.len(),
            rule_count_delta: program.rules.len() as isize - reason.rules.len() as isize,
            program_supported_rule_count: program.supported_rule_count(),
            reason_supported_rule_count: reason.supported_rule_count(),
            supported_rule_count_delta: program.supported_rule_count() as isize
                - reason.supported_rule_count() as isize,
            shared_modules: shared_sorted_strings(&program_modules, &reason_modules),
            program_only_modules: difference_sorted_strings(&program_modules, &reason_modules),
            reason_only_modules: difference_sorted_strings(&reason_modules, &program_modules),
            shared_phases: shared_sorted_strings(&program_phases, &reason_phases),
            program_only_phases: difference_sorted_strings(&program_phases, &reason_phases),
            reason_only_phases: difference_sorted_strings(&reason_phases, &program_phases),
        })
    }

    pub fn history_snapshot(&self) -> IrHistorySnapshot {
        IrHistorySnapshot {
            template_id: self.template_id.clone(),
            operation: self
                .program_model
                .as_ref()
                .and_then(|model| model.operation.clone()),
            program_model: self
                .program_model
                .as_ref()
                .map(IrModelReport::history_snapshot),
            reason_model: self
                .reason_model
                .as_ref()
                .map(IrModelReport::history_snapshot),
            model_compare: self
                .compare_models()
                .map(IrModelCompareSummary::history_snapshot),
        }
    }
}

impl IrModelReport {
    pub fn supported_rule_count(&self) -> usize {
        self.rules.iter().filter(|rule| rule.supported).count()
    }

    pub fn unsupported_rule_count(&self) -> usize {
        self.rules.len().saturating_sub(self.supported_rule_count())
    }

    pub fn modules(&self) -> Vec<String> {
        unique_sorted_strings(
            self.rules
                .iter()
                .filter_map(|rule| rule.module_name().map(str::to_string))
                .collect::<Vec<_>>(),
        )
    }

    pub fn phases(&self) -> Vec<String> {
        unique_sorted_strings(
            self.rules
                .iter()
                .filter_map(|rule| rule.phase_name().map(str::to_string))
                .collect::<Vec<_>>(),
        )
    }

    pub fn history_snapshot(&self) -> IrHistoryModelSnapshot {
        IrHistoryModelSnapshot {
            id: self.id.clone(),
            kind: self.kind.clone(),
            rule_count: self.rules.len(),
            supported_rule_count: self.supported_rule_count(),
            unsupported_rule_count: self.unsupported_rule_count(),
            modules: self.modules(),
            phases: self.phases(),
        }
    }
}

impl IrModelCompareSummary {
    pub fn history_snapshot(self) -> IrHistoryCompareSnapshot {
        IrHistoryCompareSnapshot {
            rule_count_delta: self.rule_count_delta,
            supported_rule_count_delta: self.supported_rule_count_delta,
            shared_modules: self.shared_modules,
            program_only_modules: self.program_only_modules,
            reason_only_modules: self.reason_only_modules,
            shared_phases: self.shared_phases,
            program_only_phases: self.program_only_phases,
            reason_only_phases: self.reason_only_phases,
        }
    }
}

impl IrRuleReport {
    pub fn module_name(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub fn phase_name(&self) -> Option<&str> {
        self.phase.as_deref()
    }

    pub fn signal_name(&self) -> Option<&str> {
        self.signal.as_deref()
    }

    pub fn phase_kind_name(&self) -> Option<&str> {
        self.phase_kind.as_deref()
    }

    pub fn has_unsupported_payload_offsets(&self) -> bool {
        !self.unsupported_payload_offsets.is_empty()
    }

    pub fn support_shape(&self) -> IrRuleSupportShape<'_> {
        IrRuleSupportShape {
            required_facts: &self.required_facts,
            supporting_fragments: &self.supporting_fragments,
            missing_facts: &self.missing_facts,
            unsupported_payload_offsets: &self.unsupported_payload_offsets,
            supported: self.supported,
        }
    }
}

fn unique_sorted_strings(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items.dedup();
    items
}

fn shared_sorted_strings(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|item| right.contains(item))
        .cloned()
        .collect::<Vec<_>>()
}

fn difference_sorted_strings(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|item| !right.contains(item))
        .cloned()
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(module: &str, phase: &str, supported: bool) -> IrRuleReport {
        IrRuleReport {
            rule_index: 0,
            predicate: "process_bound".into(),
            signal: Some("process_bound".into()),
            narrative: "process_bound".into(),
            dedupe: true,
            module: Some(module.into()),
            phase: Some(phase.into()),
            phase_kind: Some("bind".into()),
            required_facts: vec![],
            supporting_fragments: vec![],
            missing_facts: vec![],
            unsupported_payload_offsets: vec![],
            supported,
        }
    }

    #[test]
    fn analysis_report_exposes_the_analysis_ir_contract() {
        let stamp = IrReport::contract_stamp();
        assert_eq!(stamp.language, "gewylang");
        assert_eq!(stamp.stage, GewyLangStage::AnalysisIr);
        assert_eq!(stamp.stage_version, 1);
    }

    #[test]
    fn model_comparison_is_deterministic_and_product_independent() {
        let report = IrReport {
            template_id: "standalone".into(),
            program_model: Some(IrModelReport {
                kind: "program".into(),
                id: "program".into(),
                operation: Some("connect".into()),
                rules: vec![rule("shared", "bind", true), rule("program", "send", false)],
            }),
            reason_model: Some(IrModelReport {
                kind: "reason".into(),
                id: "reason".into(),
                operation: None,
                rules: vec![rule("shared", "bind", true)],
            }),
        };

        let comparison = report.compare_models().unwrap();
        assert_eq!(comparison.rule_count_delta, 1);
        assert_eq!(comparison.shared_modules, ["shared"]);
        assert_eq!(comparison.program_only_modules, ["program"]);
        assert!(comparison.reason_only_modules.is_empty());

        let snapshot = report.history_snapshot();
        assert_eq!(snapshot.operation.as_deref(), Some("connect"));
        assert_eq!(snapshot.program_model.unwrap().unsupported_rule_count, 1);
    }
}
