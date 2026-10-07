//! Bounded normal-return composition with explicit native continuation copying.

use std::{collections::HashSet, convert::Infallible, fmt};

use leselang_runtime_core::{StructureBudget, StructureError};

use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, valid_local_name};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperReturnLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
}

/// Complete physical output, including one Bind and continuation per normal
/// return. Folded source weights, native graphs, bytes and active scope are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperReturnShape {
    pub nodes: usize,
    pub depth: usize,
    pub returns: usize,
}

pub enum HelperReturnError<Error> {
    InvalidLimits,
    InvalidName,
    InvalidLexicalName,
    Input {
        continuation: bool,
        error: StructureError,
    },
    UnsupportedBoundary,
    Capture,
    Output(StructureError),
    Factory {
        index: usize,
        error: Error,
    },
    ChangedContinuation {
        index: usize,
    },
    InvalidPlan,
}
impl<Error> fmt::Display for HelperReturnError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper return limits exceed safety ceilings",
            Self::InvalidName => "helper return alias is invalid",
            Self::InvalidLexicalName => "helper return lexical name is invalid",
            Self::Input { .. } => "helper return input exceeds physical bounds",
            Self::UnsupportedBoundary => "helper return cannot compose at this effect boundary",
            Self::Capture => "helper return composition would capture or shadow a lexical name",
            Self::Output(_) => "composed helper exceeds physical bounds",
            Self::Factory { .. } => "native continuation materialization failed",
            Self::ChangedContinuation { .. } => {
                "materialized continuation changed its physical shape"
            }
            Self::InvalidPlan => "helper return plan does not match its owned input",
        })
    }
}
impl<Error> fmt::Debug for HelperReturnError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperReturnError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

#[derive(Clone, Copy, Eq, PartialEq)]
struct Shape {
    nodes: usize,
    depth: usize,
}
enum Route {
    Return,
    Bind(Box<Self>),
    Choose(Box<Self>, Box<Self>),
}
struct Analysis {
    shape: Shape,
    pure: bool,
    flow: Option<(Route, usize, usize)>,
}

fn preflight<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    limits: HelperReturnLimits,
) -> Result<Shape, StructureError> {
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut pending = vec![(node, 0)];
    let mut maximum_depth = 0;
    while let Some((node, depth)) = pending.pop() {
        budget.visit(depth, 0, 0)?;
        maximum_depth = maximum_depth.max(depth);
        for child in node.children().rev() {
            budget.check_pending(pending.len(), 1)?;
            pending.push((child, depth + 1));
        }
    }
    Ok(Shape {
        nodes: budget.visited(),
        depth: maximum_depth,
    })
}

// Only called after whole-tree iterative preflight bounds recursion/frontiers.
// Classify purity once bottom-up, rather than rewalking every return subtree.
fn analyze<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    continuation: Shape,
) -> Analysis {
    let mut shape = Shape { nodes: 1, depth: 0 };
    let mut pure = !matches!(
        node,
        Node::Host { .. } | Node::Call { .. } | Node::Group { .. }
    );
    let mut children = Vec::new();
    for child in node.children() {
        let analysis = analyze(child, continuation);
        shape.nodes += analysis.shape.nodes;
        shape.depth = shape.depth.max(analysis.shape.depth + 1);
        pure &= analysis.pure;
        children.push(analysis);
    }
    let flow = if pure
        || matches!(
            node,
            Node::Host { .. } | Node::Call { .. } | Node::Group { .. }
        ) {
        Some((Route::Return, 1, 1 + shape.depth.max(continuation.depth)))
    } else {
        let mut children = children.into_iter();
        match node {
            Node::Bind { .. } => children
                .next()
                .zip(children.next())
                .and_then(|(value, body)| {
                    body.flow.map(|(route, returns, depth)| {
                        (
                            Route::Bind(Box::new(route)),
                            returns,
                            1 + value.shape.depth.max(depth),
                        )
                    })
                }),
            Node::Choose { .. } => children
                .next()
                .zip(children.next())
                .zip(children.next())
                .and_then(|((when, then), otherwise)| {
                    then.flow.zip(otherwise.flow).map(
                        |((then, left, left_depth), (otherwise, right, right_depth))| {
                            (
                                Route::Choose(Box::new(then), Box::new(otherwise)),
                                left + right,
                                1 + when.shape.depth.max(left_depth).max(right_depth),
                            )
                        },
                    )
                }),
            _ => None,
        }
    };
    Analysis { shape, pure, flow }
}

fn names_are_valid<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
) -> bool {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        let valid = match node {
            Node::Local { name } | Node::Bind { name, .. } | Node::Loop { name, .. } => {
                valid_local_name(name)
            }
            Node::Fold { name, item, .. } => valid_local_name(name) && valid_local_name(item),
            Node::Member { group, .. } => valid_local_name(group),
            _ => true,
        };
        if !valid {
            return false;
        }
        pending.extend(node.children());
    }
    true
}

