//! Owned helper parameter wrappers, not template cloning or type acceptance.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::MAX_FUNCTION_PARAMETERS;

use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureTypeError, preflight_with_budget,
    valid_local_name,
};

/// One fresh parameter alias and its original caller-scope operand. Bindings are
/// supplied in declaration order, not submitted argument order. Native IR slots
/// need no Clone/Debug/serde/Send implementation; values are consumed once.
pub struct HelperBinding<Node> {
    pub name: String,
    pub value: Node,
}

/// Inclusive bounds on the complete physical output, including all Bind roots,
/// shifted operands and the body. Source expansion and active scope quotas remain
/// separate. No default policy or extra execution fuel is granted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperBindingLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_parameters: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperBindingError {
    InvalidLimits,
    ParameterLimit,
    InvalidName { index: usize },
    DuplicateName { index: usize },
    Wrappers(StructureError),
    Argument { index: usize, error: PureTypeError },
    ArgumentCapture { index: usize },
    Body(StructureError),
    ShadowedParameter,
}
impl fmt::Display for HelperBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper binding limits exceed safety ceilings",
            Self::ParameterLimit => "helper binding count exceeds its limit",
            Self::InvalidName { .. } => "helper parameter alias is invalid",
            Self::DuplicateName { .. } => "helper parameter alias is duplicated",
            Self::Wrappers(_) => "helper wrapper output exceeds physical bounds",
            Self::Argument { .. } => "helper operand is not bounded pure IR",
            Self::ArgumentCapture { .. } => "helper alias conflicts with a caller operand name",
            Self::Body(_) => "helper body exceeds shifted physical bounds",
            Self::ShadowedParameter => "helper body shadows a parameter alias",
        })
    }
}
impl std::error::Error for HelperBindingError {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

fn conflicts<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    bindings: &[HelperBinding<Node<Field, Operation, HostEffect, IrResult>>],
    references: bool,
) -> bool {
    let reserved = |name: &str| bindings.iter().any(|binding| binding.name == name);
    match node {
        Node::Bind { name, .. } | Node::Loop { name, .. } => reserved(name),
        Node::Fold { name, item, .. } => reserved(name) || reserved(item),
        Node::Local { name } if references => reserved(name),
        Node::Member { group, .. } if references => reserved(group),
        _ => false,
    }
}

/// Wrap an already hygienic owned body in fresh scalar parameter bindings.
///
/// Fixed ceilings are 16,384 physical output nodes, depth 64 and eight parameters.
/// Zero parameters permits a body without wrappers; zero nodes rejects any body;
/// zero depth permits only an unwrapped leaf. Every cold operand is bounded and
/// physically pure before scanning its names, and the complete shifted body is
/// checked before constructing any wrapper. One aggregate meter charges exactly
/// one per physical node: wrapper i is at depth i, operand i starts at i + 1, and
/// the body starts at parameter count. No source-folding cost is refunded.
///
/// Aliases must be valid and unique and must avoid every lexical name in all
/// caller operands (including local declarations/group references), conservatively
/// preventing earlier parameters from capturing later operands. Cold body binders
/// may not shadow aliases. Body references to aliases are intentional; argument
/// references are never renamed or substituted. The caller also reserves the
/// entire live prefix and global namespace, including names unused by operands.
///
/// Original native slots, child Boxes and vector buffers move unchanged. Only
/// the new language-owned Bind roots/Boxes are constructed, in reverse wrapping
/// order so execution evaluates operands once in declaration order. No callbacks,
/// operand evaluation, name allocation, template clone, second IR, dispatch,
/// runtime prefix mutation or receipt is involved. Failure drops consumed inputs
/// once, without partial output, retry or source/native reservation rollback.
///
/// This is not scalar argument/result typing, complete lexical validation or a
/// program certificate. Callers prepare exact named/type-aligned operands, perform
/// body hygiene and complete cold inference against the exact prefix, and bound
/// source/pre-fold expansion, active bindings, native schemas and authority.
/// Body literals/operator/group metadata are not certified by the shape walk.
/// Opaque Host bodies must be closed; their graphs, ingress, work and Drop remain
/// adapter-owned. Physical output bounds do not validate native payloads or spans.
pub fn bind_helper_arguments<Field, Operation, HostEffect, IrResult>(
    mut body: Node<Field, Operation, HostEffect, IrResult>,
    bindings: Vec<HelperBinding<Node<Field, Operation, HostEffect, IrResult>>>,
    limits: HelperBindingLimits,
) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperBindingError> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_parameters > MAX_FUNCTION_PARAMETERS
    {
        return Err(HelperBindingError::InvalidLimits);
    }
    if bindings.len() > limits.max_parameters {
        return Err(HelperBindingError::ParameterLimit);
    }
    for (index, binding) in bindings.iter().enumerate() {
        if !valid_local_name(&binding.name) {
            return Err(HelperBindingError::InvalidName { index });
        }
        if bindings[..index]
            .iter()
            .any(|prior| prior.name == binding.name)
        {
            return Err(HelperBindingError::DuplicateName { index });
        }
    }
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    for index in 0..bindings.len() {
        budget
            .visit(index, 0, 0)
            .map_err(HelperBindingError::Wrappers)?;
    }
    budget
        .check_pending(0, bindings.len() + 1)
        .map_err(HelperBindingError::Wrappers)?;
    for (index, binding) in bindings.iter().enumerate() {
        budget
            .check_pending(0, bindings.len() - index + 1)
            .map_err(|error| HelperBindingError::Argument {
                index,
                error: PureTypeError::Structure(error),
            })?;
        preflight_with_budget(&binding.value, index + 1, &mut budget)
            .map_err(|error| HelperBindingError::Argument { index, error })?;
        let mut pending = vec![&binding.value];
        while let Some(node) = pending.pop() {
            if conflicts(node, &bindings, true) {
                return Err(HelperBindingError::ArgumentCapture { index });
            }
            pending.extend(node.children());
        }
    }
    let mut pending = vec![(&body, bindings.len())];
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(HelperBindingError::Body)?;
        if conflicts(node, &bindings, false) {
            return Err(HelperBindingError::ShadowedParameter);
        }
        for child in node.children().rev() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(HelperBindingError::Body)?;
            pending.push((child, depth + 1));
        }
    }
    for binding in bindings.into_iter().rev() {
        body = Node::Bind {
            name: binding.name,
            value: Box::new(binding.value),
            body: Box::new(body),
        };
    }
    Ok(body)
}
