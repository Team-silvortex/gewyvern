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

The host-deployment request remains separate from ordinary wire v1 and is not
accepted by `leserpentd` at `/v1/wire`. Authenticated HTTPS submits it only to
`POST /v1/bootstrap`; Unix IPC uses the explicit `bootstrap_v1` route rather
than decode-fallback protocol guessing. Submission is disabled unless the
daemon registered a native bootstrap adapter. A platform bootstrap adapter must
resolve the vault handle locally, install or reconcile the daemon, and return a
daemon-issued session handle. Authenticated wire v1 does expose two bounded
handoff-management operations for retained controller state: query by bootstrap
ID and confirmed bind by bootstrap ID. Bind carries no proof fields or secrets;
the server must derive proof itself. Runtime commands remain unauthorized until
the domain state is `session_bound`.

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

The root `gewyvern` binary now implements the separate
`gewyvern-install-v1` stdin/stdout entrypoint. It verifies its own bounded
artifact against the request digest before mutation, creates a private immutable
runtime generation, stores the API token and generated TLS key only in mode
`0600` files, retains only the token digest in its manifest, and atomically
publishes a mode `0600` `current` generation pointer. Existing path components,
generation files, manifests, service plans, and credentials are replay-checked
without following symbolic links. This preparation command always reports
`installed`. The separate `gewyvern-activate-v1` entrypoint may report `ready`
only after native service-manager activation and an authenticated TLS health
proof succeed.

The target installer also retains a mode `0600` native service descriptor in
each immutable generation: launchd plist on macOS and systemd unit on Linux.
The descriptor references the private token, TLS identity, database, and logs by
path and contains no token text. Replay compares it byte-for-byte with the
expected descriptor. Service-manager activation remains outside the preparation
entrypoint but uses the same installer wire v1 response. The target retains the
verified descriptor inside the generation; activation publishes it atomically to
the native manager directory and loads it only through `gewyvern-activate-v1`.

The target has a native launchctl/systemctl activation primitive, fenced by the
request-derived current generation and byte-identical retained/published
descriptors. It uses no shell and carries no token in manager arguments. The SSH
installer command invokes `gewyvern-activate-v1`, then runs a bounded loopback
health request whose TLS server name remains the requested endpoint host. The
response may claim `ready` only after generated-CA validation, API-token
authentication, strict JSON health decoding, and active service proof. Any
activation or health failure restores the prior current pointer and descriptor,
removes the failed new generation, and restarts the previous service. The
separate `gewyvern-install-v1` preparation command still returns only
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

## Draft Gewyvern Provisioning Boundary

Post-session Gewyvern installation remains separate from both ordinary wire v1
and debugging-pipeline `runtime.deploy`. Its strict schema-v1 envelope is capped
at 64 KiB and is accepted only by authenticated `POST /v1/provisioning` or the
explicit Unix IPC `provisioning_v1` route. Submission stays disabled unless the
daemon registry contains the dedicated `gewyvern.runtime.provision` adapter.

The adapter resolves only a validated `vault:ssh:<key>` installation handle and
never serializes the resolved secret. Its successful outcome carries
`service_ready`; a bounded fault carries `failed`. Daemon settlement verifies the
provisioning ID, runtime ID, target, planned revision, and retired credential. A
failed outcome atomically completes at `failed`; a successful outcome derives an
authority-owned proof and commits effect completion, the runtime registration
journal entry, and revision-3 `runtime_registered` checkpoint in one transaction.

