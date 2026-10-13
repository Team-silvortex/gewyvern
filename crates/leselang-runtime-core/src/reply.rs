use std::borrow::Borrow;
use std::fmt;

use crate::{HostResultDomain, HostResultError, LocalExecutionScope, validate_host_result};

struct AcceptanceFence<'scope>(Option<&'scope LocalExecutionScope>);

impl AcceptanceFence<'_> {
    fn is_closed(&self) -> bool {
        self.0.is_some_and(|scope| !scope.is_open())
    }
}

impl Drop for AcceptanceFence<'_> {
    fn drop(&mut self) {
        if let Some(scope) = self.0 {
            scope.close();
        }
    }
}

// Preserve the shared type-before-value validator, but a synchronous revocation
// inside matches_type must not enter validate_value or replace the original domain.
struct LiveResultDomain<'a, Domain: ?Sized> {
    domain: &'a Domain,
    scope: Option<&'a LocalExecutionScope>,
}
impl<View: ?Sized, Domain: HostResultDomain<View> + ?Sized> HostResultDomain<View>
    for LiveResultDomain<'_, Domain>
{
    type Error = Domain::Error;

    fn matches_type(&self, view: &View) -> bool {
        self.domain.matches_type(view) && self.scope.is_none_or(LocalExecutionScope::is_open)
    }

    fn validate_value(&self, view: &View) -> Result<(), Self::Error> {
        self.domain.validate_value(view)
    }
}

/// Trusted live policy for the exact pending identity, not script-supplied grants.
/// Correlation equality runs first; this callback runs before reply projection,
/// type or domain validation. Authentication, generation/revision/deadline and
/// lease checks remain host-owned. Native work may mutate or unwind; the core
/// cannot preempt or roll it back, and never retries a callback.
pub trait ReplyAuthority<Identity> {
    type Error;

    fn authorize(&self, identity: &Identity) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyEnd {
    Accepted,
    Cancelled,
    HostUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyStatus {
    Pending,
    Terminal(ReplyEnd),
}

pub enum ReplyAcceptanceError<AuthorityError, ResultError> {
    Closed(ReplyEnd),
    IdentityMismatch,
    Authority(AuthorityError),
    Result(HostResultError<ResultError>),
}

impl<AuthorityError, ResultError> fmt::Display
    for ReplyAcceptanceError<AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Closed(_) => "reply handle is terminal",
            Self::IdentityMismatch => "reply identity does not match pending state",
            Self::Authority(_) => "reply failed live host policy",
            Self::Result(HostResultError::TypeMismatch) => {
                "reply type does not match its declaration"
            }
            Self::Result(HostResultError::InvalidValue(_)) => {
                "reply failed native value validation"
            }
        })
    }
}

impl<AuthorityError, ResultError> fmt::Debug for ReplyAcceptanceError<AuthorityError, ResultError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<AuthorityError, ResultError> std::error::Error
    for ReplyAcceptanceError<AuthorityError, ResultError>
{
}

/// Rejected input returned unchanged, with native errors available only by matching.
/// Formatting prints a closed error tag, never the reply, identity or native error.
#[must_use = "handle the rejected reply without implicitly retrying it"]
pub struct RejectedReply<Reply, AuthorityError, ResultError> {
    pub reply: Reply,
    pub error: ReplyAcceptanceError<AuthorityError, ResultError>,
}

impl<Reply, AuthorityError, ResultError> fmt::Debug
    for RejectedReply<Reply, AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

/// Move-only accepted data for one owning host. It is not execution/replay authority.
/// The original declaration is borrowed; identity, frame and reply move unchanged.
/// Extraction/mutation and native interior state can invalidate observed checks.
///
/// ```compile_fail
/// use leselang_runtime_core::{AcceptedReply, ScalarTypeSet};
/// fn duplicate(reply: AcceptedReply<'_, u32, (), ScalarTypeSet, u8>) {
///     let duplicate = reply.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{AcceptedReply, ScalarTypeSet};
/// fn wire(reply: AcceptedReply<'_, u32, (), ScalarTypeSet, u8>) {
///     let encoded = serde_json::to_string(&reply).unwrap();
/// }
/// ```
#[must_use = "the owning host must consume accepted frame/reply data"]
pub struct AcceptedReply<'schema, Identity, Frame, Declaration: ?Sized, Reply> {
    identity: Identity,
    frame: Frame,
    declaration: &'schema Declaration,
    reply: Reply,
}

