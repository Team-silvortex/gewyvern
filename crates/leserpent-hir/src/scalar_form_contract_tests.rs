//! Frozen product scalar root/counter boundary; shared kernels and child lowering.
//! This is a boundary oracle, not a second independent compiler.
use super::*;

fn frozen_scalar_root<'a>(
    expression: &'a Expression,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    *visited += 1;
    let span = expression_span(expression);
    if *visited > MAX_COMPUTATION_NODES || depth > MAX_EFFECT_NESTING_DEPTH {
        return Err(invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            span,
        ));
    }
    if crate::scalar_source::primitive(expression) {
        let (value, ty) = crate::scalar_source::lower_form(expression, |child| {
            let (value, ty) =
                lower_expression_with_functions(child, scope, visited, depth + 1, functions)
                    .map_err(|error| crate::scalar_source::ScalarSourceError::Native {
                        span: expression_span(child),
                        error,
                    })?;
            let pure = value.is_pure();
            Ok(crate::scalar_source::Operand {
                value,
                pure,
                scalar_type: match ty {
                    Type::Scalar(ty) => Some(ty),
                    _ => None,
                },
            })
        })
        .map_err(|error| scalar_source_diagnostics(error, span))?;
        return Ok((value, Type::Scalar(ty)));
    }
    unreachable!("the scalar boundary oracle only receives primitive roots")
}

#[test]
fn public_scalar_form_matches_frozen_product_output_diagnostics_counters_and_scope_for_3840_cases()
{
    let forms = [
        "add(right: b, left: a)",
        "sub(right: b, left: a)",
        "mul(right: b, left: a)",
        "div(right: b, left: a)",
        "rem(right: b, left: a)",
        "eq(right: b, left: a)",
        "ne(right: b, left: a)",
        "lt(right: b, left: a)",
        "le(right: b, left: a)",
        "gt(right: b, left: a)",
        "ge(right: b, left: a)",
        "and(right: b, left: a)",
        "or(right: b, left: a)",
        "concat(right: b, left: a)",
        "value_or(right: b, left: a)",
        "contains(right: b, left: a)",
        "starts_with(right: b, left: a)",
        "ends_with(right: b, left: a)",
        "char_at(right: b, left: a)",
        "split(right: b, left: a)",
        "join(right: b, left: a)",
        "append(right: b, left: a)",
        "item_at(right: b, left: a)",
        "not(value: a)",
        "len(value: a)",
        "to_string(value: a)",
        "parse_integer(value: a)",
        "parse_boolean(value: a)",
        "optional_string(value: a)",
        "has_value(value: a)",
        "0",
        "true",
        "false",
        "none",
        "\"literal\"",
        "strings()",
        "strings(first: \"first\", second: \"second\")",
        "strings(first: a, second: b)",
        "strings(first: a, first: b)",
        "strings(first: 1, second: missing)",
        "strings(first: strings())",
        "strings(v0: a, v1: a, v2: a, v3: a, v4: a, v5: a, v6: a, v7: a, v8: a, v9: a, v10: a, v11: a, v12: a, v13: a, v14: a, v15: a, v16: a, v17: a, v18: a, v19: a, v20: a, v21: a, v22: a, v23: a, v24: a, v25: a, v26: a, v27: a, v28: a, v29: a, v30: a, v31: a, v32: a, v33: a, v34: a, v35: a, v36: a, v37: a, v38: a, v39: a, v40: a, v41: a, v42: a, v43: a, v44: a, v45: a, v46: a, v47: a, v48: a, v49: a, v50: a, v51: a, v52: a, v53: a, v54: a, v55: a, v56: a, v57: a, v58: a, v59: a, v60: a, v61: a, v62: a, v63: a, v64: a)",
        "recover(value: a, fallback: b)",
        "recover(value: a)",
        "recover(value: 0, fallback: \"wrong\")",
        "recover(value: div(left: 1, right: 0), fallback: 2)",
        "recover(value: missing, fallback: missing2)",
        "recover(value: a, value: b)",
        "add()",
        "add(left: a)",
        "add(left: a, right: b, extra: 0)",
        "add(left: a, left: b)",
        "len()",
        "len(value: a, extra: b)",
        "and(left: false, right: missing)",
        "or(left: true, right: missing)",
        "value_or(left: optional_string(value: \"x\"), right: missing)",
        "optional_string(value: 1)",
        "join(left: strings(a: \"x\"), right: \",\")",
        "append(left: strings(), right: \"x\")",
        "add(left: bind(n: 1, body: n), right: a)",
        "add(left: loop(n: 0, while: false, next: n, limit: 0), right: a)",
        "add(left: fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 0), right: a)",
        "\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"",
    ];
    let mut cases = 0;
    for text in forms {
        let parsed = parse(&format!("fn main() = {text}"));
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let expression = parsed.function.unwrap().body;
        for prefix in 0..5 {
            for starting in [
                0,
                MAX_COMPUTATION_NODES - 2,
                MAX_COMPUTATION_NODES,
                MAX_COMPUTATION_NODES + 1,
            ] {
                for depth in [0, MAX_EFFECT_NESTING_DEPTH - 1, MAX_EFFECT_NESTING_DEPTH] {
                    let run = |frozen| {
                        let pairs = match prefix {
                            0 => vec![],
                            1 => vec![
                                ("a", Type::Scalar(ScalarType::Integer)),
                                ("b", Type::Scalar(ScalarType::Integer)),
                            ],
                            2 => vec![
                                ("a", Type::Scalar(ScalarType::String)),
                                ("b", Type::Scalar(ScalarType::String)),
                            ],
                            3 => vec![
                                ("a", Type::Scalar(ScalarType::Boolean)),
                                ("b", Type::Structured),
                            ],
                            _ => vec![
                                ("a", Type::Scalar(ScalarType::OptionalString)),
                                ("b", Type::Scalar(ScalarType::StringList)),
                            ],
                        };
                        let mut bindings = pairs
                            .into_iter()
                            .map(|(name, ty)| (name, ty.into()))
                            .collect();
                        let mut scope = ScopeFrame::new(&mut bindings);
                        let mut visited = starting;
                        let mut functions = functions::FunctionTemplates::default();
                        let result = if frozen {
                            frozen_scalar_root(
                                &expression,
                                &mut scope,
                                &mut visited,
                                depth,
                                &mut functions,
                            )
                        } else {
                            lower_expression_with_functions(
                                &expression,
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
                        "prefix={prefix}, visited={starting}, depth={depth}, source={text}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 3840);
}
