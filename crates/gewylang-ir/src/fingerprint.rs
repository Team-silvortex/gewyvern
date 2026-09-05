use crate::{
    BindingReport, CompilerStageProjections, EvidenceOverrideReport, FragmentParamReport,
    IrModelReport, IrReport, IrRuleReport, ParamValueReport, ReasonProfileReport, WindowReport,
};
use gewylang_contract::{GEWYLANG_LANGUAGE_ID, GEWYLANG_SYNTAX_VERSION, GewyLangStage};
use sha2::{Digest, Sha256};
use std::fmt;

/// Hash algorithm used by stable GewyLang IR fingerprints.
pub const IR_FINGERPRINT_ALGORITHM: &str = "sha256";
/// Canonical field encoding used before hashing a GewyLang IR value.
pub const IR_FINGERPRINT_ENCODING_VERSION: u32 = 1;

/// Stable identity of one complete public IR projection or model value.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IrFingerprint([u8; 32]);

impl IrFingerprint {
    /// Algorithm identifier used by this fingerprint.
    pub const fn algorithm(&self) -> &'static str {
        IR_FINGERPRINT_ALGORITHM
    }

    /// Canonical field-encoding version used before hashing.
    pub const fn encoding_version(&self) -> u32 {
        IR_FINGERPRINT_ENCODING_VERSION
    }

    /// Raw SHA-256 digest bytes.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hexadecimal digest without the algorithm/version prefix.
    pub fn digest_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(self.0.len() * 2);
        for byte in self.0 {
            output.push(HEX[usize::from(byte >> 4)] as char);
            output.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        output
    }
}

impl fmt::Debug for IrFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for IrFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{IR_FINGERPRINT_ALGORITHM}:v{IR_FINGERPRINT_ENCODING_VERSION}:"
        )?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Fingerprints for the executable and analysis projections of one compile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerStageFingerprints {
    pub binding: IrFingerprint,
    pub analysis: IrFingerprint,
}

impl BindingReport {
    /// Fingerprints every ordered Binding IR field with an explicit v1 encoding.
    pub fn fingerprint(&self) -> IrFingerprint {
        let mut encoder = FingerprintEncoder::for_stage(GewyLangStage::BindingIr, "binding");
        encoder.field("template_id");
        encoder.text(&self.template_id);
        encoder.field("fragments");
        encoder.text_list(&self.fragments);
        encoder.field("window");
        encoder.optional(self.window.as_ref(), encode_window);
        encoder.field("reason_profile");
        encoder.optional(self.reason_profile.as_ref(), encode_reason_profile);
        encoder.field("program_model");
        encoder.optional(self.program_model.as_ref(), |encoder, model| {
            encoder.text(&model.id);
            encoder.text(&model.operation);
            encoder.integer(model.rules as u128);
        });
        encoder.field("fragment_params");
        encoder.list(&self.fragment_params, encode_fragment_param);
        encoder.field("evidence_overrides");
        encoder.list(&self.evidence_overrides, encode_evidence_override);
        encoder.finish()
    }
}

impl IrReport {
    /// Fingerprints the complete ordered Analysis IR, including supportability data.
    pub fn fingerprint(&self) -> IrFingerprint {
        let mut encoder = FingerprintEncoder::for_stage(GewyLangStage::AnalysisIr, "analysis");
        encoder.field("template_id");
        encoder.text(&self.template_id);
        encoder.field("program_model");
        encoder.optional(self.program_model.as_ref(), encode_analysis_model);
        encoder.field("reason_model");
        encoder.optional(self.reason_model.as_ref(), encode_analysis_model);
        encoder.finish()
    }
}

impl IrModelReport {
    /// Fingerprints one complete ordered Analysis IR model independently.
    pub fn fingerprint(&self) -> IrFingerprint {
        let mut encoder =
            FingerprintEncoder::for_stage(GewyLangStage::AnalysisIr, "analysis_model");
        encode_analysis_model(&mut encoder, self);
        encoder.finish()
    }
}

impl<E> CompilerStageProjections<E> {
    /// Computes both stable stage identities without consuming projection data.
    pub fn fingerprints(&self) -> CompilerStageFingerprints {
        CompilerStageFingerprints {
            binding: self.binding.fingerprint(),
            analysis: self.analysis.fingerprint(),
        }
    }
}

fn encode_window(encoder: &mut FingerprintEncoder, window: &WindowReport) {
    encoder.text(&window.id);
    encoder.integer(window.duration_ms);
    encoder.integer(window.lateness_ms);
}

