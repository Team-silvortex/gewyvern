//! Closed group-export observations, not type inference or execution authority.

use std::fmt;

use leselang_runtime_core::{
    MAX_LOOP_ITERATIONS, MAX_STRING_LIST_ITEMS, StructureBudget, StructureError,
};

use crate::call_typing::{MAX_CALL_ARGUMENTS, valid_argument_name};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::native_group::preflight_native_group_members;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, valid_local_name, valid_member_name,
};

pub const MAX_GROUP_EXPORT_MEMBERS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupExportLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_groups: usize,
    pub max_members: usize,
}

/// Borrowed name and explicitly produced native observation. Neither is authority.
pub struct GroupExport<'name, Operation> {
    pub name: &'name str,
    pub operation: Operation,
}
impl<Operation> fmt::Debug for GroupExport<'_, Operation> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GroupExport(<native>)")
    }
}

/// Ordered, closed signature data. Native observations may contain interior state.
/// The public fields do not make this a validated/cached group or authentic receipt.
pub struct GroupExports<'name, Operation> {
    pub kind: GroupKind,
    pub members: Vec<GroupExport<'name, Operation>>,
}
impl<Operation> fmt::Debug for GroupExports<'_, Operation> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GroupExports")
            .field("kind", &self.kind)
            .field("members", &self.members.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupExportPhase {
    Host,
    Branch { index: usize },
    Compare { index: usize },
}
pub enum GroupExportError<Error> {
    InvalidLimits,
    Structure(StructureError),
    InvalidLanguageNode,
    UnsupportedTail,
    ImpurePreparation,
    GroupLimit,
    MemberShape,
    SignatureMismatch,
    Native {
        group_index: usize,
        phase: GroupExportPhase,
        error: Error,
    },
}
impl<Error> fmt::Display for GroupExportError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "group export limits exceed safety ceilings",
            Self::Structure(_) => "group export IR exceeds physical limits",
            Self::InvalidLanguageNode => "group export IR has invalid language data",
            Self::UnsupportedTail => "group export path does not end in a flat group",
            Self::ImpurePreparation => "group export preparation contains an effect",
            Self::GroupLimit => "group export observation count exceeds its limit",
            Self::MemberShape => "group exports have invalid arity or member names",
            Self::SignatureMismatch => "cold group exports are not one closed ordered signature",
            Self::Native { .. } => "native group export observation failed",
        })
    }
}
impl<Error> fmt::Debug for GroupExportError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for GroupExportError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;
type Routes<'expression, Node, Error> = Result<Vec<&'expression Node>, GroupExportError<Error>>;

fn member_shape<'name, Member>(
    kind: GroupKind,
    members: &'name [Member],
    maximum: usize,
    mut name: impl FnMut(&'name Member) -> &'name str,
) -> bool {
    preflight_native_group_members(kind, members, maximum, |member| {
        Ok::<_, std::convert::Infallible>(name(member))
    })
    .is_ok()
}

pub(crate) fn physical<Field, Operation, HostEffect, IrResult, Error>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
    limits: GroupExportLimits,
) -> Result<(), GroupExportError<Error>> {
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut pending = vec![(expression, 0)];
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(GroupExportError::Structure)?;
        let valid = match node {
            Computation::Literal { value } => value.is_bounded(),
            Computation::Strings { items } => items.len() <= MAX_STRING_LIST_ITEMS,
            Computation::Call { arguments, .. } => {
                arguments.len() <= MAX_CALL_ARGUMENTS
                    && arguments
                        .iter()
                        .all(|argument| valid_argument_name(&argument.name))
            }
            Computation::Local { name } | Computation::Bind { name, .. } => valid_local_name(name),
            Computation::Member { group, name, .. } => {
                valid_local_name(group) && valid_member_name(name)
            }
            Computation::Loop { name, limit, .. } => {
                valid_local_name(name)
                    && !matches!(name.as_str(), "while" | "next" | "limit")
                    && *limit <= MAX_LOOP_ITERATIONS
            }
            Computation::Fold {
                name, item, limit, ..
            } => {
                valid_local_name(name)
                    && valid_local_name(item)
                    && name != item
                    && !matches!(name.as_str(), "items" | "item" | "next" | "limit")
                    && *limit <= MAX_STRING_LIST_ITEMS as u64
            }
            Computation::Group {
                group_kind,
                branches,
            } => {
                if !member_shape(*group_kind, branches, limits.max_members, |branch| {
                    branch.name.as_str()
                }) {
                    return Err(GroupExportError::MemberShape);
                }
                true
            }
            _ => true,
        };
        if !valid {
            return Err(GroupExportError::InvalidLanguageNode);
        }
        for child in node.children() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(GroupExportError::Structure)?;
            pending.push((child, depth + 1));
        }
    }
    Ok(())
}

