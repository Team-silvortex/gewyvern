use crate::{
    BindingReport, IR_FINGERPRINT_ALGORITHM, IR_FINGERPRINT_ENCODING_VERSION, IrFingerprint,
    IrReport, IrValidationErrors,
};
use gewylang_contract::{
    GEWYLANG_LANGUAGE_ID, GEWYLANG_SYNTAX_VERSION, GewyLangContractStamp,
    MAX_GEWYLANG_SOURCE_GRAPH_BYTES,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// Stable format identifier for independently exchanged GewyLang IR JSON.
pub const IR_WIRE_FORMAT: &str = "gewylang-ir-json";
/// Envelope and field-layout version for the standalone IR JSON codec.
pub const IR_WIRE_VERSION: u32 = 1;
/// Maximum accepted standalone wire document size.
pub const MAX_IR_WIRE_BYTES: usize = MAX_GEWYLANG_SOURCE_GRAPH_BYTES * 4;
/// Maximum UTF-8 byte length retained from an untrusted wire error value.
pub const MAX_IR_WIRE_ERROR_DETAIL_BYTES: usize = 512;

/// Owned language-stage identity embedded in every standalone wire envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrWireContractStamp {
    pub language: String,
    pub syntax_version: u32,
    pub stage: String,
    pub stage_version: u32,
}

/// Fingerprint metadata embedded in every standalone wire envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrWireFingerprint {
    pub algorithm: String,
    pub encoding_version: u32,
    pub digest: String,
}

/// Strict, versioned envelope shared by Binding IR and Analysis IR payloads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IrWireEnvelope<T> {
    pub wire_format: String,
    pub wire_version: u32,
    pub language_contract: IrWireContractStamp,
    pub fingerprint: IrWireFingerprint,
    pub payload: T,
}

pub type BindingIrWireEnvelope = IrWireEnvelope<BindingReport>;
pub type AnalysisIrWireEnvelope = IrWireEnvelope<IrReport>;

/// Fail-closed errors produced by the standalone IR wire codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrWireError {
    PayloadTooLarge {
        actual: usize,
        maximum: usize,
    },
    JsonEncode(String),
    JsonDecode(String),
    ContractMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    FingerprintMismatch {
        expected: String,
        actual: String,
    },
    InvalidIr(IrValidationErrors),
}

impl fmt::Display for IrWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "IR wire payload is {actual} bytes; maximum is {maximum}"
                )
            }
            Self::JsonEncode(message) => write!(formatter, "cannot encode IR wire JSON: {message}"),
            Self::JsonDecode(message) => write!(formatter, "cannot decode IR wire JSON: {message}"),
            Self::ContractMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "IR wire contract mismatch for {field}: expected '{expected}', found '{actual}'"
            ),
            Self::FingerprintMismatch { expected, actual } => write!(
                formatter,
                "IR wire fingerprint mismatch: expected '{expected}', found '{actual}'"
            ),
            Self::InvalidIr(errors) => write!(formatter, "invalid IR wire payload: {errors}"),
        }
    }
}

impl Error for IrWireError {}

impl From<IrValidationErrors> for IrWireError {
    fn from(value: IrValidationErrors) -> Self {
        Self::InvalidIr(value)
    }
}

/// Encodes validated Binding IR into deterministic compact JSON.
pub fn encode_binding_ir_json(report: &BindingReport) -> Result<Vec<u8>, IrWireError> {
    report.validate_invariants().into_result()?;
    encode_envelope(wire_envelope(
        BindingReport::contract_stamp(),
        report.fingerprint(),
        report,
    ))
}

/// Strictly decodes, fingerprint-verifies, and validates one Binding IR envelope.
pub fn decode_binding_ir_json(input: &[u8]) -> Result<BindingReport, IrWireError> {
    let envelope: BindingIrWireEnvelope = decode_envelope(input)?;
    validate_envelope(&envelope, BindingReport::contract_stamp(), || {
        envelope.payload.fingerprint()
    })?;
    envelope.payload.validate_invariants().into_result()?;
    Ok(envelope.payload)
}

/// Encodes validated Analysis IR into deterministic compact JSON.
pub fn encode_analysis_ir_json(report: &IrReport) -> Result<Vec<u8>, IrWireError> {
    report.validate_invariants().into_result()?;
    encode_envelope(wire_envelope(
        IrReport::contract_stamp(),
        report.fingerprint(),
        report,
    ))
}

