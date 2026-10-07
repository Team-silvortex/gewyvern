# Leselang Embedding Architecture

This is the target architecture for Leselang as an independent embeddable
control language. It is not a claim that the current runtime is already
product-independent. The [language contract](leselang-language.md) describes
implemented syntax; the [control-flow contract](leselang-control-flow.md)
separates shipped mechanisms from remaining language work.

## Identity

Leselang is intended to become fully independent of Gewyvern and Leserpent,
including its runtime, host protocol, tests, and distribution. Leserpent is the
first reference host, not the definition or owner of the language's semantics.
The Lua-like goal is embedding: an application supplies capabilities and runs
programs through a small language interface. It does not imply Lua syntax,
Lua compatibility, or the same VM implementation.

GUI automation is one host profile, not the limit of the language. A future
shell for nuis OS and sirius kernel is another host profile. That distant goal
does not require an OS implementation now, imply kernel-resident execution, or
claim current `no_std`, process-control, or shell compatibility.

## Agent-First Syntax

Agent-driven GUI and OS control is the syntax design target. Bash, Lua,
JavaScript and other languages are capability references, not syntax or
compatibility targets. New syntax must justify itself through reliable program
generation, validation, composition and recovery, not resemblance to another
language. Human review and debugging must remain clear.

The design rules for future language changes are:

- Prefer a small, regular grammar with explicit names, scopes and typed values.
  Avoid implicit string-to-code conversion, context-sensitive coercions and
  multiple shorthand forms for the same operation. Fewer tokens are useful only
  when they do not increase ambiguity or hide authority.
- Make observation, decision, action and result handling composable. Programs
  should bind typed results and stable host identities rather than parse display
  text or guess coordinates. A well-typed target still needs host validation at
  execution time; observations can become stale.
- Express waiting, cancellation, bounded iteration and recovery without a
  source-level async model. Define loop exit, iteration skip and error-handling
  semantics before choosing keywords; `if`, `for`, `break` or `try` spellings are
  not commitments. A timeout with an unknown external outcome must not become
  an implicit retry of a potentially completed action.
- Support deterministic formatting and source/HIR round trips that preserve
  meaning. Diagnostics should carry stable codes, source spans and typed
  context so an agent can repair a local error without regenerating unrelated
  effects. Recovery from execution and editing a program are distinct actions;
  edited source must not silently reuse an old continuation.
- Discover operations from versioned host schemas, not invented API names.
  Program text may request capabilities but never grants them. Credentials and
  authority remain host-owned; manual GUI actions and generated calls use the
  same policy boundary. Untrusted GUI text and host results remain data, not
  executable instructions or implicit `eval` input.

Future acceptance cases should cover generation from a declared host schema,
typed result binding, a local diagnostic repair, denied authority, a stale
target, cancellation during a wait, and restart without repeating settled
effects. These are design gates, not claims that all such language features
already exist. The current grammar remains authoritative until a versioned
change adds implementation and tests; this section introduces no new syntax.

## Ownership

| Layer | Owns | Must Not Require |
| --- | --- | --- |
| Language core | syntax, values, bindings, functions, control flow, HIR, bounded evaluation, suspension and re-entry | Gewyvern, Leserpent, GUI widgets, a daemon, a particular OS |
| Host protocol | versioned operation schemas, typed inputs/results, capability requirements, identities, cancellation and resource lifetimes | product enums, widget pointers, implicit global authority |
| Optional host profiles | GUI semantics, Leserpent operations, future OS services | changes to core syntax for each new host operation |
| Embedding host | effect execution, event loop, scheduler, storage selection, policy and resources | a second implementation of language semantics |

Dependency direction is host/profile -> language contracts and core, never
core -> product host. Rust is the initial implementation and embedding surface;
other languages use a versioned protocol or narrow FFI boundary. Frameworks
need explicit developer-owned adapters or schema-generated bindings. GUI
conformance remains declared through `UiAdapterManifest`; no GUI framework is
automatically compatible.

Host operations must be declared and validated against a bounded schema before
execution. Domain names such as `runtime.deploy` belong to the Leserpent host
profile, not an ever-growing core opcode list. This is not an untyped
`call(any_string, any_json)` escape hatch or arbitrary native-library loading.

## Execution And Safety

Source remains synchronous and function-oriented. External work yields a typed
effect and an explicit continuation; the host returns a matching result to
resume it. A GUI dispatcher, background worker, or future OS event source may
be asynchronous internally without introducing source-level `async`/`await`.

Independent VM instances must not require a process-global interpreter lock.
The host serializes access to each mutable instance and dispatches GUI work on
the framework's permitted thread. This is not a promise of unsynchronized
access to one VM or of parallel execution inside every program.

Budgets cover pure evaluation, loop iterations, allocation, nesting, output and
external-effect admission. Capabilities are explicitly granted by the host;
declaring an operation never grants its authority. A callback or FFI adapter
must obey cancellation and deadlines itself: VM fuel cannot preempt arbitrary
blocking native code, and an in-process extension is not an OS security sandbox.

Continuations must contain bounded language values and versioned, scoped
resource handles, never host stack frames or native object pointers. A restored
program must revalidate host compatibility and authority before dispatch.
Removed GUI objects or expired resources produce typed failures, not accidental
rebinding to a new object with a reused identifier.

The target core does not require SQLite, a network connection, or a daemon for
an in-memory embedding. Durable journaling is a selectable integration with
explicit recovery guarantees, not a guarantee implied by all embeddings. The
current VM still links SQLite and product types; this separation remains work.
Replay consumes recorded effect results rather than silently repeating external
work. Recovery must retain fencing and idempotency rules; a language engine
alone cannot guarantee exactly-once effects for arbitrary hosts.

## Concurrency Model

The target is **multiple isolated execution contexts with bounded host workers**,
not one shared mutable language heap or one operating-system process per script.
Instance count and batch width are separate axes; neither replaces the other.

| Unit | Contract | Current Mechanism |
| --- | --- | --- |
| Engine / worker | One mutable engine is entered serially; independent engines can run on separate threads | Rust `&mut Vm`, movable `Send` engines, immutable HIR shared between workers |
| Execution context | Locals, authority, fuel, deadlines and result frames belong to one root execution | Each `start` creates an independent bounded flow; one VM can suspend multiple roots |
| Effect batch | A context waits for its declared operations, not arbitrary sibling executions | `seq` admits the next member in order; flat `all` has independently leaseable branches and an all-success barrier |

**No process-global interpreter lock** is introduced. Source remains synchronous;
only the host schedules worker threads and external effects. Pure evaluation and
journal work belong off the GUI thread. A parallel effect batch does not promise
parallel GUI mutation or create threads inside the language evaluator. Native
single-presentation debugger sessions remain serial within each session; native
parallel batches still fail preflight.

### Shared Durable Workers

Several VM workers may open the **same journal namespace**. A long-lived worker
can claim work admitted after its startup snapshot: `claim_effect` validates the
outbox request against the durable continuation and adopts that request into its
local pending cache. This **does not rerun source or refill fuel**. Sequential
readiness, parallel barriers, attempt-fenced leases, semantic retry and first-commit
replay still come from the authoritative journal transactions. Corrupt request/image
pairs fail before a lease is issued or work is exposed to the host.

`pending_count` and `pending_continuations` describe a **local cache, not a global
queue count**. One worker need not load every sibling branch to execute its claimed
operation. Shared SQLite write transactions serialize persistence, not all language
evaluation in the process. This is worker interoperability, not an implemented
thread pool or tenant isolation system.

### Dispatch Selection

Among **eligible requests**, both ephemeral and SQLite journals choose the
**fewest delivery attempts**, then **numeric admission order**. Canonical
`continuation-2` precedes `continuation-10`; text ordering alone is not used.
Only an issued lease increments the attempt. Not-before clocks, unexpired leases,
sequential predecessors, cancellation and deadlines still gate eligibility.
Retry counts and fuel are not reset by selection or worker restart.

For a **fixed eligible cohort**, lower-attempt members get a turn before an
expired lease can keep accumulating attempts. The ordering uses existing durable
counters, not a worker-local cursor, and shared workers select under one SQLite
write transaction. A partial ordering index excludes acknowledged dispatches;
it can be added to existing schema-10 journals without changing wire formats.
The ephemeral selector uses a constant-space minimum scan without cloning
unselected requests or constructing a second queue.

An effect at the delivery-attempt limit does not block eligible effects with
remaining attempts. If all eligible effects are exhausted, `claim_effect` still
reports `LSV4017`; it does not silently drop work or return an empty queue. The
host can cancel the affected continuation or apply its explicit failure policy.

This is **not a global FIFO or tenant-fairness guarantee**: newly admitted work
can delay older retries, future retries are ineligible before their clock, and
one graph can admit more effects than another. A bounded host admission policy
and resource-lane scheduler are still needed for sustained-load fairness.

### Host Admission And Backpressure

`SchedulerLimits` is **trusted host policy**, not script syntax or a capability.
`Vm::new_with_limits` and `Vm::open_journal_with_limits` accept limits for
**pending dispatches** and **active leases**, each from 1 through 10,000.
The default constructors use 10,000 for both. A host can choose smaller values:

```rust
let limits = SchedulerLimits {
    max_pending_dispatches: 128,
    max_active_leases: 4,
};
let mut vm = Vm::open_journal_with_limits(journal_path, fuel_limit, limits)?;
let claim = vm.try_claim_effect(now_ms, 5_000)?;
```

Pending dispatches include ready, retry-delayed, leased and sequentially blocked
effects. Completed effects and graph headers do not consume pending slots;
retained history still has separate journal record/byte limits. Bare continuation
imports without a host request are not dispatches. These are **outbox bounds, not
a total memory or CPU ceiling**; source parsing, pure evaluation and host-owned
continuation imports still need their own ingress/resource policy.

New single effects and **whole initial batches** are checked before sequence
allocation and again **inside the admission write transaction**. A conflicting
worker cannot over-admit based on a stale local snapshot. An early refusal leaves
identities unchanged; a racing refusal may leave unused sequence gaps, but never
publishes partial requests or reuses identities. `restore_request` also checks
new admissions transactionally; an identical existing request is idempotent even
at capacity. Pure completion does not need an outbox slot.

| Outcome | Meaning | Host Response |
| --- | --- | --- |
| `LSV2500` | Invalid host limits | Correct configuration before opening a journal |
| `LSV2501` | New work does not currently fit | Defer admission until existing work drains; do not busy-spin |
| `LSV2503` | One initial batch exceeds the configured bound | Reduce batch width or explicitly raise the host limit |
| `DispatchClaim::Leased` | One attempt-fenced lease was issued | Route only this request to the host adapter |
| `DispatchClaim::Idle` | No eligible work at this clock | Wait for admission, predecessor completion, retry time or lease expiry |
| `DispatchClaim::Backpressured` | Eligible work exists but lease capacity is full | Wait for completion, cancellation or lease expiry before polling again |

`claim_effect` remains available; it maps lease backpressure to `LSV2502`, not
`None`. Use `try_claim_effect` for the typed distinction. A backpressured claim
**does not issue a lease or increment attempts**, and SQLite avoids loading the
queued request/image on that path. Lease-capacity inspection stops once the
configured ceiling is reached, rather than decoding or counting the whole queue.
Read-only `scheduler_pressure` reports authoritative pending/active counts plus
the worker's configured limits, not its pending cache. Reading pressure does not
cancel overdue work; timed admission and dispatch entry points reap deadlines.

Both `try_claim_effect` and its legacy `claim_effect` wrapper perform **claim
parameter preflight before expiry cleanup**. Clock range is checked first, then
lease duration and its representable expiration. Invalid parameters do not enter
the journal, cancel due work, finalize a group, spend attempts or change a lease.
They also do not wait for a SQLite writer lock. Out-of-range public clocks retain
`LSV2011`; invalid lease duration/expiration retain `LSV4015`. Valid calls still
reap due deadlines before selecting work. This rule is specific to claim parameters,
not a change to completion/replay precedence or authorization policy.

`set_scheduler_limits` can lower capacity below existing occupancy without
cancelling or truncating recovery. **Already admitted chains and group tails
continue to drain**, even when new starts are backpressured. Semantic retry keeps
its pending slot but releases its active lease. Completion, cancellation and
deadline expiry release pending capacity; expired leases release execution slots
without weakening attempt fencing.

Policy is **not persisted in continuation or journal wire formats**. The host
must reapply **the same limits to all workers in one journal namespace** after
restart and coordinate policy changes; a worker with different limits can defeat
the shared policy. This cooperative quota is not a security boundary. Lease
capacity bounds valid leases, not callbacks still running after expiry; host
resource lanes and idempotency/fencing remain necessary.

Native debugger VMs apply a 64-pending/one-active-lease policy, in addition to
the existing 32-session authority limit. Their correlated GUI channel remains
serial and rejects unsupported parallel starts; this is not a new native worker
pool or a promise of parallel GUI mutation.

Scheduler faults use `LSV2500` through `LSV2503`; existing merge validation faults
`LSV2401` through `LSV2404` retain their meanings. Do not treat a malformed merge
plan or completion as retryable admission pressure.

### Bounded Host Ingress

The host-neutral `leselang-runtime-core` implements this lifecycle once as
`Admission<Input>` and `AdmissionAdapter<Input>`. Its input/output types are opaque,
with no HIR, command, GUI, database or account dependency. `RootAdmission` below
is the reference VM adapter, not a second scheduler implementation. The core's
`AdmissionAttempt::{Started, Backpressured, Rejected}` classification is explicit:
hosts must prove that `Backpressured` published no accepted work. The core does
not inspect host error-code strings, validate product authority or preempt callbacks.
The reference adapter alone maps transactional `LSV2501` to temporary pressure.
The handle is consumed before entering a due host callback and rearmed only by
explicit pressure. A callback panic propagates, but catching that unwind cannot
re-poll this handle into replaying work with unknown publication state. This is
not callback preemption or recovery of the host's already accepted work.

`RootAdmission` is a **host-owned, single-use pre-admission handle**, not an
effect continuation or a source-language feature. It takes ownership of one
lowered program, principal, capabilities and expected revision. These inputs
cannot change between retries. Creating the handle does not enter a VM, reserve
an identity or publish work; the host decides when to call `poll(&mut vm, now_ms)`.

```rust
let policy = AdmissionPolicy {
    max_attempts: 8,
    base_delay_ms: 100,
    max_delay_ms: 2_000,
};
let mut admission = RootAdmission::new(
    program, principal, capabilities, expected_revision,
    submitted_at_ms, timeout_ms, policy,
)?;
let outcome = admission.poll(&mut vm, now_ms)?;
```

`AdmissionPolicy` permits 1 through 32 **total attempts including the first**;
the base delay must be nonzero, and base/max delay must be ordered and at most
3,600,000 ms. Defaults are 8 attempts, 250 ms base and 30,000 ms maximum. Only
temporary admission pressure (`LSV2501`) schedules a retry, with deterministic
capped exponential backoff. Authorization, fuel, computation, merge validation,
oversized-batch and journal faults terminate the handle with their original code.
This is not lease redelivery or semantic effect retry, and it never resumes an
already-admitted continuation by rerunning source.

`AdmissionPolicy::validate()` provides **pure policy preflight** before the host
constructs a program or takes ownership of host-private input. It validates attempts,
base delay, maximum delay and delay ordering in that deterministic order, with
`LSV2510` diagnostics naming the failed constraint and not echoing submitted values.
The constructor repeats the same validation before clock/deadline checks. Preflight
does not authorize source, reserve ingress capacity or change any runtime state.

| Outcome | Meaning | Host Response |
| --- | --- | --- |
| `AdmissionPoll::Waiting` | Not-before time and pinned deadline, plus attempts spent | Arrange a host wakeup at or after `retry_at_ms`; do not busy-spin |
| `AdmissionPoll::Started` | One initial `Step`, including pure completion or an effect/batch | Transfer ownership to normal execution/dispatch; never restart this handle |
| `AdmissionPoll::Rejected` | Terminal pre-admission failure | Report it and release host ingress capacity |
| `AdmissionPoll::Finished` | A prior outcome consumed or cancelled the handle | No new work and no repeated `Step` |
| `Err(Fault)` | Invalid or regressing active-handle clock | Correct the trusted host clock; handle state is unchanged |

Core and reference admission handles/outcomes carry **must-use ownership diagnostics**.
Ignoring a new handle drops pending input. Ignoring a `Started` outcome does not
cancel accepted work; even `poll(...)?;` can discard that outcome while handling
only the outer `Result`. Compiler warnings and compile-fail tests cover this
misuse. Exhaustively route each result; explicit disposal remains possible with
`let _ =` or `drop`. This is not guaranteed delivery or runtime recovery of lost
results, and does not change result serialization.

The generic core has **conditional thread ownership**, not mandatory `Send` or
`Sync` bounds. A `Send` input permits moving its handle between workers, including
while waiting; `Sync` is not needed for that move. Attempts, the observed clock
watermark and the pinned deadline survive handoff. The host must preserve request
meaning/authority and use the same clock domain, not restart or reset the handle.
Inputs, adapters and success outputs can instead contain thread-local `Rc` state
and remain on a GUI dispatcher. The core does not automatically dispatch them or
relax framework affinity. Bounded callback-rendezvous tests verify independent
instances can enter hosts concurrently without a process-global admission lock;
local cancellation and unwinding do not poison sibling handles. These tests cover
admission only, not parallel evaluation or a generic GUI-operation schema.

Polling before the not-before time is **VM-free and journal-free**: it does not
evaluate source, reap unrelated deadlines, allocate identity or spend an attempt.
Every due attempt still uses the normal transactional admission checks, including
shared-worker races and whole-batch refusal; the handle does not reserve capacity
while waiting. Before successful admission, bounded pure preparation may repeat
under the VM's per-attempt fuel limit. Hosts must account for that CPU cost across
the finite attempt budget; no accepted root receives fresh fuel through this API.

The absolute deadline is **pinned at submission**, and includes both waiting and
execution. Successful admission uses only the remaining execution window. At the
deadline, even a pure program is rejected without entering the VM. Backoff never
extends that deadline; clocks are host-supplied, range-checked and nondecreasing
per active handle. Equal timestamps are allowed. Exhaustion/expiry consumes the
handle, and all terminal outcomes release its held program and authority.

| Code | Meaning |
| --- | --- |
| `LSV2510` | Invalid admission retry policy |
| `LSV2511` | Active-handle clock moved backwards; no state change |
| `LSV2512` | Total admission-attempt budget exhausted |
| `LSV2513` | Submission-pinned deadline reached before admission |

The core's **bounded exhaustion diagnostic** contains only the attempt count:
`admission exhausted after 1 attempt` or `admission exhausted after 32 attempts`.
It does not append the last host backpressure code or message, even for very large
or control-character-bearing error text. The `LSV2512` code and existing fault
JSON shape are unchanged. Direct permanent host rejections remain verbatim and
are not automatically sanitized or retried; hosts own their error/log redaction.

`cancel()` discards only a not-yet-admitted request and returns whether it did so.
After `Started`, it returns false and cannot cancel the accepted VM root; use its
normal continuation cancellation protocol. A finished handle always returns
`Finished`, without entering the VM or validating irrelevant new clock input.

`status()` is a **read-only admission snapshot**, not another polling entry point.
`AdmissionStatus::Pending` contains attempts, `retry_at_ms` and `deadline_at_ms`;
`Finished` contains only attempts spent. Creating/copying/serializing a snapshot
does not enter the VM/journal, update the clock, spend fuel, reserve capacity or
passively expire work. Snapshots can become stale; only the live handle's `poll`
can attempt admission or observe expiry. A finished snapshot does not repeat a
start/rejection result, preserve input authority or distinguish terminal causes.
The caller owns the original `AdmissionPoll` outcome. This additive observation
type does not change existing poll, continuation or journal wire shapes.

`terminal_reason()` adds a separate **payload-free terminal reason**, shared by
the generic handle and `RootAdmission` through `AdmissionEnd`. Pending handles
return `None`; ended handles return one fixed snake-case tag: `started`,
`rejected`, `cancelled`, `expired`, `exhausted` or `host_uncertain`. These copyable,
serialize-only observations do not change the legacy `Finished { attempts }`
status or poll JSON and retain no input, success output, host error or panic text.
Repeated observations cannot enter the VM or journal, including while SQLite has
an active writer, and cannot expire pending work or change a finished reason.

**Acceptance is not output delivery or execution completion**: `Started` means
only that the adapter returned acceptance. It remains true for both a pure root
that completed immediately and an effect root still waiting for execution.
`HostUncertain` records a callback unwind with unknown publication state, not a
rejection or automatic replay permission. The host must reconcile accepted-work
identity when publication is uncertain; terminal metadata supplies no authority
to resubmit, cancel accepted work, restore a handle or repeat its original result.

Admission handle **debug output contains scheduling metadata only**: it neither
prints the program/principal/capabilities nor invokes any host input formatter.
Opaque input needs no `Debug` implementation. This protection is limited to the
handle; payload-bearing steps/requests and host-written fault messages still need
host-owned logging redaction. Terminal cleanup takes the input before dropping it;
even if its destructor unwinds and the host catches that unwind, the handle cannot
rearm or clean up the same input twice. Cleanup panics are not swallowed or turned
into a false successful host result.
An **explicit pending-or-terminal ownership state** records known acceptance or
rejection before input cleanup. Successful output and direct errors stay locally
owned until cleanup succeeds, before they enter the return slot; if cleanup
unwinds, local ownership releases that undelivered output/error. The terminal
reason stays known rather than being downgraded to uncertainty. This ordering
does not catch panics, roll back accepted work, guarantee delivery or redefine
host output destructors. Only explicit pre-publication pressure can rearm input.

The handle is **not cloneable or deserializable**. Serializable outcomes are
observations, not checkpoint/replay authority. It can survive a VM reopen within
one host process, but it is not a durable ingress queue or an exactly-once
submission record. The host must bound the number and retained size of handles,
validate source/HIR ingress, arrange wakeups, drain accepted work, and manage
request ownership across process crashes. There is **no hidden queue, timer,
worker pool or global lock**. Backoff supplies neither jitter nor fairness; hosts
may stagger wakeups later than the not-before time without extending the deadline.
GUI dispatcher affinity, resource lanes and callback preemption remain host work.
Native debugger starts do not automatically opt into deferred admission.
Continuation schemas 1-11 and journal schema 10 remain unchanged.

