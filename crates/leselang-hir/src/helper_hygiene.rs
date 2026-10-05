//! Owned helper-body name isolation, not helper compilation or effect acceptance.

use std::{collections::HashSet, fmt};

use leselang_runtime_core::{ScopeFrame, StructureBudget, StructureError};

use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
    valid_local_name, valid_member_name,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperHygieneLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bindings: usize,
    pub max_reserved_names: usize,
}

pub enum HelperHygieneError<Error> {
    InvalidLimits,
    InvalidParameters,
    InvalidReservedNames,
    Structure(StructureError),
    InvalidName,
    Capture { group: bool },
    ShadowedBinding,
    BindingLimit,
    FreshName,
    Native(Error),
}
impl<Error> fmt::Display for HelperHygieneError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper hygiene limits exceed safety ceilings",
            Self::InvalidParameters => "helper parameter aliases are invalid",
            Self::InvalidReservedNames => "helper reserved names are invalid",
            Self::Structure(_) => "helper body exceeds physical bounds",
            Self::InvalidName => "helper body language name is invalid",
            Self::Capture { .. } => "helper body cannot capture caller bindings",
            Self::ShadowedBinding => "helper body cannot shadow an active binding",
            Self::BindingLimit => "helper body active binding limit exceeded",
            Self::FreshName => "helper fresh name is invalid or already reserved",
            Self::Native(_) => "native helper name allocation failed",
        })
    }
}
impl<Error> fmt::Debug for HelperHygieneError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperHygieneError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

pub(crate) fn collect_names<Field, Operation, HostEffect, IrResult, Error>(
    body: &Node<Field, Operation, HostEffect, IrResult>,
    limits: HelperHygieneLimits,
    names: &mut HashSet<String>,
) -> Result<(usize, usize), HelperHygieneError<Error>> {
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut maximum_depth = 0;
    let mut pending = vec![(body, 0)];
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(HelperHygieneError::Structure)?;
        maximum_depth = maximum_depth.max(depth);
        let mut name = |name: &str| {
            if !valid_local_name(name) {
                return Err(HelperHygieneError::InvalidName);
            }
            if !names.contains(name) {
                names.insert(name.to_owned());
            }
            Ok(())
        };
        match node {
            Node::Local { name: local } | Node::Bind { name: local, .. } => name(local)?,
            Node::Loop { name: local, .. } => {
                if matches!(local.as_str(), "while" | "next" | "limit") {
                    return Err(HelperHygieneError::InvalidName);
                }
                name(local)?;
            }
            Node::Fold {
                name: state, item, ..
            } => {
                if state == item || matches!(state.as_str(), "items" | "item" | "next" | "limit") {
                    return Err(HelperHygieneError::InvalidName);
                }
                name(state)?;
                name(item)?;
            }
            Node::Member {
                group,
                name: member,
                ..
            } => {
                name(group)?;
                if !valid_member_name(member) {
                    return Err(HelperHygieneError::InvalidName);
                }
            }
            _ => {}
        }
        for child in node.children().rev() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(HelperHygieneError::Structure)?;
            pending.push((child, depth + 1));
        }
    }
    Ok((budget.visited(), maximum_depth))
}

fn check_binding<Error>(
    name: &str,
    scope: &ScopeFrame<'_, '_, ()>,
    count: usize,
    max_bindings: usize,
) -> Result<(), HelperHygieneError<Error>> {
    if scope.get(name).is_some() {
        return Err(HelperHygieneError::ShadowedBinding);
    }
    if scope
        .len()
        .checked_add(count)
        .is_none_or(|length| length > max_bindings)
    {
        return Err(HelperHygieneError::BindingLimit);
    }
    Ok(())
}

