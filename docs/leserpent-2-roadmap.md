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
VM intentionally rejects it before continuation allocation until durable
multi-branch continuation wiring is complete; that graph is still required
before the gate exits.

Exit: programs can suspend, restart, re-enter, and replay deterministically.

## Gate 3: Native CLI Parity

Build the Rust `leserpent` CLI on the shared protocol.

- query, inspect, plan, apply, watch, export, and history commands
- stable JSON mode and human-readable mode
- canonical Leselang export for every mutation
- dry-run and confirmation UX
- local IPC and authenticated remote transport

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

Exit: the vertical slice contains no direct adapter or persistence access and
passes GUI/CLI/Leselang equivalence tests.

## Gate 5: Durable Runtime Cutover

Move authority from the compatibility bridge into `leserpentd`.

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