### Shared Language Values

`leselang-runtime-core` now owns **host-neutral scalar data**: `ScalarType`,
`ScalarValue`, `OptionalStringValue`, `StringListValue` and the existing text/list
limits. Integers remain `u64`; booleans, plain text, none, distinct optional text
and ordered text lists retain their closed tags and payloads. These are data,
not host operations, resource handles, capabilities or executable expressions.
The old HIR computation paths and VM scalar path re-export these exact types,
**not conversion wrappers** or another value implementation.

Optional decoding still requires explicit null/text, not a missing tagged
payload, and enforces 4096 UTF-8 bytes. Lists retain ordered empty/duplicate
entries, 64 entries and 4096 combined UTF-8 bytes, with incremental decoding that
does not reserve from an untrusted size hint. Owned and borrowed serde paths
follow the same original rules. Tagged value, result, projection, continuation
and journal bytes remain unchanged; there is no schema migration.

**Plain-string decoding retains its legacy decode-then-validate boundary**.
Public Rust construction/mutation and serialization do not certify boundedness;
trusted ingress must call `is_bounded()` and the reference HIR/VM still validates
before evaluation or dispatch. Receiving host domains retain their narrower
validators. A surrounding decoder may allocate first, and value formatting or
serde diagnostics may expose data: hosts still bound ingress and redact logs.
This extraction does not silently clamp text, coerce missing payloads or grant
authority through serialized values.

**Source constructor node/depth costs remain HIR-owned**, including helper
expansion before constant folding. Scopes, cost rules, host-result capture/domains
and durable frames are not moved into the data module.
The core's only normal dependency remains serde. Data-codec and cross-layer
identity tests prove this shared foundation, not a host schema, suspension engine
or independent evaluator. Continuation schemas 1-11 and journal schema 10 remain
unchanged.

### Shared Scalar Operations

The runtime core owns **closed scalar operation semantics**: `BinaryOperator`,
`UnaryOperator`, exact builtin names/wire tags and pure scalar `result_type()`
signatures. The old HIR imports re-export the same types and use those shared
signatures. This is 23 binary and seven unary data operations, not a generic
host-operation schema, string-based host dispatch or a new source language.

`apply_unary()` and `apply_binary()` accept **already evaluated operands** by
value, with no expression tree, host callback, authority, clock or global state.
They reject mismatched types and unbounded operands, including directly
constructed optional/list data and legacy plain-string decoding. Output bounds
are checked before oversized text/list materialization. Checked `u64`, strict
ASCII integer/exact boolean parsing, Unicode-scalar character indexing, ordered
list indexing and explicit optional defaults retain the existing rules. Owned
text pass-throughs move their buffers without conversion wrappers or cloning.

`ScalarError` carries **no input payload** and has fixed display text. The
reference adapter maps it to the existing `LSV1401`, `LSV1402`, `LSV1403` and
`LSV1408` diagnostics, without changing messages or exposing conversion input.
The shared typed recovery set remains arithmetic/parsing only, never type, size,
fuel, authority or host failures. No fault DTO or journal schema is migrated.

**Short-circuiting and fuel charging remain evaluator-owned**. Both supplied
values are checked by the eager scalar API; it does not promise lazy evaluation
of a right expression. The VM uses the shared left-value decision to skip lazy
right expressions, preserves exact input/output fuel charges and
re-enters with saved fuel. Hosts must bound ingress and aggregate repeated work;
per-operation bounds do not form a memory sandbox or handle allocator failure.
Standalone type matrices and edge cases plus HIR/VM wire, parity, diagnostic,
short-circuit and exact fuel-threshold tests prove this boundary, **not full
expression-evaluator independence** or generic host schemas.

### Shared Scalar Control Decisions

The core's `select_binary_left()` owns the **left-value short-circuit decision**,
not expression evaluation: false `and`, true `or`, and present optional text
produce `BinarySelection::Complete`; other cases carry the original left value
in `NeedsRight`. Present-empty text still completes, bounded optional text moves
without the former second VM string copy, and non-lazy operations avoid redundant
left scans. Completed values are bounded; selection contains actual data, so its
formatting is not a redacted observation interface.

**Deferred values are not validation certificates**. The adapter must preflight
both expression types, purity and capabilities, including cold paths, before
evaluation. `NeedsRight` then requires right evaluation/charging and the normal
eager `apply_binary()` checks on both values. The decision primitive never calls
a host, invokes a callback or grants authority/fuel through a tagged result.

`LoopBudget` supplies **condition-first typed loop accounting**, with the existing
0-1024 `MAX_LOOP_ITERATIONS` re-exported by HIR. The initial value remains bounded
and owned by the adapter; the budget retains only its type, limit, completed
count and phase. Evaluate/charge the condition first, then `check_condition()`:
false exits even at the limit, while true at the limit exhausts before another
next expression is evaluated. A permitted next value must pass `advance()`'s
same-type and bounds checks before replacing scoped state or increasing the
completed count. Invalid ordering/type/bounds do not change the counter or refund
fuel; completion/exhaustion cannot rearm. Diagnostics contain no state payload.

These counters are **move-only, non-default and non-serde**, not saved execution
authority or an enforceable sandbox. Trusted adapters can explicitly construct
fresh counters and remain responsible for initial bounds, aggregate work and
resource policy. The reference VM retains exact condition/next charges, scope
cleanup on calculation errors, nested-loop independence, original `LSV1406`
messages and the closed non-resource recovery set. Pure loops still do not
suspend mid-iteration; durable re-entry recalculates only uncommitted pure bodies
from validated saved state/fuel and replays already committed results unchanged.
No source syntax, continuation schema or journal format changes. These counters
alone are **not a second
interpreter** or complete expression-evaluator independence;
shared pure execution below now owns fold bodies and pure branches/recovery,
while effect-position control and durable host-result frames remain in VM.

### Shared Bounded Fold Traversal

The core's `FoldCursor` supplies **owned bounded fold traversal**, not evaluation
of its body or an execution grant. Construction consumes one evaluated
`StringListValue`, including on rejection. It checks the 0-64 literal limit,
collection bounds (64 entries/4096 combined UTF-8 bytes), and complete item count
against that limit before yielding anything. Excess entries fail upfront,
**never truncate**. The initial accumulator remains bounded and adapter-owned;
the cursor sees only its scalar type.

`next_item()` **moves original text buffers in source order**, preserving empty,
duplicate and Unicode entries. A bounded, same-type next accumulator must pass
`advance()` before another item can be transferred. Only successful advances
increase the completed count. Wrong phase/type/bounds leave unvisited items and
progress unchanged; this does not refund fuel or authorize retry of failed
expressions or host work. Final `None` closes traversal without rearming. The
cursor is not an unrestricted `Iterator`, a serialized program counter or replay
authority. It is move-only, non-default and non-serde; its **metadata-only Debug**
omits both the unvisited collection and accumulator data. Fixed errors contain
no input payload. The cursor reuses the owned vector and string buffers without
allocating a second traversal collection or storing a state value.

The adapter still owns cold type/purity/capability checks, bounded initial state,
body evaluation, fuel, local slots and cleanup. The reference VM prepares items
before initial state, then rejects excess count before the original collection
scan charge or any next-body calculation. Tests captured the exact fuel/error
behavior before delegation, including zero/maximum limits, Unicode text,
resource-failure recovery fences and nested/error scope cleanup. Pure uncommitted
folds may be recalculated under validated saved fuel during durable re-entry;
committed results still replay without recalculation. No mid-fold suspension,
callbacks, timers, global locks, authority grant or enforceable sandbox. Trusted
adapters can construct another cursor explicitly. Source syntax, continuation
schemas, journal formats and projection vocabulary stay unchanged. This is
**not complete expression-evaluator independence** or generic container support.

### Shared Bounded List Construction

`StringListBuilder` supplies **shared incremental list bounds** for computed
`strings`, eager `split`/`append` and bounded list decoding. Owned pushes move
text buffers; borrowed pushes **check before allocating a copy**. Count or
aggregate UTF-8 overflow returns the closed `ScalarError::StringListLimit`,
consumes/drops rejected owned text, and leaves the accepted prefix unchanged.
Failure is explicit, not silent truncation. An adapter must propagate errors
instead of publishing a partial prefix; the reference VM and eager operations
stop before evaluating later entries or returning any list.

`new()`/`Default` construct empty data without allocation or authority. Explicit
`with_capacity()` rejects counts above 64 before allocation and preserves the
VM's existing bounded preallocation. `TryFrom<StringListValue>` validates direct
unchecked data, then adopts its vector/string buffers without cloning or changing
capacity; malformed input is consumed with a payload-free `UnboundedOperand`.
These are **logical bounds, not capacity or allocator guarantees**. Hosts still
own ingress limits, aggregate work and allocation failure. `finish()` moves the
prefix into the original publicly mutable list type, **not a lasting validation
certificate**; later mutation/ingress requires explicit boundedness checks.
Builder `Debug` is metadata-only. No fuel grant, execution frame, callback,
scheduling or authority is created by constructing or finishing data.

List wire decoding still ignores untrusted length hints and runs the original
element type/byte seed before the extra-entry count guard, retaining both fixed
error messages and their precedence. Whole-expression cold type/purity/capability
checks and HIR literal folding/source costs stay unchanged. The reference VM
keeps source-order evaluation, first-overflow fail-fast, final copy/materialization
fuel, non-recoverable `LSV1403` and existing durable re-entry. Tests captured the
computed-constructor fuel and error order before delegation and cover exact
UTF-8/count boundaries, unchanged failed prefixes and buffer ownership. No source
syntax, value/result/projection/continuation/journal bytes or schema changes.
This is **not full expression-evaluator independence** or a sandbox.

### Shared Borrowed Lexical Frames

`ScopeFrame<Value>` provides **shared borrowed-name lexical bookkeeping** for
adapter-owned values, without HIR, host-result or persistence dependencies. Reads
see all visible bindings; active duplicates are rejected without changing them.
Checked **frame-relative slots** allow direct mutation only within the current
frame, and popping an empty frame never removes a parent binding. Names borrow
immutable text; values move without a clone requirement. Nested frames reborrow
the stack and clean up their own suffix on success or failure.

Drop **detaches the entire suffix before value destructors** run, normally in
reverse insertion order. Panics propagate under Rust's unwinding rules; a second
cleanup panic can abort. The must-use guard is move-only, non-default and
non-serde. Its counts-only `Debug` never invokes a payload formatter; `bindings()`
is a **data view, not a redacted observation**. Conditional Send allows worker
handoff for suitable values without excluding GUI-local non-Send values.

This is **not an authority fence or saved execution frame**. Slots can be stale or
reused after a pop; explicitly forgetting a guard bypasses its cleanup. Interior
mutability and value destructors remain adapter code. Construction does not check
an existing prefix's uniqueness or bound its size. Push/lookup scan the visible
prefix; the adapter still owns bounded name/type/capability preflight, memory/work
limits, evaluation, fuel, durable capture and recovery. No host evaluation
callback, timer, global lock or implicit execution permission is introduced.

The reference VM delegates lexical bookkeeping for `bind`, pure `loop`/`fold`
and scalar/result/group re-entry, removing transient name copies. **Durable
captures remain owned** and retain original copy/projection fuel, name order,
fault mappings and continuation/journal bytes. Compatibility tests lock exact
fuel, failed-name cleanup before recovery and suspension after enclosing scopes
have ended. This adds no source syntax, host schema or sandbox and is not full
expression-evaluator independence.

**HIR preflight shares the same lexical guard** across source lowering, helper
parameters and residual scalar/group scope revalidation. Validated external names
borrow their original storage; nested locals no longer need temporary name copies.
Owned HIR and canonical output still outlive source/parameter buffers. Name grammar,
complete cold-branch typing/purity/capability checks, helper hygiene, declaration
order and expanded node/depth limits stay HIR-owned. Failed members restore lexical
state, **not visited-node budgets**; subsequent group diagnostics retain their
original order and source spans.

**Restored projection preflight uses the same guard**, tracking scalar, saved
result/group and pending-result names without payload copies. Scalar slots grant
no result fields; duplicate active names fail under the existing `LSV1405` frame
diagnostic. Alias/conditional field masks remain closed, including cold branches
and zero-iteration loop/fold bodies. This does not widen legacy projection versions,
replace normal HIR/canonical revalidation or bypass original request authority.
Source/IR/wire schemas and exact evaluation fuel stay unchanged. Sharing storage
and cleanup is not generic host-schema or complete evaluator independence.

### Shared Ordered Scalar Projections

`ScalarProjectionField<Field>` and `validate_scalar_projection()` provide
**opaque-key ordered scalar projection validation** without product operations,
HIR or persistence. Each developer owns the field-key type and trusted ordered
`(key, ScalarType)` schema. Validation requires exact count/order/keys, all six
scalar types and per-value language bounds; it does not normalize, reorder,
truncate or synthesize fields. Keys need only `PartialEq`, not cloning, formatting,
serde or Send. Fields and schema keys are borrowed without copying payloads.

The **single-pass borrowed schema** needs no Clone/exact-size requirement, second
collection, full count or size hints. At most `fields.len() + 1` iterator calls
are made. Native iterator/comparison callbacks remain trusted code: their work
is not preempted or sandboxed, and panics propagate. Validation does not mutate
fields or roll back native callback/interior-mutability side effects.
Host policy still bounds ingress bytes/field counts and aggregate work,
selects operation/version schemas and defines distinct keys.

**Payload-free projection errors** distinguish count/key/type/bounds failures in
left-to-right order. The reference adapter maps all to the original `LSV1405`,
then checks kind tokens and optional UI text: operation-specific domains remain adapter-owned.
`ProjectedField` is the same core type specialized
for its closed `ResultField`, not a parallel DTO. Projection v1-v5 JSON remains
exact, including omission of version 1, and no absent legacy field is filled in.
Cold field masks, canonical validation, original request authority, raw receipt
matching, saved fuel and continuation/journal/replay contracts stay unchanged.

These public fields are mutable data, **not a lasting projection certificate**,
authentic receipt or execution grant. Constructors/serialization and legacy
plain-string decoding remain unchecked; optional/list decoding retains bounds.
DTO Debug/serde can expose private keys/values and are not the payload-free error
surface. Standalone tests use **two unrelated projection schemas**, not the two
complete host-evaluator acceptance proofs. This is a shared result-data preflight
foundation, **not a generic host-operation or suspension engine**.

### Shared Typed Calculation Recovery

`ScalarError::is_recoverable()` and `CalculationFailure<External>` provide a
**typed closed calculation recovery class**. Only explicit scalar checked-arithmetic
and integer/boolean parse errors qualify. Scalar type/bounds errors and every
external error remain non-recoverable, **not inferred from diagnostic strings**.
`From<External>` and `?` preserve provenance even if the payload has a familiar
code/message or is itself a `ScalarError`; tagging it as scalar must be explicit.
Trusted adapters still own that provenance, not an in-process security sandbox.

This **move-only, non-serde failure value** can contain opaque non-Clone/non-Debug
and GUI-local non-Send errors. Thread handoff is conditional on the external type,
without a Sync requirement or interpreter-global lock. Queries only observe tags;
they do not invoke callbacks, run fallbacks, refund fuel or drop payloads.
**Metadata-only failure Debug** never invokes an external formatter. Matching the
enum exposes the original error; logging redaction and destructor behavior remain
host-owned. Cleanup unwinding propagates without inventing execution outcomes.

The reference VM now keeps **typed failures until the outer fault boundary**.
Unhandled scalar errors retain original `LSV1401/1402/1403/1408` diagnostics;
external faults move verbatim. Selecting a fallback does not require first
allocating its scalar fault code/message. Cold purity/type validation, lexical
cleanup, fallback laziness, shared fuel without refunds, original host validation,
durable JSON/schema versions and first-commit recovery stay unchanged.
This class **does not implement fallback evaluation or host-effect recovery**;
the shared pure executor below evaluates scalar fallbacks, while effect dispatch,
host-effect recovery and durable policy still belong to the reference adapter.

### Shared Structural Walk Accounting

`StructureBudget` provides **checked host-granted structural accounting** with
inclusive node/depth limits. Each physical visit costs one plus adapter-supplied
source expansion weights. Checked `usize` addition rejects overflow even at
`usize::MAX`; **depth is checked before node count**. Rejection preserves the
successful prefix count, not permission to skip a failed or cold branch.

`check_pending` is a **readonly physical-frontier check, not a reservation**.
The adapter still checks each queued node's full folded cost and depth on visit.
No graph, payload, callback, queue, traversal, allocation or deduplication lives
in the meter. Repeated/cyclic edges are counted by the compliant caller, not
recognized by the core. Native work, queue memory and child-depth arithmetic
remain host-owned. Limits are explicit; the meter has no Clone, Default or serde
handle and grants no execution or re-entry authority.

Three HIR paths now share the arithmetic: computation-shape validation, bounded
prepared atomic-signature inspection and canonical effect-shape validation.
**Source weights and graph policy remain HIR-owned**. Folded optional/list values
keep constructor costs; embedded host effects share their computation count.
**Effect and computation budgets stay separate**, with unchanged depth conventions,
`LSH1405/1204/1205` messages, traversal order, source syntax and durable wire.
Helper/source lowering retains its counter and fail-without-refund policy.

This is **not a type, authority or lasting graph certificate**, evaluator fuel,
memory quota, cycle detector, callback preemption or a generic HIR engine.
Prepared signatures still require full lexical/type/capability preflight. Tests
cover exact legacy boundaries before and after delegation, full-width overflow,
cold branches and unrelated native walk layouts, not independent evaluator proof.

### Shared Named Host Signatures

`NamedParameter<Key, Domain>` provides **opaque-key native parameter metadata**,
with host-owned domain descriptors and required-presence flags. Const constructors
do not interpret or validate domains. **Required presence is not non-nullness**:
the value and its accepted scalar types remain a separate receiving-host check.
These public fields are mutable metadata, not an operation registry, stable wire
schema or lasting certificate. Debug can expose keys/domains; no automatic serde
codec is supplied. Hosts still own versioned protocol adapters and ingress limits.

`validate_named_arguments` supplies **borrowed named-shape preflight without domain
callbacks**. Order is count before comparisons, duplicate schema keys, submitted
keys left to right (duplicate before unknown), then required declaration order.
It permits name reordering and optional omission, without evaluating values,
normalizing keys, changing order or filling in defaults. Typed errors contain
**positions without submitted-key or domain payloads**; adapters map diagnostics.
No allocation, clone, hash, format, serde or Send bound is imposed on keys/domains.

Work is quadratic in host-bounded slice lengths, not automatic resource metering.
Native PartialEq comparisons must be stable and can execute code, change interior
state or unwind; they are not preempted or rolled back. GUI-local non-Send schemas
work on their owning thread. The core neither calls domain validators nor knows
which scalar values or permissions a registered host operation requires.

**All 70 reference host signatures directly specialize the core metadata**.
Named preflight delegates to the core with unchanged `LSH1407` messages/spans,
declaration-order value evaluation, host-domain checks, capability checks, exact
fuel and durable bytes. Invalid names still fail before argument calculation;
explicit none/empty text retains its original receiving-operation semantics.

This is **not a generic operation registry or effect evaluator**, type/value/domain
approval, execution authority or automatic GUI integration. Unrelated native GUI
and device schemas prove shared parameter preflight only. Generic operation/profile
resolution into generic typed HIR and suspension still need the independent-host gate.

### Shared Borrowed Argument Binding

`bind_named_arguments` runs the existing shape preflight before returning a
`NamedArgumentBindings` **zero-allocation borrowed declaration-order view**.
This describes core-owned work; native key comparisons can themselves allocate.
Forward iteration yields **original parameter references and original input
indices**, omitting absent optional parameters without defaults. Names and
schema stay borrowed; no key/domain trait beyond PartialEq is required. Values
are neither owned nor evaluated. The host pairs indices with the same submitted
value sequence and still performs complete type/domain/authority preflight.

Borrowing fences direct mutation, **not native interior mutation or authority**.
Keys must retain stable equivalence throughout use. Iteration compares again,
bounded by both caller-bounded slice lengths; native comparisons can mutate
state, run arbitrary code or unwind without preemption/rollback. Metadata-only
Debug reports slice counts without invoking native key/domain formatters. No
serde codec or owned snapshot is supplied. Thread transfer depends on borrowed
metadata being Sync; GUI-local non-Send schemas remain supported on their owner.
Repeated/reverse inspection is not an instruction to execute twice or backwards.

All reference computed host source calls now use this view instead of a separate
declaration-order lookup. **Source binding changes no saved-HIR order policy**:
reordered source names produce the same canonical HIR, but noncanonical saved
HIR/residual frames are still rejected, never normalized. Existing named-error
priority, messages/spans, cold checks, host domains, capabilities, exact budget,
continuation bytes, saved fuel and first-receipt replay remain unchanged. Raw
value resolution retains its original submitted order and diagnostic priority.

Two unrelated native GUI/device schemas prove reuse of name binding only. This
is **not a value evaluator or dispatch/suspension engine**, operation registration,
authority certificate or automatic GUI adaptation.

### Shared Native Operation Catalog

`OperationSchema` declares **host-owned operation keys, parameter domains, result
descriptors and required capability labels** without product types or callbacks.
`OperationCatalog` borrows these declarations under a nonzero exact version and
explicit inclusive operation/parameter count limits. Zero limits are valid deny-all
policy. Registration checks version, operation count, every parameter count, then
duplicate operations and duplicate parameters; required inputs need not be present
to register a declaration. Errors carry positions, never native metadata payloads.

Lookup checks **exact version before native key lookup**; owned string keys accept
borrowed str queries without normalization. `authorize` checks a known operation
before required-label membership in trusted host grants. This is **metadata preflight,
not execution authority**: resource/revision/deadline checks, result validation,
dispatch and replay still belong to the receiving host. Schema argument binding
delegates to the shared named-shape/declaration-order view, without evaluating values.

