use crate::{MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, ScalarError, StringListValue};

/// Incremental bounded language data construction, not an execution budget.
///
/// Entries keep source order and aggregate UTF-8 byte bounds. Failed pushes
/// leave the existing prefix unchanged; owned rejected text is consumed/dropped.
/// Borrowed text is checked before copying. `Debug` exposes counts, not payloads.
/// Empty/default builders grant no fuel, authority, persistence or host work.
/// The finished legacy value is publicly mutable, so later ingress must validate
/// it again. Logical bounds do not bound upstream allocation or retained capacity.
///
/// ```
/// use leselang_runtime_core::{StringListBuilder, StringListValue};
/// let mut list = StringListBuilder::with_capacity(2).unwrap();
/// list.try_push("first".into()).unwrap();
/// list.try_push_text("").unwrap();
/// assert_eq!(list.finish(), StringListValue(vec!["first".into(), "".into()]));
/// ```
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_runtime_core::StringListBuilder;
/// StringListBuilder::new();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::StringListBuilder;
/// let mut list = StringListBuilder::new();
/// list.items.push("unchecked".into());
/// ```
#[must_use = "finish the bounded list or explicitly discard its prefix"]
pub struct StringListBuilder {
    items: Vec<String>,
    remaining: usize,
}

impl StringListBuilder {
    /// Empty construction does not allocate or reserve from an external length hint.
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            remaining: MAX_SCALAR_STRING_BYTES,
        }
    }

    /// Explicit bounded preallocation; oversized requests fail before allocation.
    pub fn with_capacity(expected_items: usize) -> Result<Self, ScalarError> {
        if expected_items > MAX_STRING_LIST_ITEMS {
            return Err(ScalarError::StringListLimit);
        }
        Ok(Self {
            items: Vec::with_capacity(expected_items),
            remaining: MAX_SCALAR_STRING_BYTES,
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub const fn remaining_bytes(&self) -> usize {
        self.remaining
    }

    /// Move one owned text buffer, without cloning; rejection consumes the input.
    pub fn try_push(&mut self, item: String) -> Result<(), ScalarError> {
        let remaining = self.after_entry(item.len())?;
        self.items.push(item);
        self.remaining = remaining;
        Ok(())
    }

    /// Check count/bytes before allocating a copy of borrowed text.
    pub fn try_push_text(&mut self, item: &str) -> Result<(), ScalarError> {
        let remaining = self.after_entry(item.len())?;
        self.items.push(item.to_owned());
        self.remaining = remaining;
        Ok(())
    }

    #[must_use = "use the finished data; it is not an execution receipt or authority"]
    pub fn finish(self) -> StringListValue {
        StringListValue(self.items)
    }

    fn after_entry(&self, bytes: usize) -> Result<usize, ScalarError> {
        if self.items.len() == MAX_STRING_LIST_ITEMS {
            return Err(ScalarError::StringListLimit);
        }
        self.remaining
            .checked_sub(bytes)
            .ok_or(ScalarError::StringListLimit)
    }
}

impl Default for StringListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<StringListValue> for StringListBuilder {
    type Error = ScalarError;

    /// Validate and adopt an existing vector without cloning or changing capacity.
    /// Invalid input is consumed, with no partial builder or payload in the error.
    fn try_from(value: StringListValue) -> Result<Self, Self::Error> {
        if value.0.len() > MAX_STRING_LIST_ITEMS {
            return Err(ScalarError::UnboundedOperand);
        }
        let remaining = value
            .0
            .iter()
            .try_fold(MAX_SCALAR_STRING_BYTES, |remaining, item| {
                remaining.checked_sub(item.len())
            })
            .ok_or(ScalarError::UnboundedOperand)?;
        Ok(Self {
            items: value.0,
            remaining,
        })
    }
}

impl std::fmt::Debug for StringListBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StringListBuilder")
            .field("items", &self.len())
            .field("bytes", &(MAX_SCALAR_STRING_BYTES - self.remaining))
            .finish()
    }
}
