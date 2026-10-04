/// Borrowed lexical frame for adapter-owned values, not a saved execution frame.
///
/// Keys borrow immutable names; no name/value is cloned on insertion. Reads see
/// the entire visible prefix, but indexed mutation and popping stay in this frame.
/// Active duplicates are rejected without changing the prefix. On drop, all new
/// bindings are detached before their values are released, normally in reverse
/// insertion order. Destructor panics propagate; remaining drops follow Rust's
/// unwinding rules, and another panic during cleanup can abort the process.
///
/// The adapter validates names, types, authority and aggregate size before use,
/// and owns evaluation, fuel and durable snapshots. Existing prefix uniqueness
/// is not validated by construction. Names/values exposed by `bindings` are data,
/// not redacted observations; `Debug` reports counts without formatting either.
/// No evaluation callback, global lock or scheduling is added. Value destructors
/// remain adapter code. As with any Rust drop guard,
/// explicitly forgetting it bypasses cleanup; this is not an authority fence.
///
/// ```
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings = vec![("outer", 1)];
/// {
///     let mut frame = ScopeFrame::new(&mut bindings);
///     let slot = frame.push("local", 2).unwrap();
///     assert_eq!(frame.get("outer"), Some(&1));
///     *frame.get_local_mut(slot).unwrap() = 3;
///     assert_eq!(frame.pop(), Some(("local", 3)));
/// }
/// assert_eq!(bindings, [("outer", 1)]);
/// ```
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings: Vec<(&str, u64)> = vec![];
/// ScopeFrame::new(&mut bindings);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings: Vec<(&str, u64)> = vec![];
/// let mut frame = ScopeFrame::new(&mut bindings);
/// bindings.clear();
/// frame.push("local", 1).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings = vec![];
/// let mut frame = ScopeFrame::new(&mut bindings);
/// {
///     let name = String::from("short-lived");
///     frame.push(&name, 1).unwrap();
/// }
/// assert_eq!(frame.get("short-lived"), Some(&1));
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings: Vec<(&str, u64)> = vec![];
/// let frame = ScopeFrame::new(&mut bindings);
/// let duplicate = frame.clone();
/// ```
///
/// ```compile_fail
/// use std::{rc::Rc, thread};
/// use leselang_runtime_core::ScopeFrame;
/// let mut bindings = vec![("gui", Rc::new(1))];
/// let frame = ScopeFrame::new(&mut bindings);
/// thread::scope(|threads| { threads.spawn(move || drop(frame)); });
/// ```
#[must_use = "retain the lexical frame until its bindings should be cleaned up"]
pub struct ScopeFrame<'scope, 'names, Value> {
    bindings: &'scope mut Vec<(&'names str, Value)>,
    base: usize,
}

impl<'scope, 'names, Value> ScopeFrame<'scope, 'names, Value> {
    /// Start after a trusted existing prefix without validating or taking its values.
    pub fn new(bindings: &'scope mut Vec<(&'names str, Value)>) -> Self {
        let base = bindings.len();
        Self { bindings, base }
    }

    pub fn nested(&mut self) -> ScopeFrame<'_, 'names, Value> {
        ScopeFrame::new(self.bindings)
    }

    /// Number of all visible bindings, including the parent prefix.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Number of bindings owned by this frame, excluding the parent prefix.
    pub fn local_len(&self) -> usize {
        self.bindings.len() - self.base
    }

    /// Visible names and payloads in insertion order, not redacted logging data.
    pub fn bindings(&self) -> &[(&'names str, Value)] {
        self.bindings
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == name)
            .map(|(_, value)| value)
    }

    /// Insert one active name and return a frame-relative slot, not replay authority.
    /// On rejection the supplied value is consumed/dropped, but the scope is unchanged.
    pub fn push(&mut self, name: &'names str, value: Value) -> Result<usize, ScopeError> {
        if self.get(name).is_some() {
            return Err(ScopeError::DuplicateBinding);
        }
        let slot = self.local_len();
        self.bindings.push((name, value));
        Ok(slot)
    }

    /// Frame-relative access cannot mutate a parent's binding, even for huge indices.
    /// Slots may become stale/reused after pop; callers own their trusted lifetime.
    pub fn get_local_mut(&mut self, slot: usize) -> Option<&mut Value> {
        let index = self.base.checked_add(slot)?;
        self.bindings.get_mut(index).map(|(_, value)| value)
    }

    /// Move out only the latest local value; an empty frame never pops its parent.
    pub fn pop(&mut self) -> Option<(&'names str, Value)> {
        if self.local_len() == 0 {
            return None;
        }
        self.bindings.pop()
    }
}

impl<Value> Drop for ScopeFrame<'_, '_, Value> {
    fn drop(&mut self) {
        // Drain detaches the entire suffix before any adapter-owned destructor runs.
        let mut locals = self.bindings.drain(self.base..);
        while let Some(local) = locals.next_back() {
            drop(local);
        }
    }
}

impl<Value> std::fmt::Debug for ScopeFrame<'_, '_, Value> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopeFrame")
            .field("visible_bindings", &self.len())
            .field("local_bindings", &self.local_len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeError {
    DuplicateBinding,
}

impl std::fmt::Display for ScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("lexical binding duplicates an active name")
    }
}

impl std::error::Error for ScopeError {}