No key/domain/result/capability clone, format, serde codec, default registry, storage
or implicit authority is provided. Debug exposes only version/counts. Core work
allocates nothing; registration is quadratic and lookup linear in host-bounded
counts. Native PartialEq/Borrow must keep equivalent keys and may run code or unwind
without preemption/rollback; hosts bound key bytes and native work. Borrowing fences
direct mutation, not interior state or a lasting schema certificate. Send/Sync
depend on borrowed metadata, permitting GUI-local non-Send declarations.

**All 70 reference operations now resolve source names and metadata through this
catalog**. An immutable cached registration validates the generated profile; enum
indices are local table positions, never protocol IDs. Internal catalog version 1
is not a continuation/journal/wire or release version change. Legacy operation
tags, accepted scalar matrices, optional semantics, diagnostics, authority, fuel
and durable bytes remain unchanged. Unrelated native GUI/device catalogs prove
registration/selection/binding only, **not generic typed HIR or a complete evaluator**.

### Shared Host-Parameterized Control IR

`leselang-hir::ir::Computation<Field, Operation, HostEffect, ResultType>` is the
**same control-node representation used by reference lowering and VM residuals**,
not a second tree or conversion layer. All four host slots are explicit, without
product defaults or trait bounds on native construction. Generic ComputedArgument
and ComputedBranch carry names, child expressions and declared result metadata.
The reference `computation::Computation`, ComputedArgument and ComputedBranch paths
are concrete aliases; qualified variant construction, matching and reference
inherent methods remain available. Rust enum-member `use` imports should target
the enum definition at `leselang_hir::ir::Computation`, not the concrete type alias.

Shared **borrowed child traversal includes cold branches in declaration order**
without allocation or inspecting native payloads. Forward/reverse/mixed walks are
fused and return original references; inspecting children is not evaluation order
or permission to execute repeatedly. Shared `is_pure` inspects language nodes:
Host/Call/Group are effectful even if empty or cold; Member is a result reference.
Its pending frontier allocates, so callers bound structure before inspection.
Opaque native Host graphs are leaves and require separate adapter validation.

IR remains mutable, unchecked data. Purity is **not type or execution authority**,
an execution budget or a lasting certificate. Clone/Debug/serde/Send/Sync are
conditional on native slots; GUI-local non-clone/non-serde data is supported.
Debug/serialization may expose payloads and decoding may allocate/recurse: adapters
own ingress limits, redaction, codecs and full cold type/domain/capability checks.
Unknown string operation keys can be decoded as data; catalog resolution must
reject them before dispatch. No evaluator, result-acceptance protocol, generic
type checker, suspension engine or storage backend is introduced by this IR.

Native GUI/device catalogs demonstrate different operation/field/effect/result
types in this shared IR. Golden tests preserve all 15 node tags, both group kinds,
field order and strict unknown-field decoding. Reference tests preserve source
lowering, authority, exact residual bytes, saved fuel, restart restoration and
first-receipt replay; invalid cold fields fail before effect admission. These are
**generic IR data and reference-host execution proofs, not two complete native
language pipelines**. Full source parsing/lowering/type inference, host results and
recovery frames still need generic host policy; the VM still requires SQLite.

An isolated source-subset check copies only HIR, syntax, host-contract, runtime-core
and product-neutral identity crates into a fresh temporary workspace with its own
manifest/lockfile. Native IR tests and doctests run offline without any product
source, SQLite, GUI assets or the original workspace metadata. This proves the
shared IR's build boundary, not independent full-language execution or publish readiness.

### Shared Native Argument Typing

`ScalarArgumentDomain` declares host-owned scalar alternatives and a native literal
predicate. `ScalarArgumentType` carries **borrowed, unchecked expression facts**:
an optional scalar type, purity and an optional direct literal. A missing type is
non-scalar, not the language none value; missing literal means unknown, not null.
The domain trait requires an explicit predicate; ScalarTypeSet supports domains
whose only constraints are accepted types and language bounds. Domain methods
are trusted native code, not evaluated script or automatically generated adapters.

`check_argument_type` applies **purity, type, literal consistency, bounds, domain**
in that order (accepted alternatives precede literal consistency). Impure/non-scalar
facts do not call domain methods. Wrong-type, inconsistent or unbounded literals
do not call the native predicate. Checks borrow data without core allocation,
normalization, coercion, execution or fuel charging. Payload-free ArgumentTypeError
tags are static failures, outside pure calculation recovery; Debug on facts reports
type/purity/literal presence without literal text.

`OperationSchema::check_argument_types` checks aligned fact/name lengths, the
**complete named shape before native domain methods**, then declaration-order
argument types. Errors report declaration and original submitted indices. Optional
omissions remain absent, and required presence is not non-nullness. Native key
equivalence must remain stable; hosts bound counts/key bytes and native work.
Native methods may allocate, mutate interior state or unwind without preemption
or rollback. Success is **not checked IR or execution permission**, a version grant,
result certificate or durable continuation; catalog selection and authority remain
separate. The type facts supplied by a malicious adapter are not a security sandbox.

Shared `Computation::scalar_argument_type` provides the **same cold-aware IR bridge**
to reference lowering and unrelated native hosts, without inspecting/cloning host
slots. It borrows direct literals, but trusts the separately inferred type. Its
purity frontier allocates, so structure must be bounded first. The bridge does not
resolve unknown locals, type native fields or validate opaque effect graphs.

All 70 reference domains use the shared checker; source diagnostics/spans, cold
checks, domain matrices, argument order, authority, fuel and durable bytes remain
unchanged. Source lowering preflights names, lowers each argument in declaration
order, then uses the single-argument checker to preserve lowering-error precedence;
the schema-wide method accepts already inferred facts, not an evaluator callback.
**Dynamic type success is not actual value/domain acceptance**: no literal
predicate runs for an unknown value. The evaluated value must still pass receiving
host-domain checks before effect publication; mutated values require revalidation.
Two native GUI/device catalogs and IRs prove typed signatures/bridging, not complete
unrelated-host language pipelines. Full generic source lowering/type inference, host
result acceptance and suspension remain pending; the VM still requires SQLite.

### Shared Bounded Pure Type Inference

`leselang-hir::pure_typing::infer_pure_type` performs **real inference over the
same shared IR**, not a second tree or caller-provided type stamp. It covers all
12 pure node kinds: literals, strings, locals, fields, members, binary/unary
operations, bind, loop, fold, choose and recover. Host/Call/Group nodes are rejected
even if empty or cold; effects captured earlier are represented only by externally
supplied result-type metadata. PureType carries a closed scalar type or an opaque
host result identifier, not an evaluated value, receipt or durable frame.

`PureTypeEnvironment<Field, Operation>` provides **host-owned field, member and
type-join queries**. Result identifiers require Clone and stable PartialEq;
borrowed identifiers support non-clone/non-Debug/native-local payloads without
copying them. Equal identifiers must have interchangeable type-query behavior.
The four IR slots need no Clone/Debug/serde/Send bounds for inference. The default
result join requires exact equality. An explicit compatible join may discard
exports, but must retain **only common exports, never a branch union**. Member
queries must reject names/operation tags outside the exact bound group. These
are trusted native queries, not effect execution, automatic GUI adapters or a
security sandbox; native clone/equality/query/drop work and unwinding are host-owned.

TypeInferenceLimits is explicit inclusive policy, without a default: physical
nodes, depth and active bindings. Fixed safety ceilings are 16384 nodes, depth 64
and 1024 bindings. Zero nodes denies all, zero depth allows a leaf, zero bindings
forbids locals. Local names retain the existing bounded ASCII grammar. Limits,
scope counts/names/uniqueness and **the entire pure physical tree are preflighted
before native metadata cloning or queries**. The structural frontier and lexical
storage allocate within these limits; metadata identifiers may be cloned, but
names/IR/native payloads are not. Physical limits are not source-expansion costs,
fuel, prior allocation bounds or native callback preemption.

Inference checks **all cold operands and zero-limit bodies**: conditional branches,
short-circuit right operands, recovery fallback, loop condition/next and fold next.
Sibling bindings do not leak and active names cannot be shadowed. Loop/fold state
types must be preserved; member aliases carry their closed result metadata. No
guard, arithmetic, parsing, collection construction or loop iteration executes.
Well-typed division by zero or invalid decimal text remains a runtime failure;
dynamic values/domains still require execution-time validation. PureTypeError
contains fixed static tags, never local names, literal text or native payloads.
Success is not authority, a lasting type/bounds certificate or a wire handle.

The reference adapter **corroborates pure residual types after the mandatory
canonical roundtrip**. It preserves existing diagnostics, source weights, external
scope limits, closed-group checks and frozen projection fields. Generic type
success cannot promote noncanonical HIR into accepted reference HIR. Conservative
result joins retain the legacy Structured identity without creating a union of
group members. No command, authority, fuel, continuation version or durable bytes
change. The canonical gate is not replaced by native type inference.

Two different native GUI/device schemas infer projection/control-flow types, and
a native preparation path combines catalog selection, named binding, pure argument
inference and signature typing. An isolated five-crate workspace verifies the
same public API without product sources or SQLite. These are **pure inference and
typed preparation proofs, not complete source-to-effect language pipelines**.
Generic source lowering, full effect-flow typing, complete reply-lifecycle acceptance
and suspension/recovery frames remain pending; the production VM still requires SQLite.
Received-value validation and pure execution are shared by the later layers below.

### Shared Bounded Atomic Call Inference

`leselang-hir::call_typing` connects **real shared Call IR to native signatures**.
`check_call_arguments` accepts a receiving host's already selected OperationSchema;
`infer_call_type` selects one from CallTypeHost's borrowed native OperationCatalog,
exact version and trusted capability labels. The latter accepts **only Call roots**,
not arbitrary Host payloads, groups or prepared Bind/Choose flows. Both entries
infer each submitted expression themselves through the shared pure engine, rather
than trusting caller-written scalar types. Parameter names are exact bounded ASCII
language identifiers; native operation/field/effect/result slots remain explicit.

**One aggregate physical budget includes the call root and every argument tree**.
Depth starts at zero for the root and one for an argument. CallTypeLimits embeds
the existing explicit pure node/depth/binding policy and bounds both argument and
selected parameter counts, with a fixed ceiling of 64. Zero arguments admits a
parameterless signature when the root fits its budget; it is not a deny-all mode.
Cold and zero-iteration subtrees retain pure inference's grammar, bounds and type
checks. Hidden Host/Call/Group argument effects remain rejected.

The full argument forest and prefix are structurally preflighted **before native
catalog comparisons, metadata cloning or type/domain queries**. Catalog version,
operation and trusted-label checks then precede signature inspection; the selected
parameter count and complete named shape precede inference/domain queries.
**Declaration-order checks preserve original input and parameter positions**,
including reordered names and omitted optionals. Shape errors carry the existing
closed NamedArgumentError tags. CallTypeError contains only fixed errors and
positions, never private argument names, text or native metadata. CallTypeHost's
Debug reports version/grant counts without invoking native formatters.

Successful checks **return the original borrowed result declaration**, without
cloning schema result payloads or exposing an executable handle. Native keys,
domains and schema result metadata need no Clone/Debug/serde/Send bounds; pure
query result identifiers retain their explicit Clone/PartialEq contract and may
borrow non-clone GUI-local payloads. Core-owned frontier/name/scope storage is
bounded, but native equality, metadata cloning/drop and type/domain queries remain
trusted developer code: they may allocate, mutate interior state or unwind, with
no preemption, effect dispatch or callback rollback. Direct literals receive
domain checks; hosts separately bound catalog/grant counts and metadata payloads.
Dynamic values still require validation after evaluation. Arithmetic
is not executed: well-typed division by zero remains a runtime fault.

The reference adapter **corroborates residual Call types after the existing
canonical roundtrip**, through its original operation signature and frozen field
schema. It does not replace source lowering, widen closed group exports, bypass
authority or accept literal-only/noncanonical Call wrappers. Existing source
diagnostics/spans, declaration-order saved HIR, evaluated domain checks, wire bytes,
fuel, continuation versions and journals stay unchanged.

Two unrelated native operation schemas and an isolated five-crate build prove
bounded atomic call inference, **not complete host-neutral effect-flow typing**.
Source lowering, general prepared signatures, group/capture flow typing, host-result
acceptance, execution and suspension/recovery remain separate acceptance gates.
No automatic GUI adaptation, FFI ABI or independent language publication is claimed.

### Shared Call-Only Atomic Preparation

`leselang-hir::prepared_typing::infer_prepared_call_type` extends native Call
inference with **pure Bind/Choose wrappers and one call on every path**. Bind values
and guards use the existing bounded pure engine, including loops/folds/recovery and
closed external result/group aliases. Guards must be boolean; active names cannot
shadow and sibling names do not leak. Type checking visits both cold paths without
evaluating guards, arithmetic or effects. A result alias is externally supplied
type metadata, not a new capture or accepted receipt.

**Three phases preserve the complete cold boundary**. First, the prefix and whole
physical prepared tree are preflighted under the same explicit CallTypeLimits:
wrappers, guards, binding values, terminal calls and all arguments share one
inclusive node/depth budget. Hidden effects in preparation or operands, malformed
names, oversized literals and unsupported scalar/Host/Group terminal flows fail
before any native comparison, metadata cloning or query.

Second, **every cold terminal schema and named shape are checked before type
queries or prefix cloning**. PreparedCallSchemas is a developer-owned selector of
the original OperationSchema rows, not a registry copy or dynamic effect host.
CallTypeHost implements it through exact-version catalog lookup and trusted
capability labels. Custom profiles own equivalent policy and must return stable
original row identities: the same operation selects the same row and distinct
operations must not share a row. The engine requires **the exact same declaration
on every path, not merely equal result tags**. Identity comparison is internal;
no pointer token is exported or persisted. Native selection remains trusted code,
not a sandbox, callback preemption or execution authority.

Third, pure preparation and all terminal arguments are inferred in original
branch/declaration order. **One guarded lexical type scope is reused**, rather than
cloning an entire prefix for every call or argument. Only query identifiers may
be cloned; names/IR/literal text/operation keys/schema results are not copied.
Native query/equality/clone/drop work and interior state remain host-owned and
may unwind without rollback. Cleanup releases temporary aliases and retains the
caller's external scope; no effect, execution identity or journal is created.
Closed errors carry a preorder cold-call index and existing argument/parameter
positions without source or native payloads. Dynamic values still need receiving
domain validation after evaluation.

Success returns **the original borrowed result declaration, not an executable
prepared request**. PreparedCallSchema and SelectedPreparedCall are direct native
schema/borrow aliases, not another HIR, signature DTO, wire codec or durable frame.
Opaque Host leaves, groups, captures, multiple effects and scalar early exits are
deliberately unsupported by this call-only entry. Returning the declaration grants
no version/revision/receipt/dispatch authority and is not a lasting certificate.

The reference adapter **corroborates eligible prepared calls after the old canonical
and prepared-operation gates**. Only call-terminal Bind/Choose trees use this new
entry. Literal Host leaves and mixed/group flows retain their original validation
paths. All-call capture chains also use the separate flow check below; neither
entry weakens or narrows the original source rules.
Source diagnostics, capability checks, closed group fields, source expansion costs,
wire bytes, fuel and durable/recovery contracts remain unchanged.

Two unrelated native schemas, metadata-clone counters and an isolated five-crate
workspace verify this call-only typing layer. They are **not complete independent
source-to-effect or suspension pipelines**. Opaque-host adaptation, generic source
lowering, opaque-host flow typing, host-result acceptance, execution and optional
persistence remain separate 2.3.0 extraction gates.

### Shared Call-Result Dataflow Typing

`leselang-hir::flow_typing::infer_call_flow_type` checks **call result binding,
field projection and subsequent calls or pure scalar tails** directly in the
shared IR. Bind values may be calls, pure values or Bind/Choose chains. Choose
guards and all argument, field, operator, recovery, loop and fold operands remain
pure, including cold and zero-limit children. Newly constructed Group nodes and
opaque Host leaves are deliberately not adapted. Externally supplied group type
metadata still uses the existing closed member-query protocol, not new exports.

CallFlowEnvironment adds **explicit declaration-to-query-type mapping** to the
PureTypeEnvironment protocol. It receives the original borrowed schema result,
not a received reply, and returns a scalar type or native result identifier.
Borrowed identifiers can refer to nonclone, nonserde, GUI-thread-local native
payloads. Missing mapping is a closed ResultType failure, not implicit acceptance.
This does **not validate an actual host reply or create a receipt**.

The **whole physical tree is checked before any native callback**. All cold
catalog/version/capability/named-shape checks then finish before prefix cloning,
argument/domain queries or result-type mapping. One aggregate node/depth budget
includes capture values, bodies, guards and every argument. The explicit max_calls
bound counts **all cold call sites, not the selected execution path**; it is not
fuel, graph capacity or a dispatch reservation. Original call/parameter/input
positions remain available without source names or native payloads in errors.

One guarded lexical scope holds pure aliases and declared captured-result types,
with a single prefix copy, active-shadow rejection and sibling-local cleanup.
Forward references cannot use the result being bound. Compatible native result
joins retain **only common exports, never a union**. This general flow entry may
join distinct operation results, unlike atomic preparation's exact declaration
identity requirement. It does not authorize such a flow for a source profile or
group scheduler; existing atomic/group classification remains a separate gate.

The reference adapter **checks eligible all-call capture chains after canonical
roundtrip and legacy source validation**. Literal/mixed Host flows keep their
adapter path; explicit native groups use the additional entry below. Wire bytes,
diagnostics, authority, fuel, suspension
and durable/replay contracts are unchanged. Native metadata/query unwind releases
temporary lexical storage without replaying effects or mutating caller scopes.

Two native profiles, cold rejection/unwind/clone tests and an isolated five-crate
source subset verify this layer. This is **type observation, not an independent
source-to-effect or suspension engine**. Opaque-host adaptation,
generic source lowering, complete actual-reply lifecycle, effect dispatch/suspension
and optional persistence remain extraction gates. Later layers below add native
received-value checks and shared pure execution without closing those vertical gates.

### Shared Flat Named Group Typing

`flow_typing::infer_group_flow_type` adds **explicit flat named group typing** to
the same call-result dataflow engine. The call-only entry still rejects Group
nodes; its grammar and error boundaries do not change. Sequence needs at least
one member, Parallel at least two; the caller supplies group-site and per-group
member limits, with fixed ceilings and bounded unique nonempty ASCII names.
Reserved local words can still be member names. Cold sites and physical nodes
are counted inclusively, not as dispatch slots, selected-path fuel or concurrency.

Every member is **pure preparation ending in one uniform original operation
declaration**. Nested groups, result captures, multiple member effects and scalar
exits are rejected, including cold paths. The physical atomic preparation walk
is shared, with one enclosing node/depth budget instead of fresh per-member limits.
All cold catalogs, versions, capabilities and named shapes finish first; **exact
schema row identity for each member precedes prefix cloning and type queries**.
Different members may use different operations; result-tag equality never permits
one conditional member to mix distinct original operation declarations.

GroupFlowEnvironment supplies **original branch declaration matching and closed
ordered group metadata construction**. After inference, every original IR branch
result declaration is matched against its inferred query type before any group
exports are constructed. GroupMemberType borrows original names/operation slots
and moves ephemeral query metadata, not native result payloads or IR trees. There
is no implicit builder, wire codec, receipt, partial-completion table or saved
handle. Native implementations own metadata work, stability, closed exports and
unwind/redaction; this protocol is not a sandbox or automatic GUI integration.

Both modes share **only the enclosing lexical prefix, never earlier member
receipts or sibling locals**. A captured group becomes queryable only in its body.
Aliases preserve its closed metadata; member lookups require exact exported names
and operation tags. Native joins retain only common exports. The reference profile
preserves exact mode, order, names and operations, never a union or reordered map.
Its new metadata borrows original names/tags behind a cheap cloneable type view.

The reference adapter **corroborates all-call groups after legacy structure and
canonical source gates**. Opaque Host leaves, literal/mixed groups and source-only
flattening continue through the old adapter; nested group/capture/graph restrictions
remain unchanged. No evaluator, permission, wire, receipt, fuel, recovery or journal
format is altered. Callback unwind releases temporary type metadata without
replaying effects or mutating the caller's prefix.

Two unrelated native profiles and an isolated five-crate source subset verify
typing, declared-type forgery rejection, closed members, budgets and unwind.
This is **group type metadata, not actual group replies or execution scheduling**.
Opaque-host adaptation, generic source lowering, actual reply acceptance, execution,
suspension and optional persistence remain incomplete independent-runtime gates.

### Shared Closed Group Export Observation

`group_exports::observe_group_exports` checks one **closed ordered export signature**
across all cold group return paths, including pure-prepared Bind/Choose wrappers,
explicit flat IR groups and adapter-observed opaque Host groups. This replaces the
reference source-capture signature walker, not the existing typed group-flow engine.
It is **group export observation, not full source lowering, flow typing or authority**.

**Complete cold physical and candidate-route checks precede native hooks**. Explicit
`GroupExportLimits` bound physical nodes/groups at 16,384, depth at 64 and members
at 64. Sequence requires at least one member; parallel requires at least two.
Zero node/group/member quotas deny observations; depth zero allows only the root.
Language local/member and call-argument
names, call argument count (64), literal bounds,
loop/fold limits and group member arity/unique bounded names are checked first.
Then every return route must be a group behind structurally pure preparation;
IR members must be atomic candidates with only pure preparation/call operands.
Nested groups, captures, scalar exits and effectful guards/initializers are rejected,
including cold and zero-limit controls. All statically known group kinds, member
names and declaration orders must agree before any operation observation.

**Native schemas and opaque graphs remain adapter policy**. Hooks receive the exact
original Host or IR branch including its original result declaration. The adapter
corroborates all cold schemas/declared returns, boolean/lexical typing, versions,
capabilities and closed separately bounded native graphs. Opaque graphs are not
traversed by the shared language walker. A matching shape never grants execution.
Native group observations must also pass kind/arity/name checks before the explicit
fallible operation-identity comparator. Same spelling or result tags are not exact
schema identity; no member union, sorting, coercion or map-based reordering is added.

