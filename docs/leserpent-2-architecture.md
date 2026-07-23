# Leserpent 2.0 Architecture

This document is the authoritative target architecture for the
`1.0.0 -> 2.0.0` Leserpent line. It describes intended behavior, not the
current 1.x implementation. Delivery order and exit gates live in the
[Leserpent 2.0 roadmap](leserpent-2-roadmap.md).

## Decision

Leserpent 2.0 is a Rust control runtime with three equivalent operator
frontends:

- Leselang programs, including model-generated programs
- the native `leserpent` CLI
- graphical clients, beginning with Avalonia
- browser clients written in TypeScript

The platform stack constraint is explicit:

- Rust owns all control-plane, policy, and semantic decisions.
- C# and TypeScript frontends consume protocol projections and emit typed user
  actions but do not own control-plane decisions.
- No Node.js, Python, shell, or other additional runtime language is allowed to
  carry new control-plane authority.

The current ASP.NET and TypeScript application remains the 1.x implementation
and migration bridge. It is not the semantic center of the 2.0 system.

## Non-Negotiable Invariants

1. GUI, CLI, and Leselang submit the same versioned `CommandEnvelope` and read
   the same versioned `Query` projections.
2. No frontend owns a control-plane capability that the other frontends cannot
   express.
3. Equivalent requests under the same identity, capability set, revision, and
   input state have equivalent effects regardless of their origin.
4. Avalonia view models contain presentation logic only. They cannot reach
   Gewyvern, persistence, deployment, or orchestration adapters directly.
5. Leselang has synchronous source semantics. Asynchrony remains an internal
   runtime implementation detail.
6. External effects are typed, journaled, bounded, cancellable, and resumable.
7. A model may propose Leselang but cannot bypass parsing, type checking,
   capabilities, confirmation policy, or effect limits.
8. UI-local state such as geometry, focus, and animation may remain
   frontend-specific. Domain state and operator actions may not.
9. No control-plane decision path may be introduced in a frontend language.
   Rust owns policy, authorization, idempotency, revision, and effect
   semantics.

These rules define atomic replaceability. Replacing GUI interaction with CLI or
Leselang may change presentation and transport, but not control semantics.

The executable proof for this rule is
`gewyvern_validate leserpent-parity-recovery`. It compares the current
CLI and Leselang command/query lowering against the same domain contract, then
exercises capability, confirmation, revision, principal-scoped idempotency,
continuation restart, lease fencing, snapshot fallback, worker-crash
settlement, outbox repair, Avalonia reconnect/cache state, and the real
Rust-to-.NET WebSocket path. Each Cargo suite must report a nonzero minimum test
count, the xUnit suite must emit one internally consistent nonzero success
summary, and external conformance runners must emit exactly one declared
success marker, preventing cfg, filter, or adapter drift from turning the proof
into a vacuous pass. Each summary binds the result to bounded kernel and
toolchain provenance and removes stale success metadata before execution.

## System Shape

```mermaid
flowchart TD
    M["Model"] --> L["Leselang source"]
    U["Operator"] --> C["Rust CLI"]
    U --> A["Avalonia client"]
    W["Web client"] --> P["Leserpent protocol"]
    L --> F["Leselang frontend"]
    C --> N["Command and query normalization"]
    A --> P
    F --> N
    P --> N
    N --> R["Rust Leserpent runtime"]
    R --> J["Journal and projections"]
    R --> E["Typed effect adapters"]
    E --> G["Gewyvern runtimes"]
    J --> P
```

The Rust runtime is authoritative. Frontends are replaceable projections and
intent producers.

## Instance Topology

Leserpent 2.0 controls instances with a strict three-level topology:

- each Linux kernel/container instance maps to exactly one `gewyvern` service
- each `gewyvern` instance is registered under one `leserpentd` instance
- each `leserpentd` instance is exposed as one authenticated web service

```mermaid
flowchart LR
    subgraph K["Kernel/Container Instance"]
      K1["Instance A"]
      K2["Instance B"]
      K3["Instance C"]
    end

    subgraph G["Each instance owns one gewyvern runtime service"]
      G1["gewyvern A"]:::runtime
      G2["gewyvern B"]:::runtime
      G3["gewyvern C"]:::runtime
    end

    K1 --> G1
    K2 --> G2
    K3 --> G3

    subgraph D["leserpentd host"]
      D1["Leserpentd instance"]
      WD["/v1/wire"]
      WS["/v1/events"]
      D1 --- WD
      D1 --- WS
    end

    G1 --> D1
    G2 --> D1
    G3 --> D1

    A["Avalonia / CLI / Leselang"]
    A --> WD
    A --> WS

    classDef runtime fill:#1f2937,color:#fff,stroke:#9ca3af;
```

The model implies identity is two-dimensional at minimum:

- `daemon_id`: owner of execution, storage lease, and security posture
- `runtime_id`: immutable target identity inside that daemon

The effective control key is the ordered pair `(daemon_id, runtime_id)`. In the
current 2.0 protocol, `daemon_id` is implicit in the authenticated transport
session (service endpoint), while `runtime_id` remains explicit in the command/query
payload.
A command touching a runtime is always bound to one pair and one expected
revision; commands with a malformed `runtime_id` or mismatched session identity are
rejected before policy checks.

This is not multi-tenancy by namespace alone. It is explicit composition:
one daemon can safely host many runtimes, but commands can never fan out to
unknown runtimes because no wildcard path exists in the domain contract.

## Client As Hub / Per-Daemon Multi-Window Model

The native client entry is a daemon hub, not a single direct remote target form:

- startup presents saved `leserpentd` connection profiles;
- each profile opens a logical **daemon session**;
- each daemon session exposes fleet cards from its own `runtime` projection;
- each runtime card can open a child workspace in the same process;
- child workspaces route through that daemon session only.

A single Avalonia window can thus operate several daemon sessions in parallel,
without collapsing trust, event stream, or revision history across sessions.

In this model:

- `desktop setup` and `setup window` are process-wide shell tasks;
- `runtime list`, `runtime refresh`, `runtime inspect`, deployment, and log
  queries are session-scoped;
- session closure cancels only that session's subscriptions and leaves other
  daemon sessions running.

## Topology Error Boundaries

Topology transition rules are simple and explicit:

1. `Unmanaged` -> `Registered`: durable `RuntimeRegister` with bounded identity and origin
2. `Registered` -> `Connected`: successful `runtime health`/adapter refresh
3. `Connected` -> `BoundedError`: adapter faults, credential error, or probe timeout
4. `BoundedError` -> `Ready`: successful refresh after explicit user/plan confirmation
5. `Connected` -> `Removed`: explicit removal command with idempotent identity and dry-run

Every transition writes an event. Every transition failure keeps the old projection
and returns a typed ambiguous outcome when recovery depends on freshness.
No transition writes hidden side effects before command persistence.

## Reverse Deployment Model

