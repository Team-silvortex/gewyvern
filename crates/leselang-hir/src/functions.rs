use std::collections::BTreeMap;

use leselang_runtime_core::ScopeFrame;
use leselang_syntax::{Function, MAX_FUNCTION_PARAMETERS, MAX_FUNCTIONS, NamedArgument};

use super::*;
use crate::computation::{Computation, LocalType, MAX_COMPUTATION_NODES, ScalarType, TypeScope};

struct Template {
    prepared: crate::helper_templates::HelperTemplate<Computation, Type>,
    nodes: usize,
    depth: usize,
}

#[derive(Default)]
pub(super) struct FunctionTemplates {
    templates: BTreeMap<String, Template>,
    names: HashSet<String>,
    next_name: usize,
}

impl FunctionTemplates {
    pub(super) fn contains(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    fn fresh_name(&mut self) -> String {
        loop {
            let name = format!("_lf{}", self.next_name);
            self.next_name += 1;
            if self.names.insert(name.clone()) {
                return name;
            }
        }
    }
}

fn invalid(code: &str, message: &str, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: code.into(),
        message: message.into(),
        span: Some(span),
    }]
}

fn parameter_diagnostics(
    error: crate::helper_source::HelperParameterError,
    span: Span,
) -> Vec<Diagnostic> {
    use crate::helper_source::HelperParameterError;
    let message = match error {
        HelperParameterError::InvalidName { .. } | HelperParameterError::DuplicateName { .. } => {
            "parameter names must be bounded and unique"
        }
        HelperParameterError::UnknownType { .. } => {
            "expected integer, boolean, string, none, optional_string or string_list parameter type"
        }
        _ => "function parameter count exceeds limit",
    };
    invalid("LSH1501", message, span)
}

fn parameter_types(function: &Function) -> Result<Vec<(String, ScalarType)>, Vec<Diagnostic>> {
    use crate::helper_source::HelperParameterError;
    crate::helper_source::helper_parameters(function, MAX_FUNCTION_PARAMETERS)
        .map(|parameters| {
            parameters
                .into_iter()
                .map(|parameter| (parameter.name.to_owned(), parameter.domain))
                .collect()
        })
        .map_err(|error| {
            let span = match error {
                HelperParameterError::InvalidName { index }
                | HelperParameterError::DuplicateName { index }
                | HelperParameterError::UnknownType { index } => function.parameters[index].span,
                _ => function.span,
            };
            parameter_diagnostics(error, span)
        })
}

fn declaration_error(
    error: crate::helper_declarations::HelperDeclarationError<std::convert::Infallible>,
    fallback: Span,
) -> Vec<Diagnostic> {
    use crate::helper_declarations::HelperDeclarationError;
    match error {
        HelperDeclarationError::InvalidLimits | HelperDeclarationError::FunctionLimit => {
            invalid("LSH1501", "function count exceeds limit", fallback)
        }
        HelperDeclarationError::InvalidEntry => invalid(
            "LSH1501",
            "function names must be bounded, unique and not builtins",
            fallback,
        ),
        HelperDeclarationError::InvalidName { span, .. }
        | HelperDeclarationError::ReservedName { span, .. }
        | HelperDeclarationError::DuplicateName { span, .. } => invalid(
            "LSH1501",
            "function names must be bounded, unique and not builtins",
            span,
        ),
        HelperDeclarationError::Parameters { span, error, .. } => {
            parameter_diagnostics(error, span)
        }
        HelperDeclarationError::Source { span, .. } => invalid(
            "LSH1405",
            "function source exceeds computation bounds",
            span,
        ),
        HelperDeclarationError::MissingEntry { span } => invalid(
            "LSH1501",
            "a multi-function program requires exactly one main",
            span,
        ),
        HelperDeclarationError::EntryParameters { span, .. } => {
            invalid("LSH1501", "main cannot have parameters", span)
        }
        HelperDeclarationError::Policy { error, .. } => match error {},
    }
}

fn dependency_error(
    error: crate::helper_dependencies::HelperDependencyError,
    fallback: Span,
) -> Vec<Diagnostic> {
    use crate::helper_dependencies::HelperDependencyError;
    match error {
        HelperDependencyError::EntryCall { span, .. } => {
            invalid("LSH1502", "helpers cannot call main", span)
        }
        HelperDependencyError::Cycle { .. } => invalid(
            "LSH1502",
            "recursive helper functions are not supported",
            fallback,
        ),
        HelperDependencyError::Source { span, .. } => invalid(
            "LSH1405",
            "function source exceeds computation bounds",
            span,
        ),
        HelperDependencyError::InvalidName { span, .. }
        | HelperDependencyError::DuplicateName { span, .. }
        | HelperDependencyError::EntryConflict { span, .. } => invalid(
            "LSH1501",
            "function names must be bounded, unique and not builtins",
            span,
        ),
        _ => invalid("LSH1501", "function count exceeds limit", fallback),
    }
}