/// Strictly decodes, fingerprint-verifies, and validates one Analysis IR envelope.
pub fn decode_analysis_ir_json(input: &[u8]) -> Result<IrReport, IrWireError> {
    let envelope: AnalysisIrWireEnvelope = decode_envelope(input)?;
    validate_envelope(&envelope, IrReport::contract_stamp(), || {
        envelope.payload.fingerprint()
    })?;
    envelope.payload.validate_invariants().into_result()?;
    Ok(envelope.payload)
}

fn wire_envelope<T>(
    stamp: GewyLangContractStamp,
    fingerprint: IrFingerprint,
    payload: T,
) -> IrWireEnvelope<T> {
    IrWireEnvelope {
        wire_format: IR_WIRE_FORMAT.into(),
        wire_version: IR_WIRE_VERSION,
        language_contract: IrWireContractStamp {
            language: stamp.language.into(),
            syntax_version: stamp.syntax_version,
            stage: stamp.stage.id().into(),
            stage_version: stamp.stage_version,
        },
        fingerprint: wire_fingerprint(fingerprint),
        payload,
    }
}

fn wire_fingerprint(fingerprint: IrFingerprint) -> IrWireFingerprint {
    IrWireFingerprint {
        algorithm: IR_FINGERPRINT_ALGORITHM.into(),
        encoding_version: IR_FINGERPRINT_ENCODING_VERSION,
        digest: fingerprint.digest_hex(),
    }
}

fn encode_envelope<T: Serialize>(envelope: IrWireEnvelope<T>) -> Result<Vec<u8>, IrWireError> {
    let output = serde_json::to_vec(&envelope)
        .map_err(|error| IrWireError::JsonEncode(bounded_error_detail(error.to_string())))?;
    ensure_bounded(output.len())?;
    Ok(output)
}

fn decode_envelope<T: DeserializeOwned>(input: &[u8]) -> Result<IrWireEnvelope<T>, IrWireError> {
    ensure_bounded(input.len())?;
    serde_json::from_slice(input)
        .map_err(|error| IrWireError::JsonDecode(bounded_error_detail(error.to_string())))
}

