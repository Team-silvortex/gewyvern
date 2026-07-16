# Leserpent 1.0 To 2.0 Roadmap

This is the execution roadmap for the
[Leserpent 2.0 architecture](leserpent-2-architecture.md). The architecture
defines the invariant destination; this page defines ordered delivery gates.

The roadmap is capability-gated, not date-gated. A later gate may be prototyped
early, but it cannot become authoritative before its prerequisites are green.

## Baseline: 1.x Bridge

The current ASP.NET, TypeScript, SQLite, Orchestra, deployment, security, and
fleet behavior remains supported while Rust replacements are introduced.

Before migration:

- preserve the 1.x API and security tests
- capture command, authorization, persistence, and projection fixtures
- classify browser-only preferences separately from domain state
- prohibit new business logic in frontend event handlers
- record representative performance and deployment baselines

Exit: the current behavior is reproducible enough to compare with Rust.

## Gate 1: Domain And Protocol Kernel

Build `leserpent-domain` and `leserpent-protocol`.

- define IDs, revisions, principals, capabilities, commands, queries, events
- define `CommandEnvelope`, dry-run plans, confirmation, and result schemas
- establish canonical JSON fixtures and compatibility policy
- implement the first Rust projection and in-memory runtime
- adapt one read-only fleet route and one idempotent mutation end to end

Exit: GUI/API and Rust fixtures agree on normalized semantics.

`CommandPlan` and `PlannedOperation` now live in `leserpent-domain`, including
schema, operation/capability consistency, and embedded capability validation.
This keeps plan ownership below Leselang, CLI, GUI, protocol, and runtime
adapters.

The current compatibility policy and fixture boundary are maintained in
[`crates/leserpent-protocol/COMPATIBILITY.md`](../crates/leserpent-protocol/COMPATIBILITY.md).

## Gate 2: Leselang Frontend And VM

Build the language without UI-specific shortcuts.

The implemented language surface and its explicit limits are maintained in the
[Leselang language contract](leselang-language.md). That contract, rather than
future examples in this roadmap, defines what callers may use today.

- lossless parser and first-class diagnostics
- HIR, lightweight static types, effects, and capability checking
- stackless evaluator with `Done / Effect / Yield / Cancelled / Failed / Fault`
- bounded execution, cancellation, timeout, retry, and deterministic merge
- versioned continuation serialization
- effect journal and exactly-once continuation consumption
- formatter, explain output, and model-oriented language guide

Current evidence covers the `runtime.list` vertical slice, including a bounded
SQLite journal, transactional sequence allocation, pending recovery, and
durable first-completion replay. The dispatch outbox now adds attempt-fenced
leases, crash redelivery, multi-VM exclusion, and transactional acknowledgement.
The first mutating effect, `runtime.refresh`, now carries one stable domain
idempotency key through crash redelivery and proves single domain commit with
result replay. Typed requested cancellation and trusted wall-clock deadlines now
atomically fence ready or leased dispatches and survive restart as replayable
terminal states. Bounded semantic retry now persists deterministic not-before
clocks independently from transport attempts and preserves command idempotency
across restart. Count-based journal retention now transactionally compacts only
the oldest terminal records in bounded batches, cascades acknowledged outbox
rows, and uses secure deletion without disturbing pending or leased work.
The deterministic merge kernel now normalizes out-of-order branch completions,
selects competing terminal outcomes in declaration order, bounds recursive
structured values, and persists merged output through the existing journal.
Source-level `all` now preserves named branch order in the lossless parser,
lowers branch-local types, and authorizes the union of branch capabilities. The
schema-5 journal now reserves strict merge-group and ordered branch records,
migrates legacy schemas, and rejects malformed recovery graphs. A shared
ephemeral/SQLite primitive now validates and atomically writes the merge plan,
every branch continuation and dispatch, and all declared-order links. Injected
mid-graph failure rolls the entire SQLite transaction back; committed graphs
recover every branch after restart. Branch terminal writes now finalize the
group in the same transaction when the last branch completes, persist the
declared-order merge result, and roll the branch write back if group completion
fails. Success and permanent-failure outcomes survive restart. Retention now
treats a completed merge graph as one logical unit: ephemeral and SQLite
compaction remove its group, links, branch effects, and dispatches together,
while pending or partial graphs remain untouchable. The batch limit counts
logical units, and each graph remains physically bounded to 64 branches. The VM
now starts one-level `all` as an ordered atomic effect batch with a stable merge
token. Non-final branch completion exposes durable waiting progress, the final
branch returns the declared-order aggregate, and SQLite restart resumes only the
remaining branches. Nested `all` remains deliberately rejected before sequence
allocation; the one-level structured-execution VM contract has exited Gate 2.