pub(super) fn lower_program(tree: &SyntaxTree) -> Result<HirProgram, Vec<Diagnostic>> {
    let declarations = tree
        .function
        .iter()
        .chain(&tree.helpers)
        .collect::<Vec<_>>();
    let span = declarations
        .first()
        .map(|function| function.span)
        .unwrap_or(Span { start: 0, end: 0 });
    let accepted = crate::helper_declarations::accept_helper_declarations(
        &declarations,
        "main",
        crate::helper_declarations::HelperDeclarationLimits {
            max_functions: MAX_FUNCTIONS,
            max_parameters: MAX_FUNCTION_PARAMETERS,
            max_source_nodes: MAX_COMPUTATION_NODES,
            max_source_depth: MAX_EFFECT_NESTING_DEPTH,
        },
        |name| Ok::<_, std::convert::Infallible>(computation::is_builtin(name)),
    )
    .map_err(|error| declaration_error(error, span))?;
    let mut functions = FunctionTemplates::default();
    functions
        .names
        .extend(accepted.reserved_names().map(str::to_owned));
    let main = accepted.entry();
    let order = crate::helper_dependencies::helper_dependency_order(
        accepted.helpers(),
        "main",
        crate::helper_dependencies::HelperDependencyLimits {
            max_helpers: MAX_FUNCTIONS - 1,
            max_source_nodes: MAX_COMPUTATION_NODES,
            max_source_depth: MAX_EFFECT_NESTING_DEPTH,
        },
    )
    .map_err(|error| dependency_error(error, span))?;
    // Lower each ready helper before asking for the next, preserving error priority.
    for function in order {
        let function = function.map_err(|error| dependency_error(error, span))?;
        let parameters = parameter_types(function)?;
        let (body, result_type) = {
            let mut bindings = parameters
                .iter()
                .map(|(name, ty)| (name.as_str(), LocalType::from(Type::Scalar(*ty))))
                .collect();
            let mut scope = ScopeFrame::new(&mut bindings);
            computation::lower_expression_with_functions(
                &function.body,
                &mut scope,
                &mut 0,
                0,
                &mut functions,
            )?
        };
        if body.is_pure() && !matches!(result_type, Type::Scalar(_)) {
            return Err(invalid(
                "LSH1503",
                "a pure helper must return bounded data",
                function.span,
            ));
        }
        body.validate_in_scope(
            &parameters
                .iter()
                .map(|(name, ty)| (name.clone(), Type::Scalar(*ty)))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| {
            invalid(
                "LSH1405",
                "expanded helper exceeds computation or canonical source bounds",
                function.span,
            )
        })?;
        let cost = source_cost(&body).map_err(|_| {
            invalid(
                "LSH1405",
                "expanded helper exceeds computation or canonical source bounds",
                function.span,
            )
        })?;
        let prepared = crate::helper_templates::HelperTemplate::new(
            parameters,
            body,
            result_type,
            template_limits(),
        )
        .map_err(|_| {
            invalid(
                "LSH1405",
                "expanded helper exceeds computation or canonical source bounds",
                function.span,
            )
        })?;
        functions.templates.insert(
            function.name.clone(),
            Template {
                prepared,
                nodes: cost.nodes,
                depth: cost.depth,
            },
        );
    }
    let mut lowered = computation::lower_computation_with_functions(&main.body, &mut functions)?;
    lowered.effect = match lowered.effect {
        Effect::Compute { expression } => match *expression {
            Computation::Host { effect } => *effect,
            expression => Effect::Compute {
                expression: Box::new(expression),
            },
        },
        effect => effect,
    };
    canonical_source(&lowered.effect).map_err(|_| {
        invalid(
            "LSH1405",
            "expanded program exceeds canonical source bounds",
            main.span,
        )
    })?;
    Ok(HirProgram {
        function: HirFunction {
            name: main.name.clone(),
            effect: lowered.effect,
            result_type: lowered.result_type,
            required_capabilities: lowered.required_capabilities,
        },
    })
}

fn template_limits() -> crate::helper_templates::HelperTemplateLimits {
    crate::helper_templates::HelperTemplateLimits {
        max_nodes: MAX_COMPUTATION_NODES,
        max_depth: MAX_EFFECT_NESTING_DEPTH,
        max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
        max_parameters: MAX_FUNCTION_PARAMETERS,
    }
}

pub(super) fn source_cost(
    expression: &Computation,
) -> Result<
    crate::helper_expansion::HelperExpansionCost,
    crate::source_cost::SourceCostError<leselang_runtime_core::StructureError>,
