//! Frozen product fold coordinator; scalar kernels and child lowering are shared.
//! This checks the replaced boundary, not a second independent compiler.
use super::*;

fn frozen_lower_fold<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::scalar_source::{Operand, ScalarSourceError};
    let source =
        crate::scalar_source::fold_source(arguments, span).map_err(|error| match error {
            ScalarSourceError::FoldShape {
                span,
                missing_state: true,
            } => invalid("LSH1413", "fold requires a named initial state", span),
            ScalarSourceError::FoldShape {
                span,
                missing_state: false,
            } => {
                let name = arguments
                    .iter()
                    .find(|argument| {
                        !["items", "item", "next", "limit"].contains(&argument.name.as_str())
                    })
                    .map(|argument| argument.name.as_str())
                    .unwrap_or("");
                invalid(
                    "LSH1401",
                    format!(
                        "expected exactly the named arguments: {name}, items, item, next, limit"
                    ),
                    span,
                )
            }
            _ => scalar_source_diagnostics(error, span),
        })?;
    source
        .check_scope(scope, None)
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    let limit = source
        .limit()
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    // The collection is prepared before the accumulator, independent of argument spelling order.
    let (items, items_type) =
        lower_expression_with_functions(source.items, scope, visited, depth + 1, functions)?;
    let items = crate::scalar_source::fold_items(
        Operand {
            pure: items.is_pure(),
            value: items,
            scalar_type: match items_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(source.items),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (initial, state_type) = lower_expression_with_functions(
        &source.binding.value,
        scope,
        visited,
        depth + 1,
        functions,
    )?;
    let (initial, scalar_type) = crate::scalar_source::scalar_operand(
        Operand {
            pure: initial.is_pure(),
            value: initial,
            scalar_type: match state_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(&source.binding.value),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (next, next_type) = {
        let mut local = scope.nested();
        for (name, ty) in [
            (source.binding.name.as_str(), state_type),
            (source.item, Type::Scalar(ScalarType::String)),
        ] {
            local.push(name, ty.into()).map_err(|_| {
                scalar_source_diagnostics(
                    source.binding_error(crate::pure_typing::PureTypeError::ShadowedBinding),
                    span,
                )
            })?;
        }
        lower_expression_with_functions(source.next, &mut local, visited, depth + 1, functions)?
    };
    let next = crate::scalar_source::fold_next(
        Operand {
            pure: next.is_pure(),
            value: next,
            scalar_type: match next_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        scalar_type,
        expression_span(source.next),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    Ok((source.construct(items, initial, next, limit), state_type))
}

#[test]
fn public_fold_stages_match_frozen_product_diagnostics_counters_and_scope_for_1920_cases() {
    let forms = [
        "fold(total: 0, items: strings(a: \"a\"), item: \"entry\", next: add(left: total, right: 1), limit: 1)",
        "fold(next: total, limit: 0, total: 0, item: \"entry\", items: strings())",
        "fold(total: 0, items: strings(), item: \"entry\", next: missing, limit: 0)",
        "fold(total: 0, items: strings(), item: \"entry\", next: true, limit: 0)",
        "fold(total: none, items: strings(), item: \"entry\", next: none, limit: 0)",
        "fold(total: \"\", items: strings(a: \"a\"), item: \"entry\", next: entry, limit: 1)",
        "fold(total: true, items: strings(), item: \"entry\", next: false, limit: 1)",
        "fold(total: strings(a: \"a\"), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: optional_string(value: none), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(items: strings(), item: \"entry\", next: 0, limit: 0)",
        "fold(total: 0, item: \"entry\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), items: strings(), item: \"entry\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: 1, next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"total\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"bad name\", next: total, limit: 0)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: missing)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: 65)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: true)",
        "fold(total: missing, items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: 1, item: \"entry\", next: total, limit: 1)",
        "fold(total: refresh(), items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: refresh(), limit: 1)",
        "fold(total: 0, items: entry, item: \"entry\", next: total, limit: 1)",
        "fold(total: total, items: strings(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: bind(entry: 1, body: entry), limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: fold(total: 0, items: strings(), item: \"other\", next: total, limit: 1), limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: fold(n: total, items: strings(), item: \"other\", next: n, limit: 1), limit: 1)",
        "fold(total: 0, items: missing, item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: refresh(), item: \"entry\", next: total, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: 64)",
        "fold(total: 0, items: strings(), item: \"entry\", next: snapshot, limit: 1)",
        "fold(total: 0, items: strings(), item: \"entry\", next: loop(n: total, while: false, next: n, limit: 0), limit: 1)",
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
        for prefix in 0..5 {
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
                            1 => vec![("total", Type::Scalar(ScalarType::Integer).into())],
                            2 => vec![("entry", Type::Scalar(ScalarType::String).into())],
                            3 => vec![("prefix", Type::Scalar(ScalarType::Boolean).into())],
                            _ => vec![("snapshot", Type::Structured.into())],
                        };
                        let mut scope = ScopeFrame::new(&mut bindings);
                        let mut visited = starting;
                        let mut functions = functions::FunctionTemplates::default();
                        let result = if frozen {
                            frozen_lower_fold(
                                &arguments,
                                span,
                                &mut scope,
                                &mut visited,
                                depth,
                                &mut functions,
                            )
                        } else {
                            lower_fold(
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
    assert_eq!(cases, 1920);
}
