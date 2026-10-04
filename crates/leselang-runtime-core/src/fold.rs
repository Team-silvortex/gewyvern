use crate::{MAX_STRING_LIST_ITEMS, ScalarType, ScalarValue, StringListValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoldPhase {
    NextItem,
    Advance,
    Done,
}

/// Owned, bounded scalar-fold traversal, not evaluation of its next expression.
///
/// Construction consumes the list, including on failure, and rejects overflow
/// before handing out any item. Text buffers move in source order without cloning.
/// The adapter owns a bounded initial accumulator, whole-expression preflight,
/// fuel, evaluation, scope cleanup and recovery. This cursor retains its type,
/// not the accumulator payload; `Debug` omits the unvisited collection as well.
///
/// Each `next_item` must be followed by one validated `advance` before another
/// item can be obtained. Validation errors do not refund fuel or authorize replay
/// of failed expressions/host work. This cursor has no callbacks, timer, grant,
/// mid-fold suspension, persistence or enforceable sandbox. Trusted adapters can
/// explicitly create another cursor from their own list.
///
/// ```
/// use leselang_runtime_core::{FoldCursor, ScalarType, ScalarValue, StringListValue};
/// let mut cursor = FoldCursor::new(StringListValue(vec!["one".into(), "two".into()]),
///                                  ScalarType::Integer, 2).unwrap();
/// let mut count = 0;
/// while let Some(item) = cursor.next_item().unwrap() {
///     assert!(!item.is_empty());
///     count += 1;
///     cursor.advance(&ScalarValue::Integer(count)).unwrap();
/// }
/// assert_eq!(cursor.completed_iterations(), 2);
/// ```
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_runtime_core::{FoldCursor, ScalarType, StringListValue};
/// FoldCursor::new(StringListValue(vec![]), ScalarType::Integer, 0).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{FoldCursor, ScalarType, StringListValue};
/// let cursor = FoldCursor::new(StringListValue(vec![]), ScalarType::Integer, 0).unwrap();
/// let duplicate = cursor.clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{FoldCursor, ScalarType, StringListValue};
/// let cursor = FoldCursor::new(StringListValue(vec![]), ScalarType::Integer, 0).unwrap();
/// let moved = cursor;
/// assert_eq!(cursor.completed_iterations(), 0);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::FoldCursor;
/// let cursor = FoldCursor::default();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::FoldCursor;
/// let cursor: FoldCursor = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{FoldCursor, ScalarType, StringListValue};
/// let wire = serde_json::to_string(&FoldCursor::new(StringListValue(vec![]),
///                                                 ScalarType::Integer, 0).unwrap()).unwrap();
/// ```
#[must_use = "retain one cursor for the whole fold; dropping it discards unvisited items"]
pub struct FoldCursor {
    items: std::vec::IntoIter<String>,
    state_type: ScalarType,
    item_count: u64,
    completed: u64,
    phase: FoldPhase,
}

impl FoldCursor {
    /// The limit is 0-64; excess items fail upfront, never truncate the list.
    /// Invalid limit precedes invalid collection, which precedes excess item count.
    pub fn new(
        items: StringListValue,
        state_type: ScalarType,
        limit: u64,
    ) -> Result<Self, FoldError> {
        if limit > MAX_STRING_LIST_ITEMS as u64 {
            return Err(FoldError::InvalidLimit);
        }
        if !items.is_bounded() {
            return Err(FoldError::UnboundedItems);
        }
        let item_count = items.0.len() as u64;
        if item_count > limit {
            return Err(FoldError::IterationLimit);
        }
        Ok(Self {
            items: items.0.into_iter(),
            state_type,
            item_count,
            completed: 0,
            phase: FoldPhase::NextItem,
        })
    }

    pub const fn completed_iterations(&self) -> u64 {
        self.completed
    }

    /// Transfer one item to the adapter, without evaluating or charging anything.
    /// `None` ends traversal; any subsequent next/advance call is an ordering error.
    pub fn next_item(&mut self) -> Result<Option<String>, FoldError> {
        if self.phase != FoldPhase::NextItem {
            return Err(FoldError::InvalidPhase);
        }
        let item = self.items.next();
        self.phase = if item.is_some() {
            FoldPhase::Advance
        } else {
            FoldPhase::Done
        };
        Ok(item)
    }

    /// Validate a next accumulator before replacing scoped state or counting it.
    /// Type/bounds/order errors leave the phase, unvisited items and count unchanged.
    pub fn advance(&mut self, next: &ScalarValue) -> Result<(), FoldError> {
        if self.phase != FoldPhase::Advance {
            return Err(FoldError::InvalidPhase);
        }
        if next.scalar_type() != self.state_type {
            return Err(FoldError::StateTypeMismatch);
        }
        if !next.is_bounded() {
            return Err(FoldError::UnboundedState);
        }
        // Only an item from the private, bounded iterator admits this advance.
        self.completed += 1;
        self.phase = FoldPhase::NextItem;
        Ok(())
    }
}

impl std::fmt::Debug for FoldCursor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FoldCursor")
            .field("state_type", &self.state_type)
            .field("item_count", &self.item_count)
            .field("remaining_items", &self.items.len())
            .field("completed_iterations", &self.completed)
            .field("phase", &self.phase)
            .finish()
    }
}

/// Fixed, payload-free fold failures; the adapter owns fault codes and recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoldError {
    InvalidLimit,
    UnboundedItems,
    IterationLimit,
    InvalidPhase,
    StateTypeMismatch,
    UnboundedState,
}

impl std::fmt::Display for FoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "fold limit exceeds 64 entries",
            Self::UnboundedItems => "fold items exceed 64 entries or 4096 bytes",
            Self::IterationLimit => "fold iteration limit exhausted",
            Self::InvalidPhase => "fold operation violates item/advance order",
            Self::StateTypeMismatch => "fold next must preserve the initial state's scalar type",
            Self::UnboundedState => "fold next state exceeds its text or list bounds",
        })
    }
}

impl std::error::Error for FoldError {}