`leserpent_protocol::gewyvern_installer` owns the separate internal installer
wire used by the host-key-pinned native SSH channel. Its strict request and
response are independently capped at 64 KiB. The request binds provisioning and
runtime IDs, HTTPS endpoint, artifact digest, install profile, API/trust handles,
and a redacted zeroizing API token. The response contains no token; it binds the
same identities and handles to `installed` or `ready`, generation digest, and a
digest-verified public CA. The shared readiness validator rejects `installed`,
identity drift, handle substitution, endpoint drift, and generation mismatch.
The adapter shares one native SSH safety substrate with daemon bootstrap: exact
SHA-256 host-key pinning, password authentication from a platform secret handle,
exclusive mode-`0700` SFTP staging, bounded stdout, and timeout cleanup. A strict
mode-`0600` `--gewyvern-provisioning-config` selects the artifact, target policy,
endpoint, and API/trust handles without containing secret text. A valid
`installed` response writes no controller trust and returns no service receipt.
A valid `ready` response must persist its endpoint-bound CA under the namespaced
`gewyvern-ca` handle before a receipt can leave the adapter. Target-side native
activation, authenticated health proof, and daemon-owned registration are
implemented. Runtime ID conflict is rejected before adapter dispatch, a lost
effect lease rolls registration and checkpoint writes back together, and legacy
revision-2 `service_ready` checkpoints promote through the same transaction after
restart. No public provisioning wire field was added for this authority handoff.

The native CLI consumes this unchanged draft envelope through a separate
`runtime provision` command. Execution requires `--yes`, an explicit stable
provisioning ID, target, and `vault:ssh:*` handle. Local execution selects only
the `provisioning_v1` IPC route; remote execution selects only authenticated
`POST /v1/provisioning`. Optional bounded polling resubmits the identical request,
so a network retry cannot silently create another installation. Human progress
omits credential handles, and terminal failure, protocol rejection, and polling
exhaustion use distinct process outcomes. `runtime.deploy` remains unchanged.

The draft retirement boundary is independent again. `runtime.retire` assigns a
new retirement ID and binds it to the original provisioning ID, runtime ID, SSH
target, principal, confirmation, and opaque `vault:ssh:*` handle. Its strict
schema-v1 request/response envelope is capped at 64 KiB and rejects unknown or
raw-secret fields. The ordered state is `planned`, `retiring_service`,
`service_retired`, then `runtime_unregistered`; `failed` is terminal. A runtime
cannot become unregistered before a matching receipt proves that the service was
retired. External failure removes the retirement handle from retained state but
keeps the runtime registered, avoiding an unmanaged still-live service. Runtime
schema 13 durably checkpoints this authority and atomically completes a leased
effect, journals replayable unregistration, and commits `runtime_unregistered`;
lost leases preserve the registration. Adapter-gated daemon submission and
worker settlement now enforce live-registration preflight and reject forged or
non-terminal outcomes. The strict internal `gewyvern-retire-v1` request binds
retirement/provisioning/runtime/profile; its host-key-pinned SSH adapter accepts
only the matching service-retired receipt. Authenticated daemon submission uses
only `POST /v1/retirement` over HTTPS or the explicit `retirement_v1` Unix IPC
route. Both preserve the independent 64 KiB retirement bound and typed response,
authenticate before submission, and remain disabled unless the daemon registry
owns `gewyvern.runtime.retire`. An unavailable route creates no retirement
checkpoint. The native CLI consumes this unchanged envelope through confirmed
`runtime retire` over IPC or HTTPS. Bounded polling reuses the same retirement
ID and request, human output omits the credential handle, and protocol failure,
terminal retirement failure, and wait exhaustion remain distinguishable.
The Avalonia client consumes the same envelope over HTTPS with strict identity
validation, explicit confirmation, bounded exact-request replay, and
credential-free status projection; no desktop-only wire variant exists.
The first physical Linux proof retains the same wire-v1 identities across real
host-key-pinned SSH provisioning, forged-authority rejection, retirement, and
idempotent replay. Its redacted fixture is
`docs/fixtures/leserpent_real_ssh_retirement_20260723.json`.

The Linux physical-host proof additionally confirms that these draft bootstrap
states preserve their wire-v1 meaning across real SSH deployment: trust failure
and timeout return `failed` without authority handles, successful deployment
returns `bootstrapped` with mutation disabled, and only a matching session proof
produces `session_bound`. Target health failure rolls back local service
publication and never emits a ready installer response. No daemon wire-v1 field
was added for this proof.

