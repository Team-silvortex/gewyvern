# Leselang Language Contract

This reference is the authoritative, model-oriented contract for the currently
implemented Leselang slice. The broader destination is defined by the
[Leserpent 2.0 architecture](leserpent-2-architecture.md); unimplemented roadmap
syntax is not part of this contract.

Status: **Gate 2, evolving contract 0.13.0**. The current vertical slice parses,
lowers, authorizes, suspends, serializes, restores, and resumes the read-only
`runtime.list`, `runtime.inspect`, `runtime.history`, and `runtime.logs` effects
plus the idempotent `runtime.refresh` command effect.

## Canonical Program

```leselang
fn main() = runtime.list(
  environment: "production",
  cluster: none,
  role: "edge"
)
```

`runtime.list` returns a runtime list and requires the `runtime.read`
capability. Each optional filter accepts a string or `none`; empty strings are
normalized to `none` during HIR lowering.

The canonical single-runtime query is:

```leselang
fn main() = runtime.inspect(runtime_id: "runtime-a")
```

`runtime.inspect` requires `runtime.read`, returns exactly one typed runtime
projection, and fails with `RuntimeNotFound` when the identifier is absent. It
does not lower to a filtered list or perform hidden refresh work.

The canonical bounded history query is:

```leselang
fn main() = runtime.history(runtime_id: "runtime-a")
```

`runtime.history` requires `runtime.read` and returns at most 32 applied command
results for one runtime, ordered from newest to oldest revision. It reads stable
domain history rather than exposing persistence rows, daemon logs, or secrets.

The canonical bounded log query is:

```leselang
fn main() = runtime.logs(runtime_id: "runtime-a")
```

`runtime.logs` requires `runtime.read` and returns the newest bounded window of
at most 256 typed log records for exactly one runtime. Its canonical lowering
uses no cursor; incremental polling remains a host/watch responsibility rather
than introducing asynchronous source semantics.

The canonical mutating program is:

```leselang
fn main() = runtime.refresh(runtime_id: "runtime-a")
```

`runtime.refresh` requires `runtime.refresh`, one valid `runtime_id`, and the
expected runtime revision supplied to `Vm::start`. The VM derives stable
`leselang-command-N` and `leselang-effect-N` identifiers from the continuation
sequence and persists the complete `CommandEnvelope` before dispatch.

## Grammar

```ebnf
program       = function, EOF ;
function      = "fn", identifier, "(", ")", "=", call ;
call          = effect-call | all-call ;
effect-call   = identifier, ".", identifier, "(", [ arguments ], ")" ;
all-call      = "all", "(", branch, ",", branch, { ",", branch }, [ "," ], ")" ;
branch        = identifier, ":", call ;
arguments     = argument, { ",", argument }, [ "," ] ;
argument      = identifier, ":", value ;
value         = string | "none" | call ;
identifier    = ( letter | "_" ), { letter | digit | "_" } ;
string        = '"', { character | escape }, '"' ;
escape        = "\\", ( '"' | "\\" | "n" | "r" | "t" ) ;
```

Whitespace and `//` line comments are retained as lossless tokens, including
their byte spans. Reassembling token text must reproduce the original source.
Source is UTF-8 and limited to 256 KiB.

## Canonical Formatting

`leselang_syntax::format` is the single canonical source formatter. It removes
comments and trivia, preserves declared argument and `all` branch order, keeps
zero- and single-argument calls on one line, and renders wider calls with
two-space indentation plus trailing commas. Output ends with exactly one
newline and is rejected if escaping would expand it beyond 256 KiB.

Formatting requires a diagnostic-free syntax tree and is idempotent:
parsing and formatting canonical output must reproduce the same bytes. Native
CLI Leselang exports pass through this formatter rather than maintaining a
frontend-specific printer.

The deterministic fuzz shelf runs through
`gewyvern_validate leselang-fuzz`. Its fixed seed covers arbitrary multi-byte
UTF-8, malformed escapes, trivia, nesting, oversized source, HIR lowering, and
bounded VM startup. Diagnostic-free syntax also exercises deterministic,
bounded, idempotent canonical formatting. Every token and diagnostic span must
remain on UTF-8 character boundaries. A parallel continuation corpus mutates
encoded VM images and requires deterministic fail-closed decoding or canonical
roundtrip.

The implemented surface deliberately excludes general expressions, local
bindings, arbitrary mutation, loops, unstructured concurrency, raw HTTP, shell
execution, and host-language reflection. Synchronous source semantics do not expose
`async`/`await`; `all` is the explicit structured-concurrency form.

The frontend accepts structured declarations such as:

```leselang
fn main() = all(
  inventory: runtime.list(role: "edge"),
  refresh: runtime.refresh(runtime_id: "runtime-a")
)
```