The intended deployment control loop is reverse-first:

1. `operator credential` reaches a target host with bootstrap rights.
2. Leserpent uses that credential to deploy/refresh `leserpentd` on the target.
3. Leserpent connects to the target's `leserpentd` service endpoint (`/v1/wire`, `/v1/events`) using the target-issued session token or certificate chain.
4. The `leserpentd` session exposes a fleet panel and accepted runtime identities.
5. A confirmed `runtime.provision` intent installs or reconciles `gewyvern`, proves its authenticated service identity, and registers that runtime with the daemon.
6. Later `runtime.deploy` intents submit debugging pipelines to that already registered Gewyvern runtime; they never install a runtime or mutate host service configuration.

This makes `leserpent` responsible for control-plane bootstrap and orchestration,
while `leserpentd` remains the per-host control gate for later state and actions.

```text
Leserpent (operator)
  --bootstrap token--> host bootstrap endpoint
  --leserpentd-token/ca--> leserpentd@host (panel + wire session)
  --runtime.provision--> install + attest + register gewyvern
  --runtime.deploy--> submit pipelines to registered gewyvern
```

Four credential layers are therefore explicit:

- **bootstrap credential**: creates/reconciles a managed `leserpentd` on the target
- **session credential**: binds the `leserpentd` session and all mutation/effect calls
- **runtime installation credential**: creates/reconciles Gewyvern and is retired as soon as the authenticated service is ready
- **runtime API/trust handles**: bind later adapter calls to the attested Gewyvern endpoint without putting secrets in runtime metadata

The bootstrap credential must never become implicit session authority. A `leserpentd`
session token is checked for every mutation and workspace command, and a failed
session check must block both control and deployment operations before adapter dispatch.

The host and runtime deployment state machines are deliberately separate.
`leserpent-domain::provisioning` owns the first runtime-provisioning contract:
`Planned -> Installing -> ServiceReady -> RuntimeRegistered`, with a terminal
`Failed` branch. `ServiceReady` retires the installation credential before
registration. `RuntimeRegistered` requires a proof bound to the provisioning ID,
runtime ID, HTTPS endpoint, API/trust handles, authority ownership, and protocol
version. `leserpent-protocol::provisioning` carries this state in an independent,
strict, 64 KiB-bounded envelope that rejects unknown and raw credential fields.
`leserpentd` now accepts this envelope only through authenticated
`POST /v1/provisioning` or the explicit `provisioning_v1` IPC route, and only when
the dedicated `gewyvern.runtime.provision` adapter is registered. The adapter
resolves the installation credential locally, returns only `ServiceReady` or
`Failed`, and daemon settlement rejects identity drift before atomically advancing
the checkpoint. The separate `leserpent-protocol::gewyvern_installer` wire now
binds the internal installer exchange to provisioning/runtime identity, HTTPS
endpoint, artifact generation, API/trust handles, a zeroizing API token, and a
digest-verified public CA. Its readiness validator refuses merely `Installed`
services and all request/response identity drift. The native
`gewyvern-install-v1` target entrypoint verifies its own artifact digest, creates
a private immutable runtime generation, writes secret-bearing files with mode
`0600`, generates the endpoint TLS identity, replay-checks every retained
manifest and service-plan identity, and atomically advances a non-symlink
`current` pointer. It deliberately returns only `Installed`.
`gewyvern-activate-v1` additionally publishes and activates the retained native
launchd/systemd descriptor, while `gewyvern-service-v1` starts the managed
rustls API from generation-confined paths. `Ready` is emitted only after a
bounded loopback probe validates the requested endpoint name, generated CA,
private API token, JSON health response, and active service. Activation or
health failure restores the previous `current` pointer and descriptor, removes
the failed new generation, and restarts the previous service. Native SSH
transport now reuses the same host-key-pinned Rust substrate as daemon bootstrap:
exclusive private SFTP staging, bounded command output, timeout cleanup, and no
shell script or secret argument. A strict mode-`0600` daemon origin configuration
binds each target to its artifact, endpoint, API/trust handles, and platform
secret service. A valid `Installed` response persists no trust and yields no
service receipt; a valid `Ready` response must persist its endpoint-bound CA in
the namespaced controller trust store before returning a receipt. Daemon
settlement derives the registration proof itself and commits effect completion,
the runtime registration journal entry, and revision-3 `RuntimeRegistered`
checkpoint in one immediate SQLite transaction. It updates the in-memory
projection only after commit. A lost lease rolls all three durable writes back,
and an existing revision-2 `ServiceReady` checkpoint is promoted safely after
restart. The native CLI now exposes a separate `runtime provision` command with
explicit confirmation, opaque SSH handles, operator-supplied idempotent
provisioning IDs, authenticated IPC/HTTPS transport, bounded phase polling, and
distinct protocol-failure, provisioning-failure, and wait-exhaustion exit codes.
It never aliases this operation to `runtime.deploy`, and repeated polling reuses
the exact provisioning identity rather than creating a second install attempt.
The Avalonia Hub exposes the same route through an authority-scoped native
workspace. It requires explicit confirmation, accepts only a `vault:ssh:*`
installation handle, locks provisioning/runtime/target identity after submit,
and performs at most 30 automatic observations before yielding to an explicit
same-attempt refresh. Terminal failure directs the operator to correct the cause
and choose a new provisioning ID, preserving the failed identity for audit.
Desktop Local Orchestra is offered as an owning authority only when the private
`LESERPENT_GEWYVERN_PROVISIONING_CONFIG` path is present at app startup; otherwise
only saved authenticated daemon authorities are listed.
Remote compensation/retirement for an already registered service remains to be
productized. Its first independent contract now lives in
`leserpent-domain::retirement` and `leserpent-protocol::retirement`; it does not
extend `runtime.deploy` or reuse the provisioning ID as an operation identity.
`runtime.retire` binds a new retirement ID to the original provisioning ID,
runtime ID, SSH target, operator principal, explicit confirmation, and a newly
supplied opaque `vault:ssh:*` handle. Its state machine is
`Planned -> RetiringService -> ServiceRetired -> RuntimeUnregistered`, with a
terminal `Failed` branch. Runtime unregistration is forbidden until a fully
identity-bound receipt proves service retirement. External failure retires the
submitted SSH handle but deliberately preserves the runtime registration,
preventing a still-running service from becoming an untracked orphan. The strict
64 KiB wire rejects unknown/raw-secret fields and validates this ordering on all
responses. Runtime SQLite schema 13 now stores retirement authority alongside
the other kind-scoped checkpoints. After `ServiceRetired`, one immediate
transaction completes the leased effect, journals runtime unregistration, and
commits revision-3 `RuntimeUnregistered`; restart replays the removal, while a
lost lease rolls back all three and leaves the runtime registered. The daemon
now accepts retirement only when its adapter registry owns the dedicated effect
kind, rechecks live registration before dispatch, and rejects malformed,
non-terminal, or identity-confused adapter output before invoking that
transaction. The adapter resolves the opaque SSH handle only at its transport
boundary and emits no secret material. The production origin now registers a
second host-key-pinned native SSH adapter using the same runtime-scoped policy
and validated artifact. Its strict internal wire invokes
`gewyvern-retire-v1`, whose manifest check binds provisioning/runtime/profile
before mutation. A private `retiring -> service_retired -> retired` marker makes
power-loss recovery identity-fenced; stop/disable precedes descriptor and
runtime-root removal, neighboring runtimes are untouched, and relaxed authority
permissions fail before service mutation. Authenticated operator routes now
begin at an explicit `retirement_v1` Unix IPC route and authenticated
`POST /v1/retirement`. Both retain the retirement protocol's independent 64 KiB
bound and typed error envelope; bad authentication is rejected before
submission, and a daemon without the production retirement effect adapter
returns `retirement_unavailable` without creating a checkpoint. The main process
enables these submission gates only after registry ownership of
`gewyvern.runtime.retire` is established. CLI/Avalonia controls and physical
Linux retirement evidence remain the next implementation slice.