The production daemon worker now treats a successful bootstrap effect as a
typed state transition rather than an opaque scheduler result. Submission
atomically commits the effect and revision-1 `planned` checkpoint, making it
immediately inspectable and preventing effect retention from reopening an old
bootstrap identity. The worker atomically advances that checkpoint and its
validated outcome to revision 2 `bootstrapped` or `failed`; legacy internal
direct enqueue remains revision-1 compatible. The checkpoint contains only the public state plus its opaque
bootstrap vault handle; it contains no password, token, private key, or CA PEM.
After restart, the runtime reconstructs the domain state, keeps mutation denied
while it is `bootstrapped`, rejects mismatched daemon/session/trust proof without
advancing the checkpoint revision, and atomically promotes matching proof to
`session_bound`. Promotion removes the bootstrap handle from the current
checkpoint. The authenticated handoff query returns only the public snapshot.
The confirmed bind request contains principal, `host.bootstrap`, bootstrap ID,
and confirmation, but cannot carry `authority_owned`, daemon identity, handles,
tokens, or PEM. `NativeBootstrapSessionVerifier` resolves the retained session
and trust handles through server-owned stores, requires an exact trust endpoint,
and performs its own remote TLS/token health request before constructing the
internal proof. Verifier injection is default-off; `--bootstrap-trust-root`
enables it with the platform secret store. The native CLI exposes the complete
controller sequence as `bootstrap deploy ... --yes`, `bootstrap inspect`, and
`bootstrap bind ... --yes`. Deploy accepts only a target plus `vault:ssh:*`
handle and cannot be lowered into ordinary wire v1.

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
a distinct strict variant, requires the current revision, rejects an empty
observation, atomically applies validated capability, status, and sidecar
snapshots, and schedules no external effect. A sidecar failure is accepted only
as the stable `sidecar_fetch_failed` posture. A runtime-status failure is
accepted only as `runtime_status_fetch_failed` with all observed-fact fields
cleared. Raw failure text is rejected, and the same status validator is used by
direct intake and scheduler observation completion.

When daemon IPC is configured, the 1.x Web registration route now inspects the
daemon revision, reconciles managed-only legacy registrations through create,
submits create/update and typed successful discovery intake, and commits its
managed compatibility projection only after those authoritative operations
succeed. The adapter preserves boolean Gewyvern extensions from the source
document rather than attempting to reconstruct them from the lossy legacy
capability list. Pairing/admin tokens and discovery error strings are never
written into these commands. Without daemon configuration, the existing managed
registration path remains the explicit development fallback. Configured
individual, recovery, Fleet, and Orchestra refreshes compose available
capability, runtime-status, and sidecar observations into the same daemon intake
before updating their managed compatibility responses.

Configured runtime list, detail, and status reads now consume the existing
typed `runtime_list` and `runtime_inspect` query results over the same private
IPC boundary. The C# decoder rejects unknown and incomplete fields recursively.
Daemon name, endpoint, secret-free sidecar endpoint, tags, status, and observed
capabilities are authoritative. Journal-derived registration/update timestamps
are also authoritative when present. Sidecar status and its bounded memory-slot
summary are authoritative when present and survive journal replay. Legacy
projections without timestamps or sidecar status retain per-field managed
fallbacks, while token-presence flags intentionally remain local to the secret
boundary.
Managed-only runtimes remain visible during reconciliation. A daemon-only
runtime returns a typed 502 instead of receiving invented compatibility
metadata.

