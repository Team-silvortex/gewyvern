use crate::ScopeFrame;

/// Enter a complete owned lexical prefix with one original mutable context.
///
/// The body sees the prefix and may create nested/suffix bindings. Its owned
/// output/error stays local until suffix and then prefix cleanup finish. This
/// prevents a native destructor unwind from leaking an already returned value.
/// All incoming bindings are consumed, including on refusal/unwind; no capture,
/// accepted reply or execution authority is inferred from them.
///
/// Keys remain borrowed, values are moved, and a context may be unsized or GUI-local.
/// No Clone/Debug/Default/serde/Send/Sync bounds or internal mutable wrappers are
/// added. Native work, Drop and callback-internal cleanup remain trusted. Unwind
/// propagates; a second cleanup panic may abort, and forgetting a scope can bypass
/// suffix cleanup. Hosts admit names, duplicates, types, size and authority first.
/// They own charges on the original meter: no new budget, refund, rollback,
/// dispatch, retry or serialization is introduced.
///
/// ```
/// use leselang_runtime_core::{Fuel, FuelExhausted, ScopeFrame, with_owned_scope_state};
/// let mut fuel = Fuel::new(2);
/// let answer = with_owned_scope_state(vec![("prefix", 7)], &mut fuel,
///     |scope: &mut ScopeFrame<'_, '_, u64>, fuel: &mut Fuel| {
///         fuel.charge(1)?;
///         scope.push("local", 3).unwrap();
///         fuel.charge(1)?;
///         Ok::<_, FuelExhausted>(*scope.get("prefix").unwrap() + *scope.get("local").unwrap())
///     }).unwrap();
/// assert_eq!(answer, 10);
/// assert_eq!(fuel.remaining(), 0);
/// ```
///
/// Borrowed references to scope-owned values cannot escape:
///
/// ```compile_fail
/// use leselang_runtime_core::{ScopeFrame, with_owned_scope_state};
/// let dangling = with_owned_scope_state(vec![("owned", String::from("payload"))], &mut (),
///     |scope: &mut ScopeFrame<'_, '_, String>, _: &mut ()| Ok::<_, ()>(scope.get("owned")));
/// ```
///
/// Temporary mutable context reborrows cannot escape either:
///
/// ```compile_fail
/// use leselang_runtime_core::{ScopeFrame, with_owned_scope_state};
/// let mut state = String::from("mutable context");
/// let dangling = with_owned_scope_state(vec![("n", 1)], &mut state,
///     |_: &mut ScopeFrame<'_, '_, u64>, s: &mut String| Ok::<_, ()>(s.as_str()));
/// ```
pub fn with_owned_scope_state<'names, Value, State: ?Sized, Output, External>(
    mut bindings: Vec<(&'names str, Value)>,
    state: &mut State,
    body: impl FnOnce(&mut ScopeFrame<'_, 'names, Value>, &mut State) -> Result<Output, External>,
) -> Result<Output, External> {
    let mut scope = ScopeFrame::new(&mut bindings);
    let output = body(&mut scope, state);
    drop(scope);
    drop(bindings);
    output
}
