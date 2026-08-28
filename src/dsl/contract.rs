/// Stable language identifier carried by every public GewyLang stage stamp.
pub const GEWYLANG_LANGUAGE_ID: &str = "gewylang";
/// Canonical `.gewy` source syntax version accepted by the current compiler.
pub const GEWYLANG_SYNTAX_VERSION: u32 = 1;
/// Public expanded package and provenance projection version.
pub const GEWYLANG_EXPANDED_AST_VERSION: u32 = 1;
/// Executable `TemplateBinding` semantic contract version.
pub const GEWYLANG_BINDING_IR_VERSION: u32 = 1;
/// Diagnostics-enriched `IrReport` contract version.
pub const GEWYLANG_ANALYSIS_IR_VERSION: u32 = 1;

/// Public, versioned compiler boundaries exposed by GewyLang tooling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GewyLangStage {
    ExpandedAst,
    BindingIr,
    AnalysisIr,
}

impl GewyLangStage {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ExpandedAst => "expanded_ast",
            Self::BindingIr => "binding_ir",
            Self::AnalysisIr => "analysis_ir",
        }
    }

    pub const fn version(self) -> u32 {
        match self {
            Self::ExpandedAst => GEWYLANG_EXPANDED_AST_VERSION,
            Self::BindingIr => GEWYLANG_BINDING_IR_VERSION,
            Self::AnalysisIr => GEWYLANG_ANALYSIS_IR_VERSION,
        }
    }
}

/// Machine-readable identity for one public GewyLang compiler-stage payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GewyLangContractStamp {
    pub language: &'static str,
    pub syntax_version: u32,
    pub stage: GewyLangStage,
    pub stage_version: u32,
}

impl GewyLangContractStamp {
    pub const fn for_stage(stage: GewyLangStage) -> Self {
        Self {
            language: GEWYLANG_LANGUAGE_ID,
            syntax_version: GEWYLANG_SYNTAX_VERSION,
            stage,
            stage_version: stage.version(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_stamps_use_one_syntax_version_and_stage_specific_versions() {
        for (stage, id, version) in [
            (
                GewyLangStage::ExpandedAst,
                "expanded_ast",
                GEWYLANG_EXPANDED_AST_VERSION,
            ),
            (
                GewyLangStage::BindingIr,
                "binding_ir",
                GEWYLANG_BINDING_IR_VERSION,
            ),
            (
                GewyLangStage::AnalysisIr,
                "analysis_ir",
                GEWYLANG_ANALYSIS_IR_VERSION,
            ),
        ] {
            let stamp = GewyLangContractStamp::for_stage(stage);
            assert_eq!(stamp.language, GEWYLANG_LANGUAGE_ID);
            assert_eq!(stamp.syntax_version, GEWYLANG_SYNTAX_VERSION);
            assert_eq!(stamp.stage.id(), id);
            assert_eq!(stamp.stage_version, version);
        }
    }
}