Wire-v1 now gives cleanup and generic deletion their own `runtime_unregister`
mutation instead of abusing the projection-bearing command result. The request
requires `runtime.unregister`, explicit confirmation, one command ID, and 1
through 128 unique runtime/revision targets. Leserpentd validates every revision
before mutation, then journals all removals, deletes their Orchestra history,
and stores the replay record in one schema-v14 transaction. An exact retry
returns the original counts with `replayed=true`; command-ID drift fails with an
idempotency conflict. The configured Web adapter reserves the targets before
the daemon call, blocking new sessions and Orchestra runs until daemon-first
unregistration and managed compatibility cleanup finish. Without daemon
configuration, the reservation still protects the existing development
fallback.

Wire-v1 also exposes `runtime_unregistration_receipt` as a read-only recovery
operation. It requires `runtime.read` and one command ID and accepts no targets
or confirmation. The response always carries the command ID, an optional
validated receipt, and the replay horizon observed by the same authority
transaction. `receipt: null` is a normal bounded miss; storage or tombstone
corruption returns only the fixed lookup failure. A present receipt has a
required nonzero operation generation and no `replayed` flag because the lookup
does not execute the mutation.

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

After the private local authority has claimed a writer generation, deployment
commands and Orchestra persist/delete/checkpoint mutations require the exact
authenticated IPC `writer_fence`. The fence is validated before effect enqueue
or SQLite mutation; stale and missing writers receive fixed protocol errors.
Deployment receipts, Orchestra history, and replay-horizon reads remain
read-only and do not require a current writer ticket.

The explicit Unix IPC `bootstrap_v1`, `provisioning_v1`, `retirement_v1`, and
`daemon_retirement_v1` mutation routes use the same outer authenticated
`writer_fence`. They validate it before decoding or submitting their independent
protocol envelopes, while each route still returns its own typed error
envelope. The native CLI accepts the owner-issued ticket only through paired
writer ID and generation environment fields and never performs an implicit
authority claim.

Authenticated HTTPS uses the canonical
`X-Leserpent-Authority-Writer-Id` and
`X-Leserpent-Authority-Writer-Generation` headers instead of changing any
stable wire-v1 payload. The headers must be unique, paired, and contain a
positive decimal generation plus the exact 32-character hexadecimal writer
identity. Validation happens only after Bearer authentication. Wire mutations
and the dedicated bootstrap, provisioning, runtime-retirement, and
daemon-retirement routes enforce the ticket; wire reads and
`/v1/leselang-export` remain unfenced.

Wire mutation classification is exhaustive over every protocol request and
domain command variant. Runtime status refresh, capability refresh, and
bootstrap session binding now require the same generation ticket; debugger
cancel remains delegated to the Leselang VM and is rejected by this control
runtime. Contract tests source-scan the C# non-read endpoint set and Rust HTTPS
route table, preventing an added route from silently escaping the canonical
mutation inventory. A real multi-process daemon test retains the stable wire-v1
payload while proving durable cold generation takeover and stale refresh
rejection.

The unchanged claim payload also has deterministic unclean-commit proof. A
test-only process executes the production runtime claim while an external
SQLite reader holds its FULL-synchronous DELETE-journal commit, then receives
`SIGKILL`. Recovery exposes only the complete previous generation; after the
natural owner lease expires, the attempted writer receives the next generation
as a non-replay. Killing a second claimant after commit but before cleanup
retains the complete committed generation. This adds no protocol field,
production crash switch, or alternate writer-claim endpoint.

The unchanged payload is also sufficient when a successful response is never
decoded. A production daemon IPC test commits writer A, leaves that response
unread, and starts same-A retry plus competing-B claim from independent clients
through one barrier. It accepts only the two transaction-serial orders: A
replays before B advances, or B advances before A becomes a new non-replayed
takeover. The maximal generation is the only ticket admitted for a real
mutation, and its subsequent same-ID replay is stable. No request identifier or
wire field is added.

The same unchanged payload survives a cold process boundary. Writer B
generation `2` is committed with its response left unread, the daemon exits,
and a fresh daemon opens the same database. B/`2` replays before a queued writer
C claim advances once to `3`; only C/`3` can perform the following mutation. A
third daemon then replays C/`3`. This proves durable response-loss recovery
without adding request identity, startup gating, or a new claim route.

