use super::{DslError, PipelineModule, semantic_host::GewyvernSemanticHost};

pub(super) type CanonicalAssignment = gewylang_compiler::CanonicalAssignment<GewyvernSemanticHost>;
pub(super) type CanonicalAssignmentValue =
    gewylang_compiler::CanonicalAssignmentValue<GewyvernSemanticHost>;

pub(super) fn lower_pipeline_module_to_assignments(
    module: &PipelineModule,
    allow_template_head: bool,
) -> Result<Vec<CanonicalAssignment>, DslError> {
    gewylang_compiler::lower_pipeline_module(module, &GewyvernSemanticHost, allow_template_head)
        .map_err(DslError::from)
}
