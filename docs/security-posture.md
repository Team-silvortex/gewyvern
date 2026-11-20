# Security Posture

This note captures the practical security boundary of `gewyvern` as it exists
today.

It is not a promise that no future issues will appear. It is a concise answer
to a narrower question:

- what `gewyvern` is safe to treat as
- what `gewyvern` is intentionally not trying to be
- which boundaries matter in the active `2.5.x` line

For long-lived runtime behavior, see
[docs/service-behavior.md](docs/service-behavior.md).

For ingest trust semantics, see
[docs/ingest-modes.md](docs/ingest-modes.md).

For external-engine collaboration shape, see
[docs/external-engine-contract.md](docs/external-engine-contract.md).

If you want the nearby companion shelves around this page, use:

- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
  Task-first preflight before turning on `--serve`, exposing the API, or
  wiring a sidecar/external engine.
- [docs/service-behavior.md](docs/service-behavior.md)
  Durable note for restart, degraded mode, ingest failure, and latest-snapshot
  service behavior.
- [docs/machine-contract.md](docs/machine-contract.md)
  Narrow downstream contract for automation and sidecar consumers.

## Dependency Audit Policy

CI runs `cargo audit --no-yanked -D warnings`; maintenance advisories are not
silently ignored. RUSTSEC-2026-0543 marks all `chrono` versions unmaintained.
Native SSH/SFTP remains enabled, using these temporary compatibility fixes:

- `vendor/russh-sftp` retains the 2.3.0 API and replaces only its `chrono` calls
  with `time`/standard-library timestamps. The upstream 3.0.1 fix requires a
  newer `serde` than the pinned Leselang core allows. Provenance, retained
  Apache-2.0 license and removal criteria are in `vendor/russh-sftp/PATCHES.md`.
- `vendor/russh-util` retains the 0.52.0 API and backports the reviewed,
  not-yet-merged upstream PR #825 (`1c0ddf4ac390d4cb60a431ab94b42bfd0a2486c3`).
  It replaces the WebAssembly wall clock with monotonic `web-time`, leaving
  native `std::time::Instant` intact.

Because cargo-audit skips path/Git packages, CI additionally audits their original
registry identities in `vendor/advisory.Cargo.lock`. Regression tests bind that
inventory to both manifests and the actual lockfile; the main audit covers the
actual transitive dependencies. Replace each patch and its supplemental identity
with a released version containing the same fix when available;
rerun the dependency audit, all-feature adapter tests and workspace Clippy/build.
The upstream repositories' audit ignore lists are not used by this workspace.

## Intended Deployment Shape

`gewyvern` should currently be treated as:

- a standalone debugger/runtime
- a latest-snapshot analysis service
- a conservative evidence and diagnosis authority

It should not currently be treated as:

- a multi-tenant control plane
- an authenticated orchestration service
- a durable event warehouse
- a public-facing observability endpoint

That means the safest default mental model is:

- local or intentionally nearby usage
- bounded sessions
- read-mostly consumers
- explicit operator intent for broader exposure

## Ingest Trust Boundary

Socket-fed ingest is intentionally advisory-first.

Current ingest modes:

- `demo`
- `local-advisory`
- `remote-advisory`

Important consequence:

- local and remote socket producers are not treated as authenticated lineage
  authorities
- PID-scoped conclusions must therefore remain guarded
- ambiguous or advisory output is often the runtime refusing to overclaim, not a
  failure of the tool

`--pid` is intentionally rejected with socket ingest for this reason.

## API Exposure Boundary

The API is a read-only latest-snapshot surface with a narrow operator-facing
access boundary.

Current posture:

- `--api-socket` requires `--serve`
- remote API bind requires explicit operator opt-in
- remote API bind also requires a configured runtime admin token
- loopback callers remain the only zero-friction default
- live API state is in-memory first
- the latest served snapshot is also mirrored into the standard state root
- each successful refresh also leaves a structured on-disk history snapshot
- that on-disk history is bounded to the newest 32 refreshes
- restart still clears the live in-memory snapshot until a new serve session
  refreshes it