The `leselang-command` boundary now lowers authorized `runtime.list` and
`runtime.refresh` HIR effects into frontend-neutral `CommandPlan` values. The VM
consumes that crate rather than constructing domain envelopes itself, and tests
prove that CLI and Leselang origins preserve identical command semantics apart
from audit origin metadata. The CLI now uses that shared lowering function for
real refresh requests and can export deterministic, validated, versioned plans
locally without daemon credentials. Broader command coverage remains before
Gate 2 exits.

Exit: programs can suspend, restart, re-enter, and replay deterministically.

## Gate 3: Native CLI Parity

Build the Rust `leserpent` CLI on the shared protocol.

- query, inspect, plan, apply, watch, export, and history commands
- stable JSON mode and human-readable mode
- canonical Leselang export for every mutation
- dry-run and confirmation UX
- local IPC and authenticated remote transport

The first native slice now exists in `crates/leserpent-cli` and installs the
`leserpent` binary. It connects only through authenticated wire-v1 local IPC,
reads its token only from `LESERPENT_IPC_TOKEN`, validates owner-private socket
metadata before sending credentials, and supports `health` plus filtered
`runtime list`. Human output is control-character-safe; `--json` preserves the
versioned response envelope for automation. A real binary-to-daemon test covers
both modes. `runtime refresh` now requires explicit `--yes` for mutation, supports
non-mutating `--dry-run`, optimistic revision checks, stable caller-provided
idempotency keys, and local-only canonical Leselang export. A parity test parses,
lowers, and compares the exported command. Local `--export-plan` now emits the
same normalized refresh plan used for execution and requires an explicit stable
idempotency key. A real authenticated IPC and CLI/Leselang parity fixture now
covers the dedicated `runtime inspect` query. Runtime list and inspect now also
export canonical Leselang and validated plans locally through the same lowering
used for IPC execution. Bounded `runtime history` now returns newest-first domain
results through authenticated IPC, VM re-entry, canonical export, and a parity
fixture without exposing journal rows. The native CLI now adds a bounded,
revision-deduplicated watch loop over the same normalized inspect plan, with
line-flushed human and JSON output. Local Gate 3 operation breadth is complete;
authenticated remote CLI transport remains under the Gate 6 transport boundary.

Exit: every migrated operation is executable through CLI and Leselang with the
same parity fixtures.

## Gate 4: UI IR And Avalonia Vertical Slice

Build the first replaceable GUI path.

- pure `State -> UiDocument` evaluation
- typed events and `UiEvent -> CommandPlan`
- stable node identity and incremental `UiPatch`
- Avalonia renderer with compiled bindings and virtualization
- fleet list, runtime child workspace, logs, and one debugger workflow
- GUI action inspection, dry-run, Leselang export, and audit correlation