Groups are observed once in rightmost-first DFS order; each IR group's branches
and comparisons run once in declaration order. The final leftmost observation
moves out unchanged. `GroupExports` borrows original member names and moves opaque
operation observations with no native Clone/Debug/serde/Send/PartialEq requirement.
Debug reveals only kind/counts. Errors retain original native payloads for matching,
not formatting or source chains. Failure/unwind releases partial observations once,
returns no partial signature and never retries, dispatches or rolls back native work.
Borrowed IR is not consumed or rebuilt. Native allocation, interior state, identity,
Drop and unwind remain trusted; fresh live policy checks remain mandatory before use.

The reference source-capture adapter keeps its prior full structural/native-graph
gate before this shared entry, and retains exact prepared atomic operation checks,
canonical bytes, result-flow quotas and wire/capability policy.
Captured-group continuation quotas reuse the already checked group width instead
of querying/cloning the same signature again. Two unrelated native
formats exercise original names/schema identity. A parsed source/catalog-to-export
pipeline also prepares actual argument values through the shared interpreter with
original schema identity, exact fuel and scope cleanup, without product services
or SQLite. This is not a complete source-to-suspension/reply lifecycle proof; generic
host-result bind source, opaque flow typing, full helper compilation/registry and
complete suspension ownership remain separate extraction gates.

### Shared Received Host Results

`leselang-runtime-core::HostResultDomain<Reply>` adds **actual received-result
type and value validation**, separately from HIR's declaration-to-query-type
mapping. A developer supplies `matches_type` and `validate_value` for its original
native result descriptor. Neither method has an accept-all default.
`validate_host_result` checks the **result kind before native value validation**;
`OperationSchema::check_result` uses the same borrowed declaration selected during
static call inference. A matching type alone never accepts a stale or invalid value.

`ScalarTypeSet` implements the protocol using explicit alternatives and existing
language bounds for all six scalar kinds. None, absent optional text and empty
text remain distinct; no parsing, coercion, implicit nullability or synthesis is
performed. Core-owned work borrows only: **no reply or schema buffer is copied**,
normalized, truncated or formatted. No expression is evaluated or fuel charged.
Native descriptors, replies and errors need no Clone, Debug, serde or Send;
unsized replies, GUI-local ownership and native trait objects are supported.

`HostResultError` preserves **TypeMismatch versus InvalidValue(original_error)**.
Debug/Display never format native errors and Error exposes no private source chain.
The native error is moved unchanged to the owning adapter, not translated by its
diagnostic code, cloned, serialized or promoted into pure calculation recovery.
Explicit extraction may expose private payloads; adapter logging must redact them.
Native callbacks may allocate, mutate interior state or unwind. The core does not
roll back those changes, preempt callbacks, install retries or replay an effect.
Hosts bound raw ingress, native work and aggregate resources before validation.

The reference VM **revalidates bound raw results before projection or successor
materialization** using this protocol and the existing closed Value kind mapping.
The original raw result/request/revision correlation checks stay mandatory and
precede this gate. Existing per-value/output-item/serialized-byte validation
remains native policy, with exact native fault codes/messages. The added wrong-kind
guard maps to `LSV2103`. Projection v1-v5, continuation schemas 1-11, journal 10,
saved fuel, authority and transactional first-commit replay remain unchanged.

Successful validation is **not an authentic receipt, lasting value certificate,
accepted continuation or execution authority**. Version/capability lookup, evaluated
arguments, pending-effect identity, deadlines, stale-reply fences, group barriers
and durable replay remain adapter-owned. This API does not correlate an operation
or execution ID on behalf of the host and is not a complete generic reply lifecycle.

Two unrelated native schemas combine shared Call inference with received-result
validation; runtime-core ownership/bounds/unwind tests and reference VM regressions
cover this boundary. These are **static-call/received-value proofs, not complete
independent source-to-effect or suspension pipelines**. Generic source lowering,
opaque-host adaptation, effect evaluation, suspension and optional durable recovery remain
the extraction gates; no independent repository move is claimed.

### Shared Bounded Pure Execution

`leselang-hir::pure_evaluation::evaluate_pure_in_scope` now executes the original
generic `ir::Computation` directly. All twelve pure forms share one synchronous
implementation: literals, string lists, locals, fields, members, bindings,
conditionals, scalar recovery, loops, folds and unary/binary operators. No second
tree, product command/result type, SQLite backend, thread or global lock is needed.

`PureEvaluationEnvironment` projects **actual native result values**, unlike
`PureTypeEnvironment`'s static metadata queries. The developer owns exact field/
member exports, group identity and operation matching. Borrowed views or GUI-local
`Rc` values need only Clone for lexical reads, not Send/serde/Debug; native slots
and errors impose no such bounds. Native cloning/projection may allocate, mutate
interior state or unwind, and must not dispatch effects. Language fuel cannot
preempt or meter that native work. Received-result validation remains a separate,
mandatory host gate; a scalar projection is not an authenticated receipt.

Explicit `PureEvaluationLimits` bound physical nodes, depth and active bindings,
with fixed ceilings of 16,384/64/1,024. The entire physical tree, including cold
arms, and prefix names/uniqueness/scalar bounds are checked **before native queries,
native value cloning or fuel charges**. Host/Call/Group are rejected even cold.
This is structural preflight, **not cold static type checking**: hosts separately
call `infer_pure_type` before execution. Source expansion and prior allocations
are not metered by this API. Native scalar projections are bounds-checked before
language copying, and the caller's visible lexical prefix is never cloned or mutated.

Selected nodes and text/list work retain the reference fuel schedule. Short-circuit
operands and unselected conditional arms do not execute; zero-limit loops check
the condition before next-state evaluation. Fold evaluates collection before initial
state, rejects insufficient limits without truncation, and moves each item in order.
Only typed arithmetic/parse failures enter scalar fallback. Fuel, structural,
binding, loop/fold and native failures remain external, with no code-string inference,
refund or retry. Payload-free fault/value formatting never calls native formatters.
Lexical guards clean temporary bindings on ordinary failure and native unwind.

The existing VM **delegates pure operand, loop, fold, list and operator execution**
to this module and uses `PureValue<ResultView>` directly as its lexical value.
It no longer duplicates those algorithms or converts/copies the prefix into a
second evaluator's scope. Effect-position control now uses the shared effect walker;
native effect materialization, group admission and durable capture stay in the reference adapter. Original fault
mapping, authorization, fuel, continuation schemas 1-11, journal 10, projections
v1-v5 and transactional receipt/successor replay remain unchanged.

`crates/leselang-hir/tests/pure_evaluation.rs` proves an editor's static type check,
actual received-result validation and continuing computation, plus an unrelated
borrowed device-group host with different native slots. Tests cover laziness,
exact fuel, resource/recovery separation, bounded recursion, lexical cleanup and
native unwind. Reference VM regressions execute the same engine. This is **shared
pure execution, not complete source-to-effect suspension or durable host-neutral VM**;
those vertical extraction gates remain open.

### Shared Atomic Call Preparation

`leselang-hir::call_evaluation::prepare_call_in_scope` connects the original shared
Call IR, native catalog and actual lexical values without product types. One
aggregate physical budget includes the call root and the entire argument forest.
Whole forest/prefix preflight precedes native catalog comparisons; exact version,
trusted capability labels and complete named shape precede value execution.
Cold static typing remains a separate `infer_call_type` gate, not something this
dynamic preparation API certifies. Limits reuse the pure executor's fixed safety
ceilings and the 64-argument ceiling.

Arguments execute once in declaration order, preserving original parameter and
submission positions. Optional omission stays omission, not an inserted null or
default. **All values are evaluated before native domain callbacks**. Each value
then passes explicit scalar type, bounds and native domain checks, without coercion
or implicit nullability. The existing guarded lexical frame is reused directly;
there is no per-argument prefix clone or repeat physical-tree walk.

`PreparedCall` retains the original borrowed schema row, borrowed declaration names
and moved scalar buffers. It is must-use, non-Clone and non-serde; metadata-only
formatting does not print names, values, capabilities or native schemas. Extracted
parts are mutable data, not lasting validation certificates. The owning host must
still check live authority, revision/deadline policy and pending-effect correlation,
and validate the actual returned value against the original result declaration.
This module never dispatches effects or creates receipts, identities or journal rows.

The catalog entry charges one call-root fuel unit. The already-selected-signature
entry `evaluate_call_arguments_in_scope` counts that physical root but leaves its
execution charge to the caller. Both preserve the reference argument evaluation
and scalar-copy fuel schedule. Typed scalar failures retain their recovery class;
native failures remain opaque and external. Failure returns no partially prepared
request. Native projection/domain callbacks can mutate or unwind; they are not
preempted, rolled back or retried. Lexical guards remove temporary locals on unwind.

The reference VM now delegates computed argument preparation through this same
engine. Legacy literal resolution, canonical input gates, fault mapping, execution
authorization and durable continuation/journal formats remain adapter-owned and
unchanged. The selected-signature API does not replace catalog authorization.

`crates/leselang-hir/tests/call_evaluation.rs` joins static call inference, actual
value preparation, caller-owned invocation, received-result validation and a pure
tail for a GUI-local editor, plus an unrelated numeric device schema. It covers
declaration order, original row identity, aggregate limits, denied grants/version,
optional presence, typed failures, fuel and native unwind. This is **atomic call
preparation, not full effect dispatch, suspension or durable host-neutral execution**.
Generic source lowering and complete unrelated-host vertical proofs remain open.

### Shared Native Source Call Bridge

`leselang-hir::source_call::lower_source_call` connects the original parsed
`Expression::Call` to a developer-owned `OperationCatalog` and the shared IR.
It does not add a parser, operation-name switch, private syntax tree or product
operation defaults. `SourceCallHost` supplies trusted exact version and grants;
native string-borrowable keys can be non-Clone and non-Debug. The returned
`LoweredSourceCall` borrows the **original schema and AST argument names**.

The complete physical source call, including all cold operands, is bounded before
any native catalog lookup. Counts, nesting, call/argument/reference names and text
sizes have explicit limits with fixed safety ceilings. AST bounds do not establish
source/span authenticity; callers retain the parser and validated SyntaxTree
decoding boundary rather than trusting forged AST data. Native lookup then checks
exact version, operation and capability; the selected parameter count and **all
submitted names precede operand lowering**. Required/optional presence remains
distinct from an explicit `none` value. A limit applies to every physical AST call,
including constructors inside operands, not just the selected outer operation.

The native operand callback receives the original borrowed `NamedArgument` in
parameter declaration order. It returns a native shared-IR node and a trusted
inferred scalar type; this bridge does not infer that type or verify lexical
bindings. One separate produced-IR node/depth budget spans the call root and all
argument trees. Structural purity and bounds precede shared type/literal-domain
checks; preflight reuses established purity without a repeated tree walk. A certainly
exhausted generated node budget rejects before the next native operand callback.
Lowering and domain checks are sequential in declaration order, preserving the
reference adapter's error precedence; they are not an all-output atomic
transaction. Dynamic values require domain validation after evaluation.

Each prepared argument retains **original submitted and declaration positions**.
Language names are borrowed until `into_arguments` materializes them once; native
nodes move without a second IR, Clone, serde or Debug requirement. A failed callback,
generated-shape check or domain check releases partial output. Native query, equality,
callback and destructor work can allocate, mutate or unwind; it is neither retried,
preempted nor rolled back. The callback's lexical scope, source-expansion accounting
and any external effects remain its responsibility. Diagnostics expose closed tags
and positions, not native errors or private submitted names.

The reference computed-call lowerer delegates its former signature-order parameter
loop to this bridge. Its private preflighted entry preserves the existing parser,
helper expansion, lexical checking, diagnostics/spans and source costs; the public
entry additionally applies whole-source cold physical preflight. Literal operations
still use their legacy resolver. This adds no schema, continuation or journal format.

Native tests parse a numeric device call, retain exact AST/schema identity and
submission positions, then prepare its actual computed values through the shared
executor. An unrelated text/panel schema proves optional omission versus explicit
none and buffer-preserving IR handoff. Other tests cover version/grant/name failures,
cold AST rejection before native queries, separate aggregate source/generated budgets,
forged native facts, cleanup and native unwind. This is an **atomic source-call bridge,
not full source lowering or type inference**: shared scalar lowering below covers
literals/operators, while the host still supplies other operand lowering,
opcode mapping, complete cold type checking, helpers, opaque effects and live authority.
It is not execution, suspension, persistence or a complete extraction proof.

### Shared Scalar Source Lowering

`leselang-hir::scalar_source::lower_scalar_source` lowers parsed scalar literals
and all **23 binary and 7 unary operators** into the original generic control IR.
The reference compiler delegates the same one-form construction, including named
operand rules and core operator signatures. No second grammar, product opcode
default or parallel expression tree is introduced. Its private entry retains
legacy lexical/helper checks, diagnostic codes/spans/order and source accounting;
the public bounded entry adds whole-source preflight, not a new reference error order.

The complete cold physical AST and **all reserved operator named signatures precede
native source extensions**. Binary operands lower in left/right signature order,
regardless of submitted order; both short-circuit operands are lowered and typed.
Lowering does not perform arithmetic or parse text, so division by zero and invalid
integer text remain execution faults rather than new compile-time calculations.
Named preflight does not allocate a separate operand vector for each cold operator.

On the legacy unscoped entry, locals, bindings, loops, folds, helpers and native
projections require an explicit trusted extension receiving
the **original borrowed AST expression**. It returns native IR plus an inferred
scalar-type observation. Produced IR is checked for physical purity, bounded names,
literals, nodes and depth; a literal must agree with its reported type. Dynamic
observations **do not replace complete lexical cold type inference**. A forged
observation for an unbound local can pass this construction boundary but must fail
the shared pure type checker before evaluation. Native opcodes, fields, helper
expansion, binding scopes and opaque-effect typing remain developer-owned. The
shared scoped and projection entries below can be composed in that extension;
this entry does not silently choose an adapter's field or result schema.

One aggregate generated-IR budget charges primitive constructors before folding
and every physical native extension node. An exhausted minimum node/depth budget
rejects before the next extension callback. **Optional literal buffers move without
a second copy; folding does not refund source construction cost**. Hidden native
expansion and allocations are not automatically metered or preempted. Native errors
remain available by matching, with payload-free formatting; failure/unwind releases
partial native IR without retries, rollback or a budget refund.

Tests cover every scalar type combination, all operators, original AST identity,
native field-to-type-check-to-value execution, buffer identity, cold rejection,
aggregate expansion limits and cleanup. The source-call device proof now uses this
shared scalar lowerer rather than a test-only mini grammar. This is **shared scalar
source construction, not a complete generic source compiler or type certificate**.
Ingress/span authenticity, all cold typing, native callback work and live authority
remain host-owned. No continuation, journal, wire format or dispatch policy changes.

### Shared Scalar Source Control

The same `lower_scalar_source` entry now constructs **choose, recover and strings**
without a native callback for those forms. Their named signatures and all string
labels are checked across the complete cold AST before native extensions, including
controls nested inside an adapter-owned field/helper form. This reserves language
constructors; it does not automatically implement the surrounding host adapter.

`choose` lowers its boolean condition before both branches, in when/then/otherwise
order, and requires equal observed branch types. `recover` lowers value then
fallback, requiring two pure expressions of the same scalar type. `strings` keeps
submitted entry order, requires unique bounded language labels and pure string
items, and bounds literal lists to **64 entries and 4096 aggregate UTF-8 bytes**.
No condition, calculation or fallback executes during construction. All cold
children are constructed and checked against observed scalar signatures; execution remains selected
and lazy through the shared pure executor. Native observations still require the
complete cold lexical/field type checker before evaluation, not just signature checks.

The reference compiler delegates the same constructors and named rules. Its private
choice constructor keeps general branch type identity, preserving **non-scalar and
effect branches in the reference compiler** rather than restricting existing host
programs to scalar-only choices. The public entry accepts only pure generated IR
and returns a scalar observation. Legacy diagnostics, spans, child-error precedence,
lexical policy and source accounting remain in the reference adapter. In particular,
public cold name preflight does not redefine legacy sequential label-error order.

**Literal list buffers move without speculative prefix copies**. A wholly literal
list folds by moving owned text into the bounded core builder; a mixed list keeps
the exact literal and native nodes instead of copying a prefix that will be discarded.
One aggregate generation budget counts the list/control roots and all children
before folding, including cold branches. Folding does not refund constructor work,
and exhausted node/depth policy rejects before another native callback. Host-owned
hidden expansion and allocations remain separately bounded by the adapter.

Native source errors/unwind release partial branches/list nodes without retries or
rollback. Source `recover` does not catch lowering errors. During execution only
the closed scalar calculation class is recoverable; native field/effect failures,
static rejection and budget exhaustion do not become fallback success.

Product-free proofs cover parsed native GUI fields through cold inference and actual
control execution, original AST identities/order, lazy value access, literal/mixed
buffer identity, aggregate budgets, partial cleanup and an original native panel
schema through atomic argument preparation. This is **scalar control source
construction, not generic helper or host-result source lowering**. Opaque-host flow,
dispatch, complete suspension ownership and durable restart proofs remain open.

### Shared Scalar Source Bindings

`lower_scalar_source_with_scope` adds **pure scalar bind/local construction** on
the original generic IR. It takes `ScalarSourceLimits { source, max_bindings }`,
a borrowed scalar-type prefix and an explicit native extension. No default grants
locals: zero bindings forbids prefix/local bindings, and the fixed ceiling is
**1024 active bindings including the prefix**. Source and generated-IR node/depth
limits remain independent; bind and local constructors consume the same aggregate
generation budget, including cold branches, with no folding refund.

The complete physical source, reserved signatures, prefix names/types and every
cold source binding's shape, label, shadow and active quota are checked before
native extensions. Prefix names must be bounded and unique. **Initializers see the
parent scope; bodies see the new binding**. Active names cannot be shadowed;
initializer temporaries and sibling branches release their frames before another
binding is entered, so quotas count active frames rather than total declarations.
Bindings inside adapter-owned source arguments receive the same cold lexical
preflight, without implementing or evaluating the surrounding native form.

Bound references always become `Local` nodes without invoking the native callback.
The extension receives the original AST and a `ScalarSourceScope` exposing only
read-only borrowed type queries/counts; its Debug shows counts, not names or types.
**Unbound references remain explicit native aliases**. An initializer reference
with its own future binding's name is not a local yet: adapters without such an
ambient alias must reject it. There is no implicit self-reference or forward grant.
The legacy `lower_scalar_source` entry still delegates bind/locals to its native
callback, avoiding a silent semantic change for existing adapters.

Both the bound value and body must have scalar observations, and all native IR
must pass bounded structural purity checks. Observations are not type certificates:
**complete cold IR inference against the exact runtime prefix remains mandatory**,
including native fields, generated locals and cold branches. The adapter supplies
corresponding runtime values and bounds native expansion/work; type visibility
does not grant capabilities, dispatch, receipts or suspension ownership.

The reference compiler shares binding syntax/name rules and the original Bind
constructor while retaining atomic, group and helper-result capture checks,
diagnostic spans/order and source accounting. Scope guards release temporary
frames on errors/unwind; partial move-only native IR drops once, without rollback
or retry. Product-free proofs cover parsed bindings through cold inference and
actual execution, native AST/scope observations, prefix preservation, cold refusal,
aggregate budgets, cleanup and original catalog call-argument preparation.
This is **scalar lexical source construction, not general host-result capture or
complete helper/field source lowering**.

### Shared Native Result Binding Source

`binding_source::lower_binding_source` sequences one source Bind through an
explicit `BindingSourceAdapter`: **Value, Body, then Finish, each at most once**.
It shares binding compilation orchestration, not a complete native child compiler,
static flow checker, received-result acceptance or execution authority.

**Whole cold source and lexical preflight precedes every native hook**. Physical
AST nodes/depth, call names/operand counts, text limits and all nested bind/loop/fold
shapes, literal limits and active name conflicts are checked against the exact
borrowed prefix. Names convey no type or runtime value. Initializers retain the
parent namespace, bodies acquire the new name, and sibling scopes never accumulate
bindings. Unknown native/helper-local namespaces remain explicit adapter policy.
Inclusive ceilings are 16,384 source/output nodes, depth 64, 64 call operands and
1,024 active names. Zero node/binding quotas deny admission; output depth zero
cannot contain a Bind's operands and fails before lowering.

Hooks receive original AST references, source spans, owned child IR and the same
native metadata. Body may move metadata into its **guarded native type frame**;
the core never installs runtime locals or copies schemas/results. There is no
native Clone/Debug/serde/Send bound. Finish explicitly corroborates all cold types,
exact schemas/versions/grants, group exports, opaque graphs and capture/flow rules,
and may perform normal-return composition. No accept-all defaults are provided.

**One aggregate physical output meter precedes Finish**. Value IR starts at depth
one, after charging the Bind root and reserving a future body root; Body IR joins
that same meter at depth one. Final rewritten output is checked afresh. This is
not folded/native source-cost measurement or a refund of expansion reservations.
Produced IR language/lexical/type checks remain native adapter responsibility.
Native work, allocation, interior mutation, Drop and unwind are trusted, not fuel
limited or sandboxed. Errors retain original phase/payload for matching, never
formatting or source chains. Failure/unwind releases consumed parts once with no
partial result, retry or rollback; adapters guard their own frames even on failure.

The reference compiler now delegates pure, atomic-result, group-result and helper
bindings through this entry. Its original result classification, local type frame,
group continuation quotas, helper return rewrites, canonical/wire and authority
gates remain intact. Two distinct native formats prove original buffers/declarations
and cleanup. A parsed counter host additionally compiles source/catalog bindings,
checks actual received scalar values, prepares the next call with exact schema
identity and fuel, and rejects stale versions/missing grants. Runtime reentry in
that test is explicitly host-owned, not full suspension or durable acceptance.
Full generic native child compilation, opaque flow typing, helper registry lifecycle
and complete suspension/reply ownership remain independent extraction gates.

### Shared Native Choice Source

`choice_source::lower_choice_source` constructs one native-typed source Choose
with explicit child lowering, boolean corroboration and type comparison hooks.
It requires no native PartialEq/Clone/Debug/serde/Send implementation and never
replaces native observations with a serialized tag or a product-specific enum.