fn ensure_bounded(actual: usize) -> Result<(), IrWireError> {
    if actual > MAX_IR_WIRE_BYTES {
        Err(IrWireError::PayloadTooLarge {
            actual,
            maximum: MAX_IR_WIRE_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_envelope<T>(
    envelope: &IrWireEnvelope<T>,
    expected_contract: GewyLangContractStamp,
    expected_fingerprint: impl FnOnce() -> IrFingerprint,
) -> Result<(), IrWireError> {
    require_contract_value("wire_format", IR_WIRE_FORMAT, &envelope.wire_format)?;
    require_contract_number("wire_version", IR_WIRE_VERSION, envelope.wire_version)?;
    require_contract_value(
        "language_contract.language",
        GEWYLANG_LANGUAGE_ID,
        &envelope.language_contract.language,
    )?;
    require_contract_number(
        "language_contract.syntax_version",
        GEWYLANG_SYNTAX_VERSION,
        envelope.language_contract.syntax_version,
    )?;
    require_contract_value(
        "language_contract.stage",
        expected_contract.stage.id(),
        &envelope.language_contract.stage,
    )?;
    require_contract_number(
        "language_contract.stage_version",
        expected_contract.stage_version,
        envelope.language_contract.stage_version,
    )?;
    require_contract_value(
        "fingerprint.algorithm",
        IR_FINGERPRINT_ALGORITHM,
        &envelope.fingerprint.algorithm,
    )?;
    require_contract_number(
        "fingerprint.encoding_version",
        IR_FINGERPRINT_ENCODING_VERSION,
        envelope.fingerprint.encoding_version,
    )?;
    let expected_digest = expected_fingerprint().digest_hex();
    if envelope.fingerprint.digest != expected_digest {
        return Err(IrWireError::FingerprintMismatch {
            expected: expected_digest,
            actual: bounded_error_detail(envelope.fingerprint.digest.clone()),
        });
    }
    Ok(())
}

fn require_contract_value(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), IrWireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IrWireError::ContractMismatch {
            field,
            expected: bounded_error_detail(expected.into()),
            actual: bounded_error_detail(actual.into()),
        })
    }
}

fn require_contract_number(
    field: &'static str,
    expected: u32,
    actual: u32,
) -> Result<(), IrWireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IrWireError::ContractMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn bounded_error_detail(mut detail: String) -> String {
    if detail.len() <= MAX_IR_WIRE_ERROR_DETAIL_BYTES {
        return detail;
    }
    let mut end = MAX_IR_WIRE_ERROR_DETAIL_BYTES.saturating_sub(3);
    while !detail.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    detail.truncate(end);
    detail.push_str("...");
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvidenceOverrideReport, FragmentParamReport, IrModelReport, IrRuleReport, ParamValueReport,
        ProgramModelReport, ReasonProfileReport, WindowReport,
    };

    fn binding_report() -> BindingReport {
        BindingReport {
            template_id: "dns_probe".into(),
            fragments: vec!["packet".into()],
            window: Some(WindowReport {
                id: "inline".into(),
                duration_ms: 5_000,
                lateness_ms: 200,
            }),
            reason_profile: Some(ReasonProfileReport::Builtin {
                id: "udp_datagram_l1".into(),
            }),
            program_model: Some(ProgramModelReport {
                id: "dns_model".into(),
                operation: "dns_query".into(),
                rules: 1,
            }),
            fragment_params: vec![
                FragmentParamReport {
                    fragment: "packet".into(),
                    key: "enabled".into(),
                    value: ParamValueReport::Bool(true),
                },
                FragmentParamReport {
                    fragment: "packet".into(),
                    key: "limit".into(),
                    value: ParamValueReport::U64(64),
                },
                FragmentParamReport {
                    fragment: "packet".into(),
                    key: "filter".into(),
                    value: ParamValueReport::String("dns".into()),
                },
            ],
            evidence_overrides: vec![EvidenceOverrideReport {
                fact_kind: "packet_meta".into(),
                tier: "core_requirement".into(),
            }],
        }
    }

    fn analysis_report() -> IrReport {
        IrReport {
            template_id: "dns_probe".into(),
            program_model: Some(IrModelReport {
                kind: "program_model".into(),
                id: "dns_model".into(),
                operation: Some("dns_query".into()),
                rules: vec![IrRuleReport {
                    rule_index: 0,
                    predicate: "packet_observed".into(),
                    signal: Some("packet_observed".into()),
                    narrative: "packet observed".into(),
                    dedupe: true,
                    module: Some("dns".into()),
                    phase: Some("query".into()),
                    phase_kind: Some("send".into()),
                    required_facts: vec!["packet_meta".into()],
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
    fn binding_wire_round_trip_is_deterministic() {
        let report = binding_report();
        let first = encode_binding_ir_json(&report).unwrap();
        let second = encode_binding_ir_json(&report).unwrap();

        assert_eq!(first, second);
        assert_eq!(decode_binding_ir_json(&first).unwrap(), report);
        assert!(
            String::from_utf8(first)
                .unwrap()
                .starts_with("{\"wire_format\":\"gewylang-ir-json\",\"wire_version\":1,")
        );
    }

    #[test]
    fn binding_wire_round_trip_preserves_declarative_reason_variant() {
        let mut report = binding_report();
        report.reason_profile = Some(ReasonProfileReport::Declarative {
            id: "dns_reason".into(),
            rules: 2,
        });
        let encoded = encode_binding_ir_json(&report).unwrap();

        assert_eq!(decode_binding_ir_json(&encoded).unwrap(), report);
    }

    #[test]
    fn analysis_wire_round_trip_preserves_every_rule_field() {
        let report = analysis_report();
        let encoded = encode_analysis_ir_json(&report).unwrap();

        assert_eq!(decode_analysis_ir_json(&encoded).unwrap(), report);
    }

    #[test]
    fn decoder_rejects_unknown_fields_at_every_level() {
        let encoded = encode_binding_ir_json(&binding_report()).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        document["payload"]["unexpected"] = serde_json::Value::Bool(true);
        let tampered = serde_json::to_vec(&document).unwrap();

        assert!(matches!(
            decode_binding_ir_json(&tampered),
            Err(IrWireError::JsonDecode(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn decoder_rejects_duplicate_fields() {
        let encoded =
            String::from_utf8(encode_binding_ir_json(&binding_report()).unwrap()).unwrap();
        let duplicate = encoded.replacen(
            "\"wire_version\":1",
            "\"wire_version\":1,\"wire_version\":1",
            1,
        );

        assert!(matches!(
            decode_binding_ir_json(duplicate.as_bytes()),
            Err(IrWireError::JsonDecode(message)) if message.contains("duplicate field")
        ));
    }

    #[test]
    fn decoder_rejects_contract_and_fingerprint_drift() {
        let encoded = encode_analysis_ir_json(&analysis_report()).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        document["wire_version"] = serde_json::Value::from(2);
        let wrong_version = serde_json::to_vec(&document).unwrap();
        assert!(matches!(
            decode_analysis_ir_json(&wrong_version),
            Err(IrWireError::ContractMismatch {
                field: "wire_version",
                ..
            })
        ));

        document["wire_version"] = serde_json::Value::from(1);
        document["payload"]["template_id"] = serde_json::Value::from("tampered");
        let tampered = serde_json::to_vec(&document).unwrap();
        assert!(matches!(
            decode_analysis_ir_json(&tampered),
            Err(IrWireError::FingerprintMismatch { .. })
        ));
    }

    #[test]
    fn decoder_rejects_rehashed_but_malformed_ir() {
        let mut report = analysis_report();
        report.program_model.as_mut().unwrap().rules[0].rule_index = 7;
        let envelope = wire_envelope(IrReport::contract_stamp(), report.fingerprint(), &report);
        let encoded = serde_json::to_vec(&envelope).unwrap();

        assert!(matches!(
            decode_analysis_ir_json(&encoded),
            Err(IrWireError::InvalidIr(errors))
                if errors.first().is_some_and(|violation| {
                    violation.code == crate::IrInvariantCode::RuleIndexMismatch
                })
        ));
    }

    #[test]
    fn decoder_rejects_oversized_input_before_json_parsing() {
        let oversized = vec![b' '; MAX_IR_WIRE_BYTES + 1];

        assert_eq!(
            decode_binding_ir_json(&oversized).unwrap_err(),
            IrWireError::PayloadTooLarge {
                actual: MAX_IR_WIRE_BYTES + 1,
                maximum: MAX_IR_WIRE_BYTES,
            }
        );
    }

    #[test]
    fn encoder_rejects_invalid_ir_instead_of_normalizing_it() {
        let mut report = binding_report();
        report.fragments.push("packet".into());

        assert!(matches!(
            encode_binding_ir_json(&report),
            Err(IrWireError::InvalidIr(errors))
                if errors.first().is_some_and(|violation| {
                    violation.code == crate::IrInvariantCode::DuplicateValue
                })
        ));
    }

    #[test]
    fn wire_stage_identity_cannot_be_cross_decoded() {
        let encoded = encode_binding_ir_json(&binding_report()).unwrap();

        assert!(matches!(
            decode_analysis_ir_json(&encoded),
            Err(IrWireError::JsonDecode(_))
        ));
    }

    #[test]
    fn public_constants_define_the_resource_and_version_contract() {
        assert_eq!(IR_WIRE_FORMAT, "gewylang-ir-json");
        assert_eq!(IR_WIRE_VERSION, 1);
        assert_eq!(MAX_IR_WIRE_BYTES, 16 * 1024 * 1024);
        assert_eq!(MAX_IR_WIRE_ERROR_DETAIL_BYTES, 512);
        assert_eq!(
            gewylang_contract::GewyLangStage::BindingIr.id(),
            "binding_ir"
        );
    }

    #[test]
    fn untrusted_error_values_are_bounded() {
        let encoded = encode_binding_ir_json(&binding_report()).unwrap();
        let mut document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        document["wire_format"] =
            serde_json::Value::String("x".repeat(MAX_IR_WIRE_ERROR_DETAIL_BYTES * 2));
        let malformed = serde_json::to_vec(&document).unwrap();

        let error = decode_binding_ir_json(&malformed).unwrap_err();
        assert!(matches!(
            error,
            IrWireError::ContractMismatch { actual, .. }
                if actual.len() <= MAX_IR_WIRE_ERROR_DETAIL_BYTES
        ));
    }
}