`all` requires two to 64 uniquely named effect branches. Syntax and HIR preserve
declaration order, each branch retains its own result type, and function
authorization requires the union of all branch capabilities. The VM starts this
form as one atomic `Step::Effects` batch with a stable merge token and ordered
named requests. Completing a non-final branch returns `Step::Waiting`; completing
the final branch returns the durable aggregate result in declaration order.
SQLite recovery resumes unfinished branches without recreating completed work.
Nested `all` remains outside the current language surface and fails with
`LSV1002` before sequence allocation or journal mutation.

## HIR And Authorization

The syntax tree lowers each implemented operation into its corresponding typed
runtime effect. Lowering rejects unknown effects, duplicate named arguments,
unknown arguments, and values with the wrong shape.

Authorization is explicit and occurs before VM execution. A caller without
the effect's required capability receives a capability diagnostic; the VM does not emit an
effect request for unauthorized code.

## Execution Protocol

Authorized atomic effects first lower through `leselang-command` into a pure
`CommandPlan`. The plan owns the required capability and either a versioned
`QueryEnvelope` or `CommandEnvelope`; frontend origin is audit metadata and does
not select a different implementation. The VM owns continuation and journal
lifecycle, but it does not privately construct domain command semantics.
`CommandPlan` JSON carries its own schema version, round-trips canonically, and
is rejected before decoding when it exceeds 64 KiB.

The stackless VM advances through six protocol states:

- `Done`: evaluation completed with bounded output
- `Effect`: the host must execute a typed request and resume the continuation
- `Yield`: cooperative suspension reserved by the protocol
- `Cancelled`: terminal requested cancellation or trusted deadline expiry
- `Failed`: terminal classified effect failure or exhausted semantic retries
- `Fault`: evaluation stopped with a stable VM diagnostic

For the current slice, `start` emits one typed query or command operation. The
operation carries a continuation token and expected revision. Read-only queries
may use the direct embedded `resume` path. Mutating commands must be leased and
completed through `acknowledge_effect`; direct mutation resume fails closed.

`Vm::start_timed` accepts scheduler `now_ms` and a bounded timeout, persists the
resulting absolute deadline, and requires `resume_at`, `claim_effect`, or
`acknowledge_effect` to supply trusted scheduler time. At or after the deadline,
the VM atomically records `Cancelled(DeadlineExceeded)` before another dispatch
or result can win. `Vm::cancel_effect` records `Cancelled(Requested)` and may
fence an active lease. Both cancellation forms are durable, idempotently replayed
terminal states. The legacy `Vm::start` remains untimed for embedded compatibility.

Debugger-driven cancellation uses `Vm::cancel_effect_audited`. Journal schema 6
commits the requested terminal state and its command, principal, origin, session,
revision, and observation-time correlation in one transaction. Replays preserve
the first record, while command reuse or principal-scoped idempotency conflicts
fail closed. The queryable audit record deliberately excludes the continuation
token and idempotency key, and is removed when bounded journal retention removes
the corresponding continuation.

Workers classify execution errors as `Transient` or `Permanent` through
`Vm::report_effect_error`. Permanent errors immediately become a durable
`Step::Failed`; transient errors follow a bounded `RetryPolicy`. The default is
three semantic retries with deterministic exponential delays starting at 250 ms
and capped at 30 seconds. Policy limits are 16 retries and one-hour delays.
`retry_count` is separate from transport `attempt`, so a worker crash does not
consume business retry budget. A scheduled retry persists its `ready_at_ms` and
last classified error and cannot be claimed early; retry exhaustion becomes a
typed, replayable failure. The original effect request and command idempotency
key never change.

`merge_declared` is the bounded deterministic merge kernel for future structured
`all` evaluation. A `MergePlan` declares two to 64 uniquely named branches;
completions may arrive in any order, but successful `Value::Structured` fields
and competing terminal outcomes are always selected in declaration order.
Malformed, missing, duplicate, or unexpected completions fail closed. Structured
values are limited to 16 levels, branch names to 64 ASCII identifier bytes, and
the caller's output-item budget. The merged terminal value uses the existing
journal schema and remains replayable after restart. This kernel is public VM
protocol today; source-level `all(...)` parsing and typed lowering are
implemented, while durable multi-branch continuations remain integration work.

Continuation images are versioned and capped at 64 KiB. Execution also enforces
a source-size limit, fuel limit, 24-hour maximum deadline budget, and
10,000-item output limit.
Restored continuations advance their token sequence so a restart cannot reuse a
live token.

`Vm::new` provides an ephemeral journal for tests and embedded one-shot use.
`Vm::open_journal` opens the SQLite effect journal for service operation. It
atomically persists pending continuations before exposing an effect, restores
them after restart, and commits the first terminal step as authoritative.
Duplicate or competing completion after a process restart replays that durable
step rather than accepting a later result. Sequence allocation is transactional
across concurrent VM connections.

