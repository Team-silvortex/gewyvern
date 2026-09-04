use crate::{BindingReport, DiagnosticsReport, IrReport};

/// Product boundary for projecting one host binding into stable GewyLang IR.
///
/// The host retains ownership of executable bindings and diagnostic models.
/// This contract owns only the orchestration and stable report values that can
/// cross out of that host.
pub trait CompilerProjectionHost {
    type Binding;
    type Diagnostics;

    fn project_binding(&self, binding: &Self::Binding) -> BindingReport;

    fn project_diagnostics(
        &self,
        binding: &Self::Binding,
        diagnostics: &Self::Diagnostics,
    ) -> DiagnosticsReport;

    fn project_analysis(
        &self,
        binding: &Self::Binding,
        diagnostics: Option<&Self::Diagnostics>,
    ) -> IrReport;
}

/// Coherent compiler-stage projections from one binding and diagnostic result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerStageProjections<E> {
    pub binding: BindingReport,
    pub diagnostics: Result<DiagnosticsReport, E>,
    pub analysis: IrReport,
}

/// Projects every stable compiler IR surface without erasing host failures.
pub fn project_compiler_stages<H, E>(
    host: &H,
    binding: &H::Binding,
    diagnostics: Result<&H::Diagnostics, E>,
) -> CompilerStageProjections<E>
where
    H: CompilerProjectionHost,
{
    let analysis = host.project_analysis(binding, diagnostics.as_ref().ok().copied());
    CompilerStageProjections {
        binding: host.project_binding(binding),
        diagnostics: diagnostics.map(|diagnostics| host.project_diagnostics(binding, diagnostics)),
        analysis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBinding {
        id: String,
    }

    struct TestDiagnostics;

    struct TestHost;

    impl CompilerProjectionHost for TestHost {
        type Binding = TestBinding;
        type Diagnostics = TestDiagnostics;

        fn project_binding(&self, binding: &Self::Binding) -> BindingReport {
            BindingReport {
                template_id: binding.id.clone(),
                fragments: Vec::new(),
                window: None,
                reason_profile: None,
                program_model: None,
                fragment_params: Vec::new(),
                evidence_overrides: Vec::new(),
            }
        }

        fn project_diagnostics(
            &self,
            binding: &Self::Binding,
            _diagnostics: &Self::Diagnostics,
        ) -> DiagnosticsReport {
            DiagnosticsReport {
                template_id: binding.id.clone(),
                fragments: Vec::new(),
                program_model: None,
                reason_model: None,
            }
        }

        fn project_analysis(
            &self,
            binding: &Self::Binding,
            _diagnostics: Option<&Self::Diagnostics>,
        ) -> IrReport {
            IrReport {
                template_id: binding.id.clone(),
                program_model: None,
                reason_model: None,
            }
        }
    }

    #[test]
    fn standalone_projection_host_builds_one_coherent_stage_set() {
        let binding = TestBinding {
            id: "standalone".into(),
        };
        let diagnostics = TestDiagnostics;
        let projections =
            project_compiler_stages(&TestHost, &binding, Ok::<_, &'static str>(&diagnostics));

        assert_eq!(projections.binding.template_id, "standalone");
        assert_eq!(projections.diagnostics.unwrap().template_id, "standalone");
        assert_eq!(projections.analysis.template_id, "standalone");
    }

    #[test]
    fn standalone_projection_host_preserves_diagnostic_failures() {
        let binding = TestBinding {
            id: "standalone".into(),
        };
        let projections = project_compiler_stages(
            &TestHost,
            &binding,
            Err::<&TestDiagnostics, _>("registry-unavailable"),
        );

        assert_eq!(projections.diagnostics, Err("registry-unavailable"));
        assert_eq!(projections.binding.template_id, "standalone");
        assert_eq!(projections.analysis.template_id, "standalone");
    }
}
