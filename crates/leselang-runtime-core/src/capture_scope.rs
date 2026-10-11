use std::fmt;

use crate::{CapturedBindings, ScalarValue, ScopeError, ScopeFrame};

/// Original position in saved-scope assembly, without lexical or native payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureScopeSite {
    Scalar { index: usize },
    Result { index: usize },
    Group { index: usize },
    Pending,
    Body,
}

/// Active-name collision or an unchanged, opaque adapter-owned failure.
/// Debug/Display never format submitted names, values or native errors.
#[derive(Eq, PartialEq)]
pub enum CaptureScopeError<External> {
    Duplicate {
        site: CaptureScopeSite,
    },
    Native {
        site: CaptureScopeSite,
        error: External,
    },
}

impl<External> fmt::Display for CaptureScopeError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Duplicate { .. } => "captured scope duplicates an active name",
            Self::Native { .. } => "native captured scope hook rejected",
        })
    }
}

impl<External> fmt::Debug for CaptureScopeError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { site } => f.debug_struct("Duplicate").field("site", site).finish(),
            Self::Native { site, .. } => f
                .debug_struct("Native")
                .field("site", site)
                .finish_non_exhaustive(),
        }
    }
}

impl<External> std::error::Error for CaptureScopeError<External> {}

/// Explicit preparation and entry hooks for one caller-owned mutable context.
/// Construction does not execute hooks or admit a capture. Pending/body are FnOnce
/// at entry; native closure data require no Clone/Debug/serde/Default/Send/Sync.
pub struct CaptureScopeHooks<Scalar, Result, Group, Pending, Body> {
    pub scalar: Scalar,
    pub result: Result,
    pub group: Group,
    pub pending: Pending,
    pub body: Body,
}

