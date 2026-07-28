use std::collections::HashSet;

use leselang_syntax::{Expression, Span, SyntaxTree, format as format_syntax, parse};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, DomainError, RuntimeId, RuntimeListFilter,
    validate_debugger_session_id, validate_deployment_intent,
};
use serde::{Deserialize, Serialize};

pub const MAX_ALL_BRANCHES: usize = 64;
pub const MAX_BRANCH_NAME_BYTES: usize = 64;
pub const MAX_UI_NODE_ID_BYTES: usize = 128;
pub const CAPABILITY_UI_PRESENTATION: &str = "ui.presentation";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirProgram {
    pub function: HirFunction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirFunction {
    pub name: String,
    pub effect: Effect,
    pub result_type: Type,
    pub required_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirBranch {
    pub name: String,
    pub effect: Effect,
    pub result_type: Type,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    RuntimeList {
        filter: RuntimeListFilter,
    },
    RuntimeInspect {
        runtime_id: RuntimeId,
    },
    RuntimeHistory {
        runtime_id: RuntimeId,
    },
    RuntimeLogs {
        runtime_id: RuntimeId,
    },
    RuntimeRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeCapabilitiesRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeDeploy {
        runtime_id: RuntimeId,
        pipeline_kind: String,
        target: Option<String>,
    },
    DebuggerCancel {
        session_id: String,
    },
    UiFocus {
        node_id: String,
    },
    UiScrollIntoView {
        node_id: String,
    },
    UiAssertVisible {
        node_id: String,
    },
    UiAssertFocused {
        node_id: String,
    },
    All {
        branches: Vec<HirBranch>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    RuntimeList,
    RuntimeInspect,
    RuntimeHistory,
    RuntimeLogs,
    RuntimeRefresh,
    RuntimeCapabilitiesRefresh,
    RuntimeDeploy,
    DebuggerCancel,
    UiFocus,
    UiScrollIntoView,
    UiAssertVisible,
    UiAssertFocused,
    Structured,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSourceError {
    Syntax(Vec<leselang_syntax::Diagnostic>),
    InvalidEffect(Vec<Diagnostic>),
    RoundTripMismatch,
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
    let lowered = lower_effect(&function.body)?;
    Ok(HirProgram {
        function: HirFunction {
            name: function.name.clone(),
            effect: lowered.effect,
            result_type: lowered.result_type,
            required_capabilities: lowered.required_capabilities,
        },
    })
}

struct LoweredEffect {
    effect: Effect,
    result_type: Type,
    required_capabilities: Vec<String>,
}

fn lower_effect(expression: &Expression) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let Expression::Call {
        callee,
        arguments,
        span,
    } = expression
    else {
        return Err(vec![Diagnostic {
            code: "LSH1002".to_string(),
            message: "structured branches must contain effect calls".to_string(),
            span: Some(expression_span(expression)),
        }]);
    };
    match callee.as_str() {
        "runtime.list"
        | "runtime.inspect"
        | "runtime.history"
        | "runtime.logs"
        | "runtime.refresh"
        | "runtime.refresh_capabilities"
        | "runtime.deploy"
        | "debugger.cancel"
        | "ui.focus"
        | "ui.scroll_into_view"
        | "ui.assert_visible"
        | "ui.assert_focused" => lower_atomic_effect(callee, arguments, *span),
        "all" => lower_all(arguments, *span),
        _ => Err(vec![Diagnostic {
            code: "LSH1003".to_string(),
            message: format!("unknown effect or structured form '{callee}'"),
            span: Some(*span),
        }]),
    }
}

fn lower_atomic_effect(
    callee: &str,
    arguments: &[leselang_syntax::NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let mut seen = HashSet::with_capacity(arguments.len());
    let mut filter = RuntimeListFilter::default();
    let mut runtime_id = None;
    let mut pipeline_kind = None;
    let mut target = None;
    let mut session_id = None;
    let mut node_id = None;
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
        match (callee, argument.name.as_str()) {
            ("runtime.list", "environment") => filter.environment = value,
            ("runtime.list", "cluster") => filter.cluster = value,
            ("runtime.list", "role") => filter.role = value,
            (
                "runtime.inspect"
                | "runtime.history"
                | "runtime.logs"
                | "runtime.refresh"
                | "runtime.refresh_capabilities"
                | "runtime.deploy",
                "runtime_id",
            ) => match value.and_then(|value| RuntimeId::new(value).ok()) {
                Some(value) => runtime_id = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1104".to_string(),
                    message: format!("{callee} runtime_id must be a valid identifier string"),
                    span: Some(argument.span),
                }),
            },
            ("runtime.deploy", "pipeline_kind") => match value {
                Some(value) => pipeline_kind = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1106".to_string(),
                    message: "runtime.deploy pipeline_kind must be a valid token string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("runtime.deploy", "target") => match value {
                Some(value) => target = Some(value),
                None => target = None,
            },
            ("debugger.cancel", "session_id") => match value {
                Some(value) if validate_debugger_session_id(&value).is_ok() => {
                    session_id = Some(value);
                }
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1110".to_string(),
                    message: "debugger.cancel session_id must be a valid identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.focus", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1112".to_string(),
                    message: "ui.focus node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.scroll_into_view", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1114".to_string(),
                    message:
                        "ui.scroll_into_view node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_visible", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1116".to_string(),
                    message: "ui.assert_visible node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_focused", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1118".to_string(),
                    message: "ui.assert_focused node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            _ => diagnostics.push(Diagnostic {
                code: "LSH1103".to_string(),
                message: format!("unknown {callee} argument '{}'", argument.name),
                span: Some(argument.span),
            }),
        }
    }
    if matches!(
        callee,
        "runtime.inspect"
            | "runtime.history"
            | "runtime.logs"
            | "runtime.refresh"
            | "runtime.refresh_capabilities"
            | "runtime.deploy"
    ) && runtime_id.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1105".to_string(),
            message: format!("{callee} requires runtime_id"),
            span: Some(span),
        });
    }
    if callee == "runtime.deploy" && pipeline_kind.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1108".to_string(),
            message: "runtime.deploy requires pipeline_kind".to_string(),
            span: Some(span),
        });
    }
    if callee == "debugger.cancel" && session_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1111".to_string(),
            message: "debugger.cancel requires session_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.focus" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1113".to_string(),
            message: "ui.focus requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.scroll_into_view" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1115".to_string(),
            message: "ui.scroll_into_view requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_visible" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1117".to_string(),
            message: "ui.assert_visible requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_focused" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1119".to_string(),
            message: "ui.assert_focused requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "runtime.deploy"
        && diagnostics.is_empty()
        && let Some(pipeline_kind) = pipeline_kind.as_deref()
        && let Err(DomainError::InvalidIdentifier { field }) =
            validate_deployment_intent(pipeline_kind, target.as_deref())
    {
        diagnostics.push(Diagnostic {
            code: if field == "target" {
                "LSH1107"
            } else {
                "LSH1106"
            }
            .to_string(),
            message: if field == "target" {
                "runtime.deploy target must be a bounded text string or none"
            } else {
                "runtime.deploy pipeline_kind must be a valid token string"
            }
            .to_string(),
            span: Some(span),
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let (effect, result_type, required_capability) = match callee {
        "runtime.list" => (
            Effect::RuntimeList {
                filter: filter.normalized(),
            },
            Type::RuntimeList,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.inspect" => (
            Effect::RuntimeInspect {
                runtime_id: runtime_id.expect("validated runtime.inspect identifier"),
            },
            Type::RuntimeInspect,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.history" => (
            Effect::RuntimeHistory {
                runtime_id: runtime_id.expect("validated runtime.history identifier"),
            },
            Type::RuntimeHistory,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.logs" => (
            Effect::RuntimeLogs {
                runtime_id: runtime_id.expect("validated runtime.logs identifier"),
            },
            Type::RuntimeLogs,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.refresh" => (
            Effect::RuntimeRefresh {
                runtime_id: runtime_id.expect("validated runtime.refresh identifier"),
            },
            Type::RuntimeRefresh,
            CAPABILITY_RUNTIME_REFRESH,
        ),
        "runtime.refresh_capabilities" => (
            Effect::RuntimeCapabilitiesRefresh {
                runtime_id: runtime_id.expect("validated runtime.refresh_capabilities identifier"),
            },
            Type::RuntimeCapabilitiesRefresh,
            CAPABILITY_RUNTIME_REFRESH,
        ),
        "runtime.deploy" => (
            Effect::RuntimeDeploy {
                runtime_id: runtime_id.expect("validated runtime.deploy identifier"),
                pipeline_kind: pipeline_kind.expect("validated runtime.deploy pipeline kind"),
                target,
            },
            Type::RuntimeDeploy,
            CAPABILITY_RUNTIME_DEPLOY,
        ),
        "debugger.cancel" => (
            Effect::DebuggerCancel {
                session_id: session_id.expect("validated debugger session identifier"),
            },
            Type::DebuggerCancel,
            CAPABILITY_DEBUGGER_CONTROL,
        ),
        "ui.focus" => (
            Effect::UiFocus {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiFocus,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.scroll_into_view" => (
            Effect::UiScrollIntoView {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiScrollIntoView,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_visible" => (
            Effect::UiAssertVisible {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertVisible,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_focused" => (
            Effect::UiAssertFocused {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertFocused,
            CAPABILITY_UI_PRESENTATION,
        ),
        _ => unreachable!("unknown effects returned above"),
    };
    Ok(LoweredEffect {
        effect,
        result_type,
        required_capabilities: vec![required_capability.to_string()],
    })
}

pub fn canonical_source(effect: &Effect) -> Result<String, CanonicalSourceError> {
    let source = format!("fn main() = {}", canonical_effect_source(effect, 0));
    let formatted = format_syntax(&parse(&source)).map_err(CanonicalSourceError::Syntax)?;
    let round_trip = lower(&parse(&formatted)).map_err(CanonicalSourceError::InvalidEffect)?;
    if round_trip.function.effect != *effect {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    Ok(formatted)
}

fn canonical_effect_source(effect: &Effect, depth: usize) -> String {
    match effect {
        Effect::RuntimeList { filter } => format!(
            "runtime.list(\n{}environment: {},\n{}cluster: {},\n{}role: {},\n{})",
            indent(depth + 1),
            optional_string(filter.environment.as_deref()),
            indent(depth + 1),
            optional_string(filter.cluster.as_deref()),
            indent(depth + 1),
            optional_string(filter.role.as_deref()),
            indent(depth),
        ),
        Effect::RuntimeInspect { runtime_id } => {
            atomic_identifier_source("runtime.inspect", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeHistory { runtime_id } => {
            atomic_identifier_source("runtime.history", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeLogs { runtime_id } => {
            atomic_identifier_source("runtime.logs", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeRefresh { runtime_id } => {
            atomic_identifier_source("runtime.refresh", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeCapabilitiesRefresh { runtime_id } => atomic_identifier_source(
            "runtime.refresh_capabilities",
            "runtime_id",
            runtime_id.as_str(),
            depth,
        ),
        Effect::RuntimeDeploy {
            runtime_id,
            pipeline_kind,
            target,
        } => format!(
            "runtime.deploy(\n{}runtime_id: {},\n{}pipeline_kind: {},\n{}target: {},\n{})",
            indent(depth + 1),
            quote(runtime_id.as_str()),
            indent(depth + 1),
            quote(pipeline_kind),
            indent(depth + 1),
            optional_string(target.as_deref()),
            indent(depth),
        ),
        Effect::DebuggerCancel { session_id } => {
            atomic_identifier_source("debugger.cancel", "session_id", session_id, depth)
        }
        Effect::UiFocus { node_id } => {
            atomic_identifier_source("ui.focus", "node_id", node_id, depth)
        }
        Effect::UiScrollIntoView { node_id } => {
            atomic_identifier_source("ui.scroll_into_view", "node_id", node_id, depth)
        }
        Effect::UiAssertVisible { node_id } => {
            atomic_identifier_source("ui.assert_visible", "node_id", node_id, depth)
        }
        Effect::UiAssertFocused { node_id } => {
            atomic_identifier_source("ui.assert_focused", "node_id", node_id, depth)
        }
        Effect::All { branches } => {
            let mut source = String::from("all(\n");
            for branch in branches {
                source.push_str(&indent(depth + 1));
                source.push_str(&branch.name);
                source.push_str(": ");
                source.push_str(&canonical_effect_source(&branch.effect, depth + 1));
                source.push_str(",\n");
            }
            source.push_str(&indent(depth));
            source.push(')');
            source
        }
    }
}

pub fn validate_ui_node_id(node_id: &str) -> bool {
    !node_id.is_empty()
        && node_id.len() <= MAX_UI_NODE_ID_BYTES
        && node_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn atomic_identifier_source(callee: &str, argument: &str, value: &str, depth: usize) -> String {
    format!(
        "{callee}(\n{}{argument}: {},\n{})",
        indent(depth + 1),
        quote(value),
        indent(depth),
    )
}

fn optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "none".to_string(), quote)
}

fn quote(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn lower_all(
    arguments: &[leselang_syntax::NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    if !(2..=MAX_ALL_BRANCHES).contains(&arguments.len()) {
        return Err(vec![Diagnostic {
            code: "LSH1201".to_string(),
            message: "all requires between 2 and 64 named branches".to_string(),
            span: Some(span),
        }]);
    }
    let mut names = HashSet::with_capacity(arguments.len());
    let mut branches = Vec::with_capacity(arguments.len());
    let mut capabilities = Vec::new();
    let mut diagnostics = Vec::new();
    for argument in arguments {
        if argument.name.len() > MAX_BRANCH_NAME_BYTES {
            diagnostics.push(Diagnostic {
                code: "LSH1203".to_string(),
                message: "all branch name exceeds 64 bytes".to_string(),
                span: Some(argument.span),
            });
            continue;
        }
        if !names.insert(argument.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: "LSH1202".to_string(),
                message: format!("duplicate all branch '{}'", argument.name),
                span: Some(argument.span),
            });
            continue;
        }
        match lower_effect(&argument.value) {
            Ok(lowered) => {
                for capability in lowered.required_capabilities {
                    if !capabilities.contains(&capability) {
                        capabilities.push(capability);
                    }
                }
                branches.push(HirBranch {
                    name: argument.name.clone(),
                    effect: lowered.effect,
                    result_type: lowered.result_type,
                });
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(LoweredEffect {
        effect: Effect::All { branches },
        result_type: Type::Structured,
        required_capabilities: capabilities,
    })
}

fn expression_span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::String { span, .. }
        | Expression::None { span } => *span,
    }
}

pub fn authorize(program: &HirProgram, capabilities: &CapabilitySet) -> Result<(), Diagnostic> {
    program
        .function
        .required_capabilities
        .iter()
        .find(|required| !capabilities.contains(required))
        .map_or(Ok(()), |required| {
            Err(Diagnostic {
                code: "LSH2001".to_string(),
                message: format!("missing capability '{required}'"),
                span: None,
            })
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
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        let Effect::RuntimeList { filter } = program.function.effect else {
            panic!("expected runtime.list effect");
        };
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

    #[test]
    fn runtime_refresh_lowers_to_typed_mutating_effect() {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeRefresh);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_REFRESH]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeRefresh { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_capability_refresh_has_a_typed_shared_capability_contract() {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::RuntimeCapabilitiesRefresh
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_REFRESH]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeCapabilitiesRefresh { ref runtime_id }
                if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_inspect_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeInspect);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeInspect { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_history_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeHistory);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeHistory { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_logs_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeLogs);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeLogs { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_refresh_requires_one_valid_identifier() {
        for source in [
            "fn main() = runtime.refresh()",
            "fn main() = runtime.refresh(runtime_id: none)",
            "fn main() = runtime.refresh(runtime_id: \"bad/id\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn runtime_logs_requires_one_valid_identifier() {
        for source in [
            "fn main() = runtime.logs()",
            "fn main() = runtime.logs(runtime_id: none)",
            "fn main() = runtime.logs(runtime_id: \"bad/id\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn runtime_deploy_lowers_a_bounded_explicit_intent() {
        let program = lower(&parse(
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \"pid:42\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeDeploy);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_DEPLOY]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeDeploy {
                ref runtime_id,
                ref pipeline_kind,
                target: Some(ref target),
            } if runtime_id.as_str() == "runtime-a"
                && pipeline_kind == "http/request"
                && target == "pid:42"
        ));
        for source in [
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: none)",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"bad kind\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \" bad\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn debugger_cancel_lowers_and_canonical_source_round_trips() {
        let program = lower(&parse(
            "fn main() = debugger.cancel(session_id: \"session-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::DebuggerCancel);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_DEBUGGER_CONTROL]
        );
        assert!(matches!(
            program.function.effect,
            Effect::DebuggerCancel { ref session_id } if session_id == "session-a"
        ));

        let source = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            source,
            "fn main() = debugger.cancel(session_id: \"session-a\")\n"
        );
        assert_eq!(
            lower(&parse(&source)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = debugger.cancel()",
            "fn main() = debugger.cancel(session_id: none)",
            "fn main() = debugger.cancel(session_id: \"bad/session\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        assert!(matches!(
            canonical_source(&Effect::DebuggerCancel {
                session_id: "bad/session".into(),
            }),
            Err(CanonicalSourceError::InvalidEffect(errors))
                if errors.iter().any(|error| error.code == "LSH1110")
        ));
    }

    #[test]
    fn ui_focus_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiFocus);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiFocus { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.focus()",
            "fn main() = ui.focus(node_id: none)",
            "fn main() = ui.focus(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_scroll_into_view_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiScrollIntoView);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiScrollIntoView { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.scroll_into_view()",
            "fn main() = ui.scroll_into_view(node_id: none)",
            "fn main() = ui.scroll_into_view(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_visible_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertVisible);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertVisible { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.assert_visible()",
            "fn main() = ui.assert_visible(node_id: none)",
            "fn main() = ui.assert_visible(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_focused_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertFocused);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFocused { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.assert_focused()",
            "fn main() = ui.assert_focused(node_id: none)",
            "fn main() = ui.assert_focused(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn every_atomic_effect_has_one_semantically_stable_canonical_source() {
        for source in [
            "fn main() = runtime.list(environment: \"prod\", cluster: none, role: \"edge\")",
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: none)",
            "fn main() = debugger.cancel(session_id: \"session-a\")",
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
        ] {
            let effect = lower(&parse(source)).unwrap().function.effect;
            let canonical = canonical_source(&effect).unwrap();
            assert_eq!(
                lower(&parse(&canonical)).unwrap().function.effect,
                effect,
                "canonical source changed effect `{source}`"
            );
        }
    }

    #[test]
    fn all_lowers_branches_in_declared_order_and_unions_capabilities() {
        let program = lower(&parse(
            "fn main() = all(inventory: runtime.list(role: \"edge\"), refresh: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::Structured);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH]
        );
        let Effect::All { branches } = program.function.effect else {
            panic!("expected all effect");
        };
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.name.as_str())
                .collect::<Vec<_>>(),
            ["inventory", "refresh"]
        );
        assert_eq!(branches[0].result_type, Type::RuntimeList);
        assert_eq!(branches[1].result_type, Type::RuntimeRefresh);
    }

    #[test]
    fn all_rejects_invalid_shape_and_authorizes_every_branch() {
        for source in [
            "fn main() = all(only: runtime.list())",
            "fn main() = all(left: runtime.list(), left: runtime.list())",
            "fn main() = all(left: \"not-an-effect\", right: runtime.list())",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let long_name = "x".repeat(MAX_BRANCH_NAME_BYTES + 1);
        assert!(
            lower(&parse(&format!(
                "fn main() = all({long_name}: runtime.list(), right: runtime.list())"
            )))
            .unwrap_err()
            .iter()
            .any(|item| item.code == "LSH1203")
        );
        let program = lower(&parse(
            "fn main() = all(read: runtime.list(), write: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        let error =
            authorize(&program, &CapabilitySet::new([CAPABILITY_RUNTIME_READ])).unwrap_err();
        assert_eq!(error.code, "LSH2001");
        assert!(error.message.contains(CAPABILITY_RUNTIME_REFRESH));
    }
}