pub(crate) fn check_scope<'names, Field, Operation, HostEffect, IrResult, Error>(
    body: &'names Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'names, ()>,
    max_bindings: usize,
) -> Result<(), HelperHygieneError<Error>> {
    match body {
        Node::Local { name } if scope.get(name).is_none() => {
            return Err(HelperHygieneError::Capture { group: false });
        }
        Node::Member { group, .. } if scope.get(group).is_none() => {
            return Err(HelperHygieneError::Capture { group: true });
        }
        Node::Bind { name, value, body } => {
            check_binding(name, scope, 1, max_bindings)?;
            check_scope(value, scope, max_bindings)?;
            let mut local = scope.nested();
            local
                .push(name, ())
                .map_err(|_| HelperHygieneError::ShadowedBinding)?;
            return check_scope(body, &mut local, max_bindings);
        }
        Node::Loop {
            name,
            initial,
            condition,
            next,
            ..
        } => {
            check_binding(name, scope, 1, max_bindings)?;
            check_scope(initial, scope, max_bindings)?;
            let mut local = scope.nested();
            local
                .push(name, ())
                .map_err(|_| HelperHygieneError::ShadowedBinding)?;
            check_scope(condition, &mut local, max_bindings)?;
            return check_scope(next, &mut local, max_bindings);
        }
        Node::Fold {
            name,
            item,
            items,
            initial,
            next,
            ..
        } => {
            check_binding(name, scope, 2, max_bindings)?;
            check_binding(item, scope, 2, max_bindings)?;
            check_scope(items, scope, max_bindings)?;
            check_scope(initial, scope, max_bindings)?;
            let mut local = scope.nested();
            local
                .push(name, ())
                .map_err(|_| HelperHygieneError::ShadowedBinding)?;
            local
                .push(item, ())
                .map_err(|_| HelperHygieneError::ShadowedBinding)?;
            return check_scope(next, &mut local, max_bindings);
        }
        _ => {}
    }
    for child in body.children() {
        check_scope(child, scope, max_bindings)?;
    }
    Ok(())
}

struct Renamer<'names, 'callback, Fresh> {
    names: &'names HashSet<String>,
    used: HashSet<String>,
    fresh: &'callback mut Fresh,
}
impl<'names, Fresh> Renamer<'names, '_, Fresh> {
    fn fresh_name<Error>(&mut self) -> Result<String, HelperHygieneError<Error>>
    where
        Fresh: FnMut() -> Result<String, Error>,
    {
        let name = (self.fresh)().map_err(HelperHygieneError::Native)?;
        if !valid_local_name(&name) || self.names.contains(&name) || !self.used.insert(name.clone())
        {
            return Err(HelperHygieneError::FreshName);
        }
        Ok(name)
    }

    fn enter<Error>(
        &mut self,
        name: &mut String,
        scope: &mut ScopeFrame<'_, 'names, String>,
    ) -> Result<(), HelperHygieneError<Error>>
    where
        Fresh: FnMut() -> Result<String, Error>,
    {
        let fresh = self.fresh_name()?;
        let original = std::mem::replace(name, fresh.clone());
        // Stable owned source-name storage lets the existing lexical guard borrow
        // keys even while the original node's binding label is replaced.
        let key = self
            .names
            .get(&original)
            .ok_or(HelperHygieneError::InvalidName)?;
        scope
            .push(key, fresh)
            .map_err(|_| HelperHygieneError::ShadowedBinding)?;
        Ok(())
    }

    fn rename<Field, Operation, HostEffect, IrResult, Error>(
        &mut self,
        body: &mut Node<Field, Operation, HostEffect, IrResult>,
        scope: &mut ScopeFrame<'_, 'names, String>,
    ) -> Result<(), HelperHygieneError<Error>>
    where
        Fresh: FnMut() -> Result<String, Error>,
    {
        match body {
            Node::Literal { .. } | Node::Host { .. } => {}
            Node::Local { name } => {
                *name = scope
                    .get(name)
                    .ok_or(HelperHygieneError::Capture { group: false })?
                    .clone()
            }
            Node::Member { group, .. } => {
                *group = scope
                    .get(group)
                    .ok_or(HelperHygieneError::Capture { group: true })?
                    .clone()
            }
            Node::Bind { name, value, body } => {
                self.rename(value, scope)?;
                let mut local = scope.nested();
                self.enter(name, &mut local)?;
                self.rename(body, &mut local)?;
            }
            Node::Loop {
                name,
                initial,
                condition,
                next,
                ..
            } => {
                self.rename(initial, scope)?;
                let mut local = scope.nested();
                self.enter(name, &mut local)?;
                self.rename(condition, &mut local)?;
                self.rename(next, &mut local)?;
            }
            Node::Fold {
                name,
                item,
                items,
                initial,
                next,
                ..
            } => {
                self.rename(items, scope)?;
                self.rename(initial, scope)?;
                let mut local = scope.nested();
                self.enter(name, &mut local)?;
                self.enter(item, &mut local)?;
                self.rename(next, &mut local)?;
            }
            Node::Binary { left, right, .. } => {
                self.rename(left, scope)?;
                self.rename(right, scope)?;
            }
            Node::Unary { value, .. } | Node::Field { value, .. } => self.rename(value, scope)?,
            Node::Choose {
                when,
                then,
                otherwise,
            } => {
                self.rename(when, scope)?;
                self.rename(then, scope)?;
                self.rename(otherwise, scope)?;
            }
            Node::Recover { value, fallback } => {
                self.rename(value, scope)?;
                self.rename(fallback, scope)?;
            }
            Node::Strings { items } => {
                for item in items {
                    self.rename(item, scope)?;
                }
            }
            Node::Call { arguments, .. } => {
                for argument in arguments {
                    self.rename(&mut argument.value, scope)?;
                }
            }
            Node::Group { branches, .. } => {
                for branch in branches {
                    self.rename(&mut branch.value, scope)?;
                }
            }
        }
        Ok(())
    }
}

