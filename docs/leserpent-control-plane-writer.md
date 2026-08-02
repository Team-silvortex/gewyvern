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

Writer-claim commit is also covered by a deterministic unclean boundary test
without a production failpoint. A separate process calls the production claim
path while a SQLite reader holds the DELETE-journal writer inside its
`synchronous=FULL` commit. `SIGKILL` at that point recovers the complete prior
generation, rejects replacement before the fixed 30-second owner lease expires,
then admits generation `2` after natural expiry. A second process is killed
after generation `3` commits but before owner cleanup; integrity check and
direct recovery retain the complete generation `3` row. Physical Linux x86_64
evidence is retained under `docs/fixtures`.

A caller may also lose a successful claim response without making the claim
ambiguous inside the authority. The production daemon IPC test sends writer A's
claim completely, observes its durable generation `1` without decoding the
response, and discards that socket. Independent same-A and competing-B clients
then start through one barrier. If A linearizes first it replays generation `1`
and B advances to `2`; if B linearizes first it advances to `2` and A becomes a
new non-replayed generation `3` takeover. The test admits only these two serial
orders, rejects the losing ticket, applies a real registration with the final
ticket, and proves replay of the final identity does not advance generation.

That replay contract also survives a cold daemon boundary. A first daemon
commits writer B generation `2`, while the client deliberately never decodes
the response, then exits cleanly and removes its socket. The replacement daemon
opens the same database; an incomplete frame temporarily occupies its serial
IPC accept path while complete B-replay and C-competitor connections queue in
that order. Releasing the gate yields B/`2` as a replay and C/`3` as one new
claim. B/`2` is then rejected for mutation, C/`3` applies the mutation, and a
third cold daemon still replays C/`3` without advancing generation.

Unclean response loss now covers the complete deployment path, including the
configured socket name. A daemon commits B/`2` with no decoded response and is
then `SIGKILL`ed. Before the fixed owner lease expires, a replacement is
rejected by SQLite ownership and must leave the stale socket untouched. After
natural expiry, the replacement safely reclaims that same path only when it is
a `0600` socket owned by the effective UID, has no live listener, and retains
the same mode/device/inode through revalidation. Live listeners, insecure
sockets, regular files, and symlinks fail closed. The recovered daemon replays
B/`2`, advances one queued competitor
to C/`3`, rejects B/`2` for mutation, and applies the mutation with C/`3`.

The full unclean sequence is also repeated twice against one database and one
socket path. Cycle one commits unread B/`2`, kills the daemon, naturally
recovers and replays B/`2`, then advances C/`3`. Cycle two commits unread A/`4`
from that recovered daemon and repeats the same path before advancing B/`5`.
Both pre-expiry starts fail without touching the socket, both natural expiries
rebind it safely, generations remain contiguous from `1` through `5`, prior
C/`3` and A/`4` tickets are rejected, and final B/`5` both mutates and replays.

Recovery is also bounded under one full production IPC admission batch. After
an unread B/`2` claim, `SIGKILL`, natural lease expiry, same-path rebind, and
B/`2` replay, 64 independent writer IDs start through one barrier. All claims
must complete within 5000 ms, each must be non-replayed, and their transaction
order must allocate every generation from `3` through `66` exactly once. The
recovered B/`2` ticket and generation `65` are rejected for mutation; only the
generation `66` writer applies a real registration and replays without another
advance. Physical Linux x86_64 evidence completes the contention slice without
adding hot failover, consensus, or concurrent write authority.

The saturated batch also tolerates duplicate retries after response
abandonment. Sixteen ordered groups each queue one complete new claim whose
client closes its read half but retains the descriptor, followed by three
readable same-ID retries. The incomplete accept gate consumes the first
per-tick slot, so the 64 claims cross the production batch boundary. Every
abandoned primary still commits, all 48 retries replay their group's generation,
generations advance contiguously from `3` through `18`, and the batch completes
inside 5000 ms. Broken response delivery remains isolated to its peer; only
generation `18` can mutate and replay.

Hostile peer intake is bounded without parallelizing authority mutation.
`poll_batch` first accepts at most 64 Unix connections, reads their frames in
parallel under the existing 2000 ms per-peer timeout, then joins and dispatches
them serially in accept order. A ready accepted prefix is dispatched before the
daemon waits on later readers, but no later frame can overtake it. A physical
Linux batch interleaves 16 malformed
frames, 16 wrong-token claims, 16 slowloris prefixes that reach the full
timeout, and 16 valid claims after unclean recovery. Malformed peers receive
`invalid_json`, unauthorized peers receive `unauthorized`, slow peers receive
no response, and only valid claims allocate contiguous generations `3` through
`18`. The accept gate makes the workload span two bounded waves; valid progress
completes inside 5000 ms and final generation `18` alone can mutate.

Repeated hostile admission remains compatible with owner heartbeat and graceful
shutdown. Two consecutive 64-peer batches each mix 16 malformed frames, 16
wrong-token claims, 16 full-timeout prefixes, and 16 valid same-writer replays.
After each batch the same SQLite owner token must expose a newer lease with at
least 29 seconds remaining, while the writer generation stays unchanged.

The daemon's batch path is signal-aware rather than merely timeout-bounded.
Frame reads check the process stop flag every 100 ms while preserving the hard
2000 ms wall-clock peer deadline even when bytes keep trickling in, and serial
authority dispatch stops as soon as shutdown is observed. A third batch holds
64 slow peers in that read window before
`SIGTERM`; the daemon must exit inside 1000 ms, delete its owner row and private
socket through normal RAII cleanup, and permit an immediate same-database,
same-socket restart that replays the existing writer generation. Physical Linux
x86_64 retains 2234 ms and 2209 ms hostile batches plus 165 ms shutdown under
the same contract; the fixture records owner/socket cleanup and immediate
generation-1 replay without credentials.

Repeated process cycles retain bounded resources rather than merely bounded
latency. Three physical Linux daemons each complete one mixed 64-peer batch and
return to 5 open FDs plus 1 task. A following 64-slow-peer wave is not inferred:
`/proc/<pid>/fd` and `/proc/<pid>/task` must expose exactly 69 FDs and 65 tasks,
proving all accepted sockets and scoped readers are active before `SIGTERM`.
Each exit joins the readers, removes `/proc/<pid>`, deletes its owner row and
socket, and lets generation 1 replay in the next process. Observed shutdowns are
216 ms, 207 ms, and 208 ms.

Repeated hostile waves cannot starve valid reconnects. Three physical Linux
waves each fill the 64-connection cap with four groups of 15 slowloris peers
followed by one valid same-writer reconnect. All 12 reconnects replay generation
1 within 2186-2224 ms, every full wave drains within 2195-2225 ms, and the same
owner heartbeat advances after each wave. The isolated ready-prefix proof
returns in 70 ms while a later slow reader remains active, demonstrating that
only earlier accepted peers can impose the deterministic ordering boundary.

Local admission pressure also cannot starve the remote read plane or daemon
maintenance. Each daemon turn runs one bounded host step first and alternates
which transport is polled first afterward. Three physical Linux waves each
hold 64 incomplete Unix IPC peers while a real bearer-authenticated TLS/HTTP
runtime-list query is issued. The queries complete in 2264, 2241, and 2226 ms;
the complete waves drain in 2265, 2241, and 2227 ms. The owner lease advances
after every wave and writer generation remains 1, so transport fairness neither
creates write authority nor weakens the existing writer fence.