The runtime persistence layer now supplies that contract with shared durable
ground. Schema 12 migrated schema-11 `bootstrap_handoffs` rows into the
kind-scoped `authority_checkpoints` table, where daemon bootstrap and Gewyvern
provisioning reuse one transaction/CAS implementation without sharing identities
or phase vocabularies. Provisioning submission atomically stores its revision-1
`Planned` checkpoint with the effect and rejects an already registered runtime ID
before adapter dispatch. A failed adapter outcome advances to `Failed`; a
successful Ready receipt is identity-checked and atomically materializes effect
completion, runtime registration, and the revision-3 `RuntimeRegistered`
checkpoint. Restart restores the resulting projection and never restores the
retired installation credential. Legacy revision-2 Ready checkpoints are
promoted through the same registration transaction. Schema-11 daemon checkpoints
migrate losslessly. Schema 13 extends the same table and journal vocabulary for
retirement without changing existing bootstrap or provisioning records.

The first native contract now lives in `leserpent-domain::bootstrap` and
`leserpent-protocol::bootstrap`. It models
`Planned -> Deploying -> Bootstrapped -> SessionBound` independently from the
ordinary daemon command envelope. Only `SessionBound` authorizes later runtime
mutation. Bootstrap credentials cross the domain as validated
`vault:<provider>:<key>` handles, are retired after session binding, and cannot
be substituted for the daemon-issued session handle. The separate 64 KiB
bootstrap wire-v1 envelope rejects raw credential fields and unknown fields.
Host adapters and client workflows remain outside this semantic kernel.

The first host adapter now lives in `leserpent-adapters::bootstrap`. Its
`NativeSshBootstrapTransport` uses Rust SSH and SFTP libraries directly rather
than launching `ssh`, a shell script, Node, or Python. Each host is admitted by
an exact target policy containing a pinned SHA-256 host-key fingerprint,
expected daemon identity, HTTPS origin, install profile, and a separate
`vault:leserpentd:*` session handle. The adapter resolves the SSH bootstrap
password and daemon session token independently, uploads a bounded native
installer as mode `0700`, reads it back to verify SHA-256, and sends the install
request over a bounded zeroized stdin buffer. The typed response must echo the
bootstrap, daemon, and endpoint identities before the domain may enter
`Bootstrapped`; it still cannot authorize mutation until session proof binds.
The network driver is isolated behind the `leserpent-adapters/native-ssh`
feature so ordinary `leserpentd` builds retain the policy adapter without
linking an unused SSH client stack.

The target-side `bootstrap-install-v1` executable contract now shares a strict
64 KiB installer wire with the SSH adapter. It verifies the running artifact's
SHA-256, writes an immutable private generation containing the executable,
session token, and non-secret manifest, then atomically commits a `current`
generation marker. Existing generations are verified before replay; token or
identity conflicts preserve the old marker. The native process test executes
the actual `leserpentd` entry point in an isolated home and proves private,
idempotent installation without token output.

Installer responses distinguish `installed` from `ready`. The SSH adapter
rejects `installed`, so copied files can never be mistaken for a live daemon.
Native launchd/systemd publication and activation plus an authenticated endpoint
health proof must complete before the installer may emit `ready`; those service
layers and the real SSH host proof remain the next boundary.

The generation now also owns a target-generated self-signed TLS identity for
the requested endpoint SAN. Both certificate and private key remain in the
private generation, while the installer response returns only the public CA PEM
and its SHA-256. The installer protocol verifies that digest against the PEM
contents. `leserpentd` accepts the session secret through
`--remote-token-file`; the file must be bounded, regular, non-symlink, and deny
group/other access, so platform service descriptors do not need to expose the
token in arguments or environment variables. Each immutable generation now
retains a mode `0600` launchd plist on macOS or systemd unit on Linux. It points
at generation-owned executable, certificate, key, and token files plus private
state/log directories; replay verifies the complete descriptor before accepting
the generation. The installer atomically publishes the verified descriptor to
the target profile's LaunchAgents/LaunchDaemons or systemd unit directory before
advancing `current`; symlinked service directories fail closed. It does not load
or start that descriptor yet. Controller-side CA persistence is also still
required before the service can become authoritative.

The target code also owns a native activation primitive. Before invoking a
service manager it rebinds the request-derived generation to `current` and
requires the retained and published descriptors to match byte-for-byte. It then
uses absolute launchctl/systemctl paths and argument arrays without a shell or
secret-bearing arguments. `bootstrap-install-v1` remains the side-effect-bounded
prepare/publish path. The SSH transport invokes `bootstrap-activate-v1`, which
composes preparation, platform activation, and a bounded health probe. The probe
connects to the target loopback port while validating TLS against the requested
endpoint host name and generated CA, authenticates with the private session
token, strictly decodes wire v1, and requires `ready` plus daemon-owned authority.
Only then may the installer response claim `ready`.

The controller persists the returned CA before accepting that ready outcome.
`FileBootstrapTrustStore` validates the endpoint, parses the CA as a rustls root,
rechecks its SHA-256, and writes an endpoint-bound record through a private
`0700` directory and atomic `0600` file replacement. Unix reads and writes use
`O_NOFOLLOW` and opened-file metadata. Domain snapshots carry only a
`vault:leserpent-ca:*` trust handle; PEM never enters the bootstrap response
state. Trust persistence failure converts the operation to `Failed` and withholds
both session and trust authority handles.

