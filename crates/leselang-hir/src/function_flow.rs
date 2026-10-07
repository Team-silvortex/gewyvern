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
    let source_limits = crate::source_cost::SourceCostLimits {
        max_nodes: MAX_COMPUTATION_NODES,
        max_depth: MAX_EFFECT_NESTING_DEPTH,
    };
    let plan = crate::helper_join::HelperJoin::new(
        name,
        value,
        body,
        crate::helper_join::HelperJoinLimits {
            output: crate::helper_returns::HelperReturnLimits {
                max_nodes: MAX_COMPUTATION_NODES,
                max_depth: MAX_EFFECT_NESTING_DEPTH,
            },
            source: source_limits,
        },
        |effect| functions::host_source_extra(effect, source_limits),
    )
    .map_err(|error| {
        if matches!(
            error,
            crate::helper_join::HelperJoinError::Plan(
                crate::helper_returns::HelperReturnError::UnsupportedBoundary
            )
        ) {
            invalid(
                "LSH1503",
                "helper return cannot compose at this effect boundary",
                span,
            )
        } else {
            invalid(
                "LSH1405",
                "composed helper exceeds computation bounds",
                span,
            )
        }
    })?;
    let returns = plan.shape().returns;
    let bytes = computation::source(plan.value()).len().saturating_add(
        returns.saturating_mul(
            computation::source(plan.continuation())
                .len()
                .saturating_add(plan.name().len())
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
    plan.connect(
        |body, _| Ok(body.clone()),
        |expression, _| expression.validate_structure(),
    )
    .map_err(|_| invalid("LSH1405", "invalid composed helper structure", span))
}
