# Leselang Runtime Core

Host-neutral scalar, lifecycle, accounting and clock foundations for the future standalone
Leselang runtime. The current components are **bounded pre-admission scheduling**,
**typed scalar/projection/control decisions, lexical bookkeeping and fuel/clock/backoff
arithmetic**, not a parser, expression evaluator, complete embedded language,
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

## Language Values

`ScalarType`, `ScalarValue`, `OptionalStringValue`, `StringListValue` and their
text/list limits now live in this crate. They have no expression, host operation,
product DTO, authority, GUI or journal dependency. The six closed data types are
integer (`u64`), boolean, string, none, optional string and ordered string list.
Operation names contained in text remain data, not executable instructions.

The old `leselang_hir::computation` paths and `leselang_vm::ScalarValue` re-export
these exact types, not conversion wrappers. The tagged JSON, integer width,
absence versus present-empty text, ordered duplicate/empty list entries and all
existing continuation/projection/journal formats stay unchanged. Optional payload
must explicitly be null or text; missing tagged payload is not null. Optional
text is bounded to 4096 UTF-8 bytes, and list decoding incrementally enforces
64 entries and 4096 combined UTF-8 bytes without reserving from size hints.

Plain-string decoding retains its legacy decode-then-validate boundary, and
public Rust constructors/fields remain unchecked for compatibility. Embeddings
must call `is_bounded()` before using untrusted values; HIR/VM preflight still
applies the original value and receiving host-domain validators. Serialization
does not validate or truncate a directly constructed value. A surrounding serde
decoder may allocate before visitors run, so hosts must still bound input bytes.
Value `Debug`, serialization and serde errors may contain private data; they are
not the redacted admission-handle observation surface. Host logging owns redaction.

Source constructor node/depth costs remain HIR-owned, including helper expansion
before constant folding. Lexical value policies, fuel charging, expression
validation and host-result capture/domain checks remain adapter-owned; temporary lexical
storage/cleanup delegates to `ScopeFrame`. Shared
data is not a host schema, suspension engine or independent evaluator. Standalone
tests prove wire compatibility, UTF-8/aggregate limits, hostile sequence hints and
ownership/query semantics; reference tests separately prove legacy imports and
unchanged evaluation/expansion boundaries.

## Scalar Operations

`BinaryOperator` and `UnaryOperator` now share their original closed names/tags
through this crate and the legacy HIR re-exports. `parse()` accepts only exact
builtin names; `result_type()` supplies the same scalar signatures used by HIR
preflight and execution. These 23 binary and seven unary operations are language
data operations, not a host-operation catalog or arbitrary named dispatch.

`apply_unary()` and `apply_binary()` consume **already evaluated operands**.
They reject wrong types and unbounded direct/decoded values before scanning or
materializing results. Successful results retain 4096 UTF-8 bytes and 64 list
entries with the aggregate list-byte bound. Arithmetic is checked `u64`; parsing
is strict ASCII decimal or exact `true`/`false`, and indexing addresses Unicode
scalars or ordered list entries, returning optional text. Empty/duplicate list
entries and absence versus present-empty text retain their original semantics.
Owned text pass-throughs move buffers instead of cloning them.

`ScalarError` is a closed, payload-free error with fixed display messages, not
an execution receipt or recovery grant. The reference adapter retains the exact
`LSV1401`/`LSV1402`/`LSV1403`/`LSV1408` codes and messages. Arithmetic/parsing may
be caught by its existing `recover`; type, bounds, fuel and host failures may not.

**Short-circuiting and fuel charging remain evaluator-owned**. This eager API
validates both supplied operands, even for `and`, `or` or `value_or`; it cannot
undo evaluation of an already produced right value. The VM still skips right
expressions on the original lazy paths and preserves exact input/output charges,
remaining-fuel re-entry and wire schemas. No callbacks, clocks, global state,
authority or persistence are introduced. Hosts must still bound ingress bytes,
charge aggregate work and handle allocator failure; this is not a sandbox or a
complete evaluator. Standalone type-matrix/boundary tests and reference parity,
wire, diagnostic and exact fuel-threshold tests cover these separate contracts.

## Scalar Control Decisions

`select_binary_left()` consumes an evaluated left value and returns either
`BinarySelection::Complete` or `NeedsRight`. Only `and(false, ...)`, `or(true, ...)`
and a present optional string complete immediately. Present-empty text is still
present; bounded optional text moves directly into the result without the VM's
former second string copy. Complete results are bounded. Non-lazy operators pass
the original value through without duplicate scans or conversion wrappers.