fn encode_reason_profile(encoder: &mut FingerprintEncoder, reason: &ReasonProfileReport) {
    match reason {
        ReasonProfileReport::Builtin { id } => {
            encoder.variant("builtin");
            encoder.text(id);
        }
        ReasonProfileReport::Declarative { id, rules } => {
            encoder.variant("declarative");
            encoder.text(id);
            encoder.integer(*rules as u128);
        }
    }
}

fn encode_fragment_param(encoder: &mut FingerprintEncoder, param: &FragmentParamReport) {
    encoder.text(&param.fragment);
    encoder.text(&param.key);
    match &param.value {
        ParamValueReport::Bool(value) => {
            encoder.variant("bool");
            encoder.boolean(*value);
        }
        ParamValueReport::U64(value) => {
            encoder.variant("u64");
            encoder.integer(*value);
        }
        ParamValueReport::String(value) => {
            encoder.variant("string");
            encoder.text(value);
        }
    }
}

fn encode_evidence_override(encoder: &mut FingerprintEncoder, evidence: &EvidenceOverrideReport) {
    encoder.text(&evidence.fact_kind);
    encoder.text(&evidence.tier);
}

fn encode_analysis_model(encoder: &mut FingerprintEncoder, model: &IrModelReport) {
    encoder.text(&model.kind);
    encoder.text(&model.id);
    encoder.optional_text(model.operation.as_deref());
    encoder.list(&model.rules, encode_analysis_rule);
}

fn encode_analysis_rule(encoder: &mut FingerprintEncoder, rule: &IrRuleReport) {
    encoder.integer(rule.rule_index as u128);
    encoder.text(&rule.predicate);
    encoder.optional_text(rule.signal.as_deref());
    encoder.text(&rule.narrative);
    encoder.boolean(rule.dedupe);
    encoder.optional_text(rule.module.as_deref());
    encoder.optional_text(rule.phase.as_deref());
    encoder.optional_text(rule.phase_kind.as_deref());
    encoder.text_list(&rule.required_facts);
    encoder.text_list(&rule.supporting_fragments);
    encoder.text_list(&rule.missing_facts);
    encoder.integer_list(&rule.unsupported_payload_offsets);
    encoder.boolean(rule.supported);
}

struct FingerprintEncoder {
    hasher: Sha256,
}

impl FingerprintEncoder {
    fn for_stage(stage: GewyLangStage, projection: &str) -> Self {
        let mut encoder = Self {
            hasher: Sha256::new(),
        };
        encoder.text("gewylang-ir-fingerprint");
        encoder.integer(IR_FINGERPRINT_ENCODING_VERSION);
        encoder.text(GEWYLANG_LANGUAGE_ID);
        encoder.integer(GEWYLANG_SYNTAX_VERSION);
        encoder.text(stage.id());
        encoder.integer(stage.version());
        encoder.text(projection);
        encoder
    }

    fn finish(self) -> IrFingerprint {
        let digest = self.hasher.finalize();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        IrFingerprint(bytes)
    }

    fn field(&mut self, name: &str) {
        self.text(name);
    }

    fn variant(&mut self, name: &str) {
        self.text(name);
    }

    fn boolean(&mut self, value: bool) {
        self.hasher.update([u8::from(value)]);
    }

