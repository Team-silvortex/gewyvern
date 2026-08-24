# Leserpent Runtime Posture

## Purpose

`leserpent` should be able to stand on its own as a cross-platform control plane,
then observe and coordinate nearby runtimes.

That means its own startup and steady-state operation should depend on as few
host assumptions as possible.

`leserpent` should feel comfortable on:

- macOS
- Linux
- Windows
- local development workstations
- lightweight server hosts
- CI and container environments

without assuming kernel features that belong to `gewyvern` or another subject
runtime.

## Core Posture

`leserpent` is expected to be:

- platform-neutral
- user-space only
- HTTP-first
- file-backed by default
- operable even when every managed runtime is degraded or unreachable

It should be treated as a supervisor and coordination shell, not as a kernel
runtime or packet-processing engine.

## Current 1.x Core Dependencies

The core runtime should require only:

- the .NET runtime
- local filesystem access for state persistence
- HTTP server/client behavior
- normal user-space networking

These are the dependencies that should be enough to start the service,
restore state, render the dashboard, and explain current fleet health.

## Target 2.0 Runtime

The 2.0 architecture moves domain, command, query, policy, journal, effect,
re-entry, and replay semantics into a Rust `leserpentd` runtime. Avalonia,
the Rust CLI, Leselang, and conforming web clients become replaceable
frontends over one protocol.

This changes the implementation dependency from “the .NET service is the
control plane” to “the Rust runtime is authoritative.” It does not change the
portable, user-space, degraded-but-operable posture described here.

See the authoritative
[2.0 architecture](../../../docs/leserpent-2-architecture.md) and
[delivery roadmap](../../../docs/leserpent-2-roadmap.md).

## Optional Adapters

The following capabilities are useful, but should remain optional adapters
rather than startup requirements:

- Docker-backed scenario launch or validation
- local process launch helpers
- remote SSH-based management
- future Kubernetes or container scheduler integration
- richer discovery plugins for nearby runtimes

If one of these adapters is absent or unhealthy, `leserpent` should degrade
cleanly instead of refusing to boot.

## Explicit Non-Dependencies

`leserpent` should not require these in order to start:

- eBPF support
- Linux-only attach capabilities
- kernel verifier access
- systemd
- a database
- a message broker
- a long-lived agent mesh
- a specific container runtime

Those may matter to subjects like `gewyvern`, but they should not be part of
`leserpent`'s own boot contract.

## Degraded But Operable

A healthy control plane is not the same thing as a healthy fleet.

`leserpent` should still be considered operable when:

- no runtime has published a latest snapshot yet
- runtime capability fetches are failing
- paired sidecars are degraded or missing
- optional adapters are unavailable
- previously persisted state is all it has to work with

In those cases, the service should still be able to:

- start
- expose `/health` and `/v1/capabilities`
- restore known state
- explain which parts of the fleet are degraded
- accept or reject new runtime registrations according to current policy

## State Model

The default posture should continue to be file-backed and local-first:

- a JSON state file
- a lightweight backup file
- explicit export/import
- no external database required for first deployment

This keeps the control plane easy to run locally, easy to recover, and easy to
move between hosts.

When `LESERPENT_DAEMON_SOCKET` and `LESERPENT_DAEMON_TOKEN` are configured,
runtime registration metadata and successful discovery observations are first
persisted by `leserpentd` through revision-fenced typed commands. The managed
state remains a compatibility projection and is updated only after daemon
acceptance. Existing managed-only runtimes are reconciled into the daemon on
their next registration update. An unconfigured development host continues to
use the managed registration path.

With daemon IPC configured, `/v1/runtimes`, `/v1/runtimes/{id}`, and
`/v1/runtimes/{id}/status` read identity, endpoint, tags, status, and observed
capability facts from the daemon. The managed store supplies only the bounded
compatibility metadata that has not crossed the Rust contract. Runtime presence
also comes from the daemon: managed-only entries are omitted and detail reads
return `runtime_not_found`; daemon-only entries fail with a typed gateway error
until compatibility metadata is explicitly reconciled. Attention,
protocol-reading, recovery, Fleet summary, Fleet attention-list, and Fleet
attention-summary reads share the same projection. Fleet aggregation uses the
projected sidecar status and retains only bounded local recovery history as an
overlay. The dedicated sidecar-detail route now reads that projection as well.
Orchestra plan display, execute, retry, and session handoff rebuild plans from
one shared authoritative runtime projection, so revision checks cannot diverge
between GET and POST. Per-runtime run and event history reads also validate
daemon membership before reading durable managed history. The cleanup-plan is
the remaining read-only membership view that still needs explicit cutover
evidence before the managed overlay can be reduced further.

## Security Model

The default security posture should remain conservative and local-first:

- loopback access by default
- optional admin token for broader access
- explicit intent headers for sensitive writes
- strict endpoint validation for runtime and sidecar discovery

This keeps the control plane usable during local development while still giving
it a clear path toward protected remote operation.

## Relationship To Gewyvern

`gewyvern` can be Linux-first and runtime-surface-heavy.

`leserpent` should not inherit those assumptions.

A useful mental model is:

- `gewyvern`: subject runtime
- `etragon`: nearby sidecar or learning companion
- `leserpent`: cross-platform control plane

`leserpent` should coordinate these systems without needing to become one of
them.

## Current Direction

The current codebase already aligns with this posture in several ways:

- ASP.NET Core service shell
- local JSON state persistence with backup
- HTTP-only runtime and sidecar discovery
- optional sidecar pairing
- dashboard and API behavior that remain useful even when runtimes are
  unobserved or degraded

The next maturity step is not to add more hard dependencies. It is to keep
separating:

- core control-plane runtime
- optional integration adapters
- managed subjects and sidecars

so that `leserpent` stays portable and robust as the rest of the stack grows.