// Only candidate shape, not boolean/scalar typing, schemas or opaque atomic graphs.
pub(crate) fn atomic_candidate<Field, Operation, HostEffect, IrResult>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match node {
            Computation::Host { .. } => {}
            Computation::Call { arguments, .. }
                if arguments.iter().all(|argument| argument.value.is_pure()) => {}
            Computation::Bind { value, body, .. } if value.is_pure() => pending.push(body),
            Computation::Choose {
                when,
                then,
                otherwise,
            } if when.is_pure() => pending.extend([then.as_ref(), otherwise.as_ref()]),
            _ => return false,
        }
    }
    true
}

fn routes<'expression, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    limits: GroupExportLimits,
) -> Routes<'expression, Node<Field, Operation, HostEffect, IrResult>, Error> {
    let mut pending = vec![expression];
    let mut leaves = Vec::new();
    let mut known: Option<&Node<Field, Operation, HostEffect, IrResult>> = None;
    while let Some(node) = pending.pop() {
        match node {
            Computation::Bind { value, body, .. } => {
                if !value.is_pure() {
                    return Err(GroupExportError::ImpurePreparation);
                }
                pending.push(body);
                continue;
            }
            Computation::Choose {
                when,
                then,
                otherwise,
            } => {
                if !when.is_pure() {
                    return Err(GroupExportError::ImpurePreparation);
                }
                pending.extend([then.as_ref(), otherwise.as_ref()]);
                continue;
            }
            Computation::Host { .. } => {}
            Computation::Group {
                group_kind,
                branches,
            } => {
                if branches
                    .iter()
                    .any(|branch| !atomic_candidate(&branch.value))
                {
                    return Err(GroupExportError::UnsupportedTail);
                }
                if let Some(Computation::Group {
                    group_kind: kind,
                    branches: previous,
                }) = known
                    && (group_kind != kind
                        || branches.len() != previous.len()
                        || branches
                            .iter()
                            .zip(previous)
                            .any(|(left, right)| left.name != right.name))
                {
                    return Err(GroupExportError::SignatureMismatch);
                }
                known = Some(node);
            }
            _ => return Err(GroupExportError::UnsupportedTail),
        }
        if leaves.len() >= limits.max_groups {
            return Err(GroupExportError::GroupLimit);
        }
        leaves.push(node);
    }
    Ok(leaves)
}