impl<'schema, Identity, Frame, Declaration: ?Sized, Reply>
    AcceptedReply<'schema, Identity, Frame, Declaration, Reply>
{
    /// Inspect immutable borrows without copying input, transferring ownership,
    /// validating current native policy or selecting a new continuation.
    pub fn as_parts(&self) -> (&Identity, &Frame, &'schema Declaration, &Reply) {
        (&self.identity, &self.frame, self.declaration, &self.reply)
    }

    pub fn into_parts(self) -> (Identity, Frame, &'schema Declaration, Reply) {
        (self.identity, self.frame, self.declaration, self.reply)
    }
}

impl<Identity, Frame, Declaration: ?Sized, Reply> fmt::Debug
    for AcceptedReply<'_, Identity, Frame, Declaration, Reply>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AcceptedReply")
    }
}

pub type ReplyAcceptanceResult<
    'schema,
    Identity,
    Frame,
    Declaration,
    Reply,
    AuthorityError,
    ResultError,
> = Result<
    AcceptedReply<'schema, Identity, Frame, Declaration, Reply>,
    RejectedReply<Reply, AuthorityError, ResultError>,
>;

struct Waiting<'schema, Identity, Frame, Declaration: ?Sized> {
    identity: Identity,
    frame: Frame,
    declaration: &'schema Declaration,
}

enum State<'schema, Identity, Frame, Declaration: ?Sized> {
    Waiting(Waiting<'schema, Identity, Frame, Declaration>),
    Terminal(ReplyEnd),
}

/// Single-handle in-memory reply ownership, not an authentic receipt or journal.
/// No Clone/serde/Debug/Send bounds are imposed on frame, identity or declaration.
/// Native identity equality must encode the exact host correlation/generation.
/// Success transfers the frame once; cancellation releases it before any retry.
/// Normal rejection returns input unchanged and retains the pending frame.
/// Native equality/policy/Borrow/type/domain unwind closes as HostUncertain and
/// releases the frame; it is never silently rearmed. Destructor unwind propagates;
/// a second panic during cleanup can abort. Status/Debug never inspect payloads.
///
/// The host bounds ingress, metadata/native work and pending-handle count, owns
/// live authority and the reply's provenance, and commits durable decisions.
/// Constructing a new handle for the same identity is not globally deduplicated.
/// This supplies neither an executor, fuel refill, clock, cancellation delivery
/// nor exactly-once external effects; native frames may themselves be borrowed.
#[must_use = "retain the pending reply handle until accepted, cancelled or deliberately dropped"]
pub struct PendingReply<'schema, Identity, Frame, Declaration: ?Sized> {
    state: State<'schema, Identity, Frame, Declaration>,
}