**Whole cold source and nested choice signatures precede every hook**. Source
nodes/depth, bounded text, call names/operand counts and every nested Choose's
exact `when`, `then`, `otherwise` signature are checked before lowering. Other
native/helper signatures and complete lexical/type policy stay adapter-owned.
Inclusive ceilings are 16,384 source/output nodes, depth 64 and 64 call operands.
A Choose requires at least four output nodes and depth one before any hook.

**When, boolean check, Then, Otherwise, comparison** is the once-only semantic
order, regardless of submitted argument order or literal truth. Both cold branches
are compiled; source construction does not evaluate a condition or select effects.
One physical output meter charges the new root and each original child at depth
one, reserving future child roots before continuing. These costs are not folded
source weights, native work, expansion refunds or execution fuel.

**Complete bounded pure When preflight precedes boolean corroboration**. Even
zero-limit loops/folds cannot hide effects. A literal condition independently
requires boolean kind; the fallible native query must corroborate full lexical
and boolean typing against the original IR and its borrowed observation. Structural
purity alone does not prove typing. Branch IR passes physical bounds, not generic
lexical, schema, opaque-host graph or complete type acceptance.

**Both bounded branches precede the explicit fallible type comparator**. It
enforces the host's same declared type/interface policy without unions, casts or
new exports. Matching result interfaces do not imply identical operation rows,
capabilities or reply domains. Original schemas, versions/grants, closed group
exports, actual received values and live execution authority remain separate gates.
The exact Then observation moves out; When/Otherwise observations are released.
All original child Boxes, vector buffers and native IR slots move unchanged.

Native errors retain their phase, original span and payload for matching, not
formatting or private source chains. Failure/unwind stops later hooks, releases
consumed parts once and produces no partial result, retry or native rollback.
Adapters guard their own scopes, bound ingress/native work/Drop and revalidate
policy before use; this API does not sandbox or fuel-limit callbacks.

The reference compiler delegates general choices here while retaining diagnostics,
canonical/wire checks and authority. An unrelated parsed counter host compiles both
original schemas, then prepares only the selected call with exact arguments/fuel,
checks replies and rejects stale versions or missing cold-branch grants before
execution. This is **native choice construction, not a complete independent source
compiler, helper registry or suspension lifecycle**. Full generic native child
compilation, opaque flow typing and complete durable reply ownership remain open.

### Shared Flat Native Group Source

`group_source::lower_flat_group_source` constructs one flat `seq` or `all` on
the original generic IR. `GroupSourceLimits` explicitly separates source/output
bounds and member quotas. Sequence needs 1 through 64 named members, parallel
needs 2 through 64; names are bounded, unique identifiers, including `true`/`false`
labels. Zero node/member quotas deny entry, and output depth zero cannot contain
members. Source/output ceilings are 16,384 nodes, depth 64 and 64 operands.

**All root names and cold source headers precede member lowering**. Complete AST
physical/text/name/operand preflight and every nested Seq/All header's arity/name
shape are checked before native child callbacks. Direct nested seq/all/repeat
members are refused by this flat entry, not silently expanded or copied. Native
arguments, lexical frames, helper signatures and source expansion remain explicit
child-compiler policy. No native PartialEq/Clone/Debug/serde/Send bound is imposed.

**Lower all members, then admit all members, in declaration order**. Lower receives
each original borrowed `NamedArgument` once, including its submitted span and name,
and returns owned child IR plus the original result observation. One aggregate
physical output meter charges the root and shifted children, reserving future
member roots before continuing. Every output is bounded and structurally checked
as an atomic candidate before any admission observer runs. Host/Call tails and
Bind/Choose with pure preparation are candidates; pure values, nested Group tails
and effects hidden in zero controls are not.

**Atomic candidacy is not a type or schema certificate**. The mandatory fallible
admission observer sees exact source/IR/result pairs and must corroborate complete
lexical/boolean typing, generated language data, uniform operation identity on
every cold member route, original result declarations and live versions/grants.
Opaque Host graphs and native costs remain host-owned. Equal result tags cannot
replace operation identity or union exports. No group result type, prior sequence
receipt or implicit runtime binding is inferred or installed by construction.

Language names are materialized once; original native slots, nested Boxes, result
observations and child vector buffers move unchanged into the Group. Failure/unwind
stops later hooks, releases consumed parts once and produces no partial group,
retry, rollback, source-weight refund or fuel grant. Errors preserve native phase,
index/span and payload for matching, never formatting or private source chains.
Native work, mutation, allocation, Drop and unwind are trusted, not sandboxed;
adapters guard their own frames and bound ingress before allocating AST/native IR.

The reference compiler now delegates flat groups here, directly retaining computed
members without the intermediate effect-tree round trip. Fully opaque groups keep
their existing wire representation. Legacy nested flattening and generated labels
remain unchanged outer paths. Flat repeat now uses the next shared entry; nested
repeat expansion remains adapter policy, not generic native cloning support.
The shared path is fail-fast; cold header errors precede child compilation and
diagnostics retain group kind/span without indexing a nested error into root members.
After a native child-lowering failure in `all`, the reference adapter retains its
bounded, non-executing sibling diagnostic pass without retrying earlier members or
returning a partial group. Sequence lowering and shared native hooks remain fail-fast.
Canonical output, cold capability checks and full native authority remain intact.

An unrelated parsed counter host proves original catalog row/result ownership,
actual group argument preparation, exact fuel and reply-domain checks, rejecting
stale versions, missing later grants and forged declarations before preparation.
Both modes return native request data, not dispatched effects or a parallel scheduler.
This is **flat native group source assembly, not complete child source compilation,
repeat factories, scheduling, received group acceptance or suspension ownership**.

### Shared Flat Native Repeat Source

`repeat_source::lower_flat_repeat_source` constructs a bounded flat Sequence from
`repeat(times: 1..=64, body: ...)`. `RepeatSourceLimits` separates physical AST/IR
bounds, folded/native source-expanded bounds and a repetition quota. Inclusive
ceilings are 16,384 nodes and depth 64. Count is a positive integer literal, never
a dynamic value or coercion. Zero capacity/count or output depth zero denies entry.
Direct seq/all/repeat bodies are refused; nested flattening remains outer policy.

**Whole cold source and repeat headers precede one body lowering**. Exact times/body
names, literal counts, all physical/name/text/operand bounds and every cold repeat
header are checked before `RepeatSourceAdapter::lower_body`. Minimum Group/member
capacity also precedes that callback. The original borrowed body NamedArgument,
including its submitted span, reaches the adapter unchanged.

**Complete shifted physical and source reservations precede every factory**. A
bounded physical body walk and `source_cost::measure_source_cost` establish separate
costs. The complete repeated output reserves `1 + body_nodes * count` and
`body_depth + 1`, including the Group root, with checked arithmetic. Folded optional
none/list constructors retain their weights. Opaque Host extras are explicit
`host_cost` observations relative to that Host root; zero cost is policy, not graph
identity. Native graphs, payloads, work and observation accuracy remain host-owned.
Physical/source costs are not runtime fuel or a source/type/authority certificate.

**The original body becomes iteration_1; only later instances call a native factory**.
`materialize` visits 2 through count once, in order, borrowing the exact first owned
IR/result pair. Count one calls no factory. The core never clones a native slot,
Box or vector and imposes no native Clone/PartialEq/Debug/serde/Send bound. Factories
must faithfully reproduce semantics and scopes; every returned instance must match
the exact original physical node count/depth and measured folded/native source cost.
Shrinking, widening or changing only depth/cost fails without spending the reservation.

**All generated atomic candidates precede native admission**. Once all instances
fit, `admit` visits every iteration in order with the original and candidate IR/result
pairs; iteration one supplies the same branch twice. Each candidate must be a Host,
pure-argument Call or pure-prepared Bind/Choose atomic tail. Matching shape, cost or
result tags cannot prove faithful literal/native identity, exact operation/result
declarations, complete cold typing, closed native graphs or live versions/grants.
Those remain mandatory native admission policy. No prior receipts or runtime/type
frames are implicitly installed; generated iteration names do not grant authority.

Failure/unwind stops later hooks and drops consumed instances once without partial
output, retry, native rollback or fuel/source reservation refund. Errors retain
one-based iteration, native phase/body span or cost-observer position and original
payload for matching, never formatting or private source chains. A pre-body output
error uses iteration zero. Native factory/cost/admission/Drop work is trusted, not
sandboxed or preempted; adapters guard their own frames and bound ingress first.

The reference computed flat-repeat path delegates here, moves the first instance
and explicitly clones later reference IR in its native adapter, without an Effect
round trip. Labels, canonical wire, result types, authority and existing expansion
limits remain covered; direct nested Seq/Repeat bodies use the separate owned
repeat-sequence entry below.
An unrelated counter host prepares original-schema values with exact fuel and
reply-domain checks, rejecting changed same-sized values/declarations and stale
live policy before returning a request. This is **bounded repeat source assembly,
not a loop executor, dispatcher, complete child compiler or suspension lifecycle**.

### Shared Owned Sequential Source

`sequence_source::lower_sequence_source` composes `seq` directly on the generic IR.
The child compiler returns `SequenceSourceMember::Atomic` for an atomic source or
`SequenceSourceMember::Sequence` for a direct seq/repeat child. It explicitly owns
recursion and repeat factories; the core never guesses a group result declaration
or silently serializes parallel work. Child sequences must already be flat.

**Cold headers and the whole source precede child lowering**. All Seq/All headers,
names, literals and physical AST bounds are checked first, including unused cold
operands. Direct All members are rejected before any native hook. Repeat headers,
lexical/type scopes and helper expansion remain child-compiler policy. Inclusive
ceilings are 16,384 nodes, depth 64, 64 operands and expanded branches. Zero
capacity or zero output depth denies members.

**Expanded names and one shifted output budget precede every admission**. Original
NamedArguments are lowered once in declaration order. Native result declarations,
all four IR slots, Boxes and operand buffers move without native Clone/PartialEq/
Debug/serde/Send bounds or an opaque-effect round trip. Consumed sequence vectors
are released, not retained as another tree. Language labels join as `parent__child`;
their final 64-byte bound is checked before allocation. Empty/overwide children,
generated-name collisions, malformed atomic candidates and total physical output
limits fail before Admit. Future source member roots retain minimum capacity.

Admission visits all final branches once, in flattened order, with their original
source/member indices, borrowed NamedArguments and original result declarations.
Exact operations, complete typing, generated values, closed opaque graphs, costs,
versions and grants remain mandatory native policy, not inferred from tags or
candidate shape. Previous receipts or runtime locals are never installed implicitly.
Native callbacks must not dispatch and are trusted, not sandboxed or preempted.
Failure/unwind drops consumed parts once and stops later hooks without partial
output, retry, rollback or fuel/source refund. Native errors retain phase, indices,
span and matchable payload, but never format secrets or expose private source chains.

The reference computed nested-Seq path delegates here, moving native computed
children directly and opening legacy opaque Sequence wrappers only at its adapter.
Opaque-only wire shape, generated names, canonical bytes, cold authority and existing
limits stay compatible. Nested-repeat composition uses the shared entry below;
parallel nesting policy remains adapter-owned. An unrelated parsed counter host proves two distinct native result
domains, preserved row identity, actual values and exact fuel, with stale versions,
missing later grants and forged rows rejected before preparation. This is **owned
sequential source composition, not complete child compilation, effect dispatch,
received-result acceptance or suspension ownership**.

### Shared Owned Repeat-Sequence Source

`repeat_sequence_source::lower_repeat_sequence_source` composes a repeat whose
direct body is Seq/Repeat. Its child compiler supplies a nonempty, already-flat
owned member vector; no synthetic group result declaration is inferred. Direct
All is refused, not serialized. Recursion remains explicit child-compiler policy.
`RepeatSequenceSourceLimits` separates source/physical bounds, folded/native source
costs, repetition count and total expanded branch count. Inclusive ceilings are
16,384 nodes, depth 64 and 64 repetitions/expanded branches; zero capacity denies entry.

**All cold repeat and group headers precede one sequence-template lowering**.
Exact times/body names, positive literal counts, every cold Seq/All/Repeat header,
whole AST/name/text/operand bounds and minimum output capacity are checked before
the adapter. The original borrowed body NamedArgument and spans are preserved.

**Reserve the complete flattened member forest before every factory**. One final
Group root is charged plus count times the template forest, for both physical and
folded/native source costs. Removed intermediate Group roots are not repeatedly
charged. Whole shifted depth and total expanded width must fit. Template labels
must be unique bounded identifiers, and the longest final
`iteration_N__child` label fits 64 bytes before allocation or copying.

**The first template moves; later factories borrow its exact unprefixed rows**.
`materialize_sequence` runs once for 2 through count, never receiving a previous
copy or prefixed intermediate tree. Every instance preserves width, ordered labels,
aggregate physical node count/depth and measured source cost. These are aggregate
bounds, not per-row semantic/schema certificates. All instances and atomic candidates
precede `admit_member`, which visits iteration order then declaration order with
the exact original and candidate row; iteration one supplies the same row twice.
Native operations, fields, opaque effects, result declarations, Boxes and operand
buffers need no core Clone/PartialEq/Debug/serde/Send bound. Branch vectors are
consumed into one bounded result vector; only generated language labels are joined.

Admission must corroborate faithful values/native identity, complete typing, original
declarations, closed opaque graphs, cost accuracy and live versions/grants. No prior
receipts, type/runtime frames, dispatch or scheduling are installed implicitly.
Failure/unwind stops later hooks and drops consumed parts once without partial
output, retry, rollback or source/fuel refund. Native payloads remain matchable by
iteration/member/phase/span but never formatted or exposed through private source
chains. Callback work/allocation/Drop/unwind is trusted, not sandboxed or preempted;
bound ingress first, guard native scopes and revalidate live policy before use.

The reference computed nested-repeat path delegates here and explicitly materializes
later reference rows in its adapter. Computed Seq/Repeat no longer round-trip through
the product Effect tree; opaque-only final wire wrappers remain compatible. Tests
cover names, source costs, canonical bytes, diagnostics and cold authority. An
unrelated parsed counter host preserves two distinct result domains and exact native
row identity, prepares real repeated values with exact execution fuel, rejects
same-shape/cost argument changes during admission, and rejects stale versions or
missing later grants before request preparation. This is **owned repeat-sequence
assembly, not complete child compilation, dispatch, reply acceptance or suspension
ownership**.

### Shared Scalar Source Loops

The scoped entry also constructs **bounded pure scalar loops** directly on the
original IR. Source spelling remains `loop(state: initial, while: condition,
next: expression, limit: literal)`, with one state label and exactly the three
reserved operands. Arguments may be submitted in any order; construction always
lowers **initial, while, then next**, using the original AST nodes. Initial state
sees the parent scope; while/next see one guarded scalar state binding, counted
against the same active binding policy. Inner initializer frames and sibling
condition/next frames cannot leak names or shadow active state/prefix names.

The limit must be an **integer literal from 0 through 1024**. Computed, reference,
missing and oversized limits are not implicitly folded, truncated or expanded.
All cold loop shapes, state labels/shadows/quotas and literal limits are checked
before any native extension, including loops inside adapter-owned source forms.
Initial state and next must be pure scalars of the same type; while must be a pure
boolean. Condition typing precedes next lowering. **Zero iterations still compile
and type-check both bodies**; an observed native type is not a substitute for
complete cold IR inference against the adapter's exact type/value environment.

Construction never unrolls/evaluates a loop or grants fuel. **Limit literals are
source metadata, not generated IR nodes**: source preflight includes the limit AST,
while generation charges the Loop root and its initial/condition/next trees once.
The same four-node leaf loop fits the same generated budget at limits 0 and 1024;
all cold child expansion and pre-fold work still count. Native callback work,
allocation and Drop remain adapter-owned and cannot be preempted by these bounds.

Actual execution retains the shared condition-first `LoopBudget` semantics: a
false condition may finish exactly at the limit, while a true condition at the
limit fails before another next expression. It returns final state, not a silently
truncated result. **Loop exhaustion, fuel exhaustion and native failures are not
calculation fallback success**; scalar arithmetic/parse faults remain recoverable
under explicit `recover`. Runtime prefix values survive failures/unwind without
fuel refunds, retries or duplicate native work.

The reference compiler delegates source shape, literal-bound, pure condition/state
checks and the original Loop constructor, preserving diagnostics/spans/order,
source accounting, canonical wire bytes and result-bound host authority. The
legacy unscoped source entry still delegates loop forms to its callback. Native
proofs cover all six scalar state types, borrowed scope/AST identity, zero/max
limits, no unrolling, cleanup, cold typing and original schema call preparation.
This is **pure loop source construction, not a complete helper/field/host-result
compiler, effect suspension engine or durable independent VM**.

### Shared Scalar Source Folds

The scoped entry constructs **bounded pure scalar folds** on the original IR:
`fold(state: initial, items: collection, item: "entry", next: expression, limit: literal)`.
Exactly one state argument and the four reserved operands are required. The item
name is a borrowed literal string, bounded, distinct from the state name and
unable to shadow an active prefix/local. Both locals count against the explicit
active quota. All cold shapes, labels, shadows, quotas and literal limits are
checked before any native source extension, including adapter-owned forms.

Construction follows **items, initial, then next**, regardless of submitted order.
Items and initial see the parent scope; only next sees both guarded locals, with
a string item and the initial state's scalar type. Nested preparation frames and
sibling branches cannot leak bindings. Items must be a pure string list; initial
and next must be pure scalars of the same type. **Empty and zero-limit folds still
compile and type-check next**. Native type observations are not certificates:
complete cold inference against the exact native type/value prefix is mandatory.

The limit is an **integer literal from 0 through 64**. Item-name and limit literals
count in physical source preflight but are metadata, not generated IR nodes.
Generation charges the Fold root and its three child trees once, with no
unrolling, speculative list-buffer copy or refund of pre-fold constructor work.
Native hidden expansion, allocation, queries and destructors remain adapter-owned.

Construction does not inspect actual native collection lengths or create an
execution cursor. Actual execution evaluates collection then initial state,
checks the complete collection count against the limit, then evaluates next in
source item order. **Collection exhaustion never returns a truncated result**.
Collection/fuel exhaustion and native failures are not calculation fallback
success; explicit scalar arithmetic/parse recovery retains the closed fault set.
Collection-copy, scan and iteration fuel remain the shared executor's exact
budget, with no refund, retry, rollback or additional native dispatch authority.

The reference compiler delegates the same shape, scope, literal-limit, pure type
gates and original Fold constructor. Diagnostics, spans, error order, source
accounting, canonical wire bytes and result-bound authority remain unchanged.
The legacy unscoped entry still delegates folds to its native callback. Failed
lowering drops partial move-only native operands once; native unwind restores
the exact runtime prefix without leaking either fold local or refunding fuel.
Product-free proofs cover all six accumulator types through cold inference,
execution and original native call-argument preparation. This is **pure fold
source construction, not a complete helper/field/host-result compiler or an
independent suspension and durable-restart proof**.

### Shared Native Projection Source

`projection_source::lower_projection_source` constructs **native field/member
projections** on the same generic IR. A `ProjectionSourceEnvironment` explicitly
lowers the original field value AST, resolves a field against its observed native
result, or resolves a member against the exact bound group and literal step name.
The language owns `value`/`name` signatures and constructors, not a product field
vocabulary or automatic GUI adapter. Names borrow the original source; native
field buffers and operation slots move into IR without cloning or conversion.
**Native result observations need no Clone/Debug/serde/Send bound**. Cold inference
may separately use cloneable borrowed identifiers rather than copying payloads.

Physical source and all cold projection signatures/literal metadata inside the
entry's input are checked before every native callback, including projections
inside adapter-owned cold forms. Minimum generated node/depth capacity is checked
before lowering. The complete produced field input is then checked for physical
purity, names, literal bounds, aggregate nodes and depth **before field export
queries**. A scalar input observation cannot become a native result implicitly.
Members require bounded group/step names using the same pure-IR name policy;
they never lower a group expression or synthesize a Local child.

Source name literals count in physical AST bounds but not generated IR. A field
over a leaf result costs two IR nodes; a member costs one. A field over a member
preserves both original native slots and uses two nodes, without a second tree.
The aggregate generated budget includes every physical native input node. Native
hidden expansion/pre-fold work, allocation, metadata equality and destructors
remain separately bounded by the adapter; source limits cannot preempt that work.

These are **construction/type observations, not result acceptance or effect
authority**. Complete cold inference against the exact type/value prefix remains
mandatory, checking original member operation identity and exact field exports.
Unknown locals, wrong groups and forged type observations do not become valid
programs merely because construction succeeded. Execution retains member-before-
field queries, existing copy/scan fuel and native-failure provenance; native
failures cannot become calculation fallback success or alter prefix values.

The reference compiler delegates shared signatures, literal metadata and original
constructors while retaining its closed field table and group export policy.
**Reference field child-error precedence remains unchanged**: it still lowers
value before reporting a nonliteral name, unlike the public entry's all-cold
preflight. Diagnostics/spans, canonical wire bytes and capability requirements
remain compatible. The obsolete allocating named-operand helper is removed.
Native errors format without payloads; failure/unwind releases partial move-only
input nodes once, without retries, rollback, receipts or effect dispatch.

Two distinct native field schemas cover all six scalar types through parsed
source, cold inference and actual execution. Proofs also compose with shared
scalar construction and original native call-argument preparation. This is
**shared projection source, not a complete helper/result-capture compiler,
source-to-effect suspension pipeline or durable independent VM**.

### Shared Helper Body Hygiene

`helper_hygiene::hygienic_helper_body` isolates an **owned generic helper body**
on the original IR. The caller supplies borrowed original/fresh parameter aliases,
explicit caller-reserved names and `HelperHygieneLimits`; the callback allocates
new binding labels, not native effects or runtime values. No default policy exists.
Inclusive fixed ceilings are 16384 physical nodes, depth 64, 1024 active bindings
including parameters and 16384 submitted reserved names. Zero nodes denies a body,
zero depth allows leaves, zero bindings forbids parameters/locals, and zero reserved
capacity forbids caller reservations. Duplicate reservations are idempotent but
still count as submitted inputs.