Response loss can now be combined with unclean daemon termination. After B/`2`
commits without caller decode, `SIGKILL` leaves both the durable writer row and
the configured Unix socket. A replacement cannot remove that path before owner
lease expiry. After natural expiry, strict same-UID `0600` socket validation
rejects live listeners, insecure sockets, and non-socket paths, revalidates
mode/device/inode, and safely binds the same name. B/`2` then replays, C advances once to `3`, and only C/`3` can
perform the following mutation. The wire payload remains unchanged.

The production path also survives two complete unclean cycles without changing
the payload. Unread B/`2` and A/`4` claims are each followed by daemon
`SIGKILL`, pre-expiry rejection, natural lease expiry, same-socket recovery, and
same-ID replay. Competitors allocate only C/`3` and B/`5`; older tickets cannot
mutate, while final B/`5` mutates and replays. This retains contiguous durable
generation allocation without a response journal or request ID.

Post-recovery admission is bounded at the production daemon's 64-connection
IPC batch limit. After unread B/`2`, `SIGKILL`, natural owner expiry, same-path
socket recovery, and stable B/`2` replay, 64 distinct claimants start through
one barrier. Their arrival order is not part of the wire contract, but all
responses complete within 5000 ms and allocate every generation from `3`
through `66` exactly once without false replay. Only generation `66` can mutate
and its same-ID claim remains a replay. This adds no payload field, request ID,
hot failover, or active-active authority.

A saturated duplicate-retry batch also preserves the unchanged claim payload.
Sixteen groups queue one complete new claim whose client read half is closed,
then three readable retries for the same writer ID. Failed response delivery
does not roll back the primary claim or terminate the daemon peer loop: all 48
followers replay, only the 16 primaries allocate generations `3` through `18`,
and the final identity alone can mutate and replay. The accept gate makes the
64 claims cross a production tick boundary without adding a response journal
or changing idempotency semantics.

Hostile local peers cannot multiply the batch read timeout. The daemon reads up
to 64 accepted frames concurrently under the existing 2000 ms per-peer bound,
then dispatches completed frames serially in accept order. A mixed batch of 16
malformed frames, 16 wrong-token claims, 16 full-timeout slowloris prefixes,
and 16 valid claims gives fixed errors to complete invalid peers, no response
to timed-out peers, and generations only to valid claims (`3` through `18`).
This changes daemon scheduling only; the wire payload and authority
linearization contract remain unchanged.

Repeated hostile batches now share the daemon's cooperative shutdown boundary.
Two complete 64-peer mixed batches must each be followed by a fresh heartbeat
for the same SQLite owner token, and same-writer valid claims remain stable
replays without generation allocation. Frame readers retain the 2000 ms total
wall-clock peer deadline even for drip-fed bytes but check the process stop flag
every 100 ms; once it is set, no
later completed frame is dispatched. `SIGTERM` during 64 active incomplete
peers must therefore exit inside 1000 ms, release the owner row and Unix socket,
and allow immediate same-path restart with the existing generation replayed.
This changes neither the claim payload nor cold-takeover semantics. Contract
`1.14.0` records local executable proof and deliberately leaves physical Linux
x86_64 evidence pending.

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
cargo build --locked -p leserpentd --bin leserpentd --features native-ssh
LESERPENT_TEST_DAEMON_BIN="$PWD/target/debug/leserpentd" \
  dotnet test apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj \
  --no-restore --filter \
  FullyQualifiedName~ConfiguredRustDaemonExecutesTheDeploymentEndToEnd
```

Prove the configured C# registration adapter and real daemon agree on create,
typed discovery intake, revision-inspected update, typed list/inspect readback,
and final projection:

```bash
cargo build --locked -p leserpentd --bin leserpentd --features native-ssh
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