The native CLI connection path now consumes that handle directly through
`--remote-trust-root` and `--remote-trust-handle`. It loads and revalidates the
private record, rejects any endpoint mismatch, and feeds the retained PEM to the
same rustls client used by `--remote-ca`. File and handle trust sources are
mutually exclusive. Avalonia profiles implement the same choice with either a
managed CA path or `bootstrap_trust_root` plus `bootstrap_trust_handle`. The
renderer-independent remote client strictly decodes the Rust trust record,
requires private directory/file modes, rechecks endpoint and PEM digest, and
passes the retained PEM into the existing content-addressed desktop CA store.
The profile retains the opaque handle rather than replacing it with a cached CA
path, so every connection and topology refresh re-enters the authority binding.

The first physical-host vertical now exercises this complete path against an
x86_64 Linux host through `NativeSshBootstrapTransport`: pinned host key,
password authentication, SFTP upload/readback, systemd-user activation,
endpoint-name TLS, token authentication, private controller trust persistence,
and the Bootstrapped-to-SessionBound mutation fence. A trust-store rejection
withholds all authority handles. A one-millisecond transport deadline also
withholds authority and reconnects through the same pinned SSH/SFTP boundary to
remove the partial staging artifact.

The controller-side handoff is now part of the production daemon/runtime path,
not a test-only reconstruction. `DaemonHost` strictly decodes completed host
bootstrap effects and rejects malformed, mismatched, or non-terminal outcomes.
Runtime SQLite schema 13 then commits the scheduler outcome and a validated
private authority checkpoint atomically under the existing owner
lease. A restarted `ControlRuntime` restores `Bootstrapped` without mutation
authority. Session promotion re-enters the domain state machine, compares the
bootstrap, daemon, session, trust, authority, and protocol identities, and uses a
checkpoint revision CAS before publishing `SessionBound`. The bootstrap vault
handle is removed from the current checkpoint at that boundary; raw passwords,
tokens, CA PEM, and private keys never enter the journal.

This closes durable handoff recovery but not the product entrypoint. The
authenticated IPC/HTTPS wire now supports checkpoint query and confirmed
session bind by bootstrap ID. Bind deliberately accepts no proof fields. A
server-owned `BootstrapSessionVerifier` must resolve the retained session token
and CA record, require exact endpoint binding, and prove the target's TLS, token,
wire schema, readiness, and daemon authority before creating the internal proof.
Without a verifier the operation fails closed. The packaged daemon can enable
the native implementation with `--bootstrap-trust-root`; the Rust CLI consumes
the operations through `bootstrap inspect` and `bootstrap bind ... --yes`.

The packaged daemon now includes the feature-gated native SSH origin and can
register it through `--bootstrap-config` plus `--bootstrap-trust-root`. The
strict schema-v1 JSON contains only pinned host policy, opaque credential
handles, an absolute bounded artifact path, and a platform secret-service name;
unknown fields, raw passwords, unsafe paths, duplicate authority identities,
non-private configuration, and writable or non-executable artifacts fail
closed. The same secret provider and trust root back deployment and subsequent
session verification. The independent submission boundary is now live at HTTPS
`POST /v1/bootstrap` and the explicit Unix IPC `bootstrap_v1` route; it is not
accepted by ordinary `/v1/wire` and remains disabled without a registered
bootstrap adapter. Submission commits the effect and revision-1 `Planned`
checkpoint atomically, then worker settlement advances it to revision 2 before
session binding can advance it again. Both the Rust CLI and Avalonia Hub own the
full deploy/inspect/bind sequence. The Hub selects an existing authenticated
daemon authority, submits only a `vault:ssh:*` handle, polls the public handoff,
and keeps session binding disabled until the server publishes `Bootstrapped`.
Each operation re-resolves the authority from the connection catalog so a
replaced profile cannot inherit an open window's trust. Connection promotion
is now implemented for the locally managed authority: an explicit bootstrap
config enables its private trust root, `SessionBound` unlocks `Add to Hub`, and
the desktop resolves the Rust-compatible session handle, validates the
endpoint-bound CA record, proves target TLS/token health, and only then writes
the platform credential and secret-free catalog profile. A conflicting existing
credential fails before mutation, while catalog failure removes a newly written
token. Remote-source binding remains valid but intentionally cannot invent local
CA or session state from an inaccessible remote trust root.

The checked shape is
`docs/fixtures/leserpent-bootstrap-origin-v1.example.json`. Operators must copy
it to a canonical absolute path, replace the pinned fingerprint and identities,
set mode `0600`, provision the referenced SSH/session accounts in the named OS
secret service, and start the native-SSH daemon with both options. The artifact
may target another operating system or architecture and therefore is selected
explicitly rather than assuming that the controller executable can run on the
managed host.

Target activation is transactional around `current`, the published service
descriptor, and a newly created generation. Activation or authenticated health
failure restores the previous private files atomically, removes the failed
generation, and asks launchd/systemd to stop the failed unit and resume the
previous one when present. The physical-host negative proof occupies the target
port with the healthy primary daemon, deploys a second daemon with a different
token, observes installer rejection, and confirms that the failed unit and
generation are absent while the primary remains healthy with zero restarts.

The contract requires the operator to confirm deployment actions that cross from
host bootstrap into runtime mutation. The command graph is still single-source:
all panel actions and `runtime.deploy` intents share the same `CommandEnvelope` and
revision identity rules.

This topology contract is the intended semantic source for the statement you just
described:

- one kernel/container per `gewyvern`;
- one `leserpentd` managing many of them;
- one `leserpentd` service endpoint per host.

## Rust Workspace

The intended source ownership is:

| Crate | Responsibility |
| --- | --- |
| `leselang-syntax` | lexer, parser, lossless syntax tree, diagnostics |
| `leselang-hir` | names, types, effect declarations, validated functions |
| `leselang-vm` | stackless evaluator, continuation images, deterministic steps |
| `leselang-observe` | validated, sanitized VM/runtime projections for UI consumers |
| `leselang-command` | operation DSL lowering into `CommandPlan` |
| `leselang-ui` | pure UI DSL lowering into `UiDocument` and `UiPatch` |
| `leserpent-domain` | validated IDs, commands, queries, events, revisions, capabilities, bootstrap state, and plan authorization |
| `leserpent-runtime` | transactions, scheduling, policy, replay, projections |
| `leserpent-protocol` | IPC, HTTP, WebSocket, bootstrap wire, schema, compatibility, and shared transport safety |
| `leserpent-adapters` | typed Gewyvern health, status, deployment, discovery, and native secret-store integrations |
| `leserpent-cli` | native CLI parsing and rendering |
| `leserpentd` | local and remote runtime host |

Crates may initially be introduced behind one workspace package, but these
ownership boundaries must exist before frontend migration begins.

## Unified Intent Contract

All mutating entrypoints lower into one envelope:

```rust
pub struct CommandEnvelope {
    pub schema_version: u32,
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub expected_revision: Option<Revision>,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub origin: CommandOrigin,
    pub confirmation: Confirmation,
    pub dry_run: bool,
    pub command: Command,
}
```

`origin` is audit metadata. It must not select a different implementation.
Identity, capabilities, confirmation, and revision may change authorization,
but GUI origin alone may not grant authority.