**Whole physical and cold lexical preflight precedes every fresh-name callback**.
All original labels must be bounded and valid; free locals/groups, active shadowing,
invalid loop/fold names and active quota overflow fail even on unselected choices,
recovery fallbacks, empty folds and zero iterations. Bind initializers and loop
initial states see the parent scope; fold items/initial see the parent, while next
sees both state and item. Temporary bindings cannot escape into sibling operands,
call arguments, string items or group branches. A member's group label is lexical;
its member label and native operation identity are not renamed.

A **stable owned source-name pool** lends original keys to the shared `ScopeFrame`
while owned IR labels change. It copies each distinct original lexical name once,
not a second IR tree, native payload or caller value prefix. Cold unused fold item
names are reserved too. Aliases must be disjoint from original/reserved/other alias
names; fresh callback results must also avoid all previously generated names.
Invalid names or collisions fail once, without retry. Global namespace ownership
and any broader native reservation policy remain explicit adapter responsibilities.

**Native slots need no Clone/Debug/serde/Send bound**. Original Boxes, vectors and
native buffers remain in place; language-owned binding/reference strings alone
change. Opaque Host payloads must be closed with respect to language locals: their
hidden graphs, work, allocation and destructors cannot be inspected or preempted by
the language walk. **Failure/unwind drops the consumed body**, returns no partial
tree, and does not roll back or refund native name reservations. Native errors
format without exposing their payloads; destructors remain trusted host code.

**Hygiene is not a type certificate or a complete helper compiler**. Declarations,
recursion checks, argument source lowering, scalar parameter/result typing,
template ownership/factory policy and expanded source budgets remain separate.
Complete cold inference against the exact native type/value prefix is mandatory,
even after successful rewriting. Runtime fuel, received-result acceptance,
capabilities, effect dispatch and suspension/durable ownership are not created by
this operation. Parameter argument expressions stay in caller scope and are not
rewritten with the body; the caller owns their once-only wrappers and order.

The reference compiler delegates body isolation and scalar signature/argument
preparation while retaining complete typing, pre-clone expansion bounds and its global `_lf`
allocator. **Cached reference template cloning remains adapter policy**, not a
generic native-slot requirement. Allocation order, diagnostics, canonical wire
bytes and result-bound host authority remain compatible. Product-free proofs use
parsed scalar bindings/loops/folds and all six scalar parameter types through cold
inference and execution, preserving exact fuel. Move-only native field/operation/
effect/result slots preserve buffer identities and release once on failure/unwind.
This is helper-body isolation, not the complete source-to-effect embedding gate.

### Shared Helper Signatures And Arguments

`helper_source::helper_parameters` borrows original declaration names and parses
the **six exact scalar parameter tokens** in source order. Names are bounded and
unique; no case folding, native result parameters, aliases, inferred types or
coercion are introduced. The explicit inclusive ceiling is eight parameters;
zero permits an empty signature. Function names/bodies, entry/builtin policy,
declaration cycles, return typing and template ownership are separate checks.

`helper_source::lower_helper_arguments` uses an explicit borrowed `HelperSignature`
and `HelperSourceLimits`. The selected name must match the source callee exactly;
the source does not register or select an arbitrary helper catalog. **Whole cold
physical source precedes every lowering callback**, including nested native-owned
forms. Every signature parameter must be required, bounded and unique; duplicate,
unknown, extra or missing submitted keys fail before any argument is lowered.
**Arguments lower once in declaration order**, retaining each original submitted
index, exact borrowed parameter and original AST argument. They see only the
caller-owned scope: helper parameter bindings are not installed here. An explicit
language `none` value is different from an omitted argument or non-scalar result.

Source and output have **separate aggregate physical budgets**. Source includes the
call root and all cold operands; output is an argument forest, each root at depth
zero, with no phantom call root, helper body or Bind wrapper. Before a callback,
remaining minimum argument roots must fit. Each produced native tree is checked
for physical purity, bounded literals, language names and node/depth/control limits.
The six scalar observations and literal facts are checked exactly through the
shared argument-type rules. Nested source calls retain their separate source-arity
limit, not the eight-parameter helper ceiling. Native hidden expansion/pre-fold
work, wrapper/body expansion and complete cold lexical type inference remain
separately bounded by the caller. Physical bounds do not prove source/span authenticity.

**Dynamic observations are not helper argument type certificates**. Complete cold
inference against the exact native type/value prefix remains mandatory, including
all cold branches, local bindings and result exports. No value is evaluated or
runtime fuel granted by this constructor. Owned native slots need no Clone/Debug/
serde/Send implementation; original Boxes and native buffers move unchanged.
Failure/unwind releases partial operands once, without retries or native reservation
rollback. Native errors retain payloads only for explicit matching, never formatting
or error source chains; native allocation, callbacks and Drop remain trusted code.

The reference compiler delegates scalar token/name checks, named alignment, argument
construction preflight and exact pure scalar facts. Its private preflighted entry
preserves existing child diagnostic precedence/spans after reference source bounds.
Global source expansion, cached template cloning, parameter wrappers, body hygiene,
canonical wire policy and result-bound authority remain unchanged. Product-free
proofs connect parsed arguments to helper-body hygiene, complete inference and
actual execution under identical fuel, including native field/member buffers.
This is **shared helper argument preparation, not declaration graph compilation,
template expansion or a complete independent source-to-effect pipeline**.

### Shared Helper Declaration Admission

`helper_declarations::accept_helper_declarations` admits the supplied declaration
forest with an explicit entry name, `HelperDeclarationLimits` and fallible native
reserved-name policy. The entry need not be `main`; policy is not inferred from
runtime or UI operations. Exact bounded names, duplicate headers and the six
closed scalar parameter tokens are checked without case folding or normalization.
The entry is mandatory and parameterless. Inclusive ceilings are 32 functions,
eight parameters, 16,384 physical nodes per body and depth 64. Zero functions on
empty input reports a missing entry; zero nodes denies a body; zero depth permits
leaves. Physical body quotas are separate, not aggregate expansion or runtime fuel.

**Declaration checks retain supplied-order error priority**: ceilings/count/entry
configuration first, then each header's name, once-only policy, duplicate check,
signature and complete physical body. Missing/parameterized entry checks follow
the entire forest. Policy receives only a bounded header name, not the body; it
may run before a later body fails and must not perform execution or grant authority.
Policy failure/unwind produces no partial observation, automatic retry or rollback
of native side effects. Error formatting/source chains never expose policy payloads.

**Pending-frontier bounds precede reservation growth**. Every physical child is
visited, including unused helpers, cold choices, recovery and zero loops/folds.
Reservations borrow parameter names, argument labels, references and the first
quoted fold item. Function/callee names and ordinary string literals are not
variable reservations. This preserves unused fold bindings and caller-prefix
name collisions without cloning source names or literal buffers.

`HelperDeclarations` borrows the exact entry, helper ASTs and reservation strings;
helpers retain supplied order, not dependency order. A temporary input slice may
be dropped while the underlying declarations remain borrowed. Accessors are
read-only; Debug reveals counts only. The reference adapter retains its `main`
entry and builtin policy, explicitly copies reservations into its own fresh-name
set, then composes shared dependency planning and template/hygiene/wrapper APIs.
Header/body diagnostics, spans, canonical wire and cold capability gates remain
unchanged. Move-only host slots work through the composed pure interpreter proof
with identical values, fuel and scope cleanup.

This is **borrowed header/physical admission, not complete helper lowering or
execution authority**. Argument names/arity, text bytes, source spans, cold lexical
types, native schemas, return typing, cycles and entry calls remain separately
checked. Registry ownership, source-expanded weights and caller-prefix budgets
are not installed here. Complete helper body/return lowering and registry lifecycle
remain outer responsibilities; acceptance is not a cached semantic certificate.

### Shared Helper Dependency Planning

`helper_dependencies::helper_dependency_order` plans exact borrowed helper
declarations against an explicit entry name. The entry need not be `main`; its
declaration and body are not supplied or validated here. All helper names must be
bounded local identifiers, unique and disjoint from the entry. Errors preserve
original submitted declaration indices and source spans, not sorted positions.
Builtin/native reserved-name policy remains an explicit adapter check.

`HelperDependencyLimits` supplies inclusive helper count and per-helper physical
source bounds, with fixed ceilings of **31 helpers, 16,384 nodes and depth 64**.
Zero helpers permits an empty graph, zero nodes rejects a nonempty body, and zero
depth permits leaves. These are not ingress-byte limits, aggregate program or
template expansion budgets, literal-byte checks, span authenticity or execution
fuel. **All physical helper trees precede dependency scanning**, including cold
and unused declarations. Iterative pending-frontier checks precede queue growth.

Only exact `Expression::Call` callee matches create helper edges. References,
argument labels and string literals do not. Calls inside native-owned forms,
choices, recovery and zero loops/folds still count; there is no reachability
pruning. Repeated edges share one bounded bit, but every physical source node
still consumes its source budget. Unknown calls are not edges and are **not valid
operations** merely because planning succeeds. Every entry call is rejected
before iteration, in lexical helper order and right-to-left source-child order,
preserving the reference diagnostic priority and exact original call span.

The move-only `HelperDependencyOrder` borrows original names and ASTs, not the
temporary input reference list. It uses a bounded bitset, without name/body
clones, recursive graph traversal, a second syntax tree, native callbacks,
evaluation, bytecode, global state or template caching. Each iteration selects
the **lexically smallest ready helper**, including newly ready names before
unrelated siblings; it does not yield whole topological layers in bulk.
Successful yields mark declarations **planned, not compiled or dispatched**.

A cycle is reported once only after all ready declarations have been yielded;
the remaining count includes dependent declarations blocked by the cycle.
Iteration is fused afterward. **Discard the cursor on lowering failure** rather
than skipping a failed helper. Early abandonment does not certify an acyclic
graph. The reference lowerer fully prepares each yielded body before asking for
the next helper, preserving ready-helper type errors before a later cycle error.
Debug/error formatting exposes fixed tags/counts/spans, never source payloads.

Reference entry/builtin/signature policy, lexical name reservations, type checks,
template cloning, parameter wrappers and aggregate expansion accounting remain
unchanged, as do canonical wire bytes and capability authority. The allocating
name-set dependency graph is replaced, not the interpreter. Here frontend
lowering means source preparation into typed IR, **not an IR-to-bytecode compiler**.
Product-free proofs cover the dense 31-helper graph, reordered declarations,
cold/unused cycles, source frontiers, original borrows/spans, diagnostic priority
and composed signature, hygiene, cold inference and interpretation with exact fuel.
This is **dependency planning, not complete declaration validation or template
expansion**. Native operand validation, host-result bindings, accepted replies,
effect dispatch, suspension and durable ownership remain separate boundaries.

### Shared Helper Parameter Wrappers

`helper_bindings::bind_helper_arguments` consumes an already hygienic native body
and owned `HelperBinding` operands, supplied in **declaration order**, with explicit
`HelperBindingLimits`. Fresh aliases are bounded unique local names, not names
generated by this constructor. No native Clone/Debug/serde/Send bound is required.
The fixed ceilings are **16,384 physical output nodes, depth 64 and eight
parameters**. Zero parameters permits an unwrapped body, zero nodes rejects any
body, and zero depth permits only an unwrapped leaf. There is no default policy.

**One aggregate meter includes wrappers, operands and body**. Wrapper i is at
depth i, its operand starts at i + 1, and the body starts at parameter count.
Minimum remaining roots are checked before visiting each operand; pending body
frontiers are bounded before growth. Every cold operand is physically pure and
has bounded literals, language names and loop/fold limits before scanning names.
All shifted physical output checks precede new Bind/Box construction. These bounds
do not replace source/pre-fold expansion accounting, active binding quotas, ingress
limits, evaluated value limits or execution fuel. Folded source cost is not refunded.

**Parameter aliases cannot capture caller operands**: every alias must avoid every
language lexical name in every operand, including group references and local
declarations. This is intentionally conservative even for cold or sibling-local
names. Body bindings, zero loops/folds and cold branches cannot shadow aliases.
References in operands are never renamed or substituted; body references to aliases
are intentional. The caller must separately reserve the entire live prefix and
global namespace, including names not used by operands. No prefix is read here.

Construction wraps in reverse order so interpretation evaluates each operand once
in declaration order, including unused parameters. Original operand locals, native
slots, child Boxes, vector buffers and owned alias strings move unchanged. Only
the new language-owned Bind nodes/Boxes are created, without a second IR, template
clone, name callback, native query, evaluation, dispatch, receipt or suspension.
Failure drops consumed inputs once; no partial output, retry, native reservation
rollback or source-cost refund is returned. Opaque Host bodies must be closed;
their graphs, ingress, work, allocations and Drop remain adapter-owned.

**Output shape is not complete cold lexical or scalar type acceptance**. Exact
named/scalar operand preparation, body hygiene and complete cold inference against
the exact caller prefix remain mandatory. Body literals/operator/group metadata
are not certified by this physical walk. Unbound body/operand locals, prefix
shadowing, incompatible cold branches and unbounded body literals still fail at
their separate acceptance gates. Native schemas, versions, grants and result
ownership remain explicit adapter responsibilities.

The reference lowerer delegates the final parameter wrappers after its existing
source-weighted node/depth reservation, template clone and body hygiene. Fresh-name
order, diagnostic spans, source accounting, canonical wire and authority remain
unchanged. Product-free proofs preserve move-only native buffers and verify exact
fuel, once-only unused operands, normal/failure/unwind prefix cleanup, and parsed
dependency/signature/argument/hygiene/wrapper composition into an unrelated numeric
native call with original schema identity. This is **owned parameter wrapper
construction, not a generic template cache or complete helper expansion pipeline**.
Template materialization and header admission compose through the shared APIs below;
complete helper body/return lowering and registry lifecycle remain outer policy.
Interpretation does not gain a bytecode compiler or implicit dispatch.

### Shared Owned Helper Templates

`helper_templates::HelperTemplate` owns the original ordered scalar parameter
names, body and native return-type observation. Read-only accessors borrow exact
stored data; `into_parts` consumes the entry and moves all parts back out. Body
Boxes, vectors, native buffers and parameter strings are not rebuilt. No native
Clone/Debug/serde/Send bound, global registry, default policy, lock or durable cache
is installed. Debug shows parameter count and physical shape, not source payloads.
Native slots and return observations may still contain interior mutability.

`HelperTemplate::new` uses explicit `HelperTemplateLimits`, with inclusive ceilings
of **16,384 physical nodes, depth 64, 1,024 active bindings and eight parameters**.
Zero nodes denies a body, zero depth permits leaves, and zero bindings forbids
parameters/locals. Parameter count includes unused entries. Names are bounded and
unique without normalization; types use the closed scalar enum. Whole physical
preflight, including bounded pending frontiers, precedes the cold lexical walk.
Free locals/groups, active shadowing and binding exhaustion fail even in unused
branches, recovery and zero loops/folds. Both walks reuse the helper hygiene core,
without native type/value queries or fresh-name allocation.

**Physical template shape is not source-expanded cost**. It counts each language
node once with a root at depth zero. Folded list/optional-string source weights,
opaque Host graphs, parameter wrappers, caller prefixes, global expansion quotas,
ingress and fuel remain separately bounded. Body literals, operator types, group
schemas, declared return typing, helper-name/builtin policy and complete program
declaration acceptance are not certified by storage. Opaque Host bodies must be
closed; native graphs, allocations, work, interior mutability and Drop remain
trusted adapter responsibilities.

`HelperTemplate::materialize` rechecks the exact cached body under explicit limits
for **each attempt before the native factory**. The factory receives the original
borrowed body and runs once through `FnOnce`; it owns faithful copying or native
slot remapping, without a required Clone implementation. The produced body receives
the same complete physical/cold lexical preflight before it can be returned.
**Node count and maximum depth must match exactly**; larger, smaller or differently
nested output cannot change the reserved physical shape. Matching shape is not
literal/operator/native/schema/semantic identity or a return-type certificate.
Complete cold typing, source-cost/canonical checks and live versions/grants/authority
remain mandatory. Native callback work is not fuel-limited or sandboxed here.

Factory failure or unwind returns no partial successful body and does not retry.
Produced output drops once on rejection; native errors are retained only for
explicit matching, not formatting or error-source chains. The cached entry remains
owned, but callback/Drop side effects and caller reservations are not rolled back.
A later explicit attempt needs fresh caller admission and expansion accounting;
reusability does not grant receipt, resume, replay or duplicate-effect authority.
Consuming an entry also returns ordinary mutable data, not a lasting certificate.

The reference adapter retains name-indexed cache selection, source-weighted
nodes/depth, name reservations and scalar parameter metadata copying. It reserves
expansion cost before calling shared materialization with its existing explicit
body-clone policy, then reuses body hygiene and parameter wrappers. Legacy fresh
order, error precedence/spans, canonical wire and capability authority are unchanged.
Product-free proofs cover all five move-only native slots, original buffers and
result identity, pre-factory rejection, factory failure/unwind, same-shape type
rejection, repeated exact value/fuel parity, and parsed dependency/signature/source/
template/hygiene/wrapper composition into an unrelated native call. This is **owned
template storage and materialization, not a global registry, durable cache or
complete independent helper source pipeline**. Complete helper body/return lowering,
registry lifecycle and native source-cost observations remain outer responsibilities.

### Shared Normal Return Composition

`leselang-hir::helper_returns::HelperReturns` owns one already-lowered helper
value, a caller continuation and an explicit return alias. It admits a bounded
language-only return route before an explicit continuation factory. This is
**normal-return composition, not complete helper source lowering or execution
authority**. The direct checked/lowered HIR remains interpreted; no bytecode,
AOT or JIT stage is added.

- **Complete cold inputs precede return-route analysis**: both input trees pass
  iterative physical/frontier preflight before bounded bottom-up purity analysis.
  Pure subtrees return as a whole, including pure Bind/Choose/Loop/Fold/Recover.
  Effectful Bind follows its body, Choose follows both cold arms, and
  Host/Call/Group return atomically. Other effectful return boundaries fail,
  including cold recovery and zero-iteration loops/folds. Original initializers,
  guards, call arguments and group members are not spliced internally.
- **Every cold continuation copy is reserved before its factory**: the complete
  output costs original physical value nodes plus one Bind and one continuation
  per normal return. Depth accounts for shifted return roots and unshifted
  initializer/guard subtrees. Inclusive ceilings are 16384 physical nodes and
  depth 64; zero nodes denies inputs and zero depth cannot fit a wrapper.
  Folded source costs, opaque native graphs, canonical bytes, active bindings and
  fuel remain separate policies. The reference adapter retains its 1024/16
  physical/source limits and canonical text checks before copying continuations.
- **Only return-route binders expose locals to continuations**: aliases cannot
  shadow these exposed names, and continuation locals/group references/binders
  cannot collide with them, including cold routes. Pure terminal, initializer
  and guard locals do not escape, so legitimate caller names remain usable.
  Continuations may reference their return alias, but cannot bind it. Neither
  input is renamed; full lexical/type validation against the exact caller prefix
  still belongs to the receiving adapter.
  All language local/group/binder names are bounded valid identifiers before
  route/name allocation or capture hashing, including factory copies. Native
  argument/export labels and metadata retain their separate adapter policy.
- `return_sites()` borrows original normal-return subtrees/depths in rightmost-
  child-first DFS order for legacy source-cost observation. `connect()` consumes
  the plan and calls its explicit factory once per return in left-to-right
  declaration order. **Each factory output must preserve exact physical node
  count and maximum depth**, pass fresh physical bounds, and avoid new capture.
  Original value native slots, initializer/guard Boxes and vector buffers move
  unchanged. Native slots need no Clone/Debug/serde/Send bound; the reference
  adapter alone chooses faithful `Clone` for its existing continuation type.
- **Native failures are never normal returns or recovery success**. Factory
  errors/unwind drop consumed originals and partial output once, stop later
  factories, and return no partial IR, retry or reservation refund. Native errors
  remain matchable, not exposed through formatting or source chains. Debug shows
  counts only, not aliases, literals or native payloads.

Equal physical shape does not prove identical literals, source weights, native
schemas, exports or grants. Opaque Host graphs must be closed and separately
bounded. Trusted factories own faithful copying/remapping, native work,
allocation, interior mutability, Drop and unwind; the shared route is not an
in-process sandbox. Final output still needs complete cold typing, source/canonical
and live authority checks before evaluation. There is no receipt, suspension,
registry, durable cache, scheduler, fuel grant or automatic native dispatch here.

The reference helper-return adapter delegates its actual splicing to this shared
route rather than retaining a second private composition algorithm. Its source
weights, canonical text size, final structural/type checks and wire/capability
rules remain adapter policy. Complete helper body/return source lowering and
registry lifecycle, generic host-result bind source, opaque flow typing and full
suspension ownership remain separate extraction work.

### Shared Lowered Helper Body Admission

`helper_body::LoweredHelperBody::prepare` admits an already-lowered owned helper
body, exact scalar signature, native return observation and optional scalar result
category under explicit `HelperBodyLimits`. It produces a read-only `HelperBody`
containing the original template and source-cost observation. This is **lowered
body admission, not complete helper source compilation or execution authority**.
The caller still owns AST lowering and registry lifecycle; no implicit compiler,
global cache, native dispatch, fuel, receipt or suspension frame is installed.

**Complete cold physical and lexical checks precede pure inference and callbacks**.
Shared template preflight validates parameter/name uniqueness, active shadowing,
all free locals and cold tree bounds against exactly the supplied scalar prefix.
Inclusive ceilings are 16,384 physical nodes, depth 64, 1,024 active bindings and
eight parameters; source costs use separate 16,384/64 ceilings. Zero policies are
explicit. Bound source/wire ingress before owned construction and decoding.

**Pure helpers require independently corroborated scalar returns**. The shared
type walker checks all six scalar domains, literal bounds, both choice/recovery
arms and zero-limit loop/fold next bodies, not only the selected runtime path.
Its environment contains only scalar parameter types: native result, group,
field/member export metadata cannot be invented from a reported return type.
The declared scalar observation must equal inference before native validation.
Effectful returns still require complete adapter-owned cold type/schema policy.

