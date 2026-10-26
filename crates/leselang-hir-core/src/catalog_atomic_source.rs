//! Cold, bounded source-shape facts for the explicit prepared-member profile.

use leselang_syntax::{Expression, Function};

use crate::catalog_group_source::CatalogGroupSourceError;
use crate::choice_source::operands;
use crate::group_exports::joined_member_name_fits;
use crate::prepared_typing::PreparedCallTypeError;
use crate::repeat_sequence_source::RepeatSequenceSourceError;
use crate::scalar_source::{binding_source, primitive_name};
use crate::sequence_source::SequenceSourceError;

fn sequential(source: &Expression) -> bool {
    matches!(source, Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "repeat"))
}

// Only source-known exports: no helper substitution, IR, native metadata or dispatch.
// Closed source depth bounds recursion; each retained vector has at most `maximum`
// bounded names. Repeat visits its body once, checking the product before expansion.
fn sequential_exports<Native>(
    source: &Expression,
    maximum: usize,
) -> Result<Vec<String>, CatalogGroupSourceError<Native>> {
    if matches!(source, Expression::Call { callee, .. } if callee == "repeat") {
        let (count, body) = crate::repeat_source::header(source, maximum)
            .map_err(|error| CatalogGroupSourceError::Repeat(Box::new(error)))?;
        if !sequential(&body.value) {
            return Ok((1..=count).map(|i| format!("iteration_{i}")).collect());
        }
        let names = sequential_exports(&body.value, maximum)?;
        let width = names
            .len()
            .checked_mul(count)
            .filter(|width| *width <= maximum)
            .ok_or_else(|| {
                CatalogGroupSourceError::RepeatSequence(Box::new(
                    RepeatSequenceSourceError::Width {
                        iteration: 0,
                        span: body.span,
                    },
                ))
            })?;
        let longest = format!("iteration_{count}");
        for (member, name) in names.iter().enumerate() {
            if !joined_member_name_fits(&longest, name) {
                return Err(CatalogGroupSourceError::RepeatSequence(Box::new(
                    RepeatSequenceSourceError::Name {
                        iteration: 0,
                        member,
                        span: body.span,
                    },
                )));
            }
        }
        let mut output = Vec::with_capacity(width);
        for iteration in 1..=count {
            output.extend(
                names
                    .iter()
                    .map(|name| format!("iteration_{iteration}__{name}")),
            );
        }
        return Ok(output);
    }
    let (_, arguments) = crate::group_source::header(source, maximum)
        .map_err(|error| CatalogGroupSourceError::Group(Box::new(error)))?;
    let mut output = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        let nested = sequential(&argument.value);
        let names = if nested {
            sequential_exports(&argument.value, maximum)?
        } else {
            vec![argument.name.to_owned()]
        };
        if names.len() > maximum - output.len() {
            return Err(CatalogGroupSourceError::Sequence(Box::new(
                SequenceSourceError::Width {
                    index,
                    span: argument.span,
                },
            )));
        }
        for (member, name) in names.into_iter().enumerate() {
            let name = if nested {
                if !joined_member_name_fits(&argument.name, &name) {
                    return Err(CatalogGroupSourceError::Sequence(Box::new(
                        SequenceSourceError::Name {
                            index,
                            member,
                            span: argument.span,
                        },
                    )));
                }
                format!("{}__{name}", argument.name)
            } else {
                name
            };
            if output.contains(&name) {
                return Err(CatalogGroupSourceError::Sequence(Box::new(
                    SequenceSourceError::Collision {
                        index,
                        member,
                        span: argument.span,
                    },
                )));
            }
            output.push(name);
        }
    }
    Ok(output)
}

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
/// touching native slots. Source-known sequential export width, joined names and
/// collisions are also checked before any declaration is lowered. These are shape
/// facts, never type/schema authority; whole native output admission remains required.
pub(crate) fn cold_prepared_members<'source, Native>(
    declarations: &[&'source Function],
    ordered_helpers: &[&'source Function],
    max_branches: usize,
) -> Result<(), CatalogGroupSourceError<Native>> {
    let mut facts = Vec::with_capacity(ordered_helpers.len());
    for helper in ordered_helpers {
        facts.push((
            helper.name.as_str(),
            shape(&helper.body, ordered_helpers, &facts),
        ));
    }
    for function in declarations {
        cold_members(&function.body, ordered_helpers, &facts, max_branches)?;
    }
    Ok(())
}

/// The caller has already admitted the complete closed expression and catalog.
/// An expression has no helper namespace or fallback to external declarations.
pub(crate) fn cold_prepared_source<Native>(
    source: &Expression,
    max_branches: usize,
) -> Result<(), CatalogGroupSourceError<Native>> {
    cold_members(source, &[], &[], max_branches)
}

fn cold_members<'source, Native>(
    source: &'source Expression,
    helpers: &[&'source Function],
    facts: &[(&'source str, Shape<'source>)],
    max_branches: usize,
) -> Result<(), CatalogGroupSourceError<Native>> {
    let mut pending = vec![(source, false)];
    while let Some((node, sequential_child)) = pending.pop() {
        let Expression::Call {
            callee, arguments, ..
        } = node
        else {
            continue;
        };
        if matches!(callee.as_str(), "seq" | "all" | "repeat") {
            for (index, member) in arguments
                .iter()
                .filter(|member| callee != "repeat" || member.name == "body")
                .enumerate()
            {
                if callee != "all" && sequential(&member.value) {
                    continue;
                }
                if !matches!(shape(&member.value, helpers, facts), Shape::Atomic(_)) {
                    return Err(CatalogGroupSourceError::PreparedMember {
                        index,
                        span: member.span,
                        error: PreparedCallTypeError::InvalidFlow,
                    });
                }
            }
            if callee != "all" && !sequential_child {
                sequential_exports(node, max_branches)?;
            }
        }
        pending.extend(arguments.iter().rev().map(|argument| {
            (
                &argument.value,
                callee == "seq" || (callee == "repeat" && argument.name == "body"),
            )
        }));
    }
    Ok(())
}