/// Observe one closed signature across all cold group return paths. Inclusive
/// ceilings: 16,384 physical nodes/groups, depth 64 and 64 members per group.
/// Sequence needs one member; parallel needs two. Zero node/group/member quotas
/// deny observations; depth zero permits only the root node.
/// Complete physical/name/literal preflight and candidate-route checks precede
/// every native hook; all statically known group names/kinds/orders must match.
/// Call operands use the shared 64-argument and bounded identifier rules; native
/// required/duplicate/unknown parameter shape remains adapter schema policy.
/// Pure preparation is structural only: lexical/boolean/type checks remain host
/// policy, including cold zero controls. Opaque Host graphs are not traversed here.
///
/// Hooks observe the exact original Host or branch including its declared result.
/// Adapters corroborate every cold atomic schema, return declaration, version,
/// grants and closed bounded native graph. Host observations are checked for arity,
/// bounded unique names and kind before operation comparison. Same spelling/result
/// tags are not declaration identity: the explicit comparator must enforce the
/// host's exact operation/schema identity, not grant a union or coercion.
///
/// Groups visit once in rightmost-first DFS order; IR branch hooks visit once in
/// declaration order. Comparisons visit members in order only after shape matches.
/// The final leftmost observation is returned unchanged with borrowed original
/// names and moved native slots. No native Clone/Debug/serde/Send/PartialEq bound
/// is imposed. Failure/unwind releases observations without partial output, retry,
/// dispatch or rollback of native side effects; borrowed IR is never consumed.
/// Native errors are matchable but redacted from formatting and source chains.
/// Native work/allocation/interior mutation/Drop/unwind is trusted, not fuel-limited
/// or sandboxed. Bound ingress before IR construction; revalidate live native state
/// before use. This is not full source lowering, flow typing, capture, receipt,
/// canonical acceptance, suspension, registry storage or execution authority.
pub fn observe_group_exports<'expression, Field, Operation, HostEffect, IrResult, Export, Error>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    limits: GroupExportLimits,
    mut host: impl FnMut(&'expression HostEffect) -> Result<GroupExports<'expression, Export>, Error>,
    mut branch: impl FnMut(
        &'expression Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<Export, Error>,
    mut same: impl FnMut(&Export, &Export) -> Result<bool, Error>,
) -> Result<GroupExports<'expression, Export>, GroupExportError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_groups > MAX_TYPE_INFERENCE_NODES
        || limits.max_members > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(GroupExportError::InvalidLimits);
    }
    physical(expression, limits)?;
    if limits.max_members == 0 {
        return Err(GroupExportError::MemberShape);
    }
    let leaves = routes(expression, limits)?;
    let mut signature: Option<GroupExports<'expression, Export>> = None;
    for (group_index, node) in leaves.into_iter().enumerate() {
        let current = match node {
            Computation::Host { effect } => {
                host(effect).map_err(|error| GroupExportError::Native {
                    group_index,
                    phase: GroupExportPhase::Host,
                    error,
                })?
            }
            Computation::Group {
                group_kind,
                branches,
            } => {
                let mut members = Vec::with_capacity(branches.len());
                for (index, member) in branches.iter().enumerate() {
                    let operation = branch(member).map_err(|error| GroupExportError::Native {
                        group_index,
                        phase: GroupExportPhase::Branch { index },
                        error,
                    })?;
                    members.push(GroupExport {
                        name: &member.name,
                        operation,
                    });
                }
                GroupExports {
                    kind: *group_kind,
                    members,
                }
            }
            _ => return Err(GroupExportError::UnsupportedTail),
        };
        if !member_shape(
            current.kind,
            &current.members,
            limits.max_members,
            |member| member.name,
        ) {
            return Err(GroupExportError::MemberShape);
        }
        if let Some(expected) = &signature {
            let matches = crate::native_group_signature::compare_native_group_signatures(
                (expected.kind, &expected.members),
                (current.kind, &current.members),
                limits.max_members,
                |member| Ok(member.name),
                |member| Ok(member.name),
                |expected, current| same(&expected.operation, &current.operation),
            )
            .map_err(|error| match error {
                crate::native_group_signature::NativeGroupSignatureError::Members { .. } => {
                    GroupExportError::MemberShape
                }
                crate::native_group_signature::NativeGroupSignatureError::Native {
                    member_index,
                    error,
                } => GroupExportError::Native {
                    group_index,
                    phase: GroupExportPhase::Compare {
                        index: member_index,
                    },
                    error,
                },
            })?;
            if !matches {
                return Err(GroupExportError::SignatureMismatch);
            }
        }
        signature = Some(current);
    }
    signature.ok_or(GroupExportError::UnsupportedTail)
}

#[cfg(test)]
mod shape_tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::{GroupKind, member_shape};

    #[test]
    fn closed_export_shape_reads_each_original_label_once_without_native_traits() {
        struct Member {
            name: String,
            native: Rc<Cell<u8>>,
        }
        let mut members = ["first", "second", "third"].map(|name| Member {
            name: name.into(),
            native: Rc::new(Cell::new(7)),
        });
        let seen = RefCell::new(Vec::new());
        for (maximum, expected) in [(0, false), (2, false), (3, true), (65, false)] {
            seen.borrow_mut().clear();
            assert_eq!(
                member_shape(GroupKind::Sequence, &members, maximum, |member| {
                    seen.borrow_mut().push(std::ptr::from_ref(member));
                    member.name.as_str()
                }),
                expected
            );
            let original = if expected {
                members.iter().map(std::ptr::from_ref).collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            assert_eq!(*seen.borrow(), original);
        }
        for last in ["first", "bad tail"] {
            members[2].name = last.into();
            seen.borrow_mut().clear();
            assert!(!member_shape(GroupKind::Sequence, &members, 3, |member| {
                seen.borrow_mut().push(std::ptr::from_ref(member));
                member.name.as_str()
            }));
            assert_eq!(
                *seen.borrow(),
                members.iter().map(std::ptr::from_ref).collect::<Vec<_>>()
            );
        }
        for member in &members {
            assert_eq!(Rc::strong_count(&member.native), 1);
            assert_eq!(member.native.get(), 7);
        }
    }
}
