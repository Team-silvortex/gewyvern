# Leselang Runtime Core

Host-neutral lifecycle, accounting and clock foundations for the future standalone
Leselang runtime. The current components are **bounded pre-admission scheduling**
and **fuel/clock/backoff arithmetic**, not a parser, evaluator, complete embedded language,
durable queue or GUI adapter.

## Boundary

The crate's only normal dependency is `serde`; `serde_json` is test-only. It has
no dependency on any other workspace crate, product DTO, SQLite, async runtime,
GUI toolkit, account service or generated asset. Inputs and successful outputs
are opaque host types, with no `Clone`, serialization or language-HIR requirement.

`Admission<Input>` owns one immutable request. The host implements
`AdmissionAdapter<Input>` and explicitly returns `Started`, `Backpressured` or
`Rejected`. The core never classifies a host failure by its error-code string.
`Backpressured` MUST mean no work was accepted or published; only the host can
enforce that atomicity and validate capabilities, revisions and resource bounds.
Unused monotonic identity reservations may leave gaps; they are not accepted work
and must never be reused. Admission is not a promise of gap-free identifiers.

`AdmissionPolicy::validate()` is a pure preflight available before constructing
input or granting request ownership. It names the invalid constraint in fixed,
bounded diagnostics: attempts, base delay, maximum delay, then delay ordering.
Submitted values are not echoed. `Admission::new` applies the identical preflight
before clock/deadline validation; preflight does not bypass constructor checks.

The handle has finite attempts, capped exponential backoff, a submission-pinned
deadline and local cancellation. Polling before its wakeup does not call the
adapter. Terminal outcomes drop the held input and cannot return a start twice.
Clock errors leave active state unchanged. Fault codes and JSON shapes retain
compatibility with the reference VM; `LSV` codes do not imply a product dependency.
If a callback panics, unwinding propagates rather than fabricating a host result,
but the handle is consumed and its input released. A caller catching that unwind
cannot poll it into replaying work whose publication state is unknown.

Admission handles and poll outcomes are `#[must_use]`. An ignored handle discards
pending input; an ignored `Started` outcome does **not** cancel accepted host work.
The compiler also warns when `poll(...)?;` handles the outer `Result` but discards
the outcome. Hosts should exhaustively route each outcome; an explicit `let _ =`
or `drop` still permits intentional disposal. These are compile-time diagnostics,
not guaranteed result delivery, runtime cancellation or exactly-once execution.

Thread ownership is conditional on the opaque input: `Admission<Input>` is `Send`
when `Input` is `Send`, without requiring `Sync`. A waiting handle can move to a
new host worker without resetting its attempts, clock watermark or deadline.
No `Send`, `Sync`, `Clone` or `Debug` bound is imposed on adapters or successful
outputs. GUI-local `Rc` inputs/adapters/outputs work on their owning thread; they
cannot be moved across threads. The host must preserve identity, authority and
clock domain across any handoff and obey its framework's dispatcher affinity.
Bounded rendezvous tests prove independent callbacks can overlap, cancelling one
handle does not cancel a sibling, and callback unwinding does not poison another
instance. This is admission-lifecycle evidence, not a worker pool, automatic GUI
dispatch, thread-safe shared mutable VM, or independent evaluator proof.

Exhaustion (`LSV2512`) reports only the bounded attempt count, for example
`admission exhausted after 1 attempt` or `admission exhausted after 32 attempts`.
It never copies the last backpressure code/message into the generated fault,
regardless of its size or content. Direct `Rejected(Fault)` values from the host
remain unchanged: the core does not invent a message-sanitization or retry policy
for them. Hosts still own diagnostics and logging redaction for those errors.

`status()` returns a constant-size, copyable `AdmissionStatus`: `Pending` contains
attempts, the next not-before time and the pinned deadline; `Finished` contains
only attempts spent. It does not call the host, spend fuel, observe a clock or
passively expire work. A pending snapshot is advisory and can become stale; it
does not reserve capacity or grant replay authority. Expiry is observed by `poll`.
Terminal status deliberately does not retain successful output or host failure
text; the caller owns the original `AdmissionPoll` outcome.

