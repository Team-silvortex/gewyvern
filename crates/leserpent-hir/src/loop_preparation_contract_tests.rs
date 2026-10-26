//! Frozen product loop coordinator; scalar kernels and child lowering are shared.
//! This checks the replaced boundary, not a second independent compiler.
use super::*;

fn frozen_lower_loop<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::scalar_source::{Operand, ScalarSourceError};
    let source =
        crate::scalar_source::loop_source(arguments, span).map_err(|error| match error {
            ScalarSourceError::LoopShape {
                span,
                missing_state: true,
            } => invalid("LSH1410", "loop requires a named initial state", span),
            ScalarSourceError::LoopShape {
                span,
                missing_state: false,
            } => {
                let name = arguments
                    .iter()
                    .find(|argument| !["while", "next", "limit"].contains(&argument.name.as_str()))
                    .map(|argument| argument.name.as_str())
                    .unwrap_or("");
                invalid(
                    "LSH1401",
                    format!("expected exactly the named arguments: {name}, while, next, limit"),
                    span,
                )
            }
            ScalarSourceError::Bindings { span, .. } => invalid(
                "LSH1403",
                "loop state name must be bounded and cannot shadow an active binding",
                span,
            ),
            _ => scalar_source_diagnostics(error, span),
        })?;
    let binding = source.binding;
    if scope.get(&binding.name).is_some() {
        return Err(invalid(
            "LSH1403",
            "loop state name must be bounded and cannot shadow an active binding",
            binding.span,
        ));
    }
    let limit = source
        .limit()
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (initial, state_type) =
        lower_expression_with_functions(&binding.value, scope, visited, depth + 1, functions)?;
    let (initial, scalar_type) = crate::scalar_source::scalar_operand(
        Operand {
            pure: initial.is_pure(),
            value: initial,
            scalar_type: match state_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(&binding.value),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let body = {
        let mut local = scope.nested();
        local.push(&binding.name, state_type.into()).map_err(|_| {
            invalid(
                "LSH1403",
                "loop state name must be bounded and cannot shadow an active binding",
                binding.span,
            )
        })?;
        crate::scalar_source::lower_loop_body(&source, scalar_type, |child| {
            let (value, ty) =
                lower_expression_with_functions(child, &mut local, visited, depth + 1, functions)
                    .map_err(|error| ScalarSourceError::Native {
                    span: expression_span(child),
                    error,
                })?;
            Ok(Operand {
                pure: value.is_pure(),
                value,
                scalar_type: match ty {
                    Type::Scalar(ty) => Some(ty),
                    _ => None,
                },
            })
        })
        .map_err(|error| scalar_source_diagnostics(error, span))?
    };
    Ok((source.construct(initial, body, limit), state_type))
}

#[test]
fn public_loop_stages_match_frozen_product_diagnostics_counters_and_scope_for_1152_cases() {
    let forms = [
        "loop(state: 0, while: lt(left: state, right: 2), next: add(left: state, right: 1), limit: 2)",
        "loop(next: state, limit: 1, while: false, state: 0)",
        "loop(state: 0, while: false, next: missing, limit: 0)",
        "loop(state: 0, while: 1, next: state, limit: 0)",
        "loop(state: none, while: false, next: none, limit: 0)",
        "loop(state: \"old\", while: false, next: \"new\", limit: 1)",
        "loop(state: true, while: state, next: false, limit: 1)",
        "loop(state: strings(a: \"a\"), while: false, next: state, limit: 1)",
        "loop(state: optional_string(value: none), while: false, next: state, limit: 1)",
        "loop(state: 0, while: false, next: bind(state: 1, body: state), limit: 1)",
        "loop(state: 0, while: false, next: loop(k: state, while: false, next: k, limit: 1), limit: 1)",
        "loop(state: 0, while: false, next: loop(state: 0, while: false, next: state, limit: 1), limit: 1)",
        "loop(while: false, next: 0, limit: 1)",
        "loop(state: 0, next: state, limit: 1)",
        "loop(state: 0, while: false, while: true, next: state, limit: 1)",
        "loop(state: 0, while: false, next: state, limit: missing)",
        "loop(state: 0, while: false, next: state, limit: 1025)",
        "loop(state: 0, while: false, next: state, limit: true)",
        "loop(state: missing, while: false, next: state, limit: 1)",
        "loop(state: 0, while: missing, next: state, limit: 1)",
        "loop(state: refresh(), while: false, next: state, limit: 1)",
        "loop(state: 0, while: false, next: true, limit: 1)",
        "loop(state: 0, while: refresh(), next: state, limit: 1)",
        "loop(state: state, while: false, next: state, limit: 1)",
    ];
    let mut cases = 0;
    for text in forms {
        let parsed = parse(&format!("fn main() = {text}"));
        assert!(
            parsed.diagnostics.is_empty(),
            "{text}: {:?}",
            parsed.diagnostics
        );
        let Expression::Call {
            arguments, span, ..
        } = parsed.function.unwrap().body
        else {
            panic!()
        };
        for prefix in 0..4 {
            for starting in [
                0,
                MAX_COMPUTATION_NODES - 2,
                MAX_COMPUTATION_NODES,
                MAX_COMPUTATION_NODES + 1,
            ] {
                for depth in [0, MAX_EFFECT_NESTING_DEPTH - 1, MAX_EFFECT_NESTING_DEPTH] {
                    let run = |frozen| {
                        let mut bindings = match prefix {
                            0 => vec![],
                            1 => vec![("state", Type::Scalar(ScalarType::Integer).into())],
                            2 => vec![("prefix", Type::Scalar(ScalarType::Boolean).into())],
                            _ => vec![("reply", Type::Structured.into())],
                        };
                        let mut scope = ScopeFrame::new(&mut bindings);
                        let mut visited = starting;
                        let mut functions = functions::FunctionTemplates::default();
                        let result = if frozen {
                            frozen_lower_loop(
                                &arguments,
                                span,
                                &mut scope,
                                &mut visited,
                                depth,
                                &mut functions,
                            )
                        } else {
                            lower_loop(
                                &arguments,
                                span,
                                &mut scope,
                                &mut visited,
                                depth,
                                &mut functions,
                            )
                        };
                        let remaining = scope
                            .bindings()
                            .iter()
                            .map(|(name, local)| (*name, local.ty))
                            .collect::<Vec<_>>();
                        (result, visited, remaining)
                    };
                    assert_eq!(
                        run(false),
                        run(true),
                        "{text}, prefix={prefix}, visited={starting}, depth={depth}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 1152);
}
