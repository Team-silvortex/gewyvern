use crate::dsl::{
    DslError, GewyLangContractStamp, GewyLangStage, compile_file, load_file_with_package_context,
    parse_str_unvalidated_with_package, parse_str_with_frontend_unvalidated,
    parse_str_with_frontend_unvalidated_with_package, summarize_frontend_file,
    summarize_frontend_str, summarize_frontend_str_with_package,
};
use crate::flow::ProgramOperation;
use crate::fragment::{
    BindingDiagnostics, EvidenceTier, FragmentRegistry, ModelDiagnostics,
    PayloadOffsetSupportSummary, RegistryError, RuleTier, builtin_registry_ref,
};
use crate::reason::ReasonProfile;
use crate::template::{FragmentParamValue, TemplateBinding};

mod explain;
mod explain_support;
mod frontend;
mod frontend_focus;
mod ir_focus;
mod projection_host;
mod render;
mod render_support;
mod report_types;

use self::explain::*;
use self::explain_support::*;
use self::frontend::*;
use self::frontend_focus::*;
use self::ir_focus::*;
use self::projection_host::GewyvernProjectionHost;
pub use self::render::*;
use self::render_support::*;
pub use self::report_types::*;

pub fn compile_binding_file(path: &str) -> Result<TemplateBinding, DslError> {
    compile_file(path)
}

pub fn compile_binding_report_file(path: &str) -> Result<BindingReport, DslError> {
    let (input, package) = load_file_with_package_context(path)?;
    parse_str_unvalidated_with_package(&input, &package)
        .map(|binding| binding_report(&binding))
        .map_err(|_| DslError::InvalidValue("binding report unavailable".into()))
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
    builtin_registry_ref().binding_diagnostics(binding)
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

pub fn compile_ir_report_file(path: &str) -> Result<IrReport, DslError> {
    compile_envelope_file(path).and_then(|envelope| {
        envelope
            .ir_report
            .ok_or_else(|| DslError::InvalidValue("ir report unavailable".into()))
    })
}

pub fn compile_ir_report_str(input: &str) -> Result<IrReport, DslError> {
    let envelope = compile_envelope_str(input);
    envelope
        .ir_report
        .ok_or_else(|| DslError::InvalidValue("ir report unavailable".into()))
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
    let (envelope, source) = compile_envelope_file_with_source(path)?;
    Ok(explain_report(envelope, Some(source.as_str())))
}

pub fn compile_explain_report_str(input: &str) -> ExplainReport {
    explain_report(compile_envelope_str(input), Some(input))
}

pub fn compile_envelope_file(path: &str) -> Result<CompilerEnvelope, DslError> {
    let (envelope, _) = compile_envelope_file_with_source(path)?;
    Ok(envelope)
}

fn compile_envelope_file_with_source(path: &str) -> Result<(CompilerEnvelope, String), DslError> {
    let (input, package) = load_file_with_package_context(path)?;
    let envelope = match parse_str_with_frontend_unvalidated_with_package(&input, &package) {
        Ok((binding, frontend)) => {
            compile_envelope_from_parts(Ok(binding), Some(frontend_report(frontend)))
        }
        Err(err) => {
            let frontend = summarize_frontend_str_with_package(&input, &package)
                .ok()
                .map(frontend_report);
            compile_envelope_from_parts(Err(err), frontend)
        }
    };
    Ok((envelope, input))
}

pub fn compile_envelope_str(input: &str) -> CompilerEnvelope {
    match parse_str_with_frontend_unvalidated(input) {
        Ok((binding, frontend)) => {
            compile_envelope_from_parts(Ok(binding), Some(frontend_report(frontend)))
        }
        Err(err) => {
            let frontend = summarize_frontend_str(input).ok().map(frontend_report);
            compile_envelope_from_parts(Err(err), frontend)
        }
    }
}

fn compile_envelope_from_parts(
    parsed: Result<TemplateBinding, DslError>,
    frontend: Option<FrontendReport>,
) -> CompilerEnvelope {
    match parsed {
        Ok(binding) => {
            let registry = builtin_registry_ref();
            let diagnostics_result = registry.binding_diagnostics(&binding);
            let validation_result = match registry.validate_binding_params(&binding) {
                Err(err) => Err(err),
                Ok(()) => match diagnostics_result.as_ref() {
                    Ok(diagnostics) => registry.validate_binding_diagnostics(diagnostics),
                    Err(err) => Err((*err).clone()),
                },
            };
            let mut validation = validation_report(
                registry,
                &binding,
                diagnostics_result.as_ref().ok(),
                validation_result.err().as_ref(),
            );
            validation.checks.push("ir_invariants".into());
            let projections = match gewylang_ir::project_compiler_stages_checked(
                &GewyvernProjectionHost,
                &binding,
                diagnostics_result.as_ref(),
            ) {
                Ok(projections) => projections,
                Err(errors) => {
                    validation.ok = false;
                    validation.finding = Some(finding_from_ir_validation_errors(&errors));
                    let parse = ParseStageReport {
                        ok: true,
                        frontend,
                        report: None,
                        finding: None,
                    };
                    let diagnostics = DiagnosticsStageReport {
                        ok: false,
                        report: None,
                        finding: None,
                    };
                    return CompilerEnvelope {
                        binding: None,
                        diagnostics: None,
                        findings: findings_from_stage_reports(&parse, &validation, &diagnostics),
                        stages: CompilerStagesReport {
                            parse,
                            validation,
                            diagnostics,
                        },
                        ir_report: None,
                    };
                }
            };
            let binding_report = projections.binding;
            let ir_report = projections.analysis;
            let diagnostics_stage =
                diagnostics_stage_report(projections.diagnostics.map_err(|err| err.clone()));
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
                ir_report: Some(ir_report),
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
                ir_report: None,
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
