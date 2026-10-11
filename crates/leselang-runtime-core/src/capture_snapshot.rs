use std::fmt;

use crate::{
    CapturedBindings, ProjectedBinding, ProjectedGroupBinding, ScalarBinding, ScalarValue,
    ScopeFrame,
};

/// Explicit inclusive storage ceilings, not native schema or authority admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureSnapshotLimits {
    pub max_bindings: usize,
    pub max_name_bytes: usize,
}

/// One native-prepared row; payloads are moved into their corresponding arrays.
/// No native traits or serialization are required to construct this value.
pub enum CapturedValue<ResultData, Group> {
    Scalar(ScalarValue),
    Result(ResultData),
    Group(Group),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureSnapshotSite {
    Binding { index: usize },
    Body,
}

/// Diagnostics retain positions without formatting names, values or native errors.
#[derive(Eq, PartialEq)]
pub enum CaptureSnapshotError<External> {
    BindingCount,
    PendingName,
    BindingName {
        index: usize,
    },
    Native {
        site: CaptureSnapshotSite,
        error: External,
    },
}

impl<External> fmt::Display for CaptureSnapshotError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BindingCount => "capture snapshot binding count exceeds policy",
            Self::PendingName => "capture snapshot pending name exceeds byte policy",
            Self::BindingName { .. } => "capture snapshot binding name exceeds byte policy",
            Self::Native { .. } => "native capture snapshot hook rejected",
        })
    }
}

impl<External> fmt::Debug for CaptureSnapshotError<External> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingCount => f.write_str("BindingCount"),
            Self::PendingName => f.write_str("PendingName"),
            Self::BindingName { index } => {
                f.debug_struct("BindingName").field("index", index).finish()
            }
            Self::Native { site, .. } => f
                .debug_struct("Native")
                .field("site", site)
                .finish_non_exhaustive(),
        }
    }
}

impl<External> std::error::Error for CaptureSnapshotError<External> {}

pub struct CaptureSnapshotHooks<Row, Body> {
    pub row: Row,
    pub body: Body,
}

/// Snapshot the complete visible lexical prefix in original insertion order.
///
/// Count and every name's byte length are preflighted before allocation or hooks.
/// A row hook runs once per binding, then its original name is copied and the
/// native-prepared value is moved into the scalar/result/group array. Within each
/// array, original order is retained. The body hook runs once after all rows.
/// The source scope stays borrowed and unchanged; partial arrays are not exposed.
///
/// One original mutable context is sequentially reborrowed, including for the
/// body. It may be unsized or GUI-local; no Clone/Debug/serde/Default/Send/Sync
/// bounds, new meter or implicit accounting are added. Native hooks own scalar
/// copying, projection snapshots, body materialization and exact charge order.
/// Successful charges and native mutations survive refusal/unwind without refund.
///
/// Partial rows, uncalled body and remaining row-hook captures are released while
/// native outputs/errors are locally owned. Native callback-internal cleanup and
/// destructors remain trusted; unwind propagates and a second panic may abort.
/// This does not validate name grammar/uniqueness, scalar bounds, native schemas,
/// types, authentic replies or durable authority. Adapters bound native payloads
/// and callback work; a successful snapshot is mutable data, not an execution
/// grant or a replayable handle. No dispatch, rollback or serialization occurs.
///
/// ```
/// use leselang_runtime_core::{CaptureSnapshotHooks, CaptureSnapshotLimits, CapturedValue,
///     Fuel, FuelExhausted, ScalarValue, ScopeFrame, snapshot_captured_scope_state};
/// let mut rows = vec![("n", ScalarValue::Integer(7))];
/// let scope = ScopeFrame::new(&mut rows);
/// let mut fuel = Fuel::new(2);
/// let saved = snapshot_captured_scope_state("reply", &(), &scope, &mut fuel,
///     CaptureSnapshotLimits { max_bindings: 1, max_name_bytes: 5 },
///     CaptureSnapshotHooks {
///         row: |_: &str, v: &ScalarValue, f: &mut Fuel| {
///             f.charge(1)?; Ok::<CapturedValue<(), ()>, FuelExhausted>(CapturedValue::Scalar(v.clone()))
///         },
///         body: |_: &(), f: &mut Fuel| { f.charge(1)?; Ok(()) },
///     }).unwrap();
/// assert_eq!(saved.locals[0].value, ScalarValue::Integer(7));
/// assert_eq!(fuel.remaining(), 0);
/// assert_eq!(scope.len(), 1);
/// ```
///
/// A temporary context reborrow cannot escape in the saved body:
///
/// ```compile_fail
/// use leselang_runtime_core::{CaptureSnapshotHooks, CaptureSnapshotLimits, CapturedValue,
///     ScopeFrame, snapshot_captured_scope_state};
/// let mut rows: Vec<(&str, ())> = vec![];
/// let scope = ScopeFrame::new(&mut rows);
/// let mut state = String::from("mutable context");
/// let dangling = snapshot_captured_scope_state("r", &(), &scope, &mut state,
///     CaptureSnapshotLimits { max_bindings: 0, max_name_bytes: 1 }, CaptureSnapshotHooks {
///         row: |_: &str, _: &(), _: &mut String| Ok::<CapturedValue<(), ()>, ()>(CapturedValue::Scalar(leselang_runtime_core::ScalarValue::None)),
///         body: |_: &(), s: &mut String| Ok::<_, ()>(s.as_str()),
///     });
/// ```
pub fn snapshot_captured_scope_state<
    'scope,
    'names,
    'source,
    Source: ?Sized,
    Value,
    Body,
    ResultData,
    Group,
    External,
    State: ?Sized,
    Row,
    BodyHook,