Service workers use the durable dispatch outbox rather than dispatching the
returned `Step::Effect` directly. `Vm::claim_effect` leases one ready or expired
request and returns an attempt-fenced `DispatchLease`; another live VM cannot
claim it until that lease expires. `Vm::acknowledge_effect` verifies the request,
attempt, and expiration, then commits the terminal step and acknowledgement in
one transaction. A crashed worker therefore causes bounded redelivery, while a
late worker cannot overwrite a newer attempt. The `now_ms` argument is scheduler
time supplied by the trusted runtime host, never by a remote request.

The journal is schema-versioned. Schema 4 persists indexed absolute deadlines,
retry counts, and not-before clocks. Schema 5 adds strict merge-group and ordered
branch metadata with bounded plan, graph-state, token-namespace, and terminal-step
validation during recovery. The journal can atomically create a complete merge
graph: plan, branch continuations, dispatches, and declared-order links either
all commit or all roll back, and committed branches recover after restart. The
last terminal branch and its declared-order group result also commit together;
a failed group update leaves that branch pending under its existing lease.
Success, cancellation, classified failure, fault, and deadline terminal paths
share this finalization boundary. Retention treats a completed graph as one
logical record and atomically removes its group, ordered links, branch effects,
and dispatch rows; pending and partially completed graphs are excluded.
Schema 1 and 2 records migrate as untimed, schema 1
through 3 records migrate with zero semantic retries rather than receiving
fabricated execution history, and schema 1 through 4 receive empty merge tables.
Pre-existing conflicting merge-table names fail the migration closed. Schema 5
provides the executable durable graph lifecycle for one-level source `all`,
including atomic startup, ordered progress, final aggregation, restart recovery,
and whole-group retention. Nested `all` still fails closed with `LSV1002` before
sequence allocation.
The journal
uses full synchronous commits and a five-second lock
timeout, rejects symbolic-link final paths, and creates Unix files with `0600`
permissions. It is bounded to 10,000 records, 8 MiB per terminal step, 100
dispatch attempts, a five-minute maximum lease, and 64 MiB of total logical
payload including dispatch requests and merge plans.

`Vm::compact_journal` applies explicit count-based retention to terminal
records. It never deletes pending or leased effects, always retains at least one
completed record, and removes at most 1,000 records per transaction. The default
policy retains 5,000 completed records and deletes in batches of 500. Selection
uses durable insertion order, dispatch rows are removed by foreign-key cascade,
and SQLite secure deletion is enabled so deleted payload is overwritten in
reusable pages. `CompactionReport::reclaimed_logical_bytes` reports removed
payload; it does not promise immediate physical database file shrink.

This is a durable continuation guarantee plus at-least-once dispatch. For
`runtime.refresh`, every redelivery reuses the same domain idempotency key, so
the current domain kernel commits the refresh once and replays its first result.
This is not a blanket exactly-once guarantee for arbitrary external adapters;
each future mutating effect must prove the same end-to-end contract.
Terminal replay is guaranteed only while the record remains inside the explicit
retention window; a compacted token becomes unknown (`LSV2004`). Deterministic
merge semantics are implemented, while structured `all` continuation wiring
remains Gate 2 work.

## Diagnostics

Diagnostics use stable subsystem prefixes:

| Prefix | Owner | Examples |
| --- | --- | --- |
| `LSE` | lexer and parser | malformed input, source limit |
| `LSH` | HIR and authorization | unknown effect, duplicate argument, missing capability |
| `LSV` | VM, continuation, and journal | invalid image, revision conflict, persistence failure |

Consumers must branch on diagnostic codes rather than English messages. Spans
use byte offsets into the original UTF-8 source.

## Integration Sequence

For deterministic model or CLI integration:

1. Parse source and report all syntax diagnostics.
2. Lower the syntax tree into HIR and report semantic diagnostics.
3. Authorize required capabilities before starting the VM.
4. Open the durable journal, then call `Vm::start`; pending state is committed before return.
5. Prefer `Vm::start_timed`, then call `Vm::claim_effect` using trusted scheduler time.
6. Call `Vm::acknowledge_effect` with the same lease and typed result before expiry.
7. Let expired leases become claimable after restart; never reuse an older attempt.
8. For commands, execute the persisted `CommandEnvelope` unchanged on every redelivery.
9. Treat repeated acknowledged completion as replay, not another external operation.
10. Treat `Cancelled` as a terminal result and never dispatch its operation again.
11. Report classified failures through `report_effect_error`; never retry outside the journal.
12. Run bounded `compact_journal` maintenance and treat its retention window as part of the service contract.

`Vm::resume` remains the direct embedded path for an effect that has not been
leased. Once a request is leased, completion must use `acknowledge_effect`.

The implementation lives in `crates/leselang-syntax`, `crates/leselang-hir`,
and `crates/leselang-vm`. Delivery progress is tracked by the
[project status tensor](project-status-system.md), not inferred from future
examples in architecture documents.