/// Assemble borrowed saved names in one temporary lexical frame, then enter a
/// trusted native body hook with the complete still-live scope.
///
/// Order is all scalar rows, result rows, group rows, pending name, then body.
/// Each native value hook runs once before that row's duplicate check, retaining
/// legacy preparation-before-insertion order. Refusal stops before later hooks;
/// names are not preflighted in a second pass. Values are moved, names borrowed,
/// and no Clone/Debug/serde/Default/Send/Sync bound is added to native types.
///
/// Existing prefix values stay parent-owned. The guard restores its original
/// suffix boundary on success, error and unwind; nested body scopes use the same
/// guard. Body output and original opaque errors move out without serialization.
/// Native callback/interior mutations are not rolled back. Drop is trusted:
/// destructor panics propagate and a second cleanup panic can abort the process.
/// Explicitly forgetting an exposed guard can bypass its cleanup.
///
/// This does not validate existing prefix uniqueness, names, counts, scalar
/// bounds, native field/type schemas, receipts or execution/durable authority.
/// Adapters must bound all incoming storage and callback work, validate captures
/// before entry and charge their original meter. Push scans the visible prefix.
/// The body hook is native code, not an interpreter or an automatic execution
/// grant; no hidden dispatch, fuel creation/refill, normalization or replay occurs.
///
/// ```
/// use leselang_runtime_core::{CapturedBindings, ScalarBinding, ScalarValue, ScopeFrame,
///     with_captured_scope};
/// let capture: CapturedBindings<(), (), ()> = CapturedBindings {
///     name: "reply".into(),
///     locals: vec![ScalarBinding { name: "n".into(), value: ScalarValue::Integer(7) }],
///     results: vec![], groups: vec![], body: (),
/// };
/// let mut bindings = vec![("parent", ScalarValue::None)];
/// let mut parent = ScopeFrame::new(&mut bindings);
/// let answer = with_captured_scope(&capture, &mut parent,
///     |value| Ok::<_, ()>(value.clone()),
///     |_| Ok(ScalarValue::None), |_| Ok(ScalarValue::None),
///     || Ok(ScalarValue::Boolean(true)),
///     |_, scope| Ok(scope.get("n").cloned()));
/// assert_eq!(answer.unwrap(), Some(ScalarValue::Integer(7)));
/// drop(parent);
/// assert_eq!(bindings, [("parent", ScalarValue::None)]);
/// ```
///
/// Temporary owned scope values cannot escape through borrowed output:
///
/// ```compile_fail
/// use leselang_runtime_core::{CapturedBindings, ScopeFrame, with_captured_scope};
/// let capture: CapturedBindings<(), (), ()> = CapturedBindings {
///     name: "reply".into(), locals: vec![], results: vec![], groups: vec![], body: (),
/// };
/// let mut bindings = vec![];
/// let mut parent = ScopeFrame::new(&mut bindings);
/// let dangling = with_captured_scope(&capture, &mut parent,
///     |_| Ok::<_, ()>(0_u64), |_| Ok(0), |_| Ok(0), || Ok(7),
///     |_, scope| Ok(scope.get("reply").unwrap()));
/// ```
pub fn with_captured_scope<'data, Body, ResultData, Group, Value, Output, External>(
    capture: &'data CapturedBindings<Body, ResultData, Group>,
    parent: &mut ScopeFrame<'_, 'data, Value>,
    mut scalar: impl FnMut(&'data ScalarValue) -> Result<Value, External>,
    mut result: impl FnMut(&'data ResultData) -> Result<Value, External>,
    mut group: impl FnMut(&'data Group) -> Result<Value, External>,
    pending: impl FnOnce() -> Result<Value, External>,
    body: impl FnOnce(&'data Body, &mut ScopeFrame<'_, 'data, Value>) -> Result<Output, External>,
) -> Result<Output, CaptureScopeError<External>> {
    let output = with_captured_scope_state(
        capture,
        parent,
        &mut (),
        CaptureScopeHooks {
            scalar: |value, _: &mut ()| scalar(value),
            result: |value, _: &mut ()| result(value),
            group: |value, _: &mut ()| group(value),
            pending: |_: &mut ()| pending(),
            body: |value, scope: &mut ScopeFrame<'_, 'data, Value>, _: &mut ()| body(value, scope),
        },
    );
    drop((scalar, result, group));
    output
}

/// Enter a captured scope using the same mutable context in every ordered hook.
///
/// The closure-only entry delegates here with unit state. This has the same
/// preparation-before-duplicate ordering, borrowed names and cleanup semantics.
/// A context may be unsized, move-only or GUI-thread-local. Hooks receive sequential
/// reborrows of the original object, without cloning, resetting, serializing or
/// wrapping it in an internal mutable container. There is no implicit accounting:
/// hosts charge the original meter before native work, including body evaluation.
/// Successful charges and native context mutations survive later refusal/unwind;
/// scope cleanup is not fuel refund, mutation rollback or replay permission.
/// Native work and Drop remain trusted. Input/schema/authority admission is still
/// adapter-owned; a supplied context is not a saved execution frame or grant.
/// Uncalled once hooks and remaining mutable hook captures are explicitly released
/// while outputs/errors are still local. Native callback-internal cleanup remains
/// the callback's responsibility; a second cleanup panic may abort.
///
/// ```
/// use leselang_runtime_core::{CaptureScopeHooks, CapturedBindings, Fuel, ScalarBinding,
///     ScalarValue, ScopeFrame, with_captured_scope_state};
/// let capture: CapturedBindings<(), (), ()> = CapturedBindings {
///     name: "reply".into(),
///     locals: vec![ScalarBinding { name: "n".into(), value: ScalarValue::Integer(7) }],
///     results: vec![], groups: vec![], body: (),
/// };
/// let mut values = vec![];
/// let mut fuel = Fuel::new(3); // Explicit host grant, not scope-entry policy.
/// let output = with_captured_scope_state(&capture, &mut ScopeFrame::new(&mut values),
///     &mut fuel, CaptureScopeHooks {
///         scalar: |v: &ScalarValue, f: &mut Fuel| {
///             f.charge(1)?; Ok::<_, leselang_runtime_core::FuelExhausted>(v.clone())
///         },
///         result: |_: &(), _: &mut Fuel| Ok(ScalarValue::None),
///         group: |_: &(), _: &mut Fuel| Ok(ScalarValue::None),
///         pending: |_: &mut Fuel| Ok(ScalarValue::Boolean(true)),
///         body: |_: &(), scope: &mut ScopeFrame<'_, '_, ScalarValue>, f: &mut Fuel| {
///             f.charge(1)?; Ok(scope.get("n").cloned())
///         },
///     }).unwrap();
/// assert_eq!(output, Some(ScalarValue::Integer(7)));
/// assert_eq!(fuel.remaining(), 1);
/// assert!(values.is_empty());
/// ```
///
/// A hook's temporary mutable context reborrow cannot escape through output:
///
/// ```compile_fail
/// use leselang_runtime_core::{CaptureScopeHooks, CapturedBindings, ScopeFrame,
///     with_captured_scope_state};
/// let capture: CapturedBindings<(), (), ()> = CapturedBindings {
///     name: "reply".into(), locals: vec![], results: vec![], groups: vec![], body: (),
/// };
/// let mut values = vec![];
/// let mut context = String::from("private mutable state");
/// let dangling = with_captured_scope_state(&capture, &mut ScopeFrame::new(&mut values),
///     &mut context, CaptureScopeHooks {
///         scalar: |_: &_, _: &mut String| Ok::<(), ()>(()),
///         result: |_: &(), _: &mut String| Ok(()),
///         group: |_: &(), _: &mut String| Ok(()),
///         pending: |_: &mut String| Ok(()),
///         body: |_: &(), _: &mut ScopeFrame<'_, '_, ()>, c: &mut String| Ok(c.as_str()),
///     });
/// ```
pub fn with_captured_scope_state<
    'data,
    Body,
    ResultData,
    Group,
    Value,
    Output,
    External,
    State: ?Sized,
    Scalar,
    ResultHook,
    GroupHook,
    Pending,
    BodyHook,
>(
    capture: &'data CapturedBindings<Body, ResultData, Group>,
    parent: &mut ScopeFrame<'_, 'data, Value>,
    state: &mut State,
    mut hooks: CaptureScopeHooks<Scalar, ResultHook, GroupHook, Pending, BodyHook>,
) -> Result<Output, CaptureScopeError<External>>
where
    Scalar: FnMut(&'data ScalarValue, &mut State) -> Result<Value, External>,
    ResultHook: FnMut(&'data ResultData, &mut State) -> Result<Value, External>,
    GroupHook: FnMut(&'data Group, &mut State) -> Result<Value, External>,
    Pending: FnOnce(&mut State) -> Result<Value, External>,
    BodyHook: FnOnce(
        &'data Body,
        &mut ScopeFrame<'_, 'data, Value>,
        &mut State,
    ) -> Result<Output, External>,
{
    let mut scope = parent.nested();
    // Retain output/errors locally until native scope cleanup finishes.
    let output = (|| {
        for (index, local) in capture.locals.iter().enumerate() {
            let site = CaptureScopeSite::Scalar { index };
            let value = (hooks.scalar)(&local.value, state)
                .map_err(|error| CaptureScopeError::Native { site, error })?;
            push(&mut scope, &local.name, value, site)?;
        }
        for (index, saved) in capture.results.iter().enumerate() {
            let site = CaptureScopeSite::Result { index };
            let value = (hooks.result)(&saved.result, state)
                .map_err(|error| CaptureScopeError::Native { site, error })?;
            push(&mut scope, &saved.name, value, site)?;
        }
        for (index, saved) in capture.groups.iter().enumerate() {
            let site = CaptureScopeSite::Group { index };
            let value = (hooks.group)(&saved.group, state)
                .map_err(|error| CaptureScopeError::Native { site, error })?;
            push(&mut scope, &saved.name, value, site)?;
        }
        Ok(())
    })();
    // Uncalled FnOnce captures stay outside the fallible closure until its error is local.
    let output = match output {
        Err(error) => {
            drop(hooks.pending);
            drop(hooks.body);
            Err(error)
        }
        Ok(()) => {
            let site = CaptureScopeSite::Pending;
            match (hooks.pending)(state) {
                Err(error) => {
                    drop(hooks.body);
                    Err(CaptureScopeError::Native { site, error })
                }
                Ok(value) => match push(&mut scope, &capture.name, value, site) {
                    Err(error) => {
                        drop(hooks.body);
                        Err(error)
                    }
                    Ok(()) => (hooks.body)(&capture.body, &mut scope, state).map_err(|error| {
                        CaptureScopeError::Native {
                            site: CaptureScopeSite::Body,
                            error,
                        }
                    }),
                },
            }
        }
    };
    drop(scope);
    drop((hooks.scalar, hooks.result, hooks.group));
    output
}

fn push<'data, Value, External>(
    scope: &mut ScopeFrame<'_, 'data, Value>,
    name: &'data str,
    value: Value,
    site: CaptureScopeSite,
) -> Result<(), CaptureScopeError<External>> {
    scope
        .push(name, value)
        .map(|_| ())
        .map_err(|error| match error {
            ScopeError::DuplicateBinding => CaptureScopeError::Duplicate { site },
        })
}