/// Isolate an owned helper body using explicit original/fresh parameter aliases.
///
/// Fixed ceilings are 16,384 physical nodes, depth 64, 1,024 active bindings and
/// 16,384 caller-reserved names. Limits are inclusive; zero nodes denies bodies,
/// zero depth allows leaves, and zero bindings forbids parameters/local bindings.
/// Zero reserved capacity forbids caller reservations. No default policy exists.
/// Alias/prefix policy, the entire physical body and every cold lexical path
/// are checked before invoking the fresh-name callback. Free locals/groups and
/// active shadowing are rejected, including zero-loop/fold and recovery bodies.
/// Initializers/items use the parent scope; only lexical bodies see new bindings.
///
/// Fresh names must avoid all original lexical names, parameter aliases, caller
/// reserved names and previously generated names. Invalid/duplicate callbacks
/// fail once, without retries. Names are language-owned bounded strings; native
/// field/operation/effect/result slots move untouched and need no Clone/Debug/
/// serde/Send. The original Boxes, vectors and native buffers are not rebuilt.
/// Opaque Host payloads must be closed with respect to language locals; their
/// graphs/work/Drop and global namespace reservation remain adapter-owned.
///
/// This rewrites a body, not parameter arguments or a complete helper program.
/// Declarations/recursion, scalar parameter/result typing, source expansion costs,
/// canonical wire policy, native schemas/authority and full cold type inference
/// remain separate. Runtime prefixes, fuel and receipts are neither read nor
/// created. Failure/unwind drops the consumed body; no partial body is returned,
/// and native name reservations are not rolled back or refunded.
pub fn hygienic_helper_body<Field, Operation, HostEffect, IrResult, Error>(
    mut body: Node<Field, Operation, HostEffect, IrResult>,
    parameters: &[(&str, &str)],
    reserved: &[&str],
    limits: HelperHygieneLimits,
    mut fresh: impl FnMut() -> Result<String, Error>,
) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperHygieneError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
        || limits.max_reserved_names > MAX_TYPE_INFERENCE_NODES
    {
        return Err(HelperHygieneError::InvalidLimits);
    }
    if parameters.len() > limits.max_bindings {
        return Err(HelperHygieneError::BindingLimit);
    }
    let mut names = HashSet::new();
    for &(original, alias) in parameters {
        if !valid_local_name(original)
            || !valid_local_name(alias)
            || !names.insert(original.to_owned())
        {
            return Err(HelperHygieneError::InvalidParameters);
        }
    }
    if reserved.len() > limits.max_reserved_names
        || reserved.iter().any(|name| !valid_local_name(name))
    {
        return Err(HelperHygieneError::InvalidReservedNames);
    }
    collect_names(&body, limits, &mut names)?;
    {
        let mut bindings = parameters.iter().map(|(name, _)| (*name, ())).collect();
        check_scope(
            &body,
            &mut ScopeFrame::new(&mut bindings),
            limits.max_bindings,
        )?;
    }
    let mut used: HashSet<_> = reserved.iter().map(|name| (*name).to_owned()).collect();
    for &(_, alias) in parameters {
        if names.contains(alias) || !used.insert(alias.to_owned()) {
            return Err(HelperHygieneError::InvalidParameters);
        }
    }
    let mut renamer = Renamer {
        names: &names,
        used,
        fresh: &mut fresh,
    };
    let mut bindings = parameters
        .iter()
        .map(|(name, alias)| (*name, (*alias).to_owned()))
        .collect();
    renamer.rename(&mut body, &mut ScopeFrame::new(&mut bindings))?;
    Ok(body)
}