The first renderer-neutral slice now exists in `crates/leselang-ui`. It lowers
the typed fleet projection into a bounded `UiDocument`, resolves revision-fenced
typed events through the shared `CommandPlan` path, and computes deterministic
remove/insert/move/update patches over stable node IDs. Validation rejects
duplicate IDs, oversized or over-depth trees, unlabelled actions, stale events,
and actions rebound to another runtime. No endpoint, renderer, persistence,
transport, HTML, script, or adapter type enters the IR. Avalonia rendering and
broader child-workspace/debugger documents remain. A framework-independent
patch application reference now fences revisions and rejects malformed graph
edits; round-trip fixtures establish the semantic renderer conformance baseline.
The runtime child workspace now combines same-revision inspect and bounded
history projections into status, snapshot, refresh, and history nodes. Torn
state fails closed, endpoint data remains outside the IR, and history changes
apply incrementally. A separately bounded log projection now lowers sanitized
typed entries without adapter or endpoint fields and applies sliding windows
incrementally. The `leselang-observe` producer now validates bounded source
batches, preserves the newest sequence window, and sanitizes display text before
UI lowering without admitting endpoint or transport fields. SQLite schema 8 now
persists a 4096-record window per runtime behind an indexed sequence cursor.
Initial reads return the newest bounded window; incremental reads return only
records after the supplied cursor. The typed query requires `runtime.read` and
round-trips through authenticated Unix IPC without endpoint disclosure. The
first read-only
debugger document now models stackless synchronous
effect waiting and re-entry with bounded logical frames, sanitized summaries,
and no continuation token or local-value exposure. The `leselang-observe`
composition boundary now converts an authoritatively validated suspended VM
effect into that projection, rejects torn revisions, and proves that
continuation tokens, principals, capabilities, idempotency keys, and absolute
scheduler deadlines never reach serialized UI state. `DebuggerCancel` now uses
the shared command plan with `debugger.control`, revision/session fencing,
explicit confirmation, safe inspection, and a non-mutating dry-run. Confirmed
execution reaches the VM's durable cancellation path without returning the
continuation token. VM journal schema 6 atomically persists the requested
cancellation and its command/session/revision audit, rejects conflicting
principal-scoped idempotency reuse, and replays the original audit after a
restart. The public record omits continuation tokens and idempotency keys, and
is pruned with its retained continuation. The waiting debugger document now
declares a session-bound cancel action that lowers through the same shared
command planner. Rust and .NET reject session rebinding, and Avalonia renders a
destructive button while emitting only its stable node ID.

The first concrete cross-language renderer core now exists under
`apps/leserpent-avalonia`. Rust emits a bounded versioned JSON fixture and the
.NET 10 adapter strictly deserializes it, mounts the previous semantic tree,
applies incremental operations, mirrors runtime/action validation, and proves
equality with the Rust next document at zero build warnings. The first Avalonia
12 desktop shell now maps every current semantic node kind to real controls,
preserves stable IDs and accessibility metadata through Automation properties,
and emits only action node IDs back across the renderer boundary. A platform
smoke mode initializes the real control stack, renders the Rust fixture, and
exits cleanly. The mounted control tree now consumes remove, insert, move, and
update operations through a stable-ID visual index after a transactional
semantic candidate validates the complete patch. The compound fixture proves
unchanged and moved controls retain identity instead of rebuilding the tree.
Fleet roots now use an active `VirtualizingStackPanel` as the window viewport,
and history sections use independent bounded viewports rather than nesting
under an unbounded outer scroller. This establishes layout virtualization;
compiled-bound item view models now defer leaf-control creation until viewport
realization. A long bounded-history fixture retains unrealized items after the
window opens, proving off-screen controls are not constructed. Heterogeneous
container subtrees now remain as patchable renderer models until their parent
enters the viewport, so runtime cards no longer force eager descendant control
creation. The desktop shell now publishes through a checked NativeAOT profile
with a single pinned runtime/compiler/linker patch set. macOS arm64 and a
physical Ubuntu x86_64 host both produce five-file self-contained native
packages and pass all real control fixtures without a managed runtime
installation. The Linux debugger fixture preserves the one-to-zero
cancel-control lifecycle under Xvfb. Windows native-host evidence and mobile
shells remain.

The Avalonia renderer now maps bounded log entries to lazy monospace controls.
The 48-entry cross-language sliding fixture applies in three operations and
leaves 26 off-screen controls unconstructed after first layout.