**Native validation runs once before cost measurement and registry insertion**.
It receives the original borrowed body, scalar signature, result metadata and
scalar observation. The adapter must corroborate metadata/category agreement,
canonical bytes, all cold effects/schemas and separately bounded closed opaque
graphs. Only then does shared source-cost measurement invoke explicit native
cost observers. All checks must succeed before `into_parts` hands the template
and cost to caller-owned storage. Original Boxes, vectors, names and all native
slots move unchanged without native Clone/Debug/serde/Send bounds.

Failure/unwind drops consumed inputs once with no partial template, retry or
downstream reservation refund. A later cost failure does not roll back earlier
native validation work. Errors retain native payloads for explicit matching, not
formatting or source chains; debug reveals counts/shape only. Trusted callbacks
own native work, allocation, interior mutation, Drop and unwind, not a sandbox.
Read-only language ownership and observed cost do not certify live native state;
fresh complete validation remains mandatory for materialization and dispatch.

The reference helper adapter now uses this shared admission path and explicitly
compares canonical validation's actual return type with the lowerer's reported
type before registering. Canonical/wire/capability gates and helper diagnostics
remain in that adapter. An unrelated scalar host composes source lowering,
admission, explicit materialization, hygiene, argument wrappers and normal-return
composition with unchanged result, exact interpreter fuel and lexical cleanup.
Full effectful helper source lowering, generic host-result bind source, registry
lifecycle, opaque flow typing and complete suspension ownership remain pending.

### Shared Source Cost Measurement

`source_cost::measure_source_cost` measures the original borrowed language IR with
explicit `SourceCostLimits`. Inclusive ceilings are 16,384 source nodes and depth
64; zero nodes denies every root and zero depth permits leaves without extra
folded/native nesting. Every language node is charged once, including all cold
choices, recovery, zero loops/folds, call operands and group members. Repeated
occurrences are not deduplicated by native identity, and loops are not unrolled.

**Whole language and folded preflight precedes every native cost observer**. One
iterative walk bounds pending frontiers before each push and collects only borrowed
Host sites. Folded lists preserve one child per item, except empty lists; every
optional constructor preserves one child, **including optional none**. Shared
`literal_source_extra` is also used by reference structural/source checks. Buffers,
Boxes, vectors and all four native IR slots remain unchanged without native Clone,
Debug, serde or Send bounds. The meter does not validate literal bytes or names.

The explicit native observer runs once per Host occurrence in rightmost-child-first
DFS order. `SourceCostExtra` supplies additional nodes beyond the already counted
language root and a maximum depth offset **from that language root**, not the opaque
graph root. A direct native child has offset one; zero extras are valid explicit
adapter policy. Graph closure, ingress and truthful cost observations remain native
responsibilities. The reference callback only measures its original Sequence/All
graph; operation/effect/result metadata is not queried or remapped.

**Depth checks precede node checks** for each language visit or native append;
checked arithmetic rejects overflow. The entire language tree passes before native
callbacks, then each extra is bounded before the next observer. Failure/unwind
returns no partial cost and never retries or rolls back native side effects.
Native errors move out for explicit matching, not formatting or source chains.
The result is plain mutable cost metadata, not a cached graph certificate. Native
work, allocation, interior mutation and unwind are not sandboxed or fuel-limited.

The reference helper cache, positioned operand-depth checks, every cold helper
return and computed repeat expansion reuse this measurement. Existing diagnostic
codes/spans, canonical bytes, fresh names and cold capability checks are unchanged.
Product-free proofs include folded/unfolded parity, original native/literal buffers,
complete-cold-before-callback priority, exact inclusive limits and parsed folded
operands through reservation/template/hygiene/type/value/fuel/scope cleanup.

This is **source cost measurement, not a type certificate, canonical text budget,
execution fuel or authority**. Fresh complete lexical/type/schema/return validation
and live grants are still mandatory; mutable IR/native observations can go stale.
No global registry, lock, default policy, journal, dispatch or replay grant is added.
Complete helper body/return lowering, registry lifecycle and opaque native cost
observation/validation policy remain outer responsibilities.

### Shared Helper Expansion Reservation

`helper_expansion::reserve_helper_expansion` checks caller-observed source costs
against explicit `HelperExpansionLimits`. It appends the body cost and one Bind
wrapper per parameter to a caller-owned counter, returning no cached receipt or
authority token. **Call roots and original operands are already charged**; the
reservation never charges them again or refunds them. Inclusive ceilings are
16,384 nodes, depth 64 and eight parameters. Unused parameters still count. Zero
nodes denies every body, a body must include its root, zero depth permits an
unwrapped leaf, and zero parameters invokes no operand observer.

**Node limits precede shifted body depth and native operand observation**. Policy
ceilings, parameter count and body-root presence are checked first. Checked
addition rejects integer overflow. The body begins at caller depth plus parameter
count; operand i begins at caller depth plus i plus one. Depth observations run
once in declaration order, stopping at their first failure. The counter commits
only after every check succeeds. Rejection or native unwind preserves prior
charges without callback retry or native side-effect rollback. Native errors are
available by explicit matching only; formatting and source chains redact payloads.

**Successful reservations survive downstream failure**. Template materialization,
hygiene or wrapper rejection must not refund a committed charge. A new explicit
attempt reserves again; counters do not grant replay, resume or duplicate effects.
The API allocates no graph, owns no native slots, does not clone AST/IR, and installs
no global registry, lock, default policy or execution fuel. Native observation work,
allocation, interior mutation and unwind remain trusted adapter responsibilities.

**Source cost is not physical template shape or runtime fuel**. Folded list/optional
constructors and closed opaque host graphs retain their adapter-observed weights.
The reference adapter supplies opaque graph observations to shared measurement,
while retaining exact argument alignment and its
1024-node/depth-16/eight-parameter policy. Shared reservation precedes template
materialization, fresh aliases, hygiene and wrappers. Legacy short-circuit order,
diagnostic codes/spans, canonical wire, fresh names and capability gates are
unchanged; product-free proofs compose parsed headers/arguments with templates,
hygiene, complete cold inference and identical interpreted value/fuel/scope cleanup.

This is **budget reservation, not cost measurement, a type certificate or execution
authority**. Supplied metadata can be inaccurate or stale. Complete cold source/IR
validation, truthful native cost observation, literal/type/return/canonical checks,
active-prefix limits and live versions/grants remain separately mandatory. Complete
helper body/return lowering and registry lifecycle are not installed by this API.

### Shared Effect Control And Capture

`leselang-hir::effect_evaluation::evaluate_effects_in_scope` executes Bind/Choose
control directly over the original shared IR. Pure forms and scalar recovery
reuse the pure executor; there is no second tree, product effect type, mandatory
journal, global lock or background thread. `EffectEvaluationEnvironment` supplies
three explicit non-dispatching hooks: cold effect preflight, selected request
preparation and lexical capture. Request/capture/result types need no Clone,
serde, Debug or Send implementation; GUI-local native slots and borrowed IR work.

Whole physical tree/prefix checks precede every native hook. Then **all cold effect
schemas precede fuel and actual value queries**. Shared node/depth/binding ceilings,
64-argument and 64-member ceilings include cold branches, argument forests and
group members. Conditions and recovery operands must be physically pure. This is
not static type inference: the host checks all cold types separately. Opaque Host
payload graphs are not counted or inspected by the language walk; their ingress,
authority, graph limits and codecs remain adapter-owned. Cold callbacks are trusted
schema/policy checks, not permission to invoke effects.

The selected path charges one unit per control/effect root, without double-charging
prepared calls. Bind moves pure values through one guarded lexical frame; Choose
evaluates only its selected arm. Group request construction, original member/schema
identity and result-type checks stay in the adapter. Temporary bindings disappear
on success, failure and native unwind; scalar recovery never catches an external
adapter error. Native callbacks must preserve typed failure provenance.

For a bound request, the capture hook receives the original borrowed binding name,
body and visible lexical values. It owns snapshot/projection bounds, copying and
fuel. Success returns move-only `Suspended { request, capture }` data, not a journal
row, accepted reply or replay certificate. Capture failure/unwind drops the prepared
request; nothing is dispatched, refunded or retried. A suspension inside a binding
value is explicitly rejected rather than implicitly flattened. Native callback and
destructor work cannot be preempted or rolled back; a second unwind can abort.

The receiving host correlates and validates the actual reply, restores its captured
bindings and inserts the accepted result, then re-enters the shared walker for the
remaining body. That restoration/acceptance protocol remains host-owned; invoking
the walker again is not itself exactly-once, cancellation or durable restart proof.
The shared pending-reply handle below supplies the in-memory acceptance and frame
handoff; native restoration, correlation meaning and policy remain host-owned.
Native captures may retain live references, so hosts define stable snapshot versus
live-view semantics. Full generic suspension layouts and durable reply-lifecycle
ownership remain future work, not replaced by a cloneable language frame.

The reference VM now delegates effect-position control to this module. It keeps its
canonical HIR/type/authority gates, legacy effect resolver, result/group projections,
snapshot copying/fuel and continuation schemas 1-11, journal 10 and projections
v1-v5. Malformed forged recovery operands are rejected before fuel; valid-path
fuel and wire behavior retain their regression coverage. This is not a new
continuation format or a second scheduler.

`crates/leselang-hir/tests/effect_evaluation.rs` proves two native requests, capture
of scalar prefixes and result aliases, actual reply validation and synchronous
re-entry to a pure tail. A distinct GUI-local opaque-effect host borrows non-Clone
native slots and changes widget state only in caller-owned invocation. Tests also
cover cold rejection, lazy selection, typed recovery, capture failure/unwind,
request release, resource bounds and nested-suspension rejection. These are
**in-memory control/capture proofs, not independent source-to-effect durable pipelines**.

### Shared Pending Reply Ownership

`leselang-runtime-core::PendingReply` owns an opaque identity, native frame and
borrowed original `HostResultDomain` declaration. `try_accept` performs **identity,
live authority, borrowed reply view, type, then value validation** in that order.
The host implements `ReplyAuthority` from trusted current state, not script-provided
grants. Identity equality must encode the host's exact owner/generation/correlation;
the core cannot infer those semantics from an opaque key. `Borrow<View>` supports
owned native input with an unsized or differently shaped view, without decoding,
coercion, formatting, cloning or copying the received payload.

Normal rejection returns the **exact original input and keeps the pending frame**.
The host explicitly decides whether another attempt is appropriate; there is no
automatic retry, dispatch, fuel refill or scalar recovery. A successful attempt
closes this handle before returning move-only `AcceptedReply`; `into_parts` moves
identity, frame and reply and returns the original declaration reference. The
frame can be non-Clone, GUI-local, borrowed or entirely unrelated to Leserpent.
Extracted data is not a permanent validation or execution-authority certificate:
later mutation and native interior state may invalidate the observed checks.

Before any native equality, policy, Borrow, type or domain callback, the handle
enters a fail-closed state. **Native unwind records HostUncertain and releases the
frame**, never silently rearming it. `cancel` records Cancelled before native
cleanup, releases the frame once and rejects later input without callbacks.
Repeated cancellation preserves the existing terminal reason. Dropping pending
or unconsumed accepted data uses ordinary native cleanup. Destructor panics still
propagate, and a second panic during unwinding can abort. Status and Debug expose
metadata only; reply and native error formatters are never called by them.

This is **single-handle handoff, not durable or global replay authority**. Creating
another handle for the same key is not globally deduplicated. Hosts bound aggregate
pending count, ingress and native callback work, own reply provenance, authentication,
leases, revisions, deadlines, dispatch and external cancellation delivery. There
is no hidden executor, clock, queue, database, scheduler or cross-thread dispatcher.
Rust's ordinary conditional thread ownership applies; GUI-local native values need
neither Send nor Sync. Native work cannot be preempted or rolled back by the core.

The reference VM uses a **borrowed observation checkpoint**, after its existing
result-binding and durable correlation gates. It passes the original continuation
and raw value through the shared handle, preserving pointers, saved fuel, Fault
mapping and wire formats. Its observation-only policy grants no new authority;
journal transactions, pending maps, leases and restart ownership remain unchanged.
This is not a migration of persistent frames into the generic core.

The runtime-core tests exercise non-Clone frame/input/error types, exact pointer
handoff, every native callback unwind, rejection ordering, cancellation, destructor
unwind and an unsized text view. HIR's two-call native flow and distinct GUI-local
opaque-effect flow now receive their captures through this handle, then restore
and synchronously re-enter the shared walker. Wrong identity, revoked policy, wrong
reply type/value and post-cancellation replies never restore or run the body. These
are in-memory ownership proofs; generic source lowering, opaque-effect flow typing
and complete durable suspension remain separate extraction gates.

### Shared Sequential Reply Sessions

`sequence_evaluation::SequenceEvaluation::start` enters one borrowed flat Seq,
including flattened nested Seq/Repeat output. The session owns a single fuel meter,
not the native IR, a scheduler, a database or a copied effect tree. Native field,
operation, opaque effect, result declaration, identity, request and received value
need no Clone/Debug/serde/Send/Sync bound. Unsized native domains and borrowed reply
views are supported. Ordinary Rust ownership defines thread and GUI affinity.

**Whole cold shape and declarations precede fuel and preparation**. Inclusive
limits are 16,384 physical nodes, depth 64 and 64 members; zero capacity refuses
entry. Names, literals, operand bounds, complete physical depth and every atomic
candidate are checked before any native hook. All members then undergo mandatory
native cold typing/schema/domain/opaque-graph/version/grant preflight. Shape is not
a type, schema or execution-authority certificate. One Group root is charged only
after all cold rows pass; the host bounds ingress before constructing native IR.

**Exactly one member may await a reply**. `poll` prepares only the ready original
row, charging its root once, then installs its opaque identity and borrowed original
result domain through `PendingReply`. The host evaluates its actual arguments with
the same meter and revalidates live policy. Waiting/terminal polls run no callbacks,
charge nothing and never return the request again. Dispatch is caller-owned;
returning prepared data is neither delivery nor evidence that an effect occurred.

**Accepted replies advance before handoff, never implicitly prepare successors**.
`try_accept` uses identity, live authority, borrowed view, type and native value
validation in that order. Success hands off the original row/declaration borrows
and moves received input once, then marks the next row Ready or the session Completed.
Wrong identity, denied authority or rejected result returns the unchanged input and
retains the waiting row; no successor is prepared, result coerced or fuel refilled.
Early, old and terminal replies cannot advance the session. Completed means all
rows handed off accepted replies, not durable commits or consumed final output.

Cancellation seals state before native identity cleanup. Preparation errors/fuel
exhaustion close as Failed; preparation or acceptance unwind closes as HostUncertain
before callbacks can replay. Existing terminal reasons survive repeated cancellation
and cleanup unwind. Native allocation, work, mutation, Drop and callbacks remain
trusted, unmetered except explicit host charging, and cannot be preempted or rolled
back; a second destructor panic can abort. Metadata/Debug never formats native
payloads or exposes private error chains. No implicit retry or budget refund exists.

The parsed unrelated counter proof compiles nested Seq/Repeat through the shared
source entries, cold-types actual native calls, invokes one consumed request at a
time, validates two distinct original result domains, accepts replies and advances
with exact fuel. Stale generations, wrong result types, revoked catalog versions,
missing successor grants and cancellation never invoke a later member. Separate
tests cover move-only GUI-local data, exact pointers, unsized domains, ceilings,
rejection order and every reply callback unwind. This is **a parsed sequential
in-memory vertical slice, not the complete independent language/VM gate**.

The released reference VM still owns its journal, durable sequencing and saved
continuation formats; this session does not replace or wrap that scheduler. Hosts
own authentic receipt provenance, generation uniqueness, external cancellation
delivery, aggregate pending limits and result aggregation. Recreating a session for
the same group is not globally deduplicated. Sequence result aliases/aggregation,
parallel barriers, complete helper/opaque-host source typing and durable restart
remain separate extraction work. Binding restoration is supplied by the following
entry, not implicitly by this sequence lifecycle.

### Shared Accepted Binding Re-entry

`evaluate_resumable_effects_in_scope` is an additive entry to the same synchronous
effect walker. A suspension returns a move-only `EffectContinuation` containing
the **original borrowed binding name/body**, separately from opaque native capture.
Only the shared walker constructs it; capture metadata cannot redirect the code
or binding during restoration. The old `evaluate_effects_in_scope` entry retains
its outcome, fuel and behavior by consuming this outcome through `into_legacy`.
Neither entry dispatches, clones native IR or changes reference wire formats.

Put the continuation in `PendingReply` with the exact native identity and original
result declaration. Identity, live authority, borrowed reply view, type and value
checks still precede acceptance. Normal rejection returns the unchanged payload
and keeps the continuation pending. Cancellation or callback unwind cannot yield
an accepted continuation; late input cannot restore variables or prepare a body.

`effect_reentry::resume_accepted_effects` consumes the accepted frame once. It
**preflights the whole original body and current cold schemas before restoration**,
then calls the native `restore_capture` hook. Before reply projection it reserves
one binding slot, charges one fuel unit per restored prefix entry and checks names,
duplicates, scalar bounds, quota and reply-name shadowing. The native `bind_reply`
hook consumes the actual received payload into a scalar or native result view.
Mapped scalar bounds are checked before entry to the original body; native view
compatibility remains explicit adapter policy. Schema hooks are
not repeated by the internal preflighted walker in the same re-entry attempt.

Restored values move into a locally owned lexical scope. Result aliases preserve
native view identity, and all owned variables are released on normal return,
failure or unwind. Request/capture/reply/identity/declaration/IR need no Clone,
Debug, serde or Send bound; native result views retain the existing Clone contract
for explicit local reads, not for copying incoming payloads. Unsized original
declarations and GUI-local move-only native slots are supported. Formatting never
prints captured variables, names, native payloads or private errors.

Native hooks own snapshot integrity, result-view type compatibility, live dispatch
and opaque size/work policy. They must bound native restoration before allocation
and charge their own restore/copy/projection costs with the supplied meter; the
core's prefix charge does not preempt or pay for arbitrary native work. Accepted
input is an observation, not authentic receipt or durable replay authority. Errors
or panics consume the frame without rearming, implicit retries or fuel refunds;
native mutation/Drop cannot be rolled back and a second cleanup panic can abort.

The parsed unrelated counter proof now compiles a native result binding, invokes
a consumed read request, accepts its actual reply through the original domain,
restores the bound value and prepares/invokes a write with exact cumulative fuel.
The two-call alias flow reaches a pure tail with identical legacy fuel and native
pointer identity. A distinct GUI-local host consumes a text reply through an
unsized native domain. Adversarial tests cover redirected capture metadata,
invalid prefixes, zero quotas, stale cold policy, exhaustion, projection failures,
unwind and cancellation. This is **in-memory accepted binding re-entry, not a
complete generic compiler or durable VM**. It does not add sequence aggregation,
parallel barriers, opaque-host source typing or persistent continuation layouts.

### Shared Accepted Scalar Contracts

`ScalarTypeSet` is **explicit closed scalar-type alternatives** for native schemas.
Const empty/only/with construction keeps a private six-tag set, with idempotent
union and fixed canonical iteration. No coercion, implicit nullability, raw-bit
construction, accept-all default or automatic serde codec is supplied. Copyable
type metadata is **not authority or a lasting value certificate**; adapters own
schema versioning and protocol compatibility. It can be a NamedParameter domain
or part of a developer-owned domain descriptor without any product DTO.

`validate` performs **borrowed type-before-bounds preflight** with closed,
payload-free TypeMismatch/UnboundedValue diagnostics. None, absent optional text
and present-empty text stay distinct. Existing 4096 UTF-8 byte and 64-entry list
limits remain unchanged, including aggregate list bytes. Checks neither parse,
clone, normalize, truncate, charge fuel nor call native domain validators.
These failures remain outside pure calculation recovery. Prior allocation and
aggregate ingress/host resource bounds still require adapter enforcement.

**All 70 reference operations retain their exact accepted-type matrix** while
receiving parameter domains delegate type/bounds checks to this core metadata.
Ordered projection fields use singleton sets, retaining key/type/bounds order and
their existing ProjectionError mapping. Host node/text domains, cold checks,
capabilities, declaration-order evaluation, fuel, durable bytes and re-entry stay
unchanged. Successful checks must be repeated after mutable value/schema changes.

Two unrelated native GUI/device schemas prove combined named/type preflight,
not complete independent hosts. This is **not a generic effect evaluator or
automatic GUI adapter**; domain approval, dispatch and suspension remain host-owned.

### Shared Fuel Accounting

`leselang-runtime-core::Fuel` is **host-granted fuel accounting**, independent of
HIR, product values and persistence. It owns one `u64` counter; `charge(cost)`
debits exactly or returns allocation-free `FuelExhausted` without changing the
counter. Zero cost is a no-op. Debit never wraps or panics on underflow. There is
no default grant, refill, cloning or serde representation of the meter. The host
can explicitly construct a new grant; a numeric remaining count alone is not
restoration or execution authority. This is **not a sandbox or callback preemption**.

The reference VM routes pure evaluation and copied local/result/group projection
costs through this same meter, as well as prefix reservations, sequential branch
accounting and each host operation. Re-entry constructs it from **validated saved
remaining fuel**, never from a reopened VM's configured initial fuel. Exhaustion
still maps to the original contextual `LSV1001` messages. Internal zero-budget
effect construction now faults before subtraction instead of underflowing.

Existing cost rules and numeric **fuel wire fields remain unchanged**, including
parallel branch policy: the primitive does not introduce a batch-wide counter or
change how parallel siblings receive their existing budgets. The VM retains its
1,000,000-unit ceiling; the generic meter itself accepts any host-granted `u64`.
Admission attempts may repeat bounded pre-publication preparation, but accepted
roots, waits, lease redelivery, semantic retry and recovery do not receive new
fuel through this extraction. Host ingress still bounds aggregate preparation.

The adapter/evaluator must debit before bounded work and choose a terminal policy
for exhaustion; a failed charge leaving the counter unchanged is not a retry or
refill grant. Fuel does not meter arbitrary native CPU, elapsed time, allocation,
external side effects or total process usage. `leselang-runtime-core` owns only the
meter; HIR now shares pure copy/control costs and execution, while the reference
VM retains effect costs, continuation validation and durable replay.
Continuation schemas 1-11 and journal schema 10 remain unchanged.

