use crate::dsl::{
    DslError, compile_file, parse_file_unvalidated, parse_str_unvalidated, summarize_frontend_file,
    summarize_frontend_str, validate_compiled_binding,
};
use crate::flow::ProgramOperation;
use crate::fragment::{
    BindingDiagnostics, EvidenceTier, ModelDiagnostics, PayloadOffsetSupportSummary, RegistryError,
    RuleTier, builtin_registry,
};
use crate::reason::ReasonProfile;
use crate::template::{FragmentParamValue, TemplateBinding};

mod explain;
mod explain_support;
mod frontend;
mod frontend_focus;
mod render;
mod render_support;
mod report_types;

use self::explain::*;
use self::explain_support::*;
use self::frontend::*;
use self::frontend_focus::*;
pub use self::render::*;
use self::render_support::*;
pub use self::report_types::*;

pub fn compile_binding_file(path: &str) -> Result<TemplateBinding, DslError> {
    compile_file(path)
}

pub fn compile_binding_report_file(path: &str) -> Result<BindingReport, DslError> {
    compile_envelope_file(path).and_then(|envelope| {
        envelope
            .binding
            .ok_or_else(|| DslError::InvalidValue("binding report unavailable".into()))
    })
}

pub fn compile_frontend_report_file(path: &str) -> Result<FrontendReport, DslError> {
    summarize_frontend_file(path).map(frontend_report)
}

pub fn compile_frontend_report_str(input: &str) -> Result<FrontendReport, DslError> {
    summarize_frontend_str(input).map(frontend_report)
}

pub fn collect_binding_diagnostics(
    binding: &TemplateBinding,
) -> Result<BindingDiagnostics, RegistryError> {
    builtin_registry().binding_diagnostics(binding)
}

pub fn compile_diagnostics_report_file(
    path: &str,
) -> Result<DiagnosticsReport, CompileDiagnosticsError> {
    let envelope = compile_envelope_file(path)?;
    envelope.diagnostics.ok_or_else(|| {
        let finding = envelope
            .findings
            .findings
            .first()
            .map(|finding| finding.message.clone())
            .unwrap_or_else(|| "diagnostics report unavailable".into());
        CompileDiagnosticsError::Dsl(DslError::InvalidValue(finding))
    })
}

pub fn compile_stages_report_file(path: &str) -> Result<CompilerStagesReport, CompileStagesError> {
    let envelope = compile_envelope_file(path)?;
    Ok(envelope.stages)
}

pub fn compile_stages_report_str(input: &str) -> CompilerStagesReport {
    compile_envelope_str(input).stages
}

pub fn compile_findings_report_file(path: &str) -> CompilerFindingsReport {
    match compile_envelope_file(path) {
        Ok(envelope) => envelope.findings,
        Err(err) => CompilerFindingsReport {
            findings: vec![finding_from_dsl_error(&err)],
        },
    }
}

pub fn compile_findings_report_str(input: &str) -> CompilerFindingsReport {
    compile_envelope_str(input).findings
}

pub fn compile_explain_report_file(path: &str) -> Result<ExplainReport, DslError> {
    let source = std::fs::read_to_string(path).ok();
    compile_envelope_file(path).map(|envelope| explain_report(envelope, source.as_deref()))
}

pub fn compile_explain_report_str(input: &str) -> ExplainReport {
    explain_report(compile_envelope_str(input), Some(input))
}

pub fn compile_envelope_file(path: &str) -> Result<CompilerEnvelope, DslError> {
    let frontend = summarize_frontend_file(path).ok().map(frontend_report);
    Ok(compile_envelope_from_parse_result(
        parse_file_unvalidated(path),
        frontend,
    ))
}

pub fn compile_envelope_str(input: &str) -> CompilerEnvelope {
    let frontend = summarize_frontend_str(input).ok().map(frontend_report);
    compile_envelope_from_parse_result(parse_str_unvalidated(input), frontend)
}

fn compile_envelope_from_parse_result(
    parsed: Result<TemplateBinding, DslError>,
    frontend: Option<FrontendReport>,
) -> CompilerEnvelope {
    match parsed {
        Ok(binding) => {
            let binding_report = binding_report(&binding);
            let diagnostics_result = collect_binding_diagnostics(&binding);
            let validation_result = validate_compiled_binding(&binding);
            let validation = validation_report(
                &binding,
                diagnostics_result.as_ref().ok(),
                validation_result.err().as_ref(),
            );
            let diagnostics_stage = diagnostics_stage_report(&binding, diagnostics_result);
            let diagnostics = diagnostics_stage.report.clone();
            let parse = ParseStageReport {
                ok: true,
                frontend,
                report: Some(binding_report.clone()),
                finding: None,
            };
            let findings = findings_from_stage_reports(&parse, &validation, &diagnostics_stage);
            let stages = CompilerStagesReport {
                parse,
                validation,
                diagnostics: diagnostics_stage,
            };
            CompilerEnvelope {
                binding: Some(binding_report),
                diagnostics,
                findings,
                stages,
            }
        }
        Err(err) => {
            let parse = ParseStageReport {
                ok: false,
                frontend,
                report: None,
                finding: Some(finding_from_dsl_error(&err)),
            };
            let validation = empty_validation_report();
            let diagnostics = DiagnosticsStageReport {
                ok: false,
                report: None,
                finding: None,
            };
            CompilerEnvelope {
                binding: None,
                diagnostics: None,
                findings: findings_from_stage_reports(&parse, &validation, &diagnostics),
                stages: CompilerStagesReport {
                    parse,
                    validation,
                    diagnostics,
                },
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CompileDiagnosticsError {
    Dsl(DslError),
    Registry(RegistryError),
}

impl From<DslError> for CompileDiagnosticsError {
    fn from(value: DslError) -> Self {
        Self::Dsl(value)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CompileStagesError {
    Dsl(DslError),
}

impl From<DslError> for CompileStagesError {
    fn from(value: DslError) -> Self {
        Self::Dsl(value)
    }
}

impl From<RegistryError> for CompileDiagnosticsError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

#[cfg(test)]
mod tests;
