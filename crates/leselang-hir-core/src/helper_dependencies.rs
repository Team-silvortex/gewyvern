//! Borrowed helper dependency planning, not lowering, bytecode or execution.

use std::{fmt, iter::FusedIterator};

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, Function, Span};

use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, valid_local_name};

pub const MAX_DEPENDENCY_HELPERS: usize = 31;

/// Inclusive helper count and per-helper physical AST bounds. No default policy,
/// total ingress-byte quota, source expansion or interpreter fuel is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperDependencyLimits {
    pub max_helpers: usize,
    pub max_source_nodes: usize,
    pub max_source_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperDependencyError {
    InvalidLimits,
    HelperLimit,
    InvalidEntry,
    InvalidName {
        declaration_index: usize,
        span: Span,
    },
    DuplicateName {
        declaration_index: usize,
        span: Span,
    },
    EntryConflict {
        declaration_index: usize,
        span: Span,
    },
    Source {
        declaration_index: usize,
        span: Span,
        error: StructureError,
    },
    EntryCall {
        declaration_index: usize,
        span: Span,
    },
    Cycle {
        remaining: usize,
    },
}
impl fmt::Display for HelperDependencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper dependency limits exceed safety ceilings",
            Self::HelperLimit => "helper declaration count exceeds its limit",
            Self::InvalidEntry => "helper entry name is invalid",
            Self::InvalidName { .. } => "helper declaration name is invalid",
            Self::DuplicateName { .. } => "helper declaration name is duplicated",
            Self::EntryConflict { .. } => "helper declaration conflicts with the entry name",
            Self::Source { .. } => "helper dependency source exceeds physical bounds",
            Self::EntryCall { .. } => "helpers cannot call the designated entry",
            Self::Cycle { .. } => "helper declarations contain a dependency cycle",
        })
    }
}
impl std::error::Error for HelperDependencyError {}

fn preflight_source(
    function: &Function,
    declaration_index: usize,
    limits: HelperDependencyLimits,
) -> Result<(), HelperDependencyError> {
    let mut budget = StructureBudget::new(limits.max_source_nodes, limits.max_source_depth);
    let mut pending = vec![(&function.body, 0)];
    while let Some((expression, depth)) = pending.pop() {
        let fail = |error| HelperDependencyError::Source {
            declaration_index,
            span: crate::source_call::span(expression),
            error,
        };
        budget.visit(depth, 0, 0).map_err(fail)?;
        if let Expression::Call { arguments, .. } = expression {
            budget
                .check_pending(pending.len(), arguments.len())
                .map_err(fail)?;
            pending.extend(
                arguments
                    .iter()
                    .map(|argument| (&argument.value, depth + 1)),
            );
        }
    }
    Ok(())
}