- oversized successful API bodies are degraded instead of streamed without bound
- API overload is allowed to fail closed with a short `503` response

This API is suitable for:

- local operator tooling
- nearby sidecars
- lightweight automation

It is not equivalent to a multi-tenant or fleet-grade authenticated control
plane.

## External Engine Boundary

External engines are additive and bounded.

Current posture:

- `gewyvern` computes built-in analysis first
- the external engine receives an analysis snapshot through a process boundary
- returned enrichments are appended
- the built-in diagnosis spine remains authoritative

External engines should not be allowed to redefine:

- fact truth
- core failure semantics
- ingest trust
- strong PID attribution

If an external engine fails, built-in analysis should still complete and remain
usable.

## Resource And Safety Boundaries

The current tree already enforces several practical bounds that matter for the
current preparation line:

- socket ingest applies line and fact-count limits during read
- Unix sockets are created with restricted permissions
- slow API clients are handled independently and bounded by read/write timeout
  behavior
- API concurrency is bounded so remote readers cannot spawn unbounded handler
  threads
- oversized API success bodies are rejected with an explicit bounded error
  response
- external-engine execution is bounded by timeout, output caps, augmentation
  caps, and cache limits
- external-engine capability probing is bounded by the same timeout and output
  caps as full external analysis
- Linux eBPF smoke compilation, loaders, and `tc` calls use a shared native
  timeout/reaping guard with independent stdout and stderr caps
- protocol/profile discovery avoids symlink recursion and repeated-directory
  loops and now carries directory, manifest-count, and manifest-size budgets
- status catalogs, GUI function-chain catalogs, and their source evidence use
  bounded regular-file reads that reject symbolic links and file-growth races
- Gewyvern, `gewyc`, and validator JSON file outputs plus persisted API
  snapshots are size-bounded, privately staged, synced, and atomically
  replaced; streaming output append is private, size-bounded, symlink- and
  hard-link-safe, synced, and serialized through a bounded exclusive lock; the
  `gewyc init` command never overwrites an existing scaffold path, including a
  symbolic link
- certificate rotation and revocation state is size-, record-, path-, and
  field-bounded; malformed state is surfaced as invalid, raises policy
  attention, and cannot be silently overwritten by a state mutation; updates
  are cross-process locked, privately staged, synced, and atomically replaced
- legacy runtime migration is bounded by directory depth, entry count,
  per-file bytes, and aggregate bytes, and copies through non-symlink file
  handles without overwriting existing destinations
- the privileged Linux eBPF helper validates ownership and permissions on the
  same bounded file descriptor from which its configuration is read
- Leserpent protocol envelopes reject unknown nested fields and bound shared
  principal, capability, filter, and command-plan values before execution
- macOS bundle installation and frontend packaging apply bounded directory
  scans and non-symlink reads; native payload identity is calculated with a
  fixed-size streaming buffer rather than whole-file allocation

These are not a full security model, but they are the current concrete guards
that keep standalone use from drifting into obviously unsafe territory.

## What Operators Should Assume

Before exposing or automating `gewyvern`, assume:

1. socket ingest is advisory unless proven otherwise
2. the API is locally safe by default and remotely readable only through an
   explicit token-protected exposure choice
3. external engines can enrich results, but should not become truth authority
4. exported bundles and JSON reports are useful artifacts, not access-control
   boundaries

If a deployment needs:

- durable history
- authenticated access
- multi-instance coordination
- policy and audit

that should live above `gewyvern`, not be inferred from its current local
service shape.

## Practical `2.5.x` Goal

For the active `2.5.x` line, the security goal is not “become a control
plane”.

The goal is narrower:

- keep defaults conservative
- keep exposure explicit
- keep runtime/resource bounds documented and real
- keep debugger cross-validation and negative-validation evidence honest
- keep nearby extensibility additive rather than authority-stealing

That is the posture worth preserving while the project continues converging
toward a later stable release.
