use serde::{Deserialize, Serialize};

use crate::ScalarValue;

/// Owned scalar data saved by an adapter's lexical capture.
///
/// Construction and serialization do not validate the name or direct value.
/// Decoding retains the scalar decoder's existing bounds; the adapter still
/// validates lexical scope, total ingress and directly constructed values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarBinding {
    pub name: String,
    pub value: ScalarValue,
}

/// Adapter-defined result data associated with its original lexical name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedBinding<Result> {
    pub name: String,
    pub result: Result,
}

/// Adapter-defined group data associated with its original lexical name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedGroupBinding<Group> {
    pub name: String,
    pub group: Group,
}

/// Ordered named members, without a scheduler or a successful-result certificate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedGroup<Result> {
    pub members: Vec<ProjectedBinding<Result>>,
}

/// Owned captured lexical data and an opaque adapter-defined enclosing body.
///
/// These are mutable data, not a resumable handle, authentic reply, validated
/// frame, authority grant or durable recovery protocol. Construction does not
/// interpret the body or result/group schema, infer a version, normalize names,
/// reorder members, validate bounds or execute any host work. Adapters validate
/// the entire frame against their original program/schema/authority before use.
/// Explicit shared structural validators can check counts, name byte lengths,
/// scalar bounds and group uniqueness with native hooks, not execution authority.
/// They also own ingress bytes, aggregate counts, native resource lifetime and
/// restart/replay policy; public constructors remain unchecked for compatibility.
///
/// The optional result/group arrays default to empty and are omitted on encoding
/// when empty, retaining the reference VM's released wire shape. An absent body
/// or scalar-local array is not an implicit default. Generic types need no
/// Clone/Debug/Default/serde/Send/Sync bounds to construct or move this data;
/// derived implementations and thread ownership are conditional on those types.
/// Debug/serialization invoke their native implementations and are not redacted.
///
/// ```
/// use leselang_runtime_core::{CapturedBindings, ScalarBinding, ScalarValue};
/// struct NativeBody;
/// struct NativeResult;
/// struct NativeGroup;
/// let frame: CapturedBindings<NativeBody, NativeResult, NativeGroup> = CapturedBindings {
///     name: "reply".into(),
///     locals: vec![ScalarBinding { name: "count".into(), value: ScalarValue::Integer(1) }],
///     results: vec![],
///     groups: vec![],
///     body: NativeBody,
/// };
/// let original_body = frame.body;
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::CapturedBindings;
/// struct Native;
/// fn duplicate(frame: CapturedBindings<Native, (), ()>) {
///     let copied = frame.clone();
/// }
/// ```
///
/// ```compile_fail
/// use std::rc::Rc;
/// use leselang_runtime_core::CapturedBindings;
/// fn requires_send<T: Send>(_: T) {}
/// let frame: CapturedBindings<Rc<()>, (), ()> = CapturedBindings {
///     name: "reply".into(), locals: vec![], results: vec![], groups: vec![], body: Rc::new(()),
/// };
/// requires_send(frame);
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        deserialize = "Body: Deserialize<'de>, Result: Deserialize<'de>, Group: Deserialize<'de>"
    )
)]
pub struct CapturedBindings<Body, Result, Group> {
    pub name: String,
    pub locals: Vec<ScalarBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<ProjectedBinding<Result>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ProjectedGroupBinding<Group>>,
    pub body: Body,
}
