#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsReport {
    pub template_id: String,
    pub fragments: Vec<String>,
    pub program_model: Option<ModelDiagnosticsReport>,
    pub reason_model: Option<ModelDiagnosticsReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDiagnosticsReport {
    pub model: String,
    pub rules: Vec<RuleDiagnosticsReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDiagnosticsReport {
    pub rule_index: usize,
    pub tier: String,
    pub supported: bool,
    pub required_facts: Vec<String>,
    pub supporting_fragments: Vec<String>,
    pub missing_facts: Vec<String>,
    pub unsupported_payload_offsets: Vec<u16>,
}
