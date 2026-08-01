# Leserpent control-plane writer

Leserpent's transitional JSON control plane is single-writer across processes.
This is a cold-takeover reliability contract, not an active-active claim.

## Admission

At startup, each host competes for an owner-private, non-symlink process lease
derived from the canonical control-state path. The lease binds process ID,
stable process-start identity, and a random owner token. Exactly one fresh
process becomes `owner`; every other process remains `standby` for its lifetime.

Authenticated writer health is available at:

```text
GET /v1/persistence/control-writer-health
```

The response reports `starting`, `owner`, `standby`, `lease_lost`,
`authority_claim_failed`, or `stopped` without exposing paths, tokens, writer
IDs, or raw errors. When leserpentd authority is configured, it also reports
the active numeric authority generation.

## Mutation fence

The machine-readable route inventory is
`docs/contracts/leserpent-control-plane-mutations-v1.json`.

All non-read methods under `/v1` require writer ownership unless explicitly
listed as a read-only POST. The current sole exception is runtime registration
planning. Unknown future non-read routes fail closed.

The fence is deliberately redundant:

1. HTTP middleware rejects standby requests before validation, discovery, or
   external authority effects.
2. `RegistryService` checks ownership before in-memory, JSON, or SQLite
   mutation.
3. Persistence methods repeat the check as a final write backstop.
4. Runtime deletion recovery and checkpoint workers stay idle on standby and
   stop if ownership is lost.

For daemon-backed registration, discovery intake, unregistration, deployment,
status/capability refresh, bootstrap session binding, and Orchestra writes
there is now a fifth layer. After acquiring the local process lease, the owner
submits an authenticated, idempotent claim with a fresh random writer ID. The
Rust authority allocates and persists a monotonically increasing generation in
runtime journal schema v19. Every covered IPC mutation carries the resulting
`(generation, writer_id)` ticket. One shared managed frame codec emits that
ticket for registration, deployment, and Orchestra adapters.

Claims are accepted only on the owner-private local Unix IPC transport.
Remote HTTP wire requests cannot allocate or replace a writer generation.

Before the first claim, a migrated authority accepts the legacy unfenced
registration contract. After any claim commits, missing tickets fail with
`authority_writer_fence_required` and replaced generations fail with
`authority_writer_fence_rejected`. The daemon serializes claim and mutation
dispatch through its single owned `ControlRuntime`, so a newer claim
linearizes before every subsequently rejected stale mutation. Claim retries
with the same writer ID return the same generation.

The covered Orchestra mutation set is `orchestra_persist`,
`orchestra_delete`, `orchestra_delete_command`, and
`orchestra_delete_replay_checkpoint`. History, deployment-receipt, and replay-
horizon reads remain unfenced queries. Missing or stale deployment and
Orchestra tickets are rejected before effect enqueue or SQLite mutation.
Runtime status refresh, capability refresh, and bootstrap session binding are
also fenced before projection, effect enqueue, verifier work, or checkpoint
mutation. `debugger_cancel` remains outside this authority because the control
runtime rejects it and delegates execution to the Leselang VM authority.

The explicit local `bootstrap_v1`, `provisioning_v1`, `retirement_v1`, and
`daemon_retirement_v1` routes validate the same ticket before protocol decode
or submission. The native CLI can forward an owner-issued ticket through the
paired `LESERPENT_AUTHORITY_WRITER_ID` and
`LESERPENT_AUTHORITY_WRITER_GENERATION` variables; it never claims or takes
over authority implicitly.

Authenticated HTTPS carries the same ticket in the paired
`X-Leserpent-Authority-Writer-Id` and
`X-Leserpent-Authority-Writer-Generation` headers. Duplicate, partial, zero,
or malformed tickets fail as `400/invalid_authority_writer_fence` after Bearer
authentication. `/v1/wire` applies the ticket only to authority mutations; the
four dedicated mutation routes validate before protocol decode.
`/v1/leselang-export` and wire reads remain unfenced.

The inventory is executable rather than documentary. Status TDD recursively
scans every C# `MapPost`, `MapPut`, `MapDelete`, and `MapPatch` under the host,
and scans the Rust HTTPS route table. Either source set must exactly match the
machine-readable inventory. Rust wire classification is an exhaustive match
over both `ProtocolRequest` and `Command`, so adding an enum variant requires an
explicit fenced or read/delegated decision at compile time.

A rejected HTTP mutation returns:

```json
{
  "error": "control_plane_writer_standby",
  "reason": "This leserpentd instance is read-only because another process owns the control-plane writer lease."
}
```

## Takeover

An already-loaded standby never promotes itself after the owner exits because
its compatibility projection may be stale. Operators or supervisors start a
fresh process, which validates the stale owner record, acquires the lease, and
reloads JSON and Orchestra state before accepting writes.

All inventoried local and remote authority mutation entry points now adopt the
Rust writer ticket after its first claim. This contract still does not provide
hot failover, quorum consensus, or multi-writer conflict resolution. A standby
remains read-only, and takeover still requires a fresh process that reloads
authoritative state. A real three-daemon-process proof additionally verifies
that a live owner excludes a contender, graceful shutdown releases the lease,
the replacement advances generation `1` to `2`, the old writer is rejected,
and replaying the replacement identity after another restart retains
generation `2`.
