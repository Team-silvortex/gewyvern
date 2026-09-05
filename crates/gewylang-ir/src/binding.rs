use gewylang_contract::{GewyLangContractStamp, GewyLangStage};
use serde::{Deserialize, Serialize};

/// Stable projection of one materialized GewyLang binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BindingReport {
    pub template_id: String,
    pub fragments: Vec<String>,
    pub window: Option<WindowReport>,
    pub reason_profile: Option<ReasonProfileReport>,
    pub program_model: Option<ProgramModelReport>,
    pub fragment_params: Vec<FragmentParamReport>,
    pub evidence_overrides: Vec<EvidenceOverrideReport>,
}

impl BindingReport {
    /// Contract identity carried by serialized forms of this value.
    pub const fn contract_stamp() -> GewyLangContractStamp {
        GewyLangContractStamp::for_stage(GewyLangStage::BindingIr)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindowReport {
    pub id: String,
    pub duration_ms: u64,
    pub lateness_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum ReasonProfileReport {
    Builtin { id: String },
    Declarative { id: String, rules: usize },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramModelReport {
    pub id: String,
    pub operation: String,
    pub rules: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FragmentParamReport {
    pub fragment: String,
    pub key: String,
    pub value: ParamValueReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    content = "value",
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind"
)]
pub enum ParamValueReport {
    Bool(bool),
    U64(u64),
    String(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceOverrideReport {
    pub fact_kind: String,
    pub tier: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_report_exposes_the_binding_ir_contract() {
        let stamp = BindingReport::contract_stamp();
        assert_eq!(stamp.language, "gewylang");
        assert_eq!(stamp.stage, GewyLangStage::BindingIr);
        assert_eq!(stamp.stage_version, 1);
    }
}