Every command follows:

```text
decode -> validate -> authorize -> plan -> preview/confirm -> commit
       -> emit events -> update projections -> publish result
```

Queries read immutable projections. They never trigger hidden refreshes or
mutations. Explicit refresh is a command.

Runtime registration now has typed create and update authority slices.
`RuntimeRegister` requires `runtime.register`, explicit confirmation for an
applied command, a secret-free bounded name/endpoint/tag payload, no expected
runtime revision, and a principal-scoped idempotency key.
`RuntimeRegistrationUpdate` uses the same capability and payload bounds but
requires the exact current runtime revision. It changes registration metadata,
preserves operational status, advances the projection revision, and emits a
distinct update event. Neither command schedules a network effect. The durable
runtime journals both commands, restores them after restart, and exposes them
through authenticated IPC and HTTPS wire dispatch. The domain also enforces a
canonical endpoint identity compatible with 1.x: scheme and host case plus
default HTTP(S) ports are normalized, path and query remain significant, and a
conflict reports only the owning runtime ID.

`RuntimeDiscoveryIntake` is a separate revision-fenced command rather than an
additive field on either stable registration variant. It accepts validated
successful capability and/or status observations, rejects an empty intake,
updates both projections atomically, records the capability observation's input
revision, and emits a typed event without scheduling network effects. The 1.x
adapter retains arbitrary boolean capability extensions from the original
Gewyvern document, derives its legacy display capabilities separately, inspects
the daemon revision before update, reconciles a managed-only runtime through
create, and commits managed compatibility state only after the daemon accepts
registration plus discovery. Pairing tokens, admin tokens, discovery errors,
and raw adapter payloads do not cross the authority boundary. The managed path
remains only when daemon configuration is absent; daemon-backed read projection
migration is the next compatibility step.

That migration now covers the public runtime list, detail, and status reads.
The compatibility adapter strictly decodes daemon projections and rejects
unknown or incomplete nested fields. For reconciled runtimes, daemon identity,
endpoint, tags, status, and observed capability facts replace their managed
copies. Managed timestamps, sidecar metadata, and token-presence booleans remain
an explicit overlay because the Rust projection does not yet own those fields.
Managed-only legacy entries remain readable; daemon-only entries fail closed
rather than receiving fabricated compatibility metadata. Attention, cleanup,
protocol-reading, and recovery reads now use the shared read projection for
authoritative identity, endpoint, tags, status, and capabilities. Managed token
and sidecar metadata remains an explicit overlay until those facts move to an
authoritative durable source. Cleanup remains managed.

## Leselang Semantics

Leselang is a small functional language with synchronous source semantics:

- immutable values by default
- expressions and pattern matching
- pure functions unless their signature declares effects
- lexical capability visibility
- no promises, futures, callbacks, or `async/await` syntax
- structured `all`, `race`, `timeout`, `retry`, and `compensate`
  forms when concurrency or recovery is intentional

Evaluation runs until it completes, faults, yields, or requests an effect:

```rust
pub enum Step {
    Done(Value),
    Effect(EffectRequest),
    Yield(Continuation),
    Fault(Diagnostic),
}
```

An effect request contains a stable ID, capability requirement, deadline,
resource budget, input value, and continuation token. The runtime journals the
suspension before dispatch. Completion re-enters the continuation with a typed
`EffectResult`.

The VM must be stackless or trampolined. A suspended program cannot retain an
OS lock, database transaction, mutable borrow, socket, or host-language stack.
Continuation images are versioned data and can be rejected safely when their
schema is no longer compatible.

## Effect And Re-entry Rules

- A continuation token is single-consume.
- Duplicate effect completion is idempotent.
- Re-entry compares the expected state revision before committing.
- Cancellation and timeout are typed results, not host exceptions.
- Retry requires an explicit policy and a replay-safe effect.
- Loops have fuel, a deadline, or an external-event suspension point.
- Parallel branches merge in a deterministic declared order.
- Crash recovery reconstructs runnable work from the journal.
- Host adapters may use Rust async internally, but async types never enter
  Leselang or the domain contract.

## Adapter Secret Boundary

Rust effect targets carry validated `SecretKey` aliases, never secret values.
`SecretStore` resolves an alias immediately before network execution; missing,
invalid, or unavailable values fail before a connection is opened. Temporary
`SecretValue` instances redact `Debug` output, reject line breaks and oversized
values, and zeroize their allocation on drop. Adapter request buffers containing
authorization headers are also zeroized immediately after the socket write,
including write-failure paths.

The daemon supplies an allowlisted environment-backed store for the optional
Gewyvern admin token. An explicit secret alias instead resolves through the
native macOS Security.framework Keychain provider or Linux Secret Service;
selecting it never silently falls back to the environment. Linux loads
`libsecret-1.so.0` and `libglib-2.0.so.0` at runtime, so production hosts do not
need development packages or helper subprocesses. Configured in-memory storage
exists only as a provider and test boundary. Platform providers preserve target
and adapter semantics without adding platform code to the scheduler or domain
model.

Gewyvern targets expose two explicit transports. The existing HTTP constructor
accepts loopback socket addresses only. Remote targets require an
`https://HOST[:PORT]` origin, a regular non-symlink CA file bounded to 1 MiB,
and a secret alias; there is no HTTP or unauthenticated fallback. Rustls verifies
the DNS name or IP and negotiates HTTP/1.1. Both transports share strict,
bounded JSON response framing that rejects duplicate `Content-Length`, transfer
encoding, non-JSON content, truncation, and bytes beyond the declared body.

Deployment is a separate typed adapter capability, not a general HTTP or shell
escape hatch. `gewyvern.deployment.submit` accepts only a validated runtime ID,
idempotency key, pipeline kind, requester, explicit confirmation, and optional
target. It always posts to `/v1/deployments`, always requires the target secret,
and accepts only a matching `accepted` response. HTTP 200 is valid only for an
idempotent replay and HTTP 202 only for a new intent; echoed request fields must
match before the durable effect is completed.

The adapter is reached through the shared `runtime.deploy` operator command,
never by exposing the effect queue. Domain authorization uses the independent
`runtime.deploy` capability and requires confirmation for non-dry-run commands.
Leselang, CLI, and deterministic plan export share one lowering function. The
durable runtime derives requester, request ID, and confirmed state from the
command envelope, then materializes only the bounded runtime/pipeline/target
intent.

Avalonia remote workspaces adopt the same command boundary only when the strict
capability projection advertises authenticated deployment. UI IR declares a
bounded parameterized form with localized labels, required fields, maximum
lengths, and renderer-neutral input constraints. Avalonia generates controls
from that declaration and emits a typed `submit` event whose values are checked
again by the semantic renderer before mutation. Rust validates the same field
whitelist and constraints before lowering the event through the shared
`runtime.deploy` function. Runtime/revision context remains visible, mutation
fences cover success and ambiguous network outcomes, and principal, request
identity, capability, and confirmation are never editable fields.

