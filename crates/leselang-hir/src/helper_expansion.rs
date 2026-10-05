//! Checked source-expansion reservation, not IR shape, execution fuel or authority.

use std::fmt;

use leselang_syntax::MAX_FUNCTION_PARAMETERS;

use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};

/// Caller-observed source-expanded body cost, with a root at relative depth zero.
/// Folded constructors and closed opaque host graphs retain their source weights.
/// This is not HelperTemplateShape: observing physical IR alone may undercount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperExpansionCost {
    pub nodes: usize,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperExpansionLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_parameters: usize,
}

pub enum HelperExpansionError<Error> {
    InvalidLimits,
    ParameterLimit,
    EmptyBody,
    NodeLimit,
    BodyDepthLimit,
    ArgumentDepthLimit {
        parameter_index: usize,
    },
    Observation {
        parameter_index: usize,
        error: Error,
    },
}
impl<Error> fmt::Display for HelperExpansionError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper expansion limits exceed safety ceilings",
            Self::ParameterLimit => "helper expansion parameter count exceeds its limit",
            Self::EmptyBody => "helper expansion body cost must include its root",
            Self::NodeLimit => "helper expansion node limit exceeded",
            Self::BodyDepthLimit => "helper expansion body exceeds shifted depth",
            Self::ArgumentDepthLimit { .. } => "helper operand exceeds shifted depth",
            Self::Observation { .. } => "native helper depth observation failed",
        })
    }
}
impl<Error> fmt::Debug for HelperExpansionError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperExpansionError<Error> {}

/// Reserve body plus one Bind wrapper per parameter in a caller-owned source
/// counter. The original call root and all operand source costs have already been
/// charged; do not charge or refund them here. Limits are explicit and inclusive,
/// capped at 16,384 nodes, depth 64 and eight parameters. A body costs at least one
/// node; zero nodes denies every body, zero depth permits an unwrapped leaf, and
/// zero parameters invokes no depth observer. No allocation or graph is owned.
///
/// Check limits/count/body root, aggregate nodes, then shifted body depth before
/// invoking the observer. Observe each operand once in parameter declaration
/// order, stopping at its first error. Operand i starts at caller_depth + i + 1;
/// body root starts at caller_depth + parameter_count. All additions are checked.
/// Commit the counter only after every check succeeds, before caller-controlled
/// template materialization, hygiene and wrapper construction. Failure or unwind
/// leaves its original counter unchanged, including prior operand charges; there
/// is no callback retry, source/native-work refund or rollback of native effects.
/// A later downstream failure must not undo a successful expansion reservation.
///
/// Body cost and observed operand depth are trusted adapter observations, not
/// measurements or certificates. The caller bounds and completely validates cold
/// source/IR, preserves folded/native graph weights, exact argument alignment and
/// active prefixes, then checks types, canonical output and live authority. Native
/// observation work/allocation/unwind is not fuel-limited or sandboxed here.
/// No AST/IR cloning, registry selection, type/value queries, execution, dispatch,
/// journal, replay grant, global lock or default policy is installed. Native errors
/// are available by explicit matching only, never formatting or source chains.
pub fn reserve_helper_expansion<Error>(
    used_nodes: &mut usize,
    caller_depth: usize,
    body: HelperExpansionCost,
    parameter_count: usize,
    limits: HelperExpansionLimits,
    mut argument_depth: impl FnMut(usize) -> Result<usize, Error>,
) -> Result<(), HelperExpansionError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_parameters > MAX_FUNCTION_PARAMETERS
    {
        return Err(HelperExpansionError::InvalidLimits);
    }
    if parameter_count > limits.max_parameters {
        return Err(HelperExpansionError::ParameterLimit);
    }
    if body.nodes == 0 {
        return Err(HelperExpansionError::EmptyBody);
    }
    let total = used_nodes
        .checked_add(body.nodes)
        .and_then(|nodes| nodes.checked_add(parameter_count))
        .filter(|&nodes| nodes <= limits.max_nodes)
        .ok_or(HelperExpansionError::NodeLimit)?;
    caller_depth
        .checked_add(body.depth)
        .and_then(|depth| depth.checked_add(parameter_count))
        .filter(|&depth| depth <= limits.max_depth)
        .ok_or(HelperExpansionError::BodyDepthLimit)?;
    for parameter_index in 0..parameter_count {
        let depth =
            argument_depth(parameter_index).map_err(|error| HelperExpansionError::Observation {
                parameter_index,
                error,
            })?;
        caller_depth
            .checked_add(parameter_index + 1)
            .and_then(|offset| offset.checked_add(depth))
            .filter(|&depth| depth <= limits.max_depth)
            .ok_or(HelperExpansionError::ArgumentDepthLimit { parameter_index })?;
    }
    *used_nodes = total;
    Ok(())
}
