//! Product value-tree bounds, separate from receipt and execution admission.

use std::{collections::BTreeSet, fmt, slice};

use crate::{StructuredField, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueValidationLimits {
    pub max_depth: usize,
    pub max_fields: usize,
    pub max_query_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueBoundsError {
    Nesting,
    Fields,
    Value,
}

impl fmt::Display for ValueBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Nesting => "value nesting exceeds bounds",
            Self::Fields => "value fields are invalid",
            Self::Value => "value exceeds bounds",
        })
    }
}

impl std::error::Error for ValueBoundsError {}

struct Fields<'a> {
    remaining: slice::Iter<'a, StructuredField>,
    names: BTreeSet<&'a str>,
    child_depth: Option<usize>,
}

/// Validate released leaf/structure bounds and return the saturating item count.
///
/// Depth is inclusive and includes the caller's starting offset. A structure
/// needs 1..=max_fields children; names are checked in declaration order before
/// each child, with duplicates local to that structure. Earlier child failures
/// precede later name failures. Unrepresentable child depth is rejected after
/// its field-name check, never clamped. Leaf rules and empty-query/list counts are shared
/// with `Value::bounded_leaf_items`, including the fixed product log ceiling.
///
/// The explicit stack borrows values and names; it does not clone payloads,
/// serialize or recurse on the Rust stack. Scratch space follows active depth
/// and visited sibling names. Caller-supplied ceilings/name hooks are trusted:
/// aggregate input work/bytes and per-execution output budgets remain separate
/// gates. Saturation is not an overflow certificate or exact unbounded count.
/// No source/schema/receipt/authority admission, dispatch or fuel charge occurs.
/// Callback mutation or unwind does not roll back prior callback work.
pub fn validate_value_items(
    value: &Value,
    depth: usize,
    limits: ValueValidationLimits,
    mut validate_name: impl FnMut(&str) -> bool,
) -> Result<usize, ValueBoundsError> {
    let mut pending = Some((value, depth));
    let mut parents: Vec<Fields<'_>> = Vec::new();
    let mut items = 0usize;
    loop {
        if let Some((value, depth)) = pending.take() {
            if depth > limits.max_depth {
                return Err(ValueBoundsError::Nesting);
            }
            if let Value::Structured { fields } = value {
                if fields.is_empty() || fields.len() > limits.max_fields {
                    return Err(ValueBoundsError::Value);
                }
                parents.push(Fields {
                    remaining: fields.iter(),
                    names: BTreeSet::new(),
                    child_depth: depth.checked_add(1),
                });
            } else {
                items = items.saturating_add(
                    value
                        .bounded_leaf_items(limits.max_query_items)
                        .ok_or(ValueBoundsError::Value)?,
                );
            }
        }
        let Some(parent) = parents.last_mut() else {
            return Ok(items);
        };
        if let Some(field) = parent.remaining.next() {
            if !validate_name(&field.name) || !parent.names.insert(field.name.as_str()) {
                return Err(ValueBoundsError::Fields);
            }
            pending = Some((
                &field.value,
                parent.child_depth.ok_or(ValueBoundsError::Nesting)?,
            ));
        } else {
            parents.pop();
        }
    }
}