Capability discovery is similarly target-scoped. The discovery adapter accepts
only a configured runtime ID and always reads `/v1/capabilities` from that
target; it has no subnet, broadcast, DNS enumeration, redirect, or target
creation surface. Core claims are typed, endpoint paths are bounded and
canonicalized, deployment claims must agree with the advertised endpoint, and
future extensions are accepted only as bounded boolean flags. The observation
omits the target origin and credentials. A shared revision-bound domain contract
validates the observation again before SQLite schema 9 atomically commits the
journal entry, effect completion, and updated runtime projection. Replay and
snapshot restore reproduce the same capability state; stale observations are
rejected without projection mutation.

Capability refresh is an operator command, not an effect-queue API. Leselang
`runtime.refresh_capabilities`, CLI `runtime refresh-capabilities`, and GUI
actions all lower to `RuntimeCapabilitiesRefresh` under the existing
`runtime.refresh` capability. Domain execution advances the runtime revision and
emits a typed event; only the durable runtime may translate that event into a
`gewyvern.capabilities.discover` request carrying the new expected revision.

Capability presentation consumes only the validated domain projection. Native
CLI inspect and renderer-neutral runtime workspaces distinguish unobserved from
observed state and expose only service/version, typed core flags, canonical
endpoint paths, and bounded boolean extensions. They do not receive or render
the configured target origin, secret alias, authorization header, or raw
adapter response.

The authenticated HTTPS vertical exercises this boundary without a mock wire
shortcut. A separate CLI process submits `RuntimeCapabilitiesRefresh`; the
daemon commits its revision, materializes the durable discovery task, executes
the real target-scoped HTTP adapter, atomically commits the observation, and
serves the resulting projection to a later CLI inspect. The proof asserts every
revision transition and verifies that the adapter's network origin is absent
from human output.

## UI Contract

Leselang UI functions are pure:

```text
State -> UiDocument
UiEvent -> CommandPlan
previous UiDocument + next UiDocument -> UiPatch
```

`UiDocument` uses stable node IDs, typed properties, bounded collections,
localization keys, accessibility metadata, and named actions. It contains no
Avalonia type names, C# expressions, HTML, JavaScript, shell text, or arbitrary
network locations.

The Avalonia renderer maps the neutral UI IR into native controls. The web
renderer may map the same IR into DOM. Unsupported presentation hints degrade
visually; unsupported commands fail at capability validation rather than being
silently omitted.

UI IR version 1 is a stable renderer-neutral boundary. Patch decoding validates
operation references and embedded node metadata without requiring renderer
state, while atomic application performs the remaining parent-context and graph
checks against the exact source revision. Unknown action or patch-operation
fields fail closed rather than being ignored by an older renderer.

The desktop event boundary observes every asynchronous reconnect and mutation
task. Window closure is an explicit lifetime fence: it cancels outstanding
requests, unsubscribes remote state, rejects queued post-close projections, and
contains shutdown-time disposal failures. This keeps renderer replacement and
application shutdown independent from transport timing.

Every GUI action must support:

- inspection as a normalized `CommandPlan`
- dry-run preview
- export to canonical Leselang
- replay through CLI or Leselang
- audit correlation by command and effect ID

## Process And Transport Boundaries

Desktop deployments should run `leserpentd` separately and connect through a
local Unix socket or named pipe. This provides crash isolation and lets the CLI
and GUI share one runtime.

Remote web and mobile clients use authenticated HTTPS and WebSocket transports.
The Gate 6 transport slice is a default-off HTTPS listener in `leserpentd`.
`POST /v1/wire` accepts the same bounded wire-v1 envelope as local IPC and
dispatches through the same domain function. `GET /v1/events` upgrades to an
authenticated WebSocket only when the client requests the
`leserpent.events.v1` subprotocol. The listener requires an explicit
address, certificate, private key, and environment-only bearer token; there is
no plaintext fallback. HTTP/1.1 headers are bounded, request bodies retain the
1 MiB protocol limit, ambiguous framing fails closed, and peer-controlled
failures are isolated per connection.

Transport-independent safety mechanics have one implementation in
`leserpent-protocol::transport_safety`: HTTP header token validation, bounded
regular-file opening with atomic symlink rejection on Unix, read-time growth
enforcement, and deadline-bounded TCP connection. CLI, adapters, and daemon
consume those primitives while retaining ownership of TLS configuration,
authentication, private-key permissions, and user-facing error semantics. This
keeps policy local without duplicating security-sensitive I/O.

The synchronous connection budget starts before address resolution, so time
spent resolving consumes the remaining socket-attempt budget. The platform
resolver itself cannot be interrupted through `std::net`; callers therefore
treat this as a resolved-address connection deadline rather than claiming a
hard DNS wall-clock timeout.

The event schema is versioned independently from request/response wire-v1.
Sessions receive endpoint-redacted runtime snapshots, revision heartbeats, and
an explicit `resync_required` event when a requested cursor is ahead of the
authority. A missing or older cursor receives a fresh snapshot; the daemon does
not claim durable delta replay. Session, frame, message, write-buffer, and
per-tick inbound work are bounded, and the event channel itself is read-only.
All versioned Rust wire envelopes now reject unknown fields, matching the
schema's fail-closed top-level contract and the strict .NET decoder. Health and
remote projection payloads apply the same rule: optional v1 fields may be
absent, but misspelled or undeclared fields cannot be silently ignored.

