use std::collections::{BTreeMap, BTreeSet};

use leselang_runtime_core::ScopeFrame;
use leselang_syntax::{Function, MAX_FUNCTION_PARAMETERS, MAX_FUNCTIONS, NamedArgument};

use super::*;
use crate::computation::{Computation, LocalType, MAX_COMPUTATION_NODES, ScalarType, TypeScope};

struct Template {
    parameters: Vec<(String, ScalarType)>,
    body: Computation,
    result_type: Type,
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

fn parameter_types(function: &Function) -> Result<Vec<(String, ScalarType)>, Vec<Diagnostic>> {
    if function.parameters.len() > MAX_FUNCTION_PARAMETERS {
        return Err(invalid(
            "LSH1501",
            "function parameter count exceeds limit",
            function.span,
        ));
    }
    let mut names = HashSet::new();
    function
        .parameters
        .iter()
        .map(|parameter| {
            if !computation::valid_local(&parameter.name) || !names.insert(&parameter.name) {
                return Err(invalid(
                    "LSH1501",
                    "parameter names must be bounded and unique",
                    parameter.span,
                ));
            }
            let ty = match parameter.type_name.as_str() {
                "integer" => ScalarType::Integer,
                "boolean" => ScalarType::Boolean,
                "string" => ScalarType::String,
                "none" => ScalarType::None,
                "optional_string" => ScalarType::OptionalString,
                "string_list" => ScalarType::StringList,
                _ => {
                    return Err(invalid(
                        "LSH1501",
                        "expected integer, boolean, string, none, optional_string or string_list parameter type",
                        parameter.span,
                    ));
                }
            };
            Ok((parameter.name.clone(), ty))
        })
        .collect()
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
    if declarations.len() > MAX_FUNCTIONS {
        return Err(invalid("LSH1501", "function count exceeds limit", span));
    }
    let mut declared = BTreeMap::new();
    let mut functions = FunctionTemplates::default();
    for function in declarations {
        if !computation::valid_local(&function.name)
            || computation::is_builtin(&function.name)
            || declared.insert(function.name.clone(), function).is_some()
        {
            return Err(invalid(
                "LSH1501",
                "function names must be bounded, unique and not builtins",
                function.span,
            ));
        }
        parameter_types(function)?;
        functions.names.extend(
            function
                .parameters
                .iter()
                .map(|parameter| parameter.name.clone()),
        );
        let mut pending = vec![(&function.body, 0usize)];
        let mut visited = 0;
        while let Some((expression, depth)) = pending.pop() {
            visited += 1;
            if visited > MAX_COMPUTATION_NODES || depth > MAX_EFFECT_NESTING_DEPTH {
                return Err(invalid(
                    "LSH1405",
                    "function source exceeds computation bounds",
                    expression_span(expression),
                ));
            }
            match expression {
                Expression::Call {
                    callee, arguments, ..
                } => {
                    if arguments
                        .len()
                        .saturating_add(visited)
                        .saturating_add(pending.len())
                        > MAX_COMPUTATION_NODES
                    {
                        return Err(invalid(
                            "LSH1405",
                            "function source exceeds computation bounds",
                            expression_span(expression),
                        ));
                    }
                    functions
                        .names
                        .extend(arguments.iter().map(|argument| argument.name.clone()));
                    // Fold declares its item with a quoted name, even when never referenced.
                    if callee == "fold"
                        && let Some(Expression::String { value, .. }) = arguments
                            .iter()
                            .find(|argument| argument.name == "item")
                            .map(|argument| &argument.value)
                    {
                        functions.names.insert(value.clone());
                    }
                    pending.extend(
                        arguments
                            .iter()
                            .map(|argument| (&argument.value, depth + 1)),
                    );
                }
                Expression::Reference { name, .. } => {
                    functions.names.insert(name.clone());
                }
                _ => {}
            }
        }
    }
    let main = declared.remove("main").ok_or_else(|| {
        invalid(
            "LSH1501",
            "a multi-function program requires exactly one main",
            span,
        )
    })?;
    if !main.parameters.is_empty() {
        return Err(invalid("LSH1501", "main cannot have parameters", main.span));
    }
    let mut dependencies = BTreeMap::new();
    for (name, function) in &declared {
        let mut required = BTreeSet::new();
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            if let Expression::Call {
                callee,
                arguments,
                span,
            } = expression
            {
                if callee == "main" {
                    return Err(invalid("LSH1502", "helpers cannot call main", *span));
                }
                if declared.contains_key(callee) {
                    required.insert(callee.clone());
                }
                pending.extend(arguments.iter().map(|argument| &argument.value));
            }
        }
        dependencies.insert(name.clone(), required);
    }
    // Compile the complete acyclic declaration graph, including unused helpers.
    while functions.templates.len() < declared.len() {
        let name = dependencies
            .iter()
            .find(|(name, required)| {
                !functions.contains(name)
                    && required
                        .iter()
                        .all(|dependency| functions.contains(dependency))
            })
            .map(|(name, _)| name.clone())
            .ok_or_else(|| {
                invalid(
                    "LSH1502",
                    "recursive helper functions are not supported",
                    span,
                )
            })?;
        let function = declared[&name];
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
        let (nodes, depth) = shape(&body);
        functions.templates.insert(
            name,
            Template {
                parameters,
                body,
                result_type,
                nodes,
                depth,
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

pub(super) fn shape(expression: &Computation) -> (usize, usize) {
    let mut nodes = 0;
    let mut depth = 0;
    let mut pending = vec![(expression, 0)];
    while let Some((expression, level)) = pending.pop() {
        let (literal_nodes, literal_depth) = match expression {
            Computation::Literal { value } => computation::source_shape_extra(value),
            _ => (0, 0),
        };
        nodes += 1 + literal_nodes;
        depth = depth.max(level + literal_depth);
        if let Computation::Host { effect } = expression {
            let mut effects = vec![(effect.as_ref(), level)];
            while let Some((effect, effect_depth)) = effects.pop() {
                nodes += 1;
                depth = depth.max(effect_depth + 1);
                if let Effect::Sequence { steps } | Effect::All { branches: steps } = effect {
                    effects.extend(steps.iter().map(|step| (&step.effect, effect_depth + 1)));
                }
            }
        }
        pending.extend(expression.children().map(|child| (child, level + 1)));
    }
    (nodes, depth)
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
    let parameters = template.parameters.clone();
    if arguments.len() != parameters.len()
        || parameters.iter().any(|(name, _)| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(invalid(
            "LSH1504",
            "helper arguments must match its named parameters exactly",
            span,
        ));
    }
    let mut values = Vec::with_capacity(parameters.len());
    for (name, ty) in &parameters {
        let argument = arguments
            .iter()
            .find(|argument| argument.name == *name)
            .ok_or_else(|| invalid("LSH1504", "missing helper argument", span))?;
        let (value, value_type) = computation::lower_expression_with_functions(
            &argument.value,
            scope,
            visited,
            depth + 1,
            functions,
        )?;
        if !value.is_pure() || value_type != Type::Scalar(*ty) {
            return Err(invalid(
                "LSH1504",
                "helper arguments must be pure scalars of the declared type",
                argument.span,
            ));
        }
        values.push(value);
    }
    let template = &functions.templates[name];
    if visited
        .saturating_add(template.nodes)
        .saturating_add(parameters.len())
        > MAX_COMPUTATION_NODES
        || depth
            .saturating_add(template.depth)
            .saturating_add(parameters.len())
            > MAX_EFFECT_NESTING_DEPTH
        || values.iter().enumerate().any(|(position, value)| {
            depth
                .saturating_add(position + 1)
                .saturating_add(shape(value).1)
                > MAX_EFFECT_NESTING_DEPTH
        })
    {
        return Err(invalid(
            "LSH1405",
            "expanded helper exceeds computation bounds",
            span,
        ));
    }
    *visited += template.nodes + parameters.len();
    let mut body = template.body.clone();
    let result_type = template.result_type;
    // Arguments stay in caller scope; parameter and body locals get fresh names.
    let mut renames = BTreeMap::new();
    for (name, _) in &parameters {
        renames.insert(name.clone(), functions.fresh_name());
    }
    rename(&mut body, &mut renames, functions)?;
    for ((name, _), value) in parameters.into_iter().zip(values).rev() {
        body = Computation::Bind {
            name: renames[&name].clone(),
            value: Box::new(value),
            body: Box::new(body),
        };
    }
    Ok((body, result_type))
}

fn rename(
    expression: &mut Computation,
    renames: &mut BTreeMap<String, String>,
    functions: &mut FunctionTemplates,
) -> Result<(), Vec<Diagnostic>> {
    match expression {
        Computation::Literal { .. } => {}
        Computation::Strings { items } => {
            for item in items {
                rename(item, renames, functions)?;
            }
        }
        Computation::Local { name } => {
            *name = renames
                .get(name)
                .ok_or_else(|| {
                    invalid(
                        "LSH1503",
                        "helper cannot capture caller locals",
                        Span { start: 0, end: 0 },
                    )
                })?
                .clone();
        }
        Computation::Bind { name, value, body } => {
            rename(value, renames, functions)?;
            let fresh = functions.fresh_name();
            let previous = renames.insert(name.clone(), fresh.clone());
            let original = std::mem::replace(name, fresh);
            rename(body, renames, functions)?;
            if let Some(previous) = previous {
                renames.insert(original, previous);
            } else {
                renames.remove(&original);
            }
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            ..
        } => {
            rename(initial, renames, functions)?;
            let fresh = functions.fresh_name();
            let previous = renames.insert(name.clone(), fresh.clone());
            let original = std::mem::replace(name, fresh);
            rename(condition, renames, functions)?;
            rename(next, renames, functions)?;
            if let Some(previous) = previous {
                renames.insert(original, previous);
            } else {
                renames.remove(&original);
            }
        }
        Computation::Fold {
            name,
            item,
            items,
            initial,
            next,
            ..
        } => {
            rename(items, renames, functions)?;
            rename(initial, renames, functions)?;
            let state_fresh = functions.fresh_name();
            let item_fresh = functions.fresh_name();
            let state_previous = renames.insert(name.clone(), state_fresh.clone());
            let item_previous = renames.insert(item.clone(), item_fresh.clone());
            let state_original = std::mem::replace(name, state_fresh);
            let item_original = std::mem::replace(item, item_fresh);
            rename(next, renames, functions)?;
            for (original, previous) in [
                (state_original, state_previous),
                (item_original, item_previous),
            ] {
                if let Some(previous) = previous {
                    renames.insert(original, previous);
                } else {
                    renames.remove(&original);
                }
            }
        }
        Computation::Binary { left, right, .. } => {
            rename(left, renames, functions)?;
            rename(right, renames, functions)?;
        }
        Computation::Unary { value, .. } => rename(value, renames, functions)?,
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            rename(when, renames, functions)?;
            rename(then, renames, functions)?;
            rename(otherwise, renames, functions)?;
        }
        Computation::Recover { value, fallback } => {
            rename(value, renames, functions)?;
            rename(fallback, renames, functions)?;
        }
        Computation::Field { value, .. } => rename(value, renames, functions)?,
        Computation::Member { group, .. } => {
            *group = renames
                .get(group)
                .ok_or_else(|| {
                    invalid(
                        "LSH1503",
                        "helper cannot capture a caller group",
                        Span { start: 0, end: 0 },
                    )
                })?
                .clone();
        }
        Computation::Host { .. } => {}
        Computation::Call { arguments, .. } => {
            for argument in arguments {
                rename(&mut argument.value, renames, functions)?;
            }
        }
        Computation::Group { branches, .. } => {
            for branch in branches {
                rename(&mut branch.value, renames, functions)?;
            }
        }
    }
    Ok(())
}