>(
    name: &str,
    source: &'source Source,
    scope: &'scope ScopeFrame<'_, 'names, Value>,
    state: &mut State,
    limits: CaptureSnapshotLimits,
    mut hooks: CaptureSnapshotHooks<Row, BodyHook>,
) -> Result<CapturedBindings<Body, ResultData, Group>, CaptureSnapshotError<External>>
where
    Row: FnMut(
        &'names str,
        &'scope Value,
        &mut State,
    ) -> Result<CapturedValue<ResultData, Group>, External>,
    BodyHook: FnOnce(&'source Source, &mut State) -> Result<Body, External>,
{
    let preflight = if scope.len() > limits.max_bindings {
        Err(CaptureSnapshotError::BindingCount)
    } else if name.len() > limits.max_name_bytes {
        Err(CaptureSnapshotError::PendingName)
    } else if let Some(index) = scope
        .bindings()
        .iter()
        .position(|(name, _)| name.len() > limits.max_name_bytes)
    {
        Err(CaptureSnapshotError::BindingName { index })
    } else {
        Ok(())
    };
    if let Err(error) = preflight {
        let output = Err(error);
        drop(hooks);
        return output;
    }

    let mut locals = Vec::with_capacity(scope.len());
    let mut results = Vec::new();
    let mut groups = Vec::new();
    // The fallible closure borrows hooks/arrays, so uncalled FnOnce cleanup cannot
    // unwind after an opaque error has already moved into its return place.
    let prepared = (|| {
        for (index, (name, value)) in scope.bindings().iter().enumerate() {
            let value =
                (hooks.row)(name, value, state).map_err(|error| CaptureSnapshotError::Native {
                    site: CaptureSnapshotSite::Binding { index },
                    error,
                })?;
            match value {
                CapturedValue::Scalar(value) => locals.push(ScalarBinding {
                    name: (*name).to_owned(),
                    value,
                }),
                CapturedValue::Result(result) => results.push(ProjectedBinding {
                    name: (*name).to_owned(),
                    result,
                }),
                CapturedValue::Group(group) => groups.push(ProjectedGroupBinding {
                    name: (*name).to_owned(),
                    group,
                }),
            }
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        let output = Err(error);
        drop((hooks.body, locals, results, groups, hooks.row));
        return output;
    }

    let output = match (hooks.body)(source, state) {
        Ok(body) => Ok(CapturedBindings {
            name: name.to_owned(),
            locals,
            results,
            groups,
            body,
        }),
        Err(error) => {
            let output = Err(CaptureSnapshotError::Native {
                site: CaptureSnapshotSite::Body,
                error,
            });
            drop((locals, results, groups));
            output
        }
    };
    drop(hooks.row);
    output
}