The current transport boundary has a named reproducible proof:
`gewyvern_validate leserpent-transport`. It composes wire-v1 and legacy fixtures,
CLI/Leselang parity, a real authenticated Unix-socket vertical path, and
fail-closed IPC plus HTTPS security tests into retained evidence. The HTTPS
suite includes a real TLS loopback, strict framing/authentication rejection,
private-key file checks, shared wire dispatch, a native CLI-to-daemon HTTPS
vertical path with explicit CA trust, and authenticated WebSocket snapshot and
cursor-resync tests. Explicit CA trust is the stable CLI trust policy; it does
not depend on ambient system roots. Both IPC and HTTPS credentials, plus their
temporary authenticated request/header buffers, use zeroizing storage so
transport teardown and error exits clear secret material. A future Windows
named-pipe adapter is optional because the native CLI already uses the same
authenticated HTTPS contract on that platform. The Avalonia desktop client now
consumes that event
contract with explicit CA and hostname verification, per-origin
endpoint-redacted snapshot cache, immediate stale-state presentation, a capped
eight-attempt reconnect loop, and cursor reset on `resync_required`. Its first
mutations are deliberately not generic: runtime-bound actions open explicit
confirmation and send only typed `runtime_refresh` or
`runtime_capabilities_refresh` commands through authenticated `POST /v1/wire`,
with `runtime.refresh` capability, principal, idempotency key, and the displayed
runtime revision. Stale state cannot mutate and ambiguous network failures are
not retried automatically. Strict capability decoding applies the same source,
version, endpoint, deployment-consistency, and extension bounds as the Rust
domain. Capability controls remain fenced until a projection newer than the
command revision carries an observed snapshot whose
`capabilities_observed_for_revision` binds it to that command. The optional
field keeps old snapshots readable; capability journal replay semantically
upgrades legacy outcomes while continuing to reject unrelated divergence. A real Rust-to-.NET vertical
proves both command bindings, real adapter execution, and subsequent WebSocket
revisions agree without persisting the runtime or adapter endpoint. Unknown
mutation outcomes require a later full snapshot; heartbeats carry revision
liveness but cannot resolve command ambiguity. Desktop token resolution and
mutation use macOS Keychain or Linux Secret Service through AOT-compatible
native bindings, scoped by canonical HTTPS origin. First-run setup accepts an
optional protected token, validates it before platform mutation, and clears the
control immediately after submission; an environment token is accepted only
when no platform item exists. Malformed stored credentials fail closed and no
secret enters the profile, UI IR, or cache.
Mobile clients, mobile secure-storage lifecycle, and mobile cache lifecycle
remain separate implementations that must pass the same versioned domain
contract.
Workspace filtering, bounded diagnostic export, live-refresh/backoff planning,
snapshot deltas, and severity retention live in `Leserpent.RemoteClient` rather
than an Avalonia assembly. These policies contain no renderer or transport
dependency; desktop controls consume them, while MobileCore runs the identical
public contract before a native workspace surface is added.
Remote fleet and runtime-workspace projection follow the same boundary.
`Leserpent.RemoteClient` maps remote state into the shared `UiDocument` model,
including filtering, capability-gated actions, parameterized deployment forms,
endpoint omission, and accessible empty states. Avalonia is only a renderer of
that document, while mobile hosts can substitute native controls without
forking projection semantics.
Remote mutation fencing is also frontend-independent. A successful command
retains its revision fence until the matching runtime projection arrives; an
ambiguous timeout or network failure retains an observation fence until a newer
authoritative snapshot arrives. Capability changes additionally require a
revision-bound capability observation, and heartbeat-only progress cannot
release either safety condition.
The corresponding action-availability projection is shared domain policy.
In-flight mutation, revision fence, observation fence, and non-live state have
a deterministic precedence. It independently reports mutation and inspection
availability with bounded reasons, so native renderers cannot accidentally
enable an action by interpreting presentation state differently.
Workspace creation and subsequent state refresh pass through one availability
application point, preventing a live/idle shortcut from overriding an
unresolved mutation fence. Authority health projection is shared too: ready,
queue pressure, saturation, and automation text are derived before renderer
selection.
The host-independent `Leserpent.MobileCore` now owns the first mobile lifecycle
contract: foreground creates one session after loading an endpoint-scoped vault
token, background invalidates its generation before disconnecting, reentry
reloads the credential, and retired-session events cannot update current state.
Android Keystore and iOS Keychain vault implementations are platform adapters
rather than transport or domain forks. Android stores only AES-256-GCM
envelopes in private preferences while its non-exportable master key remains in
Android Keystore. iOS uses generic-password Keychain items scoped as
`WhenUnlockedThisDeviceOnly`. `MobileCredentialVault` keeps both adapters
narrow: shared code enforces endpoint canonicalization, opaque hashed aliases,
token bounds, read/write validation, deletion, and cancellation before
platform access.
They do not embed privileged adapters. An optional embedded Rust library may be
added later for offline mobile operation, but it must implement the same
`leserpent-protocol` contract.

The Android entry client is a thin platform composition rather than a domain
fork. `MainActivity` delegates repeated start/stop callbacks to the shared
`MobileApplicationCoordinator`, which owns secure configuration replacement,
foreground session uniqueness, background disconnect, failure state, and
terminal disposal. Android persists only the canonical endpoint in private
preferences and a validated public CA in app-private files; tokens remain in
the Keystore-backed vault. The native shell may project connection and runtime
state, but mutations must arrive through renderer-neutral form events before
being exposed on Android.

Transport schemas are versioned independently from UI releases. Unknown fields
are ignored only where the schema explicitly allows forward compatibility.
Mutations always carry intent, identity, idempotency, and revision metadata.

## Persistence And Replay

The runtime owns:

- an append-only command/effect/event journal
- current domain projections
- Leselang continuation images
- audit records
- bounded per-runtime log records and sequence cursors
- migration metadata

SQLite is the default durable implementation, not the domain interface.
Snapshots accelerate startup but are rebuildable from supported journal
history. Dual-generation recovery validates every candidate and returns a
structured storage error when none is usable; authority startup contains no
panic-only fallback. Sensitive pairing material is stored through a platform
secret adapter and never serialized into UI IR, logs, model context, or ordinary
exports.

Runtime journal schema 10 adds strict Orchestra run and event storage. One
owner-fenced transaction writes the canonical run/event pair and reads both
records back before commit. Replaying the same event identity is idempotent only
when its canonical bytes are unchanged; payload drift or cross-runtime run-ID
reuse rolls the whole transaction back. The daemon exposes this primitive only
through the typed, capability-gated `orchestra_persist` wire operation. When a
daemon socket is configured, the 1.x host composes a daemon-backed store and
does not instantiate or dual-write its managed SQLite Orchestra provider.
Canonical history reads use the independently typed `orchestra_history`
operation. Runs and events are paged with a fixed 64-record ceiling; event
queries bind both runtime and run identity, and returned event IDs are projected
from the SQLite sequence. Frontends must exhaust these pages rather than access
the journal file or request an unbounded snapshot.
`orchestra_delete` is the matching bounded mutation: it accepts at most 128
unique runtime IDs, deletes runs and cascading events in one owner-fenced
transaction, and returns actual affected counts. The same schema enforces one
request ID per runtime and retains at most 32 current runs per runtime.

## Security Boundary

Model-generated programs are untrusted input. Before execution they pass:

1. syntax and size limits
2. type and effect checking
3. command and resource planning
4. capability validation
5. destination and adapter policy
6. dry-run and human confirmation when required
7. runtime fuel, deadline, memory, output, and concurrency limits

Leselang cannot dynamically load native libraries, invoke shell commands,
reflect over host types, construct raw HTTP requests, or execute generated
Rust/C#/XAML/JavaScript. Such behavior exists only as an explicitly installed
and capability-gated adapter.

## Performance Contract

### Platform support order

The native operator path is intentionally ordered by available proof quality:
the macOS product shell and shared Linux desktop semantics first, Android only
after the desktop application/profile/menu/release paradigm is stable, and iOS
after Android parity. Windows operators use the authenticated TypeScript web
console during this cycle, and other platforms must consume either Rust-backed
native shells or the shared web client. Windows Avalonia, NativeAOT,
native named-pipe, and installer work remain valid future extensions, but they
do not block desktop stabilization.

