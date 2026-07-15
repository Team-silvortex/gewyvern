use std::collections::BTreeSet;

use leselang_syntax::{Expression, Span, SyntaxTree};
use leserpent_domain::{CAPABILITY_RUNTIME_READ, CapabilitySet, RuntimeListFilter};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirProgram {
    pub function: HirFunction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirFunction {
    pub name: String,
    pub effect: Effect,
    pub result_type: Type,
    pub required_capability: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    RuntimeList { filter: RuntimeListFilter },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    RuntimeList,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

pub fn lower(tree: &SyntaxTree) -> Result<HirProgram, Vec<Diagnostic>> {
    if !tree.diagnostics.is_empty() {
        return Err(tree
            .diagnostics
            .iter()
            .map(|item| Diagnostic {
                code: item.code.clone(),
                message: item.message.clone(),
                span: Some(item.span),
            })
            .collect());
    }
    let Some(function) = &tree.function else {
        return Err(vec![Diagnostic {
            code: "LSH1001".to_string(),
            message: "program has no function".to_string(),
            span: None,
        }]);
    };
    let Expression::Call {
        callee,
        arguments,
        span,
    } = &function.body
    else {
        return Err(vec![Diagnostic {
            code: "LSH1002".to_string(),
            message: "function body must be an effect call".to_string(),
            span: Some(function.span),
        }]);
    };
    if callee != "runtime.list" {
        return Err(vec![Diagnostic {
            code: "LSH1003".to_string(),
            message: format!("unknown effect '{callee}'"),
            span: Some(*span),
        }]);
    }

    let mut seen = BTreeSet::new();
    let mut filter = RuntimeListFilter::default();
    let mut diagnostics = Vec::new();
    for argument in arguments {
        if !seen.insert(argument.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: "LSH1101".to_string(),
                message: format!("duplicate argument '{}'", argument.name),
                span: Some(argument.span),
            });
            continue;
        }
        let value = match &argument.value {
            Expression::String { value, .. } => Some(value.clone()),
            Expression::None { .. } => None,
            Expression::Call { .. } => {
                diagnostics.push(Diagnostic {
                    code: "LSH1102".to_string(),
                    message: "filter arguments require a string or 'none'".to_string(),
                    span: Some(argument.span),
                });
                continue;
            }
        };
        match argument.name.as_str() {
            "environment" => filter.environment = value,
            "cluster" => filter.cluster = value,
            "role" => filter.role = value,
            _ => diagnostics.push(Diagnostic {
                code: "LSH1103".to_string(),
                message: format!("unknown runtime.list argument '{}'", argument.name),
                span: Some(argument.span),
            }),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(HirProgram {
        function: HirFunction {
            name: function.name.clone(),
            effect: Effect::RuntimeList {
                filter: filter.normalized(),
            },
            result_type: Type::RuntimeList,
            required_capability: CAPABILITY_RUNTIME_READ.to_string(),
        },
    })
}

pub fn authorize(program: &HirProgram, capabilities: &CapabilitySet) -> Result<(), Diagnostic> {
    capabilities
        .contains(&program.function.required_capability)
        .then_some(())
        .ok_or_else(|| Diagnostic {
            code: "LSH2001".to_string(),
            message: format!(
                "missing capability '{}'",
                program.function.required_capability
            ),
            span: None,
        })
}

#[cfg(test)]
mod tests {
    use leselang_syntax::parse;

    use super::*;

    #[test]
    fn runtime_list_lowers_to_typed_effect_and_normalized_filter() {
        let program = lower(&parse(
            "fn main() = runtime.list(environment: \" production \", cluster: none)",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeList);
        assert_eq!(
            program.function.required_capability,
            CAPABILITY_RUNTIME_READ
        );
        let Effect::RuntimeList { filter } = program.function.effect;
        assert_eq!(filter.environment.as_deref(), Some("production"));
        assert_eq!(filter.cluster, None);
    }

    #[test]
    fn lowering_rejects_unknown_and_duplicate_arguments() {
        let errors = lower(&parse(
            "fn main() = runtime.list(role: \"edge\", role: none, mystery: \"x\")",
        ))
        .unwrap_err();
        assert!(errors.iter().any(|item| item.code == "LSH1101"));
        assert!(errors.iter().any(|item| item.code == "LSH1103"));
    }

    #[test]
    fn capability_check_is_explicit_and_origin_independent() {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        assert_eq!(
            authorize(&program, &CapabilitySet::default())
                .unwrap_err()
                .code,
            "LSH2001"
        );
        authorize(&program, &CapabilitySet::new([CAPABILITY_RUNTIME_READ])).unwrap();
    }
}
