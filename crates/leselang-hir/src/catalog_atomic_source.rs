//! Cold, bounded source-shape facts for the explicit prepared-member profile.

use leselang_syntax::{Expression, Function};

use crate::catalog_group_source::CatalogGroupSourceError;
use crate::choice_source::operands;
use crate::prepared_typing::PreparedCallTypeError;
use crate::scalar_source::{binding_source, primitive_name};

#[derive(Clone, Copy, PartialEq)]
enum Shape<'source> {
    Pure,
    Atomic(&'source str),
    Other,
}

// The shared closed-source gate has bounded depth and validated intrinsic headers.
// Helper edges use already computed facts, never recursive source substitution.
fn shape<'source>(
    source: &'source Expression,
    helpers: &[&'source Function],
    facts: &[(&'source str, Shape<'source>)],
) -> Shape<'source> {
    let Expression::Call {
        callee, arguments, ..
    } = source
    else {
        return Shape::Pure;
    };
    if callee == "bind" {
        let Ok(form) = binding_source::<()>(source) else {
            return Shape::Other;
        };
        return if shape(&form.binding.value, helpers, facts) == Shape::Pure {
            shape(form.body, helpers, facts)
        } else {
            Shape::Other
        };
    }
    if callee == "choose" {
        let Ok([when, then, otherwise]) = operands::<()>(source) else {
            return Shape::Other;
        };
        if shape(when, helpers, facts) != Shape::Pure {
            return Shape::Other;
        }
        let then = shape(then, helpers, facts);
        let otherwise = shape(otherwise, helpers, facts);
        return if then == otherwise {
            then
        } else {
            Shape::Other
        };
    }
    if matches!(callee.as_str(), "seq" | "all" | "repeat")
        || !arguments
            .iter()
            .all(|argument| shape(&argument.value, helpers, facts) == Shape::Pure)
    {
        return Shape::Other;
    }
    if primitive_name(callee) || matches!(callee.as_str(), "loop" | "fold" | "field" | "member") {
        return Shape::Pure;
    }
    if helpers.iter().any(|helper| helper.name == *callee) {
        return facts
            .iter()
            .find(|(name, _)| *name == callee)
            .map_or(Shape::Other, |(_, shape)| *shape);
    }
    Shape::Atomic(callee)
}

/// All closed declarations and catalog/named/dependency gates run before this.
/// Every helper body is classified once in dependency order, then every member
/// occurrence (including unused helpers) is checked without expanding a body or
/// touching native slots. These are shape facts, never type/schema authority.
pub(crate) fn cold_prepared_members<'source, Native>(
    declarations: &[&'source Function],
    ordered_helpers: &[&'source Function],
) -> Result<(), CatalogGroupSourceError<Native>> {
    let mut facts = Vec::with_capacity(ordered_helpers.len());
    for helper in ordered_helpers {
        facts.push((
            helper.name.as_str(),
            shape(&helper.body, ordered_helpers, &facts),
        ));
    }
    for function in declarations {
        let mut pending = vec![&function.body];
        while let Some(node) = pending.pop() {
            let Expression::Call {
                callee, arguments, ..
            } = node
            else {
                continue;
            };
            if matches!(callee.as_str(), "seq" | "all") {
                for (index, member) in arguments.iter().enumerate() {
                    if !matches!(
                        shape(&member.value, ordered_helpers, &facts),
                        Shape::Atomic(_)
                    ) {
                        return Err(CatalogGroupSourceError::PreparedMember {
                            index,
                            span: member.span,
                            error: PreparedCallTypeError::InvalidFlow,
                        });
                    }
                }
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}
