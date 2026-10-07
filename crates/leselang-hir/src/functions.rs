use std::collections::BTreeMap;

use leselang_runtime_core::ScopeFrame;
use leselang_syntax::{MAX_FUNCTION_PARAMETERS, MAX_FUNCTIONS, NamedArgument};

use super::*;
use crate::computation::{Computation, LocalType, MAX_COMPUTATION_NODES, ScalarType, TypeScope};
use crate::host_call::HostOperation;
use crate::result_field::ResultField;

struct Template {
    prepared: crate::helper_body::HelperBody<Computation, Type>,
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
}

fn fresh_name(names: &mut HashSet<String>, next_name: &mut usize) -> String {
    loop {
        let name = format!("_lf{}", *next_name);
        *next_name += 1;
        if names.insert(name.clone()) {
            return name;
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
    let mut functions = crate::helper_registry::assemble_helper_registry(
        &accepted,
        crate::helper_registry::HelperRegistryLimits {
            dependencies: crate::helper_dependencies::HelperDependencyLimits {
                max_helpers: MAX_FUNCTIONS - 1,
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
            },
            template: template_limits(),
            source_cost: source_cost_limits(),
        },
        functions,
        |function, functions| {
            let source_limits = source_cost_limits();
            let admitted = crate::helper_body_source::lower_preflighted_helper_body(
                function,
                crate::helper_body_source::HelperBodySourceLimits {
                    max_source_nodes: MAX_COMPUTATION_NODES,
                    max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
                    body: crate::helper_body::HelperBodyLimits {
                        template: template_limits(),
                        source_cost: source_limits,
                    },
                },
                &mut 0,
                |function, parameters, visited| {
                    let mut bindings = parameters
                        .iter()
                        .map(|parameter| {
                            (
                                parameter.name,
                                LocalType::from(Type::Scalar(parameter.domain)),
                            )
                        })
                        .collect();
                    let mut scope = ScopeFrame::new(&mut bindings);
                    let (body, result_type) = computation::lower_expression_with_functions(
                        &function.body,
                        &mut scope,
                        visited,
                        0,
                        functions,
                    )?;
                    Ok(crate::helper_body_source::HelperBodyObservation {
                        expression: body,
                        result_type,
                        scalar_result: match result_type {
                            Type::Scalar(ty) => Some(ty),
                            _ => None,
                        },
                    })
                },
                |body, parameters, result_type, _| {
                    let checked = body.validate_in_scope(
                        &parameters
                            .iter()
                            .map(|(name, ty)| (name.clone(), Type::Scalar(*ty)))
                            .collect::<Vec<_>>(),
                    )?;
                    if checked != *result_type {
                        return Err(crate::CanonicalSourceError::RoundTripMismatch);
                    }
                    Ok(())
                },
                |effect| host_source_extra(effect, source_limits),
            )
            .map_err(|error| {
                use crate::helper_body_source::HelperBodySourceError;
                if let HelperBodySourceError::Lowering(error) = error {
                    return error;
                }
                if let HelperBodySourceError::Parameters(error) = error {
                    use crate::helper_source::HelperParameterError;
                    let span = match error {
                        HelperParameterError::InvalidName { index }
                        | HelperParameterError::DuplicateName { index }
                        | HelperParameterError::UnknownType { index } => {
                            function.parameters[index].span
                        }
                        _ => function.span,
                    };
                    return parameter_diagnostics(error, span);
                }
                if matches!(
                    error,
                    HelperBodySourceError::Admission(
                        crate::helper_body::HelperBodyError::PureResultRequired
                    )
                ) {
                    invalid(
                        "LSH1503",
                        "a pure helper must return bounded data",
                        function.span,
                    )
                } else {
                    invalid(
                        "LSH1405",
                        "expanded helper exceeds computation or canonical source bounds",
                        function.span,
                    )
                }
            })?;
            Ok(admitted)
        },
        |function, admitted, functions| {
            functions
                .templates
                .insert(function.name.clone(), Template { prepared: admitted });
            Ok::<_, std::convert::Infallible>(())
        },
    )
    .map_err(|error| {
        use crate::helper_registry::HelperRegistryError;
        match error {
            HelperRegistryError::Dependency(error) => dependency_error(error, span),
            HelperRegistryError::Lowering { error, .. } => error,
            HelperRegistryError::Registration { error, .. } => match error {},
            HelperRegistryError::Parameters { span, error } => parameter_diagnostics(error, span),
            HelperRegistryError::Template { span, .. }
            | HelperRegistryError::ParameterScopeLimit { span }
            | HelperRegistryError::Signature { span }
            | HelperRegistryError::SourceCost { span, .. } => invalid(
                "LSH1405",
                "expanded helper exceeds computation or canonical source bounds",
                span,
            ),
            HelperRegistryError::InvalidLimits => {
                invalid("LSH1501", "function count exceeds limit", span)
            }
        }
    })?;
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
    let limits = source_cost_limits();
    crate::source_cost::measure_source_cost(expression, limits, |effect| {
        host_source_extra(effect, limits)
    })
}

fn source_cost_limits() -> crate::source_cost::SourceCostLimits {
    crate::source_cost::SourceCostLimits {
        max_nodes: crate::pure_typing::MAX_TYPE_INFERENCE_NODES,
        max_depth: crate::pure_typing::MAX_TYPE_INFERENCE_DEPTH,
    }
}

pub(super) fn host_source_extra(
    effect: &Effect,
    limits: crate::source_cost::SourceCostLimits,
) -> Result<crate::source_cost::SourceCostExtra, leselang_runtime_core::StructureError> {
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
    let cached = template.prepared.template();
    crate::helper_templates::preflight(cached.parameters(), cached.body(), template_limits())
        .map_err(
            |_: crate::helper_templates::HelperTemplateError<std::convert::Infallible>| {
                invalid(
                    "LSH1405",
                    "expanded helper exceeds computation bounds",
                    span,
                )
            },
        )?;
    let parameters = cached.parameters().to_vec();
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
    let argument_spans = lowered
        .iter()
        .map(|argument| argument.argument.span)
        .collect::<Vec<_>>();
    let values = lowered
        .into_iter()
        .map(
            |argument| crate::helper_instance_finish::HelperInstanceArgument {
                value: argument.value,
                scalar_type: Some(argument.parameter.domain),
            },
        )
        .collect();
    let template = &functions.templates[name];
    let reserved_names = scope
        .bindings()
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    let (body, result_type) = crate::helper_instance_finish::finish_helper_instance(
        crate::helper_instance::SelectedHelper {
            name,
            body: &template.prepared,
            reserved_names: &reserved_names,
            caller_depth: depth,
        },
        values,
        crate::helper_instance::HelperInstanceLimits {
            source: crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            max_parameters: MAX_FUNCTION_PARAMETERS,
            max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
            max_reserved_names: MAX_COMPUTATION_NODES,
        },
        visited,
        &mut InstanceFinisher {
            scope,
            names: &mut functions.names,
            next_name: &mut functions.next_name,
            argument_spans: &argument_spans,
            span,
        },
    )
    .map_err(|error| match error {
        crate::helper_instance::HelperInstanceError::Native { error, .. } => error,
        crate::helper_instance::HelperInstanceError::Hygiene(
            crate::helper_hygiene::HelperHygieneError::Capture { group },
        ) => invalid(
            "LSH1503",
            if group {
                "helper cannot capture a caller group"
            } else {
                "helper cannot capture caller locals"
            },
            Span { start: 0, end: 0 },
        ),
        crate::helper_instance::HelperInstanceError::Hygiene(
            crate::helper_hygiene::HelperHygieneError::Native(error),
        ) => error,
        _ => invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            span,
        ),
    })?;
    Ok((body, *result_type))
}

struct InstanceFinisher<'adapter, 'scope, 'source> {
    scope: &'adapter TypeScope<'scope, 'source>,
    names: &'adapter mut HashSet<String>,
    next_name: &'adapter mut usize,
    argument_spans: &'adapter [Span],
    span: Span,
}
impl
    crate::helper_instance_finish::HelperInstanceFinisher<
        ResultField,
        HostOperation,
        Effect,
        Type,
        Type,
    > for InstanceFinisher<'_, '_, '_>
{
    type Error = Vec<Diagnostic>;
    fn admit_argument(
        &mut self,
        value: &Computation,
        expected: ScalarType,
        index: usize,
    ) -> Result<(), Self::Error> {
        if value.validate_in_type_scope(self.scope).ok() != Some(Type::Scalar(expected)) {
            return Err(invalid(
                "LSH1504",
                "helper arguments must be pure scalars of the declared type",
                self.argument_spans.get(index).copied().unwrap_or(self.span),
            ));
        }
        Ok(())
    }
    fn argument_depth(&mut self, value: &Computation) -> Result<usize, Self::Error> {
        source_cost(value).map(|cost| cost.depth).map_err(|_| {
            invalid(
                "LSH1405",
                "expanded helper exceeds computation bounds",
                self.span,
            )
        })
    }
    fn materialize(&mut self, body: &Computation) -> Result<Computation, Self::Error> {
        Ok(body.clone())
    }
    fn fresh_name(&mut self) -> Result<String, Self::Error> {
        Ok(fresh_name(self.names, self.next_name))
    }
    fn admit(&mut self, value: &Computation, result: &Type) -> Result<(), Self::Error> {
        if value.validate_in_type_scope(self.scope).ok() != Some(*result) {
            return Err(invalid(
                "LSH1405",
                "expanded helper exceeds computation bounds",
                self.span,
            ));
        }
        Ok(())
    }
}