> {
    let limits = crate::source_cost::SourceCostLimits {
        max_nodes: crate::pure_typing::MAX_TYPE_INFERENCE_NODES,
        max_depth: crate::pure_typing::MAX_TYPE_INFERENCE_DEPTH,
    };
    crate::source_cost::measure_source_cost(expression, limits, |effect| {
        let mut effects = vec![(effect, 1)];
        let mut budget =
            leselang_runtime_core::StructureBudget::new(limits.max_nodes, limits.max_depth);
        let mut depth = 0;
        while let Some((effect, effect_depth)) = effects.pop() {
            budget.visit(effect_depth, 0, 0)?;
            depth = depth.max(effect_depth);
            if let Effect::Sequence { steps } | Effect::All { branches: steps } = effect {
                for step in steps {
                    budget.check_pending(effects.len(), 1)?;
                    effects.push((&step.effect, effect_depth + 1));
                }
            }
        }
        Ok(crate::source_cost::SourceCostExtra {
            nodes: budget.visited(),
            depth,
        })
    })
}

pub(super) fn lower_call<'a>(
    name: &str,
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let template = functions
        .templates
        .get(name)
        .ok_or_else(|| invalid("LSH1504", "unknown helper", span))?;
    let parameters = template.prepared.parameters().to_vec();
    let signature = parameters
        .iter()
        .map(|(name, ty)| leselang_runtime_core::NamedParameter::required(name.as_str(), *ty))
        .collect::<Vec<_>>();
    let lowered = crate::helper_source::lower_preflighted_helper_arguments(
        arguments,
        &signature,
        crate::helper_source::HelperSourceLimits {
            source: crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            max_parameters: MAX_FUNCTION_PARAMETERS,
        },
        |argument| {
            let (value, ty) = computation::lower_expression_with_functions(
                &argument.value,
                scope,
                visited,
                depth + 1,
                functions,
            )?;
            Ok((
                value,
                match ty {
                    Type::Scalar(ty) => Some(ty),
                    _ => None,
                },
            ))
        },
    )
    .map_err(|error| match error {
        crate::helper_source::HelperSourceError::Lowering { error, .. } => error,
        crate::helper_source::HelperSourceError::Argument { argument_index, .. } => invalid(
            "LSH1504",
            "helper arguments must be pure scalars of the declared type",
            arguments[argument_index].span,
        ),
        crate::helper_source::HelperSourceError::Output { argument_index, .. } => invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            arguments[argument_index].span,
        ),
        _ => invalid(
            "LSH1504",
            "helper arguments must match its named parameters exactly",
            span,
        ),
    })?;
    let values = lowered
        .into_iter()
        .map(|argument| argument.value)
        .collect::<Vec<_>>();
    let template = &functions.templates[name];
    crate::helper_expansion::reserve_helper_expansion(
        visited,
        depth,
        crate::helper_expansion::HelperExpansionCost {
            nodes: template.nodes,
            depth: template.depth,
        },
        parameters.len(),
        crate::helper_expansion::HelperExpansionLimits {
            max_nodes: MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
            max_parameters: MAX_FUNCTION_PARAMETERS,
        },
        |index| {
            values
                .get(index)
                .ok_or(crate::source_cost::SourceCostError::Structure(
                    leselang_runtime_core::StructureError::NodeLimit,
                ))
                .and_then(|value| source_cost(value).map(|cost| cost.depth))
        },
    )
    .map_err(|_| {
        invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            span,
        )
    })?;
    let body = template
        .prepared
        .materialize(template_limits(), |body| {
            Ok::<_, std::convert::Infallible>(body.clone())
        })
        .map_err(|_| {
            invalid(
                "LSH1405",
                "expanded helper exceeds computation bounds",
                span,
            )
        })?;
    let result_type = *template.prepared.result_type();
    // Arguments stay in caller scope; parameter and body locals get fresh names.
    let parameter_names = parameters
        .iter()
        .map(|(name, _)| (name.as_str(), functions.fresh_name()))
        .collect::<Vec<_>>();
    let aliases = parameter_names
        .iter()
        .map(|(name, alias)| (*name, alias.as_str()))
        .collect::<Vec<_>>();
    let body = crate::helper_hygiene::hygienic_helper_body(
        body,
        &aliases,
        &[],
        crate::helper_hygiene::HelperHygieneLimits {
            max_nodes: MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
            max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
            max_reserved_names: 0,
        },
        || Ok::<_, Vec<Diagnostic>>(functions.fresh_name()),
    )
    .map_err(|error| match error {
        crate::helper_hygiene::HelperHygieneError::Capture { group } => invalid(
            "LSH1503",
            if group {
                "helper cannot capture a caller group"
            } else {
                "helper cannot capture caller locals"
            },
            Span { start: 0, end: 0 },
        ),
        crate::helper_hygiene::HelperHygieneError::Native(error) => error,
        _ => invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            span,
        ),
    })?;
    let body = crate::helper_bindings::bind_helper_arguments(
        body,
        parameter_names
            .into_iter()
            .zip(values)
            .map(|((_, name), value)| crate::helper_bindings::HelperBinding { name, value })
            .collect(),
        crate::helper_bindings::HelperBindingLimits {
            max_nodes: MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
            max_parameters: MAX_FUNCTION_PARAMETERS,
        },
    )
    .map_err(|_| {
        invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            span,
        )
    })?;
    Ok((body, result_type))
}