**Deferred values are not validation certificates**. `NeedsRight` does not
certify either operand's complete signature or bounds; the adapter must evaluate
and charge the right expression and call `apply_binary()`. Whole-expression
type/purity/capability checks still include cold paths before any evaluation.
The eager operation API continues to validate both supplied operands. Neither
selection nor an enum tag calls a host or evaluates a callback. Selection contains
actual data, so its `Debug` output is not a redacted metadata surface.

`LoopBudget` shares the original 0-1024 scalar-loop bound with HIR through
`MAX_LOOP_ITERATIONS`. It retains only the initial scalar type, limit, completed
count and condition/advance phase, never the state payload. The adapter validates
and owns initial state, then evaluates/charges the condition before calling
`check_condition()`. False completes even at the limit; true at the limit returns
`LoopError::IterationLimit` and cannot rearm. A permitted next expression must be
evaluated/charged and passed to `advance()` before replacing scoped state.
Only a bounded, same-type next value advances the count; type/order/bounds errors
leave it unchanged, not a refund or permission to replay failed work.

Budgets are allocation-free, move-only, non-default and non-serde. They have no
fuel grant, timer, global lock, host handle, callback, persistent frame or
mid-loop suspension. A trusted host can construct a new budget explicitly, so
this counter is not execution authority. The reference VM uses the shared
decisions but still owns expressions, lexical state, fuel, cleanup and recovery;
loop exhaustion remains the non-recoverable `LSV1406` with its original message.
Fold-body, conditional/recovery evaluation and all host-result frames stay in the
reference interpreter. This is not a second interpreter or a complete independent
evaluator. Core boundary/ownership tests and reference cold-preflight, exact-fuel,
scope-cleanup and durable iteration tests cover that division.

## Bounded Fold Traversal

`FoldCursor` owns one evaluated `StringListValue` and the initial accumulator's
type, not its payload or expression. It validates the 0-64 limit, then the
64-entry/4096-byte collection, then its complete count against the limit before
yielding anything. There is no truncation. Construction consumes the list on
success or failure; error messages contain no submitted text.

`next_item()` moves each original string buffer in source order, including empty,
duplicate and Unicode text. Before another item can be obtained, `advance()` must
accept one bounded next accumulator of the original scalar type. Only successful
advances increment the counter. Invalid ordering/type/bounds do not consume any
additional item or change progress, and do not refund fuel or authorize failed
work to retry. The final `None` closes the cursor; it cannot rearm. Unlike an
unrestricted Rust iterator, items cannot be drained without state validation.

The cursor is move-only, non-default and non-serde, with metadata-only `Debug`.
It reuses the owned vector/strings without allocating traversal buffers, keeps
no accumulator values, and adds no callback, global lock, fuel grant or execution
authority. A trusted adapter may create another cursor explicitly. It must
validate the initial state, preflight cold expressions, charge work, evaluate
the fold body and clean up both local slots on error. The reference VM keeps
collection-before-initial evaluation, limit-before-scan-charge/next failures,
exact fuel costs, non-recoverable `LSV1406` and existing durable replay. Fold
bodies still cannot suspend or execute host effects. No source or wire changes;
this primitive is not a full evaluator or generic container/host-operation API.

## Bounded List Construction

`StringListBuilder` shares incremental 64-entry/4096-byte construction across the
reference VM's computed `strings`, eager `split`/`append`, and list decoding.
`try_push(String)` moves the owned buffer; `try_push_text(&str)` checks count and
remaining UTF-8 bytes before allocating a copy. Rejected pushes consume/drop
owned inputs but leave the accepted prefix/count/bytes unchanged. Callers must
handle errors explicitly, not silently publish a partial prefix. The reference
interpreter and eager operations stop on the first fault without partial output.

`new()` and `Default` start empty without allocation; they build data, not fuel
or execution grants. `with_capacity()` accepts only 0-64 entries before reserving,
preserving the VM's bounded constructor preallocation. `TryFrom<StringListValue>`
validates and adopts an existing vector/string buffers without cloning or
changing capacity. Logical length/byte limits do not cap inherited capacity,
upstream allocations, total host work or allocator failures. `finish()` consumes
the builder and produces the legacy publicly mutable list, not a lasting bounds
certificate; later mutated or decoded ingress must still validate boundedness.
Builder `Debug` exposes counts only, never entries.

