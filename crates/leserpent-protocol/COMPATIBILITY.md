# Leserpent Protocol Compatibility

Wire v1 is a strict versioned boundary. Request, response, and event envelopes,
health payloads, protocol errors, queue health, and remote runtime projections
reject unknown fields instead of silently discarding them. Additive wire fields
therefore require an explicit schema-version transition; documented optional
v1 fields may still be absent for legacy peers.

This policy governs the migration bridge between the Leserpent 1.x ASP.NET API
and the Rust domain/protocol kernel. The bridge translates representations; it
does not own business rules.

## Supported 1.x Slice

The first compatibility slice covers:

- `GET /v1/runtimes` with `environment`, `cluster`, and `role` filters
- confirmed `POST /v1/runtimes/{id}/deployments` requests before any remote
  deployment side effect
- the atomic Orchestra `run + event` persistence envelope used to preserve
  identity, outcome, attempt, step, and request-ID relationships
- `POST /v1/runtimes/{id}/refresh-status`
- the `runtime_not_found` error returned by those runtime surfaces

Canonical 1.x response fixtures live in `tests/fixtures/legacy-*.json`. The
versioned Rust envelope fixtures remain in `tests/fixtures/*-v1.json`.

## Normalization Rules

- blank filters become absent filters
- non-blank filters are trimmed and matched case-insensitively
- runtime lists are ordered by name case-insensitively, then by runtime ID
- 1.x camelCase status fields map to the domain's snake_case status projection
- refresh requests always use `compatibility_adapter` as audit origin
- refresh observations must match the requested runtime ID and revision
- deployment requests bind the route runtime ID, confirmation, audit principal,
  pipeline intent, optional target, and idempotent request ID in one strict
  camelCase envelope
- deployment compatibility validation runs after local runtime/capability
  checks but before network I/O; bridge rejection cannot leave a hidden remote
  deployment behind or change existing not-found/capability responses
- when configured, Rust returns the canonical deployment envelope and the 1.x
  host uses those exact normalized fields for both the remote adapter and
  Orchestra audit; C# does not independently renormalize authoritative intent
- duplicate mutation requests use the normal principal-scoped idempotency rules
- compatibility JSON is subject to the same 1 MiB message limit as native wire
  envelopes

Unknown 1.x presentation fields may be ignored when they do not change the
normalized runtime identity, tags, status, authorization, event, or revision.
Secrets and token-presence metadata are not copied into the domain projection.

## Change Policy

The compatibility fixture set is append-only within a released wire version.
A changed normalization rule requires a protocol contract version change and a
fixture that demonstrates both the old input and the new normalized result.
The Orchestra persistence fixture deliberately validates outside the current
synchronous SQLite transaction. Blocking on the asynchronous bridge while a
database transaction is open is forbidden; authority moves only when Rust owns
the persistence call rather than acting as a post-write observer.
The configured ASP.NET route now delegates Orchestra storage to the daemon only
after the live IPC path passes these fixtures and the existing Leserpent
security tests. An unconfigured development host remains on managed SQLite.

## Live Bridge

`leserpent-compat-bridge` is a persistent line-delimited JSON process. The 1.x
host enables it only through an absolute `LESERPENT_RUST_BRIDGE_BIN` path. Each
request carries a correlation ID and one fixed operation; frames are capped at
1 MiB before allocation growth. The host serializes responses with its normal
source-generated JSON contract, asks Rust to validate that exact payload, and
only then returns or commits it. Deployment request validation is deliberately
pre-effect; the bridge never validates a deployment only after it has occurred.

The bridge is optional during source migration. When configured, failure is
closed with `502 compatibility_bridge_failed`; the host retries one transport
failure after restarting the child process. Packaging the bridge beside the
1.x host remains required before it can be enabled by default.

Wire-v1 also accepts the typed, create-only `runtime_register` command. It
requires `runtime.register`, explicit confirmation, no expected runtime
revision, and bounded secret-free name, endpoint, and tag fields. Unknown
command fields fail closed, so pairing or admin tokens cannot accidentally
cross this domain boundary. The daemon journals successful registration,
returns an idempotent command result, schedules no external effect, and restores
the projection after restart. This is a protocol/runtime foundation rather than
a claim that the 1.x Web registration route has already cut over; update and
canonical endpoint-conflict semantics remain the next compatibility gate.

The daemon's typed deployment receipt is intentionally narrower than a generic
effect-result query. It requires deployment capability, binds command ID and
request ID to the persisted deployment payload, and returns only pending,
completed, or failed state. A completed receipt carries the adapter-validated
Gewyvern outcome; another effect kind or mismatched request identity fails
closed without exposing its payload or outcome.

The daemon also accepts the frozen Orchestra run/event envelope through the
typed `orchestra_persist` operation. It requires `orchestra.write`, validates
cross-record identity and outcome invariants, and commits both canonical records
in one schema-v10 transaction. An exact event replay returns the stored pair and
the unchanged event count; reusing its identity with different bytes fails
closed and rolls back the run update. The configured ASP.NET store consumes this
operation directly and never opens a managed Orchestra transaction.
The companion `orchestra_history` operation uses the same capability and
returns either canonical run pages or runtime/run-bound event pages. Limits are
restricted to 1 through 64 records, offsets are bounded, and event IDs are the
authoritative database sequence. It never returns both record families in one
page.
The matching `orchestra_delete` mutation accepts 1 through 128 unique runtime
IDs and atomically deletes their runs and cascading events. Its response reports
actual affected runtime, run, and event counts without exposing stored payloads.

## Reproducible Proof

Prove that the configured C# host consumes the canonical envelope returned by
the real Rust child process:

```bash
cargo build --locked -p leserpent-protocol --bin leserpent-compat-bridge
LESERPENT_TEST_RUST_BRIDGE_BIN="$PWD/target/debug/leserpent-compat-bridge" \
  dotnet test apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj \
  --no-restore --filter \
  FullyQualifiedName~ConfiguredRustProcessReturnsTheCanonicalDeploymentAuthority
```

Prove the complete configured deployment path through a real daemon, its
durable effect worker, and a bounded local Gewyvern endpoint:

```bash
cargo build --locked -p leserpentd --bin leserpentd
LESERPENT_TEST_DAEMON_BIN="$PWD/target/debug/leserpentd" \
  dotnet test apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj \
  --no-restore --filter \
  FullyQualifiedName~ConfiguredRustDaemonExecutesTheDeploymentEndToEnd
```

Run `gewyvern_validate leserpent-transport` from the workspace root to prove
the current wire-v1 authenticated local, HTTPS, and WebSocket transport boundaries. The command
retains separate logs for protocol fixtures, legacy adaptation, CLI/Leselang
parity, the native CLI-to-daemon Unix socket, IPC rejection paths, and a real TLS
loopback through `POST /v1/wire`, plus the native remote CLI and revisioned
`/v1/events` vertical paths. Event frames use an independently versioned schema,
omit runtime endpoints, and require the `leserpent.events.v1` subprotocol. Its
summary explicitly excludes Windows named pipes, remote GUI, and mobile clients, so passing this
shelf does not imply full Gate 6 completion.