The same renderer maps debugger logical frames lazily. A 40-frame fixture moves
from `WaitingEffect` to `Yielded` in seven operations, removes the cancel
control, and leaves 18 frames unconstructed after first layout. Control smoke
evidence records one realized debugger-cancel button before the patch and zero
after re-entry.

The authoritative Rust diff now updates a working document as it emits each
operation and refuses to return unless that document converges exactly on the
target. This fixes invalid move indexes previously exposed by a sliding bounded
history window and reduces that transition from 34 operations to 3.

Exit: the vertical slice contains no direct adapter or persistence access and
passes GUI/CLI/Leselang equivalence tests.

## Gate 5: Durable Runtime Cutover

Move authority from the compatibility bridge into `leserpentd`.

The initial `leserpent-runtime` slice validates and executes shared
`CommandPlan` values and now persists runtime registration plus mutating plans
in one ordered SQLite journal. Restart replay rebuilds projections, completes
pending commands, seals terminal command failures, and rejects divergent stored
outcomes. Journal records and payloads are bounded; the database is private and
opened without following links. The journal now transactionally migrates v1 to
v2, records migration history, validates the claimed schema shape, and preserves
legacy replay order. Schema v3 added validated domain snapshots that preserve
projection revisions and idempotency results; startup restores the snapshot and
replays only its incremental journal suffix. Snapshot metadata and payload share
one integrity check. Schema v4 retains two generations, falls back from a
corrupted newest generation, preserves the journal suffix needed by the prior
generation, and deletes at most 1000 covered records per checkpoint. Schema v5
adds a conditional SQLite owner lease: a second live runtime is rejected,
expired ownership can be replaced atomically, and both stale reads and stale
writes are fenced on their next operation. Schema v6 adds a bounded persistent
effect queue with idempotent enqueue, atomic claim, lease
renewal, attempt-token fencing, crash redelivery, retry delay, terminal
completion/failure, and max-attempt sealing. Daemon heartbeat, typed adapter
worker loops, and status projection integration are implemented; queue
backpressure visibility and terminal task retention are now implemented, while
bounded worker concurrency remains the gate boundary. The runtime now exposes
an explicit owner heartbeat plus a
bounded synchronous worker step. Executors return `Complete`, `Retry`, or
`Reject`; the runtime alone commits the fenced lease transition, while callers
retain control over concurrency, cancellation, and shutdown.

Schema v7 freezes the durable runtime semantics. It records the introduction of
typed status-observation journal entries even though no physical table change is
needed, so a v6 binary rejects a newer database by version instead of attempting
an incompatible replay. Startup requires migration history to be exactly 1
through 7, verifies the effect-table columns and claim index, and rejects unknown
journal kinds before rebuilding projections. Complete v6 databases migrate
transactionally to v7.

Schema v8 adds the dedicated `runtime_logs` table and its
`(runtime_id, sequence)` index. Appending and per-runtime retention occur in one
owner-fenced transaction, retaining at most 4096 records per instance. The
versioned domain query limits each response to 256 records, supports monotonic
cursor continuation, and omits runtime endpoints. Migration history must now be
exactly 1 through 8; malformed pre-existing tables and indexes fail closed.

Applied refresh events now materialize their typed status effect before the
command returns. Startup reconstructs a missing effect from durable applied
command results, closing the crash window between command-journal completion and
queue insertion without duplicating an existing idempotent task.

Status-refresh completion is also durable end to end. A typed observation
carries its runtime ID and expected projection revision; the runtime validates
it on a cloned projection, then appends the replay record and completes the
leased effect in one SQLite transaction before publishing the new in-memory
state. Restart replay verifies the recorded projection outcome. Stale revisions
and malformed adapter outcomes fail the task closed without stopping the worker.

The effect queue reports exact `ready`, `leased`, `completed`, and `failed`
counts together with active capacity and saturation. Authenticated wire-v1
health includes this data through an optional field, so older response payloads
remain decodable. Terminal retention is explicit and bounded: daemon maintenance
runs every 256 heartbeats by default, retains the newest 8192 terminal tasks,
and removes at most 100 per pass without touching ready or leased work. This
also defines the persisted effect-ID idempotency horizon; producers must use
globally unique effect IDs rather than intentionally recycling compacted IDs.