fn exposed_names<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    route: &Route,
) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut pending = vec![(node, route)];
    while let Some((node, route)) = pending.pop() {
        match (node, route) {
            (Node::Bind { name, body, .. }, Route::Bind(route)) => {
                names.insert(name.to_owned());
                pending.push((body, route));
            }
            (
                Node::Choose {
                    then, otherwise, ..
                },
                Route::Choose(left, right),
            ) => {
                pending.push((then, left));
                pending.push((otherwise, right));
            }
            _ => {}
        }
    }
    names
}

fn names_conflict<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    alias: &str,
    reserved: &HashSet<String>,
) -> bool {
    let conflicts =
        |name: &str, reference: bool| reserved.contains(name) || (name == alias && !reference);
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        let conflict = match node {
            Node::Bind { name, .. } | Node::Loop { name, .. } => conflicts(name, false),
            Node::Fold { name, item, .. } => conflicts(name, false) || conflicts(item, false),
            Node::Local { name } => conflicts(name, true),
            Node::Member { group, .. } => conflicts(group, true),
            _ => false,
        };
        if conflict {
            return true;
        }
        pending.extend(node.children());
    }
    false
}

/// Owned immutable language inputs and a bounded language-only return route.
/// Native slots may have interior mutability; this is not an execution certificate.
/// Debug exposes output counts only. No implicit Clone/serde/Send, registry or lock.
#[must_use = "retain or explicitly consume the owned helper return plan"]
pub struct HelperReturns<Node> {
    name: String,
    value: Node,
    continuation: Node,
    continuation_shape: Shape,
    route: Route,
    exposed_names: HashSet<String>,
    shape: HelperReturnShape,
    limits: HelperReturnLimits,
}
impl<Node> fmt::Debug for HelperReturns<Node> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperReturns")
            .field("shape", &self.shape)
            .finish()
    }
}
impl<Node> HelperReturns<Node> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &Node {
        &self.value
    }
    pub fn continuation(&self) -> &Node {
        &self.continuation
    }
    pub const fn shape(&self) -> HelperReturnShape {
        self.shape
    }
}
impl<Field, Operation, HostEffect, IrResult>
    HelperReturns<Node<Field, Operation, HostEffect, IrResult>>
{
    /// Take ownership after complete cold physical preflight, normal-return grammar,
    /// conservative name-capture checks and aggregate expanded-output admission.
    /// Inclusive ceilings are 16,384 nodes and depth 64; zero nodes denies roots,
    /// zero depth cannot fit the required Bind. Every pure subtree is one return;
    /// effectful Bind follows only its body, Choose follows both branches, and
    /// Host/Call/Group return atomically. Other effectful boundaries are rejected,
    /// even when cold or in zero-iteration control. Initializers/guards stay intact.
    /// Host failures are not returns and are never converted into recovery paths.
    ///
    /// All language local/group/binder names must be valid bounded identifiers
    /// before route/name allocation; native export/argument labels remain host policy.
    /// The alias must be valid and avoid value Bind names exposed along return routes.
    /// Continuations may reference it but not bind it; all continuation lexical
    /// names must avoid these exposed binders, including cold return routes. Pure
    /// terminal/initializer/guard locals do not escape into continuations. Neither
    /// input is renamed. Complete prefix-aware lexical
    /// typing, active scope, source costs, canonical bytes and native schemas/grants
    /// remain caller checks. Opaque Host graphs must be closed and separately bounded.
    /// No callbacks or native cloning/query/dispatch happen during admission.
    pub fn new(
        name: String,
        value: Node<Field, Operation, HostEffect, IrResult>,
        continuation: Node<Field, Operation, HostEffect, IrResult>,
        limits: HelperReturnLimits,
    ) -> Result<Self, HelperReturnError<Infallible>> {
        if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
            || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        {
            return Err(HelperReturnError::InvalidLimits);
        }
        if !valid_local_name(&name) {
            return Err(HelperReturnError::InvalidName);
        }
        let value_shape = preflight(&value, limits).map_err(|error| HelperReturnError::Input {
            continuation: false,
            error,
        })?;
        let continuation_shape =
            preflight(&continuation, limits).map_err(|error| HelperReturnError::Input {
                continuation: true,
                error,
            })?;
        if !names_are_valid(&value) || !names_are_valid(&continuation) {
            return Err(HelperReturnError::InvalidLexicalName);
        }
        let analysis = analyze(&value, continuation_shape);
        let (route, returns, depth) = analysis
            .flow
            .ok_or(HelperReturnError::UnsupportedBoundary)?;
        if depth > limits.max_depth {
            return Err(HelperReturnError::Output(StructureError::DepthLimit));
        }
        let nodes = continuation_shape
            .nodes
            .checked_add(1)
            .and_then(|nodes| nodes.checked_mul(returns))
            .and_then(|nodes| value_shape.nodes.checked_add(nodes))
            .filter(|nodes| *nodes <= limits.max_nodes)
            .ok_or(HelperReturnError::Output(StructureError::NodeLimit))?;
        let reserved = exposed_names(&value, &route);
        if reserved.contains(&name) || names_conflict(&continuation, &name, &reserved) {
            return Err(HelperReturnError::Capture);
        }
        Ok(Self {
            name,
            value,
            continuation,
            continuation_shape,
            route,
            exposed_names: reserved,
            shape: HelperReturnShape {
                nodes,
                depth,
                returns,
            },
            limits,
        })
    }

    /// Borrow normal-return subtrees and their original root depths, in rightmost-
    /// child-first DFS order. The private route was fully admitted; no purity walk,
    /// native observation/evaluation or output materialization occurs here. Callers
    /// can reserve folded/native source costs and canonical bytes before connect.
    pub fn return_sites(
        &self,
    ) -> impl Iterator<Item = (&Node<Field, Operation, HostEffect, IrResult>, usize)> {
        let mut pending = vec![(&self.value, &self.route, 0)];
        std::iter::from_fn(move || {
            while let Some((node, route, depth)) = pending.pop() {
                match (node, route) {
                    (_, Route::Return) => return Some((node, depth)),
                    (Node::Bind { body, .. }, Route::Bind(route)) => {
                        pending.push((body, route, depth + 1))
                    }
                    (
                        Node::Choose {
                            then, otherwise, ..
                        },
                        Route::Choose(left, right),
                    ) => {
                        pending.push((then, left, depth + 1));
                        pending.push((otherwise, right, depth + 1));
                    }
                    _ => return None,
                }
            }
            None
        })
    }

    /// Consume once through a trusted continuation factory, called once per normal
    /// return in left-to-right declaration order (not return_sites' observation order).
    /// Each copy is physically preflighted and capture-checked before wrapping;
    /// its exact node count/depth must match the admitted continuation. Original
    /// value native slots, initializer/guard Boxes and vector buffers move unchanged.
    /// Only language route/wrapper storage is owned here, never a second native IR.
    ///
    /// Equal shape does not prove semantic/native/literal identity or source weights.
    /// The factory owns faithful copying/remapping, bounded native work/allocation,
    /// Drop and unwind. Validate complete cold typing/canonical/source/native policy
    /// on the final output before evaluation. There is no fuel, receipt, suspension,
    /// rollback, automatic retry or refund of prior caller-owned reservations.
    /// Error/unwind drops consumed/partial inputs once without returning partial IR.
    /// Native errors are retained for matching, not formatting or source chains.
    pub fn connect<Error>(
        self,
        mut factory: impl FnMut(
            &Node<Field, Operation, HostEffect, IrResult>,
            usize,
        ) -> Result<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperReturnError<Error>> {
        let mut index = 0;
        let mut materialize = || {
            let current = index;
            index += 1;
            let body = factory(&self.continuation, current).map_err(|error| {
                HelperReturnError::Factory {
                    index: current,
                    error,
                }
            })?;
            let actual =
                preflight(&body, self.limits).map_err(|error| HelperReturnError::Input {
                    continuation: true,
                    error,
                })?;
            if actual != self.continuation_shape {
                return Err(HelperReturnError::ChangedContinuation { index: current });
            }
            if !names_are_valid(&body) {
                return Err(HelperReturnError::InvalidLexicalName);
            }
            if names_conflict(&body, &self.name, &self.exposed_names) {
                return Err(HelperReturnError::Capture);
            }
            Ok(body)
        };
        connect(self.value, self.route, &self.name, &mut materialize)
    }
}

fn connect<Field, Operation, HostEffect, IrResult, Error>(
    value: Node<Field, Operation, HostEffect, IrResult>,
    route: Route,
    name: &str,
    materialize: &mut impl FnMut() -> Result<
        Node<Field, Operation, HostEffect, IrResult>,
        HelperReturnError<Error>,
    >,
) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperReturnError<Error>> {
    match (value, route) {
        (value, Route::Return) => {
            let body = materialize()?;
            Ok(Node::Bind {
                name: name.into(),
                value: Box::new(value),
                body: Box::new(body),
            })
        }
        (
            Node::Bind {
                name: local,
                value,
                body,
            },
            Route::Bind(route),
        ) => Ok(Node::Bind {
            name: local,
            value,
            body: Box::new(connect(*body, *route, name, materialize)?),
        }),
        (
            Node::Choose {
                when,
                then,
                otherwise,
            },
            Route::Choose(left, right),
        ) => Ok(Node::Choose {
            when,
            then: Box::new(connect(*then, *left, name, materialize)?),
            otherwise: Box::new(connect(*otherwise, *right, name, materialize)?),
        }),
        _ => Err(HelperReturnError::InvalidPlan),
    }
}