`terminal_reason()` separately returns `Option<AdmissionEnd>` without changing
the existing status/poll JSON. Pending handles return `None`. Terminal reasons
are `Started`, `Rejected`, `Cancelled`, `Expired`, `Exhausted` and `HostUncertain`,
serialized as fixed snake-case tags without any payload. They are observations,
not deserializable handles, outcome replay or cancellation authority. `Started`
means the adapter reported acceptance, **not output delivery or execution
completion**. Callback unwinding records `HostUncertain`, not a false rejection
or permission to resubmit; the host must reconcile any unknown publication state.
Reading a reason does not call a host, observe time or perform cleanup.

`Debug` displays scheduling metadata only and never invokes the input's formatter;
inputs need no `Debug` implementation. Script text, authority and host-private
fields do not enter handle diagnostics. This does not sanitize arbitrary host
faults or payload-bearing start/result DTOs: hosts must still redact those before
logging. Terminal cleanup consumes input before running its destructor; cleanup
unwinding propagates but cannot rearm the admission handle.
The internal state is either pending input or a payload-free terminal reason.
Known acceptance/rejection is recorded before cleanup and remains sticky even if
cleanup unwinds. Successful output and direct errors remain locally owned until
input cleanup succeeds; otherwise unwinding releases them rather than stranding
an unreachable result in the return slot. This is not result-delivery assurance
or accepted-work rollback; output destructors retain their host-defined semantics.

There is no hidden queue, timer, worker pool, global lock or callback preemption.
Hosts own clocks, bounded aggregate ingress, wakeups, authority and accepted-work
cancellation. Opaque inputs can contain interior mutability: hosts must keep their
meaning and authority immutable across attempts. This crate is not a security
sandbox or an exactly-once submission protocol.

## Fuel Accounting

`Fuel` is a constant-size, host-granted meter: `new(remaining)`, `remaining()` and
`charge(cost)`. Successful charges debit exactly; an excessive charge returns the
allocation-free `FuelExhausted` and leaves the counter unchanged. Zero cost is a
no-op, even at zero remaining. Debit never wraps or panics on integer underflow.
There is no default grant, refill, `Clone`/`Copy`, or serde implementation. Counts
are observations, not authority; a trusted host can explicitly grant a new meter
or reconstruct one from a validated remaining counter. The primitive accepts the
full `u64` range; product limits and cost rules remain the adapter's responsibility.

The reference VM uses this meter for pure computation, copied locals/projections,
group-prefix reservation, sequential branch accounting and host-operation charges.
Result, successor and group re-entry reconstruct meters from already validated
remaining counters, not the reopened VM's default grant. Existing `LSV1001`
diagnostics, cost rules and numeric `fuel_remaining` wire fields are preserved;
zero-budget internal effect construction now returns a fault instead of underflow.
Bare parallel branch budgets retain their existing policy, not a newly shared
process-wide or batch-wide counter. Waiting/lease redelivery does not grant fuel.

Hosts/evaluators must charge before the work they intend to bound and handle an
exhaustion error explicitly. The meter cannot enforce that contract on native
adapters, preempt a callback, meter wall time/memory, authorize a continuation, or
make the existing evaluator host-neutral. It introduces no global lock or runtime
dependency. Core tests cover arithmetic boundaries and ownership; reference VM
recovery tests separately cover durable budget continuity and legacy wire bytes.

## Scheduler Clock Arithmetic

`checked_clock_add(now_ms, offset_ms)` performs allocation-free, exact integer
arithmetic within `0..=MAX_CLOCK_MS` (`i64::MAX`). It checks input range first,
then overflow/result range, returning `ClockError::OutOfRange` or `Overflow`.
Zero offset is valid, including at the maximum clock. The helper never wraps,
silently clamps, observes a real clock, creates a timer or advances runtime state.
Hosts own clock-domain identity, monotonicity, timeout policy and error-code mapping.