Bounded synchronous concurrency is now explicit. `leserpentd` claims at most
four effect kinds per batch by default (configurable from one through 32), runs
different adapter kinds on scoped native threads, and settles outcomes back on
the single authority thread in claim order. A batch never leases two tasks for
the same adapter kind, so mutex wait time cannot consume a second task's lease.
Adapters receive a cooperative cancellation context; cancellation schedules a
retry, while a panic is contained and rejects only its claimed task. The main
signal loop passes its stop flag into batch execution.

The repeatable Linux stress harness is
`scripts/validation/leserpentd_linux_stress.sh`; it runs natively on the trusted
remote shelf and retains JSON under
`target/validation/leserpentd-linux-stress/`. The 2026-07-15 x86_64 run on Linux
6.17.0-35 proved eight-way observed parallelism with all 256 effects completed,
cooperative cancellation returning a lease to `ready` in 25 ms, strict rejection
of task 10,001 at the 10,000-active capacity, and real process-exit recovery as
attempt two after the 30-second owner lease elapsed. The saturation phase took
28.2 seconds with one transaction per task and `synchronous=FULL`.

The runtime now provides atomic enqueue batches of one through 1000 tasks.
Validation, duplicate-ID detection, existing-record idempotency checks, and the
active-capacity decision all complete before insertion; any conflict or overflow
rolls back the whole batch. A full queue still accepts exact idempotent replays.
On the same Linux host, filling all 10,000 active slots as 100 batches of 100
took 584 ms, a 48.3x improvement over 28,185 ms, without changing
`synchronous=FULL` durability.

With the schema-v7 compatibility fence, atomic batch API, Linux stress evidence,
and negative migration tests in place, the reusable control-runtime contract is
frozen as `1.0.0`. This freezes the runtime cell, not the wider Leserpent 2.0
architecture; daemon platform parity, adapters, renderers, and remote control
continue on their own status-tensor tracks.

The initial `leserpentd` crate now hosts `ControlRuntime` as a standalone
process. It drives explicit owner heartbeats and bounded worker steps through a
typed adapter registry, supports finite-step smoke execution, and stays in a
heartbeat-only safe mode when no adapter is installed. Graceful Unix signal
shutdown, authenticated local health, and bounded synchronous worker concurrency
are implemented; Windows IPC parity and broader production adapters remain
before the daemon can become authoritative.

On Unix, `leserpentd` now exposes the existing wire-v1 request and response
envelopes over a private `0600` Unix socket. Each bounded line frame carries an
IPC token sourced only from `LESERPENT_IPC_TOKEN`; token comparison is constant
time, malformed or oversized frames fail closed, and internal storage failures
are not reflected to clients. Windows named-pipe parity remains before local IPC
is cross-platform complete.

Wire v1 now includes an authenticated health request whose response is emitted
only after the runtime renews its owner lease, so `ready` proves current local
authority rather than process liveness alone. The daemon maps SIGINT and SIGTERM
to an atomic stop flag; normal stack unwinding removes the socket inode and
releases the SQLite owner row. A real process smoke verifies exit code zero and
lease release after SIGINT.

The first production integration now lives in the independent
`leserpent-adapters` crate rather than the daemon host. Its typed registry
rejects unknown effect kinds, and the initial Gewyvern health adapter accepts
only configured loopback targets, keeps admin tokens out of scheduler payloads,
bounds HTTP responses, and validates the `/health` JSON contract. A vertical
test proves SQLite enqueue through daemon claim and adapter execution to fenced
completion. The typed status adapter now combines `/health` and
`/v1/latest/meta` into a frontend-neutral snapshot without journaling its admin
token. `leserpentd` registers both health and status effect kinds for configured
targets, and status outcomes flow through the runtime's atomic projection and
replay path. Deployment, discovery, remote TLS, and secret storage remain future
adapter slices.