/// Ephemeral dependency order over exact borrowed declarations. Each successful
/// next selects the lexically smallest ready name and marks it planned, not
/// compiled, dispatched or accepted. Discard the cursor on lowering failure;
/// advancing it is not permission to skip a failed helper or resume execution.
/// A cycle is reported once only after ready declarations have been yielded,
/// then the iterator is fused. Early abandonment does not validate the graph.
/// Debug prints counts only, never source names, parameters, bodies or literals.
#[must_use = "exhaust dependency planning or discard it after a lowering failure"]
pub struct HelperDependencyOrder<'source> {
    helpers: Vec<(usize, &'source Function)>,
    dependencies: [u32; MAX_DEPENDENCY_HELPERS],
    planned: u32,
    terminal: bool,
}
impl fmt::Debug for HelperDependencyOrder<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperDependencyOrder")
            .field("helpers", &self.helpers.len())
            .field("planned", &self.planned.count_ones())
            .field("terminal", &self.terminal)
            .finish()
    }
}
impl<'source> Iterator for HelperDependencyOrder<'source> {
    type Item = Result<&'source Function, HelperDependencyError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.terminal {
            return None;
        }
        if self.planned.count_ones() as usize == self.helpers.len() {
            self.terminal = true;
            return None;
        }
        for (index, (_, function)) in self.helpers.iter().enumerate() {
            let bit = 1u32 << index;
            if self.planned & bit == 0 && self.dependencies[index] & !self.planned == 0 {
                self.planned |= bit;
                return Some(Ok(*function));
            }
        }
        self.terminal = true;
        Some(Err(HelperDependencyError::Cycle {
            remaining: self.helpers.len() - self.planned.count_ones() as usize,
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.terminal {
            0
        } else {
            self.helpers.len() - self.planned.count_ones() as usize
        };
        (usize::from(remaining != 0), Some(remaining))
    }
}
impl FusedIterator for HelperDependencyOrder<'_> {}

/// Plan all helpers, including unused and cold calls. The designated entry is
/// caller-owned: it need not be `main`, and its declaration/body is not validated
/// here. Entry/helper names must be bounded local identifiers and disjoint;
/// duplicate indices refer to the original supplied helper slice, never sorted
/// positions. Native/builtin reserved-name policy and scalar signatures stay in
/// the caller. Unknown calls do not become helper dependencies or valid operations.
///
/// Fixed ceilings are 31 helpers, 16,384 physical nodes per helper and depth 64.
/// Zero helpers permits an empty graph; zero nodes denies nonempty helper bodies;
/// zero depth permits leaves. All physical trees precede dependency scanning.
/// This does not bound literal bytes, check operand names/types or certify source
/// spans. The caller bounds ingress, validates source/signatures and rejects every
/// unknown/denied operation before dispatch. Native work is never invoked here.
///
/// Dependency scanning uses lexical helper order and right-to-left source children,
/// retaining the reference entry-call diagnostic precedence. Every physical call
/// counts, including calls hidden inside native forms, choices, recovery and zero
/// loops/folds. Repeated dependency edges share one bounded bit, but source nodes
/// are never deduplicated. Names/ASTs are borrowed, with no clones, recursive graph
/// walk, second tree, native callbacks, bytecode, template cache or global state.
pub fn helper_dependency_order<'source>(
    helpers: &[&'source Function],
    entry: &str,
    limits: HelperDependencyLimits,
) -> Result<HelperDependencyOrder<'source>, HelperDependencyError> {
    if limits.max_helpers > MAX_DEPENDENCY_HELPERS
        || limits.max_source_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_source_depth > MAX_TYPE_INFERENCE_DEPTH
    {
        return Err(HelperDependencyError::InvalidLimits);
    }
    if helpers.len() > limits.max_helpers {
        return Err(HelperDependencyError::HelperLimit);
    }
    if !valid_local_name(entry) {
        return Err(HelperDependencyError::InvalidEntry);
    }
    for (declaration_index, function) in helpers.iter().enumerate() {
        let span = function.span;
        if !valid_local_name(&function.name) {
            return Err(HelperDependencyError::InvalidName {
                declaration_index,
                span,
            });
        }
        if function.name == entry {
            return Err(HelperDependencyError::EntryConflict {
                declaration_index,
                span,
            });
        }
        if helpers[..declaration_index]
            .iter()
            .any(|previous| previous.name == function.name)
        {
            return Err(HelperDependencyError::DuplicateName {
                declaration_index,
                span,
            });
        }
    }
    for (index, function) in helpers.iter().enumerate() {
        preflight_source(function, index, limits)?;
    }
    let mut helpers = helpers.iter().copied().enumerate().collect::<Vec<_>>();
    helpers.sort_unstable_by(|(_, left), (_, right)| left.name.cmp(&right.name));
    let mut dependencies = [0; MAX_DEPENDENCY_HELPERS];
    for (index, (declaration_index, function)) in helpers.iter().enumerate() {
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            if let Expression::Call {
                callee,
                arguments,
                span,
            } = expression
            {
                if callee == entry {
                    return Err(HelperDependencyError::EntryCall {
                        declaration_index: *declaration_index,
                        span: *span,
                    });
                }
                if let Ok(dependency) = helpers
                    .binary_search_by(|(_, function)| function.name.as_str().cmp(callee.as_str()))
                {
                    dependencies[index] |= 1u32 << dependency;
                }
                pending.extend(arguments.iter().map(|argument| &argument.value));
            }
        }
    }
    Ok(HelperDependencyOrder {
        helpers,
        dependencies,
        planned: 0,
        terminal: false,
    })
}