Execution deadlines, dispatch leases and semantic retry timestamps use this same
addition primitive without changing their legacy fault codes/messages. Admission
backoff explicitly clamps an overflowing future wakeup to its submission-pinned
deadline; this is caller policy, not hidden saturation in the arithmetic helper.
The reference VM validates claim clock/lease parameters before expiry cleanup or
journal entry. Invalid claims leave pending work, leased groups, attempts and
durable state unchanged, even when another SQLite writer holds the journal lock.
Valid claims still reap due deadlines before dispatch. This claim-specific rule
does not replace the existing replay/deadline precedence of completion APIs.

Read-only scheduler pressure and acknowledgement of an existing lease validate
the current timestamp, not a synthetic future lease. They accept `MAX_CLOCK_MS`;
issuing a new positive-duration lease at that clock still fails on overflow.
Pressure snapshots do not reap due execution deadlines. Existing completion
fencing remains unchanged: an acknowledgement at its stored lease expiration can
succeed only while its attempt is current and its execution deadline is not due.
This does not grant a new lease or extend a deadline.

## Retry Delay Arithmetic

`capped_exponential_delay(base_ms, max_ms, doublings)` computes exactly
`min(base_ms * 2^doublings, max_ms)` without allocation, wrapping or a loop over
the exponent. All `u64` durations and `u32` exponents are supported. Zero base or
cap yields zero; a cap below the base is respected even at exponent zero. A
positive base overflowing multiplication or shift yields the cap, not an error
or a reset to the first delay. This is delay arithmetic, not absolute-clock
arithmetic; adding the delay to a clock still uses `checked_clock_add`.

Admission and semantic-effect retry share this one primitive. Each adapter still
validates its own positive-delay bounds and translates its one-based attempt or
retry count to `doublings = count - 1`. Admission attempts, delivery attempts and
semantic retries are distinct budgets; only their arithmetic is shared. Existing
default trajectories, fault codes and wire fields are unchanged. A permanent
host rejection still does not retry, and zero semantic retries still terminates
on the first transient failure. Policy validation is not bypassed by this helper's
broader arithmetic domain.

There is no attempt counter, fuel grant, jitter, timer, sleep, callback or retry
classification in the helper. Waiting/retrying does not replenish saved fuel or
extend an execution deadline. Wide-integer oracle tests cover boundary arithmetic;
reference VM tests separately check exact delays, lease fencing and recovery with
a reopened worker whose default fuel is zero. Neither proves native callback
preemption or a full host-neutral evaluator.

## Reference Adapter

`leselang-vm::RootAdmission` uses this same implementation, captures HIR and
authority, and maps only its transactional `LSV2501` rejection to temporary
pressure. Existing `leselang_vm::Fault`, `AdmissionPolicy`, `AdmissionWait` and
`AdmissionPoll` paths remain available; `RootAdmission::status()` exposes the same
metadata-only status type, and `RootAdmission::terminal_reason()` uses the shared
`AdmissionEnd`. Repeated terminal observations do not enter the VM or journal,
including while another SQLite connection holds a writer lock. Continuation and journal bytes do not
change. Core tests use unrelated in-memory counter and text hosts; these prove
generic admission, **not** generic typed language-operation schemas.

## Standalone Package Check

From the current monorepo:

```sh
cargo +1.98.0 package -p leselang-runtime-core --allow-dirty --locked --offline
cargo +1.98.0 test --manifest-path target/package/leselang-runtime-core-*/Cargo.toml --locked --offline
```

Cargo normalizes inherited workspace metadata and verifies the packaged crate
in an isolated workspace. The second command tests that normalized package;
neither needs product sources. This is a local package check, not a registry
publication. A future independent source repository will supply its own manifest
metadata and CI rather than copying the inherited manifest without its workspace.
The package-path wildcard assumes a clean `target/package` containing one version;
select the exact version if old package directories are retained. The `leselang-core`
CI job checks the packaged build and tests separately from product quality gates.

## Remaining Extraction Work

Generic typed host-operation schemas, the evaluator/value/suspension engine,
optional journal backends and language-wide unrelated-host proofs remain to be
separated. The existing VM, UI and observation crates still use product command
and result types. FFI and the future operating-system shell are not implemented
by this crate. The embedding status target remains developing, not released.
