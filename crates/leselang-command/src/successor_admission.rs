//! Complete product successor admission, without execution or recovery authority.

use crate::{
    ContinuationAdmissionLimits, EffectRequestView, RequestAdmissionError,
    SuccessorCorrespondenceError, Value, validate_effect_request,
    validate_preflighted_successor_pair,
};

/// Trusted native policies, evaluated against one stable snapshot.
///
/// Tokens and failures need no Clone, Debug, serde, equality or thread traits.
/// Identity projections must identify the original owners; canonical sequences
/// must be nonzero, bounded and in the same native domain for both requests.
pub trait SuccessorAdmissionHost<Token> {
    type Error;

    fn group_owner_text<'a>(&self, token: &'a Token) -> &'a str;
    fn valid_token(&self, token: &Token) -> bool;
    fn canonical_sequence(&self, token: &Token) -> Option<u64>;
    fn valid_group_name(&self, name: &str) -> bool;

    /// Admit the original non-scalar completed value against the parent's result
    /// type, recursive bounds, output limit and native encoded-size policy, in
    /// that order. [`crate::ResultValueDomain`] can supply these checks. This is
    /// called once, only after both complete requests and the completion kind pass.
    fn validate_completed_value(
        &self,
        parent: EffectRequestView<'_, Token>,
        value: &Value,
    ) -> Result<(), Self::Error>;
}

/// Ordered provenance without copying requests or exposing native error payloads.
#[derive(Eq, PartialEq)]
pub enum SuccessorAdmissionError<Native> {
    Parent(RequestAdmissionError),
    Next(RequestAdmissionError),
    Completion,
    Value(Native),
    Correspondence(SuccessorCorrespondenceError),
}

impl<Native> std::fmt::Display for SuccessorAdmissionError<Native> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Parent(_) => "invalid successor parent request",
            Self::Next(_) => "invalid successor next request",
            Self::Completion => "successor requires a non-scalar completed value",
            Self::Value(_) => "invalid successor completed value",
            Self::Correspondence(_) => "invalid successor correspondence",
        })
    }
}

impl<Native> std::fmt::Debug for SuccessorAdmissionError<Native> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for SuccessorAdmissionError<Native> {}

/// Admit parent request, next request, completion kind, native value domain and
/// successor correspondence in the original reference order. `None` represents
/// any non-completed native state; scalar completions cannot create a successor.
///
/// All arguments borrow original data. Refusal skips subsequent gates and hooks;
/// native side effects/unwind propagate without retry or rollback. Hooks can run
/// again in later gates (notably canonical identity); they must observe the same
/// immutable policy snapshot. The entry adds no evaluation, fuel debit or codec.
///
/// Callers bound ingress, pending payloads and hook work. Passing is consistency,
/// not a validation certificate or an execution grant: authenticate receipt and
/// original authority/budgets, check live deadlines and leases, and atomically
/// settle journals outside this function. Recheck mutable or decoded data.
pub fn validate_successor_pair<Token, Host: SuccessorAdmissionHost<Token> + ?Sized>(
    parent: EffectRequestView<'_, Token>,
    next: EffectRequestView<'_, Token>,
    completed: Option<&Value>,
    limits: ContinuationAdmissionLimits,
    host: &Host,
) -> Result<(), SuccessorAdmissionError<Host::Error>> {
    use SuccessorAdmissionError::{
        Completion, Correspondence, Next, Parent, Value as InvalidValue,
    };

    validate_effect_request(
        EffectRequestView { ..parent },
        limits,
        |token| host.group_owner_text(token),
        |token| host.valid_token(token),
        |name| host.valid_group_name(name),
        |token| host.canonical_sequence(token),
    )
    .map_err(Parent)?;
    validate_effect_request(
        EffectRequestView { ..next },
        limits,
        |token| host.group_owner_text(token),
        |token| host.valid_token(token),
        |name| host.valid_group_name(name),
        |token| host.canonical_sequence(token),
    )
    .map_err(Next)?;
    let value = completed.ok_or(Completion)?;
    if matches!(value, Value::Scalar { .. }) {
        return Err(Completion);
    }
    host.validate_completed_value(EffectRequestView { ..parent }, value)
        .map_err(InvalidValue)?;
    validate_preflighted_successor_pair(
        parent,
        next,
        value,
        limits.binding,
        |token| host.group_owner_text(token),
        |token| host.canonical_sequence(token),
        |name| host.valid_group_name(name),
    )
    .map_err(Correspondence)
}
