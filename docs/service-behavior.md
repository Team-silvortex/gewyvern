# Service Behavior

This page is a durable operational note for long-lived `gewyvern` service
behavior.

It describes what to expect when `gewyvern` is used as a standalone
CLI/service with:

- `--serve`
- `--api-socket`
- `--external-engine-bin`

It focuses on restart, failure, and degraded-mode expectations.

For the higher-level system map, see
[docs/system.md](docs/system.md).

For the machine-facing data contract, see
[docs/machine-contract.md](docs/machine-contract.md).

If you want a task-first validation path instead of a behavior note, use:

- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)

If you want the nearby companion shelves around this page, use:

- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
  Task-first preflight for exposing or operating the service surface.
- [docs/machine-contract.md](docs/machine-contract.md)
  Narrow machine-facing contract for the API and analysis surfaces.
- [docs/security-posture.md](docs/security-posture.md)
  Broader deployment and exposure boundary for what this service should be
  trusted as.

## Scope

This note is about runtime behavior, not schema details.

It answers:

- what stays available during `--serve`
- what the API actually serves
- what happens when ingest fails
- what happens when the external engine fails
- what `degraded` does and does not mean

## `--serve` Lifecycle

`--serve` is the long-lived session loop for socket ingest.

Current expectations:

- `--serve` requires `--unix-socket` or `--tcp-socket`
- a single `gewyvern` process stays alive across many sessions
- each completed session or scan refreshes the in-memory latest snapshot
- the optional API serves that live latest snapshot
- the standard state root mirrors that latest snapshot on disk
- each successful refresh also archives a structured history snapshot on disk
- the on-disk history keeps the most recent 32 refreshes and prunes older ones

`--serve` is intentionally session-oriented, not a historical datastore.

It should be understood as:

- accept one socket-fed session or sweep
- analyze it
- publish the latest result
- wait for the next one

## API Snapshot Model

When `--api-socket` is enabled, the API exposes only the latest live snapshot.

This means:

- there is no built-in historical query API
- each new completed session replaces the previous latest snapshot
- historical snapshots are persisted on disk for operator-side inspection, not
  served as a queryable timeline API
- persisted history is intentionally bounded to the most recent 32 refreshes

The intended use is:

- local operator tooling
- nearby sidecars
- lightweight automation

and not a multi-tenant observability backend.

## Restart Expectations

Current restart behavior is intentionally simple:

- restarting `gewyvern` clears the in-memory latest API snapshot
- restarting `gewyvern` does not restore prior API state
- new socket-fed sessions rebuild the latest snapshot from scratch
- previously mirrored latest/history snapshot files may still remain under the
  standard state root until operators rotate or prune them

This is expected behavior, not a failure mode.

## Socket Ingest Failure Behavior

Socket ingest is expected to be noisy and partially unreliable.

The current service behavior is:

- a bad session should not terminate the whole `--serve` loop
- malformed or rejected ingest is handled per session
- the service continues waiting for future sessions

For TCP/Unix socket ingestion:

- local advisory ingest is the preferred default
- remote ingest stays explicitly opt-in
- PID-attributed conclusions remain guarded by trust and advisory semantics

## API Failure And Exposure Behavior

The API is intentionally conservative by default.

Current expectations:

- `--api-socket` requires `--serve`
- remote API bind is rejected unless `--allow-remote-api` is explicitly set
- each API client is handled independently so one slow client should not block
  the whole listener
- API client reads and writes are both bounded by timeout behavior
- API handler concurrency is bounded; overload may return a short `503
  service_busy` response instead of spawning unbounded worker threads
- oversized successful API bodies are rejected with a bounded `503
  response_too_large` payload rather than streamed without limit
- API routes serve the latest snapshot if present, or `404`/empty-state style
  responses if not

This makes the API suitable for local or intentionally exposed read-only
consumers, but not equivalent to an authenticated control plane.

## External Engine Hook Behavior

When `--external-engine-bin` is configured, `gewyvern` still owns the core
analysis snapshot.

The external engine is additive.

Current expectations:

- `gewyvern` computes its built-in analysis first
- the external engine receives the analysis snapshot over a process boundary
- returned augmentations are appended to built-in augmentations
- the diagnosis spine remains authoritative

The external hook is therefore:

- enrich/rerank friendly
- safe to disable
- intentionally unable to replace the core analysis model

## External Engine Failure Behavior

External analysis failure should degrade gracefully.

Current behavior:

- the core `gewyvern` analysis still completes
- external failure does not delete built-in conclusions
- an advisory augmentation such as `external_engine_failed` may be appended
- downstream consumers can still rely on the core diagnosis spine
- capability probing for an external engine is subject to the same timeout and
  output-budget expectations as a full external analysis invocation

This is important for standalone operation:

- `gewyvern` must remain useful without any external sidecar
- external engines may improve the result, but they are not required for the
  base runtime truth

## Degraded Mode Semantics

`degraded` should be read as:

- the runtime saw loader failures, fact rejections, or another condition that
  reduces confidence in the completeness of the observed session

It should not be read as:

- "the result is unusable"
- "the process crashed"
- "the API is broken"

In practice:

- a degraded run may still produce a useful diagnosis spine
- degraded status is a signal to read confidence, basis, and operator guidance
  more carefully

## Practical Reading Order

For long-lived service operation, a good reading order is:

1. `ingest_mode`
2. `pid_attribution_status`
3. `primary_failure_*`
4. `operator_guidance_*`
5. `augmentations`
6. `degraded`

That sequence tells you:

- how trustworthy the source is
- how strong the diagnosis is
- what the built-in next step is
- whether any additive external hints were present
- whether the session had operational rough edges

## What This Service Is Not

`gewyvern --serve` should not currently be treated as:

- a durable event store
- a fleet control plane
- a historical analytics warehouse
- an authenticated orchestration service

Its intended shape is narrower:

- standalone runtime debugger
- latest-snapshot analysis service
- conservative evidence and diagnosis authority