    fn integer(&mut self, value: impl Into<u128>) {
        self.hasher.update(value.into().to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.integer(value.len() as u128);
        self.hasher.update(value.as_bytes());
    }

    fn optional<T: ?Sized>(&mut self, value: Option<&T>, encode: impl FnOnce(&mut Self, &T)) {
        self.boolean(value.is_some());
        if let Some(value) = value {
            encode(self, value);
        }
    }

    fn optional_text(&mut self, value: Option<&str>) {
        self.optional(value, |encoder, value| encoder.text(value));
    }

    fn list<T>(&mut self, values: &[T], mut encode: impl FnMut(&mut Self, &T)) {
        self.integer(values.len() as u128);
        for value in values {
            encode(self, value);
        }
    }

    fn text_list(&mut self, values: &[String]) {
        self.list(values, |encoder, value| encoder.text(value));
    }

    fn integer_list<T>(&mut self, values: &[T])
    where
        T: Copy + Into<u128>,
    {
        self.list(values, |encoder, value| encoder.integer(*value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProgramModelReport, ReasonProfileReport};

    fn binding_report() -> BindingReport {
        BindingReport {
            template_id: "dns_probe".into(),
            fragments: vec!["packet".into(), "route".into()],
            window: Some(WindowReport {
                id: "inline".into(),
                duration_ms: 5_000,
                lateness_ms: 200,
            }),
            reason_profile: Some(ReasonProfileReport::Builtin {
                id: "udp_datagram_l1".into(),
            }),
            program_model: Some(ProgramModelReport {
                id: "dns_probe_program".into(),
                operation: "dns_query".into(),
                rules: 1,
            }),
            fragment_params: vec![FragmentParamReport {
                fragment: "packet".into(),
                key: "capture".into(),
                value: ParamValueReport::Bool(true),
            }],
            evidence_overrides: vec![EvidenceOverrideReport {
                fact_kind: "packet".into(),
                tier: "core_requirement".into(),
            }],
        }
    }

    fn analysis_report() -> IrReport {
        IrReport {
            template_id: "dns_probe".into(),
            program_model: Some(IrModelReport {
                kind: "program_model".into(),
                id: "dns_probe_program".into(),
                operation: Some("dns_query".into()),
                rules: vec![IrRuleReport {
                    rule_index: 0,
                    predicate: "udp_datagram_observed".into(),
                    signal: Some("udp_datagram_seen".into()),
                    narrative: "udp_datagram_observed".into(),
                    dedupe: true,
                    module: Some("dns".into()),
                    phase: Some("query".into()),
                    phase_kind: Some("send".into()),
                    required_facts: vec!["packet".into()],
                    supporting_fragments: vec!["packet".into()],
                    missing_facts: Vec::new(),
                    unsupported_payload_offsets: Vec::new(),
                    supported: true,
                }],
            }),
            reason_model: None,
        }
    }

    #[test]
    fn fingerprints_are_stable_and_stage_separated() {
        let binding = binding_report();
        let analysis = analysis_report();

        assert_eq!(binding.fingerprint(), binding.clone().fingerprint());
        assert_eq!(analysis.fingerprint(), analysis.clone().fingerprint());
        assert_ne!(binding.fingerprint(), analysis.fingerprint());
        assert_eq!(binding.fingerprint().algorithm(), "sha256");
        assert_eq!(binding.fingerprint().encoding_version(), 1);
        assert_eq!(binding.fingerprint().digest_hex().len(), 64);
        assert!(binding.fingerprint().to_string().starts_with("sha256:v1:"));
    }

    #[test]
    fn canonical_fingerprint_encoding_matches_the_v1_golden_vector() {
        let actual = format!(
            "binding={}\nanalysis={}",
            binding_report().fingerprint(),
            analysis_report().fingerprint()
        );
        assert_eq!(
            actual,
            concat!(
                "binding=sha256:v1:e08c13b47d4d1cbc7fa7e9ad76a053b55e00c1a43752b47dad8aa12fb01e6a4f\n",
                "analysis=sha256:v1:16006d585fa79e7ea36de039065bf359c8f88ece46c90e27302bdd4ad7a0498c"
            )
        );
    }

    #[test]
    fn analysis_fingerprint_covers_rule_semantics_and_supportability() {
        let report = analysis_report();
        let original = report.fingerprint();

        let mut changed_semantics = report.clone();
        changed_semantics.program_model.as_mut().unwrap().rules[0].dedupe = false;
        assert_ne!(original, changed_semantics.fingerprint());

        let mut changed_support = report;
        let rule = &mut changed_support.program_model.as_mut().unwrap().rules[0];
        rule.supported = false;
        rule.missing_facts.push("socket_state".into());
        assert_ne!(original, changed_support.fingerprint());
    }

    #[test]
    fn length_prefixes_keep_adjacent_values_unambiguous() {
        let mut first = binding_report();
        first.fragments = vec!["ab".into(), "c".into()];
        let mut second = binding_report();
        second.fragments = vec!["a".into(), "bc".into()];

        assert_ne!(first.fingerprint(), second.fingerprint());
    }

    #[test]
    fn model_fingerprint_is_independent_from_report_envelope() {
        let report = analysis_report();
        let model = report.program_model.as_ref().unwrap();
        let model_fingerprint = model.fingerprint();

        let mut renamed_report = report.clone();
        renamed_report.template_id = "renamed_template".into();
        assert_eq!(
            model_fingerprint,
            renamed_report.program_model.as_ref().unwrap().fingerprint()
        );
        assert_ne!(report.fingerprint(), renamed_report.fingerprint());
    }

    #[test]
    fn stage_projection_fingerprints_preserve_each_stage_identity() {
        let projections = CompilerStageProjections::<()> {
            binding: binding_report(),
            diagnostics: Err(()),
            analysis: analysis_report(),
        };
        let expected_binding = projections.binding.fingerprint();
        let expected_analysis = projections.analysis.fingerprint();

        assert_eq!(
            projections.fingerprints(),
            CompilerStageFingerprints {
                binding: expected_binding,
                analysis: expected_analysis,
            }
        );
    }
}