### Shared Scheduler Clock

`checked_clock_add(now_ms, offset_ms)` is **portable scheduler-clock arithmetic**
in `leselang-runtime-core`, with allocation-free `ClockError::{OutOfRange, Overflow}`.
The inclusive range is `0..=MAX_CLOCK_MS`, where the maximum is `i64::MAX`.
It validates the input before adding and checks the resulting range; zero offset
is allowed. It performs exact integer arithmetic, not floating-point conversion.
It never silently saturates, samples a system clock, installs a timer or advances
state. The host still owns duration policy, clock domains, per-handle monotonicity
and contextual diagnostics. A valid timestamp is not execution or replay authority.

The core execution-deadline adapter and VM dispatch-lease/semantic-retry adapters
map this shared arithmetic back to their existing error codes/messages. Admission
backoff has an explicit overflow-to-pinned-deadline clamp; lease/retry timestamps
instead fail on overflow. These distinct policies must not be flattened into one
retry rule. The primitive does not extend a restored deadline or validate a
continuation by itself. No absolute-time wire field or journal schema changes.

**Current-time validation is distinct from future-time construction.** Read-only
`scheduler_pressure` and acknowledgement of an existing lease accept the full
inclusive clock range, including `MAX_CLOCK_MS`; they do not require a synthetic
one-millisecond future lease. A new positive-duration claim at `MAX_CLOCK_MS`
still fails with `LSV4015` before any state change. Pressure observation never
reaps execution deadlines or consumes pending work, including at that boundary.
Out-of-range pressure queries retain `LSV4015`; out-of-range completion/cancellation
clocks retain `LSV2011` and do not mutate state.

The existing lease boundary is preserved: a lease is counted active only while
its expiration is strictly later than the observation clock, and redelivery can
supersede its attempt at that expiration. A completion at the exact stored lease
expiration can still commit if no newer attempt won first; it must pass the usual
request, attempt and expiration fences. A due execution deadline takes precedence
and produces `DeadlineExceeded`, even if that lease would otherwise be valid.
Retry overflow remains `LSV2203` without consuming an untimed pending lease;
the host can still complete or cancel it. None of this extends a lease or changes
durable terminal replay, clock-domain ownership or continuation authority.

### Shared Retry Delay Arithmetic

`leselang-runtime-core::capped_exponential_delay(base_ms, max_ms, doublings)` is
**pure capped exponential delay arithmetic**, shared by root admission and
semantic effect retry. It computes `min(base_ms * 2^doublings, max_ms)` exactly
over the full integer domains without allocation or exponent-sized loops.
Zero base/cap produces zero; a lower cap is respected from exponent zero. Shift
or multiplication overflow with a positive base produces the cap, never a
wrapped interval or a reset to the base. Absolute wakeup/deadline addition still
uses the separate clock helper and each caller's overflow policy.

This broader arithmetic domain does not weaken policy validation. Admission's
positive delays, finite total attempts and submission-pinned deadline remain
unchanged; semantic retry retains its own validated policy, error classification
and retry limit. Both callers translate one-based counts to `doublings = count - 1`.
Admission attempts, delivery attempts and semantic retries remain distinct budgets.
The helper has no state, host callback, timer, jitter or cancellation authority.

Retry/wait/recovery does not refill saved fuel or extend execution deadlines.
Reference tests check the default trajectory, one-millisecond intervals,
non-power-of-two caps and maximum policy bounds in both journal backends, including
restarting with zero default fuel between retries. Not-before gates and stale-lease
fences still apply, and permanent failures do not become retryable. This arithmetic
extraction adds no policy or wire fields and is not evaluator independence.

### Restore And Allocator Recovery

Successful `restore` and `restore_request` imports advance the **durable sequence
high-water mark in the same transaction** as the continuation and optional outbox
request. Identical imports repair a lagging watermark without creating another
dispatch. Rejected imports and failed watermark writes roll back together with
their records; bare images still reserve identity even though they are not
dispatches. Already-open workers allocate from the shared watermark, not only
their local startup snapshot. Lower imports never reduce a higher watermark.

Opening a journal validates its effects, dispatches, complete group graph and
debugger audit against **one consistent write-locked snapshot**, and repairs
legacy lagging metadata before committing. The watermark includes pending and
completed numeric identities, canonical merge IDs and validated **unused cold group
reservations**, including cancelled groups. Cold reservations must have allocation
proof from the **original watermark before repair**; a missing proof fails closed,
and a high imported identity cannot legitimize a forged reservation. All validation
must succeed before repair; a late failure leaves metadata unchanged. Opaque legacy
group IDs remain valid and are not interpreted as numeric reservations. At the
largest valid identity, allocation fails explicitly rather than wrapping.

Normal allocation remains a constant-size metadata operation; it does not scan
the journal on every allocation. Startup reuses the existing bounded validation
scan and serializes with journal writers, not with unrelated VM engines.
Compaction never lowers the watermark. Repair cannot reconstruct identities
already deleted by an older buggy journal whose watermark also lost them; restore
that journal from a trusted backup or use a new host identity namespace rather
than claiming historical deduplication. Imports do not authorize cross-journal
command replay or change continuation schemas 1-11 or journal schema 10.

### Identity And Host Resources

Effect IDs, continuation tokens, merge IDs and generated command idempotency keys
are **journal-local, not globally unique**. Independent journals may both emit
`effect-1` and `leselang-effect-1`. Host routing/deduplication must include a stable
journal/session namespace that survives restart; it is host-owned, not script text.
The native debugger routes presentations through session ID plus effect ID and
revision, and cancelling one session cannot consume another session's continuation.

For raw mutating command envelopes directed at the same command service, use one
shared sequence/journal namespace. Separate journals require a persisted, validated
adapter-level identity mapping before sharing that service; the current raw VM
command API does not supply that mapping. Do not invent a fresh random namespace
on redelivery or blindly rewrite a VM request and invalidate its result correlation.
Sharing a journal also shares trusted worker access; it is not a tenant boundary.

VM-local isolation does not isolate shared host resources. The host must serialize
conflicting mutations by **resource lane** (for example, a window/form or service),
respect its GUI dispatcher and revalidate revision/target authority. `all` is not
a transaction, a data-race detector or a promise that two writes to the same form
are safe. Independent lanes can progress without one universal GUI/VM lock.

The next scheduler layer needs bounded aggregate ingress/wakeup ownership, fair
resource-lane scheduling, accepted-work cancellation ownership and an explicit
execution identity. The quota and admission APIs do not start a worker pool,
install a timer or automatically queue rejected source.
No shared globals, implicit `spawn`, source-level async model, nested parallel
graphs or native multi-presentation batch protocol are added by this contract.

## Current Boundary And Next Proof

`leselang-syntax`, `leselang-host-contract`, and `leselang-hir` have no Gewyvern
or Leserpent product crates in their dependency closures. That is a dependency
proof, not full semantic independence: reference HIR still names concrete runtime and UI
effects, though its computation nodes now specialize shared host-parameterized IR.
The reference lowerer/type rules and host contract still contain runtime selectors and deployment
validation. `leselang-vm`, `leselang-ui`, and `leselang-observe` still consume
Leserpent command/result types. `leselang-command` is intentionally the product
adapter and should remain outside the standalone core.

The first extracted runtime component is `leselang-runtime-core`: admission
ownership, finite backoff, fuel accounting, portable clock/deadline and retry-delay arithmetic
plus shared closed scalar data/operation, bounded control decisions, owned fold
traversal, incremental list construction and borrowed lexical bookkeeping with
the `Fault` DTO.
Its only normal dependency is `serde`, with `serde_json` used only in tests; its
normal, build and test dependencies contain no other workspace crate. The reference
VM delegates through an explicit adapter and re-exports the existing public type
paths, preserving fault and outcome JSON as well as continuation/journal bytes.
Counter and text hosts exercise two unrelated opaque admission schemas, including
non-cloneable inputs/outputs and separate worker ownership. This is **not** the
typed language-operation schema or complete evaluator acceptance gate.

[Runtime-core package instructions](../crates/leselang-runtime-core/README.md)
cover Cargo's normalized standalone package and its isolated test build. The
source manifest currently inherits monorepo version metadata; packaging removes
that inheritance. Moving source to a new repository requires its own metadata
and CI, not a blind directory copy. Native operation declarations/catalog preflight
are shared, and the control IR supports native argument typing, bounded pure
type inference, shared pure execution, atomic call value preparation and effect
control with native capture hooks, an atomic source-call bridge, shared literal/operator
and choose/recover/list source construction, bounded scalar lexical bind/local/loop/fold source,
native field/member projection source, owned generic helper-body hygiene, shared
scalar helper signatures/argument preparation, borrowed helper dependency planning,
owned parameter wrappers, owned helper template entries with explicit materialization,
borrowed helper header/physical declaration admission with explicit entry/policy,
checked source-expansion reservations, shared source-weight measurement with explicit
opaque native observations, owned bounded normal-return composition with explicit
continuation factories, lowered helper-body admission with independent pure return
corroboration and explicit native validation, closed cold group export observation
with explicit native identity hooks, bounded native result-binding source orchestration
with explicit child/type-frame/finalization hooks, native choice source construction,
flat native group source assembly with all-member output checks before admission,
bounded flat repeat source assembly with complete physical/source reservations,
explicit once-ordered native factories and exact output shape/cost admission,
owned sequential source composition with explicit child sequences and native admission,
owned repeat-sequence source assembly with complete forest reservations and positional admission,
borrowed sequential reply sessions with once-ordered native request preparation,
one owned fuel meter, reply-gated re-entry and cancellation, in-memory pending-reply handoff,
and accepted binding re-entry with core-owned original sites and bounded native restoration.
Full generic host-result/group child source compilation, complete helper body/return lowering and
registry lifecycle/opaque-native cost observation and validation policy,
opaque-effect typing/dispatch, complete suspension and durable
reply ownership still need separation. The current
VM's SQLite backend is not moved into, or required by, this lifecycle crate.

The extraction acceptance gate is:

1. A headless host unrelated to Gewyvern compiles, runs, suspends, resumes and
   cancels a program without any Gewyvern/Leserpent product dependency or GUI.
2. Two independent host schemas can supply different typed operations without
   modifying the parser, core HIR or VM. Unknown operations, wrong result types,
   denied capabilities and incompatible versions fail before unsafe dispatch.
3. The in-memory path needs no persistence backend. A durable backend proves
   restart, stale-result rejection and effect replay under the same protocol.
4. The existing Leserpent adapter preserves command bytes, authorization,
   revision fences, GUI/CLI/code parity and journal migration compatibility.
5. Dependency-closure tests cover the complete embedded core, not only the
   syntax crates; a standalone consumer builds without this workspace's product
   metadata or generated product assets.

The status cell `leselang/language-vm/host-neutral-embedding` tracks this target
as developing, separately from mature released Leserpent integration. Keeping code in this
monorepo temporarily does not relax these boundaries or require an immediate
repository move.

## 2.3.0 Repository Extraction Target

**2.3.0 is the desired extraction checkpoint, not an automatic version-triggered move.**
The existing language and free Leserpent capability set remain the compatibility
baseline; this track removes product coupling rather than adding an OS shell or
another GUI framework. Prioritize complete vertical proofs over more isolated helpers.

| Stage | Required Exit Proof |
| --- | --- |
| 2.2.0 foundation | Host-defined catalogs feed generic typed IR, evaluation, values and suspension frames; one unrelated host compiles, runs, yields, resumes and cancels without product DTOs or mandatory SQLite |
| 2.3.0 extraction gate | Two unrelated host schemas pass the same complete language pipeline; exact versions, unknown operations, wrong typed replies and denied capabilities fail before unsafe dispatch; optional durable storage proves restart, stale-reply rejection and no duplicate settled effects |
| Repository move | Existing Leserpent wire/auth/GUI parity remains green; the complete core has product-free dependency closure, its own source manifests, CI, docs, MIT license and publish boundaries; a clean external consumer builds without monorepo metadata/assets |

Current native catalog/IR tests satisfy registration/selection/binding and generic
node reuse, not the 2.2.0 vertical gate. Remaining blockers are concrete reference
lowering/type policy and host operation/result acceptance,
product-coupled evaluation/recovery frames and mandatory VM SQLite linkage.
If any acceptance proof remains missing at 2.3.0, retain the shared implementation
in this repository and record the blocker instead of shipping a misleading split.
Physical extraction follows acceptance, not directory relocation alone. FFI
packaging and the nuis OS / sirius kernel shell remain deferred.

## Control Flow And The Future Shell

Pure scalar computation, immutable `bind`, lazy `choose` and whole-group computed
host arguments now precede host suspension alongside `seq`, bounded `repeat`
and flat `all`. Bounded pure `loop` adds condition-first scalar-state iteration
under the same fuel budget, before suspension or after one captured result.
Atomic results can re-enter pure scalar bodies or continue bounded multi-step
chains using typed field projections and durable lexical frames, without copying
raw host objects into continuations. Chains keep original authority and shared
budgets, with atomic admission and replay. Mixed pure/suspending branches can
return a typed scalar early or continue through another capture; cold branches
still undergo type/capability checks. Explicit `to_string`, `parse_integer` and
`parse_boolean` bridge scalar calculations and text-based host parameters with
strict formats, shared fuel and redacted conversion faults, never implicit
coercion or a host-validator bypass. Pure `recover` selects a lazy same-type
fallback for local arithmetic/parse failures without refunding fuel or catching
host, authority, resource-limit or cancellation failures. Named group-result
binding now exposes statically typed `member`/`field` projections to a pure scalar
body after the whole group succeeds, with shared fuel and atomic journal replay.
A sequential group can instead drive one atomic tail (up to 63 prefix members
plus one tail), admitting it transactionally with the final prefix acknowledgement.
The reserved identity, original authority/budgets and whole-group recovery prevent
replay from recalculating or dispatching it twice.
That successor can now be captured for a pure scalar body using schema 8. Bounded,
versioned named-member projections preserve the earlier group/alias environment;
raw receipts stay in the journal, and final calculation commits with the successor
receipt only after cumulative output checks.
Flat parallel `all` prefixes now support those same single-successor forms with
schema 9 and an all-success barrier. Their 2 to 63 prefix members remain
independently leaseable and can finish out of order. Exactly one tail is admitted
with the final successful receipt, under the original authority/shared budgets;
cumulative raw-output overflow is a durable terminal. Complete journal recovery
also verifies that every prefix member succeeded before a tail was admitted.
This is a Rust batch API capability, not native multi-presentation GUI support.
Continuation schema 10 extends sequential/parallel groups to multiple captured
atomic successors with a final pure scalar. Prefix plus longest cold chain stays
within 64 slots; all possible successor identities are reserved, while only one
selected successor is pending at a time. Each receipt and next request or final
scalar commits transactionally under original authority/shared budgets. Closed
group/result frames are validated against committed successful predecessors on
whole-journal recovery; isolated restoration is rejected. The database remains
at journal schema 10. Sequential chains work through the native single-presentation
channel; parallel starts retain the batch preflight fence.
Continuation schema 11 adds typed conditional scalar exits before the first group
capture or between captures. Every path returns the same scalar type, while cold
branches still undergo type/capability checks and reserve the longest graph.
The raw receipt and selected scalar or successor commit together after cumulative
output checks, under original authority/shared budgets. A scalar exit does not
bypass the parallel prefix's all-success barrier. Recovery verifies that the saved
body can return without another suspension, replays without recalculation, and
rejects detached restoration. Journal schema 10 is unchanged. Sequential native
debugger flows support these exits without extra presentation requests; parallel
prefixes remain Rust-batch-only.
Reusable pure helpers now declare typed scalar parameters and infer scalar results;
all declarations are checked, recursion is forbidden, and bounded hygienic expansion
uses existing HIR nodes and durable bodies rather than product-specific macros.
Selection and form-requirement results also export typed booleans for decisions
and helpers; versioned closed projection frames preserve old journals without
synthesizing missing fields. Their current operation vocabulary is still a host
adapter concern, not a guarantee of arbitrary GUI-property access.
Confirmed text/form results now export closed string fields for `expected`,
`field` and submitted `value`, usable in helpers, conditional exits and subsequent
host arguments. These are acknowledged request data, not new live GUI-property
queries. Projection vocabulary v3 preserves v1/v2 frames without synthesizing
missing fields, including through aliases and cold branches. Strings keep original
operation domains, shared copy fuel, raw-output fences and the 64 KiB image/plan
limit; saved text must match committed raw receipts. Continuation schemas 1-11 and
journal schema 10 remain unchanged. Native sequential form chains support this
without exposing private frames. Node/action/form-input kind assertion/wait pairs
now also export canonical `kind` token strings through closed vocabulary v4.
Tokens share source/wire spelling and the receiving operation's domain validation;
no case folding, enum ordinals or arbitrary properties are added. Legacy v1/v2/v3
frames stay exact without synthesized fields, including cold aliases/group members;
saved kinds must match committed raw receipts under the same budgets and transaction.
Native sequential kind-driven waits/decisions do not expose private frames.
Typed `optional_string` values now distinguish absent from present-empty text.
Explicit construction, `has_value` and lazy `value_or` support pure helpers/loops;
only four placeholder/unavailability operations export `optional_expected` and
their optional-text arguments accept it directly. Closed projection v5 preserves
v1-v4 without synthesized fields, and explicit null/string payloads reject missing
data. Original text/size/fuel/authority bounds, raw-receipt matching and transactional
replay also apply to optional locals/frames. Native defaults preserve absent/empty
semantics without exposing private frames. Arbitrary credential objects, dynamic
properties and generic nullable/container types remain unsupported.
Bounded `contains`/`starts_with`/`ends_with` predicates now inspect exact strings;
`char_at` returns one optional Unicode scalar at a zero-based index, not a byte
or grapheme offset. Out-of-range access is absent, with no implicit unwrap.
Pure helpers/loops and saved result/group bodies reuse the existing binary HIR,
complete-input scan/copy fuel, host validators and transactional replay. The
Rust native debugger proves text-selected form prefixes through the existing
correlated acknowledgement channel, not direct GUI introspection or a new ABI.
Bounded `string_list` data and pure `fold` now support ordered text collection,
splitting/joining/indexing, filtering and aggregation. Lists are limited to 64
entries and 4096 combined UTF-8 bytes; traversal is synchronous, type-preserving
and shares fuel without expansion, truncation or mid-loop suspension. Closed
value payloads and hygienic accumulator/item bindings survive transactional
re-entry; existing host domains still reject lists until explicitly converted.
Reusable effectful functions now compose whole-flow/tail calls and explicit result
bindings through bounded normal-return splicing into existing HIR. Pure signature
arguments evaluate once; hygienic result/group locals and all cold returns retain
type/capability checks. The runtime has no hidden call stack: existing journals
resume without source/helper tables under the original fuel, authority and deadline,
including transactional successor rollback and first-commit replay. Continuation
schemas 1-11, journal schema 10 and projection vocabularies v1-v5 remain unchanged.
Prepared atomic members now admit helpers and pure preparation/selection inside
seq/all/repeat when every path has one uniform `HostOperation` signature. The full
group resolves before admission, retaining source-order preparation, signature-order
once-only arguments, isolated locals, expanded repeat bounds and fail-before-dispatch
semantics. Journals save resolved requests only; existing named projections,
all-success barriers, transactional execution-row rollback, non-reused reserved
identities and first-commit recovery remain intact. Native sequential group-to-form
proofs use correlated acknowledgements; parallel debugger starts remain preflighted
out before journals, not exposed as a GUI batch. Result-capturing/multi-step group
members and pure operand/loop/fold/recovery positions still cannot hide effects.
Selected named groups now preserve a closed signature through pure `choose`,
including group-returning helpers: every cold path keeps the same mode, ordered
names and `HostOperation` signatures. Guards and selected preparations run once;
only resolved requests are saved, so restart does not reselect. Existing member
projections, parallel all-success barriers, shared budgets and transactional
receipt/successor replay remain intact. Native sequential acknowledgements cover
the selected group-to-form path; native parallel starts still fail preflight
before journals. This is static composition, not dynamic topology or nested
result-dependent groups.
Prepared atomic result bindings reuse the same bounded `HostOperation` classifier
for direct pure preparation/selection inside a capture value. Every cold path has
one uniform operation/result signature, never hidden captures or multiple effects.
Preparation locals do not escape into saved frames. Existing atomic/group chains
and early exits retain closed legacy projection fields, shared authority/fuel/deadline,
one-slot capture reservations and transactional receipt/successor retry. Committed
requests are resolved before suspension and never reselected on restart; rollback
may recalculate uncommitted pure choices, and concurrent workers commit one successor.
Native correlated receipts cover the result-driven focus-to-form path without new
GUI endpoints or private frames. Existing schemas and wire shapes remain unchanged.
Selected data-returning functions allow pure choices of direct helper calls and
pure data fallbacks as an explicit binding value. Different supported helper
topologies join through the same bounded data type, with selected-only argument
evaluation, hygienic local scopes and every cold return reserved before cloning.
Existing HIR normal-return splicing keeps transactional caller admission, parallel
barriers and shared authority/fuel/deadline fences without a runtime call stack
or wire migration. Native single/sequential paths retain correlated acknowledgements;
selected parallel starts still fail preflight before journals. Arbitrary nested
effectful binding values remain rejected.
Generic/nested containers remain unsupported. Effectful loops with
exit/skip semantics and explicit host-effect recovery/cleanup remain pending. These are core language
semantics, not GUI macros or complete control flow; see the
[implemented control-flow contract](leselang-control-flow.md) for exact limits.

A future OS profile can supply process/service control, filesystem access,
bounded streams, pipelines, job cancellation and terminal interaction through
the same typed capability boundary. Interactive sessions and long-lived jobs
need renewable, host-authorized execution slices rather than unbounded VM
steps. Process launch is an explicit host operation, not implicit interpolation
through Bash. No particular OS API names or shell syntax are frozen here.

OS integration is a deferred direction, not a prerequisite for Leserpent
maintenance or the first independent embedding. The next proof is the small
unrelated host above, not an OS-sized rewrite.