- SQLite journal, projections, snapshots, and migrations
- effect workers with backpressure and recovery
- secret-storage adapters
- local daemon lifecycle and process isolation
- compatibility API for the existing TypeScript dashboard
- import of supported 1.x state with rollback evidence

Exit: Rust is authoritative and the old service is an adapter, not the owner.

## Gate 6: Mobile And Remote Console

Extend the same contracts rather than forking product behavior.

- Android and iOS Avalonia entry projects
- mobile navigation and adaptive presentation
- authenticated HTTPS/WebSocket protocol
- reconnect, offline read cache, and explicit stale-state presentation
- push/deep-link integration through platform adapters
- optional embedded Rust feasibility study for offline execution

The current desktop slice implements the read-only half of this gate. A pure
`Leserpent.RemoteClient` library owns strict event decoding, explicit CA and
hostname verification, endpoint-bound atomic snapshot cache, stale-state
transitions, cursor reset, and an eight-attempt capped reconnect loop. The
Avalonia shell projects snapshots through the same neutral `UiDocument`
renderer used by fixtures. A separate conformance executable proves codec,
cache, resync, and retry behavior; a real Rust daemon to .NET client TLS
vertical path has also been exercised. Remote mutation, mobile lifecycle, and
platform-specific secure token storage remain before Gate 6 completion.

Exit: desktop and one mobile target pass the same semantic conformance suite;
platform-only presentation differences are documented.

## Gate 7: 2.0 Seal

- remove frontend-exclusive business logic
- freeze version-1 command/query/effect/UI schemas
- complete security, fuzz, failure-injection, and migration audits
- publish performance comparisons against the 1.x baseline
- document packaging, rollback, recovery, and protocol compatibility
- keep the TypeScript client only if it remains a conforming renderer

