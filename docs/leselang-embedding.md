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
external side effects or total process usage. The core has no evaluator yet, and
the reference VM still owns cost rules, continuation validation and durable replay.
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
proof, not full semantic independence: HIR still names concrete runtime and UI
effects, and the host contract contains runtime selectors and deployment
validation. `leselang-vm`, `leselang-ui`, and `leselang-observe` still consume
Leserpent command/result types. `leselang-command` is intentionally the product
adapter and should remain outside the standalone core.

The first extracted runtime component is `leselang-runtime-core`: admission
ownership, finite backoff, fuel accounting, portable clock/deadline and retry-delay arithmetic
and the shared `Fault` DTO.
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
and CI, not a blind directory copy. Generic typed operation catalogs, evaluator
values/suspensions and optional persistence still need separation. The current
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
