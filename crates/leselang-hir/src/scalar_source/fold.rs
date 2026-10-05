//! Fold source metadata and pure gates, not collection evaluation or traversal.

use super::{Node, Operand, ScalarSourceError, scalar_operand};
use crate::ir::Computation;
use crate::pure_typing::{PureTypeError, valid_local_name};
use crate::source_call::span;
use leselang_runtime_core::{MAX_STRING_LIST_ITEMS, ScalarType, ScopeFrame};
use leselang_syntax::{Expression, NamedArgument, Span};

pub(crate) struct FoldSource<'source> {
    pub binding: &'source NamedArgument,
    pub items: &'source Expression,
    pub item: &'source str,
    pub next: &'source Expression,
    limit: &'source Expression,
    at: Span,
}

impl FoldSource<'_> {
    pub fn binding_error<Error>(&self, error: PureTypeError) -> ScalarSourceError<Error> {
        ScalarSourceError::FoldBindings {
            span: self.at,
            error,
        }
    }

    // Reference callers retain their source-cost binding bound. Independent
    // callers always supply their explicit active quota, including the prefix.
    pub fn check_scope<Value, Error>(
        &self,
        scope: &ScopeFrame<'_, '_, Value>,
        max_bindings: Option<usize>,
    ) -> Result<(), ScalarSourceError<Error>> {
        if scope.get(&self.binding.name).is_some() || scope.get(self.item).is_some() {
            return Err(self.binding_error(PureTypeError::ShadowedBinding));
        }
        if max_bindings
            .is_some_and(|limit| scope.len().checked_add(2).is_none_or(|count| count > limit))
        {
            return Err(self.binding_error(PureTypeError::BindingLimit));
        }
        Ok(())
    }

    pub fn limit<Error>(&self) -> Result<u64, ScalarSourceError<Error>> {
        let Expression::Integer { value, .. } = self.limit else {
            return Err(ScalarSourceError::FoldLimit {
                span: span(self.limit),
                exceeds_bound: false,
            });
        };
        if *value > MAX_STRING_LIST_ITEMS as u64 {
            return Err(ScalarSourceError::FoldLimit {
                span: span(self.limit),
                exceeds_bound: true,
            });
        }
        Ok(*value)
    }

    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        items: Node<Field, Operation, HostEffect, IrResult>,
        initial: Node<Field, Operation, HostEffect, IrResult>,
        next: Node<Field, Operation, HostEffect, IrResult>,
        limit: u64,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Fold {
            name: self.binding.name.clone(),
            item: self.item.to_owned(),
            items: Box::new(items),
            initial: Box::new(initial),
            next: Box::new(next),
            limit,
        }
    }
}

pub(crate) fn fold_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<FoldSource<'_>, ScalarSourceError<Error>> {
    let reserved = ["items", "item", "next", "limit"];
    let binding = arguments
        .iter()
        .find(|argument| !reserved.contains(&argument.name.as_str()))
        .ok_or(ScalarSourceError::FoldShape {
            span: at,
            missing_state: true,
        })?;
    if arguments.len() != 5
        || [binding.name.as_str(), "items", "item", "next", "limit"]
            .iter()
            .any(|name| {
                arguments
                    .iter()
                    .filter(|argument| argument.name == *name)
                    .count()
                    != 1
            })
    {
        return Err(ScalarSourceError::FoldShape {
            span: at,
            missing_state: false,
        });
    }
    let get = |name: &str| {
        arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value)
            .ok_or(ScalarSourceError::FoldShape {
                span: at,
                missing_state: false,
            })
    };
    let item_expression = get("item")?;
    let Expression::String { value: item, .. } = item_expression else {
        return Err(ScalarSourceError::FoldItem {
            span: span(item_expression),
        });
    };
    if !valid_local_name(&binding.name) || !valid_local_name(item) || item == &binding.name {
        return Err(ScalarSourceError::FoldBindings {
            span: at,
            error: PureTypeError::InvalidName,
        });
    }
    Ok(FoldSource {
        binding,
        items: get("items")?,
        item,
        next: get("next")?,
        limit: get("limit")?,
        at,
    })
}

pub(crate) fn fold_items<Node, Error>(
    operand: Operand<Node>,
    at: Span,
) -> Result<Node, ScalarSourceError<Error>> {
    let (items, ty) = scalar_operand(operand, at)?;
    if ty != ScalarType::StringList {
        return Err(ScalarSourceError::FoldItems { span: at });
    }
    Ok(items)
}

pub(crate) fn fold_next<Node, Error>(
    operand: Operand<Node>,
    state_type: ScalarType,
    at: Span,
) -> Result<Node, ScalarSourceError<Error>> {
    let (next, ty) = scalar_operand(operand, at)?;
    if ty != state_type {
        return Err(ScalarSourceError::FoldState { span: at });
    }
    Ok(next)
}