No-argument desktop launch is the product entry rather than a fixture shortcut.
It reads a bounded, atomically persisted profile containing only the HTTPS
origin and CA path, resolves the endpoint-scoped token from Keychain or Secret
Service, then constructs the same `RemoteMainWindow` used by explicit CLI
startup. Missing credentials or invalid profiles return to an accessible setup
window. Renderers never receive the token and fixture loading remains an
explicit conformance-only path.

Desktop connection management is a product operation rather than a second
bootstrap implementation. The macOS application menu and the renderer status
bar open the same setup window. A new validated remote session becomes the main
window before the previous session is disposed, so invalid replacement input
cannot destroy a working console. Forgetting a saved connection is explicit and
confirmed: the maintenance boundary reloads and compares the persisted profile,
deletes only its canonical endpoint credential, then clears the profile. A
stale UI cannot delete newly replaced state, and environment fallback is never
mutated.

Remembered desktop trust anchors are immutable application state rather than
ambient path references. `DesktopCertificateAuthorityStore` strictly decodes one
CA PEM, rejects trailing material, non-CA certificates, invalid signing usage,
links, and oversized files, then canonicalizes it into a SHA-256-named private
file. Startup migrates legacy external paths and rechecks that managed content
still matches its fingerprint path before constructing any transport. Pruning
is bounded to recognized regular certificate and crash-temporary names; unknown
entries fail closed. Ephemeral connections may use an external CA for the
current process, but never persist that path or create managed residue.

The design optimizes semantic work before renderer choice:

- owned or interned IDs instead of leaked `'static` strings
- compact versioned IR
- stackless VM frames
- incremental query projections
- keyed UI reconciliation and bounded patches
- streaming event transport with backpressure
- zero-copy decoding where measurement justifies it
- compiled Avalonia bindings and virtualized collections

Native AOT is a deployment target, not a substitute for measurement. Each
phase records cold start, resident memory, command latency, effect throughput,
UI patch cost, and binary/package size before tightening budgets.
The first desktop proof uses source-generated JSON metadata and an explicit
NativeAOT publish profile. Its runtime, compiler, linker, targeting, and
app-host packs share one pinned patch version, so SDK patch drift cannot silently
change the native dependency graph. macOS arm64 now packages that output and
the Rust `leserpentd` through the native `gewyvern_leserpent_bundle` boundary.
The boundary rejects symlinks, unknown or non-arm64 payloads, a missing daemon,
and implicit replacement; excludes `.pdb` and `.dSYM`; and emits stable plist
identity, a checked `.icns`, a native application menu, and
Dock-reopen/explicit-Quit lifecycle behavior. The desktop supervisor creates
app-private loopback TLS material, labels its random credential as local-process
state, proves authority health before use, and performs SIGTERM-first daemon
cleanup with immediate restart proof. Daemon resolution is package-local and
fail-closed; its child environment contains only the ephemeral token, while TLS
files use atomic `0600` creation under a checked non-symlink `0700` directory
and private-key export buffers are zeroed. A physical Ubuntu x86_64 host
produces a five-file, approximately 76 MiB package with a stripped PIE ELF.
Both native executables pass the real control-tree fixtures. Windows native
desktop remains deliberately unclaimed until a suitable host exists; the
TypeScript web console is the current Windows access path.

The macOS release boundary is another native Rust entrypoint. It requires the
bundled `leserpentd`, validates its ARM64 Mach-O identity and executable mode,
and signs it together with nested dylibs before the outer app. It refuses
non-Developer-ID identities, requires Hardened
Runtime and secure timestamps, and applies the checked empty entitlement set:
NativeAOT needs no JIT exception and this direct-distribution build is not App
Sandboxed. Notarization accepts only a pre-stored Keychain profile, packages
with `ditto --keepParent`, waits for explicit acceptance, removes the temporary
archive, staples and validates the ticket, and performs a final Gatekeeper
assessment. Ad-hoc verification is a separately labelled local-only mode and
cannot satisfy the formal release gate. Hardened ad-hoc code has no Team ID, so
individually signed native libraries cannot pass runtime library validation;
the verifier explicitly withholds a runtime-launch claim. Local UI smoke uses
an ordinary ad-hoc bundle, while formal Hardened Runtime launch requires one
Developer ID identity across the main executable, `leserpentd`, and all nested
dylibs.

Packaged desktop startup is not a second composition path. Both normal
no-argument launch and its release probe call `DesktopProductStartup`, which
loads the bounded profile, resolves the canonical endpoint credential, creates
validated remote options, and preserves credential provenance. The macOS proof
uses only an isolated temporary profile and high loopback endpoint, refuses to
overwrite an existing Keychain item, generates the fixture token internally,
and deletes it in a guarded `finally`. A subsequent system Keychain lookup must
report no item. This proves the app bundle consumes saved profile and native
Keychain state without introducing a test-only credential provider.

The named `gewyvern_validate leserpent-benchmark` shelf now makes the
performance contract executable for runtime cold open, command-query latency,
effect enqueue throughput, UI document/patch/codec cost, .NET workspace-log
incremental merge cost, and release binary size. Budgets intentionally detect disaster regressions rather than compare
unrelated CPUs or filesystems; exact measurements are retained per host class.

Accessibility is a cross-boundary proof, not a renderer assumption. Rust rejects
unlabelled actions in the neutral IR; Avalonia then audits realized Automation
IDs, names, help text, action control types, and theme contrast. Accessibility and NativeAOT proof
processes use separate .NET artifacts roots, so concurrent release checks cannot
race on project intermediates, reference assemblies, or PDBs. Intermediate
graphs are removed after success while retained logs and release artifacts stay
within their named evidence shelf. The named managed shelf passes on macOS and
physical Linux/Xvfb, and macOS NativeAOT
consumes the same proof metrics. The checked theme floor is 4.723 against a 4.5
WCAG AA requirement.

## Compatibility And Migration

During migration, the 1.x ASP.NET service remains usable. New Rust components
first run beside it through a compatibility adapter:

```text
existing API <-> compatibility adapter <-> leserpent-protocol
```

No big-bang rewrite is permitted. A surface moves only after parity fixtures
prove that old and new implementations produce equivalent normalized commands,
authorization decisions, events, and projections.

The existing TypeScript dashboard remains a supported bridge until the shared
UI IR and at least one native client pass the same conformance suite.

## 2.0 Definition

Leserpent 2.0 is ready only when:

- Rust owns command, query, policy, journal, effect, and replay semantics
- Leselang, CLI, and Avalonia pass one parity matrix
- no C# or TypeScript frontend introduces control-plane business logic
- model-generated programs execute only through the normal capability boundary
- suspended programs survive restart and resume exactly once
- GUI actions round-trip through canonical Leselang
- desktop and one mobile target pass release tests
- compatibility and rollback from the final 1.x bridge are documented
