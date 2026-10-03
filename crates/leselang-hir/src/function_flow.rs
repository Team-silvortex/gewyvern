use crate::computation::{self, Computation, MAX_COMPUTATION_NODES};
use crate::{Diagnostic, MAX_EFFECT_NESTING_DEPTH, functions};
use leselang_syntax::{Expression, MAX_SOURCE_BYTES, Span};

fn invalid(code: &str, message: &str, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: code.into(),
        message: message.into(),
        span: Some(span),
    }]
}

/// Accepts checked choices of direct helpers and pure data, not arbitrary effectful values.
pub(super) fn is_data_call_selection(
    source: &Expression,
    value: &Computation,
    functions: &functions::FunctionTemplates,
) -> bool {
    if value.validate_structure().is_err() {
        return false;
    }
    let mut pending = vec![(source, value)];
    let mut visited = 0usize;
    while let Some((source, value)) = pending.pop() {
        visited += 1;
        if visited > MAX_COMPUTATION_NODES {
            return false;
        }
        if value.is_pure() {
            continue;
        }
        let Expression::Call {
            callee, arguments, ..
        } = source
        else {
            return false;
        };
        if functions.contains(callee) {
            continue;
        }
        let Computation::Choose {
            when,
            then,
            otherwise,
        } = value
        else {
            return false;
        };
        if callee != "choose" || arguments.len() != 3 || !when.is_pure() {
            return false;
        }
        for (name, value) in [("then", then.as_ref()), ("otherwise", otherwise.as_ref())] {
            let Some(argument) = arguments.iter().find(|argument| argument.name == name) else {
                return false;
            };
            if visited + pending.len() >= MAX_COMPUTATION_NODES {
                return false;
            }
            pending.push((&argument.value, value));
        }
    }
    true
}

/// Connects a hygienically expanded function's normal returns to the caller.
/// Host failures are deliberately not converted into returns or recovery paths.
pub(super) fn bind_result(
    name: String,
    value: Computation,
    body: Computation,
    span: Span,
) -> Result<Computation, Vec<Diagnostic>> {
    let (value_nodes, mut depth) = functions::shape(&value);
    let (body_nodes, body_depth) = functions::shape(&body);
    let mut returns = 0usize;
    let mut pending = vec![(&value, 0usize)];
    while let Some((value, level)) = pending.pop() {
        if value.is_pure() {
            returns += 1;
            depth = depth.max(level + 1 + functions::shape(value).1.max(body_depth));
            continue;
        }
        match value {
            Computation::Bind { body, .. } => pending.push((body, level + 1)),
            Computation::Choose {
                then, otherwise, ..
            } => {
                pending.push((then, level + 1));
                pending.push((otherwise, level + 1));
            }
            Computation::Host { .. } | Computation::Call { .. } | Computation::Group { .. } => {
                returns += 1;
                depth = depth.max(level + 1 + functions::shape(value).1.max(body_depth));
            }
            _ => {
                return Err(invalid(
                    "LSH1503",
                    "helper return cannot compose at this effect boundary",
                    span,
                ));
            }
        }
    }
    // Reserve every cold return path before cloning its continuation.
    let nodes = value_nodes.saturating_add(returns.saturating_mul(body_nodes.saturating_add(1)));
    if nodes > MAX_COMPUTATION_NODES || depth > MAX_EFFECT_NESTING_DEPTH {
        return Err(invalid(
            "LSH1405",
            "composed helper exceeds computation bounds",
            span,
        ));
    }
    let bytes = computation::source(&value).len().saturating_add(
        returns.saturating_mul(
            computation::source(&body)
                .len()
                .saturating_add(name.len())
                .saturating_add(16),
        ),
    );
    if bytes > MAX_SOURCE_BYTES {
        return Err(invalid(
            "LSH1405",
            "composed helper exceeds canonical source bounds",
            span,
        ));
    }
    let expression = connect(value, &name, &body);
    expression
        .validate_structure()
        .map_err(|_| invalid("LSH1405", "invalid composed helper structure", span))?;
    Ok(expression)
}

fn connect(value: Computation, name: &str, body: &Computation) -> Computation {
    if value.is_pure() {
        return Computation::Bind {
            name: name.into(),
            value: Box::new(value),
            body: Box::new(body.clone()),
        };
    }
    match value {
        Computation::Bind {
            name: local,
            value,
            body: continuation,
        } => Computation::Bind {
            name: local,
            value,
            body: Box::new(connect(*continuation, name, body)),
        },
        Computation::Choose {
            when,
            then,
            otherwise,
        } => Computation::Choose {
            when,
            then: Box::new(connect(*then, name, body)),
            otherwise: Box::new(connect(*otherwise, name, body)),
        },
        value => Computation::Bind {
            name: name.into(),
            value: Box::new(value),
            body: Box::new(body.clone()),
        },
    }
}
