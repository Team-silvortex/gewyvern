# Leserpent Protocol Compatibility

Wire v1 is a strict versioned boundary. Request, response, and event envelopes,
health payloads, protocol errors, queue health, and remote runtime projections
reject unknown fields instead of silently discarding them. Additive wire fields
therefore require an explicit schema-version transition; documented optional
v1 fields may still be absent for legacy peers.

This policy governs the migration bridge between the Leserpent 1.x ASP.NET API
and the Rust domain/protocol kernel. The bridge translates representations; it
does not own business rules.

## Draft Bootstrap Boundary

Host bootstrap is intentionally separate from the authenticated daemon wire.
`leserpent_protocol::bootstrap` defines a strict schema-v1 envelope capped at
64 KiB. Its request carries a principal, the `host.bootstrap` capability, a
confirmed target intent, and only a validated `vault:<provider>:<key>`
credential handle. Unknown fields such as passwords, private keys, and session
tokens fail decoding. The response carries only a validated bootstrap state or
a bounded error. Canonical request and planned-state response fixtures live in
`tests/fixtures/bootstrap-*-v1.json` and freeze the draft field shape.

This draft boundary does not change ordinary wire v1 and is not accepted by
`leserpentd` at `/v1/wire`. A platform bootstrap adapter must resolve the vault
handle locally, install or reconcile the daemon, and return a daemon-issued
session handle. Runtime commands remain unauthorized until the domain state is
`session_bound`. Promoting this draft requires native service activation and
retained cross-process positive and negative proof.

The first implementation is `leserpent-adapters::SshBootstrapAdapter` with the
pure Rust `NativeSshBootstrapTransport`. SSH bootstrap and daemon session
secrets are resolved from different provider-scoped handles. Its internal
installer exchange is not a public daemon wire: it is a bounded, versioned,
single-purpose stdin/stdout exchange over a host-key-pinned SSH channel, and
only a validated `BootstrapResponseEnvelope` may leave the adapter.

`leserpent_protocol::bootstrap_installer` owns the separate internal installer
wire used inside that pinned SSH channel. It caps requests and responses at
64 KiB, redacts and zeroizes the session token, rejects unknown fields, and
distinguishes an atomically `installed` generation from a health-proven `ready`
service. Adapter compatibility requires `ready`; `installed` is intentionally
non-authoritative.

Installer response v1 also carries the target-generated public CA PEM and its
SHA-256. Decoding verifies that the digest matches the PEM bytes. The private
key and session token never enter the response; daemon services consume the
token from a private bounded token file instead of command-line or environment
text.

The target installer also retains a mode `0600` native service descriptor in
each immutable generation: launchd plist on macOS and systemd unit on Linux.
The descriptor references the private token, TLS identity, database, and logs by
path and contains no token text. Replay compares it byte-for-byte with the
expected descriptor. Service-manager activation remains outside installer wire
v1. The target publishes the verified descriptor atomically before advancing
its `current` generation, but does not load it into the service manager;
therefore the response remains `installed` rather than `ready`.

The target has a native launchctl/systemctl activation primitive, fenced by the
request-derived current generation and byte-identical retained/published
descriptors. It uses no shell and carries no token in manager arguments. The
SSH installer command now invokes it through `bootstrap-activate-v1`, then runs
a bounded loopback health request whose TLS server name remains the requested
endpoint host. The response may claim `ready` only after generated-CA validation,
session authentication, strict wire-v1 decoding, and daemon authority proof.
The separate `bootstrap-install-v1` preparation command still returns only
`installed`.

Bootstrap state schema v1 now carries an optional opaque
`trust_credential_handle`. It is absent before deployment and after failure, and
required together with the session handle in `bootstrapped` and `session_bound`
states. The native SSH adapter persists the installer CA in an endpoint- and
digest-bound private controller trust record before emitting that handle. PEM is
never serialized into the public bootstrap state. Because this boundary remains
draft, the canonical planned-state fixture was upgraded in place.

The native CLI can consume this handle through an explicit trust-store root.
It rejects missing, malformed, wrong-provider, or endpoint-mismatched records
before network access, and does not permit a trust handle to be mixed with an
explicit CA file. This changes no daemon wire-v1 request or response shape.

Avalonia connection profiles make the same trust-source choice. Handle-backed
profiles retain `bootstrap_trust_root` and `bootstrap_trust_handle`, revalidate
the private Rust record and exact endpoint before every connection, and import
the validated PEM into the existing content-addressed desktop CA store. They do
not persist PEM or replace the opaque handle with a cached certificate path.
This is a local profile-schema extension and does not change daemon wire v1.

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

Wire-v1 also accepts typed `runtime_register`,
`runtime_registration_update`, and `runtime_discovery_intake` commands. All
require `runtime.register`,
explicit confirmation, and bounded secret-free name, endpoint, and tag fields.
Create rejects an expected runtime revision; update requires the exact current
revision and emits a distinct update event. Unknown command fields fail closed,
so pairing or admin tokens cannot accidentally cross this domain boundary. The
daemon journals successful create and update commands, returns idempotent
results, schedules no external effect, and restores the updated projection
after restart. Canonical endpoint identity normalizes scheme/host case and
default HTTP(S) ports while retaining path/query identity. A conflict returns
`runtime_endpoint_conflict` with only the owning runtime ID. Discovery intake is
a distinct strict variant, requires the current revision, rejects an empty or
failed/raw observation, atomically applies validated capability and status
snapshots, and schedules no external effect.

When daemon IPC is configured, the 1.x Web registration route now inspects the
daemon revision, reconciles managed-only legacy registrations through create,
submits create/update and typed successful discovery intake, and commits its
managed compatibility projection only after those authoritative operations
succeed. The adapter preserves boolean Gewyvern extensions from the source
document rather than attempting to reconstruct them from the lossy legacy
capability list. Pairing/admin tokens and discovery error strings are never
written into these commands. Without daemon configuration, the existing managed
registration path remains the explicit development fallback.

Configured runtime list, detail, and status reads now consume the existing
typed `runtime_list` and `runtime_inspect` query results over the same private
IPC boundary. The C# decoder rejects unknown and incomplete fields recursively.
Daemon name, endpoint, tags, status, and observed capabilities are authoritative;
managed timestamps, sidecar metadata, and token-presence flags are overlaid only
for the legacy response contract. Managed-only runtimes remain visible during
reconciliation. A daemon-only runtime returns a typed 502 instead of receiving
invented compatibility metadata. Other Web operations continue to use managed
lookups until their attention/recovery contracts are migrated deliberately.

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

Prove the configured C# registration adapter and real daemon agree on create,
typed discovery intake, revision-inspected update, typed list/inspect readback,
and final projection:

```bash
cargo build --locked -p leserpentd --bin leserpentd
LESERPENT_TEST_DAEMON_BIN="$PWD/target/debug/leserpentd" \
  dotnet test apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj \
  --no-restore --filter \
  FullyQualifiedName~ConfiguredRustDaemonOwnsRegistrationDiscoveryAndUpdateEndToEnd
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
