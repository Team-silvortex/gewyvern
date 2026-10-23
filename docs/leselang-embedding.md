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

### Optional Leserpent Profile

`leselang-hir` now has an explicit `leserpent-profile` feature, enabled by default
for compatibility. It owns the existing fixed operations, product result fields,
UI DTOs, legacy lowerer/canonical formatter and their root API paths. The
implementation is included from `leserpent_profile.rs` only when enabled; it is
not a second IR or lowerer. Existing Leserpent consumers need no API or wire change.

An independent local consumer can omit that profile:

```toml
[dependencies]
leselang-hir = { path = "../gewyvern/crates/leselang-hir", default-features = false }
```

The native dependency closure contains `leselang-syntax`,
`leselang-runtime-core` and serde, not `leselang-host-contract`,
`silvortex-identity`, the product VM, SQLite or a daemon. The same public generic
catalog/compiler, typed IR, pure/effect evaluation and borrowed reply sessions
remain available. Native operation/result types, authority and reply correlation
still come from the embedding adapter; disabling the product profile grants none.
`Effect`, `lower`, `computation`, `host_call` and `result_field` are product APIs,
not native-profile fallbacks. Compile-fail tests enforce their absence.

**Cargo features are additive**. Another dependency enabling the default profile,
an explicit `leserpent-profile` grant or `--all-features` brings it back. Use an
isolated consumer's resolved dependency graph to check the actual embedding
boundary, rather than treating one manifest flag as a process-wide exclusion.
There is no runtime switch or security sandbox implied by the build feature.

Run the native tests independently from the default compatibility suite:

```bash
cargo test --locked --offline --profile lean -p leselang-hir --no-default-features
cargo test --locked --offline --profile lean -p leselang-hir
```

Product-only harnesses and reference comparisons require the profile; generic
cases in mixed harnesses still run without it. The native suite includes parsed
scalar and structured-result multi-function suspension/re-entry/cancellation
verticals. CI runs the native suite and strict native-only lint separately from
the default/all-feature product compatibility gates. This closes the optional HIR
profile/dependency boundary, **not the complete independent VM or repository extraction gate**.
Full groups, portable durable recovery, product-adapter migration and independent
source metadata/distribution remain tracked work.

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
reference VM can disable its default SQLite backend, but still uses product
types; full host-neutral runtime/recovery separation remains work.
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
Its open branching cursors may allocate, so callers bound structure before inspection.
Leaves and unary chains need no scratch allocation; wide nodes retain one sibling
cursor rather than one pending reference per child.
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
recovery frames still need generic host policy. SQLite is now an optional reference
backend, not a prerequisite for native or memory-only builds.

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
branching purity cursor stack may allocate, so structure must be bounded first. The bridge does not
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
and suspension/recovery frames are not established by this slice alone; SQLite is
an optional reference backend rather than a native-core dependency.
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
persistence remain separate acceptance gates before physical extraction.

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

### Uniform Atomic Call Execution

`prepared_call_evaluation::prepare_atomic_call_in_scope` executes the existing
prepared-call IR contract: pure Bind/Choose preparation ending in exactly one
registered Call on every path. One aggregate physical budget covers all cold
branches, pure operands and the borrowed value prefix. Physical admission precedes
native catalog comparisons; every cold terminal call then passes exact version,
grants and named shape before execution. Every terminal must select the **same
original schema row**, not merely an equal result type. A different operation in
an unselected arm is rejected without consuming fuel or projecting native values.
Host/Group members, effect-valued bindings, scalar tails and hidden effects in
operands are not admitted by this call-only profile.

Selected execution reuses the shared synchronous control walker and existing
declaration-ordered argument evaluator. The caller supplies one existing Fuel and
still-live ScopeFrame: there is no prefix snapshot, continuation capture, extra
wrapper charge or double call-root charge. Bounded pure loops, scalar recovery and
lazy operands keep their existing semantics. Values are all evaluated before
native argument-domain callbacks. Optional arguments remain omitted; returned
PreparedCall borrows the original catalog schema and owns its evaluated arguments.
Direct-call cost and failure behavior match the released `prepare_call_in_scope`.

Physical/catalog admission is not cold type inference: hosts still use
`infer_prepared_call_type` to check types across all paths. Preparation does not
dispatch effects, allocate identities, grant current execution authority or
validate returned replies. Native errors move out without formatting private
payloads or becoming scalar-recoverable. Temporary locals unwind on failure or
native panic; no fuel refund, rollback or automatic retry is promised. Native
catalog/projection/domain work, result-view cloning and Drop are host-bounded,
trusted and non-preemptible.

`crates/leselang-hir/tests/prepared_call_evaluation.rs` proves original declaration
identity, once-only selected projection, declaration/submission positions, exact
fuel, cold denials, guarded cleanup, bounded loops/recovery and native unwind with
non-Clone native slots and opaque errors. A numeric host joins pure Bind/Choose
members through complete native group preparation into both original Seq and All
reply protocols. Seq publishes one request per accepted reply; All accepts reverse
reply order and returns original ordered rows. The same borrowed meter runs
`100 -> 79 -> 74`, without an additional meter or effect before explicit invocation.

This entry itself is **uniform atomic IR preparation, not a new source group-helper profile**.
Legacy catalog group program entries still require direct registered member calls.
The separate opt-in compiler gate below admits prepared source members; it does
not change this runtime entry. No product VM migration, native scheduling, durable
recovery or complete independent VM/repository extraction is claimed.

### Prepared Atomic Group-Helper Program

`catalog_group_source::compile_catalog_prepared_group_helper_program` explicitly
extends the group-helper program profile with atomic helper members and pure
Bind/Choose preparation. Every path must end in exactly one registered call to the
same original operation. The legacy source, entry-only and group-helper entries
retain their direct-member policy; there is no implicit upgrade or new grammar.

```lese
fn main() = all(left: prepare(n: 40), right: counter.right(n: 40))
fn prepare(n: integer) = bind(amount: add(left: n, right: 1),
    body: choose(when: true, then: counter.left(n: amount),
        otherwise: counter.left(n: n)))
```

The shared physical, closed-reference, helper dependency, named/catalog and group
header gates run across all declarations first. A native-free source-shape pass
then classifies helper bodies once in dependency order, without AST substitution.
Every member occurrence, including unused helpers, must have atomic shape before
native compilation hooks. Pure-only members, nested groups, effectful operands,
multi-effect bindings, scalar tails and mixed operations are rejected. These
bounded facts are admission policy, not types or execution authority.

Expanded IR reuses shared uniform prepared-call inference before branch mapping.
It requires the same original schema row across all cold terminal calls; equal
result tags are insufficient. The borrowed original operation and result row feed
the existing group declaration mapping and whole group flow typing. Helpers keep
shared hygiene, recompilation, nonrefunding work/depth quotas and shifted aggregate
output limits, with no Clone/Debug/serde/Send requirements on native slots.

Hosts prepare complete groups with `prepare_atomic_group_in_scope` while the lexical scope
is live, then use the existing prepared Seq/All reply protocols. Preparation does
not dispatch or capture helper-local values. Correlation is distinct from fresh
publication authority; actual invocation, authentic replies and cancellation
remain explicit host responsibilities. This is not result-dependent Seq operands,
atomic external dispatch, a scheduler or durable execution authority.

Parsed numeric-host tests cover forward aliases, helper arguments, pure Bind/Choose,
original rows, wrong identities, reverse All replies, one outstanding Seq request,
enclosing scope restoration and return value 83 with one meter `300 -> 260 -> 244`.
Cold unused-member denials precede mapping; preparation refusal, fuel exhaustion,
cancellation and fresh revocation do not dispatch successors or re-enter the caller.
Already invoked effects are not rolled back. Native callbacks, cloning of host
result views and Drop remain trusted, host-bounded and non-preemptible. This source
profile does not promote product VM migration or independent VM/repository extraction.

### Prepared Host Expressions

`catalog_group_source::compile_catalog_prepared_group_source` accepts one expression
and an explicit borrowed type prefix. Independent hosts can compile prepared
Seq/All members, pure Bind/Choose preparation, Repeat and direct sequential
composition against existing host variables without synthesizing a parameterless
entry or helper declarations. The earlier `compile_catalog_group_source` keeps
its direct-member-only behavior; multi-function callers keep the prepared program
entry and its separate nonrefunding helper-expansion budget.

This entry uses the same closed lexical/source gate, full catalog admission,
atomic-member facts, source-only export preflight, owned group kernels and final
whole-output inference. Missing/duplicate prefix names, unsupported nested groups,
mixed terminal operations, invalid grants or export bounds cannot bypass those
gates. An expression has no helper namespace or ambient fallback. Native result
metadata in a prefix still belongs to its original host/schema; equal field names
do not authorize a foreign result.

The prefix supplies types, not runtime values or credentials. The host separately
provides bounded matching initial bindings to its original reply session. Values
are not interpolated into source or baked into IR: the same admitted tree can run
with fresh inputs in separate sessions against the same original schema/context.
Current grants, native provenance, authentic receipts, capture ownership and
cancellation must still be checked at use time. Compile-time types neither validate
untrusted input values nor authorize dispatch; there is no automatic session or
cross-host cache policy.

The headless GUI proof compiles one repeated label/visibility expression using
external `target`, `prefix`, `suffix` and `enabled` bindings. Two executions return
`alpha.!` and `bravo.?` through original member receipts and one restored capture,
with equal fuel for equal-size inputs; a third changes `enabled` to false and takes
the other prepared branch without recompilation. Missing/wrong runtime inputs, exhausted
fuel and revoked grants fail before invocation. Cancellation or stale authority
after group acceptance prevents captured-input restoration and final projection;
settled effects are not replayed or rolled back. These tests run unchanged in the
standalone consumer, not in a real GUI toolkit. Helper-returning group members,
result-dependent group operands and generic durable recovery remain separate gates.

### Prepared Repeat Programs

The same `compile_catalog_prepared_group_helper_program` entry accepts
`repeat(times: 3, body: prepare(n: 40))` in the entry or an acyclic helper.
It compiles to the existing sequential group with `iteration_1` through
`iteration_3` exports. A caller can read an original reply with
`member(value: batch, name: "iteration_3")` and then project its declared fields.
No additional parser, evaluator, scheduler or native compiler callback is needed.

`times` is a positive integer literal bounded by `max_branches`. All declarations,
including unused helpers and cold branches, pass repeat headers, closed names,
catalog policy and uniform atomic-member shape checks before native mapping.
Pure Bind/Choose and atomic helper bodies work. Direct sequential composition is
described below; effectful operands, mixed terminal operations and result-dependent
sequence operands remain outside this profile. Older compiler entries retain their limits.

The existing repeat kernel reserves the full shifted physical and folded source
cost before creating later instances. Each instance is recompiled through the
original hygienic helper path and nonrefunding program work budget, without copying
native slots. Full group type inference and aggregate call/output limits still apply.
Generated names use the existing repeat convention; no additional execution fuel
meter is created. Native metadata hooks remain trusted and must bound their work.

Counter and headless GUI programs compare three repetitions with three explicit
Seq members: identical runtime fuel, one outstanding reply at a time, original
receipt checks, enclosing capture restoration and the same final projected value.
Cancellation, revoked capability or stale widget generation stops remaining
publication without replay or rollback of settled effects. These proofs also run
in the standalone source consumer. This closes prepared-repeat compilation,
not generic durable recovery or complete VM extraction.

### Composed Sequential Programs

The prepared program entry also accepts direct `seq` and `repeat` children of
`seq`, and a direct `seq` or `repeat` body of `repeat`. For example:

```lese
fn main() = repeat(times: 2, body: seq(
    label: label.set_text(target: "status", text: "ready"),
    visible: widget.set_visible(target: "status", visible: true)))
```

The original owned sequence/repeated-sequence kernels flatten this into one Seq.
Names preserve source order as `iteration_1__label`, `iteration_1__visible`,
`iteration_2__label`, `iteration_2__visible`; nested Seq names use `parent__child`.
The enclosing program can project these exact names through the existing member
and field operations. Native declaration/payload ownership moves unchanged.

All cold headers, grants and helper facts precede native mapping. Before lowering
any declaration, source-only sequential export analysis also checks total expanded
width, 64-byte joined labels and collisions, including unused helpers and untaken
choice arms. It visits each direct repeat body once and checks the width product
before generating at most 64 bounded names; it does not expand helpers or touch
native slots. The shared join predicate covers both this check and the owned
assembly kernels. Repeat name checks use the longest iteration label, so crossing
from iteration 9 to 10 cannot silently exceed the byte limit. Cold repeat errors
use iteration 0, with the original span and member index where applicable.

Source depth and group-site limits still bound recursive compilation. The existing
owned kernels independently check final width, names, collisions and shifted
physical/folded source costs; repeated forests are reserved before later factories.
Source export facts are not type/schema authority or cached permission to dispatch.
Each helper instance keeps its original hygienic, nonrefunding compilation budget.
Complete final group inference remains mandatory. Intermediate source groups do
not become runtime frames or add execution fuel: six counter spellings and three
heterogeneous GUI spellings match their explicit flat sequence's runtime budget.

One original reply session releases one step at a time. Wrong member receipts,
cancellation, stale widget generations and revoked grants cannot publish a later
step or re-enter the caller; settled mutations are neither replayed nor rolled back.
Both host proofs also run unchanged in the independent source consumer.

Only direct source composition is enabled. Nested `all`, Seq/Repeat inside an All,
group-returning helper aliases or Bind/Choose wrappers used as members, and operands
depending on earlier replies remain separate gates. No nested parallel scheduler,
durable group recovery, automatic GUI binding or complete runtime extraction is claimed.

### Complete Atomic Group Preparation

`prepared_call_evaluation::prepare_atomic_group_in_scope` joins the original flat
Seq/All IR to complete prepared native members, without a host-owned preparation
loop. `PreparedGroupEvaluationLimits` supplies one aggregate physical IR budget:
the group root counts once, and every cold member subtree is shifted by one depth.
The borrowed value prefix has separate bounded admission under the same pure
limits, not a snapshot. Shared closed-name/arity checks run before native lookup.

All member physical gates precede all native catalog selection. Every cold call
then passes exact version, grants and named signatures, with one uniform original
schema row per member. Different members may use different original operations;
equal result tags do not join different rows within one member. All once-ordered
native member admissions precede fuel, projection or value cloning. The mandatory
host callback corroborates each original branch/declaration and current policy
against its original schema; it must also bound its own work and native payloads.
These are observations, not reusable dispatch grants or a replacement for whole
cold type inference, which remains a compiler/host responsibility.

Private atomic observations reuse the same catalog selector and synchronous
executor as `prepare_atomic_call_in_scope`. One separate native group reservation
and one member reservation precede selected execution; each actual call root is
charged once. The original `Fuel` and still-live `ScopeFrame` are borrowed through
all preparation. There is no argument replay, prefix copying, helper-local capture,
or second evaluator. There is no native Clone/Debug/serde/Send requirement. Only one complete
move-only `PreparedNativeGroup` escapes; failures/unwind release prepared prefixes
and temporary locals without partial output, retries, refunds or rollback.

Tests cover two distinct original rows, native admissions before projection,
unknown cold operations and mixed-row branches, native refusal/unwind, inclusive
whole-forest limits and all fuel exhaustion positions. The 64-member direct-call
forest needs exactly 193 physical nodes at depth 2 and consumes `400 -> 143`.
Parsed helper programs use this bridge through both original Seq/All protocols,
authentic reply re-entry and enclosing return 83 on `300 -> 260 -> 244`.
Legacy single-call preparation and legacy direct-member host costs remain unchanged.
Preparation does not invoke effects: fresh publication/invocation authority,
authentic replies, scheduling and cancellation delivery stay host-owned. Native
callbacks, result-view cloning and Drop remain trusted and non-preemptible. This
does not add result-dependent Seq operands, durable recovery, product VM migration
or complete independent VM/repository extraction.

### Headless GUI Host Vertical

`crates/leselang-hir/tests/prepared_gui_host.rs` is a second, unrelated host schema
for the prepared-helper pipeline. This is a headless GUI protocol model, not an
Avalonia/native-window adapter or an automatic framework binding. Its native
keys, fields, declarations and input envelopes need no Clone/Debug/serde/Send.
Only host-owned result views and type observations opt into cloning.

`label.set_text(target, text)` returns a string acknowledgement;
`widget.set_visible(target, visible)` returns a boolean acknowledgement. Parsed
forward helpers prepare these heterogeneous operations through the existing
compiler and `prepare_atomic_group_in_scope`, without a second evaluator or
product operation DTO. Seq publishes one outstanding member; All accepts replies
out of order and returns the original declaration order. Authenticated group
assembly then restores both enclosing scopes and returns `"ready.!"`, using one
meter with exact `1000 -> 948 -> 925` accounting. Preparing, waiting and accepting
replies do not invoke effects, project fields or execute the caller.

The adapter rechecks current widget generation and capability leases at
preparation, publication, actual invocation, acceptance and re-entry. Wrong
identity, foreign schema, wrong scalar type, stale generation and oversized
text preserve the original rejected input buffer. Reordered enclosing rows cannot
enter the caller. Cancellation or revocation stops the next Seq publication;
revoked already-prepared All data cannot authorize an invocation. Every fuel
exhaustion position before dispatch and during enclosing re-entry is terminal,
releases native acknowledgement views and never retries a mutation. Already
applied widget changes are not rolled back. Native callbacks, captures, field
copying and cleanup remain trusted, bounded by the adapter and non-preemptible.

This closes a second-schema in-memory prepared-group acceptance proof, not product
VM migration, durable recovery or complete independent VM/repository extraction.

### Shared IR Execution Isolation

The headless GUI adapter reuses one compiled tree, catalog and widget across
multiple **interleaved executions**. Each execution has a fresh local identity
from a `LocalExecutionScope` lease and stable borrowed member/aggregate reply
domains. Generation is still
checked as live widget authority, but generation and member index alone are not
an execution identity. The fixture admits its single prepared group only once
per execution context, even after completion or failed correlation. A new run
needs a new context, not a new compilation. Context allocation and the number of
simultaneous contexts are host-owned resource limits, outside the language meter.

Dispatch retains the original execution/member identity; acknowledgements retain
the same provenance. Member domains check it along with the original result
schema. Aggregate domains check the execution, original group, row order and
original member-domain addresses. Thus a correct outer delivery label cannot
turn a foreign acknowledgement or mixed aggregate into a local result. Rejection
returns the original input buffer and leaves the pending slot available for its
real reply. Repeat members using the same schema cannot replay an earlier
iteration's receipt. Neither rejection nor acceptance resumes the caller.

The proofs interleave two scoped Seq executions with different inputs, exercise
two All barriers with reverse-order replies, cross-deliver both member and whole
group replies, and attempt to reuse an execution context. Each caller retains
its own captures, fuel and output. Cancellation is delivered explicitly to the
core session, native collection state and execution scope; already-issued
native work rechecks its lease before mutation. Cancelling one execution does
not revoke the shared widget's grants or cancel its neighbour. Already applied
changes remain applied, and sharing a widget does not make mutations serializable
or transactional.

These are single-threaded host-adapter proofs using the unchanged shared kernels,
not a cross-thread scheduler or a wire authentication protocol. The weak `Rc` identity
is process-local, intentionally non-serializable and not a durable receipt.
External delivery authentication, aggregate quotas, native callback cost and
actual GUI-thread routing remain adapter responsibilities. This does not certify
the Avalonia adapter, concurrent GUI access, durable recovery or full VM extraction.

### Guarded Native Execution Lifetime

`leselang-runtime-core::{LocalExecutionScope, LocalExecutionLease, ScopedLocalExecution}`
adds an opt-in local ownership fence, also available through `leselang_vm::native::core`.
The owner is unique; leases may be cloned but are weak and cannot extend owner
liveness. `close()` is idempotent and irreversible. Distinct scopes remain distinct
even while old closed leases survive; there is no global ID allocator.

The GUI adapter now uses the shared lease for its execution identity, original
reply domains, publication/invocation and re-entry checks. Bind the existing
session into `scope.bind(session)` after constructing the host's lease-based
context. Poll/accept/cancel continue through `value_mut()` on the **same** session.
Wrapper Drop first closes the scope and then drops the native value, including
on unwind. Thus escaped requests do not stay authorized because an error path
forgot a manual boolean update. Merely declaring an owner beside a session does
not guarantee that cleanup order; binding owns both in the required order.

Close explicitly on terminal completion/failure or before cancellation cleanup;
the generic wrapper deliberately does not infer another type's terminal state.
If an unwind is caught while retaining the wrapper, close it explicitly after
the uncertain outcome; only actual wrapper destruction performs automatic closure.
An external collector still needs explicit cancellation and disposal. Late member
replies are rejected with their original buffer intact; already accepted replies
cannot restore captures or project fields after closure. `into_inner()` closes
before returning the still-owned value. Mutable access does not guard arbitrary
payload replacement: close first if such host cleanup requires revocation.

Core tests exercise borrowed move-only values, cleanup ordering, normal and
cleanup unwind, original-buffer extraction and independent owners. The GUI proof
exercises Seq/All owner abandonment, unwind, escaped dispatch, late receipts and
accepted re-entry, then runs the same compiled tree in a fresh scope with unchanged
fuel. These are cooperative local checks: no thread safety, scheduling, callback
preemption, rollback, cryptographic authority or durable cancellation is implied.
Aggregate owner/lease quotas remain host-owned. Forgetting the owner or process
abort bypasses Drop. Full host parity, durable recovery and extraction stay open.

### Local Session Entry

`leselang_vm::native::LocalVm` is exactly shared HIR's
`local_effect_session::LocalStackedControlSession`; its computation-oriented alias
is `LocalStackedEffectSession`. This is an opt-in ownership wrapper around the
original `StackedControlSession`, not another evaluator or reply state machine.
The unwrapped `native::Vm` stays unchanged, including its conditional Send behavior.

Create a `LocalExecutionScope`, install its lease in the host's request identities,
live policy and original result domains, and move the owner into
`LocalVm::new(scope, &ir, bindings, fuel, limits)`. Use the usual `poll`, `try_accept`,
`cancel`, `status` and `fuel_remaining`; `lease()` returns a non-owning observer.
There is no mutable raw-session escape or automatic native dispatch.

The wrapper mirrors **actual session status**, not an error-code string or every
`Err`: normal identity, authority and result-domain rejections leave the pending
scope open and return the original reply. Acceptance alone does not resume or
close it. Completion and normal execution failure close before the call returns.
Escaping poll/accept unwinds close at the call boundary even if an outer catcher
retains the wrapper; the original core remains `HostUncertain`. Repeated polls
and cancellation preserve the original terminal reason and budget. Construction
with a closed owner is inert; the first poll/accept cancels without running IR.

Explicit cancel and owner Drop revoke before retained-session cleanup, including
cleanup unwind. The call-boundary unwind fence cannot run before callback-local
or consumed-frame cleanup deeper inside the original evaluator: that native code
remains trusted and non-preemptible. There is no rollback, exactly-once dispatch,
wire authentication, automatic framework adaptation or external queue cancellation.
Hosts still check leases before effects and dispose of external collectors.
Returned values remain owned data, not a still-live execution grant.

The GUI vertical tests Seq/All completion, recoverable rejection, cold failure,
fuel exhaustion, preclosed owners, caught poll/accept unwind while retaining the
session, and escaped requests after cancel/Drop. The native facade additionally
proves borrowed move-only initial-value cleanup ordering, sticky cancellation
after cleanup unwind, and the original Vm's Send capability. Existing GUI meter
accounting stays `1000 -> 948 -> 925`. Local ownership is deliberately !Send/!Sync;
no global lock, scheduler, new quota or product dependency is added. Complete
host parity, durable recovery and repository extraction remain separate gates.

### Fenced Reply Acceptance

`LocalVm::try_accept` now passes its original scope through the existing
SessionCore to `PendingReply::try_accept_in_scope(identity, reply, authority,
&scope)`. The standalone runtime-core entry is also available to hosts owning
their own pending handles. Both paths reuse the original acceptance algorithm:
identity equality, live authority, borrowed view, type and value validation each
run at most once, in that order, against the original declaration.

The local fence is armed **after taking ownership of the pending frame** and
before invoking native validators. If any validator unwinds, the scope closes
before core-owned identity, frame and reply cleanup, even when a catcher keeps
the VM alive. The reply handle and session retain HostUncertain; late input is
returned unchanged without rerunning policy or spending fuel. No continuation
runs during acceptance. Unrelated owners stay open, and the unscoped API keeps
its existing behavior and conditional Send capability.

A normal refusal with an open scope disarms the fence, returns the exact original reply, and keeps
the original pending frame and open scope for an explicit corrected attempt.
Success likewise does not close the scope: a later poll still owns re-entry.
The core entry cancels a pending handle if its supplied scope is already closed,
before any native queries, preserving existing terminal reasons on cleanup panic.
Scope/identity association is host policy, not established by this argument;
live authentication, provenance and effect-boundary checks remain necessary.

Callback-local cleanup happens before an unwind reaches this fence. Handling or
disposing accepted data after handoff is outside it. There is no callback sandbox,
rollback, external cancellation, global deduplication or durable receipt. The
five-stage cleanup-order matrix and final/captured native facade tests run in
the source checkpoint; full host/recovery/extraction gates remain open.

#### Synchronous Revocation During Acceptance

A standalone host that retains its scope owner can close it synchronously inside
identity comparison, live authority, Borrow, type matching or value validation.
The scoped reply entry rechecks liveness after each normal callback and does not
enter later checks once closed. Even an otherwise valid reply must not transfer
an accepted continuation after that revocation. The shared type-before-value
validator still calls the original native domain; the private observation wrapper
only prevents value validation after a type callback closes the scope.
Identity comparison retains the original native inequality hook, including a
host-defined `PartialEq::ne`, rather than silently substituting `eq`.

Closure wins over an ordinary refusal returned by the same callback. The pending
handle seals Cancelled before releasing its retained identity/frame or disposing
the now-superseded native error. It returns `Closed(Cancelled)` and the original
input without formatting, cloning, refunding fuel or running the continuation.
If that cleanup unwinds, the handle remains Cancelled but cannot return the input.
A callback that itself unwinds instead of returning retains the existing
HostUncertain path. These checks do not preempt callback-local work or cleanup.

When the shared SessionCore receives a normal `Closed(Cancelled)` rejection, both
final and captured requests become Terminal Cancelled, not Awaiting with a dead
pending handle. A cleanup unwind that prevents that normal handoff still leaves
the enclosing session conservatively HostUncertain; its scope is already closed.
The matrix covers all five phases, refusal precedence, original input ownership,
cleanup unwind, preclosed owners, unchanged budgets and unrelated scope liveness.
Unscoped acceptance does not adopt this opt-in policy, and LocalVm still owns its
scope uniquely rather than exposing a revocation-capable shared lease.

#### Scoped Group Acceptance

`SequenceSession`, `ParallelSession`, `PreparedSequenceSession` and
`PreparedParallelSession` expose `try_accept_in_scope(..., &scope)` for hosts
that retain an explicit `LocalExecutionScope`. Both legacy and prepared entries
reuse their original private reply cores and shared PendingReply validation.
The scope is borrowed for that call, never stored in the collector or inferred
from a native identity. Unscoped acceptance and conditional Send bounds are
unchanged; no additional meter, scheduler or evaluator is introduced.

A preclosed scope cancels before index/status validation or any acceptance callback,
including a parallel barrier already in ReadyResults. Existing terminal reasons
remain unchanged. Synchronous revocation during a callback stops later checks,
returns the original input with `Closed(Cancelled)` on normal cleanup, and seals
the collector instead of restoring Awaiting. Ordinary rejection with an open
scope preserves the pending member, accepted prefix, original domains and fuel.

Cancellation drops a prepared sequence's unpublished suffix and a parallel
barrier's accepted-but-unpublished results. Sequence results and requests already
handed out remain host-owned. A callback unwind revokes before retained sibling
or suffix cleanup and conservatively leaves the collector HostUncertain. Cleanup
after the collector's Cancelled seal preserves that reason; cleanup inside the
pending handle that interrupts its return still leaves the collector HostUncertain.

These entries do not cancel an enclosing VM or external dispatch, authenticate a
scope/identity relationship, or undo native effects. LocalVm still owns its scope
privately; this is not automatic propagation from its adapter callback. A host
using an externally owned scope with an unwrapped session must explicitly cancel
that enclosing session. The headless GUI proof covers this handoff, late dispatch
refusal and an independent execution finishing with unchanged fuel. It is not an
Avalonia-window test or completion of the durable-VM extraction gate.

#### Scoped Group Polling

All four collectors also expose `poll_in_scope(..., &scope)`. Use the same explicit
scope for publication, reply acceptance and result handoff; neither method stores
or authenticates a scope/identity relationship. A preclosed owner cancels Ready,
Awaiting and ReadyResults without native preparation/admission, extra fuel or
output. Already terminal reasons remain sticky. In particular, an All batch whose
replies have arrived but have not been handed off is still cancellable.

Sequence publication rechecks after original native preparation or prepared-row
admission. All publication rechecks after each preparation and each original
identity comparison. Synchronous revocation stops later callbacks and prevents
any partial batch handoff. The collector seals Cancelled before disposing the
newly returned member, superseded error or retained prefix/suffix. No request is
returned, and work already charged is not refunded.

An ordinary preparation/admission refusal or exhausted meter remains Failed,
preserves the original returned native error and revokes before retained cleanup.
Escaping native callback unwind remains HostUncertain. A cleanup unwind before
the parallel failure handoff also retains its conservative HostUncertain reason;
cleanup after a Cancelled seal cannot change that reason. Callback-local data
consumed before returning to the library remains outside these fences.

Successful scoped and unscoped polling have exact fuel parity, original borrowed
rows/domains and declaration-ordered All results. No additional state machine or
meter is introduced. Start/preparation/correlation before these calls remain
host-owned: PreparedParallelSession::start already returns requests, and a later
scoped poll cannot undo that publication. Recheck native liveness at dispatch and
explicitly cancel an enclosing unwrapped session. The headless GUI proof covers
zero/partial/full replies, no successor or completed-barrier delivery after close,
once-only retained cleanup, and an unaffected sibling finishing at the same fuel.

### Fenced Reentry Cleanup

`LocalVm::poll` carries its existing owner into the original SessionCore and
stacked re-entry implementation. A private optional fence is shared with the
local call boundary; there is no additional public evaluator, session state,
native error interpretation or fuel meter. The unscoped APIs pass no owner and
retain their existing behavior and conditional Send capability.

On terminal evaluation/refusal or escaping unwind, initial/restored binding
prefixes and still-retained return frames, unused replies and identities observe
revocation before their cleanup at these boundaries. Invalid restored prefixes
close before their values are disposed. Final projection failure, including an
unbounded scalar result, closes before retained identity cleanup. A failed
request-correlation hook closes before dropping the prepared successor's retained
continuation, both on the initial request and after accepted re-entry.

Successful re-entry and successor publication keep the lease live. A scalar
fault handled by the language's `recover` is not a terminal slice failure and
does not revoke the execution. Native facade tests compare each suspension and
final fuel against the unscoped VM on the same recovered-computation program.
No already-settled operation is repeated and no budget is refunded. As before,
a failed poll becomes Failed only when its cleanup returns normally; cleanup
unwind keeps HostUncertain rather than inventing a completed handoff.
If retained identity cleanup panics after producing a value, the fence closes
before that value's disposal instead of publishing it to the caller.

The fence covers owners still held at these boundaries, not values already
consumed by native callbacks or temporaries cleaned inside the evaluator. A
successful native destructor that subsequently panics cannot be preempted;
callback/Drop-local cleanup remains trusted and a second panic can abort. Native
effects are not rolled back, external collectors are not automatically cancelled,
and returned data is not a durable receipt. The independent source tests include
nested cold/restore/projection failures, invalid prefixes, initial/successor
correlation refusal, cleanup panic and normal recovery/fuel parity. Complete
host/recovery/extraction gates remain open.

### Fenced Native Host Work

`LocalVm::with_host_work(input, work)` protects the adapter-owned interval between
dispatch publication and reply acceptance. It uses the **same SessionCore pending
state**, not a new queue, scheduler, dispatcher or interpreter. Only Awaiting
enters `work(input)`. Initial/accepted Ready and every terminal state return the
original move-only input as `Err(input)`, without calling work, spending fuel or
changing the existing terminal reason. A preclosed owner is cancelled first.
The host can inspect `status()` after refusal; this is not a retry hint.
Use this entry for normal outputs that should retain the pending session. If a
normal native error must terminate it, use classified outcomes below instead of
simply propagating that error while leaving a retained VM live.

```rust,ignore
let reply = match vm.with_host_work(request, |request| host.invoke(request)) {
    Ok(result) => result?, // Normal native errors are not inferred to be terminal.
    Err(original) => return host.dispose_unstarted(original, vm.status()),
};
vm.try_accept(&identity, reply, &host)?;
// A later explicit poll performs re-entry with the original meter.
```

The outer Result says whether the callback ran, not whether its operation
succeeded. All normal outputs, including borrowed values and native `Err`, stay
verbatim; the original pending state is restored without accepting a reply,
executing a continuation or resetting fuel. Repeated host calls are permitted
while Awaiting: hosts authenticate request/execution association and prevent
duplicate mutation themselves. Merely passing an arbitrary input is not proof
that it belongs to this execution or was ever published by this VM.

Before entry, the original core seals HostUncertain and retains its pending state
locally. A normal return restores that state. An escaping callback unwind closes
the local lease **before retained pending identities/frames are cleaned up**;
the core stays HostUncertain even if an outer catcher retains the VM. Further
host work, acceptance or polling cannot retry that uncertain operation. Applied
effects remain applied; native escaped requests still check their original lease.
An unrelated live execution is not revoked.

This is an explicit synchronous call boundary, not a native sandbox. The callback
must check live identity/schema/grants/lease and bound native work. Callback-local
cleanup still precedes the unwind fence; unused rejected closure captures also
have ordinary Rust destruction semantics. Put fallible output processing or
disposal inside the callback if it must be fenced: the boundary cannot observe
later failures after output handoff, or a panic caught entirely inside native
code. No callback preemption, external collector cancellation, rollback, crash
recovery or exactly-once effect guarantee is added. Drop panic during unwind may
abort, and process abort/forget bypass Rust lifetime cleanup.

Native facade tests cover final and captured pending identity cleanup ordering,
borrowed/move-only inputs and outputs, and preclosed owners. Seq/All GUI tests
cover ordinary refusal, successful re-entry with unchanged `1000 -> 948 -> 925`
fuel, panic before/after mutation, revoked escaped work and non-awaiting refusal.
These same tests run unchanged in the independent source consumer; they are
headless adapter proofs, not an Avalonia integration or full extraction gate.

### Classified Native Outcomes

`LocalVm::with_host_outcome(input, work)` shares the existing host-work lifecycle
but requires the adapter to return `HostWorkOutcome<Output>`. The enum is available
from `leselang_vm::native`, shared HIR's `effect_session`, and the local entry
module. It adds no product DTO, scheduler, interpreter or wire format.

| Explicit Outcome | Original Session State | Native Output |
| --- | --- | --- |
| `Pending(output)` | Remains Awaiting with its original frames and budget | Returned unchanged |
| `Cancelled(output)` | Terminal Cancelled; local lease revoked before retained cleanup | Returned unchanged |
| `Failed(output)` | Terminal Failed; local lease revoked before retained cleanup | Returned unchanged |
| `Uncertain(output)` | Terminal HostUncertain; local lease revoked before retained cleanup | Returned unchanged |

The host chooses the classification from its actual adapter protocol. A normal
error can mean a recoverable refusal, an unrecoverable failure, or an uncertain
publication/result: neither an error code nor Rust `Err` determines the choice.
`Failed` does not certify that no effect occurred. `Uncertain` does not require
raising a Rust panic. `Cancelled` records a host stop decision, not proof that
native effects stopped or were rolled back. No terminal variant can accept a late reply, run the
tail or perform another host call through this execution; cancellation and
repeated polling preserve the selected reason. If pending native cleanup unwinds,
that reason is already sealed and the lease already closed.

The outer `Result<Output, Input>` still records entry, **not operation success**:
terminal outcomes return `Ok(output)` too. The callback is never entered outside
Awaiting; the original input is returned instead. There is no native Clone,
Debug, Send, serde or 'static bound. Even enum Debug prints only its variant tag,
not the payload. Normal returns preserve borrowed data and original buffer
ownership. Cleanup panic can prevent output delivery, in which case ordinary
Rust cleanup drops the output while the execution remains closed.

`with_host_work` remains compatible by wrapping every normal return as Pending;
both entries use the same original SessionCore transition and meter. Only the
interpreter can complete the language computation after reply acceptance. The
adapter must still authenticate requests/results, bound native work, prevent
duplicate dispatch and explicitly cancel external collectors. This classification
is local policy, not a durable receipt, transaction rollback or exactly-once proof.

Final/captured native tests exercise returned error identity, borrowed move-only
payloads and retained-session cleanup panic for all three terminal classes. Seq/All
GUI tests exercise errors before and after mutation, escaped-request revocation,
unchanged budgets and an independent sibling that still completes normally.

#### Cancellation Handoff

A GUI adapter may observe cancellation while it owns a group collector inside
the host-work callback. Return `HostWorkOutcome::Cancelled(collector)` rather
than misclassifying the stop as Failed, or returning Pending and relying on a
later VM cancellation. The original VM seals Cancelled and revokes its lease
before returning that original collector. The callback cannot borrow the same
VM again to cancel it while host work is active.

The host must then explicitly cancel/drain its collector and dispose queued work
and any already accepted rows. Passing collectors through the output avoids
their normal disposal inside the callback, which occurs before this boundary.
Callback-local unwind remains outside that guarantee. A partial Seq prefix or
an All barrier with accepted members is not an external rollback transaction;
an All ReadyResults barrier has not published a result just because every native
operation has finished. Accepted rows handed off by Seq are separately host-owned.
An already-completed collector keeps its own Completed reason when cancelled;
the enclosing computation can still be Cancelled without evaluating its tail.

The headless GUI proof cancels Seq/All after zero, one and two native completions.
It checks collector ownership until explicit cleanup, releases retained receipts,
rejects escaped work and late group replies, preserves the enclosing budget and
leaves a sibling execution live. Neither classification nor cleanup replays or
undoes the completed native prefix. This remains a cooperative local adapter
protocol, not automatic queue cancellation or an actual Avalonia integration.
The same proofs run in the outside consumer; full host/recovery/extraction gates
remain open.

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

### Shared Native Source Call Completion

`source_call::LoweredSourceCall::finish_call` consumes prepared native operands
into the original generic `Computation::Call`. `SourceCallFinishLimits` supplies
**current whole-call limits**: one root plus the complete cold operand forest,
with inclusive node/depth ceilings of 16384/64 and argument/parameter ceiling 64.
The selected schema's optional parameters count even when omitted. Zero nodes
rejects before callbacks; a parameterless call permits one node, zero depth and
zero arguments. Earlier source budgets are not reinterpreted as current IR policy.

The complete current physical checks precede **once-only native operation mapping**.
Mapping receives the original borrowed schema and returns an owned native operation
and result observation. The core, not the mapper, constructs the call from the
exact original operands in declaration order; language names materialize once,
optional omission stays absent, and native nodes/buffers are not cloned or rebuilt.
There are no native Clone/Debug/serde/Send bounds, operation defaults or new codecs.
The existing `into_arguments` remains a low-level handoff, not final admission.

**Mandatory once-only whole-call admission** sees the constructed call, original
schema and exact result observation. The host must independently confirm native
operation/schema/result identity, complete cold lexical typing, canonical form,
current version/grants and its remaining native policy. Cached scalar observations
and a successful structural walk are not type certificates or dispatch authority.
Schema borrowing does not freeze interior domain state or live authority. Native
callbacks, queries, equality, allocation and destructors remain trusted bounded
adapter work, not an in-process sandbox or an external-side-effect transaction.

Only admitted owned call/result pairs return. Mapping/admission failure or unwind
releases the original operands and any constructed native outputs; no partial call,
retry, dispatch, fuel charge/refund or rollback is added. Closed diagnostics retain
native errors for explicit matching without formatting private payloads/source chains.
The reference computed-call compiler delegates construction to this finisher and
uses its exact current private type scope, preserving 64-member groups and existing
diagnostics/source accounting. Final reference authorization remains separate.

Move-only native tests prove original schema/buffer identity, current narrowed
forest limits, optional omissions, failure/unwind cleanup and private errors.
Unrelated numeric-device and text-panel calls pass fresh native admission and
prepare actual values through the existing shared evaluator. Forged scalar facts,
foreign same-shaped rows/results and changed domains/version/grants are rejected
by explicit native admission. This is **native call completion, not complete generic
source compilation**, effect execution, durable suspension or automatic authorization.

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

### Shared Helper Body Source Preparation

`helper_body_source::lower_helper_body` prepares one original borrowed function
declaration through explicit native recursion and shared body admission. It is
**helper source preparation, not a complete independent compiler**. Declaration
selection, entry/builtin policy, dependency planning and registry lifecycle remain
caller-owned; native lowering and opaque type/schema rules remain host policy.

**The core owns the exact scalar signature**. All source/output/cost ceilings,
the declaration name, six closed scalar tokens, declaration-order parameters and
parameter-prefix capacity pass before callbacks. The lowerer receives the original
Function and borrowed parameter names, not an invented or normalized signature.
Its output contains an expression, native return observation and optional scalar
category, but cannot replace the parameters. Only the core materializes bounded
declaration names into the admitted template signature.

**Whole cold source checks precede native lowering**. The public entry bounds all
physical AST nodes, depth, nested call arity, identifier names and literal text,
including unused branches and zero-limit controls. Inclusive ceilings are 16,384
nodes, depth 64, 64 call arguments, 1,024 bindings and eight scalar parameters.
Source shape and generated-template/cost policies are separate; explicit zero
policies do not silently inherit defaults. Aggregate ingress bytes, parser/span
authenticity, operation authorization and native callback work remain host-owned.

**One original source counter crosses body recursion and admission**. The lowerer
runs once with the original mutable counter and must recursively charge that same
counter, seed exactly the declared scalar prefix, and reject every cold capture
or denied operation. A successful over-limit callback fails before admission.
There is no copied meter, precharge estimate, hidden refund or retry. Counter bounds
are not proof that a trusted native lowerer faithfully traversed the source.

The resulting owned IR goes through `LoweredHelperBody::prepare`: whole physical
and lexical output checks, independent pure-return inference, **one native
corroboration**, then original native source-cost measurement. Native validation
must corroborate actual return metadata/category, canonical bytes and all cold
opaque effects/schemas. Original native slots, Boxes and result buffers move
unchanged without native Clone/Debug/serde/Send requirements. No body copy, partial
cache entry, execution grant, fuel, dispatch or suspension state is created.
Errors retain native payloads for matching, never formatting or source chains.
Failure/unwind releases owned output but does not undo earlier native work or
source charges; input allocation, interior mutation, Drop and unwind remain trusted.

The product helper compiler now delegates source preparation to this shared
pipeline. Its private preflighted entry relies on the existing declaration/parser
boundary and retains recursive diagnostic priority rather than rerunning public
cold lexical checks ahead of older errors. It still shares all header/counter and
output admission gates, exact parameter seeding, canonical return corroboration
and body+cost storage. Ready helpers lower before the next dependency observation.
Existing diagnostics, spans, wire formats and capability checks remain unchanged.

An unrelated scalar host proves declaration -> body preparation -> instance
completion -> evaluation: body accounting advances 10 -> 13, a distinct caller's
already charged call/operand counter advances 2 -> 6, the result is 42 and fuel
advances 100 -> 95 with the caller prefix restored. Move-only native proofs cover
original declaration/parameter/Box/result identity, once callbacks, private errors,
unwind and no source-budget refund. Generic recursive leaf lowering, opaque flow
typing, native storage lifecycle and durable suspension ownership remain open.

### Shared Helper Registry Assembly

`helper_registry::assemble_helper_registry` assembles all admitted helpers around
an **owned caller-supplied registry state**. It reuses the original borrowed
declaration observation and its designated entry name, which need not be `main`.
The entry is neither prepared nor registered here. Concrete storage, lookup,
generated-name reservations, source fidelity and complete native policy remain
explicit host choices; no second registry implementation or global cache is made.

**Prepare and register one ready helper before advancing**. Dependency/template/
source-cost safety ceilings pass before callbacks. Complete physical helper source
and entry-call checks precede the first native preparation. The dependency cursor
selects the lexically smallest ready declaration; each current scalar prefix then
passes parameter and binding limits before once preparation. Only after successful
registration does the cursor advance. A ready helper's native error keeps priority
over a later dependency cycle. No eager order collection, pruning of unused
helpers, unknown-call authorization, skip/resume or default policy is installed.

**Registration requires the exact original declaration signature**. Prepared
body parameters must match names, types and order, not merely arity or a reported
return type. Current complete physical/lexical template bounds and cached source
cost limits pass before the storage callback, with depth checked before nodes.
Registration receives the original borrowed Function and the original owned
HelperBody, with body and cost still paired. Native Boxes, signature/result buffers
and state allocations are not implicitly copied or remapped; no native/state
Clone/Debug/serde/Send requirements are imposed.

**Return owned state only after complete assembly**. A preparation, signature,
shape, cost, registration or late-cycle failure returns no partial registry state.
Error/unwind releases core-owned state and unregistered output without retries,
native payload formatting/source chains or hidden reservation refunds. This is
**not transactional rollback of external storage or native side effects**. State
may contain trusted interior mutability or caller-created aliases; callbacks and
Drop can mutate external state. Native allocation, work, mutation and unwind remain
host responsibilities. No execution grants, fuel, dispatch or durable journal are
created, and callbacks must not publish execution authority.

Preparation must freshly corroborate faithful source accounting and every cold
native type/schema/canonical policy. Stored result/cost observations are not live
certificates; materialized copies and dispatch still require fresh validation.
Existing state entries acquire no implicit admission from this assembly API.

The product compiler now delegates dependency-ordered registry assembly to this
shared entry. Its original local template storage and name allocator move through
both callbacks unchanged. Shared helper-body source preparation and native return/
canonical checks remain in preparation; actual cache insertion is the explicit
registration callback. Only a complete returned state reaches main lowering.
Existing diagnostics, spans, wire formats and cold capability policy are retained.

An unrelated scalar host proves declarations -> registry assembly -> helper-body
source preparation -> instance completion -> entry evaluation. `base`, `plus`,
then caller-owned `boot` record exact source charges 1, 4, 4; the result is 42,
fuel advances 100 -> 97 and the evaluation prefix is restored. Move-only native
proofs cover original body/state buffers, exact signatures, current limits, once
ordering, private errors, unwind and late-cycle cleanup without refunds. This is
**shared registry orchestration, not complete generic recursive compilation**;
generic leaf lowering, opaque flow typing, native storage lifecycle and durable
suspension ownership remain open.

### Shared Program Assembly

`program_source::assemble_program` connects an admitted declaration forest to
the shared helper registry and an **explicit once-only native entry lowerer**.
The original declaration observation selects the parameterless entry; no name,
AST, registry table or native output is replaced or implicitly copied. Entry names
need not be `main`. All helpers, including unused ones, prepare/register in the
existing lazy ready order before entry lowering starts.

**Public entry source preflight precedes native helper work**. Entry source
node/depth/argument ceilings are checked before any callback, then the complete
cold entry body is checked for physical bounds, call/reference/argument names and
literal text. The helper registry applies its own current ceilings and gates.
This preflight does not validate native schemas, unknown-call authority, cold
typing or the authenticity of source spans; keep parser/decoded-AST admission and
bounded ingress in front of this API. Entry source limits have no output or native
work budget meaning. Zero entry nodes denies the root; zero depth permits leaves.

**Complete registry -> entry lowering -> mandatory final admission**. A helper
failure or late dependency cycle prevents entry compilation. The entry lowerer
runs exactly once with the original Function and the complete owned state; final
admission runs once with that same entry, the original borrowed output and that
state. No state/output is handed back until final admission succeeds. Native
output is opaque: hooks must independently bound generated output and native
graphs before growth, corroborate complete cold types/results/costs, canonical
bytes, current schemas/versions/grants and any reported capability list. Neither
the declaration observation nor successful assembly is execution authority.

The returned `ProgramSource` retains the **original entry, native output and
owned state together** with read-only observations and an explicit `into_parts`
handoff. Native buffers are move-only without Clone/Debug/serde/Send requirements;
Debug prints only the type name. **State may move between assembly stages**:
this is logical owned-state continuity, not a stable-address or pinning guarantee.
Self-referential host state needs a caller-owned pinned allocation.

Entry/native admission failure or unwind drops core-owned output and state
without partial program return, retries, private-error formatting/source chains
or hidden refunds. This is **not transactional rollback of external storage or
native side effects**; caller aliases, interior mutation, allocation/work and
Drop/unwind remain trusted. Hooks must not dispatch or publish runtime authority.
Fresh live checks remain mandatory before output use, dispatch and reply handling.
No new counter, fuel, global storage, scheduler, codec, journal or durable VM is
installed. This is shared program assembly, not complete generic recursive
compilation; native leaf/opaque-flow typing and durable recovery remain open.

The multi-function product compiler delegates helper/entry/final-canonical
ordering to the crate-private preflighted entry. Its parser/declaration boundary
already bounds source; this preserves ready-helper diagnostics before later entry errors while
sharing registry, once-only entry lowering and final admission. Existing result
metadata, capability lists, direct-Host unwrapping, wire and later authorization
stay unchanged. The legacy single-function compiler retains its existing path.
Only fully admitted output reaches product `HirProgram` creation in this shared path.

An unrelated scalar host composes declarations -> shared program assembly ->
helper-body preparation/instances -> entry admission -> evaluation. `base`,
`plus`, `boot` retain exact source charges 1, 4, 4; result 42 uses fuel 100 -> 97
and restores the evaluation prefix. Move-only native proofs cover exact borrowed
entry identity, original buffers, all-cold public entry gates, complete registry
ordering, mandatory fresh admission, private errors and failure/unwind cleanup.

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
nested parallel groups, complete helper/opaque-host source typing and durable restart
remain separate extraction work. Binding restoration is supplied by the following
entry, not implicitly by this sequence lifecycle.

### Shared Parallel Reply Barrier

`parallel_evaluation::ParallelEvaluation::start` enters a borrowed flat All with
2 through 64 atomic candidates. The same `SequenceEvaluationEnvironment` is
re-exported as `ParallelEvaluationEnvironment`: cold schema/type/opaque policy,
live preparation, exact identities and original result declarations use the same
protocol in both modes. No second evaluator, product operation list or scheduler
is introduced. It works with `leserpent-profile` disabled and requires no SQLite.

Whole physical/language shape and every candidate pass before native cold policy;
all cold policy passes before the one group-root fuel debit. First `poll` prepares
all members exactly once in declaration order using one owned meter, charging each
member root plus explicit native work. All original identities must be pairwise
distinct. Complete preparation and identity checks precede any request handoff.
A late preparation failure or insufficient fuel drops the complete local prefix
and closes Failed, with **no partial batch publication** or refund. Identity
equality and other native work are trusted, bounded by the host, not preempted.

The returned `Requests` batch belongs to the host. It may schedule members in any
order and on appropriate GUI/worker lanes. This is **not atomic external dispatch**:
the language cannot roll back a request already delivered by the host or prevent
a caller from dropping part of the batch. No threads, source-level async, automatic
retry, delivery lease, clock/deadline service or global deduplication are created.

`try_accept` takes both the member index and exact opaque identity. Unknown,
already accepted, early and terminal members are rejected before native hooks.
For a pending member the shared order remains identity, live authority, borrowed
reply view, type, then native value validation. Ordinary rejection returns the
unchanged input and retains the barrier. Replies can arrive out of order, but
accepted original declarations and move-only replies remain private to the session.
Waiting polls do not prepare, charge, redispatch or publish a partial result.

After all replies pass, state becomes `ReadyResults`, not Completed. Only a later
explicit `poll` hands off the complete ordered original-row/declaration/reply
batch once and closes Completed. It installs no result aliases and does not infer
an aggregate group value or restore an enclosing continuation. Cancellation can
discard accepted results even after the barrier is ready, before that handoff.
Completed means accepted reply ownership was handed off, not a durable commit or
proof of atomic external success; provenance and native result semantics remain
host policy. Mutable result data can invalidate earlier checks.

Preparation, pairwise identity comparison or any reply callback unwind seals
HostUncertain before owned prefixes and sibling results can replay. Cancellation
seals Cancelled before cleanup; repeated polls/cancellation preserve terminal
reasons. Native destructors may unwind and two cleanup panics can abort. No native
Clone/Debug/serde/Send/Sync bound is imposed; identities explicitly require equality,
and unsized borrowed domains/views are supported. Diagnostic formatting exposes
closed phases/status only, never native requests, replies or private error chains.

The parsed counter proof compiles an All with two distinct result domains,
prepares actual operands, invokes consumed requests in reverse order and accepts
replies into an ordered barrier with exact fuel. Revoked policy, stale generation
and cancellation retain the gates; preparation itself invokes neither counter.
Separate proofs cover 64-member inclusive bounds, duplicate identity, move-only
payload pointers, all acceptance-hook unwinds and partial/ready-barrier cleanup.
This is **a flat in-memory parallel reply lifecycle, not the complete independent VM**.
Nested groups, complete generic group-result projection/re-entry, group/helper-program
composition, product-adapter migration and portable durable recovery remain open.

### Shared Prepared-Operand Sequential Publication

`prepared_sequence::PreparedSequenceSession::start` consumes a complete scoped
`PreparedNativeGroup` against the exact original flat-Seq IR. Shared whole
physical/language shape and atomic candidates, original kind/slice/count and row
correspondence, then **all fresh native preparation admissions** precede every
correlation hook or additional fuel debit. All once-ordered correlation and pairwise
distinct native identity checks complete before a session can be handed off.
Refusal/exhaustion/unwind returns no session or partial request and consumes every
prepared prefix/suffix without retries or refunds. Native side effects cannot be
rolled back, and a second cleanup panic may abort.

**Correlation is eager, publication is sequential**. Arguments are already prepared
once under the live lexical scope, not evaluated from earlier sequence replies.
A correlation root unit plus one per member, with explicit native work charges,
uses **the enclosing original fuel meter**. The wrapper owns **no fuel** and creates
no second meter. Start does not dispatch or publish a request. Compared with the
legacy lazy preparation entry, this is an opt-in snapshot/precorrelation policy,
not a silent change to source or existing execution semantics.

Each explicit `poll` requires a fresh native publication hook receiving the exact
original row, retained identity, original result domain and retained request.
It checks current schema/grants/provenance before releasing **one outstanding
request**. Waiting/terminal polls run no hooks; acceptance only returns that member's
ordered original accepted row and makes its successor Ready. It does not implicitly
publish a successor, evaluate operands or aggregate a group. A publication refusal
is terminal Failed; callback unwind seals HostUncertain. The host explicitly bounds
or prepays native publication policy work and aggregate retained memory/ingress;
the no-fuel poll signature is not free or preemptible native execution.

Both entries delegate **the same private SequenceReplyCore**, including original
identity/live authority/type/value rejection, unchanged rejected owned input and
terminal-before-cleanup fencing. The prepared wrapper locally guards every retained
future request/identity during poll, acceptance and cancellation, so refusal or
unwind drops the complete suffix without rearming. Already published requests and
accepted rows are external ownership; cancellation delivery and aggregation remain
explicit host orchestration, not rollback or external exactly-once execution.
Unsized domains/views and move-only native requests need no Clone/Debug/serde/Send
bounds beyond native identity equality.

The parsed parameterized `seq` helper shares the counter host, captures and complete
`assemble_group_reply` path with the All proof. It prepares exact `41`/`40` operands,
publishes member 0, waits for its validated reply, then publishes member 1. Shared
whole-group assembly and an explicit enclosing poll restore two original scopes
and produce `83` on **100 -> 78 -> 62**. Cancellation before publication executes
no call; cancellation or fresh grant revocation after member 0 never invokes member
1 or the caller. Late correlation denial returns no dispatch. This is a local
in-memory native proof, not transactional external dispatch, nested group-member
scheduling, portable durable recovery, product migration or independent VM promotion.

Legacy SequenceSession signatures, lazy preparation order, owned meter handoff and
fuel costs remain unchanged. This profile deliberately reserves all correlation
work up front; it cannot consume sequence-result-dependent arguments or free native
correlation side effects on cancellation.

### Shared Prepared-Operand Parallel Reply Sessions

`prepared_parallel::PreparedParallelSession::start` consumes a complete scoped
`PreparedNativeGroup` against the exact original flat-All IR. Shared whole
physical/language shape and atomic candidate admission, original kind/slice/count
and row-index correspondence, then **all fresh native preparation admissions**
precede every correlation hook and additional fuel debit. A foreign, missing or
reordered member set cannot substitute a look-alike group.

Start borrows **the enclosing original fuel meter**, charging one correlation root
unit and one unit per member before its once-ordered correlation callback. Native
callbacks consume the already prepared operands, preserve original domains and
bound/charge their own live correlation and dispatch work. **No argument is
reevaluated, no operand is copied, and no second meter is created**. Earlier
scoped preparation costs remain separate, explicit and nonrefundable.

Only after all correlation and distinct identity checks does start return the
complete native request batch and its reply barrier. Failure, exhaustion or unwind
consumes prefix/suffix preparations without partial publication or retry; native
side effects/Drop cannot be rolled back or preempted, and a second cleanup panic
may abort. Complete local publication is **not atomic external dispatch**.

Both prepared and legacy parallel sessions delegate **the same private
ParallelReplyCore**: original index/domain and current authority checks, unchanged
rejected owned inputs, out-of-order acceptance, the complete all-success barrier,
once-only ordered original-row output and terminal-before-cleanup fencing.
The prepared wrapper owns **no fuel**. Waiting/terminal polls and acceptance
neither execute successors nor debit or redeem the enclosing meter. Cancellation
also closes accepted-but-unpublished results; acceptance unwind seals HostUncertain.
Native domains may be unsized and replies may borrow an unsized view; no native
Clone/Debug/serde/Send requirement is added beyond identity equality.

The parsed parameterized counter helper now uses this entry to hand off exact
`41`/`40` operands, accept member 1 before member 0, and produce ordered accepted
rows. An explicit enclosing poll uses shared `assemble_group_reply` with fresh
whole-group policy before one native reply projection and two restored scopes;
the result is `83` on **100 -> 78 -> 62**, including two prepaid native invocation
units and explicit correlation/assembly costs. Incorrect member identity and
reordered outer rows preserve the original input. Cancelling either local barrier
or enclosing session, revocation after acceptance and late correlation denial
cannot run caller successors. Cancellation delivery between those two owners
remains explicit host orchestration, not automatic external cancellation.

Legacy ParallelSession signatures, preparation order, owned-meter handoff and
fuel costs remain unchanged. Cold types, provenance, live grants, aggregate native
memory/work/capture limits, authentic receipts and actual dispatch remain adapter
policy. This is a local in-memory synchronous counter proof, not new scheduling,
durable recovery, nested group-member support, product migration or independent
VM maturity promotion.

### Shared Complete Group Reply Assembly

`group_reply::assemble_group_reply` consumes the complete ordered accepted rows
against their original borrowed flat Seq/All IR. Whole physical/language shape,
every atomic candidate, inclusive arity, complete count, declaration order and
exact original row identity pass before fuel or native admission. It never sorts,
fills a missing member, redirects a row or publishes a partial aggregate. A foreign
IR, truncated/reordered result or duplicate position fails these cold gates.

After these gates, one assembly root plus one unit per member is charged atomically.
Mandatory fresh whole-group admission receives the private move-only `GroupReply`
and the same meter. The host checks expected owner/generation, exact declaration
correspondence, current schema/type/value bounds and live policy. Original row
pointer agreement alone does not authenticate a declaration, reply or execution
generation. Mixed executions of the same IR must be rejected by this native gate.
Failure/unwind consumes the owned inputs, with no retry, refund or partial output;
trusted native work/destructors are not preempted and a second cleanup panic can abort.

`GroupReply::lookup` reuses closed bounded member-name lookup and mandatory fresh
selected-row admission. Unknown or policy-hidden names return no member; invalid
shape/name/limits and native errors remain errors. It returns the original borrowed
accepted row without native payload copying, coercion or fallback operation. The
private object exposes read-only borrows; `into_members` deliberately consumes its
complete-group representation. Interior native state can still change after checks.
There are no native Clone/serde/Send/Sync requirements or implicit wire format.

Seq/All sessions expose `into_completed_fuel`: only Completed transfers the original
owned meter, without refill or refund. Ready, waiting, ReadyResults, Failed, Cancelled
and HostUncertain return the unchanged session. All must explicitly hand off Results
before this transfer; Seq must have accepted its last member. Completion is ownership
handoff, not durable commit or a statement of external success.

The aggregate is data, not authority or a replacement continuation. The owning host
must still correlate it with the original `EffectContinuation` through `PendingReply`,
fresh live policy and complete result-domain validation before `resume_accepted_effects`.
Restoration/projection check live policy again; the aggregate cannot choose another
binding name/body or revive cancelled work. No automatic dispatch or retry is added.

The native-only `group_reply_reentry` proof now compiles the complete parsed
Bind/All/member/field/Choose expression through `compile_catalog_group_source`,
without manually constructing an outer IR binding or supplying its group type.
It dispatches two move-only native counter record replies in reverse order,
assembles the group, accepts the original continuation and computes 48. One original
meter moves through all stages: outer Bind/Group 2, batch root/member lifecycle 3,
assembly 3, explicit native binding projection 1 and body evaluation 14 units leave
77 of 100. Both lifecycle roots are explicit; no copied remaining-balance meter is
created. Wrong outer identity preserves input; revocation before acceptance or
re-entry prevents restoration, projection and extra calls. Wrong operation/member
and stale record views cannot bypass closed lookup.

This is a closed flat-group re-entry vertical, not a universal group compiler or
complete independent VM. Nested group results, generic group/helper source typing,
product-adapter migration and portable durable recovery remain open. The shared
assembly checks ownership/shape; native aggregate semantics remain adapter-owned.

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

### Shared Bounded Return Frames

`effect_stack::evaluate_stacked_effects_in_scope` explicitly opts into **nested binding
returns** over the same synchronous effect walker. `resume_accepted_stacked_effects`
consumes an accepted `EffectReturnStack` through the existing native re-entry hooks.
The legacy single-frame evaluator, re-entry API and `ControlSession` retain their
original nested-suspension rejection, wire shape, fuel and diagnostic behavior.
There is no second evaluator, background worker or automatic product migration.

The private stack retains the **original borrowed binding names/bodies**, ordered
innermost to outermost, and one opaque native capture per pending return. A helper
can suspend repeatedly, finish its own body and return a scalar/structured value to
the awaiting caller. Capture hooks snapshot only their exact live lexical prefixes;
the core does not clone native IR or invent a capture representation. Native result
views retain the existing Clone contract for explicit reads, not incoming payloads.

`EffectStackLimits` adds an inclusive **0 through 64 pending-frame ceiling** over the
unchanged `EffectEvaluationLimits`. Active lexical bindings and pending return sites
have separate bounds. Zero frames permits pure completion or a final request, not
a suspended binding. Each new frame passes its quota before that frame's capture
hook; earlier preparation/captures may have occurred and are not rolled back. On
re-entry, **new and already pending frames share one quota**: older outer frames
reduce the space available to newly suspended helpers. Lowering policy below the
retained stack size refuses before restoration, projection or fuel debit.

The **complete pending body forest shares one physical budget**. Every cold body
and current schema passes before any native restore/projection; per-frame budget
resets cannot admit an oversized combined forest. Only the innermost frame projects
the accepted native reply once. Outer frames bind the already computed return value,
**not a fabricated native reply or result-domain cast**. A final request from an inner
body adopts the next original return site, without replaying prior source/effects.
All restoration, prefix charging, name/shadow checks and value binding reuse the
single-frame primitives with the **same original fuel meter**, without refunds.
These entries borrow host-held fuel; hosts must retain that root meter between
replies. They do not enforce continuity against a deliberately substituted meter.

Native restoration must validate each capture's **own provenance and current view**
independently of the latest accepted leaf's identity/result domain. That domain is
not the type of an outer logical return value. Adapters own compatible result views,
framework affinity, live policy and stable-snapshot versus live-reference semantics.
They must bound **aggregate capture bytes/work across all frames**, not only each
snapshot: the frame ceiling is not a total native-memory or callback CPU ceiling.

`PendingReply` retains its original identity/authority/type/value acceptance and
unchanged rejected-input ownership. Cancellation before re-entry consumes all return
frames without restoration. Error/unwind consumes owned frames without partial
continuation output, dispatch, retry or refill; earlier native work is not rolled back.
Debug/errors expose only closed phases/frame counts, not names, captures or native
payloads. Native callbacks/Drop remain trusted; a second cleanup panic can abort.

The unrelated move-only/unsized scalar host crosses two requests, preserves caller
and helper scopes, writes 42 and returns 53. The meter follows **100 -> 89 -> 79 -> 69**;
only two native replies project, while three logical frames restore. The parsed
helper-internal All proof keeps the original out-of-order reply barrier and complete
group admission, then returns 49 to the outer caller with remaining fuel 69. Group
reply projection occurs once, with two frame restorations and no extra native calls.
An additional parsed helper returns the original group view through an outer frame,
then projects members there; it still projects the native reply once and leaves fuel 71.
Quota/forest denial, wrong identity, late revocation, cancellation, bad prefixes and
native failure/unwind tests verify cleanup without replay or per-frame resets.

This closes bounded **in-memory nested binding return composition**, not nested
group-member scheduling, recursive helpers, portable durable recovery, independent
VM acceptance or repository extraction. Hosts opt in and adapt this protocol;
existing control sessions/product VMs are not silently widened. Embedding maturity
remains developing, and durable layouts/authority remain separate extraction gates.

### Shared Borrowed Control Sessions

`effect_session::EffectSession` ties the shared effect walker, `PendingReply` and
accepted binding re-entry into **one owned-fuel in-memory control session**. It
borrows immutable native IR and owns initial variables, pending/accepted captures
and received input. It neither translates the IR nor replaces the reference VM,
journal or group scheduler. Native data needs no Clone/Debug/serde/Send/Sync bound;
result views retain the pure executor's explicit Clone contract for local reads.
Unsized original native domains and GUI-local borrowed dispatch are supported.

`new` only stores IR and moves variables/fuel; it runs no callbacks and is not
admission or an ingress bound. The first `poll` uses the shared whole physical,
prefix and all-cold native preflight before execution and fuel. Selected request
preparation is still non-dispatching. `correlate_request` must recheck live policy,
retain the exact original result domain, encode owner/generation/site identity
and bound/charge native metadata work. The core installs that pending identity
before returning the consumed dispatch payload once. Hosts actually invoke it.

**Waiting and terminal polls are inert**: no callbacks, fuel debit or repeated
request/value handoff. `try_accept` checks identity, live authority, borrowed view,
type and native value; rejection returns the exact input and keeps waiting. Early,
duplicate and terminal replies are returned unchanged without native callbacks.
**Acceptance marks Ready but never restores, prepares or projects output**. The
accepted input/frame stays owned by the session until the next explicit `poll`.
Cancellation in this window releases it without executing a successor.

A later ready `poll` reuses original-site binding restoration, or maps a final
accepted native reply through `bind_reply`. Those hooks own live policy, mutable
native views and result-view compatibility. Mapped scalar bounds precede final
output handoff; one Value marks Completed, and later polls return only terminal
metadata. Completed describes handoff, not an authentic receipt, a durable commit
or proof that the caller consumed its result. Pure tails use the same session.

**Close before callbacks and owned cleanup**. Preparation, correlation, restoration,
projection or acceptance unwind seals HostUncertain. Owned initial-prefix drops
finish before a request can become waiting; final identity drops finish before
completion. Normal execution/fuel errors close Failed. Cancellation seals Cancelled
before initial, pending or already accepted payload cleanup, preserving known
terminal reasons even when native Drop unwinds. No implicit retry, fuel refill or
refund exists. Debug/errors format only closed metadata, never native payloads.

The parsed native binding proof now drives real read/write calls through this
session without a caller-managed fuel meter or pending handle. Rejected identity
retains the actual native reply; it is accepted, restored and used to write 42,
with exact cumulative fuel. Cancelling after read acceptance never prepares or
invokes write. A distinct GUI-local host moves a text input after borrowed native
dispatch, reaches its pure tail, or cancels between acceptance and projection.
Separate tests cover move-only native identity/IR/request/capture/input, unsized
domains, pointer preservation, quotas, live revocation, every acceptance callback
unwind, cleanup panic, final bounds, exhaustion and once-only output.

This is **an in-memory borrowed control session, not a complete independent VM**.
It adds no global deduplication, receipt provenance, background scheduling,
parallel barrier, sequence result aggregation or durable restart. The host still
bounds ingress/aggregate sessions/native work and owns cancellation delivery.
Native mutation/allocation/Drop cannot be preempted or rolled back; a second panic
in cleanup can abort. Complete generic source/opaque-host typing and durable
continuation/recovery formats remain extraction gates, not implicitly completed.

### Shared Owned-Fuel Nested Control Sessions

`effect_stack_session::StackedEffectSession` (generic `StackedControlSession`)
explicitly opts into bounded nested binding returns **without caller-managed fuel
or pending handles**. It uses the **same private SessionCore lifecycle** as the
legacy single-frame `ControlSession`, not a second state machine or evaluator.
The original `EffectSessionEnvironment` supplies correlation and native re-entry;
adapters need no new callback trait or native capture representation.

```rust
let mut execution = StackedEffectSession::new(
    &program,
    initial_bindings,
    Fuel::new(100),
    EffectStackLimits { effects: effect_limits, max_frames: 4 },
);
// poll prepares/correlates a request; the host explicitly dispatches it.
// try_accept validates the original input, then only marks Ready.
// A later poll resumes the exact pending bodies or hands off one final value.
```

The session **owns immutable limits and the original root meter**. Neither can be
replaced or refilled after construction, including between repeated helper replies.
All current/pending frame quotas and whole-body-forest admission reuse the stack
profile. Construction has no host callbacks and first poll performs admission.
**Acceptance only marks Ready**: it does not restore/project/run a successor.
**Waiting and terminal polls are inert**. Wrong identity, authority, type or value
returns the exact original input while keeping the whole pending stack.

Cancellation seals Cancelled **before whole-stack cleanup**, whether initial,
awaiting or already accepted. Dropping the session releases all owned frames/input
without running re-entry, but does not notify or roll back external effects.
Execution/acceptance cleanup unwind seals HostUncertain; normal faults seal Failed.
Cancellation cleanup unwind retains its already sealed Cancelled reason.
Final scalar bounds and native cleanup precede once-only value/request handoff.
No native Clone/Debug/serde/Send/Sync bound or portable checkpoint is added. Native
Drop remains trusted, and a second cleanup panic can abort the process.

The move-only scalar host preserves both scopes across read/write and returns 53
with exact **100 -> 89 -> 79 -> 69** fuel. Parsed flat-group helpers return either
49 (scalar helper) or 48 (native group passed through the outer frame), projecting
the authenticated native reply once. This fixture uses **explicit native work
reservation**: correlation prepays six batch/assembly units from the root meter
before publishing dispatch; an owned submeter spends only those reserved units.
It cannot refill the root session or refund unused work. The reservation is this
finite adapter's reviewed cost policy, **not a generic scheduler or automatic cost
inference**. Native ingress, aggregate capture bytes and callback CPU still require
separate host bounds. Acceptance callbacks are not implicit fuel-charged execution.

This closes **owned-fuel in-memory nested binding sessions**, not nested group-member
scheduling, recursive helpers, portable durable recovery or an independent VM.
Legacy single-frame APIs/diagnostics remain unchanged; no product VM migration or
embedding maturity promotion is implied. Hosts still own authentic correlation,
current capture provenance, live policy, framework affinity and cancellation delivery.

### Shared Recursive Native Control Source

`control_source::lower_control_source` is one recursive **Bind/Choose composition
entry around explicit native leaves**. It connects source control positions to
the borrowed session without requiring hosts to manually chain separate binding
and choice constructors. It does not replace the reference frontend or its VM.
Scalar operands, fields, native calls, groups and helpers remain explicit leaf
compilers; recursive control hidden inside a leaf is still that adapter's policy.

**Whole cold checks precede every native hook**: physical source/literal bounds,
reserved scalar, choice and projection signatures, bounded unique initial names and all
bind/loop/fold lexical rules. Even an unselected branch cannot hide malformed
names, shadowing, an excessive active scope or an invalid loop/fold literal limit.
The shared ceilings remain 16,384 nodes, depth 64, 64 operands and 1,024 active
names. Bounds apply before callbacks, not before the caller constructs the AST.

**Borrow original native type observations, never clone a type frame**. A binding
initializer sees only parents. Its body borrows that exact owned observation;
sibling branches see only parents, and their local names cannot leak. The prefix
and `ControlSourceScope` expose bounded read-only references, not native runtime
locals, grants or automatically installed host frames. Native types may be
non-Clone, non-Debug, non-PartialEq and GUI-local. Interior mutation remains trusted.

Lowering is once-only in Value/Body and When/Then/Otherwise order, independent of
named submission order or literal truth. **One aggregate shifted output budget
retains future sibling roots** while checking original native leaf IR. Each shared
constructor is charged once; native depth is not reset at a new control boundary.
Conditions independently pass whole bounded pure IR checks before the native
boolean hook, and an incorrect literal kind cannot be overridden. Branch type
comparison is explicit and fallible; it cannot silently union group exports.

**Whole-output native admission runs once before handoff**, only after every cold
child fits. The mandatory `admit` hook corroborates original result observations,
complete cold typing, versions/grants, opaque graphs and capture compatibility.
It must reject physically valid but unsupported unfinished nested captures.
Admission is neither dispatch nor a reply certificate. All native callbacks own
work/allocation/hidden-expansion policy and cannot be preempted or rolled back.
Physical accounting is not source-weight accounting, fuel or a native sandbox.

The parsed counter proof now composes read, result-driven choice, another binding
and a pure tail through this entry and the shared session. Actual read 41 selects
write 42, then produces 43; the opposite branch writes 0 and produces 1. Only the
selected native call runs, original catalog rows survive, exact cumulative fuel
is preserved, wrong identity returns the original reply, and cancellation after
read acceptance prevents write. Independent cold native typing rejects invalid
arguments, incompatible branch types, wrong versions/grants and unfinished capture.

Separate move-only tests cover original AST and buffer identity, native type
borrows, sibling isolation, quota/minimum-root fences, shifted depth, all native
error/unwind phases and observation cleanup panic. Failure/unwind publishes no
partial tree, retries nothing and releases owned parts; a second cleanup panic
can still abort. This is **source composition, not a complete independent compiler
or durable VM**. Full leaf/helper/group compilation and opaque typing/recovery
remain extraction gates; the product frontend and wire contracts are unchanged.

### Shared Fixed Source Signatures

The crate-private `source_call::fixed_named_operands` decodes **nonempty fixed
named signatures without heap allocation**. Scalar unary/binary/recovery/choice
forms, native control choice and field/member projections now share this one
metadata gate. It returns a fixed stack array borrowing the **original AST
operands in semantic protocol order**, not a reordered source tree. The scalar
constructor no longer builds a temporary operand Vec for each fixed form.

**Exact complete names precede native work**: arity, missing/unknown/duplicate
arguments and even duplicate internal signature names fail closed without a
partial view. Empty signatures are deliberately unsupported at this internal
entry. Callers still perform their existing whole-source physical and cold
checks first, including malformed unselected children. Each caller maps rejection
to its original error variant and parent span; unary Value, binary Left/Right,
recovery Value/Fallback and choice When/Then/Otherwise order are unchanged.
Native buffers, observations, callbacks, cleanup and type/policy checks do not
move into this helper. The product frontend uses the same scalar/choice/projection
constructors, preserving its own source-cost and diagnostic precedence.

This is **private source metadata decoding, not schema or execution authority**.
Dynamic native call schemas, variadic strings/groups and bind/loop/fold lexical
rules retain their separate gates. No new grammar, coercion, native trait bound,
dispatch, fuel grant, cached admission or durable VM is added. Only fixed operand
scratch allocation is removed; whole compilation is not allocation-free, and no
end-to-end speedup is inferred without a benchmark. Full native schema and durable
suspension remain open; independent embedding maturity is unchanged.

### Shared Exact IR Child Cursors

`Computation::children()` now returns a compact borrowed **ExactSizeIterator**
with the same declaration order, reverse/mixed iteration and original node
identity. Direct language length/count and forward/reverse skips are constant-time,
including out-of-range `usize::MAX` skips that seal the cursor without overflow.
Empty variadic and opaque leaf nodes expose zero children. Construction and
iteration allocate nothing and impose no native Clone/Debug/serde/Send bounds.
The current cursor occupies three machine words, not a stable foreign ABI layout.

Shared `binding_source::physical` now uses **pending sibling cursors, not a wide
node frontier**. Only branching ancestors with remaining siblings retain a cursor;
unary chains accumulate no frames. The checked integer pending count still fences
every child's minimum physical root before cursor growth. **Original reverse DFS
and cumulative budget error priority are preserved**, including existing initial
charges, shifted depth, cold children and rejection before later native hooks.
Internal depth increment overflow returns `StructureError::DepthLimit`, after the
existing pending-root node check; public source/output ceilings are unchanged.

Binding, choice, recursive control, group/sequence source and staged helper
completion use the existing shared physical entry without a second native tree.
Wide-node scratch now scales with open branching depth rather than sibling width;
the physical walk may still allocate its bounded cursor Vec. Exact child counts
are **structural metadata, not native payload admission or execution authority**.
Host effects, original operations/result observations, source weights, full cold
type/policy checks and fuel remain in their existing separate protocols. Other
walkers may retain their own bounded frontiers; this does not claim that all
compiler scratch is allocation-free or that opaque graphs are now validated.
Full native schema and durable suspension remain open; independent VM maturity
is unchanged. Tests compare the old and new physical walks across budgets and
cover wide nodes, all child layouts, original opaque slots and arithmetic edges.

### Catalog-Driven Expression Compilation

`catalog_source::compile_catalog_source` is one shared compilation entry for the
closed **scalar/call Bind/Choose expression profile**. The host implements
`CatalogSourceEnvironment` for native operation-slot construction and exact
field/member exports, alongside the existing declared-result type queries. It
does not provide a recursive source compiler or manually compose scalar, call,
projection and whole-flow admission stages. Native slots need no Clone/Debug/
serde/Send; only result query identifiers retain the existing Clone/PartialEq
contract and may borrow nonclone native declarations.

**Whole cold structure and catalog policy precede native compiler hooks**.
The shared control preflight checks physical source, intrinsic signatures and
lexical scopes first. A declaration-order catalog pass then checks every cold
native call's exact version, known operation, grants and complete parameter names
before operation mapping, field/member exports or result-type queries. Cold call
quotas include unselected arms and calls hidden in pure operand positions. The
host's Borrow/equality methods are still trusted metadata work, not sandboxed.
Unsupported seq/all/repeat forms fail closed, not as overridable catalog calls.

The compiler owns scalar lexical bind/loop/fold, Bind/Choose result capture, pure
call operands and direct bound field/member projection. Original catalog schema
order controls operands, and mapped native slots must retain both key equality
and exact source-name identity. One aggregate shifted control-output budget and
final **whole cold call-flow inference** corroborate every constructed type, local
and call against the same catalog; observations are not trusted output certificates.
Unknown locals cannot fall back to a global host registry. Nested effects cannot
be hidden inside pure scalar operands, even if their catalog names are known.

The parsed structured device host now uses this entry instead of its own scalar/
call/projection/admission orchestration. A separate scalar counter host compiles
through the same entry and executes with shared `EffectSession`: parameter
preparation, suspension, wrong/stale reply rejection with original ownership,
accepted scalar re-entry, once-only completion and cancellation. Native panel
and counter catalogs also prove different operation names and scalar types
without changing the parser or IR. Existing live-policy and foreign structured
result rejection remain in their original execution adapters.

This is a **closed-profile source-to-session vertical proof**, not complete
independent language/VM acceptance. Multi-function helpers use the separate
entry below; flat group expressions use the opt-in entry after it. Repeat/nested
group compilation and scheduling, opaque Host graphs, arbitrary
result-expression projections and portable durable frames remain open. The
existing product compiler and wire formats are unchanged. There is no evaluator,
storage backend or lasting execution authority installed by this source entry.
Native callbacks/work/interior mutation/Drop remain bounded adapter policy;
failures and unwind release core-owned partial IR without retry or publication.

### Catalog-Driven Multi-Function Compilation

`catalog_program::compile_catalog_program` extends that expression profile with
an explicit parameterless entry and acyclic pure/effectful helpers. The same
`CatalogSourceEnvironment` supplies metadata only: no recursive compiler,
template-copy callback, new native Clone/serde/Send requirement or global registry
is introduced. Parameters use the six exact scalar tokens and declaration-order
pure operands. Helpers can return scalars or original structured result metadata.

**All declarations, including unused helpers, precede native compilation hooks**.
Shared declaration/signature checks and the entire dependency graph reject
duplicate/reserved names, missing or parameterized entries, recursion and entry
calls. Each complete cold source passes physical/intrinsic/lexical gates and all
helper named signatures/native catalog versions/grants. Native call quotas count
the declaration forest; final inference also checks every expanded output call.
Intrinsics and catalog operations cannot be replaced by local declarations.
Catalog Borrow/equality and other native metadata work remain trusted, not sandboxed.

Bodies compile in **closed parameter scopes**, not caller scopes. Shared hygiene
renames parameter aliases and local bind/loop/fold/result captures while moving
original native slots and buffers. Caller arguments retain their own scopes;
the shared scalar scope supplies typed loop/fold and nested-bind locals to helper
operands. Intrinsic labels such as `body` are not legal variable reservations.
Parameter wrappers evaluate pure arguments once in declaration order. Unknown
locals and caller capture, including unused helper bodies, fail closed.

This implementation deliberately **recompiles bodies instead of cloning native
templates**. `max_compilation_nodes` charges each compiled body source occurrence
and every parameter wrapper, including unused-helper checks and nested/repeated
expansions, without refund. `max_expansion_depth` bounds nested shared expression
entries; original AST/IR depth, output nodes, active bindings and cold native call
limits remain separate. These are bounded language-work policies, not exact CPU,
allocation, opaque native graph, ingress-byte or execution-fuel measurements.
There is no unbounded expansion or performance-speedup claim. A future template
cache must preserve explicit native ownership rather than add implicit copies.

Scalar-counter and structured-device multi-function programs compile through
this entry and run with the same shared `EffectSession`. Tests cover typed/wrong
replies, original record field projections, pure tails, once-only output,
cancellation and live capability revocation after accepted input. Neither
vertical needs product VM, SQLite or a changed parser/wire grammar.

This closes **the acyclic helper portion of the catalog expression profile**,
not full independent language/VM acceptance. Group/helper-program composition,
repeat compilation and scheduling, opaque Host graphs, complete product-adapter migration and portable
durable recovery remain open. The legacy product compiler/diagnostic priority is
unchanged; no evaluator, persistence backend, dispatch authority or partial
program is published by this entry. Native failure/unwind releases owned output.

### Catalog-Driven Flat Group Compilation

`catalog_group_source::compile_catalog_group_source` is an additive entry for a
complete **flat-group control expression**, not manually spliced IR fragments.
The developer implements `CatalogGroupSourceEnvironment` on top of the same
operation/export metadata adapter and shared `GroupFlowEnvironment`. The only
new source hook maps an original selected catalog declaration and observed type
into a native branch IR slot. It does not compile children, invoke effects or
clone native templates. Higher-ranked group metadata cannot borrow generated
temporary IR; it may retain original catalog/owner borrows or owned query data.

Complete cold physical, intrinsic and closed lexical checks precede every native
mapping/type hook. Unbound references are rejected even in unselected arms,
later members and loop/fold bodies; member labels never introduce sibling locals.
This reuses the existing Bind/Loop/Fold scope transitions in a closed opt-in mode,
without changing open native-alias source adapters. Supplied prefixes retain their
own bounded names; a bind initializer cannot read its not-yet-installed alias.

All group headers, unique member names, arity and direct-call eligibility then
pass across the complete cold source, before catalog queries or native slot mapping.
All catalog names, versions, capabilities, parameter names and call quotas pass
before operation/branch construction or group export/type queries. Seq needs one
member and All two; inclusive member ceiling is 64. Every cold group site counts,
including unselected Choose arms. Nested groups, repeats, helper calls and opaque
Host leaves remain outside this entry; they cannot masquerade as registered calls.

Existing scalar/call lowering constructs each member with pure operands and the
same outer lexical prefix. Shared flat assembly enforces one shifted member forest
budget before native member admission. Each original branch declaration must match
fresh call-flow inference; complete group-flow inference corroborates mode, ordered
names, operation identity and member types before constructing closed group metadata.
The outer shared control builder owns one whole-expression output budget and repeats
whole group-flow admission. A supplied observation or per-leaf limit cannot replace
this final gate. Unknown exports, wrong slots/declarations, incompatible Choose groups
and inconsistent native metadata fail closed, without unioning unrelated members.

Native fields, operations, effects and branch declarations need no Clone/Debug/serde/
Send; query metadata alone retains its existing Clone/PartialEq protocol. Construction
may allocate bounded metadata, and native callbacks may mutate or unwind; those costs
are host-owned, not automatically paid by source/IR quotas or rolled back. Failures
consume core-owned prefixes once, never publish partial IR or retry native callbacks.
Closed diagnostics retain typed phase/span/native errors for matching without exposing
private payloads through formatting or source chains. Output is still mutable data,
not authority, a receipt, a frozen catalog snapshot or a durable continuation.

The complete parsed counter expression now uses this entry, then the original shared
parallel barrier, fresh complete result assembly, accepted outer binding and member/
field re-entry. Its two calls and exact remaining fuel are unchanged. Independent
move-only declaration tests cover Seq/All, source/IR bounds, complete cold denial,
shared loop/fold lexical scopes, malformed exports, native unwind and cleanup.

Legacy call-only expression and acyclic helper-program entries remain group-rejecting;
their diagnostics/product wire APIs are unchanged. The additive entry-only helper
program profile below composes flat entry groups with existing helpers; a separate
opt-in profile also compiles flat-group helper bodies and return values. Nested result
scheduling, repeat scheduling, product migration and portable durable recovery remain
open. This flat source-to-re-entry profile is not complete independent VM
acceptance or repository extraction.

### Catalog-Driven Group Entry Programs

`catalog_group_source::compile_catalog_group_program` compiles a complete program
with an explicit parameterless entry, **flat groups in the entry only**, and the
existing acyclic scalar-parameter helpers. It is not a second helper compiler.
Shared declaration/dependency admission, body recompilation, hygienic native-slot
moves, pure parameter wrappers and nonrefunding work/depth guards are reused.
The original `compile_catalog_program` remains call-only and group-rejecting;
its diagnostic priority and the product compiler/wire APIs are unchanged.

Every declaration, including unused helpers, passes **closed cold lexical checks
before native mapping or type queries**. Parameter scopes reuse Bind/Loop/Fold rules;
unbound references cannot capture entry locals or hide in unselected arms. All
helper signatures, entry-call/cycle rules and catalog versions/grants/named calls
pass across the complete declaration forest before compiling any body. Cold native
call quotas include unused helpers; final group-flow inference separately counts
the complete expanded entry. Group quotas count every cold entry site, including
unselected Choose arms. Helpers may not shadow catalog operations or intrinsics.

Pure helpers may compute native group operands or a result-driven entry tail.
Ordinary effectful helpers may supply separate entry leaves with scalar or original
structured results. **Direct group members must still be registered native calls**:
even a zero-parameter helper returning one call is rejected before native mapping.
Helper bodies cannot return or contain groups, and nested groups/repeat remain
unsupported. Structured/group parameters and arbitrary result-expression projections
are not implicitly added. Effectful helper operands still fail the original pure
IR gate; native mapping work is not rolled back and no effects are dispatched.

`CatalogGroupProgramLimits` retains one shared `CatalogProgramLimits` plus entry
group/member ceilings. Compilation charges unused validation and every repeated
helper body/parameter wrapper without refunds. The group entry enters the same
expansion guard as an ordinary entry; helper recursion cannot reset it. Shared
shifted whole-output physical limits, active binding limits, exact branch declarations
and final whole group-flow admission still apply after expansion. Neither per-leaf
quotas nor supplied result observations are admission certificates. Group-entry
errors retain a typed `CatalogSourceError::Group` phase with redacted formatting.

The unrelated counter program parses forward helpers, compiles a flat All entry,
accepts out-of-order replies behind the original barrier, and resumes a hygienic
two-parameter helper tail after authenticated group binding. It returns 48 with
exact remaining fuel 73 from the original 100-unit meter; the existing helper-free
case still leaves 77. Wrong reply identity preserves the original input. Cancellation
and live revocation prevent helper-tail projections/restoration without retry or
new dispatch. Move-only declaration/operation tests also cover ordinary effect helpers,
closed exports, inclusive work/depth/output limits and native error/unwind cleanup.

This closes **entry-level flat-group/helper composition**, not group-returning helper
compilation, nested result scheduling, complete product migration, durable recovery
or independent VM acceptance. No template-copy callback, AST substitution, evaluator,
storage, native sandbox or durable authority is introduced. The remaining extraction
gates stay explicit; this API alone does not promote embedding maturity.

### Catalog-Driven Group Helper Programs

`catalog_group_source::compile_catalog_group_helper_program` is an **explicit opt-in**
for flat Seq/All in complete acyclic helper bodies, including group return values.
It retains the same parameterless named entry, scalar helper parameters and original
host catalog/type protocols. The call-only `compile_catalog_program` and entry-only
`compile_catalog_group_program` preserve their earlier profiles and diagnostics.

```text
fn main() = bind(batch: collect(n: 40), body: member(value: batch, name: "left"))
fn collect(n: integer) = all(left: counter.left(n: inc(n: n)), right: counter.right(n: n))
fn inc(n: integer) = add(left: n, right: 1)
```

This remains **one shared helper compiler**: a private language lowering policy
selects the existing group-flow adapter while preserving shared declaration checks,
body recompilation, fresh-name hygiene, pure parameter wrappers and nonrefunding
work/expansion guards. No developer-supplied template-copy/compiler callback or AST
substitution is introduced. Original native fields, operations, effects and branch
declarations move without new Clone/Debug/serde/Send requirements; native query
metadata cannot borrow temporary compiled IR.

Every declaration, including unused helpers and unselected arms, passes cold closed
references, helper signatures/dependencies and catalog versions/grants/named calls
**before native mapping or type queries**. Group quotas count the **complete cold
declaration forest** and separately the **complete expanded output**. Whole physical,
active lexical and group-flow admission corroborates original branch declarations
and exact mode/ordered exports. Group results cannot be coerced into scalar operands
or joined by unioning incompatible exports. Direct members remain registered native
calls; helper aliases, nested members, repeat, recursive helpers and group/structured
parameters are not added. Failure or unwind releases owned output once, without retry,
partial IR or native rollback.

For parameterized helpers, the native adapter must **prepare complete operands while
their lexical scope is live**, before parameter wrappers release that scope. The
move-only fixture snapshots both original catalog schemas and values 41/40 from the
same 100-unit fuel meter, leaving 92; preparation does not dispatch effects. The core
does not copy locals into requests or define a generic captured-result wire format.
Native capture, dispatch and live acceptance policy remain explicitly adapter-owned.

The separate parsed zero-parameter group-return helper runs through the original
parallel reply barrier, complete result assembly, authenticated outer binding/member
re-entry and hygienic scalar tail. It returns 48 with **exact remaining fuel 73**,
preserves wrong-identity reply ownership, and stops before tail projection on
cancellation or live revocation. These are unrelated native hosts, not the product VM.

Compilation is **not nested suspension scheduling**: an initializer that already
suspended inside its helper still fails with `NestedSuspension` in the shared single-frame effect
evaluator, without returning a partial continuation or dispatching prepared work.
This API does not implement nested graph scheduling, portable durable recovery,
complete product migration or independent VM acceptance, and **does not promote
embedding maturity**. Native callbacks, resource costs and unwind remain trusted.
The separate bounded return-frame profile above explicitly composes in-memory
binding returns; it is not enabled by this compiler entry or by existing sessions.

### Shared Cursor Purity Inspection

Shared `Computation::is_pure()` now consumes the same exact borrowed child cursors
through a **pending sibling cursor stack, not a wide node frontier**. Leaves and
unary chains need no scratch allocation. Only open branching ancestors retain
cursors, so wide-node scratch depends on open branching depth rather than sibling
width. Branching may still allocate; a cursor is larger than a single node pointer,
so this is not a universal memory or end-to-end speedup claim for narrow trees.

The original **reverse DFS and early effect rejection** remain unchanged. Every
Host/Call/Group is effectful even if empty, invalid or cold; Member remains a pure
result reference. Guards are not evaluated, loop limits do not skip cold children,
and opaque native graphs are never inspected. Original fields, operations, effects
and result metadata need no Clone/Debug/serde/Send bounds and are not consumed.

Reference lowering, helper body/argument preparation and group export checks use
this shared classification directly, without another native tree or public API.
`scalar_argument_type` still borrows only direct literals and passes through the
separately supplied type hint; it does not infer or validate that hint. IR remains
mutable, and a later mutation requires fresh classification. Purity is **not a
structural bound, type certificate, native admission or execution authority**.
Callers still bound ingress and perform full cold type/schema/policy checks before
dispatch; this walk adds no hidden node/depth ceilings or execution fuel.

Differential tests compare the former frontier walk across all node variants and
cold control positions, every position in a wide effect-containing list, 65,536
pure siblings, 4,096 unary links, fresh mutations and opaque nonclone native slots.
These are structural inspection proofs, not admission of such sizes by an adapter.
Full native schema and durable suspension remain open; independent VM maturity
and reference wire formats are unchanged.

### Shared Opaque Host Flow Typing

`flow_typing::infer_host_flow_type` extends the shared declared-type chain to
**mixed Host/Call Bind/Choose flow** with explicit `HostFlowEnvironment` and
`HostFlowTypeLimits`. Existing call-only and group entries still reject opaque
Host nodes, and the new host entry deliberately rejects newly constructed groups.
This is a separate opt-in boundary, not implicit admission of every native effect.

**Whole cold structure precedes native work**. The complete language tree and
bounded unique prefix are checked first, then every cold call's version/grants and
named schema, then opaque admission once per original Host leaf in source order.
Only after all admissions finish may prefix type identifiers clone or guard,
argument, result, field/member and join queries run. Failed cold admission cannot
be hidden by an unselected branch, an unused binding or a false loop guard.
Choose types both arms; it never evaluates its guard or dispatches either arm.

**Native admission owns hidden graph safety**. One Host counts as one language
node; `max_hosts` bounds cold leaves, not private payload allocation, native graph
size, execution fuel or native callback work. Admission must bound the complete
native payload and corroborate exact operation/schema/declaration identity and
current policy. Hooks borrow original effects without Clone/Debug/serde/Send
requirements. Result metadata may borrow the original nonclone declaration for
the IR lifetime; only query identifiers need Clone/PartialEq. Closed failures
include host admission/result positions but never native payloads or source text.

Guards, call arguments, projections and operator/loop/fold/recovery operands
remain pure, including cold paths. One guarded lexical frame preserves result
captures, aliases, sibling isolation, forward-reference and shadowing rejection,
active binding ceilings, and closed external group exports. Explicit host joins
must retain only common exports, never combine disjoint declarations into a union.
Physical limits remain inclusive, including 64 recursive depth and 1,024 prefix
bindings; zero host capacity denies every Host, with no expanding default.

The product uses this chain for atomic native leaves and explicitly adapted flat
native group wrappers after its existing bounded canonical admission. Mixed
native/computed-call captures and compatible branches share inference without
changing native source restrictions, canonical or wire bytes, raw-receipt rejection
or later capability checks. Mixed computed groups use the explicit combined entry
below, not the group-rejecting leaf entry. Nested native wrappers retain their
existing separate policy path.
No canonical reparse occurs inside host type admission. Unrelated numeric and GUI
hosts, borrowed nonclone declarations, cold fences and unwind cleanup are tested
without a product evaluator or authority registry.

This is **opaque leaf flow typing, not complete native graph compilation**. It
does not classify executable atomic/group flows, validate actual replies, grant
dispatch authority or create a reusable certificate. Native callbacks may mutate
state or unwind; neither external work nor policy changes are rolled back. Every
later admission must recheck current mutable payload/policy. Full native schema
lowering, nested opaque graph adaptation and durable suspension remain open.

### Shared Mixed Host Group Typing

`flow_typing::infer_host_group_flow_type` explicitly combines flat prepared groups
with surrounding Host/Call Bind/Choose type flow. `HostGroupFlowEnvironment`
requires both existing host and group protocols plus **original operation and
selected declaration pair admission**. Operation observations borrow the native
payload or canonical native declaration for the IR lifetime; they are never
reconstructed look-alike keys. Native operation/effect/branch-result slots need
no Clone/Debug/serde/Send traits. Only ephemeral query identifiers can clone.

**One original schema row on every member path**. Each member has pure Bind/Choose
preparation ending in one atomic Host or Call on every cold path. Calls and mapped
hosts must select the same original schema row, not merely equal result tags or
operation names. Sequence requires one member and Parallel two. Nested groups,
effectful preparation initializers, scalar exits, captured preparation and impure
guards/arguments remain rejected. Earlier sequence receipts and sibling locals
never become member preparation prefixes.

Whole physical IR, prefix, names, group shape and cold budgets are checked before
native hooks. All cold call schemas/named shapes finish before once-per-Host
admission. Group hosts then map once to their original operation, select a current
version/capability/schema row, check the parameter ceiling, and corroborate that
exact original effect/operation/result-declaration pair. All mappings and uniform
row checks finish **before prefix clones or semantic type queries**. Host and Call
positions remain separate, so cold host leaves never shift call error positions.

`HostGroupFlowTypeLimits` explicitly bounds hosts, calls, groups, per-group members
and one aggregate language node/depth budget. A Host still counts only one language
node; native admission must bound hidden payloads/graphs, current policy and atomic
eligibility. No limit silently expands, and zero capacity denies that site class.
The old call-only/group and opaque-leaf entries retain their rejection rules.

Original branch declarations must match their inferred types before one ordered
closed group observation is constructed. Original names and operation identities
reach the existing group member/field protocols unchanged. Failed declarations,
mapping/pair rejection and callback unwind cannot publish partial group exports,
replay effects, retry hooks or mutate the caller's prefix. External native work is
not rolled back; mutable payload/policy needs fresh admission on every later use.

The product adopts this entry after bounded canonical admission for mixed flat
groups, conditional native/computed member preparation, helper-produced members
and legal group successors. It borrows canonical native operation/result rows and
checks their exact identity without reparsing inside host admission. Canonical
and wire bytes, closed exports, native diagnostics and later capability checks
remain unchanged. **Type success does not widen executable source profiles**:
existing bounded capture-chain restrictions still reject otherwise typeable flows.

This is flat mixed group typing, not nested native graph compilation, execution
classification, actual receipt acceptance, a durable continuation or reusable
authority certificate. Full native schema lowering, native graph/repeat adaptation
and durable suspension remain extraction gates.

### Shared Flat Native Group Observations

Flat native `seq/all` wrapped in an opaque Host now use the existing
`HostFlowEnvironment` boundary, not another language IR or public graph ABI.
The reference adapter admits only a bounded **closed flat native group**: exact
mode, valid unique ordered names, atomic original operation rows and matching
original branch declarations. Nested graphs and Compute members are not admitted
by this view. Canonical product admission still checks native payload domains
before this routing; the view alone is not a general native payload validator.

**One member observation representation across native and computed groups**.
Native names borrow the original branch strings; operations borrow the exact
canonical native declaration rows. Computed group names/operations retain their
original IR borrows. Only a bounded reference-counted query identifier is owned;
native effects, operations and declarations are not cloned or reserialized.
Same mode, order, names and original row signatures preserve closed exports through
joins and aliases even when native payload targets differ. Mismatched signatures
never union exports. A graph Host can be a captured group, but cannot masquerade
as an atomic member of a newly constructed group.

**Private native graph bounds remain explicit adapter policy**. One Host consumes
one language node and one host site, not one slot per hidden native member. The
unrelated GUI proof applies a separate inclusive graph-node ceiling, original
operation/declaration ownership and current version/grants to every cold graph
before prefix clones or semantic/member queries. Forged equal-tag declarations,
invalid cold members and later native graph mutation require fresh admission.
The generic core never inspects product graph variants or invents execution grants.

Independent move-only GUI graphs and native/computed group choices retain exact
original name/operation/declaration pointers. Callback unwind retries nothing,
publishes no partial observation and leaves the borrowed prefix unchanged;
native external work and Drop cannot be rolled back or preempted. Product proofs
cover group captures, result-driven successors, helpers, pure aliases, closed
conditional exports, cold forged metadata, wire parity and capability denial.

This is **flat native graph type observation, not recursive native graph
compilation**. It changes neither source execution profiles, canonical/wire
contracts, receipt acceptance, scheduling/barriers nor durable continuations.
Full native schema lowering, nested native graphs/repeats and durable suspension
remain extraction gates. The independent embedding maturity is not promoted.

### Shared Borrowed Native Graph Inspection

`native_graph::inspect_native_graph` now owns the bounded **physical native graph
walk**, with explicit `NativeGraphLimits`, borrowed `NativeGraphShape` views and
original branch-to-child mapping. Sequence and Parallel views may nest; leaves
remain native payloads. No second graph/IR tree, public graph codec, implicit
schema registry or native Clone/Debug/serde/Send/PartialEq bound is introduced.
Unsized GUI-local nodes are supported through the same explicit borrowed entry.

**Declaration-order DFS with one sibling cursor per open group**. The core retains
the original member slices and maps each original branch once, immediately before
its child. It never fills a frontier with every future sibling. Limits bound
cursor depth; every repeated/shared edge is charged again. Cycles terminate by
node/depth denial, not deduplication or a separately inferred cycle certificate.
The inclusive ceilings are 16,384 nodes, root-zero depth 64 and 64 members. Sequence
requires one member, Parallel two. Zero nodes denies before hooks; zero members
denies groups, not leaves; zero depth permits only root leaves. No quota expands.

**Physical visit before View; arity before Child; inline Leaf checks**. View and
edge hooks are trusted structural protocols that must expose the original graph
without hidden expansion. Their native work/allocation and leaf payload bytes are
not metered by this walk. Leaf checks run at their occurrence, preserving native
structural error order. This is deliberately **not whole-cold type admission**:
complete cold typing, names, declared results, schema identity, grants and closed
exports remain separate admission gates after complete structural inspection.

The reference canonical effect preflight delegates this walk while retaining its
16,384-node and 64-level conventions, inclusive arity and `LSH1201/1204/1205` errors.
Compute leaves retain their separate language shape budget and inline diagnostic
precedence. Canonical source/wire bytes, later type/domain/capability checks and
existing source execution profiles remain unchanged. Physical nesting does not
enable nested result exports or permit Parallel inside sequential control flow.

Errors retain native occurrence/phase/member positions and matchable opaque native
failures, never payload formatting or private source chains. Unwind retries
nothing, consumes no borrowed graph and publishes no partial summary. External
native work/interior mutation/Drop cannot be rolled back or preempted. Counts are
mutable ephemeral metadata, not original graph identity, runtime fuel, a durable
continuation, result acceptance or an execution certificate. Fresh graph and live
policy admission are required after mutation. Nested type/export compilation,
full native schema lowering and durable suspension remain extraction gates.

### Shared Closed Native Member Admission

`native_group::admit_native_group_members` now admits one original closed member
slice through inclusive Sequence/Parallel arity, once-borrowed names and explicit
native member callbacks. This is **member-slice admission, not graph typing**.
Sequence requires one member, Parallel two; the explicit ceiling is 64 and zero
denies all groups before hooks. The shared ASCII member-label grammar allows
digits, hyphens and lexical reserved words; it is not the local-variable grammar.

**All member names precede any native pair admission**. Each original name is
observed once in declaration order and checked for validity/duplicates before the
first native member callback. A **fixed bounded borrowed name table** avoids a
heap collection, repeated name queries or native payload clones. Every original
member is then admitted once per entry, in declaration order. The returned
`NativeGroupMembers` exposes the original kind, slice and cached name references,
not a converted graph, result values or a persisted operation registry.

The product flat native group view and an unrelated GUI host now delegate this
same gate. Native callbacks still corroborate atomic eligibility and the exact
original operation/result declaration. Canonical/source/wire and capability
checks remain separate; Compute or nested native groups do not become atomic
members. **Complete native graph and payload bounds remain separate gates**,
including `native_graph::inspect_native_graph` where appropriate. The product
still requires prior canonical admission; a matching declaration alone does not
validate native payload domains or later live policy.

**Native name/admission errors retain member index and phase without payload
formatting** or private source chains. Failure/unwind returns no partially
admitted view, retries no hook and consumes no borrowed native slots. Callback
work, allocation, interior mutation, external side effects and Drop are trusted
adapter policy and cannot be rolled back/preempted. Supplied names are explicit
native observations, not automatic proof of authenticity. The view is ephemeral,
not a lasting type/schema/grant, reply acceptance, fuel or execution certificate.
Full native schema, nested type/export compilation and durable suspension remain
extraction gates; independent VM maturity is not promoted.

### Shared Complete Scoped Native Group Preparation

`native_group_preparation::prepare_native_group_members` is an opt-in shared
complete member-slice preparation entry. It delegates to the existing
`admit_native_group_members` gate: inclusive Seq >=1, All >=2, <=64, **all
once-observed names and all cold native admissions before any preparation or
fuel debit**. The allocation-free admission-only API remains unchanged.
This new entry is not a second graph walker or a scheduler.

The caller lends **one original fuel meter**. One group-root unit and one unit
per member precede its once-only preparation callback, in declaration order.
The callback may close over a **still-live lexical ScopeFrame** and delegate to
`evaluate_call_arguments_in_scope`; actual argument evaluation/copying and native
work must remain bounded and charged by that adapter. The core neither recreates
the scope nor resets, refills or refunds a meter. Existing preparation adapters
retain their exact legacy fuel accounting; this additional cost is opt-in.

Only a **complete move-only PreparedNativeGroup** is handed off. Each private
core-constructed row links one original member index/reference to its unchanged
opaque preparation. Read-only inspection or consuming extraction requires no
native Clone/Debug/serde/Send bound. The adapter must own or safely retain all
actual operands after lexical cleanup; Rust lifetimes still govern any borrows.
Extracted preparations are mutable native data, not schema/grant/receipt authority.

Failure, exhaustion or unwind releases the prepared prefix, publishes no partial
batch and never retries or refunds earlier work. Matchable errors preserve the
original member position and admission/preparation phase without private payload
formatting or source chains. Callbacks, allocations and Drop remain trusted native
work; their external side effects cannot be rolled back or preempted, and a second
cleanup panic may abort. Complete ingress/IR/type/scope admission, aggregate native
memory limits, current grants and authentic external dispatch remain host policy.

An unrelated parsed Rust counter host now compiles a parameterized helper with
flat-All calls, prepares `41` and `40` before parameter cleanup, and drives an
owned-fuel stacked session. One validated original-group reply restores both
scopes, computes helper value `81`, then caller value `83`. The fixed meter moves
**100 -> 81 -> 68**, including an explicit two-unit native invocation reservation;
acceptance and awaiting/terminal polls do not debit or execute. Wrong identity or
reordered original rows return the unchanged owned input; cancellation, fresh
revocation and late preparation refusal cannot execute successors. Projection
follows the existing bound-reference source profile: bind the member before field
access. This is a local synchronous host proof, not a real network receipt or a
concurrent external-dispatch guarantee.

No product VM path is migrated. This generic slice gate does not infer flatness,
identity uniqueness, authentic schemas, aggregate result domains or durable
captures. Nested group-member scheduling, portable durable recovery and full
independent VM extraction remain open; independent VM maturity is not promoted.

### Shared Fresh Native Export Assembly

`native_group_exports::observe_native_group_exports` assembles one fresh closed
signature from an original native member slice. **All cold names before operation
observations** means the shared member preflight checks current inclusive arity,
the explicit maximum and every once-borrowed name before any native operation
hook or export allocation. **No cached member view or grant is accepted**. Each
entry repeats that complete cold pass; Sequence/Parallel keep the same closed
minimums and 64-member safety ceiling.

Every original member then produces one opaque operation observation, once in
declaration order. Only the bounded export metadata vector is allocated; native
graphs are not converted or copied. Names retain their original borrowed buffers,
and opaque observations need no Clone/Debug/equality/serde/Send traits. The
**Mandatory once-only whole-signature native admission** receives the original
kind/member slice and the complete ordered candidate before handoff. The adapter
must corroborate original names, operation/result/schema identity, atomic
eligibility and current policy; the shared core cannot infer those native facts.

The product type-observation and source-export paths now delegate the same entry,
preserving canonical static result rows and product operation tags. An unrelated
GUI host also uses it with original operation/declaration references; a separate
device-schema regression confirms no product schema is required. Canonical
payload domains, private whole-graph bounds, capability policy and existing wire
formats remain separate. Compute and nested native groups still cannot become
atomic members, and incompatible native/computed signatures are not unioned.

**Owned observation prefixes release without partial output** on name/operation
failure, final admission refusal or unwind. Errors retain member positions and
operation/signature phases without formatting private payloads or source chains.
No hook is retried and no borrowed native graph is consumed. Native hooks,
allocation, interior mutation and Drop are trusted adapter behavior; destructors
may unwind and external effects cannot be rolled back or preempted.

**Mutable export metadata is not current authority**. The returned `GroupExports`
can be changed after handoff and certifies no future schema, live grant, reply
acceptance, fuel, dispatch or durable continuation. Fresh admission remains
required at the relevant later boundary. This is closed export assembly, not
nested graph typing or a complete independent VM; full native schema and durable
suspension remain extraction gates.

### Shared Fresh Closed Signature Comparison

`native_group_signature::compare_native_group_signatures` compares two original
closed member slices without converting native graphs. **Complete left then right
name preflight precedes every native pair**: each side repeats current inclusive
arity, the explicit maximum, shared label grammar and duplicate checks before
native comparison. An invalid left side stops before right hooks. Valid
mode/count/order differences still finish both cold name passes, then return
false without touching native operation pairs.

**Same name spelling is not native row identity**. Names with equal contents can
borrow different original buffers; operation/result/schema equivalence is an
explicit adapter callback on the original member pairs. Pairs run once in
declaration order and stop at the first false/error/unwind. **Aliased slices do
not bypass current comparison**. The same input addresses are not a shortcut or
a cached certificate. Neither side is consumed, cloned, formatted or serialized;
two fixed borrowed name tables need no heap collection or native trait bounds.
**Heterogeneous native member representations remain adapter-owned**: an unsized
GUI-local operation and a separate device format can be compared by their
protocol without making either schema a product enum or universal wire type.

Product closed source-export comparison, computed type-member equality and an
unrelated GUI host now delegate the shared entry. Source export failures retain
their existing group/member positions and diagnostics, including native comparator
errors. The product still compares immutable operation tags; its existing external
metadata variant lacks a group mode and is deliberately not bridged to computed
groups. Existing canonical wire, private payload limits, flat execution profiles
and later capability checks remain unchanged. **Different signatures never form
an implicit union**.

**Side/member errors stay matchable without private formatting** or source chains.
Invalid member data is an error, not equality; valid structural or explicit native
mismatches are false. Hook failure/unwind stops without retries or a partial true
result, but external callback work, interior state, allocation and Drop cannot be
rolled back or preempted. Supplied names and comparators remain trusted native
protocols, not automatic authenticity, live-policy or payload validation.

**A true comparison is not future authority**. Changed names, limits, native rows
or live policy require fresh relevant admission; storing a prior boolean does not
certify type identity, accepted results, fuel, dispatch or durable suspension.
This is closed signature comparison, not full native schema or nested graph
typing, and independent VM maturity remains unchanged.

### Shared Fresh Closed Member Lookup

`native_group_lookup::lookup_native_group_member` queries a fresh original closed
member slice with an explicit current ceiling. **Invalid ceiling and query precede
native name hooks**: the maximum is capped at 64 and the query uses the shared
bounded ASCII member-label grammar. **Complete member names precede selected native
admission**, including inclusive arity, all once-borrowed names and duplicate
checks. **A matching prefix cannot hide an invalid or duplicate tail**. The fixed
borrowed name table needs no heap collection or native Clone/Debug/equality,
serde or Send bounds; callback work and allocation are still adapter-owned.

**Missing members never query native operations**.
**One once-owned native admission on the original member** follows a matching
slot; false returns no
member without trying another slot, and true returns that same original borrowed
reference. The adapter must expose original names and corroborate the selected
operation/result/schema and relevant current policy against the actual query.
This is not automatic original-name or native-row authenticity, admission of
other rows or the whole graph, or a nested graph type profile. Complete private
native graph and payload bounds remain earlier or separate adapter gates.

Product computed member queries and an unrelated GUI test host now delegate the
same lookup. The product preserves its immutable operation-tag rule; the GUI
adapter preserves its existing exact operation-row identity rule. Legacy external
product metadata has no group mode and keeps its old query profile, without an
invented mode or bridging to computed groups. Existing result declarations,
canonical wire, diagnostics, native graph limits and later execution/capability
gates remain unchanged. No native result values are read or replies accepted.

Query/member errors retain the original phase and index with opaque native errors
matchable by the caller, without private formatting, source chains or serialization.
Native failure/unwind stops without retry, partial output or consuming borrowed
members. Once-owned captured hook state is released normally even when a missing
member prevents invocation. Trusted callbacks, interior mutation and Drop effects
cannot be rolled back or preempted; cleanup panics are not disguised as success.

**Borrowed selection is not future authority**. A returned member is metadata,
not a type/grant/receipt/fuel/dispatch certificate or cached admission. Changed
names, limits, native identity or current policy require fresh relevant checks.
This is closed member selection, not full native schema or durable suspension;
independent VM maturity remains unchanged.

### Shared Language Group Shape Preflight

The crate-private `native_group::preflight_native_group_members` now supplies
**One shape gate for native and language groups**. `group_exports` uses it for
original IR branches and returned export metadata; `flow_typing` and
`effect_evaluation` use it before atomic child preparation or native effect hooks.
The old count/index name loops are removed. **Each original label is borrowed
once per shape pass**, using the same fixed bounded table and current inclusive
arity, ceiling, grammar and duplicate policy. Separate passes remain fresh; no
cached member view or earlier success is reused as an admission certificate.

**Language shape accessors are infallible and non-native**: they only borrow the
original label, never select operations, inspect payloads or admit native members.
The original whole-IR physical checks, scopes, group limits and traversal order
remain in their existing walkers. **Existing diagnostics keep their phase and
member index**: flow typing retains group/branch positions, export observation
retains MemberShape and effect execution retains GroupShape. Malformed whole
group names still precede atomic child preparation; complete physical passes
still precede native hooks, prefix copies and fuel use.

**Source labels keep their stricter lexical grammar**. Broader IR labels such as
`1`, `all-4` and `while` remain legal in these existing IR shape profiles, not new
source spellings or canonical wire forms. Source headers keep their separate
argument-name checks, and the product's coarse canonical gate is unchanged.
**Shape success does not admit a native graph or effect**. Exact declarations,
atomic eligibility, current schema/policy and dispatch/reply checks stay separate;
no execution, reply acceptance, fuel spending or durable suspension is introduced.

### Shared Native Host Source

`host_source::lower_host_source` bounds the complete cold original AST before
once-only native preparation, then requires **once-only original effect/result
admission** before constructing the single Host node. The original AST, owned
opaque effect and native result observation reach admission without a name
registry, payload/type clone or second IR tree. Native errors retain phase/span
and matchable payloads, but formatting and source chains never expose those payloads.

**One Host root is not an opaque graph certificate**. Source limits include cold
native operands, while the generated-language meter checks one root at depth zero.
It cannot see native payload allocation, nested effect graphs or expansion costs.
Preparation must bound growth before allocation; the mandatory native hook must
corroborate exact operation/result/schema identity, whole opaque graphs, literal
domains, lexical policy and current versions/grants. Returned observations and
mutable output are not durable validation certificates. Complete cold typing and
fresh authority/result-domain checks remain mandatory before execution.

The product's private preflighted path preserves original parser/declaration,
recursive source-counter and native diagnostic order. Literal native metadata is
not visited again as computation. It retains the nested-computation rejection and
explicitly matches the original callee operation, produced effect operation and
declared result type. Computed-argument calls keep their separate shared call path;
helper Host unwrapping, native group exports, result bindings, canonical/wire bytes
and later capability gates are unchanged. Failure/unwind drops owned parts without
partial handoff, retry, dispatch, refunds or rollback of external native work.
Unrelated numeric and panel adapters prove independent move-only construction.
This is native leaf source orchestration, not complete generic native schema
lowering, opaque-flow typing, actual reply acceptance or durable recovery.

### Shared Bound Native Projection Source

`bound_projection_source::lower_bound_projection_source` consumes an explicit
read-only prefix of borrowed `PureType` observations, directly compatible with
`ControlSourceScope::bindings()`. It constructs **field/member access from the
exact bound native result type**, not a second mutable name registry or implicit
native result-copy factory. The existing projection entry still owns complex
pure-input lowering; this entry intentionally requires a direct bound reference.

**Missing or scalar bindings never query native exports**. Bound result metadata
is borrowed unchanged into the explicit field/member hook. Hooks must corroborate
the original schema/owner/version and exact closed exports; a Result observation
alone is neither a group certificate nor permission to union look-alike schemas.
Member queries receive both the original group observation and original AST names,
and must reject non-groups. Native fields/operations and newly produced member type
observations move unchanged, with no native Clone/PartialEq/Debug/serde/Send bound.

**Cold projection metadata and minimum output bounds precede every query**. The
whole source, field/member signatures and literal metadata, bounded unique prefix
names and inclusive limits are checked first. Field output is exactly Field plus
Local: two nodes at depth one. Member is one node at depth zero, without a fake
Local or value-lowering callback. Name literals remain source metadata. Physical
ceilings are unchanged; hidden native work/allocation and ingress remain host-owned.

The recursive control source entry now checks every cold projection header before
any native leaf runs, including projections hidden inside unselected branches or
opaque extensions. Bad field literals/member references therefore fail before
partial native compilation, not only when that individual projection is lowered.

**Reference member frontend delegates bound source construction**. Its lexical
prefix borrows the exact original native observations and member signatures; no
group vector, result declaration or name-only registry is cloned. Scalar/missing
bindings fail before export queries, and native non-group records supply no members.
The adapter resolves one declared member from that exact observation and explicitly
maps its original operation/result type; the shared entry owns cold source/header,
prefix, physical limits and the single Member node without a synthetic Local.
Original recursive source accounting is unchanged: group/name operands are metadata,
not evaluated values or new computation charges. Root diagnostic codes/spans,
64-member group signatures, helper hygiene, canonical/wire bytes and capability
checks remain intact. Field lowering retains its existing child-error-first path
through shared staged construction described below. This does not claim the
complete compiler frontend has migrated. A member
type observation still grants neither result acceptance nor dispatch authority.

**Reference field frontend delegates staged source construction**. Both the public
projection entry and the product's private preflighted path use the same consuming
field completion: keep the original owned input/type observation, decode literal
metadata, check the whole pure input against the field root's shared physical
budget, then query exact native exports once and construct the original Field node.
No second input tree, result-signature clone or value/effect evaluation is added.
Public whole cold source/header/literal checks still precede every native callback.
The private product path retains header-before-child, child-before-name and
closed-name-before-export diagnostics. Its closed field-name decoder is metadata
mapping, not a native export query; unknown names therefore keep their legacy error
even for scalar or impure inputs. Pure/bounds checks precede actual export admission.
The native adapter owns field vocabulary and type/export policy, while original
recursive source counters charge the child once and never charge name metadata as
computation. Partial input/key observations drop on error/unwind without retry,
dispatch or rollback of external work. Canonical wire/type and capability validation
remain separate mandatory gates; this is shared projection construction, not
complete independent recursive compilation, durable recovery or a type certificate.

**Structured native reply to projected control is proven**. A distinct parsed
device host compiles a record-returning read, binds its accepted native result,
uses the record's boolean field to select a branch and its numeric field to build
the selected write argument. Actual count 41 with ready=true writes 42; ready=false
writes 0 without reading the numeric field. Original catalog/result-domain/layout
identity and cumulative fuel 100 -> 98 -> 90 or 93 are retained. Incoming input is
move-only, and field reads clone only an explicit Rc view, not the native snapshot.
The snapshot is released after parameter preparation. Acceptance does not project
fields, and cancellation after acceptance releases input without projection/write.

Wrong identities preserve the original input buffer; a foreign record layout is
rejected by the original native domain. Whole native typing rejects missing fields,
unbound/scalar aliases, wrong cold parameter types and missing versions/grants
before any actual call. Post-acceptance grant revocation or native view/schema
identity changes close Failed before field observation/write, without retry.
Separate two-schema tests cover all six scalar exports,
exact group observations, original name/type/slot pointers, quotas, zero/exact
output limits and native query error/unwind without dropping the borrowed prefix.

This is **bound type-directed projection construction, not result acceptance or a
complete independent compiler**. Full cold inference/admission, actual value/view
compatibility, live authority, schema mutation and received-value policy remain
native host gates. No source operation dispatches effects or grants authentic
receipts. Errors format closed metadata, not private native payloads; errors/unwind
publish no partial IR, retry nothing and cannot roll back native side effects.
Native work/Drop cannot be preempted, and a second cleanup panic may abort. Complete
helper/group compilation, opaque typing and durable restart remain extraction gates.

### Shared Helper Source Instance

`helper_instance::lower_helper_instance` combines the existing argument,
expansion-reservation, template, hygiene and parameter-binding contracts for
**one explicitly selected admitted helper body**. Selection borrows the original
body and result observation; it is not another registry, native Clone ABI or
complete declaration compiler. The product retains its recursive frontend and
now shares the completion stage described below.

**Current-limit cache checks precede argument callbacks**. Closed cached lexical
shape, body/parameter/binding quotas and caller-reserved identifiers are checked
first. Required named keys are aligned before any argument lowering. Operands
compile once in parameter declaration order, borrowing original AST arguments,
not submission order. Native callbacks own exact caller-scope cold scalar typing.
Literal facts cannot be replaced by a forged scalar observation.

**Combined output fits before materialization**. One shifted meter includes every
parameter wrapper, original operand and cached body shape. Every cold operand
lexical name joins the caller's explicit global/live-prefix reservations, including
unused names. Fresh parameter and body aliases must avoid these names, original
body names and each other; arguments are never renamed or substituted.

**Source reservation commits before the single native factory**. The caller's
counter must already include the original call and all operand source weights.
Shared reservation adds only the cached source-weighted body and wrappers at the
explicit caller depth; native operand-depth observations retain folded/opaque
depth. Invalid reservation leaves the counter unchanged. Once committed, later
copy, name, hygiene, wrapping or final admission failure/unwind never refunds it.
Matching physical shape does not prove faithful native semantics or source cost.

Materialization uses an explicit host copying/remapping policy, rechecks physical
shape and closed lexical scope, then shared hygiene and wrappers move original
operand slots/buffers unchanged. **Mandatory whole-output admission runs once**
with the exact original borrowed result observation before handoff. Native types
need no Clone/PartialEq/Debug/serde/Send. Errors retain matchable native payloads,
but formatting and source chains expose only closed metadata. There is no dispatch,
evaluation, receipt, retry, native rollback, fuel refund or persistent helper cache.

A parsed unrelated helper with reordered arguments, a native field operand and a
local temporary computes 13, observes its field once and restores the caller prefix.
That host accepts only its exact declared record.count projection, never an unknown
source/operation fallback or a field export from another binding.
All six scalar results support zero-parameter instances. Tests retain exact source
argument/result/native-buffer identity, inclusive joint output/source budgets,
checked depth overflow, cold-name capture rejection, current-limit cache fencing,
copy/name/admission error and unwind, same-shape cold type corruption, and once-only
owned cleanup without dropping cached native payloads.

This is **one helper instance, not a complete independent compiler**. Full source
recursion, dependency-ordered body/return compilation, registry lifecycle, opaque
native flow/cost validation, enclosing output/active-prefix limits and live schema,
authority and result-view compatibility remain host/extraction gates. Host work,
allocation, interior mutation and Drop cannot be preempted; bound ingress first,
and a second cleanup panic may abort. Group execution and durable restart are not
added by this entry.

### Shared Staged Helper Instance Completion

`helper_instance_finish::finish_helper_instance` completes declaration-ordered
operands that the caller has already recursively compiled. **The reference helper
frontend now uses this shared completion stage** instead of its separate expansion,
copy, fresh-name, hygiene and parameter-wrapper pipeline. Cached storage retains
the original admitted `HelperBody` and its cost together, rather than splitting
template ownership from duplicate node/depth fields. The original source entry
delegates to the same implementation without repeating its native argument hooks.

**One original source counter crosses the recursive/completion boundary**.
Operands, including nested calls to the same helper, finish compiling before the
borrowed cached body is selected for completion. Their source costs are already in
the caller counter; body and wrapper costs commit only once. No copied meter,
precharge approximation, nested-helper ban, native body clone for lookup or second
registry is introduced. Header/current-limit template checks still precede product
argument callbacks, and lazy dependency order, diagnostics and fresh alias order
remain unchanged for existing programs.

**Complete cold output and all scalar facts precede native operand checks**.
Exact operand count, cached physical/lexical shape, shifted joint output, caller
prefix and every cold operand name are bounded first. Shared closed scalar/literal
facts reject mismatches before any native operand hook. Each native operand then
receives explicit once-only caller-scope admission in declaration order, including
unused parameters. Their reported scalar observations are not type certificates;
the host must infer exact actual types, closed exports, versions and live policy.
All operand admissions precede depth observations and materialization.

The product adapter retains both scalar/native-result bindings and exact named
group-member signatures when corroborating operands and final output. The live
prefix uses the language binding quota, not the expression's root depth; a test
with nine cold nested folds and 18 caller bindings preserves valid compilation.
The live group prefix also permits the full 64-member source signature, with
exact last-member lookup rather than a narrowed recovery-only group limit.
Existing public residual validation keeps its original quota/depth policy. Its original
global alias allocator continues to protect unused global names, while the shared
stage explicitly fences the active caller prefix and all operand names. Final
native admission checks the whole renamed/wrapped tree against the exact original
return observation; structural equality is not semantic or authority proof.

Tests prove a move-only native field buffer and cached result observation are
retained, a forged type on an unused native field is denied before depth/copy or
commit, and wrong cold scalar facts invoke no native hooks. Explicit scalar host
evaluation returns 42 with fuel 100 -> 95 and restores its caller prefix. Zero/exact
joint limits, declaration order, atomic budget failure, argument/copy/admission
error and unwind, no downstream refund, and zero-parameter completion are covered.
Product nested same-helper operands and group-field arguments retain canonical
round-trip bytes; malformed unused arguments retain LSH1504 and source spans.

This is **staged instance completion, not a complete independent compiler**.
Precompiled operands remain unchecked mutable IR with explicit observations;
original source/selection/signature/order authenticity is caller policy. Complete
generic helper body/leaf recursion, opaque typing/recovery and durable restart
remain extraction gates. Native work, allocation, mutation and Drop are trusted,
not fuel-preempted or sandboxed. Failures publish no partial instance, retry
nothing, retain closed error formatting and cannot roll back host side effects;
committed caller reservations remain charged, and a second cleanup panic may abort.

### Shared Bounded Helper Return Join

`helper_join::HelperJoin` combines owned normal-return planning, complete joined
source-cost bounds, explicit continuation copying and mandatory final admission.
**The product compiler now delegates return joining to this shared entry**; its
canonical byte gate, LSH1503/LSH1405 diagnostics, native typing, wire contracts and
live authority policy remain outside or in their original adapter gates.

**Both cold inputs and combined folded weights precede native observation**.
Physical shapes, closed lexical names, capture rules and every return route are
checked first. Complete language/folded costs include each cold continuation copy,
not just the selected branch. Independent physical and source ceilings remain
inclusive 16,384 nodes and depth 64. Optional/list constructor weights are retained;
they are not replaced by physical node counts. Enclosing expansion counters and
canonical text bytes must still be checked by the caller before consuming the plan.

**Original native cost hooks run once per Host occurrence**: value first, then
continuation, each in rightmost-first DFS order. Return-depth calculations reuse
only explicitly observed extra-cost metadata, keyed by the original language node.
Zero-sized native payloads may share one address; their occurrences are never
merged by native pointer identity. This bounded temporary cache is dropped before
handoff, carries no payloads/grants and is not a persistent registry or reservation.

`connect` consumes the plan once. Continuation factories run left-to-right once
per normal return; each copy retains physical/name/capture checks. Joined language
and folded source weights are rechecked before **mandatory whole-output admission
runs once**, receiving the complete output and planned native-inclusive cost.
Same physical shape, source counts or depth do not certify native/literal identity,
typing, schema versions or grants. The host must corroborate faithful copies,
bounded native graphs/costs, canonical bytes and live authority before execution.
There is no implicit native copy, Clone/Debug/serde/Send requirement or extra IR.

Tests cover zero/exact/overflow limits, bad cold inputs before hooks, combined
folded costs, distinct zero-sized occurrences, shifted native depth, same-shape
folded corruption and same-cost cold native grant corruption. A move-only host
retains all four native slot types and original Boxes/vector/buffer identities.
A distinct scalar host returns 42 with unchanged fuel, serialized language IR and
caller prefix. A bounded return corpus matches fresh complete post-copy source
measurements; existing product helper wire/capability/diagnostic proofs remain.

This is **bounded return joining, not a complete independent compiler** or semantic
certificate. Complete dependency/body/leaf lowering, opaque flow/recovery typing,
group execution and durable restart remain extraction gates. The plan owns local
counts, not a shared counter: caller reservations are neither altered nor refunded.
No execution, dispatch, replay authority, retry or native rollback is added.
Observation/copy/admission errors and unwind publish no partial IR and release
consumed owned inputs once. Errors retain matchable native payloads but expose only
closed formatting, not native source chains. Host work/allocation/interior mutation
and Drop are trusted and cannot be preempted; a second cleanup panic may abort.

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

### Native VM Build Profile

`leselang-vm` now separates its host-neutral in-memory entry points from the
released Leserpent specialization. The product-free configuration is:

```toml
leselang-vm = { path = "../leselang-vm", default-features = false }
```

`leselang_vm::native` exposes the exact shared `syntax`, `hir` and `core` crates.
Its `Vm` is the existing `StackedControlSession`, not a second evaluator or
conversion wrapper. Catalog compilation and synchronous reply-gated sessions use
the same native implementation as the HIR profiles. Native adapters still own
catalog versions, live grants, authentic receipt identities, dispatch, aggregate
capture/ingress bounds, thread affinity and accepted-work cancellation delivery.
The session borrows original IR and keeps one fuel meter across all re-entry.
No timer, worker, global lock, implicit async syntax or journal is installed.

The native normal/build dependency closure contains only the language crates and
registry libraries: no Leserpent command/domain/host contract, account service,
SQLite, async runtime or GUI toolkit. Cargo features are additive; another
dependency may enable `leserpent-profile` or `sqlite-journal`. Feature selection
is not a capability or security boundary. CI tests/lints each profile separately.

The new `tests/native_embedding.rs` exercises counter/integer and panel/string
schemas through helper compilation, nested captures, two reply-gated effects,
owned invalid/stale receipt rejection, independent owners and out-of-order roots
on one host, live policy re-entry, cancellation and terminal fuel exhaustion.
These are two scalar-host profiles, not complete
reference-language/group coverage, a real GUI adapter or durable recovery.
The full `tests/host_embedding.rs` acceptance proof remains planned and absent;
the four-crate source checkpoint now exports this native VM facade unchanged.
Complete independent VM and repository extraction gates remain open.

### Native Host Pipeline Matrix

`crates/leselang-vm/tests/native_host_pipeline.rs` supplies continuous atomic and
mixed-group program matrices through the public native facade. A stateful integer counter and
a headless panel model have different operation signatures and Rust reply types:
the counter returns a scalar; the panel returns a structured text/visibility
acknowledgement projected by the language. The atomic matrix uses
`compile_catalog_program`; the mixed matrix uses
`compile_catalog_prepared_group_helper_program`. Both reuse the production
compiler and `native::LocalVm`, not a fixture interpreter or hand-built IR.

The programs join bounded loops/folds, string lists, helper parameters/returns,
bindings and two effects whose successor arguments depend on the first receipt.
Counter calculation recovery and panel conditional field projections run through
the same evaluator. The acceptance matrix checks cold operation/signature/type/
version/grant refusal, retryable owned-reply rejection, predecessor receipt
rejection, policy changes at dispatch/acceptance/re-entry, four cancellation
boundaries and two isolated executions sharing one compiled IR. Every initial
fuel budget below observed completion is tested; the exact completion budget
ends at zero and one extra unit remains one. Idle polls and reply acceptance
do not refill or spend the original execution meter, and settled effects are
not replayed by these in-memory paths.

The mixed matrix executes atomic call -> first group -> captured atomic successor
-> second group -> final projection for both hosts. The first group is `seq`,
`all` or bounded flat `repeat`; the second is `seq` or `all`, giving twelve
host/program combinations. Runtime preparation, member collection and group
assembly use the existing `prepare_atomic_group_in_scope`,
`PreparedSequenceSession`, `PreparedParallelSession` and `assemble_group_reply`.
Native dispatch is deliberately explicit. Parallel replies arrive in reverse
order while result projection retains original declaration order. One parent
fuel meter covers preparation, correlation, capture, assembly and re-entry;
every smaller budget fails without refill or replay, and the exact budget succeeds.

Negative cases cover wrong group sites, reordered/missing members and stale root
receipts with unchanged rejected-input ownership; zero/partial/complete group
receipts and accepted-group cancellation; grant/revision revocation before an
atomic successor; and an invalid late group before the initial effect. Independent
execution domains can share the same compiled IR without accepting foreign member
receipts, even when the incoming envelope falsely claims the pending root label.
The unchanged sibling still completes after the other execution is cancelled.
These are bounded flat prepared-member profiles with per-execution fixture
correlation domains, not a new generic scheduler, nested parallel execution or
automatic dispatch deduplication.

An additional matrix covers sixteen conditional host/program paths: both hosts,
`seq`/`all`, both result-selected alternatives and early/captured returns. A
group-returning helper selects from the first reply, its group is aliased, and a
member projection either returns a scalar or drives one more captured helper.
The catalog compiler now accepts `field(value: member(value: alias, name: "right"),
name: "text")` both as a scalar binding and inside helper/host arguments. This
composes exactly Field + Member without a synthetic local or additional runtime
instruction. The public direct-reference projection helper keeps its original
restriction; arbitrary nested expressions or effects are not admitted by this fix.

Physical source, cold names, lexical prefix and two-node/one-depth output bounds
precede native member and field queries. The member observation moves into the
field query without Clone/Debug requirements. Unknown exports, opaque native
errors and unwind stop immediately. Unselected groups are still fully typed:
wrong argument types, unknown calls and different group modes fail before any
dispatch, but a well-typed division by zero in an unselected argument is never
evaluated. Wrong cold-site receipts retain their original buffer for retry;
cancelled or revoked accepted groups cannot escape through an early return.
Every smaller execution budget fails, the exact budget completes at zero, and
an extra unit remains unspent. These cases extend the same exported matrix;
they do not close complete-language or durable-recovery acceptance.

The retained [conditional projection export proof](fixtures/leselang_native_projection_source_export_macos_arm64_20261013.json)
records a macOS arm64 isolated source export with 250 files and 2,103 successful
test executions across the exported workspace and independent consumer. Both
run the same 20-test pipeline file byte-for-byte. Its source/manifest hashes
identify this checkpoint, not a claim that later edits were already verified.

The unchanged matrix is included in the isolated source workspace and external
consumer. A further 72 host/program/data combinations join single-effect helpers,
sequential/parallel group helpers and pure fallbacks into the same caller
continuation. Integer, boolean, string, none, optional string and string list all
survive a subsequent captured effect unchanged. Definition-local names cannot
capture the caller's same-named local. Wrong receipt identities preserve input
ownership; unselected sites reserve no root/member identities. Every smaller
fuel budget fails, exact fuel succeeds at zero, and extra fuel is not consumed.

Selected unused arguments still evaluate; unselected arguments remain lazy.
Cold return type, named argument, impure operand and operation errors fail before
dispatch, including in pure-fallback programs. Revocation after accepted atomic
or group receipts prevents the caller tail for all six data types. Another 60
cases cover absent versus present-empty optional strings, empty lists, 4096-byte
optional strings and 64-entry/4096-byte lists across all three paths and hosts.
Their exact/one-less fuel boundaries are tested. Computed one-byte/one-entry
overflow fails with fuel remaining and cannot dispatch the caller tail.
These are additional acceptance tests of the existing implementation, not new
language syntax, public APIs, storage or compiler/runtime contract versions.

The [selected data export proof](fixtures/leselang_native_selected_data_source_export_macos_arm64_20261013.json)
records the expanded 27-test pipeline in both isolated workspace and consumer,
with 2,117 successful test executions, strict Clippy and byte-identical copied
sources. It supersedes the smaller matrix as acceptance evidence, not as a full
language or durable-recovery promotion; the earlier checkpoint remains historical.

The fixture adapters own live leases, revisions, domains and dispatch;
native code is trusted, not sandboxed. These are protocol-model tests, not actual
GUI windows, cryptographic receipt authentication or durable exactly-once proof.
This closes the continuous two-host atomic slice and bounded mixed multi-group/atomic programs,
not the full language gate: complete reference-language parity, host-neutral
durable graph recovery and reference-adapter compatibility still require acceptance.
`host_embedding.rs` remains the planned
full proof, and `complete_vm` / `full_extraction_gate` remain false.

### Optional Reference VM Storage

`leselang-vm` now has an optional VM SQLite backend. The `sqlite-journal` feature
is enabled by default, preserving the existing durable APIs, journal schema,
restart/migration checks and all database-backed compatibility tests. An embedded
consumer that needs only the reference VM's in-memory path can use:

```toml
leselang-vm = { path = "../leselang-vm", default-features = false, features = ["leserpent-profile"] }
```

Consumers of the previous database-free reference API must now explicitly enable
`leserpent-profile`; without it, only the product-free native facade is compiled.
The default feature set is unchanged: `sqlite-journal` implies the reference
profile, preserving all released root API paths and wire/journal behavior.

`Vm::new` and `Vm::new_with_limits` reuse the existing ephemeral journal and
evaluator. Disabling the feature removes `rusqlite` and `libsqlite3-sys` from the
normal/build dependency closure, including the bundled native SQLite build.
`Vm::open_journal` and `Vm::open_journal_with_limits` are unavailable in this
configuration: there is no silent memory fallback or fake durable success.
Cargo features are additive; another dependency enabling `sqlite-journal` restores
the backend. This is a build/storage choice, not a capability or security boundary.

Database-dependent mixed/durable suites explicitly require `sqlite-journal`;
existing memory-only suites and `tests/ephemeral_profile.rs` run in both modes.
CI separately tests/lints the database-free reference VM and the default/all-feature
workspace. The new vertical covers typed helper/scalar wire compatibility,
reply-gated Seq with one saved budget, out-of-order All followed by captured
successor re-entry, forged images and terminal malformed replies, independent
local roots, cancellation/deadlines, retry authority, backpressure and compaction.
Memory state does not survive VM destruction, and serialized continuation data
alone does not restore a completed outbox. Hosts still own cross-VM routing and
current dispatch authority; equal local token text is not global execution identity.

Both reference storage modes enforce the same debugger cancellation audit rule:
a new command may attribute only a pending effect. An ordinary cancellation,
propagated sibling cancellation or cancellation already attributed to another
command cannot acquire a new audit after completion (`LSV4033`). Refusal preserves
the original terminal result, timestamp and audit. Exact replays match the effect,
command, principal-scoped idempotency key, origin, session and revision, returning
the original audit time rather than the retry time. Completed successes and
deadline cancellations remain non-cancellable (`LSV2014`). A stale waiting
projection in the debugger cannot convert a refusal into an Applied result.
`src/debugger_audit_contract_tests.rs` runs the same nine scenarios through
memory and SQLite targets; the durable target also closes/reopens the journal
with zero fresh fuel. Memory does not gain persistence. These audit identities and
policies remain product-adapter concerns, not host-neutral execution authority.

Audits share the retention lifetime of their effect or complete task group.
Compaction removes audits for pruned effects, including their command/idempotency
reservations, while preserving audits and exact replay for retained effects and
members of unfinished groups. The memory journal performs one audit sweep per
deleting batch, matching SQLite's existing cascading deletion; no-op or invalid
compaction does not discard audits. Tests cover bounded multi-batch cleanup,
mixed ordinary/audited cancellation, live-group protection, whole-group removal,
restart and key reuse without reviving pruned continuation identities.
Retention is host-controlled, not an automatic TTL or a permanent deduplication
registry. Hosts needing longer-lived audit evidence must retain it separately.

Audited cancellation also reconciles completed state against a shared journal,
as ordinary cancellation already does. A long-lived VM can replay an exact audit
committed for work admitted after its startup snapshot, observes peer success or
deadline cancellation before deciding whether a new audit is legal, and discards
cached completions removed by another VM. Audit identity and clock validation
still run first. Unknown pending work is not adopted and known pending images
still require exact correspondence; observing a terminal result is not a new
execution grant. The final SQLite transaction remains authoritative if another
worker completes or compacts between observation and submission. Dedicated durable
regressions cover zero-fresh-fuel observers, external pruning/key reuse and eight
two-thread races: identical commands replay one committed timestamp; competing
commands cannot create a second attribution. These are same-machine shared-file
tests, not multi-host or process-crash acceptance.

Terminal observation distinguishes an absent record from live pending work and a
completed result in one journal lookup. If another VM completes and prunes an
effect before a local pending snapshot sees its terminal result, resume,
acknowledgement, cancellation and audited cancellation discard that stale entry
and refuse with `LSV2004`. Error reporting also drops the stale cache entry but
retains the journal's missing-lease refusal `LSV4021`. A pruned terminal result is
not recreated or inferred from a supplied receipt. The fast path for an already
cached terminal still checks existence without decoding the payload again.

Pending observations preserve exact-image checks and do not adopt unknown work.
Read failures and invalid durable state (`LSV4007`) preserve local pending state
for investigation/recovery rather than masquerading as retention. Unfinished
groups remain protected; once the whole group is pruned, cached-completed and
cached-pending branches are each forgotten on observation. `pending_count()` and
`pending_continuations()` remain local snapshots: claims, reads and compaction do
not promise a global refresh of every pending cache. A completion/retention race
after observation may still be refused by the final transaction; callers can
retry observation, never blindly replay the external effect.

The memory backend can also report an untracked effect: legacy image-only
`restore()` does not create an outbox row. This is not authoritative absence and
does not invalidate the VM's local restored continuation. It preserves exact
image checks, zero-fresh-fuel completion and terminal replay without introducing
an envelope, dispatch lease or implicit admission for unknown tokens.

This removes mandatory VM SQLite linkage, not product DTO coupling. The reference
VM still depends on Leserpent command/result contracts and recovery frames when
`leserpent-profile` is enabled. Native facade isolation does not migrate those
reference ownership/recovery models into host-neutral execution.
It is not the complete product-free independent VM, optional host-neutral durable
backend or repository extraction gate.

### Shared Owned Capture Data

The reference VM now uses runtime-core's `CapturedBindings<Body, Result, Group>`,
`ScalarBinding`, `ProjectedBinding<Result>`, `ProjectedGroupBinding<Group>` and
`ProjectedGroup<Result>` as its exact five legacy capture types, without conversion
wrappers or a second saved-data model. The body and result/group schemas are
explicit native parameters; two unrelated schemas and move-only thread-local
payloads work without adding Clone/Debug/serde/Default/Send/Sync bounds at construction.
Derived traits and thread ownership are conditional, not universally promised.

These containers are unchecked mutable data, not executable continuations,
authentic receipts, authority grants or a host-neutral durable recovery protocol.
Their closed serde containers preserve optional-array omission/defaults and all
required fields, but the host still bounds ingress and validates names, counts,
types, schema/version, original image and live authority before re-entry. Native
Debug/serde/Drop remain host code; payload diagnostics are not redacted. The private
reference adapter retains its original lexical/projection/group checks and schema
selection. Frozen capture JSON, original buffer ownership, actual successor
re-entry, forged-frame rejection and exact journal bytes across restart are tested
with unchanged fuel and terminal receipt replay. Product `Computation` stays in
the reference HIR; effect-specific recovery semantics remain reference-owned. Its
`ProjectedResult` and `ResultBinding` specializations are now owned by the
command adapter.
This closes capture DTO ownership, not the independent VM acceptance gate.

### Product Capture Validation Entry

The command adapter's `validate_result_binding()` is the complete product
data/type entry: structural capture admission, closed-field observation when
results/groups are saved, then canonical lexical/type reconstruction. Explicit
`ResultBindingLimits` carries capture/name, group-member and access-walk policy;
the trusted name predicate remains caller-supplied. HIR safety ceilings still
apply. Scalar-only captures preserve reverse literal wrapping and do not use
projection-observer limits. The lower-level `validate_result_binding_structure()`
is deliberately weaker and does not validate lexical scope or result types.

The VM delegates both gates, retaining its original limits, name grammar, schema
selection and `LSV1405` mapping. Canonical reconstruction still makes temporary
owned IR after earlier admission; input captures and wire are never mutated.
The 70-operation/five-version typed-field matrix, scalar/result/group cases,
name-callback refusal/unwind order and exact core/adapter/VM buffer proofs cover
this boundary. Callers bound pending effects and aggregate ingress before entry,
and still authenticate original images/receipts, authority and durable state.
No new budget, dispatch, replay grant or independent recovery implementation is
provided by this data validator; complete repository extraction remains open.

### Shared Structural Capture Validation

The real reference VM delegates its saved-frame and projected-group structural
paths to runtime-core's `validate_captured_bindings()` and
`validate_projected_group()`, through the command adapter's
`validate_result_binding_structure()` and `ProjectedGroupExt`. Explicit inclusive
policies keep name byte limits,
saved-count limits and native group syntax outside automatic DTO construction.
Zero limits are not repaired; an inverted group interval rejects every group.
Frame order is count, pending name, all scalar rows, result rows, group rows, body.
Each result/group row keeps name-before-native interleaving; body runs last once.
Group order is count, all original names and exact uniqueness, then ordered
member result hooks. Invalid name syntax precedes duplicate detection at that row.
Count/pending/scalar refusal runs no native hook; early group count/name refusal
runs no member result hook. Nothing normalizes, copies or reorders saved data.

`CaptureStructureError<External>` keeps native errors unchanged with their original
positions; its payload-free Debug/Display never invoke native formatters. Native
data/errors need no Clone/Debug/serde/Default/Send/Sync bounds. Trusted callbacks
can mutate interior state or unwind; no callback preemption or rollback is promised.
Success is an instant data check, not a lasting certificate or execution grant.
These explicit validators do not authenticate receipts, original images or schema
identity, enforce complete lexical masks or define native operation domains.
The adapter retains those checks, original fault mapping, result-flow checks,
saved fuel and durable replay ownership. Frozen wire, malformed/forged frames and
exact journal bytes across restart remain covered in both reference build profiles
where applicable. This is not product-free execution or repository extraction.

### Shared Captured Scope Entry

The real reference VM delegates restored-projection scope assembly through the
command adapter's `validate_captured_projection_accesses()` to the shared
`with_captured_scope()`. This borrows original saved names on a nested `ScopeFrame`
of the caller's original stack and moves explicit native scope values in scalar,
result, group, pending-name order before one body hook. Each native value hook
runs once before its row's duplicate check, preserving preparation/insertion
interleaving without an extra preflight pass. All names share one namespace;
collisions with the visible parent prefix and earlier capture rows stop later
hooks. Nested capture entry keeps outer names live and restores its own suffix.

Normal return, refusal and unwind restore the original suffix boundary, not
native interior mutations. Native data, scope values, outputs and errors need no
Clone/Debug/serde/Default/Send/Sync bounds. `CaptureScopeError<External>` reports
original positions and retains native errors unchanged; payload-free Debug/Display
never invoke native formatters. Drop is trusted and destructor unwind propagates;
a second cleanup panic can abort. This is not callback preemption or rollback.
Outputs and native errors remain locally owned until scope cleanup finishes;
destructor unwind releases them rather than leaking a prematurely moved return.

This helper does not validate existing prefix uniqueness, capture bounds, native
schemas, authentic receipts or durable authority. Hosts admit input and callback
work first, validate names/types, charge their original meter and own execution.
Scalar names occupy slots without granting fields in the reference adapter;
operation masks, version-closed projections, cold-branch field policy and the full canonical
type checker remain product policy. No syntax, continuation/journal bytes,
`LSV1405`, saved fuel or once-only terminal replay behavior changes. Scope entry
is not a saved execution frame or an automatic execution grant, and is not the
complete independent VM/repository extraction gate.

### Shared Projection Access Walk

`leselang_hir::projection_access::observe_projection_accesses()` is available
without the reference profile and now drives the real VM's restored-field checks.
It walks the shared host-parameterized IR, preserving ordered argument/group
children, both conditional branches, recovery fallbacks and zero-iteration
loop/fold bodies. An iterative physical node/depth preflight includes cold
branches and completes before any native hook; inclusive host limits cannot
exceed the HIR safety ceilings. It does not evaluate conditions or dispatch work.

`ProjectionAccessPolicy` explicitly owns local observations, field admission,
opaque host/call/member metadata and conditional joins. Observation, native IR
slots and errors need no Clone/Debug/serde/Default/Send/Sync bounds. Original names
are borrowed on the caller's scope; temporary suffixes are restored on success,
refusal and unwind. Owned outputs/errors stay local until suffix cleanup finishes.
Native hooks, interior mutations and destructors remain trusted; no callback
preemption or rollback is provided. Redacted diagnostics retain original native
errors and their phase without invoking native formatters.

The command adapter now owns version-closed field masks, field intersection,
group member operation matching and member fallback through
`validate_captured_projection_accesses()`. The VM supplies its original node/depth
ceilings explicitly to the composed adapter entry, which validates captured
structure before observation and runs canonical scope/type validation afterward.
The observer builds only temporary
scope entries and masks, with no copied payloads, data repair or dispatch. All
70 operations across v1-v5 and 12 fields cover saved locals, group/member aliases,
live calls and conditional intersections without widening old vocabularies.
The complete canonical type
checker remains required: unknown/scalar-only locals, native payloads, name grammar,
scalar bounds, schemas and execution authority are not validated by this observer.
Success is an instant observation, not a cached certificate or execution grant.
Legacy `LSV1405`, frozen wire, saved fuel, receipts, journal restart and once-only
successor replay remain unchanged. This is not the complete independent VM or
repository extraction gate. Native-only and separate-workspace consumers exercise
unrelated borrowed and move-only schemas without product field masks or SQLite.

### Shared Stateful Capture Re-entry

`with_captured_scope_state()` supplies `CaptureScopeHooks` with sequential reborrows
of one original caller-owned mutable context. Scalar, result, group, pending and
body hooks share it without a context clone/reset, new meter, internal mutable
wrapper or serialization. The existing closure-only API delegates with unit state.
Native contexts may be unsized, move-only or GUI-thread-local; no extra
Clone/Debug/serde/Default/Send/Sync bounds are required. Stable capture data can be
borrowed into outputs; temporary context reborrows and scope-owned values cannot.

The real reference VM now routes atomic and group execution re-entry through this
shared entry. Scalar/projection preparation and body evaluation charge the original
Fuel with unchanged costs, including preparation before duplicate-name refusal.
Failed charges are atomic, earlier successful charges are not refunded, and scope
cleanup does not roll back native context mutations. Native errors keep their
original ownership and positions. Uncalled once hooks and remaining mutable hook
captures are released while outputs/errors are still locally owned; destructor
unwind propagates. Native callback-internal cleanup remains the callback's
responsibility, and a second cleanup panic may abort. There is no preemption.

The adapter must still admit capture bounds, native schemas, canonical types,
authentic receipts and durable authority before entry. The native group contract
continues to reject saved result/group rows rather than granting new group scope.
Supplying a mutable context is not a saved execution frame or execution grant.
Frozen pre-refactor comparisons cover every original fuel boundary, duplicates,
body faults, effect tails and successor capture; real memory restore with a fresh
VM retains saved fuel and once-only terminal replay. Existing journal/restart
proofs remain required. Syntax, frozen wire, error codes and replay authority are
unchanged. This is not a complete independent VM or repository extraction gate.

### Shared Stateful Capture Snapshot

`snapshot_captured_scope_state()` prepares the complete visible lexical prefix,
including parent bindings, without mutating it. Explicit inclusive
`CaptureSnapshotLimits` preflight the count, pending name and every saved name's
UTF-8 byte length before allocation or native hooks. Native name grammar,
uniqueness, scalar bounds, payload limits, schemas and execution authority are
not implied by these storage ceilings.

`CaptureSnapshotHooks` prepares each row once in original lexical order, then
materializes the body once after all rows. `CapturedValue` moves native payloads
into the scalar/result/group arrays; each array preserves its original order.
Names are explicitly copied for owned capture data. The original mutable context
and Fuel may be unsized or GUI-local, with no extra native trait bounds, new meter
or implicit accounting. Adapters own scalar copying, projection snapshots, body
materialization and exact charge order. Successful charges are not refunded and
native mutations are not rolled back after refusal/unwind.

Partial rows, uncalled body and remaining row-hook captures are released while
outputs/errors stay local. Callback-internal cleanup and destructors remain
trusted; unwind propagates and a second cleanup panic may abort. The real reference
VM now delegates capture construction and re-entry to the shared core. Frozen
comparisons cover every fuel boundary, mixed lexical/category order, exact capture
bytes and native projection refusal precedence. Existing saved-fuel memory and
journal/restart/once-only replay proofs remain required. Single-request restore
cannot recreate reference group ownership: `LSV1409` still requires the complete
group journal, even when captured data are present. Native schemas, canonical
types, authentic receipts and durable authority remain adapter-owned. A successful
snapshot is mutable data, not an execution grant or durable handle; no dispatch,
rollback or serialization occurs. This is not a complete independent VM or
repository extraction gate.

### Shared Owned Prefix Cleanup

`with_owned_scope_state()` consumes one complete lexical prefix and reborrows
the original caller context. Keys remain borrowed and values are moved; the context
may be unsized or GUI-local without extra native traits or an internal mutable
wrapper. The body may add nested/suffix values. Outputs/errors stay local until
suffix and then prefix cleanup finish, including on refusal or destructor unwind.
Charges and native mutations are not refunded or rolled back. Name/type/bounds
and authority admission remain adapter-owned; owned bindings are not a grant.

The shared HIR uses this entry for initial execution and single/stacked accepted
re-entry. Remaining captures, replies, return frames and original identities also
finish framework-owned cleanup before output/error handoff. Opaque cold refusals,
projection errors and prepared successors stay owned during that cleanup. Session
correlation refusal likewise releases its unused continuation while the error is
still local. A cleanup unwind keeps HostUncertain sealed: no completion, new
publication, refund, rearm or automatic retry is permitted.

Native callbacks, callback-internal cleanup and destructors remain trusted;
unwind propagates and a second cleanup panic may abort. This is framework-owned
cleanup ordering, not callback preemption or a transaction around native effects.
The original cold admission, fuel debit order, canonical types, reply authority,
frame quotas and wire/journal/replay rules are unchanged. Regression proofs check
exact destructor counts for leaked native errors and unpublished successors,
multiple unused return frames, owned prefixes and GUI-local original meters.
This is not a complete independent VM or repository extraction gate.

### Shared Committed Checkpoint Recovery

`CheckpointRecovery<Key, Pending, Completed>` stages already-decoded native rows
without exposing partial maps. Inclusive record and sequence ceilings precede
native key comparison; duplicates across pending/completed partitions reject the
whole pass instead of overwriting a row. Refusal or comparison unwind closes the
pass before cleanup. No Clone/Debug/serde/Send/Sync bounds are imposed on native
data; keys alone require Ord. Sequence values are not global execution identities.

`finish()` borrows the complete `RecoveredCheckpoint` for one final host-owned
graph validation and storage commit. Owned host state and uncalled callback
captures finish cleanup before the snapshot, original commit metadata or native
error is handed off. Failed commit discards staged maps. Cleanup unwind cannot
publish the snapshot; a successful commit followed by unwind is uncertain, not
permission to replay effects. Native comparison, callbacks, callback-internal
cleanup and Drop remain trusted; no preemption or automatic rollback is supplied.

The real reference SQLite loader uses this boundary inside its original immediate
transaction, retaining bounded decoding, row/schema/deadline checks, dispatch,
merge and audit validation, watermark repair, fault codes and frozen wire.
Two unrelated consumer-owned byte formats prove saved budgets and completed-row
partitioning without product crates or SQLite. Consumer serialization/version and
live authority remain explicit host policy. These are storage protocol proofs,
not a complete native durable evaluator or automatic restart authority.

Recovered maps are mutable data, not authenticated receipts or execution grants.
The adapter must hold the same storage snapshot/transaction across loading and
finalization, admit aggregate bytes/graphs, verify original frames and current
authority, and own dispatch/replay/migration. Complete native saved-graph execution
policy and reference-adapter compatibility remain extraction blockers.

### Shared Saved Frames And Budgets

`ContinuationFrame<Token, Revision, ResultType, Effect, Binding>` now owns the
saved envelope in runtime-core; `ResourceBudget` owns its original accounting
observations. The reference `ContinuationImage` is an exact generic specialization,
not a copied DTO, and its budget is an exact re-export. Frozen legacy field order,
optional omission/defaults and validation/error precedence remain compatible.
Construction adds no native Clone/Debug/serde/Default/Send/Sync bounds; derived
traits stay conditional and native Debug/codecs remain trusted and unredacted.

`resource_budget()` only copies metadata. `validate(ResourceBudgetLimits)` checks
explicit inclusive ceilings in fuel, relative deadline, absolute deadline, output
order. Zero fuel/output is valid; relative and present absolute deadlines must be
nonzero. No allocation, native work, normalization, saturation, clock check or new
fuel grant occurs. Both containers remain mutable unchecked data, not authentic
receipts, executable handles or permission to enter a saved body. Original-frame,
schema, captured scope, live authority, bounded codecs/storage and replay admission
must precede host-owned meter reconstruction.

The separate `durable_control` consumer compiles parsed counter and text-panel
sources through the actual native compiler and effect walker, saves owned captures
as its own bytes, recovers checkpoint partitions and a sequence watermark, and
enters restored lexical state with the original remaining fuel. Successor budget
and revision policy are inherited rather than reset. Success and accepted-reentry
refusals are settled before receipt replay; replay never invokes or prepares an
effect again. Forged fuel/body data, stale identity, wrong reply domain, revoked
capability and deadline refusal do not enter the body. These are consumer-owned
byte-codec/journal model proofs, not a production durable backend, crash-atomic
external-effect transaction, generic saved-graph evaluator or complete independent
VM/extraction gate. The adapter owns storage authentication and dispatch semantics.

### Shared Saved-Body Execution

`resume_captured_effects_in_scope()` now joins saved scalar/result/group restoration,
reply binding and the actual shared effect walker in one HIR entry. Both the real
reference VM's atomic/group captures and the parsed independent consumer use it;
the consumer no longer supplies a body-evaluation callback. `CapturedEffectHooks`
only map saved values and the pending reply using the original caller-owned Fuel.
Native saved slots, IR operations, requests and captures need no Clone/Debug/serde/
Default/Send/Sync bounds; result views retain the existing evaluator Clone contract.

The original row order and preparation-before-duplicate debit precedence remain:
scalar, result, group, pending, then body physical/scope and all cold effect checks,
then execution of the exact borrowed saved body. There is no extra early pass,
meter reconstruction/refill or callback retry. Scope suffix and remaining hook
owners are cleaned before outputs/errors escape. Scalar failures stay typed;
native lookalikes remain external. Cleanup unwind releases unpublished requests
and captures without refund or replay; a second cleanup panic may abort.

This executes already-admitted data, not admission or reply acceptance. Adapters
must first bound codecs/aggregate storage/native work and validate original program,
capture/schema, current authority, authentic replies and durable state. Existing
borrowed accepted-session admission order is unchanged. The host still owns receipt
settlement, dispatch, crash uncertainty and saved group/graph journal recovery;
this is not a complete independent durable VM or repository extraction gate.

### Restored Native Return Stacks

`SavedEffectReturnFrame` is unchecked host-decoded data: a borrowed name/body and
an owned native capture, ordered innermost to outermost. The new
`restore_effect_return_stack()` entry checks the inclusive frame quota, valid
names and one shared structural budget for the complete body forest. Equal names
in different lexical frames remain valid. Zero frames, invalid limits, malformed
names and aggregate node/depth overflow reject before restoration or host queries;
native capture owners are consumed without Clone/Debug/serde requirements. No callback, evaluator,
fuel grant, codec or journal is introduced by restoration.

The resulting stack uses the existing `PendingReply` and
`resume_accepted_stacked_effects()` lifecycle. All saved cold bodies pass current
host checks before any prefix restoration, the accepted reply is projected once,
and outer frames receive computed values rather than fabricated receipts. New
suspensions and remaining outer frames keep the original shared return quota.
This also preserves existing cancellation, refusal, ownership and unwind behavior.

The exported `durable_control` model now compiles counter and text-panel helpers,
serializes three pending return frames, discards the live source/IR and restores
each successor from host-owned checkpoint bytes. Three correlated effects finish
without recompilation; replay of settled success/refusal never prepares or invokes
work again. Every smaller starting budget fails, the exact budget ends at zero,
and one extra unit remains one. Forged frame order/body/fuel, stale identity,
wrong reply domain, revoked authority and deadline rejection do not restore values.
A denied outer cold body settles without restoring/projecting the inner reply.

Structural success does not authenticate saved sites, order or captures. The host
must validate the original committed graph, aggregate decoded bytes, frame schema,
resource metadata and live authority before allowing execution. Snapshot codecs,
settlement and external dispatch remain host-owned. These are byte-codec/journal
model tests, not a production durable backend, process-crash proof, saved group
graph recovery or an exactly-once external-effect guarantee. Full extraction stays
open; legacy reference continuation/journal schemas are unchanged.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_stack_source_export_macos_arm64_20261013.json)
records 2,131 test executions and strict lint success across the exported workspace
and independent consumer. Both use the same saved-control and return-stack tests
byte for byte; full VM/extraction flags remain false.

### Restored Parallel Reply Barriers

`PreparedParallelSession::restore()` imports an already-published flat `all`
group as ordered `SavedParallelMember` rows. Each row is either awaiting its
original identity/domain or contains an `AcceptedReply` obtained through the
existing pending-reply protocol. Complete physical shape, count and row order
precede native identity comparison; duplicate identities reject before the
mandatory whole-batch admission callback. No sorting, partial group or missing
member is silently repaired.

The returned session uses the same private `ParallelReplyCore` as live execution.
It starts awaiting the remaining replies or ready to hand off all accepted rows,
never ready to prepare requests. No preparation, dispatch, new meter, automatic
fuel debit/refill or retry occurs. Existing wrong-reply rejection, out-of-order
acceptance, declaration-ordered once-only results, scoped cancellation and unwind
fences remain in force. Later group assembly still charges the original budget.

The native tests exercise all eight completion masks on a three-member group
with numeric and text payloads, inclusive 64-member bounds, unsized domains and
move-only ownership. Two byte-codec/journal models also discard the live group,
restore each remaining receipt from saved IR/rows and retain the original fuel.
They prove no extra preparation/dispatch and settled-result replay without
reopening the barrier. These are model tests, not process-crash acceptance.

Restoration is not receipt authentication or durable authority. The host must
validate the same committed graph, exact row/domain/identity correspondence,
accepted payload provenance, current policy/deadline, aggregate decoded storage,
saved budget and dispatch/settlement state. Previously accepted data needs fresh
admission, and cancelled or settled work must never be rearmed from an old image.
Native callbacks/equality/cleanup remain trusted bounded work. A host may charge
that work to its original meter; the core cannot preempt it or roll it back.
Complete mixed-group continuation recovery, production storage and
crash-atomic settlement remain open. This is not a complete independent durable
VM or an exactly-once external-effect guarantee.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_parallel_source_export_macos_arm64_20261013.json)
records 2,149 test executions, strict lint success and the same nine saved-parallel
tests copied unchanged into the independent consumer. Full extraction remains open.

### Restored Sequential Frontiers

`PreparedSequenceSession::restore()` accepts a complete original `seq` as
`SavedSequenceMember` rows: a contiguous accepted prefix, at most one awaiting
member, then correlated but unpublished requests. It derives the cursor from
these states instead of accepting a caller-supplied position. Complete shape,
count, index/name and frontier checks precede native identity comparisons;
duplicate identities across all three regions reject before mandatory whole-
snapshot admission. Gaps, multiple outstanding members and accepted suffix rows
cannot be silently repaired or converted into new work.

Restoration returns the existing `SequenceReplyCore`-backed session plus its
original accepted prefix for host-owned aggregation. Awaiting work is not
republished. Each queued request still passes the existing fresh poll admission,
and only one request can be outstanding. An all-accepted snapshot restores a
Completed session, never a Ready cursor past the last member. Cancellation or
native unwind releases pending/queued ownership without reclaiming the already
returned prefix. No payload copy, operand preparation, correlation, meter grant,
automatic retry or dispatch is introduced. Normal reply refusal keeps the exact
input and frontier; original scoped lifecycle and terminal fencing stay shared.

Numeric and text payload tests cover all seven valid three-member frontiers and
all twenty invalid state arrangements, preserved pointer ownership, 64 members,
unsized domains, duplicate identities, fresh policy and cleanup unwind. Two
consumer-owned byte-codec/journal models discard the live group between steps,
restore the saved IR and original queued requests, and publish each member once.
The original remaining fuel pays for final group assembly, ending exactly at
zero; settled replay does not reopen the session. This proves model recovery,
not process-crash acceptance or transactional external effects.

The host still authenticates the same committed graph, request/domain/identity
correspondence, accepted receipt provenance, publication/settlement metadata,
saved budget and current policy/deadline. Old cancelled or settled snapshots
must never be rearmed. Native callbacks and destructors are trusted bounded work;
the core does not preempt, roll back or persist them. Complete mixed-group/helper
continuation recovery and production crash-safe settlement remain separate gates.
Flat Seq/All restore entries are not a full independent durable VM or extraction
completion claim.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_sequence_source_export_macos_arm64_20261013.json)
records 2,167 test executions, strict lint success and all nine saved-sequence
tests copied unchanged into the independent consumer. Full extraction remains open.

### Composed Native Mixed Recovery

The unchanged exported `native_host_pipeline` now joins saved Seq/All collectors
with `restore_effect_return_stack()` and the original stacked walker. Parsed
counter and structured text-panel programs pass through two enclosing helpers,
an atomic call, a `seq`/`all`/`repeat` group, a captured atomic successor and a
second group. Every suspension retains at least three return frames. During
group completion the stack exists only as host-owned saved frames; the old group
controller is dropped before its replacement is admitted.

The two-member groups cover all five sequential frontiers and four parallel
completion masks. Their Cartesian combinations give 252 two-host paths, each
with exactly six dispatches and eight original identities. Results and remaining
fuel match uninterrupted execution; both group orders also check every smaller
fuel budget, the exact budget and one extra unit. Awaiting Seq rows never
republish, accepted All rows need no new reply, and missing All replies arrive
in reverse order. Wrong root identities preserve the same native receipt buffer
and pending return stack for valid acceptance.

A bounded host inventory owns original move-only requests behind checked handles.
Restoration rechecks root/member identities, native domains, accepted values and
queued request correspondence. Stale revisions, revoked permissions, closed
execution scopes and redirected suffix handles reject before the captured tail.
No compiler, evaluator, group controller implementation or language API is added.

This is an in-memory composition proof, not serialized mixed-graph recovery.
Original IR/site plans, captures (including group views), native inventory and
remaining fuel survive in the host; the fixture explicitly copies accepted
parallel payloads into its own bounded snapshot. The core gains no Clone/serde
requirement, codec, settlement store or replay authority. Portable mixed snapshots,
production storage authentication, process-crash uncertainty and transactional
external-effect settlement remain separate gates. Full extraction remains open.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_mixed_source_export_macos_arm64_20261013.json)
records 2,175 test executions and strict lint success. Each workspace runs the
same 31-test native pipeline, including four composed recovery tests; all 244
copied source files match the repository byte for byte. Portable-checkpoint and
process-crash acceptance flags remain false alongside full VM/extraction flags.

### Restored Native Call Operands

`PreparedCall::restore()` rebuilds a call from host-decoded `PreparedArgument`
rows against an explicitly selected schema. Rows must remain in declaration
order, preserve original submission positions as a complete permutation, and
match the declared names. Inclusive argument/parameter ceilings, every row and
scalar bound, then the shared named-signature check precede native callbacks.
The mandatory whole-call host admission runs before the existing scalar-domain
checker in declaration order. Missing required arguments reject; omitted optional
arguments are not filled or renumbered. The inclusive maximum is 64 operands.

Values move without cloning their buffers. Names rebind to the selected schema,
so decoded name storage can be dropped immediately. Native operation/result/
capability metadata needs no Clone/Debug/serde bounds. Rejection carries closed
positions or the original native error; formatting does not expose payloads.
Unwind releases input and callback ownership without retry or rollback. No
argument expression is evaluated, fuel created/refilled, identity allocated or
request dispatched by this entry. The live preparation path is unchanged.

The exported `saved_call` proof covers all six scalar kinds, moved buffer/name
ownership, optional omissions, malformed late rows, bounds, native refusal and
unwind. Its numeric and text byte-codec models prepare actual native calls,
serialize the queued operands and flat Seq graph, and drop the original request,
schema, IR and collector objects before decoding. All seven three-member
frontiers resume with fresh schemas through the original Seq restore/poll/reply
path. Each program still prepares and dispatches exactly three calls, and saved
fuel pays for the original group assembly. Changed operands/operation/fuel fail
the fixture store's exact-byte check; revoked authority also rejects after
restoration but before queued publication.

Structural/domain success cannot authenticate prior evaluation or original
operation/schema correspondence. The host must bound aggregate decoding/storage,
authenticate committed operands and graph, preserve the original resource budget,
check publication/settlement state and recheck current policy before dispatch.
Do not rearm already-published, cancelled or settled work. These are byte-codec
model tests, not a production durable backend or process-crash acceptance.
Portable mixed helper/group captures, durable settlement and full extraction
remain open; no runtime wire format or product continuation schema is changed.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_call_source_export_macos_arm64_20261013.json)
records 2,193 test executions, strict lint success and all nine saved-call tests
copied unchanged into the independent consumer. All 246 copied source files match
the repository byte for byte. Complete VM, portable mixed checkpoint, process-
crash acceptance and full extraction flags remain false.

### Restored Assembled Group Values

`restore_group_reply()` handles an already-assembled saved Seq/All value, not a
new batch of completed receipts. It shares live assembly's borrowed cold checks:
physical/language bounds, atomic candidates, complete count, original order and
exact branch identity. Only then does mandatory whole-group native admission
run. Rows may be rebound to freshly decoded IR using the completed saved Seq/All
collectors. No new row format or alternate collector is introduced.

Restoration does not charge assembly again, create/refill fuel, reevaluate calls,
prepare operands or dispatch requests. The host must authenticate that assembly
was already committed with the original budget. Merely completed receipts still
use `assemble_group_reply()` and pay its normal root-plus-members charge. Native
decoding/admission work and aggregate memory remain separately host-bounded.
This distinction is a mandatory host contract, not inferred from pointers or an
accepted wrapper. Original graph, identity, domain/value provenance and current
policy must also be checked; later lookup still requires fresh selected-row
admission. Native inputs remain move-only and unsized domains are supported
without Clone/Debug/equality/serde/Send/Sync bounds on the restore entry.

The unchanged exported `saved_group` proof covers cold failures, inclusive
64-member limits, moved buffers, zero remaining fuel, owned native errors,
callback cleanup, unwind and authority/domain changes after acceptance. Two
host-owned numeric/text byte-codec models drop all original IR, schema, receipt,
collector and meter objects before decoding. Both Seq and All restore under
three saved-budget boundaries, including zero, with no repeated preparation or
dispatch and no second assembly debit. The model's committed-byte comparison
rejects forged graph/value/identity/fuel and incomplete assembly before rebinding.

This restores a flat assembled value only. It does not install captured bindings,
authenticate storage itself, resume a complete mixed helper/group checkpoint,
provide a process-crash transaction or promote the full extraction gate.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_group_source_export_macos_arm64_20261013.json)
records 2,209 test executions and strict lint success. All eight saved-group tests
run unchanged in the independent consumer, and all 248 copied source files match
the repository byte for byte. Complete VM, portable mixed checkpoint, process-
crash acceptance and full extraction flags remain false.

### Byte-Backed Captured Re-entry Composition

The `saved_group::captured` consumer model now joins the existing snapshot,
saved-group, saved-stack and accepted-reply interfaces instead of adding another
restoration API. A host-supplied initial scope contains a scalar, an atomic result,
one completed three-member group and an alias to that same group. The actual
typed-IR walker prepares one further call and suspends under two return frames.
The fixture stores the returned scope snapshots, group graph/receipts, incoming
pending reply and original remaining budget as host-defined bytes, then drops all
original runtime, IR, schema, meter, collector and native view objects.

After bounded exact-committed-byte admission, fresh objects use
`restore_effect_return_stack()`, `PendingReply` and
`resume_accepted_stacked_effects()`. Complete cold body-forest admission precedes
native capture materialization. Saved collectors plus `restore_group_reply()`
rebuild the completed group once; both aliases and both frames share that new
view. Every restoration and selected member/field read rechecks current policy.
The pending reply projects once, not once per return frame. No old request or
result inventory is retained, and no new interpreter, dispatch path or scope
wire schema is introduced in the library.

Six additional tests cover numeric/text hosts and both Seq/All group kinds,
uninterrupted result and exact fuel parity at every continuation cutoff, outer
cold refusal before any capture work, captured-policy denial independent of the
pending receipt, revocation on the second return despite a cached group, forged
scope/alias/frame/body/budget data, and unchanged wrong-identity input for explicit
retry. Four original preparations/dispatches comprise the three group members and
the pending call; reconstruction performs none. The initial seed result is fixture
input, not a separately dispatched program operation.

This is a fixed typed-IR capture model, not general compiled-source checkpoint
coverage. The host owns codec authenticity, original capture/site correspondence,
native copy/ingress limits and settlement. The fixture bounds bytes to 64 KiB,
frames to two and the shared group to one three-row rebuild. These native storage
costs are separate from interpreter fuel; original capture and re-entry charges
are unchanged. This completed-only model does not combine partially completed
groups or unpublished operands; the sequential extension below adds those two
boundaries. Multiple captured group graphs and durable completion remain open. Full
portable mixed-checkpoint, production process-crash and extraction gates stay open.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_saved_capture_source_export_macos_arm64_20261013.json)
records 2,221 test executions and strict lint success. The unchanged saved-group
suite now runs 14 tests in each workspace, including six capture-composition
tests. All 248 copied source files match the repository byte for byte. Its
completed-capture model flag is true; source-compiled checkpoint, full portable
mixed-checkpoint, process-crash, complete VM and extraction flags remain false.

### Captured In-Flight Sequence Recovery

`saved_group::captured::inflight` combines the existing byte-backed capture model
with `PreparedCall::restore()` and `PreparedSequenceSession::restore()`. The
shared typed-IR walker prepares a three-member Seq whose arguments read members
of a previously completed captured group, then suspends under two return frames.
The two group executions use distinct identity generations and result-domain
objects, so restoring the first cannot accidentally authorize the second.

All seven accepted/awaiting/prepared frontiers run with numeric/text payloads and
a captured Seq or All: 28 combinations. Only committed bytes survive the initial
execution scope; the original graph, native views, prepared calls, schema objects,
collectors and meter are dropped. A fresh adapter reconstructs only unpublished
calls from their saved scalar operands. Accepted rows remain accepted, awaiting
work consumes its original incoming result, and neither is dispatched again.
Across the model's two phases there are exactly six original preparations and
six total dispatches. Recovery performs zero preparations and publishes only
the previously unpublished suffix.

Four tests cover those frontiers, every assembly/return fuel cutoff, current
authority refusal before request restoration and again at queued publication,
outer cold refusal before capture materialization, second-return capture
revocation, unchanged wrong-root input, and forged operands/frontier/captures/
graph/budget. Completed captured assembly is restored without a second charge;
the unfinished Seq still pays its normal four-unit assembly charge. The captured
group is rebuilt once and shared across aliases/frames; the new group projects
once. Outer cold return refusal occurs after the pending group finishes, not
before its already-authorized member dispatches.

This remains a fixed typed-IR host-codec model with a 64 KiB byte limit, two return
frames and two three-member graphs. Exact committed-byte correspondence stands
in for host persistence authenticity, not a production signature or journal.
Native decoding/copying is separately bounded host work. The parallel extension
below adds partial All composition; repeat, multiple captured graphs and
source-compiled mixed checkpoints remain separate. Production
process-crash settlement and full extraction remain open. No library wire schema,
interpreter, runtime API or dependency was added for this proof.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_inflight_capture_source_export_macos_arm64_20261013.json)
records 2,229 test executions and strict lint success in the exported workspace
and independent consumer. The unchanged saved-group suite runs 18 tests in each,
including four in-flight composition tests; all 248 copied files match the
repository byte for byte. The sequential-capture model flag is true while full
mixed-checkpoint, partial-parallel composition, process-crash, complete VM and
extraction flags remain false.

### Captured Partial Parallel Recovery

The `saved_group::captured::inflight::parallel` model now joins partial All
completion with the same captured variables, group aliases and two return frames.
The common adapter prepares real native call operands by reading the captured
group; `PreparedParallelSession::start()` publishes the complete batch once.
Eight accepted/awaiting masks and six receipt orders run with two payload schemas
and captured Seq/All groups: 192 combinations. The captured and pending groups
retain distinct identity generations and result domains.

Only bounded committed bytes escape the first execution scope. Original graphs,
schemas, native views, calls, collectors and meter are dropped. A fresh host uses
`PreparedParallelSession::restore()` for already-published rows: it restores no
prepared calls, evaluates no operands and dispatches nothing. Awaiting rows consume
their original modeled incoming values; accepted rows stay accepted. Every
successful barrier hands off exactly once in original member order regardless of
arrival order, then uses the same charged assembly and stacked return code as Seq.
Across the original program there are six preparations and six dispatches. The
captured group is rebuilt once, aliases/frames share it, and the pending group
projects once. Every assembly/return budget cutoff matches uninterrupted execution.

Five tests cover the matrix, all fuel cutoffs, wrong identity/index and duplicate
receipts preserving input ownership and barrier state, fresh authority/domain
refusals before capture work, cold body refusal, second-return capture revocation,
and forged completion/graph/capture/budget data. An unpublished `Prepared` row is
explicitly refused because this saved All represents an already-published batch.
Cancellation and closed-scope polls cover all eight masks, including all results
ready but not handed off: retained results drop, late replies remain rejected,
fuel stays unchanged, and no capture or parent re-entry runs. The test host
explicitly drops the pending return stack; barrier cancellation does not supply
automatic parent or external-operation cancellation.

This is still a fixed typed-IR byte-codec model, not a new library codec or a
production journal. It inherits the 64 KiB ingress bound, two frames and two
three-member graphs; committed-byte correspondence stands in for host authenticity.
The modeled inbox is not a network redelivery guarantee. This earlier fixed-IR
proof did not include source compilation; the bounded extension follows below.
Concurrent races, repeat/multiple captured graphs, durable process-crash settlement
and full extraction remain open.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_parallel_capture_source_export_macos_arm64_20261015.json)
records 2,239 test executions and strict lint success in both the exported
workspace and independent consumer. The unchanged saved-group suite runs 23 tests
in each, including five partial-parallel composition tests. All 248 copied source
files match the repository byte for byte. Partial-parallel capture coverage is
true; complete VM, portable mixed checkpoint, concurrent-race acceptance,
production process-crash and full extraction flags remain false.

### Source-Compiled Captured Recovery

`saved_group::captured::inflight::source` now parses real source and calls
`compile_catalog_prepared_group_source` with the host's explicit type prefix.
The prefix describes a boolean, an atomic native result and two aliases of one
captured group; it contains no runtime values or execution grants. Named native
operation keys let the compiler's owned IR enter the same saved-group adapters
directly, without a second tree conversion or interpreter. Syntax, catalog and
type metadata are dropped before execution; runtime values come from the host.

The source binds a prepared three-member Seq/All, reads operands from the captured
group, projects its reply and returns through two frames. Numeric and text hosts
and captured Seq/All groups cover 220 combinations: 28 sequential frontiers and
192 parallel mask/arrival-order combinations. The original objects are dropped
before bounded committed bytes are decoded by a fresh host. Exact result, fuel,
capture restoration and projection counts match uninterrupted execution at every
assembly/return budget cutoff. Seq reconstructs only previously unpublished
operands and dispatches that suffix once; All restores no calls or dispatches.
One rebuilt captured group is shared by its aliases and both return frames.

Four source tests cover the matrix, every fuel cutoff, fresh authority refusal,
cold return-body refusal and second-return capture revocation. Compiler negatives
cover syntax, unknown operations/locals/members/fields, incorrect operand types,
invalid cold call shape, catalog version mismatch, missing active capability and
missing capability for an unreachable call. These cases stop before runtime work.
Saved state still needs exact host-committed byte correspondence; source compilation
does not make decoded IR, receipts or fuel authoritative.

This is an explicit-prefix source-expression checkpoint model: the completed
captured graph is supplied by the fixture host, not created by a captured helper
earlier in a complete source program. It retains two three-member graphs, two
frames and a 64 KiB ingress ceiling. Helper-origin whole-program recovery is
covered by the bounded extension below; repeat/multiple captured graphs, production persistence/process-crash settlement,
concurrent races and full extraction remain open. No public runtime API, product
wire schema or VM maturity changes; the complete-VM and extraction flags stay false.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_compiled_capture_source_export_macos_arm64_20261015.json)
records 2,247 test executions and strict lint success in both the exported
workspace and independent consumer. Each runs the unchanged 27-test saved-group
suite, including four source-compiled composition tests. All 248 copied source
files match repository bytes. The explicit-prefix source-checkpoint flag is true;
helper-origin whole-program, portable mixed checkpoint, process-crash, complete VM
and full extraction flags remain false.

### Helper-Origin Program Recovery

`saved_group::captured::inflight::whole_program` now starts from an empty runtime
scope and compiles both `main` and a zero-argument group helper through the existing
`compile_catalog_prepared_group_helper_program`. There is no external type prefix,
preseeded result or completed-group shortcut. The helper prepares, dispatches and
accepts all three first-group calls through the original prepared Seq/All collectors.
Charged assembly yields an authenticated group reply; the original stacked walker
returns it into `main`, whose pure bindings create a boolean, an atomic member and
two group aliases. Only then does the second source group prepare its operands.

The bootstrap host authenticates the first group with its original identity and
domain, while subsequent work has a distinct generation. Wrong first-group ownership,
revoked authority and a denied original result domain return the same input without
restoring captures, projecting results or preparing the next group; repairing only
that policy permits explicit retry. The one bootstrap return completes before checkpointing; the
pending group retains two return frames. Seq/All checkpoint writers and restore
paths are shared with the earlier models, not copied into another interpreter.

Numeric/text payloads and all four first/second group-kind pairs cover eight
program variants and 220 frontier/order combinations. Only committed bytes leave
the original execution scope. They retain the compiled pending group, original
return bodies, first-group receipts, derived captures and original remaining fuel.
Source, helper tables, graphs, schemas, requests, collectors and native views are
dropped; restoration neither recompiles nor reruns the first helper. Seq continues
only its unpublished suffix, and All dispatches nothing. The captured group rebuilds
once, with aliases shared across both frames. Every pending-group assembly/return fuel cutoff
matches uninterrupted execution, including exact refusal and callback counts.

Five tests also cover 40 compiler refusals across helper resolution/arguments/types,
unbound locals, invalid or cyclic unused helpers, cold calls, catalog versions and
capabilities; fresh restoration and cold return authority; and forged graph,
capture identity, frame references, frontier and budget bytes. The 64 KiB ingress
bound and exact committed-byte admission remain host-owned, not a new codec or
authenticity certificate. This is one bounded two-group helper-origin program family,
not arbitrary source-program recovery. Multiple captured graphs, additional
control-flow shapes, cancellation/crash settlement and production persistence remain
separate gates. Complete VM/full extraction flags and native embedding maturity
remain unchanged; no runtime API or product wire schema changes.

The [macOS arm64 isolated-source proof](fixtures/leselang_native_helper_capture_source_export_macos_arm64_20261015.json)
records 2,257 test executions and strict lint success in the exported workspace
and independent consumer. Both run the unchanged 32-test saved-group suite,
including five helper-origin composition tests. All 248 copied source files match
repository bytes. The bounded helper-origin checkpoint flag is true, while external
prefix/preseeded-capture flags are false. Arbitrary-program, portable checkpoint,
process-crash, complete VM and full extraction flags remain false.

### Bounded Repeat Program Recovery

`saved_group::captured::inflight::whole_program::repeat` extends the same empty-scope
source model with `repeat(times: 3, body: operation0(...))` and
`seq(head: operation0(...), tail: repeat(times: 2, body: operation1(...)))`.
The existing helper-program compiler expands these into ordinary Sequence IR,
preserving `iteration_1..iteration_3` and `head, tail__iteration_1, tail__iteration_2`
in declared order. The fixture admits four syntactic group sites for two composed
groups, but retains three expanded members per group and all other physical limits.
No runtime loop, scheduler, alternate evaluator or public API is introduced.

Catalog operation identity is deliberately separate from receipt position: repeated
members share the operation schema but retain distinct native identities and result
domains. The test host returns a deterministic per-call result, not an echo of its
operand. Its admission and member projection check the original operation tag, while
queue authority and result validation remain tied to the individual call position.
Source-generated captures and prepared operands cross the same byte checkpoint;
restoration neither recompiles nor replays preparation or the completed helper.

All pairs of Seq/All/repeat/composed-repeat with at least one repeat, across numeric
and text payloads, cover 24 program variants and 332 additional frontier/order
combinations: 140 sequential and 192 parallel. Six tests compare uninterrupted and
restored results, original names/tags, exact fuel and callback counts, including
every assembly/return fuel cutoff. Same-operation wrong-member, foreign-domain and
duplicate receipts preserve the input and queue state; the original valid receipt
can still complete the run. Current/cold/second-return authority is rechecked.
There are 120 compiler refusals for invalid bounds/arguments, expansion width,
unknown cold helpers, malformed calls, versions and grants, plus 16 changed-byte
checkpoint refusals. The latter use host-owned exact committed-byte admission;
they are not proof of arbitrary untrusted-codec validation or receipt authenticity.

The [isolated-source proof](fixtures/leselang_native_repeat_capture_source_export_macos_arm64_20261015.json)
records 2,269 test executions with strict lint checks in the exported workspace and
independent consumer. Each runs the unchanged 38-test saved-group suite, including
these six repeat-composition tests; all 248 copied source files match repository
bytes. This remains a bounded two-group family with one captured graph. Arbitrary
source-program recovery, multiple captured graphs, production persistence and
cancellation/crash settlement remain separate gates. Complete VM/full extraction
flags and native embedding maturity remain unchanged.

### Two-Capture Program Recovery

`saved_group::captured::inflight::whole_program::multiple` starts from empty scope,
compiles `main` and two helpers, and executes two complete source groups before
a third Seq/All suspends. Original authenticated bootstrap returns create two
captured groups with two aliases each, an atomic result and a scalar. The pending
group's operands read both groups; the saved return body projects the second group
and an atomic result from the first.
Compiler limits explicitly reserve ten calls and three groups; native members stay
bounded to three, visible captures to six, and pending return frames to two.

The existing test-host snapshot writer now maps original group pointers into a
bounded table rather than assuming every group reference is zero. Each graph keeps
its original IR, receipt identities and completed-assembly budget. Identical graph
shapes and payloads do not collapse distinct native owners. Only bytes leave the
initial execution scope; both original helpers, schemas, requests, replies and
views are dropped. The shared Seq/All writers, prepared collectors and original
return walker are reused without a second interpreter or public runtime API.

On restoration, each captured graph rebuilds once under its own native owner;
aliases share that graph across both return frames. Every cache hit rechecks the
current authority and result domains. Revoking one owner does not authorize or
revoke the other, and an explicit policy repair reuses the original cached graph.
Denial of either captured domain between return frames still fails, even after the
first frame populated both caches. Completed helper calls and preparation are not
replayed; only unpublished pending Seq requests dispatch, while All dispatches none.

Six tests cover 16 numeric/text program variants and 440 frontier/order combinations
(56 sequential, 384 parallel), including every pending assembly/return fuel cutoff
and exact callback counts. They also cover 56 source/compiler-budget refusals and
16 cross-graph reference, owner, capture-shape and budget changes. Those changes
fail both exact committed-byte admission and the bounded table checks even when
presented as their own committed bytes. This is not arbitrary codec validation or
an authenticity certificate; the host still owns ingress size, payload validation,
native grants and persistence.

The [isolated-source proof](fixtures/leselang_native_multi_capture_source_export_macos_arm64_20261015.json)
records 2,281 test executions and strict lints in the exported workspace and
independent consumer. Each runs the unchanged 44-test saved-group suite; all 248
copied source files match repository bytes. This proves a bounded two-capture,
three-group source family, not arbitrary captured-graph collections, portable
checkpoints, production persistence or cancellation/crash settlement. Native
embedding remains Developing/Draft at completion 10; complete VM and full extraction
remain false. Historical single-capture proof files retain their original claims.

### Restoring Pending Session Ownership

`native::Vm::restore_pending` and `native::LocalVm::restore_pending` install
host-decoded binding return frames into the **original** reply-gated session.
This closes the ownership gap between rebuilding a return stack and managing its
acceptance, cancellation and cleanup. It does not introduce another evaluator,
state machine, scheduler or persisted session format.

The host supplies the original pending identity, borrowed result declaration,
innermost-to-outermost `SavedEffectReturnFrame` list, remaining `Fuel`, immutable
stack limits and a mandatory admission callback. Frame names, counts and the whole
body forest are structurally checked before that callback. The callback borrows
the exact inputs and must validate their committed graph/site/capture correspondence,
current authority, saved budget and nonterminal settlement. Structural success is
not authentication. Zero fuel is retained as zero; admission never grants fuel.

Success starts `Awaiting`. Restoration and idle polling do not prepare or dispatch
effects, restore captures, project results or accept a reply. `try_accept` retains
the original identity/authority/domain gates; rejected replies return intact and
successful acceptance only makes the session `Ready`. A later explicit `poll`
uses the same owned meter, rechecks cold effects and resumes original return sites.
Successor requests still use the original correlation and preparation machinery.

`LocalVm` additionally consumes a unique execution scope. A closed scope refuses
before admission. Admission refusal or escaping unwind closes the scope before
retained identity/frame cleanup; structural refusal also fences unused admission
closure cleanup. Cancel and owner Drop close before pending/accepted state disposal.
An unwind while disposing cancelled captures leaves `Cancelled` sticky rather than
rearming work. Native callbacks and callback-internal/consumed-value destructors
remain trusted Rust code; they cannot be preempted or rolled back, and a second
panic during unwinding may abort the process.

`StackedSessionRestoreError` distinguishes structural refusal, native admission
refusal and closed scope without formatting native payloads or requiring `Debug`,
`Clone`, serialization, `Send` or `'static` on host-owned data. The native facade
re-exports the same error, not a translated error representation.

Eight added regression tests move real original evaluator suspensions through this
entry and cover exact budgets, successor handoff, recoverable receipt rejection,
closed scopes, cold/current authority, cancellation before/after acceptance, owner
Drop, admission/acceptance/reentry unwind and refusal-cleanup unwind. These tests
run unchanged in the exported standalone consumer. The
[isolated-source proof](fixtures/leselang_native_restored_session_source_export_macos_arm64_20261015.json)
records the resulting verification and byte hashes.

This is pending **binding** session restoration, not a final-effect checkpoint
codec, production journal, cancellation/process-crash settlement or exactly-once
certificate. Hosts still cancel external queues/collectors explicitly, reconcile
already-published work and prevent cancelled or completed records being restored.
The metered integration below joins the earlier byte-checkpoint fixtures, but does
not yet combine their collector inventory and meter into one durable owning VM.
Native embedding stays Developing/Draft at completion 10, with complete VM and
full extraction gates false.

### Moving Actual Pending VM State

`native::Vm::try_into_pending_binding(self)` supplies the outbound half of
`restore_pending`: it consumes a waiting binding VM and returns
`PendingStackedBinding` with the original identity, declaration, return stack,
remaining `Fuel` and limits. No native callback, payload clone, allocation,
acceptance, dispatch or budget reconstruction occurs during transfer. There is
no old VM left to accept the same reply. Initial sessions, final-effect requests,
accepted receipts and every terminal state return the original VM unchanged.
The underlying `PendingReply::try_into_parts` similarly refuses terminal handles.

The host can move each `EffectContinuation::into_parts` into a
`SavedEffectReturnFrame`, then call `restore_pending` with mandatory native
admission. Returned fields remain ordinary host-owned data, not an authenticated
image. Hosts must preserve original accounting and policy, validate provenance,
bound encoding/storage and reconcile settlement before reusing saved state.
Dropping transferred owners releases them but does not cancel external work.
`LocalVm` intentionally exposes no transfer escape: its scope remains owned until
closure. Hosts using the unwrapped VM retain responsibility for their own scope.

The two-host native pipeline now moves actual VM suspensions after either or both
effects, then restores and completes with exact original output, requests,
invocations and remaining fuel at every cutoff. It rejects stale predecessor
receipts and rechecks live policy after transfer. Existing restoration/unwind
tests now obtain their frames from real VM polling rather than a lower-level
walker. The [pending-transfer proof](fixtures/leselang_native_pending_transfer_macos_arm64_20261018.json)
records the isolated-consumer checks at native contract `0.163.0`.
Portable codecs, complete durable graph ownership and process-crash settlement remain open;
`complete_vm` and `full_extraction_gate` remain false.

### Final-Request Ownership And Restoration

`native::Vm::try_into_pending_final(self)` now moves a waiting final request into
`PendingStackedFinal`: original identity, borrowed result declaration, remaining
fuel and limits, with no return stack. Binding waiters return unchanged rather
than losing their captures; initial, accepted and terminal states also refuse.
The binding-only transfer API still rejects final requests. Neither transfer
clones native owners, dispatches work, creates receipts or refills accounting.

`native::Vm::restore_pending_final` and `native::LocalVm::restore_pending_final`
adopt this state through the existing `SessionCore` final-reply path. Policy
ceilings are checked before mandatory native admission; the host must prove exact
original request/domain/budget correspondence, current authority and nonterminal
settlement. Zero remaining fuel and zero return-frame allowance are preserved.
Successful restoration starts Awaiting without preparing, dispatching or projecting.
Acceptance retains the original identity/authority/domain checks and only marks
Ready. Explicit poll projects the final reply once with the same meter; it never
invents a return frame or re-evaluates the original call.

Local restoration closes on refusal/unwind before retained identity or unused-hook
cleanup, and callback revocation takes precedence over its ordinary result. Cancel,
owner Drop and acceptance/projection failures retain the existing terminal fences.
Native callback-internal cleanup remains trusted: a reply consumed by a projection
hook can be destroyed before its unwind reaches the framework fence. LocalVm has
no transfer escape, including for final requests.

The [final-request proof](fixtures/leselang_native_final_transfer_macos_arm64_20261018.json)
covers counter and structured-widget hosts, direct final calls and captured-helper
tails, transfer both before and after external dispatch, every original fuel cutoff,
stale/invalid replies, live revocation and cancellation. These are actual native VM
round trips, not synthesized saved stacks. Native contract `0.164.0` remains
Developing/Draft at completion 10. This closes in-memory final-request ownership,
not a persistent codec, committed receipt or crash-atomic external-effect protocol;
complete durable graph recovery and physical extraction remain separate gates.

### Moving Actual Parallel Collector State

`PreparedParallelSession::try_into_saved(self)` consumes an awaiting or
ready-results All collector into original ordered `SavedParallelMember` rows.
Waiting identities/domains and already-accepted replies move through the shared
`ParallelReplyCore`; no native equality, admission, reply projection, cloning,
dispatch or fuel charge occurs. The bounded row vector allocates. Terminal
collectors return unchanged, including after ordered results were handed off.
An inconsistent private slot refuses without discarding converted prefix owners.

`restore` still requires whole-shape/order/identity checks and mandatory native
admission of current authority, original domains and nonterminal settlement.
Transfer is ownership, not permission: saved rows remain mutable host data and
the host must not rearm cancelled, revoked or already-settled work. The enclosing
VM meter, execution scope and published requests remain outside the collector.
Dropping transferred rows releases their owners but does not cancel external work.

The two-host source pipeline now transfers actual All collectors before dispatch,
after each out-of-order reply and after the final receipt, then completes captured
successors and later Seq/All groups on the original VM meter. Every original fuel
cutoff matches uninterrupted execution. The mixed saved-helper proof no longer
copies accepted payloads or fabricates replacement acceptance handles. Numeric
and text byte-checkpoint models likewise encode the original transferred replies,
not newly generated example values; their existing committed-byte correspondence,
stale-reply and settled-result checks remain host-owned.

Move-only ownership tests cover every three-member mask, repeated round trips,
the 64-member ceiling, unsized domains, exact buffer pointers, native admission
refusal/unwind, cancellation, terminal refusal and destructor failures. The
[parallel-transfer proof](fixtures/leselang_native_parallel_transfer_macos_arm64_20261018.json)
records isolated-source validation at native contract `0.165.0`. No persistent
codec, storage transaction or crash-atomic external-effect protocol is added;
complete durable graph recovery and full extraction remain open.

### Moving Actual Sequential Collector State

`PreparedSequenceSession::try_into_saved(self, accepted_prefix)` combines the
host-owned original `SequenceAccepted` prefix with the queue's current pending
identity/domain and unpublished requests. Prefix count, indices and exact original
IR row pointers must match the existing cursor. Refusal returns the queue and
prefix unchanged; native equality, admission, cloning, projection, dispatch and
fuel changes are absent. The bounded saved-row vector allocates. A complete queue
may move its full prefix into all-accepted rows, which restore as Completed and
never publish another request. Cancelled, failed and uncertain owners refuse.

This structural check does not prove the prefix belongs to the same execution:
two executions can share one IR. The required native restore admission still
validates original receipt ownership, current authority, domains, prepared request
inventory, budget and settlement. Saved fields are ordinary host data, not an
authenticated checkpoint. Enclosing VM meter/scope, external work and cancellation
delivery remain host-owned; dropping saved rows does not undo published effects.

The two-host source VM pipeline now transfers Seq and expanded-repeat queues
before publication, after publication, after dispatch and after acceptance,
including the completed frontier. Original accepted buffers and future requests
move unchanged. Exact output, remaining fuel, correlation count and actual calls
match uninterrupted execution at every budget cutoff, including failure paths.
Mixed-helper recovery no longer reconstructs saved Seq rows or acceptance handles;
the existing numeric/text byte-checkpoint writers encode the original transferred
replies and prepared requests instead of regenerating example payloads.

Regressions cover all seven three-member frontiers, maximum-width boundary
frontiers, unsized domains, repeated round trips, wrong-length/reordered/foreign-IR
prefixes, same-IR foreign-execution admission, malformed private cursors, cancellation,
native refusal/unwind and exactly-once owner disposal. The
[sequential-transfer proof](fixtures/leselang_native_sequence_transfer_macos_arm64_20261018.json)
records native contract `0.166.0` and independent consumer validation. This closes
another in-memory ownership gap; production durable codecs, complete graph recovery
and full repository extraction remain open.

### Coupled VM And Collector Transfer

The two-host source pipeline now moves the waiting parent `native::Vm` while its
actual Seq/All collector rows are temporarily saved, then restores both through
the existing admission entries. This is a composed consumer proof, not another
evaluator, public graph snapshot API or storage transaction. Each checkpoint
replaces the parent twice; no old VM remains to accept the pending root receipt.
Seq transfers before publication, after publication/dispatch and after acceptance;
All transfers before dispatch, between dispatches and after reverse-order replies.
Prepared operands, outstanding replies and accepted prefixes remain original
host-owned values, rather than reconstructed samples or newly accepted wrappers.

Twelve host/program combinations cover atomic calls, Seq/All/expanded repeat,
group-result capture, a captured successor and a second group. Original lexical
buffer pointers, shared group-result owners and member reply buffers survive
parent transfer. Exact output, requests, invocations and remaining fuel match the
uninterrupted `LocalVm` path at every fuel cutoff. Stale root receipts are returned
intact without consuming the correct pending slot or charging extra fuel.

Every checkpoint also exercises closed scope, revoked grants, stale root identity,
wrong root declaration and altered capture-site refusal. Injected native admission
unwind is tested before work, mid-graph and at the last boundary. The original
0.166 fixture owned one `ScopedLocalExecution<Option<Vm>>` and explicitly closed
it on native refusal/unwind or terminal completion. Shared-owner restoration
below replaces the manual restoration fencing. This does not add a transfer escape to
`LocalVm`, automatically cancel external work, or certify arbitrary adapter cleanup.
The scope, live policy, request inventory and native storage remain host-owned.

The [coupled-transfer proof](fixtures/leselang_native_coupled_transfer_macos_arm64_20261020.json)
records this unchanged consumer running outside the product workspace. No public
API changed: native contract `0.166.0`, Developing/Draft completion 10, remains
unchanged. Complete durable graph ownership, authentic checkpoint codecs,
crash/settlement reconciliation and full extraction are still separate gates.

### Scoped Collector Restoration

`PreparedSequenceSession::restore_in_scope` and
`PreparedParallelSession::restore_in_scope` borrow an existing
`LocalExecutionScope` for the restoration boundary. They reuse the original
Seq/All reply cores; no second collector or owning scope is created. The existing
`restore` entries keep their unscoped behavior and original validation order.
Closed scopes return `ScopeClosed` before shape, native equality or admission.
Structural or native refusal closes the scope before retained saved rows and
unused admission-hook owners are released. An escaping native equality/admission
unwind also closes before core-owned row cleanup. This includes prepared Seq
requests, accepted replies and pending identities, without new payload trait bounds.

Identity equality and whole-snapshot admission remain native callbacks. If either
revokes the scope, restoration stops before another native check and returns
`ScopeClosed`, even if that callback returned a duplicate identity, success or a
native error. Superseded native errors are disposed only after closure. Ordinary
native errors with an open scope are returned unchanged, after closing the scope.
Callback-internal cleanup remains trusted: the core cannot fence a destructor
executed inside native code before control returns. Cleanup can unwind and a second
panic can abort; this is cooperative lifetime control, not a native-code sandbox.

Success leaves the scope open and retains original accepted/pending/prepared
owners. Restored sessions do not store the scope: use that same scope for later
publication, acceptance and result handoff, and cancel external work explicitly.
No restoration step creates fuel, dispatches work, duplicates replies, authenticates
storage or changes settlement policy. An all-accepted Seq remains Completed;
an all-accepted All remains ReadyResults for its once-only ordered handoff.

The [scoped-restoration proof](fixtures/leselang_native_scoped_restore_macos_arm64_20261020.json)
covers original live snapshots, all seven Seq frontiers and eight All masks on
numeric/text hosts, closed/invalid/duplicate snapshots, native equality and admission
revocation/unwind, original error ownership and cleanup failures. Observer destructors
assert that retained native cleanup never sees an open scope on refused paths.
The same tests run in the isolated source consumer. Native contract `0.167.0`
remains Developing/Draft at completion 10; full durable recovery and extraction
are not promoted by this local lifetime boundary.

### Shared-Owner Graph Restoration

`ScopedLocalExecution::as_parts_mut()` lends the original
`(&LocalExecutionScope, &mut Value)` without extracting the unique owner. A native
coordinator can retain `Option<native::Vm>` in the value and pass that same scope
to raw `Vm::restore_pending_in_scope` and the existing Seq/All `restore_in_scope`
entries. There is no cloned owner, extra liveness cell, state machine, scheduler,
meter or product dependency. Borrows cannot outlive the owning wrapper.

The raw binding-VM entry delegates to the same fenced restoration core already
used by `LocalVm`. Closed scopes skip native admission; structural/native refusal,
revocation and escaping unwind close before retained input cleanup. Success
retains original return frames, captures, identity, declaration, limits and fuel,
but does not retain the borrowed scope. Legacy unscoped restore is unchanged.
That binding-only increment added no transfer escape from `LocalVm` and no
new final-effect restore entry.

The coupled source pipeline now restores parent and collectors under that one
fence. If the parent has already been restored and child admission then refuses,
revokes or unwinds, all leases are closed while the original parent remains owned
in Awaiting state. The coordinator explicitly cancels that retained parent; it
does not mistake revocation for automatic cancellation or successful settlement.
Tests inject each failure at every child restore boundary across twelve
numeric/text host and Seq/All/repeat combinations. Requests, invocations and fuel
do not advance on refused restoration, and sibling owners remain open. Parent
structural/native refusal and unwind no longer rely on manual scope closure.
Every fuel cutoff still matches the uninterrupted `LocalVm` path.

The [shared-owner proof](fixtures/leselang_native_shared_owner_macos_arm64_20261020.json)
records the same coupled tests in a product-free source consumer. Native contract
`0.168.0` remains Developing/Draft at completion 10. Arbitrary host mutation through
the borrowed value is not fenced; callback-internal cleanup remains trusted.
Later dispatch must still check live leases and native authority. Authentic
durable codecs, crash reconciliation, external cancellation and complete graph
extraction remain separate gates, not properties of this local borrow.

### Shared-Owner Final Group Restoration

`native::Vm::restore_pending_final_in_scope` now gives final requests the same
borrowed-owner restoration boundary as binding requests. It delegates to the
existing final restoration core used by `LocalVm`; it does not synthesize a
return frame, replace the original meter or introduce another evaluator.
The coordinator lends its original scope to both the pending parent and prepared
Seq/All collectors. Success starts Awaiting and does not retain the borrowed
scope. Zero-fuel admission remains valid; only explicit acceptance and polling
can project the final result, subject to the original budget and live host policy.

Twelve numeric/text host and source combinations cover final Seq, All and repeat
groups, both directly and after a completed atomic helper. The shared pipeline
moves the actual pending parent and collector states repeatedly at each transfer
boundary. Every fuel cutoff matches uninterrupted execution, including final
group assembly failure. Stale root receipts and reordered group rows retain
retryable ownership without spending fuel. Parent and child restore refusal,
revocation and unwind close the same owner; a retained parent is cancelled
explicitly, and sibling scopes remain open. Pending, collected and accepted
final results can be cancelled without projection or rollback claims.

The [final shared-owner proof](fixtures/leselang_native_final_shared_owner_macos_arm64_20261020.json)
also records structural-limit refusal before native admission, move-only
identity/error cleanup, callback revocation winning over its result, and the
unchanged tests in an isolated source consumer. Native embedding is `0.169.0`,
still Developing/Draft at completion 10. The raw session does not own subsequent
scope closure or external cancellation; callback-internal cleanup remains trusted.
This is in-memory final-group composition, not a durable codec, process-crash
settlement, complete independent VM or repository extraction.

### Borrowed-Scope VM Lifecycle

`native::Vm::try_accept_in_scope`, `poll_in_scope` and `cancel_in_scope`
extend the shared-owner restoration path through the rest of the original
session lifecycle. `LocalVm` delegates to these same implementations; there is
no second state machine, evaluator, meter or receipt conversion.

Keep the original coordinator scope around the raw VM and collectors, lend it
to every scoped operation, and still check live authority and leases at dispatch:

```rust,ignore
let mut owner = scope.bind(vm);
let (scope, vm) = owner.as_parts_mut();
vm.try_accept_in_scope(&identity, reply, &authority, scope)?;
let next = vm.poll_in_scope(&environment, scope)?;
// Explicit cancellation uses vm.cancel_in_scope(scope).
```

A closed scope cancels before validation or evaluation callbacks. Ordinary
reply rejection returns the original input, leaves Awaiting open and spends no
fuel. Completion or terminal refusal closes the owner; escaping callback unwind
preserves HostUncertain. Cancellation revokes before retained cleanup and does
not overwrite Completed, Failed or HostUncertain, even if cleanup itself unwinds.
Core-owned pending identities, frames and receipts use the existing inner fences.
Values already consumed inside a native callback remain native-owned.

These are explicit borrowed operations:
the raw VM does not remember or authenticate its owner.
Passing another scope is an adapter contract violation;
ordinary `poll`, `try_accept` and `cancel` remain unscoped. Dropping a raw VM alone
does not close a borrowed scope. Use `scope.bind(vm)` (or the coordinator's existing
scope-bound state) for owner-first Drop ordering. `LocalVm` still has no ownership
transfer escape. Arbitrary host work, dispatch, external cancellation and durable
settlement remain host duties, not effects of these methods.

Six move-only lifecycle tests cover binding and final restoration, retryable
receipts, closed-scope admission, validation/re-entry unwind, terminal cleanup,
sibling isolation and raw-versus-bound Drop. The existing two-host coupled
Seq/All/repeat pipelines now use scoped operations throughout, without manual
completion closure. Every fuel cutoff and parent/child restore fault remains
covered. The [borrowed-lifecycle proof](fixtures/leselang_native_scoped_lifecycle_macos_arm64_20261020.json)
records their isolated source-consumer verification. Native embedding `0.170.0`
remains Developing/Draft at completion 10; complete VM, durable recovery and
repository extraction are not promoted.

### Borrowed-Scope Host Work And Metering

Restored raw VMs can now fence host-owned work without transferring the
coordinator's scope into `LocalVm`:

- `with_host_work_in_scope(input, work, scope)` returns a normal output unchanged
  and pending; a normal native Err is not an automatic terminal failure.
- `with_host_outcome_in_scope(input, work, scope)` accepts explicit
  Pending/Cancelled/Failed/Uncertain classification from the adapter.
- `with_metered_host_outcome_in_scope(input, work, scope)` additionally lends the
  original session-owned Fuel, shared with assembly and later re-entry.

`LocalVm` delegates to these exact implementations. Both paths use the original
session core's Awaiting transition and the same inner cleanup fence; there is no
second interpreter, replacement meter, input/reply conversion or automatic charge.
Non-awaiting and closed-scope calls return the original input without entering
work. Debits survive Pending, terminal classification and unwind. Exhaustion
policy remains explicit, and native callbacks must never refill or replace Fuel.
A compile-fail check prevents returning the borrowed meter from a callback.

Cancelled/Failed/Uncertain close the scope before core-retained pending cleanup.
Escaping native unwind preserves HostUncertain; cleanup unwind does not overwrite
an already selected terminal reason. A terminal outcome may return an original
collector to the coordinator after revocation for explicit cancellation. Ok(output)
only means work was entered, not that it succeeded or accepted a reply.
If a callback closes the scope but returns Pending, its classification is not
rewritten: the scope remains closed and the next scoped operation cancels before
entering native work. Hosts must check the lease before dispatching returned work.
Callback-local cleanup, live authority, output handling and external cancellation
remain host duties. The methods neither authenticate an input nor prevent repeat
calls, roll back effects or provide a native-code sandbox.

The existing two-capture source checkpoint fixture now routes identical saved
graphs and prepared collectors through both owning LocalVm and a scope-bound raw
VM. Each ownership mode covers 440 numeric/text frontier/order combinations,
every assembly/return fuel cutoff, 1,760 cancellation/Drop cases and 660 live
collector/capture/cold-return refusals against the original manual re-entry.
Four additional move-only tests cover binding/final host work, input identity,
state refusal, exact meter continuity, outcome/cleanup unwind and callback
revocation. The [scoped host-work proof](fixtures/leselang_native_scoped_host_work_macos_arm64_20261020.json)
records independent source and external-consumer verification. Native embedding
`0.171.0` remains Developing/Draft at completion 10. This strengthens local
composition, not durable settlement, complete language parity or repository
extraction.

### Guarded Restored Source Pipelines

The continuous native source fixture now composes the borrowed-scope APIs in one
execution path rather than testing restoration and host work only in isolation.
Two unrelated numeric/text hosts run 24 host/program combinations: mixed
atomic/Seq/All/repeat helper captures with later effects, plus direct and
atomic-prefix final groups.

Each parent moves through the existing pending binding/final handoff while
quiescent. The restored VM then lends its original meter and scope to native
work; prepared Seq/All collectors move through their own restoration checkpoints
inside that callback. The parent cannot be transferred while mutably borrowed
by active host work. Replies still pass original identity/domain checks and
explicit VM polling before capture return or final projection.

Without extra native charging, every fuel cutoff matches the original source
pipeline's values, invocations, request counts and remaining budget. With an
explicit adapter-owned charge at every collection checkpoint, restored and
untransferred parents match at every cutoff. Successful extra debits account
exactly for the difference from the unmetered path; failed charges do not refill
the budget or permit later dispatch.

At every native collection checkpoint, refusal, scope revocation, explicit
cancellation and escaping unwind are injected in both charged and uncharged
modes. The retained parent seals Failed, Cancelled or HostUncertain through the
existing host-work entry, without a coordinator close/cancel repair. Requests
and invocations stop at the injected frontier; later host work and polling cannot
replay it, while a sibling scope stays open. Stale receipt rejection preserves
the original input and meter on successful paths.

The [guarded pipeline proof](fixtures/leselang_native_guarded_pipeline_macos_arm64_20261020.json)
records the same three integration tests in a product-free source export and its
external consumer. No public API or native contract version changes are needed:
native embedding stays `0.171.0`, Developing/Draft at completion 10. This extends
continuous in-memory composition, not arbitrary complete-language parity,
durable codecs, process-crash settlement, external cancellation delivery or
physical extraction. Work already performed is not rolled back, and cleanup
inside native callbacks remains a host responsibility.

### Decoded Checkpoint Successor Effects

The bounded source-checkpoint fixture now continues beyond pure return bodies.
The ordinary source compiler produces a four-group program. Two completed groups
are captured, the third Seq/All group is saved at a collection frontier, and its
decoded return frames select a fourth group on a distinct host generation.
Operands for that new group come from a restored atom, the second captured group,
and the computed result of the third group.

Only bounded host-owned bytes leave the initial execution; its tree, adapters,
collectors and domains are dropped before decoding. Recovery rebuilds the two
captured graphs and the pending collector, then uses the existing session,
host-work, correlation, reply acceptance and final-projection paths. The adapter
correlates the exact source-selected successor node and current result domain;
decoding a handle does not itself grant dispatch authority.

Numeric and text result models cover 880 source/frontier/order combinations per
ownership mode. The owning session and scope-bound raw session produce identical
values, remaining fuel, successor counts and terminal states. Settled captured
groups are never prepared or dispatched again; the pending group dispatches only
its saved unpublished operands. The new group is correlated once and dispatches
three operations. Old replies are rejected both with their old identity and with
the new identity but wrong result domain, retaining their original allocation
and the VM's fuel.

Eight representative host/group combinations exercise every fuel cutoff from
restored collection through successor preparation, assembly and final projection.
Both owners use one saved meter. Live denial before re-entry, revocation after
correlation, and cancellation of the new request close the parent before any new
dispatch; subsequent poll/work cannot reopen it.

The [decoded successor proof](fixtures/leselang_native_checkpoint_successor_macos_arm64_20261020.json)
records independent source and external-consumer verification. No public API or
contract version changes: native embedding remains `0.171.0`, Developing/Draft at
completion 10. This closes a composition gap in a bounded fixture, not arbitrary
source checkpoint compatibility, a production codec/journal, process-crash
settlement or complete native VM extraction. Committed-byte provenance is supplied
by the test host; the fixture does not establish cross-process exactly-once effects.

### Metered Owned Checkpoint Recovery

`native::LocalVm::with_metered_host_outcome(input, work)` lends the original
session-owned `Fuel` to a synchronous native callback while the session is
Awaiting. It uses the same `SessionCore` transition as unmetered host work, with
the same Pending/Cancelled/Failed/Uncertain classification and unwind fencing.
Other states return the original input without entering the callback or charging.
The callback cannot retain the borrowed meter in its output.

This makes group assembly use the same budget as saved-session restoration and
later capture returns, without a second root meter or ad hoc debit reconciliation.
Charges survive normal outputs, cancellation, failure and unwind. There is no
automatic charge or refund. Fuel exhaustion does not infer a terminal state from
an arbitrary native output; the adapter explicitly chooses Failed when required.
Like existing evaluator callbacks, native code is trusted: mutable meter access
must not be used to replace/refill Fuel, and required charging cannot be enforced
against an adapter that ignores the contract. This is not a native-code sandbox.

The existing two-capture source checkpoint fixture now decodes its host-owned bytes,
installs original saved frames through `restore_pending`, restores the original
prepared Seq/All collectors and assembles their result inside metered host work.
Wrong root replies are returned intact. Correct acceptance only marks Ready;
explicit polling then restores both captured graphs and the original return bodies.
No completed helper is rerun, no preparation is repeated, Seq only dispatches its
unpublished suffix, and All publishes no replacement requests.

Four added integration tests cover 440 numeric/text frontier/order combinations,
every assembly/return fuel cutoff against the original manual path, 1,760
cancellation/owner-Drop cases, and 660 live collector/capture/cold-return refusal
cases. Reduced-budget tests explicitly lower their test-owned saved grant; they
do not certify arbitrary modified checkpoints. Three session regressions cover
pending debit continuity, state refusal and terminal/cleanup unwind, plus a
compile-fail check prevents escaping the meter borrow.

Cancellation remains explicit host orchestration: return the collector through
Cancelled, wait for local revocation, then cancel and dispose it. Native dispatch
checks the execution lease immediately before work. Owner Drop revokes but does
not itself cancel an external collector. Already-applied effects are not undone;
callback-internal cleanup remains native responsibility. The bounded fixture's
post-checkpoint bodies are pure; unexpected successor effects refuse correlation
instead of fabricating a declaration. This is not arbitrary source checkpointing,
a production durable owner, a journal, replay prevention or crash settlement.

The [isolated-source proof](fixtures/leselang_native_owned_recovery_source_export_macos_arm64_20261015.json)
records the copied original tests and source hashes. The native embedding cell
remains Developing/Draft at completion 10; full VM and extraction flags stay false.

### Shared Dispatch Lease Fencing

`validate_dispatch_lease(DispatchLeaseStamp, DispatchLeaseObservation, now_ms)`
extracts the reference journal's numeric current-lease check into runtime-core.
It compares exact request correspondence, active state, attempt, semantic retry
count, readiness and expiry before checking elapsed time. A mismatch is
`Superseded`; an unchanged but expired lease is `Expired`. No allocation, callback,
serialization, arithmetic repair, fuel charge or storage mutation occurs.

The expiry instant is deliberately inclusive for completion. The scheduler can
re-lease at that same instant, but only the current attempt can settle: the first
committing transaction wins. This preserves the released reference contract,
including completion at the maximum supported clock. Scheduling and completion
therefore have different boundary predicates by design, not an off-by-one fix.

Both reference journals use this check for acknowledgement and retry; successor
settlement delegates too. The SQLite adapter keeps it within the existing immediate
transaction after exact request/image admission and authoritative terminal replay.
The in-memory acknowledgement path now also rejects a timestamp before a semantic
retry's `ready_at_ms`, matching SQLite. The regression first fails against the old
in-memory predicate, then passes with the shared check. Cancellation stays terminal:
late acknowledgements and error reports return the committed cancellation rather
than scheduling or executing another attempt, including after SQLite reopen.

The five unchanged exported tests include a 1,296-case field matrix, full-width
numeric comparisons and opaque GUI-local request identity. Native request types
need no Eq/Clone/serde trait here: the host supplies `same_request` only after its
own exact provenance checks. Hosts separately validate clock/counter limits and
current authority. An observation is not authenticated, not a reservation and not
safe to retain across yields or storage changes. Terminal receipt replay, database
atomicity, durable owning sessions and external-effect crash settlement remain
host responsibilities; this does not turn `LocalVm` restoration into a journal.

The [isolated-source proof](fixtures/leselang_native_dispatch_lease_source_export_macos_arm64_20261015.json)
records byte-identical source and consumer tests. Full VM/extraction flags remain
false, and native embedding stays Developing/Draft at completion 10.

### Shared Semantic Retry Planning

`plan_retry_schedule(current_retry_count, now_ms, RetryTiming)` returns either
`Some(RetrySchedulePoint)` or `None` for exhaustion. It checks the portable clock,
positive/ordered delays and then the granted retry count. The first retry uses
the base delay; later attempts reuse `capped_exponential_delay`. Time addition
reuses `checked_clock_add`, with no wrapping, implicit refill or deadline clamp.
Full-width exhausted counters return no proposal rather than overflowing.

`validate_retry_schedule` separately checks exact next count and an inclusive
host retry ceiling before requiring a strictly future portable ready time. The
reference in-memory and SQLite journals use this before changing their state.
The in-memory path now rejects out-of-range ready times and over-limit retry
counts just as SQLite does; checked count progression also removes an internal
unchecked-addition edge. Public callers still pass the original reference policy
and error checks, so this is journal-boundary hardening, not a claim that ordinary
public calls previously bypassed those checks.

The reference VM now routes product classification and transient planning through
the command adapter while retaining the original fault codes and terminal replay.
Permanent errors do not evaluate retry policy;
transient errors validate policy even when already exhausted. Committed terminal
results, including cancellation, replay before late malformed error/policy data
is inspected. Lease redelivery and admission attempts remain distinct from
semantic retry counts. Execution deadlines still win independently of a future
retry time, and the original saved fuel is never replenished.

Eight unchanged exported tests cover 20,720 planning cases against wide-integer
arithmetic, 1,512 proposed-schedule checks, full-width clocks/counters and explicit
observation limits. Reference tests additionally check refusal without lease
mutation, SQLite reopen, and terminal replay with invalid late metadata. Numeric
proposals carry no native error, identity, resource handle, timer or storage owner.
Hosts must validate exact policy provenance, live authority and current leases,
then check/commit atomically. Reusing a stale count can still make the same pure
proposal: this is not a retry registry, durable owner or crash settlement.

The [isolated-source proof](fixtures/leselang_native_retry_plan_source_export_macos_arm64_20261015.json)
records original source and consumer test hashes. Native embedding remains
Developing/Draft at completion 10, with complete VM/extraction flags false.

### Shared Dispatch Capacity

`check_dispatch_admission(pending, additional, limit)` separates a permanently
oversized batch (`BatchTooLarge`) from transient occupied capacity (`Backpressured`).
The batch check comes first even when the queue is already full. Exact-limit sums
are admitted; arithmetic overflow refuses, including at `usize::MAX`. Zero limits
and empty additions have explicit semantics, without importing journal-specific
ceilings. The reference adapter still validates nonzero limits against its original
record ceiling and preserves `LSV2500` through `LSV2503` and wire DTOs unchanged.

`dispatch_lease_capacity_exhausted(active, limit)` separately handles execution
slots, including occupancy above a newly lowered limit. Both reference journals
delegate to it. SQLite still counts at most the configured limit before deciding;
it does not scan or decode the whole pending queue on a busy claim. Eligibility
remains host-owned: delayed retries and blocked sequential successors are idle
until runnable, even with all slots occupied. A refused claim spends no attempt.

Pending capacity includes leased, delayed and dependency-blocked dispatches.
Lowering a policy blocks new work but does not revoke existing leases or re-admit
already accepted groups/tails. Six unchanged exported tests cover 729 admission
combinations, 49 execution-slot combinations, overflow/zero bounds and composition
with the original synchronous `Admission` lifecycle. The host maps typed pressure
to its own diagnostics; rejected or cancelled input never publishes work. Reference
tests additionally exercise lower limits and delayed retries across SQLite reopen.

These functions neither read a clock nor allocate identities, retain payloads,
change counters or grant execution. Their inputs are observations, not authority.
Hosts must read, check and publish under the same owner or transaction, and cannot
reuse a successful observation after yielding. This is not a reservation, queue or
atomic admission owner. No new scheduler, storage layer or global lock is added.

The [isolated-source proof](fixtures/leselang_native_dispatch_capacity_source_export_macos_arm64_20261015.json)
records byte-identical source and consumer tests. Native embedding remains
Developing/Draft at completion 10, with complete VM/extraction flags false.

### Shared Dispatch Issuance Planning

`plan_dispatch_lease(now_ms, lease_ms, max_lease_ms)` checks the portable clock,
positive host-bounded duration and future expiry, in that order. It reuses the
shared checked clock addition without wrapping or clamping. The host supplies
its policy ceiling; zero grants no duration. `next_dispatch_attempt(current,
max_attempts)` computes exactly one delivery attempt under an inclusive ceiling,
rejecting numeric exhaustion before the limit check. No counter reset occurs.
Delivery attempts remain distinct from semantic retries and admission polls.

Both reference journals share these primitives, preserving their original
`LSV4015`/`LSV4017` diagnostics. Issuance preflight still happens before deadline
cleanup or SQLite locking. The journal passes the checked expiration into its
backend; eligible selection and capacity checks precede attempt progression,
which commits with the lease under the original owner/transaction. This avoids a
fallible expiration calculation after mutating the in-memory attempt counter.

Issuance is not settlement. A current receipt is valid at the last portable clock
even when no future lease can be represented. The successor path previously
validated a fictitious 1ms new lease and rejected such a completion. A red/green
regression now covers direct and conditional successors in both journals,
including out-of-range rejection, SQLite reopen, exact successor replay and
cancellation without refilling saved fuel. Enqueuing a successor does not grant
it a lease: a claim at the maximum clock still refuses. A separate regression
proves that exhausting redelivery attempts leaves the last current lease valid
for completion at its inclusive expiry, including across reopen.

Six unchanged exported tests cover 294 timing combinations and 17,161 counter
combinations against wide-integer oracles, zero/full-width limits, and composition
with original lease fencing. Native clock-domain identity, monotonicity, work
eligibility, capacity, deadlines, request authority and atomic commit remain host
responsibilities. Repeated numeric input can produce the same plan; it must not
be reused across a yield. This is not a lease grant, durable owner or crash settlement.

The [isolated-source proof](fixtures/leselang_native_dispatch_plan_source_export_macos_arm64_20261015.json)
records original source and consumer hashes. Native embedding remains
Developing/Draft at completion 10, with complete VM/extraction flags false.

### Host-Owned Dispatch Data

The reference VM now specializes runtime-core's `DispatchLease<Request>`,
`RetrySchedule<Error>` and `FailedEffect<Error>` directly. These replace three
product-VM-owned definitions rather than introducing a mirror protocol. Both
journals and existing consumers keep the released names and use the same types;
native hosts need no product request, continuation, error classification or Step.

Opaque move-only requests/errors, borrowed views and GUI-local resources can be
constructed and moved without Clone/Debug/serde/Send/Sync/Default/'static bounds.
Derived traits remain conditional. The original lease retry-count zero default,
field order and outer unknown-field policy are preserved, while each nested host
codec keeps its own validation. Decode does not enforce counter/time policy or
authenticate saved data. Adapters still check identity, grants, deadlines and
current storage in the same transaction/owner as settlement.

The product `RetryDisposition` enum remains product-owned, including supported
variant imports, fault classification and terminal Step semantics. This is data
ownership extraction, not a generic scheduler, queue, durable owner or full VM
extraction. Numeric and GUI-local host tests compose the real lease/retry planners
without cloning payloads; reference tests pin pre-refactor wire bytes and exact
type identity. The same native tests run unchanged in the exported consumer.

The [isolated-source proof](fixtures/leselang_native_dispatch_data_source_export_macos_arm64_20261016.json)
tracks this contract at `0.162.0`. Embedding remains Developing/Draft at completion
10; full-language coverage, durable ownership and reference migration remain
separate gates rather than being implied by shared dispatch data.

### Shared Flat Group Graph Inspection

`validate_group_graph()` in runtime-core inspects complete original rows against
an ordered declaration. Its explicit inclusive member ceiling is 64; zero only
admits empty input. Count checks precede allocation and every native hook, while
language arity and declaration-name admission stay separate. Each original row
is observed once before checking position, name, owner exclusion and duplicate
identity. Invalid positions are rejected, not renumbered. No sort, truncation,
payload conversion or retry changes the original order or failure precedence.

Keys require only Eq/Hash; rows/errors require no Clone/Debug/serde/Default/Send/
Sync bounds. Native errors move unchanged behind payload-free diagnostics, and
hook owners clean up before result/error handoff. Native observation/comparison/
hash/cleanup unwind stops the pass without publishing a partial observation;
native work cannot be rolled back, and a second cleanup panic may abort.

The actual reference VM uses this same kernel for group-request admission and
SQLite group recovery. Frozen old request checks prove exact fault precedence.
A late malformed group blocks checkpoint handoff and watermark repair within
the original immediate transaction. Independent tests use unrelated numeric
counter and opaque GUI-local identities, and gate a complete checkpoint before
commit. The separate exported consumer runs those exact production-code tests.

This returns a transient structural observation, not a saved-graph certificate
or execution grant. Native schema/authority, exact owner metadata, values,
cross-group ownership, sequence reservations, dispatch/terminal state, authentic
receipts and the same storage snapshot/transaction remain adapter-owned. No
body execution, meter reconstruction, effect dispatch or replay is added.
Complete independent durable graph recovery remains open; reference migration
is also a separate gate. This is not the complete independent VM or repository extraction gate.

### Shared Sequence Watermark Planning

`SequenceWatermarkRequirement::after_observation()` computes the required numeric
successor under an explicit inclusive watermark ceiling. Zero represents an empty
highest-sequence observation and requires one; it is not a valid identity or an
allocation permission. Overflow/ceiling refusal precedes native metadata reads.
The watermark ceiling is distinct from the native effect-sequence ceiling.

`reconcile()` rejects zero/out-of-ceiling stored watermarks, preserves existing
gaps and reports exact `next_sequence`/`write_required` observations. Equal/higher
stored values require no write. No shrinking, gap filling, clamping, wrapping,
identity parsing, normalization or implicit defaults are added. Requirements keep
private ceiling fields, but remain copyable stateless numeric observations, not
one-shot storage handles. Public updates are mutable data; construction/inspection
cannot authenticate them. Everything is allocation-free arithmetic without native
hooks, codecs, reads/writes, meter construction or evaluation.

Real SQLite allocation and `advance_sequence_watermark()` use the shared
requirement; reconciliation stays after the original signed-metadata read and
conversion. Native error codes, immediate transactions and commit order remain
unchanged. Live allocation and restore use the same checked successor requirement,
with native canonical-token/sequence checks and durable insertion before local
watermark publication. Frozen SQLite tests cover 72 combinations of empty/large/
exhausted observations, invalid/missing metadata and rejected writes, preserving
exact native fault order and resulting watermarks. Real import/recovery/exhaustion
refusals leave all five journal tables and original pending frames/budgets intact.
A write-rejecting trigger cannot break recovery at an already-correct watermark;
repair/retry preserves cold tails and stale-worker allocation without ID reuse.
Numeric and opaque GUI-local checkpoint models run unchanged in the independent
consumer; these models are not a storage implementation.

Success is prospective numeric planning, not a durable allocator, reservation,
receipt authenticity, execution grant or commit certificate. Original allocation
watermark proof still precedes imported-ID repair. Native identities, reservations,
authority/receipt/complete graph admission and same-snapshot commit-before-publish
remain host-owned. Planning never makes a failed write/commit safe or grants replay.
Complete independent durable graph recovery remains open; this is not the complete
VM or extraction gate.

### Shared Inherited Continuation Context

`validate_inherited_context()` borrows original and saved accounting observations.
Native authority agreement precedes native revision agreement, each hook at most
once, followed by relative deadline, exact optional absolute deadline and output
limit equality. No allocation, payload copying, identity parsing, normalization,
default deadline or automatic limit narrowing is added. Native opaque identities,
revisions and errors need no Clone/Debug/Eq/serde/Default/Send/Sync bounds. Both hook
owners release before result/error handoff; native errors move unchanged with
redacted diagnostics and no error source. Refusal or unwind stops later hooks
without retry. Native side effects/cleanup cannot be rolled back or sandboxed;
a second cleanup panic may abort.

Fuel is deliberately ignored here: native group allocation/decreasing-fuel checks
precede context, whereas atomic successor consumption and token order follow it.
Matching zero/maximum metadata is not valid-budget admission. Ingress validation,
original/current authority, revision semantics, schema/body/result admission,
absolute time checks, fuel progression and sequence reservations remain host-owned.

Real `successor::same_inherited_context()` delegates this exact phase for atomic
successors, group prefixes and group tails. It uses native principal/capability
and optional-revision comparisons without changing original fault codes or guard
order. Frozen query/presentation-domain tests preserve the old predicate, including
ignored fuel/schema fields. Twenty-four real SQLite cases mutate principal,
capabilities, revision, relative/absolute deadlines or output limits across atomic,
sequential, parallel and tail succession. Each forged request passes standalone
native admission, but recovery refuses with all five journal tables unchanged,
before imported watermark repair. Restoring original frame/request/deadline bytes
recovers original pending frames/budgets and only then repairs the imported identity
watermark. Opaque GUI-local proofs run unchanged in the independent consumer.

Success is transient context agreement, not receipt authenticity, current
authorization or a budget/execution grant. Original identities/bodies, complete
graph recovery and same-snapshot commit-before-publish remain host-owned. No meter
construction, refill, evaluation, dispatch, replay or storage writes occur here.
Complete independent durable graph recovery remains open; this is not the complete
VM or extraction gate.

### Shared Capture Extension

`validate_capture_extension()` borrows original and successor groups/results.
An explicit inclusive aggregate binding limit (ceiling 64) applies independently
to both frames before hooks, without allocation or payload copies. Native names,
duplicates, schemas and arity remain host policy. Group data membership precedes
exact named-group retention, then one native `begin_results()` call. Each saved
result is projected before original-data membership; membership comparisons stop
on the first match. Aliases of original or current data are allowed, but cannot
replace original named bindings or the pending current binding. A current match
never skips later native errors. Missing current binding precedes named-result
retention. Nothing is normalized, reordered, copied or retried.

Opaque data/errors need no Clone/Debug/Eq/serde/Default/Send/Sync bounds. The domain
owner releases before result/error handoff. Native errors move unchanged with
redacted diagnostics and no error source; refusal or unwind stops later hooks.
Native side effects/cleanup cannot be rolled back or sandboxed; a second cleanup
panic may abort. Borrowed originals remain untouched.

Real `successor::validate_pair()` delegates only this capture extension phase
through the command adapter's `validate_result_binding_extension()` in live
successor admission and SQLite recovery. The adapter owns product group/result
equality, original name retention and version-closed current-result projection;
the VM supplies its original binding ceiling. Native request/result admission,
schema succession, original authority/revision/deadlines/output limits, decreasing
fuel and token order, and saved-body typing still precede the shared kernel.
Frozen native alias/retention tests preserve exact faults, including late invalid
projections. Structurally valid alias renaming, deletion, current-name replacement
and rebinding refuse recovery with all five journal tables unchanged; repairing
only the stored frame/request restores original pending frames and budgets.
Opaque GUI-local proofs run unchanged in the independent consumer.

The adapter's payload-free `CaptureReplayError` keeps mismatch distinct from
invalid projection, so the VM preserves `LSV1407` versus `LSV1405` without leaking
capture data. Product field matching retains selected-scalar copies, not whole
frame copies. The 70-operation/five-version comparison matrix and direct alias,
malformed-capture and bound-order tests cover this boundary. Complete frame/raw
admission remains required before these comparisons; no second evaluator is added.

Success is transient retention, not provenance, receipt authenticity or an
execution grant. Original bodies/current authority, complete graph recovery and
same-snapshot commit-before-publish remain host-owned. No body execution, fuel
construction, dispatch, replay or storage writes occur here. Complete independent
durable graph recovery remains open; this is not the complete VM or extraction gate.

### Product Result View Lifecycle

`leselang_command::ResultView` is the borrowed adapter view used by the actual
reference evaluator for raw replies, saved scalar projections and raw/saved
groups. `project_scalar` copies one available scalar; `select_member` borrows
the original result after exact name/operation lookup. Raw signature lookup
precedes raw row lookup. It does not check declared result types, original
arguments, duplicate names or raw reply identity: those require complete admission
before evaluation. Views themselves are not serialized or authenticated handles.

`capture_result` projects raw data or copies an already admitted saved projection.
`capture_group` materializes a raw group with explicit `GroupCaptureLimits` and a
trusted name predicate, but copies saved groups unchanged without revalidation or
invoking raw policy. Raw construction preserves projection-before-name-check order;
saved versions and field sets are never upgraded. Hosts bound ingress, aggregate
copy sizes and callback work before these data operations. Native callback unwind
propagates, prior side effects are not rolled back and partial captures are dropped.

`ResultViewError` distinguishes invalid access from invalid projection without
carrying payloads. The VM maps them to its unchanged `LSV1402` and `LSV1405`,
supplies existing group policy, and keeps the original snapshot/re-entry meter,
canonical admission and journal gates. Payload-free view diagnostics do not print
native reply data. Member lookup borrows instead of copying, while explicit
capture construction and selected-scalar reads retain their existing copies.

The adapter-only lifecycle matrix covers all 70 operations and five saved field
vocabularies: raw selection/capture, saved wire round-trip, restored reads/copies,
complete binding validation and successor/group correspondence. Frozen reference
tests compare old and new faults, copy bytes and original member addresses,
including malformed schemas and duplicate-name behavior below the admission
boundary. Existing snapshot/re-entry tests retain every original fuel boundary.
This is a complete product result-view data path, not a second evaluator,
authenticated restore API or completion of host-neutral durable graph recovery.

### Adapter-Only Captured Execution Proof

`crates/leselang-command/tests/captured_execution.rs` takes parsed reference
programs through the existing `evaluate_effects_in_scope`,
`snapshot_captured_scope_state` and `resume_captured_effects_in_scope` entries.
`ResultView` handles raw/saved projections; the normal command-adapter validators
check complete bindings and successor/group correspondence. There is no fixture
expression interpreter, new complete runtime entry or VM/storage dependency,
including dev-dependencies. The language CI job runs this proof explicitly.

Two flat first-group spellings (`seq` and `all`) join a scalar prefix, group and
member aliases, two computed atomic successors and a final scalar. Direct and
wire-restored runs produce the same effects, capture bytes, result and total fuel.
One original meter spans every re-entry; all lower grants refuse without refill,
and the exact completion grant succeeds. Saved captures for versions 1-5 preserve
their old field sets through successor construction. Validation rejects a read
of a missing old field even in a cold branch before restoration begins.

Malformed versions/names/locals, mismatched result types or payloads, missing or
reordered group members and revoked fixture policy leave fuel, preparation count
and original inputs unchanged. Corrected input can resume the original data.
Parsed scalar recovery and a conditional successor execute in the shared walker;
native projection refusal and fuel exhaustion never enter the scalar fallback.
Normal completion also proves that the restored temporary scope is cleared.

The fixture supplies bounded headless presentation replies, explicit admission
and native copy accounting. It does not implement authenticated identities,
dispatch deduplication, a parallel scheduler, arbitrary nested groups,
crash-atomic storage or a production embedding API. Wire restoration here is an
in-memory codec round-trip, not process restart or durable authority. This closes
a reference-adapter execution proof after the data lifecycle, not the two-host
complete-language, durable recovery or repository-extraction acceptance gates.

### Adapter-Owned Request Preparation

`leselang_command::prepare_preflighted_effect_in_scope` now owns the reference
profile's raw/call/group request construction. Both the production VM and the
adapter-only proof use it. A raw `Host` borrows its exact effect; a computed call
uses existing HIR argument evaluation and operation resolution; each computed
group member uses `evaluate_effects_in_scope` with the original scope, environment
and fuel. Members prepare in declaration order, including `all`, and their
resolved operation/result type must match the prepared signature. There is no
second expression interpreter or parallel dispatch in this function.

This is an explicitly **preflighted hook**, not an untrusted-input entry point.
Whole-expression structure, types, cold signatures, opaque payload policy and
authority remain required caller gates. Raw effects are not revalidated; group
counts/names are assumed admitted. Explicit limits bound nested shared execution
and call preparation, not opaque payloads or the enclosing group. The outer
executor pays the root charge. A later failure neither refunds fuel nor rolls
back native hook work, and hooks must not dispatch. Live dispatch policy remains
host-owned; returned request data grants no authority or durable-replay right.

`EffectPreparationError` retains nested typed calculation/native failures, call
failures and operation-resolution diagnostic codes. Its Display/Debug output
contains no payload and requires no native Clone/Debug/serde traits. The VM alone
maps these errors to its existing fault codes without turning native failures
into recoverable scalar errors. Frozen before/after tests compare direct hooks
and full evaluation at 160 fuel grants per case, including malformed HIR, late
member failures and scope cleanup. The adapter-only parsed proof now covers
computed flat groups, capture/re-entry, two successors and every fuel cutoff,
plus zero-work cold refusals and non-rollback late failures. This does not add a
production embedding driver, arbitrary nesting, authentic receipts or storage.

### Adapter-Owned Capture Scope Bridge

The command adapter now owns product row conversion in
`snapshot_preflighted_result_scope` and
`resume_preflighted_result_binding_in_scope`. These are thin specializations of
`snapshot_captured_scope_state` and `resume_captured_effects_in_scope`, not new
scope walkers or body evaluators. The production VM and adapter-only execution
proof both use them. `ResultCaptureLimits` carries explicit snapshot/group bounds;
`ResultCaptureHost` supplies scalar/result/group accounting, projection-error
mapping and the row-level saved-projection restriction. There are no default
costs, VM fault codes, new fuel grants or native Clone/Debug/serde requirements.

Snapshot count/name-byte checks precede native hooks. Scalars charge before
copying; result/group snapshots materialize before their original host charge;
the body copies last. Raw groups alone use the supplied member/name policy;
saved groups clone unchanged without vocabulary widening or implicit admission.
Each category keeps original row order and failures expose no partial snapshot.

Restoration charges and copies scalars first, then admits and charges each saved
result/group before borrowing its exact original projection. The pending view
adds no new charge or copy. The VM's group-result profile can still reject saved
projection rows at their original position, after earlier scalar accounting.
Insertion, duplicate refusal, body evaluation and temporary-suffix cleanup stay
in the shared kernels. Host errors remain native, not scalar-recoverable; earlier
charges and callback work survive later refusal or unwind without refund/retry.
Parent rows and caller-owned wire data remain unchanged.

Focused VM-free tests verify mixed snapshots, exact original result/member
addresses, alternative explicit accounting, zero-work structural refusal,
projection-before-charge failure, profile denial, duplicates, move-only opaque
errors and unwind cleanup. Frozen VM tests retain original scalar/result/group
fuel cutoffs, snapshot bytes and fault precedence. The complete adapter proof
retains five saved vocabularies and computed group/successor re-entry.

These preflighted entries are **not untrusted-frame ingress**. Callers must bound
body/payload/aggregate storage, validate full schemas/types/names, correlate raw
replies, retain the authentic meter and check current authority/journal state
before entry. Callback work remains trusted and cannot be preempted; a second
cleanup panic may abort. No codec, cancellation driver, authenticated receipt,
deduplication, transaction or independent durable-graph recovery is introduced.

### Shared Product Execution Environment

`leselang_command::PreflightedResultEnvironment` composes the preceding view,
preparation and snapshot APIs for both the reference VM and VM-free proof. It
implements the shared HIR pure/effect environment traits, not another evaluator.
Computed groups recurse through the same environment and borrowed host. Explicit
`ResultExecutionLimits` carries effect/capture limits; pass `limits().effects` to
the outer evaluator and retain the original caller-owned fuel throughout.

`ResultExecutionHost` extends `ResultCaptureHost`. Required hooks own cold effect
preflight, selected preparation checks, prepared-data observation/refusal,
selected-field policy, group-name policy and typed preparation/capture failure
mapping. There are no implicit grants or costs. The host can be unsized and its
errors need no Clone/Debug/serde/Send/Sync traits. Environment Debug is payload-free
and runs no hooks. Native errors must remain native, while recoverable scalar
errors retain their typed provenance. A prepared-data hook is not dispatch or a
receipt; it must not invoke effects, refill the meter or retry prior work.

Construction does not perform admission. Full input/HIR/frame/reply validation,
current authority, opaque work bounds, dispatch and journal gates stay with the
host. Trusted hook mutation or unwind does not roll back earlier work. Shared
cleanup restores the parent scope and never refunds fuel. Tests exercise one
dynamic host across computed seq/all members, every fuel cutoff, native payload
identity, cold/late refusal, non-recoverable field policy and unwind cleanup.
Frozen VM comparisons retain original costs, faults, captures and wire bytes.
This is product-adapter reuse, not a generic embedding driver, durable recovery,
real GUI execution or completion of the independent-repository gate.

### Shared Capture Correspondence

`validate_capture_correspondence()` borrows original saved groups/results and
the original successful result prefix. Inclusive aggregate saved/original counts
(each ceiling 64) precede hooks, bounding work to groups + results * originals
without allocation. Native policy still owns language arity and required captures.
Groups admit once in their original order, followed by complete capture-major
original-row scans. A true match does not skip later rows: their native errors
must still be observed. Unmatched captures stop before the next capture; nothing
is sorted, normalized, copied or retried.

Opaque groups/results/rows and errors need no Clone/Debug/Eq/serde/Default/Send/
Sync bounds. Native errors move unchanged with payload-free diagnostics. Both
hook owners release before result/error handoff; refusal or unwind stops later
hooks. Native side effects/cleanup cannot be rolled back or sandboxed; a second
cleanup panic may abort. Original borrowed data is never consumed.

Real `GroupResultBinding::validate_saved_capture()` uses this kernel through the
command adapter's `validate_group_capture_correspondence()` during live capture
advancement and SQLite bound-group recovery. Native cold prefix/operation
shape and missing-binding checks still precede correspondence. Native group member
names, signatures, operation domains, projection versions and raw-result matching
remain unchanged. Frozen native fault-order tests preserve late errors after a
successful earlier match. Real sequential/parallel alias/result mutations refuse
before checkpoint handoff or watermark repair, leaving all stored records intact;
fixing only invalid capture bytes restores original pending frames and budgets.
Numeric and opaque GUI-local proofs run unchanged in the independent consumer.

The standalone product entry repeats row-shape checks before its bounded shared
walk and accepts explicit group limits/name policy. The VM retains its earlier
row-shape-before-missing-binding gate and maps mismatch to `LSV1409`, invalid
projection to `LSV1405`. Callbacks and group scratch work remain native; refusal
and unwind do not roll back callback side effects. Group/member correspondence
still does not certify original branch arguments, declared raw types or receipts.

Success is transient correspondence, not receipt authenticity, a graph certificate
or execution grant. Original identity/captures/current authority, complete graph
validation and same-snapshot commit-before-publish remain adapter-owned. No body
execution, fuel reconstruction, dispatch, replay or storage writes occur here.
Complete independent durable graph recovery remains open; this is not the complete
VM or extraction gate.

### Shared Ordered Outcome Summaries

`OrderedOutcomeSummary` owns bounded observations of already-admitted original
terminal outcomes, not success payloads. Explicit member limits (ceiling 64)
precede observation; output limits remain native policy. Counts saturate rather
than wrap. Sequential output overflow becomes a stop only if no earlier stop
exists; parallel overflow is deferred until `finish()` and overrides earlier
stops. Otherwise the first stop in declared observation order wins. Pending rows
are not observations, and an observation limit is not proof of group completeness.

Stops move without Clone/Debug/Eq/serde/Default/Send/Sync bounds. Ignored stops and
uncalled native fault factories release before handoff. Refusal or callback/cleanup
unwind closes the whole pass and releases partial state; it cannot be resumed,
finished or implicitly refilled afterward. Replaced parallel stops and unpublished
summaries also release on cleanup unwind. Native side effects are trusted work,
not rolled back or sandboxed; a second cleanup panic may abort.

Real live sequential-stop inspection and SQLite bound-group recovery share this
core while keeping original value validation, native fault codes and capture/
reservation checks. Early sequence stops still avoid observing later terminals.
Bound recovery still admits later rows before final receipt comparison. Real
forged-receipt tests refuse substitutions of parallel aggregate-output faults,
sequential overflow or original first failures without modifying stored records
or repairing the watermark. Correcting only the receipt restores original recovery
and budgets. Numeric and opaque GUI-local tests run unchanged in the independent
consumer, using the same production core.

Summary fields are mutable observations, not authentic receipts, settlement or
execution grants. Hosts own original schema/identity/capture/authority, outcome
validation, complete graph admission and same-snapshot commit-before-publish.
No source/saved body execution, fuel construction, dispatch, replay or storage
writes occur here. Complete independent durable graph recovery remains open;
this is not the complete VM or extraction gate.

### Shared Sequential Frontier Inspection

`inspect_sequential_frontier()` borrows one complete original sequential group.
Explicit inclusive limits (ceiling 64) precede hooks; empty input is structurally
valid and language arity remains native policy. Classify rows once in order:
completed members must form a contiguous prefix. A later completion refuses
before prefix decoding or blocked-dispatch observation. Then admit the completed
prefix once, followed by the blocked suffix once. The first pending member is
the active frontier, excluded from the blocked suffix. With all members complete,
the frontier is the member count; both hooks still run on their original slices,
including empty slices. No sorting, copying, normalization or allocation occurs.

Real SQLite pending-sequence recovery delegates phase ordering to this kernel.
Bounded terminal decoding and native failure/output checks precede the original
attempt/retry query. Existing faults and wire formats remain unchanged. Frozen
SQLite phase/fault parity and real late-completion, blocked-attempt/retry and
failed-prefix tests refuse before watermark repair or snapshot handoff; all
stored records remain unchanged. Repairing only invalid observations permits
normal recovery with original values and budgets. Numeric checkpoint and opaque
GUI-local proofs run unchanged in the independent consumer.

Rows and native faults need no Clone/Debug/Eq/serde/Default/Send/Sync bounds.
Uncalled hook owners release before result/error handoff; refusal or unwind stops
without retry or later hooks. Native faults move unchanged with payload-free
diagnostics. Native side effects cannot be rolled back; a second cleanup panic
may abort. Success is transient inspection, not a dispatch lease, receipt or
execution grant. Original schema/identity, terminal policy, attempt/retry evidence,
current authority, same-snapshot graph/receipt checks and commit-before-publish
remain adapter-owned. No saved body, fuel reconstruction, dispatch, storage repair,
replay or storage commit occurs here. Complete independent durable graph recovery
remains open; this is not the complete VM or extraction gate.

### Shared Successor Chain Inspection

`inspect_successor_chain()` borrows original adjacent rows in declaration order.
Inclusive bounds (2..=max_members, ceiling 64) precede every native hook. The first
row runs once; each following link receives the original previous frame and rows
once. No sorting, renumbering, payload conversion or later-link retry occurs.
Rows, frames and errors require no Clone/Debug/Eq/serde/Default/Send/Sync bounds.
Previous frames release before the next link; final frames and callback owners
release before result/error handoff. Refusal or unwind stops inspection, including
unpublished-next-frame cleanup when releasing the previous frame panics.
Native side effects cannot be rolled back; a second cleanup panic may abort.

Real SQLite chain recovery delegates to this kernel while retaining native pair
policy: bounded parent-result decode precedes next-request observation, then
original schema, authority, budgets and captured-result checks run unchanged.
Frozen SQLite fault-order tests and unchanged numeric/GUI-local consumer tests
exercise the same production core. The final row's terminal is not decoded by
this pair inspector; separate native record and receipt validation still apply.

Success is transient inspection, not a receipt, graph certificate or execution
grant. The same storage snapshot, native policy and commit-before-publish remain
adapter-owned. No saved body execution, reconstructed fuel, allocation repair,
dispatch, replay or storage commit occurs here. Complete independent durable
graph recovery remains open; this is not the complete VM or extraction gate.

### Shared Recovery Fences

`SequenceReservations` bounds numeric slot identity across a complete recovery
pass. Explicit inclusive count/sequence limits precede duplicate checks; gaps are
valid and no slot is implicitly reused. Refusal closes the pass and discards the
partial index. Successful `finish()` exposes only count/highest observations,
not a durable allocator, identity authority or committed recovery snapshot.

`validate_reservation_proof()` checks the original allocation watermark, expected
record presence and owner-namespace exclusion in that order. Native adapters must
admit original slot/schema/owner, cross-group identity and same-snapshot storage
observations. Imported high identities cannot justify repairing the watermark
before checking cold reservations. The function reads/writes no storage itself.

`validate_group_receipt()` checks complete-member/state/receipt consistency before
its once-only native receipt hook. Pending requires incomplete members/no receipt;
completed requires every member complete/one receipt. Native receipts/errors move
unchanged with no Clone/Debug/serde/Default/Send/Sync bounds and payload-free error
diagnostics. Rejected inputs and uncalled hook owners clean up before result/error
handoff; cleanup unwind stops handoff, and a second panic may abort.

The actual SQLite recovery uses all three fences inside the original immediate
transaction. Frozen state/decode fault precedence and wire formats are unchanged.
Real duplicate/cold-slot tests reject without repairing the original watermark
even with larger imported identities; correcting data permits normal recovery.
Unrelated numeric and GUI-local model schemas share these exact fences before
checkpoint commit in the exported independent consumer, without product storage.

These are transient structural observations, not authentic receipts or execution
grants. Native raw-result/value correspondence, current policy, receipt settlement,
storage atomicity, dispatch and replay remain adapter-owned. No source or saved
body is executed during recovery. Complete independent durable graph recovery
remains open; this is not the complete independent VM or extraction gate.

The reference native receipt hook now checks unbound parallel/sequential and
result-chain/dataflow receipts against a bounded data-only remerge of original
branch terminals. Exact declared field order/names/values, cancellation/failure
metadata, sequential early stops and parallel output faults use the same native
merge policy as commit. Correspondence refusal blocks watermark repair, dispatch
mutation and snapshot handoff inside the original transaction; correcting only
the bad receipt allows recovery with the original terminal values and budgets.
State/decode/terminal admission precedes this once-only hook. It never runs DSL
or a saved body. This is same-snapshot correspondence, not receipt authenticity
or cryptographic integrity against an attacker rewriting all records. Bound
expressions retain their separate native checks; no generic comparison, signature
or complete independent durable VM is introduced.

### Native Source Checkpoint

`cargo dev package leselang --output target/leselang-native-source --verify`
exports syntax, runtime-core, native HIR/session and native VM facade source as a self-contained
MIT workspace. The output parent must exist and the destination must be fresh.
`--dry-run` performs bounded source planning without writes or verifier calls.
The Rust workflow refuses source symlinks, unsafe/protected paths, duplicate or
oversized inventory and existing destinations. Failed verification or changed/
unplanned export files leave no published checkpoint. A sibling reservation
serializes cooperating publishers; this is a trusted local-filesystem workflow,
not a sandbox against hostile concurrent filesystem mutation.

Production Rust files and native/mixed tests are copied byte-for-byte from the
same implementation. Product-only modules/tests and the optional product
dependency stay behind. Explicit crate metadata replaces monorepo inheritance;
registry versions/checksums come from the original locked transitive closure.
The reference cfg name is recognized only for unchanged unreachable guards,
not an enableable Cargo feature, so exported all-features builds remain native.
Packages are `publish = false` until a separate publication decision.

The VM manifest projection removes its optional product/storage dependencies and
their explicit reference test/example targets. Its package metadata, features,
dependencies and gated targets have a closed reviewed shape: added build hooks,
unknown dependencies/features, custom target paths, duplicate or incomplete
sections fail source planning, rather than silently changing the exported profile.
The exported VM package has no enableable reference or SQLite feature; its
`native::Vm` remains the exact shared `StackedControlSession`. Upstream source
documentation for the omitted product profile does not grant that feature here.

The export has its own locked/offline CI and a separate `consumer/` workspace
whose relative dependencies point only into the exported source. `--verify`
checks local dependency paths, runs all native tests and strict lints, then runs
the consumer's checkpoint/saved-frame/group-graph/recovery-fence/captured-reentry/durable-control/scope/session/stack/GUI/native-VM tests and
strict lints. Verification uses a fresh, exclusive Cargo target directory in the
system temporary area, not the monorepo's `target` or an inherited target cache.
The exported workspace and consumer share only that invocation's disposable
build artifacts. Overlapping exports have distinct targets; no repository-wide
cache eviction or cleanup is performed. This deliberately cold-builds language
crates for the portability proof; the locked registry download cache may still
be reused. Ordinary exports without `--verify` do not compile or create a target.

The Rust exporter removes its verification cache before sealing or publishing
the source inventory. Verification and cleanup errors are both retained; failed
cleanup blocks publication even when Cargo succeeded. The owned source stage and
target also attempt cleanup during unwinding. Unix directories are created with
mode 0700, and recursive cleanup does not follow links into unrelated directories.
Abrupt process termination can still leave temporary directories; this is not a
transactional filesystem service or a sandbox for arbitrary verifier code.

`export-manifest.json` records relative file paths, exact SHA-256 and provenance;
`verified` becomes true only after those checks and exact inventory revalidation.
It is not a signature or a durable execution/authority certificate.
The manifest separately reports `native_vm_facade = true` while retaining
`complete_vm = false` and `full_extraction_gate = false`. The copied native VM
host proof runs both as a workspace integration test and through the separate
consumer's direct VM dependency, without root product metadata or assets.
The retained [macOS arm64 export proof](fixtures/leselang_native_vm_source_export_macos_arm64_20261012.json)
records the four-crate inventory, exact facade/consumer provenance and successful
workspace/consumer tests and strict lints. It is local source-portability evidence,
not a claim of fresh Linux validation or the remaining complete-runtime gates.
The newer [isolated verification proof](fixtures/leselang_native_isolated_verification_macos_arm64_20261020.json)
reruns the complete locked/offline `--verify` path with a fresh disposable cache,
including workspace/consumer tests and strict Clippy, byte-identical source
inventory, and cache removal before publication. It records a verified source
checkpoint, not completion of host-neutral durable recovery or physical extraction.

This closes the native source portability proof, **not the complete independent
VM or repository extraction gate**. Product-coupled durable recovery and reference
wire/auth/GUI compatibility remain separate; FFI and an OS shell are deferred.
The repository CI runs this export in its temporary directory outside the monorepo.

### Remaining Product Boundaries

`leselang-syntax`, `leselang-host-contract`, and `leselang-hir` have no Gewyvern
or Leserpent product crates in their dependency closures. That is a dependency
proof, not full semantic independence: the default `leserpent-profile` HIR still
names concrete runtime and UI effects, though its computation nodes specialize
shared host-parameterized IR. Disabling that optional profile removes its product
DTOs/lowerer and host-contract dependency from the native build. The reference
lowerer/type rules and host contract still contain runtime selectors and deployment
validation when enabled. The `leselang-vm` reference profile, `leselang-ui`, and
`leselang-observe` still consume Leserpent command/result types; the native VM
facade and its source export omit that profile. `leselang-command` is intentionally the product
adapter and should remain outside the standalone core.

The first extracted runtime component is `leselang-runtime-core`: admission
ownership, finite backoff, fuel accounting, portable clock/deadline and retry-delay arithmetic
plus shared closed scalar data/operation, bounded control decisions, owned fold
traversal, incremental list construction, owned capture data and borrowed lexical bookkeeping with
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
flat parallel sessions with complete local request publication, out-of-order reply
acceptance and a once-only declaration-ordered result barrier,
accepted binding re-entry with core-owned original sites and bounded native restoration,
and borrowed control sessions owning fuel/pending/accepted input with explicit poll and once-only output,
plus recursive Bind/Choose source composition with borrowed native type observations,
one shifted output meter and mandatory whole-output admission.
Bound native field/member source now borrows exact result observations from that
prefix, with a parsed structured-reply field/branch/argument/cancel vertical.
Full generic host-result/group child source compilation, complete helper body/return lowering and
registry lifecycle/opaque-native cost observation and validation policy,
opaque-effect typing/dispatch, complete suspension and durable
reply ownership still need separation. The current
VM's SQLite backend is not moved into, or required by, this lifecycle crate.

The catalog-driven expression entry above now joins shared scalar/control/call/
projection construction and whole-flow inference without a native recursive
compiler callback. Structured-device and scalar-counter parsed programs use it
with shared borrowed sessions through typed replies, re-entry and cancellation.
The multi-function entry now closes acyclic scalar-parameter helper compilation
and return capture through the same sessions without native template copying.
Both verticals also run with `leserpent-profile` disabled and its operation
contract absent. This closes expression/helper and optional HIR profile boundaries, not the full group/opaque-native,
product-adapter migration or durable VM acceptance gate below.

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

The first product-ownership migration moves `EffectOperation`, `EffectResult`,
`PresentationEnvelope`, `PresentationOperation`, `PresentationResult` and
`DebuggerCancelResult` into the existing `leselang-command` adapter. Released VM
paths are exact re-exports, so downstream callers retain type identity and wire
bytes. The network protocol now imports the adapter directly, removing its
normal dependency on the VM and the VM's default SQLite backend. A closed
62-operation/reply wire matrix, strict decoding tests, exact type identity and
reference captured-successor/replay tests guard this boundary.
`PresentationOperation::from_effect` now also owns all 62 GUI payload mappings
and the fixed wait-timeout policy in the adapter. It performs only data copying,
returns None for query/command/structured/computation effects, and is not an
authorization or validation certificate. The VM charges the same single unit,
constructs one authority envelope and validates the original request as before;
non-GUI lowering, identity and confirmation remain unchanged. Pre-migration
frozen-byte, budget, refusal-order and reply-correspondence tests also run in the
database-free reference profile. This is not a real-window GUI test or complete
adapter parity: authority envelope construction, result admission, continuation
ownership, authorization and journal recovery still use the reference VM and
host validators. No native runtime API is added.

The next product slice moves the borrowed reply/pending-effect/envelope
correspondence predicate into `PresentationResult::matches_effect`. Its closed
62-operation matrix rejects cross-operation and per-field mismatches, preserves
fixed timeouts and none/empty/present text, and retains navigation/count limits.
It does not allocate, copy payload strings, authenticate the principal or consume
an execution budget. `None` preserves legacy image-only restoration; a saved
request still passes its original operation. The VM invokes this check before
the released typed-value projection and retains the same `LSV2103` mismatch
fault and terminal replay. Before/after tests cover legacy restore, type faults
and moving owned strings; authority and full durable extraction remain separate
gates, not properties certified by the product correspondence predicate.

Product `Value` and `StructuredField` now also live in the adapter with exact
released VM re-exports. Their Serde declarations, recursive field ordering,
legacy unknown-field handling and shared scalar payload are unchanged.
`Value::result_type` owns the product type-tag map; `From<PresentationResult>`
exhaustively moves all 62 GUI reply shapes into those values without copying
strings. Correspondence is checked before conversion discards wait metadata,
and the reference VM still compares the result tag against the saved type.
Conversion and type lookup do not validate payloads or grant authority; depth,
output-count, encoded-size, revision and journal gates remain reference-owned.
Closed projection/type/wire tests and before/after nested-value characterization
cover the migration without adding a native runtime API or a second evaluator.

`Value::bounded_leaf_items` also moves the released leaf checks into the product
adapter. It borrows values without allocation, traversal, serialization or fuel
consumption. List/history use the supplied query ceiling, logs their fixed
product ceiling; empty queries return zero and empty scalar lists one. `None`
means an invalid leaf or a structure requiring caller traversal, never an empty
result. This is deliberately not complete domain-record validation, pending
reply correspondence or a new authority gate. Saved same-node navigation and
max-length values retain their previous acceptance rules. The closed 62-GUI matrix
also covers identifier/text/count limits and byte-based scalar limits without
requiring the VM or SQLite.

`validate_value_items` now owns structured traversal in the adapter as well. An
explicit stack borrows values/names without recursive Rust calls or payload copies.
`ValueValidationLimits` supplies inclusive depth, per-structure field and query
ceilings; the caller supplies the name predicate and starting depth offset. Depth
overflow is rejected after the field-name check rather than clamped; total item
counts retain saturating arithmetic. Empty structures
remain invalid even when all their hypothetical leaves would count zero. Names
are validated in declaration order, duplicates are local to each structure, and
the first child's failure precedes a later sibling's name failure. Error categories
contain no private data; host callback work is trusted and is not rolled back on
unwind. Scratch storage follows active depth and visited sibling names, not copies
of the input tree.

The VM supplies its original limits and maps `ValueBoundsError` to unchanged
`LSV2403` messages. Type admission still precedes value checks, with per-execution
output limits and encoded size afterward. This walker does not cap aggregate
input work/bytes or authenticate domain records, receipts, schemas or authority;
a saturating count alone cannot certify exact overflow-free accounting. Frozen
before/after tests preserve counts, bytes and refusal precedence at every original
starting-depth boundary. The VM-free capture/re-entry proof uses it at native
ingress; direct tests cover all 77 types, explicit ceilings, borrowed name addresses,
hook unwind and 4096-level values. Admission, durable recovery and complete
independent extraction remain open; no new wire/schema version is introduced.

`PresentationOperation::matches_effect` now owns all 62 request payload predicates
as a borrowed, exhaustive product-effect match. No cloning, serialization, fuel
debit or authority decision occurs there; non-GUI effects return false. The VM
uses the existing host signature catalog to select one GUI admission branch,
keeping image/successor/token/effect-ID checks before capability/envelope checks,
then payload matching and finally budget equality. Cross-category operations
still return `LSV2013` without inspecting an unrelated envelope. Native and wire
entry paths retain their distinct existing refusal order, including principal
decoding before wire admission. Closed cross-operation, torn-field, bound, wait
and optional-text matrices plus before/after refusal/restore tests guard this
slice. Canonical restoration, authenticated identity, dispatch, continuation and
durable recovery remain reference responsibilities; no independent-core gate is
closed by this product predicate alone.

`query_matches_effect` and `command_matches_effect` move the four query and four
command correspondence rules into the adapter as borrowed predicates. Log queries
retain no cursor and the fixed product limit; filters, runtime/session IDs,
deployment pipeline/target, command ID, idempotency key, revision, Leselang origin,
dry-run rejection and explicit confirmation keep their original behavior. The VM
still derives identity expectations from the canonical continuation token and
uses its saved revision; callers must never derive expectations from the received
envelope. Operation category and capability/schema/principal checks still precede
matching, while budget checks follow it. Before/after native, wire and restore
tests preserve fault precedence, cross-category refusals and unmodified pending
state on rejection. Adapter matrices reject torn fields and unrelated registration
commands, and explicitly demonstrate that matching is neither full canonical
validation nor authority. Dispatch, continuation ownership and durable recovery
remain reference responsibilities; no new runtime API or evaluator is introduced.

`EffectResult::matches_control_plane_effect` now owns the four query and four
command reply correspondence rules as a borrowed adapter predicate. It retains
runtime identity, all-history-entry identity, saved command IDs, applied status
and the debugger observation-time ceiling. Queries preserve legacy image-only
acceptance and do not inspect a supplied operation; mutations require the saved
command envelope. That envelope must be admitted and delivery authenticated by
the caller. A generic command reply has no operation tag, so matching must never
be used to infer authorization from the reply itself. The matcher does not check
query revisions, item limits, complete domain records or replay state. The composed
adapter entry uses the original HIR signature to fence the saved type, then matches before
query revision/output checks and owned conversion. No new payload copies or
serialization are introduced. Frozen cross-type, identity/status/time, fault-order
and dispatch matrices pass before and after migration; SQLite restart retains
the first terminal result even for a changed later receipt. Wire, budgets and
authority remain unchanged, and complete durable extraction remains open.

`EffectResult::into_control_plane_value` moves the eight control-plane reply
shapes into their released values using the original declared result type.
Generic command replies do not identify which operation was authorized; callers
must check original identity, correspondence, type and context before conversion.
A category/type mismatch consumes the reply and returns None; GUI replies retain
their separate conversion. Conversion alone does not validate status, timestamps,
domain fields, revisions, budgets or authority. The shared reply entry uses one borrowed query
revision/item gate before calling the converter, keeping revision-before-count
fault order, empty-list counts, inspect's non-list behavior and the fixed log
ceiling. No buffer clones or serialization are introduced; inspect's existing box
allocation still occurs only after those gates. All 77 declared type shapes,
released value wire, original buffer/box identity, query-policy boundaries and
memory/SQLite replay remain covered. No scheduler, schema or extraction gate changes.

`EffectResult::into_effect_value` composes these existing matchers/converters for
all 70 atomic operations. Its explicit `EffectValueContext` borrows the original
effect, declared type and optional saved operation, alongside expected revision
and query item limit. None of these values may be inferred from the received
reply. The context is ordinary data, not an authenticated pending handle; Debug
does not print its payload. Queries/GUI keep legacy matching when the operation
is absent, while mutations still require original saved command IDs.

GUI correspondence precedes its owned conversion and saved-type check.
Control-plane signature/correspondence precedes query revision, item limits and
conversion. Inspect retains its non-list policy, logs retain the fixed product
ceiling, and empty list/history replies may pass a zero ceiling. GUI/command
replies gain no new revision/item gate at this stage. `EffectValueError` carries
typed mismatch, revision-conflict or output-limit metadata; the VM alone maps it
to its existing fault codes/messages. The reply is consumed on either outcome,
with no clones, serialization, new budget, dispatch, retry or replay authority.

The reference VM and VM-free capture/successor proof use this shared entry.
Frozen cross-category/native/wire comparisons, a 70-operation declaration matrix,
query policy boundaries and buffer-identity tests preserve compatibility. Separate
tests deliberately show that conversion does not authenticate malformed envelopes.
Complete source/frame/domain/value/byte admission, live permissions, deadline,
lease, revision provenance and journal transactions remain host gates. This is
product reply composition, not a complete independent VM or durable recovery API.

`ResultValueDomain` also composes received-value checks outside the VM, implementing
the core `HostResultDomain<Value>` contract. Its public declaration takes the
original result type, explicit `ValueValidationLimits`, output ceiling and a
borrowed `ResultValueHost`, including unsized native hosts. Use core
`validate_host_result` or `PendingReply` for type-before-value checking; the
trait's value-only callback does not repeat the type gate. Traversal starts at
zero, then enforces the per-execution ceiling, then invokes the host's required
encoding check. A zero ceiling rejects even an empty query at this stage.

The host supplies name grammar and encoding/byte policy without defaults. Native
encoding errors keep their original ownership and need no Clone/Debug/codec
traits; adapter formatting never exposes their payload. No fuel, new codec,
dispatch, retry or authorization is added. Scoped pending acceptance observes
liveness around the whole domain callback, not each field hook; synchronous
native work remains host-bounded. Domain success is a transient observation,
not authenticated identity, receipt binding or durable execution permission.

The reference VM retains its original caps, bounded JSON writer and exact
`LSV2102`/`LSV2103`/`LSV2403`/encoding faults. Frozen before/after comparisons cover
type/value/count/byte refusal order and unchanged image/value owners. The VM-free
captured-successor proof uses the same domain with its own limits/codec policy;
77-type tests and direct pending-handle tests cover native errors, explicit retry,
once-only handoff, cancellation and unwind cleanup. Original authority, leases,
frame provenance and complete durable recovery remain separate host gates.

`Value::project_scalar` also keeps product-specific scalar extraction in
`leselang-command`, instead of teaching the reference VM every result variant.
It borrows a value and returns the requested owned scalar, or None for an
unsupported field. Strings/optional strings retain the original field-only
copies; collections and full replies are not cloned or serialized. It is not a
validator or a metered execution API: callers must retain input bounds, reply
acceptance and budget checks. The VM maps refusal to the same `LSV1405` and
fences contextual access to fields absent from legacy captures; intrinsic
capture validation now lives in the adapter described below.
The 77-type/12-field matrix checks the HIR catalog against released wire values;
frozen capture bytes, malformed-frame refusals and existing memory/SQLite
dataflow tests preserve the old recovery contract. No schema or product-free
durable recovery milestone changes with this adapter move.

`ProjectedResult` and `ProjectedField` are now owned by `leselang-command` and
exactly re-exported at the released VM paths. Construction, smallest vocabulary
selection, explicit legacy-version capture, ordered field/type/bounds validation,
product kind/optional-text domains and stored-field correspondence move as one
contract. The nested field remains the shared core's `ScalarProjectionField`;
no second generic validator or converted DTO is introduced. All 70 operation
shapes across versions 1-5 preserve exact field order and bytes, including v1's
omitted version. Unknown fields stay rejected; serde alone does not validate a
frame. `InvalidProjectedResult` is a payload-free adapter error which the VM maps
to the original `LSV1405` on every capture, lookup and correspondence path.

These helpers still compare only exported scalars, not the raw result's entire
operation identity; operations with no projections have empty frames. A successful
capture does not grant authority or prove trusted version provenance. Field lookup
does not implicitly revalidate mutable data or meter copies. Original host-result
admission, lexical/group access fences, shared fuel and complete journal graph
validation remain VM/host responsibilities. Frozen before/after invalid-frame
checks, exact type/buffer identity and existing memory/SQLite dataflow tests keep
that boundary explicit. Complete host-neutral durable recovery remains open.

`ProjectedBinding`, `ProjectedGroup` and `ProjectedGroupBinding` are likewise
adapter specializations of the unchanged runtime-core containers, exactly
re-exported by the VM. `ProjectedGroupExt` owns complete group capture, intrinsic
member validation and ordered signature/receipt correspondence. Callers explicitly
supply inclusive member limits and a trusted name predicate; the VM keeps its
1-63 interval and branch grammar. The adapter neither installs those defaults nor
charges a new meter. Source shape/name/atomic-operation checks and materialization
precede stored group validation, as before; callers must bound raw ingress first.
Stored validation checks count, all names/duplicates, then each result, and must
pass before signature or receipt comparison. Name-hook side effects/unwinding
are trusted native behavior and are not rolled back.

Signatures compare original order, member names and effect operation tags, not
declared branch result tags or request arguments. Captured fields alone do not
admit raw result types. Those checks, complete group authority and replay remain
with the VM/host. Refusals retain `LSV1405`, and no group normalization or legacy
field widening occurs. Before/after VM refusal tests, all 70 operations in mixed
v1-v5 groups, exact shared-type/buffer identities and memory/SQLite group recovery
cover the move. This reduces reference product coupling; it does not close the
complete host-neutral durable graph gate.

Reference continuation validation now obtains the expected result type from
`HostOperation::for_effect(...).result_type()` via the same HIR signature catalog
used by execution and binding, rather than a second 70-arm VM mapping. Lookup
does not evaluate, clone payloads, charge fuel or grant authority. Existing frame,
token syntax and program-counter checks still precede it; semantic binding and
resource checks follow it. Structured/compute frames keep `LSV2015` and a mismatched
saved result tag keeps `LSV2016`. Canonical token/source checks remain independent
restore gates, not part of the signature classifier. A frozen 70-operation fixture
tests native/request wire round trips, every cross-operation and six scalar plus
structured result tags, unchanged pending state and identity watermarks, and
SQLite reopening with each operation's saved result type corrupted in turn.
The rejected journal payload remains untouched. Before/after checks preserve
continuation schemas 1-11, journal 10, fault precedence and existing grants.
The reference catalog still uses product types; complete host-neutral durable
recovery and repository extraction remain open.

<a id="230-repository-extraction-target"></a>

## Repository Extraction Readiness

The reference operation-request policy is now adapter-owned. The VM delegates
category/capability/envelope/payload correspondence to
`leselang_command::EffectOperation::validate_request(OperationRequestContext)`;
the adapter also owns borrowed declared-authority access. The VM composes shared
frame and successor-pair admission while retaining canonical sequence/identity
allocation, budget grants, closed current/legacy request decoding and durable state.
Payload-free
adapter errors map to the same `LSV2011`/`LSV2012`/`LSV2013` faults, including the
earlier category mismatch fence. Tests cover all 70 operations without importing
the VM and compare the reference entry against its frozen pre-migration behavior.
The [macOS request-admission proof](fixtures/leselang_reference_operation_admission_macos_arm64_20261016.json)
records the baseline, parity and runtime/profile verification evidence.
This is reference-adapter convergence, not a new native-core capability or proof
of complete extraction; the native embedding contract remains `0.162.0`.

Post-image correspondence is now shared as one ordered entry,
`validate_preflighted_effect_request(EffectRequestView, canonical_sequence)`.
It checks cold successor capabilities before invoking the caller's identity hook,
then derives product IDs, validates the operation and compares every budget field.
The borrowed view imposes no traits on the opaque token; the trusted hook recognizes
canonical nonzero bounded identities, runs at most once, and does not allocate an
identity. Unwind propagates without retry, and skipped hooks release their captures.
Closed errors contain no token or payload. The complete request entry below fully
validates the frame before this low-level pass, and both current and legacy request
decoders still call the full VM admission gate. Equal forged budgets do not become
authenticated grants.
The [request-correspondence proof](fixtures/leselang_reference_request_correspondence_macos_arm64_20261018.json)
records the 24,570-case frozen operation matrix, 1,680 additional cold-body/conflicting
error cases, opaque local-token ownership and the VM-free source-to-two-requests,
wire-restored capture/reentry path with unchanged preparation and fuel. This is
product-adapter convergence, not a portable request codec or independent durable VM.

`leselang_command::lower_effect_operation` now owns the matching construction
boundary: GUI/query/command envelopes and product origin/confirmation/dry-run
policy. The VM supplies original authority/revision and a lazy command-identity
factory; GUI lowering never invokes it, while other paths invoke it once before
control-plane lowering. Fuel, sequence grammar/allocation, continuation creation,
full request admission and journal ownership stay in the VM. Moving the new image
directly into the request removes a redundant clone without changing bytes.
VM-free construction tests cover all 70 operations, owned factory cleanup and
refusal/unwind order; 1,120 before/after VM comparisons preserve wire/fuel/faults.
The [construction proof](fixtures/leselang_reference_operation_lowering_macos_arm64_20261016.json)
records this adapter migration, not independent durable-VM acceptance.

The adapter also owns released continuation schemas 1-11 in
`leselang_command::continuation_schema`; existing VM constant paths are exact
re-exports. `preflighted_result_binding_schema_version` selects binding versions;
`validate_preflighted_continuation_schema` checks group/version compatibility with
an explicit caller sequence ceiling. Both require bounded structure preflight.
These borrowed protocol checks do not authenticate tokens/groups, allocate IDs,
grant authority or recover journals. The VM keeps structure before schema, maps
refusals to unchanged `LSV1409`/`LSV2001` faults, and retains token/PC/type/capture/
budget checks and durable restoration. The frozen old frame validator passes
22,848 before/after comparisons, including legacy group-only versions, malformed
owners, capture variants and conflicting errors. The
[continuation-schema proof](fixtures/leselang_reference_continuation_schema_macos_arm64_20261016.json)
records this reference protocol migration without promoting native completeness.

Group recovery metadata now follows the same boundary: `GroupResultBinding` is
owned by `leselang-command` and exactly re-exported at the released VM path.
Its five saved fields, omission/default rules and closed decoding remain unchanged.
The adapter classifies bounded group bodies and selects the group schema; its
`successor_schema_matches` compares version/owner relationships only. Both frames
must still pass VM admission before the latter predicate is used. Equal owner
strings alone are not authentic ownership, and stored reservations are not a live
allocator grant. The private VM extension composes plan admission and keeps member/
request/capture checks, fuel ownership, inherited authority, identity monotonicity,
evaluation and journal recovery.
The [group protocol proof](fixtures/leselang_reference_group_protocol_macos_arm64_20261016.json)
records 288 frozen before/after data/classification comparisons, 4,900 version/owner
edges, exact cross-crate type identity and reference recovery regression checks.
No wire/journal version or native completeness claim changes.

Structured plan vocabulary also lives in the adapter: `MergePlan`, the 14
`ExecutionOrder` variants and the released branch-count/name constants retain
their original VM paths through exact re-exports. `MergePlan::new` keeps its
parallel-only construction, complete input conversion, original owned strings,
64-member/name limits and public `Fault`/`LSV2401` compatibility. Outer plan checks
share `validate_shape(MergePlanShapeLimits)` with the VM, in binding-presence,
successor-name, count, then name/duplicate order. Explicit shape limits never alter
the reference constructor or VM policy; raw serde still does not admit a plan.
The VM maps shape errors before checking the full group binding, and keeps all
request, authority, fuel and durable recovery gates. The
[structured-plan proof](fixtures/leselang_reference_merge_plan_macos_arm64_20261016.json)
records 1,120 frozen plan comparisons, constructor/default/closed-codec parity,
exact type/enum-variant imports and database-backed plus database-free regressions.
This moves a product protocol boundary, not the complete native durable VM.

`GroupResultBinding::validate_preflighted_plan` now owns the related group-plan
correspondence rules in the adapter. `GroupPlanLimits` explicitly supplies branch,
sequence and fuel ceilings; the reference VM continues to pass its original values.
The shared rule checks execution mode, cold flow bounds, prefix names and operation
signatures, successor reservations and ordered collision-free suffix names. It
neither changes the input nor reserves identities or spends/refills fuel. Member
slices are borrowed; successor naming retains the released finite search window.
Callers must bound ingress and preflight structure before recursive classification,
and select the original binding associated with the plan. The rule deliberately
does not validate the plan's nested binding field, outer shape or complete types:
group-plan correspondence is not recovery admission. Shared admission composes
structure preflight, correspondence, native encoding and full binding/type checks
in that order; the VM supplies capped encoding and preserves
`LSV1405`/`LSV1409`/`LSV3002` precedence. Request identity,
current authority, receipts, execution and durable journal gates remain mandatory.
The [group-plan proof](fixtures/leselang_reference_group_plan_macos_arm64_20261016.json)
records 3,360 frozen before/after comparisons across ten valid flow forms, 14 orders
and 24 corruption/boundary variants, plus collision exhaustion and VM-free adapter
tests. The existing source-compiled Seq/All vertical now composes this rule with
fixture-owned ingress/type policy before wire-restored group reentry, two captured
successors and the scalar tail. Result, preparation count and total fuel match the
original pipeline; five corruptions per mode refuse without input mutation,
preparation or fuel changes. Fixture identities and admission are not production
receipt authenticity or durable recovery. Wire schemas and native completeness
remain unchanged.

The ordered binding path itself is shared through `validate_group_binding` and
`GroupBindingLimits`, not reimplemented by the VM-free consumer. Explicit plan and
capture limits precede a caller-owned once-only encoding hook, then complete binding
typing and the scalar-result requirement. Early structure/plan refusal never calls
the encoder; native encoding errors move unchanged inside `GroupBindingError` and
generic diagnostics do not format private payloads. Native callbacks retain their
side effects, unwind propagates without retry, and uncalled captures are dropped.
The VM maps binding/plan errors to existing faults and passes its original bounded
encoder; source-compiled Seq/All saved reentry uses the same composition with fixture
policy. The [group-admission proof](fixtures/leselang_reference_group_admission_macos_arm64_20261018.json)
retains the frozen 3,360-case oracle and adds ten-form type/once-call checks, error
precedence, borrowed input, move-only thread-local errors and callback cleanup.
This is group-binding admission, not outer shape admission, a portable codec,
authentic request/receipt ownership or complete independent durable recovery.

The admitted plan/request pass is shared through `validate_preflighted_group_requests`
and `GroupRequestContext`. Native borrowed views avoid building a second request
collection. Prefix signatures/owners and exact sequential or parallel fuel precede
successor identity, captured type/group signatures, decreasing fuel and inherited
context; original prefix identities are checked last. Token text and canonical
sequence policy remain native, with bounded successor naming and explicit binding
limits. Trusted hooks retain traversal order, propagate unwind without retry and
do not confer authority. The complete graph entry below composes full plan and
request/frame admission around this pass; receipt matching and atomic journal
recovery remain mandatory. Shared inherited context reuses the core kernel without deciding fuel;
both group-member and saved-request checks now share one capture-type walker.
The [group-request proof](fixtures/leselang_reference_group_requests_macos_arm64_20261018.json)
covers 40 real memory-journal states across ten Seq/All flows and 2,880 frozen
comparisons. VM-free parsed group reentry checks original/restored rows at three
stages per mode, rejects 48 torn-row variants without preparation or fuel changes,
and retains uninterrupted output. Opaque local-token borrowing, callback refusal
order and unwind also have direct adapter tests. This does not promote the native
embedding contract or claim host-neutral durable recovery or physical extraction.

Preflighted successor-pair correspondence is shared through
`validate_preflighted_successor_pair`. Full parent/child request admission and the
parent's completed non-scalar result-domain checks still precede this entry.
The adapter then preserves schema/body, inherited context, descending fuel,
increasing canonical sequence, next-before-prior final typing and capture-extension
order. Payload-free errors map to the original native faults. Native token-text,
sequence and name hooks operate on one admitted snapshot; no new token traits,
identity allocation, meter, evaluator or journal owner is introduced.
The [successor-pair proof](fixtures/leselang_reference_successor_pair_macos_arm64_20261018.json)
records nine real pairs from seven atomic/group flow forms and 576 frozen
comparisons. Parsed ordinary/conditional three-effect source uses the same gate
after bounded wire restoration at every fuel cutoff; twelve torn-row variants
and opaque local-token refusal/unwind tests cover the adapter boundary. This
does not authenticate raw receipts, admit unbounded values or authorize recovery;
live grants, original budgets, leases and atomic settlement remain native gates.

Ordered product-frame admission is now shared through
`validate_continuation_frame` with explicit `ContinuationAdmissionLimits`.
The real VM and VM-free parsed-source consumer use the same binding-structure,
schema, native-token, program-counter, result-signature, complete-binding and
saved-budget checks in that order. Native hooks borrow opaque tokens without
Clone, Debug, serde, equality or thread traits; owner-text and token hooks run at
most once, later hooks are skipped after refusal, and native unwind is never
retried or rolled back. Closed adapter errors retain no private payload and map
to the original VM fault codes and messages. Zero remaining fuel/output retain
their historical representation; frame consistency does not grant execution.
The [frame-admission proof](fixtures/leselang_reference_frame_admission_macos_arm64_20261018.json)
retains 22,848 frozen schema/structure comparisons and adds 1,260 budget and
conflicting-fault comparisons across all 70 operations. Bounded atomic and Seq/All
wire restoration now passes the complete frame gate before request correspondence
and reentry, with unchanged preparation and fuel; twelve torn atomic frames are
refused without repair. Ingress codecs, canonical
pending-effect admission, original authority/budgets, deadlines, receipts and
atomic durable settlement remain caller-owned. No wire/journal version changes,
new evaluator or native durable-VM/extraction promotion are implied.

For requests not already admitted, prefer `leselang_command::validate_effect_request`
over manually composing frame and correspondence passes. The real VM and VM-free
source consumer now use this complete entry for original and wire-restored atomic
and Seq/All requests. `RequestAdmissionError` preserves frame-versus-correspondence
provenance without private data; native mapping retains every prior fault code and
message. Caller limits and owner-text/token-syntax/canonical-sequence hooks remain
explicit, borrow original owners and keep their once-only refusal/unwind order.
The [complete-request proof](fixtures/leselang_reference_request_admission_macos_arm64_20261018.json)
retains 24,570 operation and 1,680 cold-authority comparisons, adding 4,200 mixed
frame/request fault cases against frozen frame and request gates, including checked
decoding. Twelve torn source frames refuse before request identity; parsed reentry
keeps original preparation and every fuel cutoff. The lower-level preflighted API
remains available, not a frame certificate. Success does not admit unbounded ingress,
strengthen legacy canonical pending-effect rules, authenticate grants or authorize
recovery. Recheck mutable or decoded data; original authority, deadlines, receipts
and durable settlement remain native responsibilities, not an extraction promotion.

For a pair not already admitted, use `leselang_command::validate_successor_pair`
with explicit frame limits and a `SuccessorAdmissionHost`. The real VM and parsed
atomic/conditional and Seq/All source consumers now share the complete order: parent request,
next request, non-scalar completion, native result domain, then correspondence.
The host supplies original token/name policy and validates the completed value's
type, recursive bounds, output items and encoded size; `ResultValueDomain` supports
the latter without a VM dependency. `None` represents a non-completed native state.
`SuccessorAdmissionError` distinguishes parent, next, completion, original native
value error and correspondence. Generic diagnostics never format native payloads,
and move-only local errors require no Debug or thread traits. Hooks borrow original
owners, preserve early refusal and propagate unwind without retry; sequence hooks
run again during correspondence and must observe the same immutable policy.
The [complete-successor proof](fixtures/leselang_reference_successor_admission_macos_arm64_20261018.json)
retains nine real atomic/group pairs and 576 frozen comparisons, adding 5,400 mixed
parent/child/completion fault cases against frozen request, frame and pair gates.
VM-free source reentry preserves wire bytes, output, preparation and every fuel
cutoff; both group modes admit original and wire-restored captured successor pairs.
Direct tests cover mutated/decoded rows, empty output grants, opaque local
owners, original error ownership and unwind at every reached native hook. This
entry neither authenticates a receipt nor establishes live authority, consumes
fuel, runs saved code or publishes journal state. The native durable embedding
remains developing; no wire schema change or physical extraction is claimed.

Complete outer-plan and input-graph admission are shared through
`leselang_command::validate_merge_plan` and `validate_merge_graph`, with explicit
`MergePlanAdmissionLimits`, `MergeGraphAdmissionLimits` and a
`MergeGraphAdmissionHost`. The reference VM and VM-free parsed Seq/All consumer
now use the same order: outer shape and complete binding, group-owner syntax,
member count, each complete request before row name/owner exclusion/duplicate
identity, then bound-plan revalidation and group correspondence. Bound graphs
deliberately retain the second binding/encoding pass from the reference path;
there is no cached admission certificate. Unbound plans skip encoding and group
correspondence but still refuse members carrying group ownership. Plan encoding
failures retain their original ownership and distinguish the first and second
pass; generic diagnostics never format native payloads. Borrowed local tokens
need PartialEq for owner correspondence, not Hash, Clone, Debug, serde or thread
traits. Native text must faithfully identify original tokens, and views/policy
must stay stable throughout the call. Refusal skips later work; callback unwind
propagates without retry, fuel debit, identity allocation or journal publication.
The [complete-merge proof](fixtures/leselang_reference_merge_admission_macos_arm64_20261018.json)
retains 2,880 frozen group-request comparisons and adds 2,880 full-graph conflicting
fault cases across 40 real snapshots from ten flow forms, plus 14 unbound/identity
cases. Source reentry admits both original and decoded graphs at every saved
stage, preserving results, preparation and fuel while rejecting 48 torn rows.
Direct tests cover ten-form typing, opaque local owners, both original encoding
failures and unwind at every reached callback. This is product-adapter convergence,
not host-neutral durable recovery: bounded ingress, original grants/budgets,
current deadlines, authentic receipts and atomic storage settlement remain native.
That migration was recorded at reference contract `1.100.0`; native embedding remains `0.162.0`,
Developing at completion 10, without a physical extraction claim.

The stored-group pass now uses core `inspect_group_recovery` and
`GroupRecoveryHost` rather than SQLite-owned orchestration. The adapter first
admits owner/plan, accumulates cross-group reservations and loads original rows.
The shared pass inspects the complete member graph, short-circuit completion,
required successor chain and pending sequence, then state/receipt consistency,
bounded native receipt admission and final bound-record/reservation proofs.
`GroupRecoveryPolicy` is derived from admitted plan/state, not an untrusted choice
of which checks to skip. Pending groups run the final proof without a receipt.
The host keeps SQL, codecs, exact original receipt correspondence, current policy
and original-watermark authority; no saved body runs and nothing commits here.
Late refusal drops the decoded receipt before handoff. Move-only local owners and
native errors need no Debug, serialization or thread traits; unwind skips later
hooks without retry. Host state remains caller-owned.
The [stored-group recovery proof](fixtures/leselang_reference_group_recovery_macos_arm64_20261018.json)
covers 374 frozen comparisons over 22 real SQLite states from eight flow forms,
checking exact fault order and unchanged original tables. Standalone consumers
cover 96 state/policy combinations, graph refusal, owned native errors, every
reached hook unwind and checkpoint handoff with distinct native keys. A late record
or commit failure cannot publish their staged snapshot. The source exporter
includes the new core implementation and the same independent consumer tests.
That migration was recorded at reference execution contract `1.101.0`, not native durable
VM completeness: original authority, cross-group proofs, atomic commit and the
complete language recovery pipeline still govern the remaining extraction gate.

The complete stored-group collection now delegates to core
`inspect_group_recovery_batch`, with one bounded global reservation index. It
preserves row order: native owner/plan admission, original numeric observation,
successor reservations, execution-order proof, then the complete per-group pass.
Only after every group succeeds does it return count/maximum observations.
No later group is admitted after an earlier failure; no partial summary is
published, and original rows/errors move without payload-formatting or cloning.
The SQLite adapter retains bounded SQL/decoding, original authority/watermark
proofs and the final immediate-transaction commit. The count limit is already
covered by the journal's outer aggregate-record gate; it creates no new grant.
The [batch recovery proof](fixtures/leselang_reference_group_batch_macos_arm64_20261018.json)
records 364 frozen comparisons over 14 two-group SQLite states from eight flow
forms, including corruptions in either group and conflicting faults, with all
original tables unchanged. Standalone consumers cover global reservation limits,
duplicate/invalid sequences, original error ownership, unwind at every hook and
iterator observation, and late-proof/commit failure before checkpoint publication.
That migration advanced reference execution to `1.102.0`; native embedding stays `0.162.0`,
Developing at completion 10. This closes the collection orchestration boundary,
not authentic portable storage, complete native durable execution or physical
repository extraction.

Stored effect rows now use core `inspect_record_recovery` and
`RecordRecoveryHost` before entering checkpoint staging. The shared order is
image admission, canonical sequence, identity, deadline, owner, then state.
Pending rows prove successors without reading terminal bytes; completed rows
admit the original terminal receipt before proving successors. Thus a corrupt
state/receipt cannot hide an earlier identity/deadline/owner refusal. Native SQL
headers, bounded codecs, frame checks, original budgets and authority remain in
the adapter; admitted records grant no execution rights. Opaque local keys need
no comparison, cloning or serialization, and an unsized borrowed host is supported.
Callbacks own their internal cleanup: unused owners must be released before
constructing return values. Refusal/unwind never retries a hook or publishes a
partial checkpoint, but this is not rollback of arbitrary native side effects.
The [stored-record proof](fixtures/leselang_reference_record_recovery_macos_arm64_20261018.json)
records 784 frozen comparisons across 49 original records from eight flow forms
and 24 SQLite states, including conflicting faults and malformed unused pending
receipts, with all original tables unchanged. Independent consumers cover both
states, every reached hook refusal/unwind, original move-only local ownership,
late cleanup and late-proof/commit failure before checkpoint publication.
That migration advanced reference execution to `1.103.0`; native embedding remains `0.162.0`,
Developing at completion 10. Global graph proofs, authentic storage and complete
native durable execution remain separate extraction obligations.

Stored dispatch recovery now uses core `inspect_dispatch_recovery` through
`DispatchRecoveryHost`. The shared pass admits the original request, proves
identity/image, reads metadata, checks delivery/retry bounds and history, admits
the original retry error only when present, then checks completion agreement.
It preserves rejection order without clamping counters, eagerly decoding unused
data or reconstructing opaque native payloads. SQL, bounded codecs, schema/lease
integrity, authentic requests and the checkpoint transaction remain native.
The [dispatch recovery proof](fixtures/leselang_reference_dispatch_recovery_macos_arm64_20261018.json)
records 1,176 frozen comparisons across 49 dispatches from eight flow forms and
24 SQLite states. All original tables remain unchanged; late dispatch refusal
prevents checkpoint publication and watermark updates. Independent consumers
cover 10,584 numeric/history/state boundary combinations, move-only local owners,
unsized hosts, exact native error ownership, every reached hook unwind, cleanup,
and late-proof/commit failure. The source-export consumer reuses these tests.
That migration advanced reference execution to `1.104.0`; native embedding stays `0.162.0`,
Developing at completion 10. This is another migrated recovery pipeline, not a
complete independent durable evaluator or permission to replay settled effects.

The reference loader now uses `CheckpointRecovery::finish_recovery` and a native
`CheckpointFinalizer` for the complete commit boundary: dispatch validation,
graph validation, exact row/graph maximum observation, audit validation, watermark
reconciliation and atomic commit. No arithmetic repairs or grants are synthesized. Staged maps
are not passed to these hooks. Metadata and native errors stay locally owned
through failure and cleanup, and a closed pass invokes no hook. A successful
commit followed by unwind remains uncertain, not an automatic replay opportunity.
The [finalization proof](fixtures/leselang_reference_finalization_macos_arm64_20261018.json)
records 420 full-load comparisons over 28 original SQLite states, including nine
flow forms, real cancellation audit records, conflicting faults, watermark write
failure and deferred-foreign-key failure at actual COMMIT. Returned snapshots,
faults and all original tables match the frozen pipeline; refused transactions
roll back and release their locks. Independent consumers test the five ordered
phases, original full-width maxima, move-only metadata/error ownership, every hook
unwind, destructor failures and commit-then-unwind uncertainty.
Product audit identities/origins, request authority, concrete graph/receipt proofs,
SQL/schema policy and storage atomicity stay native rather than coupling the
language core to Leserpent. That migration advanced reference execution to `1.105.0`; native
embedding remains `0.171.0` after decoded-checkpoint successor convergence, still Developing at
completion 10. This closes the ordered
finalization boundary, not complete native durable execution or repository extraction.

### Product Request Codec Ownership

The command adapter now owns the closed current-operation and legacy-query
request envelopes through `decode_effect_request` and `DecodedEffectRequest<C>`.
The continuation representation belongs to the caller: the released VM supplies
its original `ContinuationImage`, while the VM-free source/re-entry fixture uses
its existing numeric-token frames. That fixture now decodes the same canonical
product envelope before its unchanged frame/request admission.

The VM retains its public `EffectRequest` fields, canonical serialization and
complete post-decode validation. Only serde envelope parsing and legacy query
normalization moved. Current-before-legacy matching, rejection of unknown,
missing, duplicate or mixed fields, nested codecs, optional absolute deadlines,
and the original untagged error name remain unchanged. No round-trip conversion,
new meter, token format, capability grant or dispatch is introduced.

Decoded data is deliberately untrusted. Successful decoding is not semantic
budget validation, current authority, authentic continuation provenance or
permission to replay. Callers still bound ingress and run full frame, request,
budget and live-policy checks. Streaming callers also enforce end-of-input or
their transport framing; the decoder reads one value, not an entire stream.
Native continuation values need only Deserialize,
not Clone, Debug or Serialize; borrowed JSON continuations can retain the
original input lifetime.

The [request codec proof](fixtures/leselang_reference_request_codec_macos_arm64_20261020.json)
records frozen comparisons for 70 operation shapes plus four legacy queries:
74 valid canonical outputs, 1,332 malformed inputs and 592 semantic-admission
failures retain their exact bytes or errors. The existing VM-free captured
source/successor pipeline and released journal suites use the migrated decoder.
That migration advanced reference execution to `1.106.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. This removes a real product protocol boundary
from the VM without claiming complete native durable recovery or repository
extraction. Wire and journal schema versions do not change.

### Product Debugger Audit Ownership

The command adapter owns `DebuggerAuditContext`, `DebuggerAuditRecord`, bounded
identity validation, attribution projection/matching and exact persisted origin
labels. The released VM paths re-export those same types, with unchanged fields,
serde behavior and byte order. The observer constructs audit metadata directly
from the command adapter rather than importing product DTOs through the VM.
Both journal backends use the same adapter projection and comparison functions;
SQLite keeps its own row decoding and maps unknown origin labels to its original
`InvalidQuery` error.

Audit metadata is not authority or proof of a successful cancellation. Projection
does not validate or commit. Attribution matching intentionally excludes the
original timestamp and cannot compare the idempotency key or continuation,
which are absent from a record. The journal still checks those persisted
identities and the authoritative terminal state before accepting an exact replay.
Cancellation admission order, current operation checks, conflict handling,
retention, recovery validation and transactional publication are unchanged.
The adapter returns an identity error; the reference VM retains the exact
`LSV2015` mapping. No product policy moves into the host-neutral language core.

The [debugger audit proof](fixtures/leselang_reference_debugger_audit_macos_arm64_20261020.json)
records 270 valid wire comparisons, 5,670 malformed wire comparisons and 3,945
native identity checks against frozen pre-migration logic. Direct VM-free
adapter tests exercise all five origin labels and the non-authoritative metadata
boundary; the existing memory, SQLite reopen/conflict and observer suites remain
the end-to-end cancellation checks.
That migration advanced reference execution to `1.107.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. This is a product adapter migration, not
complete host-neutral durable recovery or permission to split the repository.
Product version, wire schemas and journal schema remain unchanged.

### Product Semantic Retry Ownership

The command adapter now owns `RetryPolicy`, `EffectErrorClass`, `EffectError`,
the released default/maximum policy constants, error metadata validation and
`plan_effect_retry`. Public VM paths re-export the original shapes and values.
The reference VM still owns `RetryDisposition`, terminal steps and journal
publication. The adapter delegates numeric retry counts, capped exponential
backoff and checked ready-times to the existing runtime-core planner; there is
no second scheduler, timer, retry registry or fuel grant.

Decision order remains part of compatibility. VM clock admission, deadline
cleanup and authoritative terminal replay precede the adapter. Inside the
adapter, invalid error metadata precedes classification; permanent errors do
not inspect retry policy or clock. Transient errors validate product policy
before the shared clock/budget planner, even when the retry budget is already
exhausted. Generic planning callers must still validate ingress clocks and
current authority themselves. A numeric proposal is never permission to
dispatch, reuse an expired lease or reset an execution deadline.

The original `LSV2201`/`LSV2202`/`LSV2203` mapping stays in the reference VM.
Persisted error validation uses the same adapter metadata rules. Serde decoding
remains separate from semantic admission, including the released acceptance of
unknown metadata fields; this migration does not silently tighten the wire
format. Retry payloads are borrowed during planning and retain their ownership.

The [retry policy proof](fixtures/leselang_reference_retry_policy_macos_arm64_20261020.json)
records 60,480 frozen planning comparisons, 3,156 native metadata checks and 164
wire comparisons with unchanged serialized bytes or exact diagnostics. VM-free
adapter tests cover policy/error precedence and clock overflow. The original
memory/SQLite retry pipeline now accepts adapter-owned types directly and
continues to verify saved fuel/deadlines, stale leases, restart, retry exhaustion
and terminal replay before late invalid errors or policy.
That migration advanced reference execution to `1.108.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. Product version and wire/journal schemas stay
unchanged; complete host-neutral durable recovery and repository extraction
remain separate gates.

### Product Merge Completion Ownership

The command adapter now owns the actual completion-set admission and ordered
assembly used by the reference VM, through `merge_preflighted_completions`.
It checks the explicit output ceiling and completion count, rejects unknown or
duplicate branch names, validates every outcome and applicable member binding,
then checks the saturated aggregate item count. Only after complete admission
does it select the first terminal outcome or assemble fields in declaration
order. Arrival order never chooses the winning terminal. Prefix mode preserves
all fields, while completed chain/dataflow modes select their declared final
value.

The bridge retains the existing VM `BranchCompletion`, `BranchOutcome`, `Step`,
continuation checks and fault mapping. The adapter consumes these through
`MergeCompletionHost`; native terminals and errors need no Clone/serde/Send
bounds. Callback hosts are trusted validators, not a sandbox or external-effect
rollback mechanism. The function requires a fully admitted bounded plan and
binding before entry; its output is uncommitted data, not an authenticated
receipt or permission to execute/replay.

VM order remains full plan admission, adapter assembly, final value validation,
bounded wire encoding, then saved-binding completion where applicable. Journal
ownership, publication, capture/successor handling and original fuel remain
unchanged. No replacement evaluator, collector session or persistence format is
introduced.

The [merge completion proof](fixtures/leselang_reference_merge_completion_macos_arm64_20261020.json)
records 9,720 frozen outcome comparisons across six admitted plan forms, both
completion orders, five budgets and prefix/final modes, plus 432 malformed-plan
and completion/refusal comparisons. Exact steps, fault precedence, serialized
bytes and the input plan are preserved. Direct VM-free tests cover move-only
terminal/error identity, late validation failure after an early stop, saturating
counts, early refusal without native hooks and declared final-value selection.
The existing memory/SQLite group, capture and successor suites exercise the
migrated production path, including modes beyond the six frozen fixture forms.
That migration advanced reference execution to `1.109.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. This is product algorithm migration, not
complete native durable recovery or repository extraction; product, wire and
journal versions stay unchanged.

### Product Bounded JSON Ownership

The product adapter's `leselang_command::bounded_json::{encode, measure}` now
owns the shared compact-JSON output bound. Command-plan encoding and the released
VM's continuation, request, journal and merge-output paths use the same writer.
`measure` counts writes without an output buffer; `encode` returns an output
buffer only on success. Both borrow the input, invoke its serializer once and accept
non-Clone, non-Send and borrowed values. Their typed errors carry the first
attempted byte count or the original serialization error, not VM fault codes.

VM wrappers retain the exact `LSV3001`/`LSV3002` labels and messages. Plan wrappers
retain `PlanCodecError`, including its attempted-size field and validation-first
order. Recursive structure admission still precedes continuation serialization;
semantic validation, authority, budget defaults, journal transactions and wire
schemas remain with their existing owners. The native source export gains no
JSON codec or product dependency from this migration.

The [bounded JSON proof](fixtures/leselang_reference_bounded_json_macos_arm64_20261020.json)
records 9,800 frozen boundary cases across all 70 released operation requests and
their images, operations, budgets and effect steps, checking both encoding and
size validation. Another 200 plan cases preserve bytes, attempted sizes and
validation order across all five origins. Direct tests cover UTF-8/escaped-byte
bounds, borrowed local serializers and native serialization errors.

Overflow is now sticky even if a custom serializer catches a write error: it
cannot turn truncated output into successful encoding or successful measurement.
This is an intentional hardening of that malformed custom-serializer case, not
a change to released derived DTO encoding. The helper is not a CPU, recursion
or allocator-capacity sandbox; it neither validates arbitrary custom JSON output
nor rolls back serializer side effects. A successful measurement is not a permit
to skip bounds on a later encoding of a stateful serializer.

That migration advanced reference execution to `1.110.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. `complete_vm` and `full_extraction_gate` remain
false; product version stays `2.4.0`, with no new native API or durable-recovery claim.

### Product Retention Policy Ownership

`leselang_command::retention_policy` now owns the released `RetentionPolicy`,
`CompactionReport`, default retention/batch sizes and batch ceiling. The old VM
paths re-export the exact types and constants. Field names, default values,
unknown-field decoding and `LSV2301` refusal remain unchanged. Both memory and
SQLite journals use the same pure `removal_count` calculation after policy
admission; the owning journal still supplies its total-record ceiling.

A count is not eviction authority. Journals still identify eligible logical
units, protect live group members, order candidates and own deletion/replay
reconciliation. SQLite retains its immediate transaction, row checks and audit
cascade; no schema or durable transaction boundary is changed.

The memory implementation now measures all selected payloads with the shared
bounded JSON writer before deleting any map entry. Byte accounting no longer
materializes temporary JSON buffers. If a later payload fails its encoding
bound, all selected dispatches, groups and audit rows remain unchanged instead
of losing an earlier part of the batch. Commit then applies the preflighted
token/group removals under the original exclusive journal borrow. This is
in-process failure protection, not crash durability or allocation-failure recovery.

The [retention proof](fixtures/leselang_reference_retention_macos_arm64_20261020.json)
records 168 policy-admission and 1,008 batch-sizing boundary cases, including
full-width counts; 20 frozen successful-compaction comparisons preserve exact
reports, pruning order, live-group protection and audit state. Five isolated
corrupt-state injection sites cover plan, merged result, local completion,
request and retry-error encoding. A red/green test demonstrates the former
partial mutation and the corrected whole-batch refusal; these private fixtures
do not claim that invalid payloads can pass normal public admission.

That migration advanced reference execution to `1.111.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. Product version stays `2.4.0`; full extraction
and complete native durable recovery remain separate, unsatisfied gates.

### Product Scheduler Policy Ownership

`leselang_command::scheduler_policy` owns `SchedulerLimits`, `SchedulerPressure`,
policy admission and the product-facing capacity checks. The VM's released type
paths are exact re-exports; its private bridge retains `LSV2500` through `LSV2503`
and their exact messages. `DispatchClaim` stays a concrete VM enum, including
variant imports and its existing owned lease payload, rather than becoming a
generic alias with incompatible Rust usage.

Journal storage, retention and default scheduler limits now share one product
`MAX_JOURNAL_RECORDS` constant, re-exported from the original VM path. The adapter
delegates arithmetic to the existing runtime-core dispatch-capacity functions:
oversized batches take precedence over transient pressure, full-width addition
cannot wrap, and lowering a limit does not revoke an existing lease.

These structs and checks provide no current-state certificate, reservation,
queue or lease. Policy admission is separate from decoding. Journals still count
outstanding work, decide eligibility before reporting idle/backpressure, and
check capacity under their original owner or SQLite transaction before
publishing. A stale or constructed pressure snapshot grants no dispatch authority.

The [scheduler-policy proof](fixtures/leselang_reference_scheduler_policy_macos_arm64_20261020.json)
records 49 policy-admission comparisons, 3,136 capacity/pressure combinations
with exact faults and serialized snapshots, and 1,044 closed-shape/claim-wire
comparisons including all 70 released operation requests. Direct adapter tests
cover zero/full-width limits and observation-only behavior. The existing
backpressure integration suite now imports adapter policy types directly and
exercises both journals, stale workers, global capacity, lease fencing, lowered
limits, idle eligibility, retries, restoration and rollback.

That migration advanced reference execution to `1.112.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. The Cargo product version remains `2.4.0`.
No native API, wire/journal schema, new scheduler or full durable-extraction
claim is introduced.

### Product Bounded Decode Ownership

`leselang_command::bounded_json::decode` completes the product codec boundary:
command plans, reference continuations and SQLite journal payloads now delegate
their byte-limited JSON parsing to the same adapter implementation. The generic
result may borrow from the input buffer. The separate `BoundedJsonDecodeError`
does not add variants to the released encoding-error enum.

The full input length, including whitespace and UTF-8 bytes, is checked before
any deserializer runs. The existing serde recursion limit, complete-input
requirement, DTO-specific field rules and parser diagnostics are preserved.
The plan wrapper still checks schema before semantic admission; the continuation
wrapper still validates the restorable image after decoding; journal recovery
still owns graph validation, transactions and snapshot publication. Existing
`PlanCodecError`, `LSV3002`/`LSV3003` and `LSV4006`/`LSV4007` mappings are unchanged.

This is a byte boundary, not an allocation/CPU quota or a sandbox for custom
deserializers. Successful parsing is not evidence of semantic validity,
authorization or recoverability. A custom deserializer's side effects after
entry are not rolled back, including when trailing input is later rejected.

The [bounded-decode proof](fixtures/leselang_reference_bounded_decode_macos_arm64_20261020.json)
records 240 frozen command-plan comparisons, 980 continuation comparisons over
all 70 released operations, and 7,560 journal payload comparisons. Direct codec
tests cover borrowed output, pre-serde size refusal, exact parser diagnostics,
recursion rejection and UTF-8/whitespace accounting. Thirty-two corrupt-image
reopen cases cover atomic, sequential, parallel and captured-effect programs
in pending and completed states: rejected reopen leaves the original stored
rows unchanged, and repairing the test fixture permits reopening again.

That migration advanced reference execution to `1.113.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. The Cargo product version remains `2.4.0`.
There is no new native API or wire/journal schema change; this migration does
not complete host-neutral durable recovery or the full extraction gate.

### Product Identity And Terminal Data Ownership

The product adapter now owns `ContinuationToken`, `CancellationReason`,
`Cancellation`, `BranchOutcome` and `BranchCompletion`. Released `leselang_vm`
paths are exact reexports, not wrapper conversions or generic enum aliases:
`use leselang_vm::BranchOutcome::Cancelled` and cancellation-reason variant
imports remain valid. String/terminal wire layouts, Debug output and the
original per-type unknown-field policy are preserved.

`leselang_command::continuation_identity` also owns lexical token checks,
canonical continuation/merge sequence observations, the product sequence ceiling
and legacy scheduling/retention age ordering. These are distinct rules:
`continuation-01` is not a canonical restorable identity, but its legacy age
ordering still compares equal to `continuation-1`; derived `Ord` remains lexical.
Journal group reservation still combines canonical observations with the saved
binding reservations under its original recovery policy.

The token's inner field stays private. `ContinuationToken::from_wire(String)`
moves raw wire/storage data without normalization or semantic admission, like
the existing transparent deserializer. It can retain invalid data so callers
reject it at the original boundary and preserve error priority. Neither raw
construction, canonical parsing nor terminal data grants execution, cancellation,
lease or restore authority. The reference VM retains full request/restore/outcome
admission, allocation and cancellation; execution observations move in the later
section below. Journals
retain transactional sequence watermarks, graph checks and finalization.

The [identity/outcome proof](fixtures/leselang_reference_identity_outcome_macos_arm64_20261020.json)
records 65 identity cases, 4,225 pairs comparing both legacy and lexical order,
and 16,232 frozen wire/field-policy comparisons across all five moved types.
Direct adapter tests check 640 ASCII/length combinations, original string-buffer ownership,
numeric boundaries and concrete variant imports. Public memory-profile tests
use adapter types with exact VM type identity and demonstrate that constructing
or decoding cancellation data does not cancel live work. Clock-boundary tests
import adapter cancellation reasons across both memory and SQLite paths.

That migration advanced reference execution to `1.114.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. Product version remains the Cargo workspace's
`2.4.0`; no native API, wire/journal schema or completed durable-extraction claim
is introduced by this product ownership move.

### Concrete Product Request Ownership

`leselang_command` now owns the concrete `EffectRequest` and the exact
`ContinuationImage` specialization. Existing `leselang_vm` paths reexport the
same Rust types. There is no wrapper, wire translation or buffer copy. Product
consumers can decode and inspect requests without depending on the VM or SQLite.
`EffectRequest::as_view` borrows the original fields, not a validation certificate.

`leselang_command::reference_request` also owns the released static ceilings,
binding/frame consistency policy and its original LSV diagnostics. The generic
`leselang_command::validate_effect_request` entry still takes explicit host
policy; it is not replaced by product defaults. The concrete deserializer uses
the unchanged current/legacy closed codec, then checks the full frame before
cold successor authority, canonical request identity, operation correspondence
and equal budgets. Both semantic refusal order and serde diagnostics are retained.

Checked decoding is not bounded ingress, authentic capability grants, live clock
admission, canonical source admission or recovery permission. The plain image
alias still decodes raw data. Request fields remain mutable; VM restore rechecks
them through adapter-owned bounded/canonical admission, then retains existing-state
conflicts, journal transactions and sequence allocation. No decoding operation
creates a pending task or changes the source VM. Hosts must supply trustworthy
authority and decide when to call restore.

The [concrete-request proof](fixtures/leselang_reference_concrete_request_macos_arm64_20261023.json)
reuses the independent frozen semantic validator for all 74 valid current/legacy
wire cases, 1,332 malformed shapes and 592 semantic decode refusals. Direct
adapter tests exercise no-copy views, conflicting-fault priority, mutable-data
revalidation, raw-frame versus checked-request admission and explicit byte limits.
The public memory-profile test uses adapter-owned requests through actual VM
restore/resume, including refusal after a decoded budget is changed.

That migration advanced reference execution to `1.115.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. Product version stays `2.4.0`. This is removal
of a product ownership dependency, not a new native API, schema migration,
complete host-neutral durable VM or completed extraction gate.

### Execution Observation Ownership

The product adapter owns `NamedEffectRequest`, `StructuredEffectBatch`,
`MergeWait`, `Step`, `RetryDisposition` and `DispatchClaim`, plus the existing
runtime-core specializations `DispatchLease`, `FailedEffect` and `RetrySchedule`.
Released `leselang_vm` paths reexport exactly the same types. The three enums
remain concrete, so imports such as `use leselang_vm::Step::Done` continue to
work; neither wrapper conversions nor generic-enum aliases are introduced.
Owned boxes, request buffers and payloads move without copies.

`leselang_command::execution_outcome` describes observations, not execution
permissions. Serialization preserves field order, tagged variant names, omitted
lease retry defaults and each type's original unknown-field policy. In particular,
`DispatchClaim` remains closed while the other outer observation types retain
their existing permissive fields. Nested requests still use full checked request
decoding. Raw yield images, terminal data, group counters and batch names do not
gain new semantic validation or state-transition authority merely by decoding.

The reference VM and journals still own group membership, output bounds, original
terminal admission, cancellation, live lease attempts, clocks, retry decisions and
atomic completion. Journal terminal checks and merge-outcome checks retain their
distinct legacy policies; this migration does not silently unify or tighten them.
An apparent `Done`, `Cancelled`, `Scheduled` or `Leased` value is not proof that
any corresponding transition occurred. Hosts must authenticate and correlate
observations and bound ingress before using them.

The [execution-observation proof](fixtures/leselang_reference_execution_outcome_macos_arm64_20261023.json)
records 46,224 frozen wire/Debug/error comparisons across all 70 operations,
including empty/oversized batches, invalid counts and all execution variants.
Another 630 comparisons check nested request refusals versus raw yield data.
Twelve journal cases preserve terminal-admission differences. Direct adapter tests
check concrete variant imports, box/buffer identity, field policy and legacy lease
defaults. The actual memory VM test proves all nine old/new type identities, then
decodes a claimed completion and a forged delivery attempt: neither settles live
work, and only the original current lease can complete it.

That migration advanced reference execution to `1.116.0`; native embedding stays `0.171.0`,
Developing/Draft at completion 10. Product version remains `2.4.0`. No wire or
journal schema, native API, complete durable VM or full extraction gate changes.

### Optional Observation VM Bridge

`leselang-observe` no longer needs to link an evaluator to project received
requests, sanitize runtime logs or inspect a debugger cancellation plan. Those
read-only paths use the exact adapter-owned `EffectRequest` and its original
static validator, plus the shared `Fault` type. Mutation-result DTOs and all
existing diagnostic variants remain available without the executor.

| Configuration | Available Behavior | Dependency Boundary |
| --- | --- | --- |
| `default-features = false` | Waiting debugger/log projections, cancellation-plan inspection and observation DTOs | No VM or SQLite, including test dependencies |
| `default-features = false, features = ["vm-bridge"]` | The above plus the existing audited cancellation functions | Reference VM with memory journal; no SQLite |
| Defaults, or `features = ["sqlite-journal"]` | Existing released behavior | Reference VM with optional durable storage enabled |

The default is intentionally unchanged for existing consumers such as
`leserpentd`. The public `execute_debugger_cancel` and
`execute_debugger_cancel_effect` paths reexport the feature-gated bridge; there
is no stub or silent mutation fallback when the bridge is absent. A read-only
consumer should depend on the crate with `default-features = false` explicitly.
Cargo features are additive, not an authorization boundary: another dependency
can enable the bridge or SQLite for the same package in a shared build.

The bridge retains the original order: plan inspection, bounded/canonical
continuation encoding, projection/image correspondence, dry-run handling, then
the VM's audited cancellation. Even dry runs still validate the original image;
an effect-level dry run still refuses to masquerade as a completed cancellation.
Original pending state, trusted clocks, journal commit and audit replay remain
authoritative. Successful request decoding, UI projection, plan inspection or
decoding an `Applied` observation does not authenticate a caller, establish a
live continuation or grant cancellation permission.

The [observation-boundary proof](fixtures/leselang_reference_observe_boundary_macos_arm64_20261023.json)
records separate read-only, memory-bridge and default test/lint runs, dependency
closure checks and the actual daemon debugger tests. Read-only tests construct
received data without a VM and preserve sanitation, mutable-request rejection,
deadline saturation and revision/confirmation checks. Bridge tests additionally
reject unknown and altered continuations without creating audit records, then
cancel the original live request and replay its original audit timestamp.

That migration advanced reference execution to `1.117.0`; native embedding remains `0.171.0`,
Developing/Draft at completion 10. This is a product observation boundary, not a
host-neutral UI adapter or complete standalone VM. Product version stays `2.4.0`;
native durable recovery and the full extraction gate remain incomplete.

### Static Continuation Codec Ownership

The product adapter now owns `encode_continuation`, `decode_continuation` and
the unchanged 64 KiB `MAX_CONTINUATION_BYTES` ceiling. The old `leselang_vm`
paths reexport those exact functions. Protocol consumers can read, check and
write product continuation data without linking an evaluator or SQLite.
`reference_continuation` also supplies the separate static preflight stages used
by the VM and its journal, without turning those stages into an execution API.

The original ordering is preserved rather than unified:

- Encoding checks recursive result-binding structure, measures bounded JSON,
  validates the complete frame, checks canonical identity and pending-effect
  source, then emits bounded bytes.
- Decoding checks raw input length before JSON parsing, retains serde recursion,
  unknown/duplicate-field and legacy-schema policies, then performs the static
  frame/identity/source checks. Exact-limit whitespace remains valid.
- Size-only journal preflight still stops before later request admission; it must
  not be treated as a validated request. Full size validation omits the output
  buffer but retains all encoding admission checks.

All original LSV codes, messages and wire bytes remain unchanged. The name
`validate_restorable_image` denotes the released **static** gate only: a decoded
group member still lacks a complete group journal, and an otherwise valid image
may conflict with live or already-consumed state. VM restore still checks those
conditions, records pending state and advances the identity watermark only at
its original boundary. Hosts must authenticate inputs and decide whether to
invoke restoration; neither successful decoding nor canonical source is a grant.

The observation mutation bridge imports encoding from the product adapter and
only `Vm` from the evaluator. `leserpentd` imports operation/result/step/scheduler
data from the adapter, including its test consumers; its VM import is limited to
`Vm` and the engine's `DEFAULT_FUEL`. Existing cancellation and replay behavior
is not reimplemented in those consumers.

The [continuation-codec proof](fixtures/leselang_reference_continuation_codec_macos_arm64_20261023.json)
records 7,560 independent frozen comparisons across 70 operations and 2,652
schema/group/binding comparisons, plus byte-limit and recursive-preflight cases.
Four direct adapter tests run without a VM. A public memory-runtime test uses
the adapter codec, rejects conflicting live images and incomplete group restore
without reserving identities, and preserves consumed-state refusal after actual
completion. Existing malformed-decode tests now use the independent frozen
static-admission oracle rather than calling the migrated validator as expected output.

Reference execution advances to `1.118.0`; native embedding stays `0.171.0`,
Developing/Draft at completion 10. Product version remains `2.4.0`. This does not
introduce a native codec, change a wire/journal schema, complete host-neutral
durable recovery or satisfy the full repository-extraction gate.

**The current 2.3.0-and-later phase closes acceptance gates; physical extraction is deferred.**
It is not an automatic version-triggered move. No replacement extraction version
or date is assigned. Continue the shared implementation in this monorepo until
complete vertical and compatibility proofs pass; do not trade reliability for a
release label or duplicate the runtime to manufacture apparent independence.
The existing language and free Leserpent capability set remain the compatibility
baseline; this track removes product coupling rather than adding an OS shell or
another GUI framework. Prioritize complete vertical proofs over more isolated helpers.

| Stage | Required Exit Proof |
| --- | --- |
| 2.2.0 foundation | Host-defined catalogs feed generic typed IR, evaluation, values and suspension frames; one unrelated host compiles, runs, yields, resumes and cancels without product DTOs or mandatory SQLite |
| 2.3.0 and later convergence | Two unrelated host schemas pass the same complete language pipeline; exact versions, unknown operations, wrong typed replies and denied capabilities fail before unsafe dispatch; optional durable storage proves restart, stale-reply rejection and no duplicate settled effects |
| Repository move | Existing Leserpent wire/auth/GUI parity remains green; the complete core has product-free dependency closure, its own source manifests, CI, docs, MIT license and publish boundaries; a clean external consumer builds without monorepo metadata/assets |

Catalog-only tests prove registration/selection/binding and generic node reuse.
Additional parsed source/session verticals now prove native typed calls, scoped
helper/group operands, out-of-order member replies, bounded returns and cancellation
without the product profile or SQLite. The prepared-helper profile also covers an
unrelated headless GUI schema with heterogeneous string/boolean acknowledgements,
current widget-generation leases and terminal enclosing-return exhaustion.
These shared in-memory profiles are not the
complete independent VM or durable extraction gate. Remaining blockers include
reference adapter migration/compatibility, product-coupled recovery frames and
host-neutral durable recovery. Native source metadata/CI now have the checkpoint
proof above; complete runtime publication remains separate. Mandatory
VM SQLite linkage has been removed by the optional backend above; product
contracts remain in the reference VM's database-free dependency closure.
The next work is complete pipeline convergence, not increasing the number of
public helpers as a progress proxy:

1. Run the existing language capability set through one complete in-memory
   source-to-effect/reply/capture/cancel pipeline for two unrelated hosts, rather
   than accepting only restricted prepared-expression profiles.
2. Separate native VM operation/result ownership from Leserpent product DTOs.
   Disabling `sqlite-journal` removes storage linkage, not those product types.
3. Close host-neutral durable graph recovery and saved-frame compatibility,
   including stale receipts, settled-effect deduplication and original budgets.
4. Re-run full reference-adapter wire/authority/GUI/CLI and journal migration
   parity before moving the complete core and its consumer/CI into a new repository.

`leselang/language-vm/host-neutral-embedding` remains developing. The isolated
source checkpoint contains syntax, runtime foundations, native HIR and the native
VM facade, not the
complete reference VM; its `complete_vm` and `full_extraction_gate` flags remain
false. The planned `crates/leselang-vm/tests/host_embedding.rs` proof is not yet
present and must not be treated as passed evidence. The status tensor remains the
single source of truth; this section defines exit proofs, not a second progress table.
Retain the shared implementation and record missing evidence instead of shipping
a misleading split. Reassess after complete host verticals or recovery/parity
gates close, not whenever a version number changes.
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