The decoder still ignores untrusted sequence hints and validates each item's
type/bytes before the extra-entry count check, with byte/count messages unchanged.
VM entries still evaluate and charge in source order; computed byte overflow
precedes later expression failures, final materialization fuel stays exact, and
`LSV1403` remains outside calculation recovery. HIR still owns constant folding,
labels, source node/depth accounting and cold type/purity/capability checks. No
syntax, value/result/projection/continuation/journal bytes or schema changes.
The builder evaluates nothing and calls no host; this is not a full evaluator.

## Borrowed Lexical Frames

`ScopeFrame<Value>` shares temporary lexical storage and cleanup without depending
on expression or host-result types. Keys borrow immutable names; insertion does
not copy names or values. Reads see the complete visible prefix; `push` rejects
active duplicates with the payload-free `ScopeError::DuplicateBinding`.
`get_local_mut` uses checked frame-relative slots and `pop` moves only local
bindings, so neither directly replaces nor removes parent bindings. Nested
frames reborrow the stack and restore their own boundary on normal/error exit.

Dropping a frame detaches its entire suffix before any value destructor runs,
normally releasing values in reverse insertion order. Destructor panics propagate
under Rust unwinding rules; another cleanup panic can abort. This move-only,
must-use guard has no default or serde representation. Its `Debug` formats counts
without invoking name/value formatters; `bindings()` exposes real data and must
not be treated as a redacted log. Thread handoff is conditional on value ownership,
and GUI-local non-Send values need no extra trait bound or global lock.

Existing prefix uniqueness, name grammar, type/authority checks and aggregate
size remain adapter responsibilities. Slots can be stale/reused after a pop, and
interior-mutability/destructor behavior belongs to the value type. Forgetting the
guard bypasses cleanup: this is not an authority fence, sandbox, saved frame or
callback preemption. Push/lookup scan the visible prefix; bounded preflight and
evaluation/fuel policies still belong to the adapter.

The reference VM delegates `bind`, `loop`, `fold` and result re-entry lexical
bookkeeping here, avoiding transient local-name copies. Durable suspension still
copies names/data into owned snapshots under the original fuel charges. Tests
cover nested failure/recovery, parent preservation, buffer ownership and unwind,
as well as exact existing fuel and continuation bytes. No syntax, host schema,
projection, journal or execution-authority changes; this is not a full evaluator.

HIR lowering, helper parameters and residual scope revalidation also borrow names
through this frame; owned HIR/canonical output still copies names where required.
Cold branches retain name/type/purity/capability and node/depth checks. Failed
lowering restores lexical state, not visited-node budgets or hygienic-name grants.
The VM's restored-projection preflight uses the same guard without widening legacy
field masks: scalar names occupy slots but grant no result fields. Result/group
version selection, mask intersections, canonical validation and authority stay adapter-owned.

## Ordered Scalar Projections

`ScalarProjectionField<Field>` holds an opaque developer-defined key and one
shared scalar. `validate_scalar_projection()` borrows the fields and a trusted
ordered `(key, ScalarType)` schema, enforcing exact count/order/keys, all six
scalar types and each value's language bounds. Keys need only `PartialEq`, not
`Clone`, `Debug`, serde or `Send`; thread-local native keys are supported. No
keys/values are copied, normalized, reordered, truncated or synthesized.

The validator never collects/counts a second schema or reads size hints. It
makes at most `fields.len() + 1` calls to the supplied iterator, so a non-cloneable,
non-exact-sized schema is usable without a second pass. Comparisons and iterator
callbacks are trusted native code; they can perform unbounded work or panic.
Unwinding propagates; the validator itself does not mutate fields or roll back
native callback/interior-mutability effects. This is not callback preemption or
an overall CPU/ingress limit. Hosts select operation/version schemas,
define distinct keys, bound field counts/input bytes and charge aggregate work.

`ProjectionError` reports fixed payload-free count/key/type/bounds errors, in
left-to-right check order. The reference VM maps every error to the unchanged
`LSV1405`, then checks operation-specific kind tokens and optional-text domains.
Its public `ProjectedField` is a direct specialization, not a conversion DTO.
Closed projection v1-v5 JSON, v1 omission, frame masks, authority, raw receipt
matching, saved fuel, continuation/journal versions and replay stay unchanged.

The field DTO remains ordinary mutable data. Direct construction, serialization
and legacy plain-string decoding do not validate the schema; optional/list scalar
decoding keeps its original bounds. Successful validation is not a lasting bounds
certificate, authentic host receipt or permission to execute. DTO `Debug` and
serde may expose keys/values; only generated projection errors are payload-free.
Tests prove two unrelated projection schemas, boundary/ownership behavior and
reference parity, **not** two independent host evaluators or a complete generic
operation/suspension engine. The package still has only serde as a normal dependency.

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
