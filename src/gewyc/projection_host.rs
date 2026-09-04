use super::ir_focus::ir_report_from_binding;
use super::render_support::{
    evidence_tier_text, fragment_param_report, model_diagnostics_report, program_operation_text,
    reason_profile_report,
};
use super::{
    BindingReport, DiagnosticsReport, EvidenceOverrideReport, FragmentParamReport,
    ProgramModelReport, WindowReport,
};
use crate::fragment::BindingDiagnostics;
use crate::template::TemplateBinding;
use gewylang_ir::{CompilerProjectionHost, IrReport};

pub(super) struct GewyvernProjectionHost;

impl CompilerProjectionHost for GewyvernProjectionHost {
    type Binding = TemplateBinding;
    type Diagnostics = BindingDiagnostics;

    fn project_binding(&self, binding: &Self::Binding) -> BindingReport {
        BindingReport {
            template_id: binding.template.id.to_string(),
            fragments: binding.template.fragment_set.to_vec(),
            window: binding
                .template
                .window_profile
                .as_ref()
                .map(|window| WindowReport {
                    id: window.id.to_string(),
                    duration_ms: window.duration_ms,
                    lateness_ms: window.lateness_ms,
                }),
            reason_profile: binding
                .template
                .reason_profile
                .as_ref()
                .map(reason_profile_report),
            program_model: binding.template.program_model.as_ref().map(|model| {
                ProgramModelReport {
                    id: model.id.to_string(),
                    operation: program_operation_text(&model.operation).to_string(),
                    rules: model.rules.len(),
                }
            }),
            fragment_params: binding
                .fragment_params
                .iter()
                .flat_map(|(fragment, params)| {
                    params.iter().map(|(key, value)| FragmentParamReport {
                        fragment: fragment.clone(),
                        key: key.clone(),
                        value: fragment_param_report(value),
                    })
                })
                .collect(),
            evidence_overrides: binding
                .evidence_overrides
                .iter()
                .map(|(fact_kind, tier)| EvidenceOverrideReport {
                    fact_kind: fact_kind.to_string(),
                    tier: evidence_tier_text(tier).to_string(),
                })
                .collect(),
        }
    }

    fn project_diagnostics(
        &self,
        binding: &Self::Binding,
        diagnostics: &Self::Diagnostics,
    ) -> DiagnosticsReport {
        DiagnosticsReport {
            template_id: binding.template.id.to_string(),
            fragments: binding
                .template
                .fragment_set
                .iter()
                .map(|fragment| (*fragment).to_string())
                .collect(),
            program_model: diagnostics
                .program_model
                .as_ref()
                .map(model_diagnostics_report),
            reason_model: diagnostics
                .reason_model
                .as_ref()
                .map(model_diagnostics_report),
        }
    }

    fn project_analysis(
        &self,
        binding: &Self::Binding,
        diagnostics: Option<&Self::Diagnostics>,
    ) -> IrReport {
        ir_report_from_binding(binding, diagnostics)
    }
}
