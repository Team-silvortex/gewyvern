use std::borrow::Borrow;
use std::fmt;

use crate::{HostResultDomain, HostResultError, validate_host_result};

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
        let error = if &waiting.identity != actual_identity {
            Some(ReplyAcceptanceError::IdentityMismatch)
        } else if let Err(error) = authority.authorize(&waiting.identity) {
            Some(ReplyAcceptanceError::Authority(error))
        } else {
            validate_host_result(waiting.declaration, reply.borrow())
                .err()
                .map(ReplyAcceptanceError::Result)
        };
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