Exit: every criterion in the architecture's
[2.0 definition](leserpent-2-architecture.md#20-definition) is evidenced.

## Continuous Proof Shelves

Every gate maintains:

- parser and VM fuzzing
- golden syntax/HIR/IR diagnostics
- command-origin parity tests
- authorization and confirmation tests
- idempotency and revision-conflict tests
- crash/restart/re-entry tests
- journal migration and replay tests
- UI IR snapshot and accessibility tests
- IPC/HTTP/WebSocket compatibility tests
- desktop/mobile AOT smoke tests
- latency, memory, throughput, and package-size benchmarks

The parser/VM fuzz shelf now has a named native entrypoint:
`gewyvern_validate leselang-fuzz`. A fixed replayable seed covers 2048 arbitrary
UTF-8 source cases and 2048 continuation mutations. The shelf checks lossless
reconstruction, character-safe spans, deterministic syntax JSON, HIR lowering,
bounded VM startup, fail-closed continuation decoding, and canonical image
roundtrip while retaining its configuration and transcript. Its first run found
and fixed an escape-followed-by-multibyte-character lexer panic.

The UI accessibility shelf now has the native entrypoint
`gewyvern_validate leserpent-accessibility`. It audits real Avalonia controls
across all four fixtures for stable unique Automation IDs, exact names and help
text, explicit action labels, and a 4.5 WCAG AA text-contrast floor. Managed
macOS and physical Linux/Xvfb proofs produce matching counts, while the macOS
NativeAOT job requires the same accessibility marker and metrics. The first run
raised destructive-button contrast from 3.841 to 4.723.

The transport shelf now has the native entrypoint
`gewyvern_validate leserpent-transport`. It retains separate transcripts for
canonical wire-v1 decoding, legacy-v1 adaptation, CLI/Leselang semantic parity,
the real authenticated native CLI-to-daemon Unix socket path, and daemon IPC
security rejection paths. Gate 6 adds a server suite for the default-off
authenticated HTTPS `POST /v1/wire` server: a real TLS roundtrip, constant-time
bearer authentication, strict bounded HTTP framing, private-key file safety,
and shared wire-v1 dispatch. A seventh suite drives the native CLI through that
endpoint with explicit CA trust and proves health, query, bounded watch,
confirmed command/idempotency, and auth-error exit semantics. An eighth suite
proves strict WebSocket authentication, required subprotocol negotiation, and
cursor parsing; the TLS server suite also proves revisioned endpoint-redacted
snapshots and explicit future-cursor resynchronization. Its summary still
excludes Windows named pipes, remote GUI, and mobile clients. macOS arm64 and a
physical Linux x86_64 host pass the same eight suites, 28 tests, and 41 declared
invariants.

The performance shelf now has the native entrypoint
`gewyvern_validate leserpent-benchmark`. It measures fixed-size SQLite
cold start, 256-runtime query latency, 10,000-effect batch throughput, a
1,027-node UI document's generation/diff/codec costs, and release CLI/daemon
size. Broad fail-closed budgets guard against order-of-magnitude regressions;
raw macOS arm64 and physical Linux x86_64 results remain separate host-class
baselines with machine-readable evidence.

The current command-origin and recovery shelf now has the native entrypoint
`gewyvern_validate leserpent-parity-recovery`. Eight non-vacuous suites
execute at least 129 tests across neutral command lowering, domain
authorization/idempotency, debugger confirmation, CLI/Leselang equivalence,
VM continuation/journal re-entry, runtime SQLite recovery injection, and the
authenticated remote wire boundary, and native remote CLI parity. The
shelf retains per-suite transcripts and rejects any run whose filtered test
count falls below its declared minimum. This proves the currently migrated
command surface and shared local/remote CLI dispatch; WebSocket event delivery
is covered by the transport shelf, while remote GUI and future mobile operations
still require their own parity fixtures. macOS arm64
and a physical Linux x86_64 host both report the same eight suites, 129 tests,
and 46 declared invariants.

The HTTPS listener is intentionally opt-in:

```bash
chmod 600 /etc/leserpent/tls/server.key
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpentd --database /var/lib/leserpent/runtime.sqlite \
  --remote-listen 0.0.0.0:9443 \
  --remote-cert /etc/leserpent/tls/server.crt \
  --remote-key /etc/leserpent/tls/server.key
```

The three remote arguments must be supplied together. The certificate and key
must be regular files rather than symlinks, and the private key must grant no
group or other access on Unix. Use a CA-issued certificate and deployment
network policy; the bearer token is never passed on the command line.

The native CLI uses the same endpoint without changing command syntax:

```bash
export LESERPENT_REMOTE='https://control.example.internal:9443'
export LESERPENT_REMOTE_CA='/etc/leserpent/tls/ca.pem'
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpent --json runtime list
```

CLI endpoint and CA flags are available as `--remote` and `--remote-ca`; local
socket and remote transport selection are mutually exclusive.

The desktop AOT shelf now has a named native entrypoint:
`gewyvern_validate leserpent-aot`. It detects only checked host RIDs, performs
the locked restore and no-restore publish, validates Mach-O/ELF signatures and
a bounded artifact manifest, executes all four control fixtures, and retains a
versioned evidence index. macOS arm64 self-host execution is automated; Linux
x64 uses the same command with Xvfb. Windows stays unclaimed until its lock and
physical-host proof exist.

## Explicit Deferrals

The roadmap does not require:

- replacing Gewyvern's Linux-first runtime
- moving Etragon into the Leserpent core
- making Avalonia WASM the only web renderer
- arbitrary user-native plugins
- model access to shell, raw HTTP, or host-language reflection
- a distributed scheduler before the local journal/runtime is proven

## First Implementation Slice

The first code-bearing slice should be deliberately narrow:

1. create the Rust domain/protocol crates
2. model `runtime.list` and one idempotent `runtime.refresh` command
3. lower one Leselang function into that command
4. execute it through an in-memory effect adapter
5. invoke the same command from the Rust CLI
6. compare both paths with the current Leserpent API fixture

This proves the architecture before parser breadth or GUI migration expands.
