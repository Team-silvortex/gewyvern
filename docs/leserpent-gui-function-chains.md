# Leserpent GUI Function Chains

This document defines how Leserpent distinguishes renderer capability from a
user-reachable product feature. The authoritative inventory is
[`project/release/leserpent-gui-function-chain.json`](../project/release/leserpent-gui-function-chain.json).
Its Rust validator lives in `src/gui_function_chain.rs`.

## Closure Rule

A GUI feature is closed only when every stage declared by its function chain
has source-anchored production evidence:

1. `entry`: a user can reach the control from the shipped product.
2. `semantic`: the control resolves to a typed, frontend-neutral intent.
3. `transport`: the intent crosses the authenticated bounded transport.
4. `authority`: the intended Rust authority, or the declared 1.x compatibility
   authority, executes it.
5. `persistence`: durable mutations retain state, audit, or a receipt.
6. `projection`: the result returns through a bounded safe projection.
7. `feedback`: the operator sees a terminal or explicitly pending outcome.
8. `automation`: the same operation is available through canonical Leselang
   when equivalence is part of that chain.

Renderer fixtures cannot satisfy `entry`, transport mocks cannot satisfy
`authority`, and a successful request without authoritative projection cannot
satisfy `feedback`.

## Coverage States

| State | Score | Meaning |
| --- | ---: | --- |
| `closed` | 100 | Every required stage has production evidence. |
| `partial` | 50 | A production path exists, but at least one required stage or operation is missing. |
| `conformance-only` | 25 | Protocol, fixture, or renderer proof exists without a product path. |
| `absent` | 0 | No implementation exists on the required surface. |

Target and bridge surfaces are reported separately. A closed 1.x Web bridge is
valuable compatibility evidence, but it does not make the Rust-hosted 2.0 Web
surface complete.

## 2026-08-28 Baseline

| Surface | Lifecycle | Score | Closed | Partial | Conformance only | Absent |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Avalonia desktop | target | 100 | 9 | 0 | 0 | 0 |
| Rust-hosted Web | target | 100 | 1 | 0 | 0 | 0 |
| ASP.NET Web | bridge | 100 | 5 | 0 | 0 | 0 |

The combined target score is 100. The closed Avalonia families are fleet
observation, runtime control, existing-runtime registration, daemon lifecycle,
Gewyvern lifecycle, authority health, Rust-authoritative Orchestra control, and
the suspended Leselang debugger workflow, plus live product Leselang
presentation automation. The native
Orchestra workspace now closes revision-fenced plan discovery, automatic run,
queued-only cancellation, lineage-bound retry, authenticated history/event
drilldown, and idempotent cleanup. Guided plans stay visibly review-first. The
registration editor separately closes already-running runtime intake and
revision-fenced metadata updates: the daemon produces a side-effect-free plan,
field edits invalidate it, and an explicit confirmation applies the same command
identity without sending deployment credentials or service secrets. The
Rust-hosted console now maps plan discovery, run and event history, the fleet
board, execute, cancel, retry, and generic session handoff directly onto native
Rust authorities. The session aggregate is bounded, request-idempotent,
restart-recoverable, portable across export/import, and removed transactionally
with its owning runtime. This closes the last target GUI function-chain gap
without moving authority back into the ASP.NET bridge.

`leserpentd` now serves the exact packaged TypeScript console from its existing
authenticated HTTPS listener. Public HTML, JavaScript, CSS, branding, and
language-pack requests reject credential-bearing variants; API requests require
the daemon Bearer token and reject disagreeing legacy and Authorization values.
The first-screen capabilities, fleet summary, attention, runtime, session, and
safe cleanup-preview reads project live `ControlRuntime` state without endpoint
credentials. With explicit `--web-console-writer`, fleet status/capability
refresh enters the existing typed effect queue and single-runtime deletion uses
the durable revision-fenced unregistration transaction. Each daemon start uses
a fresh CSPRNG writer identity, and the resulting ticket stays inside
`leserpentd`; same-origin mutation intent is mandatory, and a later bridge writer
claim moves Rust Web to visible `409` standby. Cleanup previews now classify the
live failed, unobserved, and filtered-slice targets into canonical v2 plan tokens;
execution recomputes that plan, requires `CLEAR N` for a whole slice, commits at
most 128 revision-fenced targets atomically, and replays a durable receipt after
a lost response. The shared UI disables cleanup controls whenever the daemon
reports that this writer capability is unavailable. The adapter layer now
atomically creates, replaces, and removes native Keychain or Secret Service
items. Health, status, discovery, and deployment share an atomically replaceable
target catalog, so a committed target becomes visible to every adapter without
restart. Registration planning remains strict and secret-free. The writer-only
`/v1/runtimes/register` path now persists a schema-v21 intent before writing the
credential, applies an idempotent runtime command, hot-activates the target, and
then commits a durable binding. Exact retries replay the same operation, startup
recovers every pre-commit window, and copy-on-write secret aliases are drained by
durable garbage collection after rotation, conflict, or deletion. The coordinator
accepts root `http://127.0.0.1:PORT/` and `http://[::1]:PORT/` origins without a
CA. A remote Gewyvern target must instead use a root HTTPS origin without path,
query, or credentials and carry an explicitly reviewed PEM CA of at most 32 KiB.
The browser computes and displays its SHA-256 fingerprint before planning; the
secret-free v3 plan token binds that digest, and submission rejects PEM drift
before mutating runtime state, credentials, or bindings. Binding schema v2 keeps
the public CA and digest for exact restart recovery while legacy schema-v1
loopback bindings remain readable. Ambient platform PKI is never registration
authority. Real TLS tests prove accepted in-memory CA trust, wrong-CA rejection,
the browser-to-daemon boundary, and registration replay.