impl<'schema, Identity, Frame, Declaration: ?Sized>
    PendingReply<'schema, Identity, Frame, Declaration>
{
    pub fn new(identity: Identity, frame: Frame, declaration: &'schema Declaration) -> Self {
        Self {
            state: State::Waiting(Waiting {
                identity,
                frame,
                declaration,
            }),
        }
    }

    pub fn status(&self) -> ReplyStatus {
        match &self.state {
            State::Waiting(_) => ReplyStatus::Pending,
            State::Terminal(reason) => ReplyStatus::Terminal(*reason),
        }
    }

    /// Close before native frame/identity destructors run. Repeated cancellation
    /// does not change an existing terminal reason or invoke native callbacks.
    pub fn cancel(&mut self) -> bool {
        match std::mem::replace(&mut self.state, State::Terminal(ReplyEnd::Cancelled)) {
            State::Waiting(waiting) => {
                drop(waiting);
                true
            }
            State::Terminal(reason) => {
                self.state = State::Terminal(reason);
                false
            }
        }
    }

    /// Correlate, authorize, project a borrowed view, check type, then validate value.
    /// Each native callback runs at most once per explicit attempt. On normal
    /// rejection the caller receives the exact original reply; there is no retry,
    /// coercion, scalar recovery, value copy or execution. Successful handoff makes
    /// this handle terminal before its frame/reply can be used by the caller.
    /// The supplied identity and reply are independent arguments: equality does
    /// not authenticate the payload. Hosts must preserve their binding at ingress
    /// or use an original per-execution/member domain that validates provenance
    /// carried by the reply. A shared schema and generation alone cannot isolate
    /// concurrent executions of the same operation or compiled tree.
    pub fn try_accept<Reply, View: ?Sized, Authority>(
        &mut self,
        actual_identity: &Identity,
        reply: Reply,
        authority: &Authority,
    ) -> ReplyAcceptanceResult<
        'schema,
        Identity,
        Frame,
        Declaration,
        Reply,
        Authority::Error,
        Declaration::Error,
    >
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.try_accept_with_scope(actual_identity, reply, authority, None)
    }

    /// The same acceptance with an opt-in local unwind boundary. An already
    /// closed scope cancels pending state before querying native callbacks. An
    /// escaping equality/authority/Borrow/type/value unwind closes the scope
    /// before this handle's owned identity, frame and reply are dropped. Native
    /// callback-local cleanup still precedes this boundary; a second panic can abort.
    ///
    /// Recheck after each normal native callback. Synchronous scope closure stops
    /// later checks and seals Cancelled before retained cleanup, returning the
    /// original rejected input. Cancellation wins over the same callback's refusal;
    /// its superseded error is disposed only after sealing the terminal reason.
    /// Native cleanup can unwind and prevent input handoff, but cannot rearm it.
    /// Normal acceptance/rejection with an open scope leaves liveness unchanged.
    /// This does not
    /// bind the scope to an identity, authenticate a reply, or replace live host
    /// policy. The host must supply the owning execution's scope and still check
    /// provenance and current leases. Accepted-data disposal is outside this call.
    pub fn try_accept_in_scope<Reply, View: ?Sized, Authority>(
        &mut self,
        actual_identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: &LocalExecutionScope,
    ) -> ReplyAcceptanceResult<
        'schema,
        Identity,
        Frame,
        Declaration,
        Reply,
        Authority::Error,
        Declaration::Error,
    >
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        if !scope.is_open() {
            self.cancel();
        }
        self.try_accept_with_scope(actual_identity, reply, authority, Some(scope))
    }

    fn try_accept_with_scope<Reply, View: ?Sized, Authority>(
        &mut self,
        actual_identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: Option<&LocalExecutionScope>,
    ) -> ReplyAcceptanceResult<
        'schema,
        Identity,
        Frame,
        Declaration,
        Reply,
        Authority::Error,
        Declaration::Error,
    >
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        // Close first: native equality/Borrow/validators may unwind after changing
        // their own state. An interrupted attempt must not retain a retryable frame.
        let waiting =
            match std::mem::replace(&mut self.state, State::Terminal(ReplyEnd::HostUncertain)) {
                State::Waiting(waiting) => waiting,
                State::Terminal(reason) => {
                    self.state = State::Terminal(reason);
                    return Err(RejectedReply {
                        reply,
                        error: ReplyAcceptanceError::Closed(reason),
                    });
                }
            };
        // Declared after waiting so revocation precedes owned native cleanup.
        let mut fence = AcceptanceFence(scope);
        let error = (|| {
            let identity_mismatch = &waiting.identity != actual_identity;
            if fence.is_closed() {
                return None;
            }
            if identity_mismatch {
                return Some(ReplyAcceptanceError::IdentityMismatch);
            }
            if let Err(error) = authority.authorize(&waiting.identity) {
                return Some(ReplyAcceptanceError::Authority(error));
            }
            if fence.is_closed() {
                return None;
            }
            let view = reply.borrow();
            if fence.is_closed() {
                return None;
            }
            validate_host_result(
                &LiveResultDomain {
                    domain: waiting.declaration,
                    scope,
                },
                view,
            )
            .err()
            .map(ReplyAcceptanceError::Result)
        })();
        if fence.is_closed() {
            self.state = State::Terminal(ReplyEnd::Cancelled);
            drop(error);
            drop(waiting);
            fence.0 = None;
            return Err(RejectedReply {
                reply,
                error: ReplyAcceptanceError::Closed(ReplyEnd::Cancelled),
            });
        }
        fence.0 = None;
        if let Some(error) = error {
            self.state = State::Waiting(waiting);
            return Err(RejectedReply { reply, error });
        }
        self.state = State::Terminal(ReplyEnd::Accepted);
        Ok(AcceptedReply {
            identity: waiting.identity,
            frame: waiting.frame,
            declaration: waiting.declaration,
            reply,
        })
    }
}

impl<Identity, Frame, Declaration: ?Sized> fmt::Debug
    for PendingReply<'_, Identity, Frame, Declaration>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingReply")
            .field("status", &self.status())
            .finish()
    }
}