Persistence checkpoint and export no longer cross the bridge. Writer mode gates
`POST /v1/persistence/save`, which commits the runtime's checksum-bound snapshot
only after its journal is terminal and is restart-recovered by the same SQLite
authority. Capabilities derives `lastSavedAt` from the newest checksum-valid
snapshot instead of maintaining a second status cache. Authenticated
`GET /v1/persistence/export` emits the legacy-compatible
schema-1 control-plane document directly from live runtime projections, walks all
retained Orchestra runs through bounded validated pagination, rejects export while
registration recovery is pending, and never serializes target credentials or
internal secret handles. The response is a `no-store` attachment and remains under
the protocol response bound. The shared importer now accepts the backend's
advertised compatible schema range instead of incorrectly requiring exactly
schema 1. Writer-fenced `POST /v1/persistence/import` now strictly decodes that
portable schema, validates and imports bounded sessions, rejects unresolved
recovery metadata, non-terminal Orchestra runs, active effects, and incomplete
authority checkpoints, then atomically replaces the revision-rebased domain
snapshot, validated Orchestra history, and session aggregate. Two checksum-bound
copies define the new recovery epoch, runtime logs are cleared, and both static
catalog targets and dynamic credential bindings must retain their canonical
runtime/origin identity. Unit, restart, and real TLS tests cover success,
rollback, and conflict paths.

The same authenticated host now exposes strict dynamic Orchestra routes. Plan
responses adapt the Rust catalog to the shared camel-case UI contract; bounded
history/event readers and a 256-item fleet projection consume only validated
SQLite envelopes. Execute, cancel, and retry require daemon-owned writer mode,
retain command replay and plan-revision fences, and return `202 Accepted`. The
guided session plan uses the same writer fence and exposes its control only when
`orchestraSessionHandoffAvailable` is true. SQLite schema 22 persists the
request-idempotent session and its terminal handoff run, while strict list,
detail, create, and stop routes expose the bounded projection. Real TLS tests
prove create, replay, lookup, stop, and direct session creation over the shared
TypeScript contract. The Rust Web target is therefore closed.

The debugger workspace starts a bounded daemon-owned VM only to its first
effect, mounts the Rust-authored `UiDocument`, and routes its session-bound
cancel action through a strict .NET client. Cancellation is reviewed as a
side-effect-free dry-run, sealed to the issuing client and principal, before
explicit confirmation consumes the continuation and writes the VM audit. The
desktop New action stays fenced while that effect is waiting. Active-session
reconstruction after a daemon restart remains a VM-host resilience concern, not
part of this closed in-process observation-and-cancellation chain. Deadline
expiry still converges to a terminal revision and releases the 32-session
registry. Cancellation audit is restart-durable within a bounded 64-journal
retention horizon.

The debugger's **Run live** entry now starts that same Rust VM and receives only
the typed pending `PresentationOperation`; continuation state never crosses the
wire. Avalonia applies each atom to the current product window, reports a typed
applied or rejected outcome, and lets the Rust authority consume the continuation
and persist the re-entry result. The loop is capped at 64 effects, strict .NET
decoding binds effect ID, revision, node ID, and required capabilities, and an
adapter rejection converges to visible terminal failure instead of leaving a
suspended session. Wire, real TLS, strict-client, and native-control probes cover
the closed chain.

## Commands

Validate the status tensor and all GUI source anchors together:

```bash
cargo run --quiet --bin gewyvern_status -- validate
```

Inspect the GUI-only human or machine view:

```bash
cargo run --quiet --bin gewyvern_status -- gui
cargo run --quiet --bin gewyvern_status -- gui --json
```

Any new operator command, query, protocol request, product route, or GUI surface
must be added to the inventory and assigned to exactly one function chain. A
non-closed claim must retain an explicit gap; a closed claim must carry every
required stage anchor.
