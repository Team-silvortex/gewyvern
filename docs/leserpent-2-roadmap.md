# Leserpent 1.0 To 2.0 Roadmap

This is the execution roadmap for the
[Leserpent 2.0 architecture](leserpent-2-architecture.md). The architecture
defines the invariant destination; this page defines ordered delivery gates.

The roadmap is capability-gated, not date-gated. A later gate may be prototyped
early, but it cannot become authoritative before its prerequisites are green.

Implementation stack rule:

- control-plane authority is Rust-first.
- all native shells and renderers are C# where applicable.
- the browser/operator web client remains TypeScript-only.
- no additional UI/runtime language is allowed to own canonical control-plane
  semantics (Node.js, Python, shell scripts, etc. are explicitly excluded).

## Baseline: 1.x Bridge

The current ASP.NET, TypeScript, SQLite, Orchestra, deployment, security, and
fleet behavior remains supported while Rust replacements are introduced.

Before migration:

- preserve the 1.x API and security tests
- capture command, authorization, persistence, and projection fixtures
- classify browser-only preferences separately from domain state
- prohibit new business logic in frontend event handlers
- record representative performance and deployment baselines

Exit: the current behavior is reproducible enough to compare with Rust.

## Gate 1: Domain And Protocol Kernel

Build `leserpent-domain` and `leserpent-protocol`.

- define IDs, revisions, principals, capabilities, commands, queries, events
- define `CommandEnvelope`, dry-run plans, confirmation, and result schemas
- establish canonical JSON fixtures and compatibility policy
- implement the first Rust projection and in-memory runtime
- adapt one read-only fleet route and one idempotent mutation end to end

Exit: GUI/API and Rust fixtures agree on normalized semantics.

`CommandPlan` and `PlannedOperation` now live in `leserpent-domain`, including
schema, operation/capability consistency, and embedded capability validation.
This keeps plan ownership below Leselang, CLI, GUI, protocol, and runtime
adapters.

The current compatibility policy and fixture boundary are maintained in
[`crates/leserpent-protocol/COMPATIBILITY.md`](../crates/leserpent-protocol/COMPATIBILITY.md).

## Gate 2: Leselang Frontend And VM

Build the language without UI-specific shortcuts.

The implemented language surface and its explicit limits are maintained in the
[Leselang language contract](leselang-language.md). That contract, rather than
future examples in this roadmap, defines what callers may use today.

The syntax boundary now includes a bounded canonical formatter with stable
two-space layout, supported string escapes, declaration-order preservation, and
deterministic parse-format idempotence in the retained fuzz shelf. Native CLI
source export consumes this formatter directly. Syntax-tree JSON validates
contiguous token coverage, EOF placement, UTF-8 spans, and bounded AST depth;
token access also remains panic-free after caller mutation. This syntax v1
contract is stable while HIR and VM contracts continue to evolve independently.

- lossless parser and first-class diagnostics
- HIR, lightweight static types, effects, and capability checking
- stackless evaluator with `Done / Effect / Yield / Cancelled / Failed / Fault`
- bounded execution, cancellation, timeout, retry, and deterministic merge
- versioned continuation serialization
- effect journal and exactly-once continuation consumption
- formatter, explain output, and model-oriented language guide

The execution protocol v1 rejects unknown continuation/effect fields,
unsupported program counters, standalone structured-group images, incoherent
effect/result-type pairs, and semantically invalid effect requests. Explicit
legacy query-request decoding remains covered for journal migration.

Current evidence covers the `runtime.list` vertical slice, including a bounded
SQLite journal, transactional sequence allocation, pending recovery, and
durable first-completion replay. The dispatch outbox now adds attempt-fenced
leases, crash redelivery, multi-VM exclusion, and transactional acknowledgement.
The first mutating effect, `runtime.refresh`, now carries one stable domain
idempotency key through crash redelivery and proves single domain commit with
result replay. Typed requested cancellation and trusted wall-clock deadlines now
atomically fence ready or leased dispatches and survive restart as replayable
terminal states. Bounded semantic retry now persists deterministic not-before
clocks independently from transport attempts and preserves command idempotency
across restart. Count-based journal retention now transactionally compacts only
the oldest terminal records in bounded batches, cascades acknowledged outbox
rows, and uses secure deletion without disturbing pending or leased work.
The deterministic merge kernel now normalizes out-of-order branch completions,
selects competing terminal outcomes in declaration order, bounds recursive
structured values, and persists merged output through the existing journal.
Source-level `all` now preserves named branch order in the lossless parser,
lowers branch-local types, and authorizes the union of branch capabilities. The
schema-5 journal now reserves strict merge-group and ordered branch records,
migrates legacy schemas, and rejects malformed recovery graphs. A shared
ephemeral/SQLite primitive now validates and atomically writes the merge plan,
every branch continuation and dispatch, and all declared-order links. Injected
mid-graph failure rolls the entire SQLite transaction back; committed graphs
recover every branch after restart. Branch terminal writes now finalize the
group in the same transaction when the last branch completes, persist the
declared-order merge result, and roll the branch write back if group completion
fails. Success and permanent-failure outcomes survive restart. Retention now
treats a completed merge graph as one logical unit: ephemeral and SQLite
compaction remove its group, links, branch effects, and dispatches together,
while pending or partial graphs remain untouchable. The batch limit counts
logical units, and each graph remains physically bounded to 64 branches. The VM
now starts one-level `all` as an ordered atomic effect batch with a stable merge
token. Non-final branch completion exposes durable waiting progress, the final
branch returns the declared-order aggregate, and SQLite restart resumes only the
remaining branches. Nested `all` remains deliberately rejected before sequence
allocation; the one-level structured-execution VM contract has exited Gate 2.

The `leselang-command` boundary lowers every current authorized runtime query and
mutation into frontend-neutral `CommandPlan` values, including bounded logs,
capability refresh, confirmed deployment, and VM-authority-only debugger
cancellation. The VM, CLI, and UI consume that crate rather than constructing
domain envelopes themselves. Parity tests cover every CLI-exportable Leselang
operation, while deterministic validated plan export remains local and requires
no daemon credentials. Debugger cancellation crosses the separately audited
`leselang-observe` boundary and is deliberately rejected by the ordinary daemon
command executor.

Exit: programs can suspend, restart, re-enter, and replay deterministically.

## Gate 3: Native CLI Parity

Build the Rust `leserpent` CLI on the shared protocol.

- query, inspect, plan, apply, watch, export, and history commands
- stable JSON mode and human-readable mode
- canonical Leselang export for every mutation
- dry-run and confirmation UX
- local IPC and authenticated remote transport

The first native slice now exists in `crates/leserpent-cli` and installs the
`leserpent` binary. It connects only through authenticated wire-v1 local IPC,
reads its token only from `LESERPENT_IPC_TOKEN`, validates owner-private socket
metadata before sending credentials, and supports `health` plus filtered
`runtime list`. Human output is control-character-safe; `--json` preserves the
versioned response envelope for automation. A real binary-to-daemon test covers
both modes. `runtime refresh` now requires explicit `--yes` for mutation, supports
non-mutating `--dry-run`, optimistic revision checks, stable caller-provided
idempotency keys, and local-only canonical Leselang export. A parity test parses,
lowers, and compares the exported command. Local `--export-plan` now emits the
same normalized refresh plan used for execution and requires an explicit stable
idempotency key. A real authenticated IPC and CLI/Leselang parity fixture now
covers the dedicated `runtime inspect` query. Runtime list and inspect now also
export canonical Leselang and validated plans locally through the same lowering
used for IPC execution. Bounded `runtime history` now returns newest-first domain
results through authenticated IPC, VM re-entry, canonical export, and a parity
fixture without exposing journal rows. Bounded `runtime logs` now follows the
same execution/export/plan path and returns at most 256 records. The native CLI now adds a bounded,
revision-deduplicated watch loop over the same normalized inspect plan, with
line-flushed human and JSON output. Local Gate 3 operation breadth is complete;
authenticated remote CLI transport remains under the Gate 6 transport boundary.

Exit: every migrated operation is executable through CLI and Leselang with the
same parity fixtures.

## Gate 4: UI IR And Avalonia Vertical Slice

Build the first replaceable GUI path.

- pure `State -> UiDocument` evaluation
- typed events and `UiEvent -> CommandPlan`
- stable node identity and incremental `UiPatch`
- Avalonia renderer with compiled bindings and virtualization
- fleet list, runtime child workspace, logs, and one debugger workflow
- GUI action inspection, dry-run, Leselang export, and audit correlation

Gate 4's renderer-neutral UI IR and replaceable Avalonia renderer contracts are
complete and stable. Formal Developer ID signing and notarization are
release-assurance evidence rather than renderer blockers; mobile client work
remains owned by Gate 6.

The first renderer-neutral slice now exists in `crates/leselang-ui`. It lowers
the typed fleet projection into a bounded `UiDocument`, resolves revision-fenced
typed events through the shared `CommandPlan` path, and computes deterministic
remove/insert/move/update patches over stable node IDs. Validation rejects
duplicate IDs, oversized or over-depth trees, unlabelled actions, stale events,
and actions rebound to another runtime. No endpoint, renderer, persistence,
transport, HTML, script, or adapter type enters the IR. Broader renderer and
debugger interactions are covered by the stable v1 contract and conformance
fixtures. A framework-independent
patch application reference now fences revisions and rejects malformed graph
edits; round-trip fixtures establish the semantic renderer conformance baseline.
The runtime child workspace now combines same-revision inspect and bounded
history projections into status, snapshot, refresh, and history nodes. Torn
state fails closed, endpoint data remains outside the IR, and history changes
apply incrementally. A separately bounded log projection now lowers sanitized
typed entries without adapter or endpoint fields and applies sliding windows
incrementally. The `leselang-observe` producer now validates bounded source
batches, preserves the newest sequence window, and sanitizes display text before
UI lowering without admitting endpoint or transport fields. SQLite schema 8 now
persists a 4096-record window per runtime behind an indexed sequence cursor.
Initial reads return the newest bounded window; incremental reads return only
records after the supplied cursor. The typed query requires `runtime.read` and
round-trips through authenticated Unix IPC without endpoint disclosure. The
first read-only
debugger document now models stackless synchronous
effect waiting and re-entry with bounded logical frames, sanitized summaries,
and no continuation token or local-value exposure. The `leselang-observe`
composition boundary now converts an authoritatively validated suspended VM
effect into that projection, rejects torn revisions, and proves that
continuation tokens, principals, capabilities, idempotency keys, and absolute
scheduler deadlines never reach serialized UI state. `DebuggerCancel` now uses
the shared command plan with `debugger.control`, revision/session fencing,
explicit confirmation, safe inspection, and a non-mutating dry-run. Confirmed
execution reaches the VM's durable cancellation path without returning the
continuation token. VM journal schema 6 atomically persists the requested
cancellation and its command/session/revision audit, rejects conflicting
principal-scoped idempotency reuse, and replays the original audit after a
restart. The public record omits continuation tokens and idempotency keys, and
is pruned with its retained continuation. The waiting debugger document now
declares a session-bound cancel action that lowers through the same shared
command planner. Rust and .NET reject session rebinding, and Avalonia renders a
destructive button while emitting only its stable node ID.

The first concrete cross-language renderer core now exists under
`apps/leserpent-avalonia`. Rust emits a bounded versioned JSON fixture and the
.NET 10 adapter strictly deserializes it, mounts the previous semantic tree,
applies incremental operations, mirrors runtime/action validation, and proves
equality with the Rust next document at zero build warnings. The first Avalonia
12 desktop shell now maps every current semantic node kind to real controls,
preserves stable IDs and accessibility metadata through Automation properties,
and emits only action node IDs back across the renderer boundary. A platform
smoke mode initializes the real control stack, renders the Rust fixture, and
exits cleanly. The mounted control tree now consumes remove, insert, move, and
update operations through a stable-ID visual index after a transactional
semantic candidate validates the complete patch. The compound fixture proves
unchanged and moved controls retain identity instead of rebuilding the tree.
Fleet cards now declare a runtime-bound Inspect action alongside Refresh. The
Inspect event is revision-fenced and lowers in Rust to a frontend-neutral
`runtime.read` query plan; .NET validates the same runtime binding and renders
the action without constructing a query itself. The remote desktop now executes
that plan through the authenticated wire boundary: it composes Inspect, bounded
History, and bounded sanitized Logs only at the same revision, discards
endpoint-bearing wire DTOs before creating safe state, and opens one reusable
child window per runtime. Log append does not advance the control revision, so
the child exposes explicit Reload/F5 instead of pretending event-driven freshness.
Open workspaces refresh from newer live event revisions and share the fleet
window's mutation confirmation/fencing path. A fixed eight-window bound prevents
accidental connection and window fan-out.
Fleet roots now use an active `VirtualizingStackPanel` as the window viewport,
and history sections use independent bounded viewports rather than nesting
under an unbounded outer scroller. This establishes layout virtualization;
compiled-bound item view models now defer leaf-control creation until viewport
realization. A long bounded-history fixture retains unrealized items after the
window opens, proving off-screen controls are not constructed. Heterogeneous
container subtrees now remain as patchable renderer models until their parent
enters the viewport, so runtime cards no longer force eager descendant control
creation. The desktop shell now publishes through a checked NativeAOT profile
with a single pinned runtime/compiler/linker patch set. macOS arm64 and a
physical Ubuntu x86_64 host both produce five-file self-contained native
packages and pass all real control fixtures without a managed runtime
installation. The Linux debugger fixture preserves the one-to-zero
cancel-control lifecycle under Xvfb. macOS/Linux regressions remain the desktop
priority. Android entry groundwork is parked until the macOS application,
connection profile, menu/lifecycle, and release-bundle paradigm is stable.
Windows native desktop is deferred and Windows operators use the authenticated
TypeScript web console in this cycle.

The Avalonia renderer now maps bounded log entries to lazy monospace controls.
The 48-entry cross-language sliding fixture applies in three operations and
leaves 26 off-screen controls unconstructed after first layout.

The same renderer maps debugger logical frames lazily. A 40-frame fixture moves
from `WaitingEffect` to `Yielded` in seven operations, removes the cancel
control, and leaves 18 frames unconstructed after first layout. Control smoke
evidence records one realized debugger-cancel button before the patch and zero
after re-entry.

The authoritative Rust diff now updates a working document as it emits each
operation and refuses to return unless that document converges exactly on the
target. This fixes invalid move indexes previously exposed by a sliding bounded
history window and reduces that transition from 34 operations to 3.

The no-argument desktop hub now has a local Rust-authority path. It supervises
an app-bundled `leserpentd`, creates private loopback TLS material and an
ephemeral local-process token, and reaches the normal remote window only after
the shared health client proves a ready owned authority. The process boundary
uses SIGTERM-first cleanup so journal ownership is released before immediate
restart; a real .NET-to-Rust verification command covers startup, TLS health,
shutdown, and restart. The macOS bundler fails closed when the arm64 daemon
payload is absent.
The Hub now implements the architecture's actual daemon topology instead of a
single remote-launch shortcut. A bounded private catalog atomically migrates the
legacy profile, derives stable IDs from normalized authenticated authorities,
and renders Local Orchestra plus every saved `leserpentd` as independent daemon
branches. The Hub remains open while authority-keyed session windows are reused;
different daemon sessions can run concurrently without sharing runtime state.
Managed trust pruning retains the full catalog CA set and the local authority.
A real Avalonia probe verifies the hierarchy and independent Open/Manage controls.
Each daemon branch now exposes a bounded direct runtime preview. A strict shared
RemoteClient query sends the canonical read-only `runtime_list` envelope,
validates unique runtime identities and revision ownership, drops runtime
endpoints before projection, and renders at most six gewyvern children per card.
The Hub limits topology reads to four concurrent authorities, cancels them on
close, marks live versus endpoint-bound cached evidence, and preserves manual
session opening when a preview is unavailable. The local-orchestra vertical now
proves this query against a real Rust `leserpentd`, not only a fixture codec.
Topology card lifecycle is now a renderer-neutral RemoteClient state machine.
The desktop refreshes all cards every 30 seconds with the existing four-query
concurrency bound and no same-card overlap. A failed refresh retains the latest
child tree as explicitly stale, rather than erasing useful evidence; live,
cached, retained, and unavailable states remain distinct, and revision
regression is rejected before rendering.
Live daemon identity is now stronger than a successful fleet read. Each refresh
performs strict `health` and `runtime_list` requests in parallel, composes them
into one topology snapshot, and refuses the live phase without a ready owned
protocol-v1 authority proof. Validated queue pressure is visible per daemon;
cached snapshots retain no fabricated health. The real local Rust-daemon
vertical proves this composition across the TLS process boundary.
Runtime preview rows now route directly to the existing per-runtime workspace
through their owning daemon session. The session is created or reused first,
then holds a bounded pending request until an authoritative event snapshot has a
`snapshot_revision` at least as new as the Hub topology revision. Feed heartbeat
revision is tracked separately and cannot release the workspace fence. A newer
snapshot that no longer contains the runtime rejects the request, and the shared
eight-workspace limit counts pending requests as well as open windows.
The supervisor also rejects symlinked state/daemon boundaries, performs no
ambient `PATH` daemon discovery, clears the inherited child environment, writes
all TLS material with atomic owner-only creation, and zeroes exported private
key buffers after the daemon identity is persisted.

Exit: the vertical slice contains no direct adapter or persistence access and
passes GUI/CLI/Leselang equivalence tests.

## Reverse Deployment Bootstrap (Cross-cutting Before Gate 5)

The control path for remote hosts is now first-class: operator credentials are
used to bootstrap a target-host `leserpentd`, then all subsequent mutation uses
the target-issued `leserpentd` session credential.

The slice includes these required behaviors:

- bootstrap input is a non-session bootstrap token and optional bootstrap endpoint;
  the input is bounded, logged (redacted), and cannot imply any immediate runtime
  mutation authority;
- bootstrap result is a managed `leserpentd` service identity, endpoint, and
  session credential handle that can be promoted to a saved connection profile;
- client entrypoint prefers saved profiles, opens a logical daemon session, and only
  then exposes runtime fleet projections for that daemon;
- `runtime.register` and `runtime.deploy` in that daemon use the same envelope,
  confirmation and revision semantics as local execution, with the active session
  identity treated as the authoritative `daemon_id` binding.

The model explicitly forbids crossing from bootstrap to mutation without an
explicit confirm/fence handoff. A control attempt that still uses bootstrap-only
proof must be rejected before adapter dispatch. After the first accepted session,
bootstrap inputs become non-authoritative metadata.

Failure behavior is part of the slice:

- bootstrap transport failure writes bounded bootstrap fault evidence without
  creating remote runtime authority;
- partial bootstrap artifacts are treated as disposable and do not auto-mark the
  host as trusted;
- if bootstrap succeeds but session establishment fails, status remains
  `Bootstrapped` but no mutating intent is accepted until a valid session
  binds in;
- if session establishment succeeds but `runtime.deploy` is denied, the daemon
  session remains readonly for inspected operations and returns signed rejection.

The first bootstrap kernel is implemented in Rust. `leserpent-domain` owns a
validated `Planned / Deploying / Bootstrapped / SessionBound / Failed` state
machine, bootstrap-only capability and confirmation checks, principal binding,
opaque vault credential handles, daemon/session identity matching, and the rule
that only `SessionBound` permits runtime mutation. `leserpent-protocol` owns a
separate strict 64 KiB bootstrap wire-v1 envelope rather than adding bootstrap
proof to the ordinary daemon command channel. Unit tests cover successful
handoff, pre-session mutation denial, transport failure cleanup, identity
confusion, unknown secret fields, raw-secret handles, and malformed response
state.

The native Rust SSH transport is now implemented in `leserpent-adapters`. It
requires an exact host policy and pinned SHA-256 host key, resolves bootstrap
and session credentials through separate vault handles, transfers the bounded
installer through SFTP, verifies its read-back digest and private executable
mode, and accepts only a bounded typed installer response matching the planned
bootstrap, daemon, and endpoint identities. It never invokes a system `ssh`
binary, shell script, Node, or Python. Transport and policy tests prove that
missing policy, wrong credential provider, host-key rejection, and remote
identity drift fail before session authority is granted.

The target-side native `bootstrap-install-v1` entry point is now implemented.
Its shared strict installer wire keeps the session token in zeroized private
storage and separates `installed` from `ready`. The daemon verifies its own
uploaded digest, commits an immutable generation with private file modes, and
atomically advances a non-symlink `current` marker. Unit tests cover digest
failure, symlink rejection, retained-token conflict, rollback preservation, and
idempotent replay; a subprocess proof executes the real entry point and confirms
that stdout contains no token. The SSH adapter rejects the `installed` state.

Native launchd/systemd publication and activation, timeout recovery, a real SSH
cross-process ready proof, WinRM, CLI commands, and the Avalonia Hub flow remain
before this gate exits.

The next service prerequisite is also complete: each immutable generation now
creates and validates a self-signed endpoint TLS identity. The public CA PEM and
its content-bound SHA-256 return over the pinned installer channel, while the
private key stays mode `0600` on the target. Daemon startup can consume the
session token from a bounded private `--remote-token-file`, removing secrets
from launchd/systemd arguments and environments. Controller-side trust
persistence, native service activation, and authenticated health promotion to
`ready` remain. The installer now renders a retained mode `0600` launchd plist
or systemd unit inside the immutable generation. It references only private
files and state/log paths, never token text; idempotent replay rejects descriptor
tampering. The descriptor is atomically published to the profile's native
service directory before `current` advances, with symlinked directories rejected.
Loading that descriptor remains a separate privileged step, so this preparation
still reports only `installed`.

A native service-manager activation primitive verifies `current` plus the
retained and published descriptor before using absolute launchctl/systemctl
executables with shell-free, secret-free argument arrays. The SSH production
path now calls `bootstrap-activate-v1`: after activation, an eight-second bounded
probe connects through loopback while validating the requested TLS server name,
generated CA, private session token, wire-v1 health payload, and daemon-owned
authority. Only that complete path returns `ready`; `bootstrap-install-v1`
continues to stop at `installed` for safe preparation and process testing.

Controller trust retention now has an independent native boundary. The SSH
outcome carries the validated CA only as far as `FileBootstrapTrustStore`, which
parses it as a rustls root, binds endpoint and digest, and atomically commits a
private record without following symlinks. The draft domain/wire schema exposes
only a `vault:leserpent-ca:*` handle. Persistence must succeed before the state
can become `Bootstrapped`; failure clears all authority handles. A real-host
proof remains.

The Rust CLI now completes that consumption path. Remote options select exactly
one CA source: an explicit PEM file or a bootstrap trust root plus
`vault:leserpent-ca:*` handle. Handle resolution revalidates the private record
and requires exact endpoint identity before constructing the existing rustls
transport. Existing authenticated HTTPS, IPC, and Leselang parity verticals
remain green. Avalonia now retains the same opaque handle in its connection
profile, strictly decodes the private Rust record through RemoteClient, rejects
endpoint/digest/source confusion, and imports only the validated PEM into its
content-addressed CA store.

The Linux real-host gate is now closed. An ignored, explicit-environment Rust
vertical uploads a size-optimized x86_64 artifact through the production Russh
and SFTP adapter, activates a systemd-user daemon, proves TLS/token/authority
health, commits private controller trust, and keeps mutation disabled until the
matching session and trust identities bind. The same run proves real trust-store
rejection, one-millisecond timeout cleanup, and occupied-port health rollback.
The failed daemon leaves no unit, generation, or staging artifact, while the
primary daemon remains active with zero restarts. Evidence is retained in
`docs/fixtures/leserpent_real_ssh_bootstrap_20260722.json`; credentials and PEM
are deliberately absent.

Controller restart durability is also implemented. The daemon worker converts
only a validated terminal bootstrap outcome into a private schema-v1 checkpoint,
and runtime SQLite schema 12 commits that checkpoint in the same transaction as
effect completion. Restart leaves `Bootstrapped` read-only. The legacy internal
direct-enqueue proof reaches terminal revision 1 and binds at revision 2; the
production submission path starts at revision-1 `Planned`, reaches terminal
revision 2, and binds at revision 3. In both paths a mismatched proof preserves
the current revision, while matching daemon/session/trust authority retires the
bootstrap handle and survives another restart. Malformed adapter output is
terminally rejected without creating or advancing authoritative handoff state.
Unit coverage lives in `crates/leserpentd/src/lib.rs`.

Authenticated checkpoint query and bind-session operations are now available on
the shared IPC/HTTPS dispatch and in the native CLI. Query returns only the
public snapshot. Bind requires explicit confirmation and only a bootstrap ID;
client-supplied authority booleans, daemon identities, handles, and secrets are
unknown fields. A default-off server verifier resolves the private session and
trust handles, checks exact endpoint binding, and proves remote TLS/token health
before the runtime may publish `SessionBound`. The packaged daemon enables this
resolver with `--bootstrap-trust-root` and its platform secret store.

The bootstrap origin is now packaged. `leserpentd` accepts a private, bounded,
strict schema-v1 `--bootstrap-config` only in native-SSH builds, requires the
existing `--bootstrap-trust-root`, loads a non-writable executable artifact,
rejects duplicate daemon/session/trust identities, and registers
`SshBootstrapAdapter<NativeSshBootstrapTransport>` with the same platform
secret service and controller trust root used by session verification. Official
Avalonia Linux publishing and documented macOS bundling enable the feature.

The independent authenticated submission route is also complete. HTTPS uses
`POST /v1/bootstrap`; Unix IPC requires an explicit `bootstrap_v1` route and
never guesses from a failed ordinary-wire decode. Submission is origin-gated
and atomically persists both its effect and revision-1 `Planned` checkpoint,
with idempotent replay and divergent-identity rejection. Worker completion
advances to revision 2 in the same transaction as effect settlement. The native
CLI now exposes `bootstrap deploy ... --yes` with only a target and
`vault:ssh:*` credential handle, plus the existing inspect and bind commands.
Avalonia Hub now exposes the same authority-scoped sequence through native
controls: explicit deployment confirmation, independent `/v1/bootstrap`
submission, bounded handoff polling, and phase-gated server-verified binding.
The locally managed authority now also promotes a bound receipt into the saved
connection catalog only after endpoint-bound trust loading, Rust-compatible
session-handle resolution, and live target health proof. Remote authorities do
not export their local trust stores.

The post-session runtime path now has a separate domain/protocol foundation rather
than overloading `runtime.deploy`. `runtime.provision` models confirmed native
installation, authenticated service readiness, installation-credential retirement,
and identity-bound runtime registration. Its independent strict 64 KiB protocol
rejects unknown fields and raw credentials. `runtime.deploy` remains only the
debugging-pipeline submission operation for an already registered Gewyvern endpoint.
Runtime SQLite schema 13 now provides shared, kind-scoped authority checkpoints.
It atomically queues provisioning with revision-1 `Planned`, settles installation
to `ServiceReady` or `Failed`, restores both after restart, retires the installation
credential at readiness, and revision-CAS gates `RuntimeRegistered`. Existing
schema-11 daemon bootstrap checkpoints migrate losslessly into the same storage
without sharing operation identities. Authenticated HTTPS now uses only
`POST /v1/provisioning`, Unix IPC uses only `provisioning_v1`, and both remain
disabled until a dedicated adapter is registered. The daemon validates adapter
identity and terminal phase before atomically settling `ServiceReady` or `Failed`;
restart retains the public service authority without restoring the install secret.
The internal Gewyvern installer wire is now separate, strict, 64 KiB-bounded,
secret-redacted, and request/ready-response identity bound. It distinguishes
`Installed` from health-proven `Ready`, validates the artifact generation and
public CA digest, and refuses credential-handle substitution. The Gewyvern target
binary implements its `gewyvern-install-v1` preparation half: it digest-checks
the bounded source artifact before mutation, creates a private immutable runtime
generation, retains secret-free replay metadata and a service plan, generates the
endpoint TLS identity, rejects symbolic-link layouts, and atomically publishes the
current generation. It returns only `Installed`. Its `gewyvern-activate-v1`
entrypoint atomically publishes and activates a launchd/systemd descriptor, and
the descriptor starts `gewyvern-service-v1` without secret arguments. The managed
service exposes rustls HTTPS and requires its private API token even on loopback.
A bounded endpoint-name TLS/token health proof is required before `Ready`; failure
restores and restarts the prior generation. The host-key-pinned native SSH
transport now shares bootstrap's Rust connection, exclusive SFTP staging, bounded
command, and timeout-cleanup substrate. A strict private daemon origin config
selects targets and opaque handles; API tokens resolve only through the platform
store. `Installed` withholds trust and a receipt, while `Ready` persists its
endpoint-bound CA before returning authority. The activation path has also passed
a real Linux systemd proof with correct-token HTTP 200, wrong-token HTTP 403, and
post-proof service/unit/runtime cleanup. Daemon settlement derives the
authority-owned registration proof from that receipt and commits effect
completion, runtime registration, and the revision-3 `RuntimeRegistered`
checkpoint in one SQLite transaction. Runtime identity conflict is rejected
before dispatch, lost leases roll the entire registration transaction back, and
legacy revision-2 Ready checkpoints promote safely after restart. The native CLI
now exposes `runtime provision` with explicit `--yes`, an operator-owned
provisioning ID, `vault:ssh:*` handle, authenticated IPC/HTTPS transport, and an
optional bounded `--wait` phase loop. Reusing the same ID is an idempotent replay;
new attempts require a new ID. Human and JSON progress surfaces omit installation
secrets, while protocol failure, terminal provisioning failure, and observation
exhaustion remain distinguishable to automation. Avalonia now provides the same
confirmed authority-scoped operation from the Hub. Its native workspace locks
the complete provisioning identity after submit, renders every bounded phase,
caps automatic observation at 30 requests, and reuses only the exact request for
manual refresh. Failed attempts remain immutable and the UI explicitly requires
a new provisioning ID after remediation. A configured private
`LESERPENT_GEWYVERN_PROVISIONING_CONFIG` also lets the managed Local Orchestra
own this operation without weakening the remote authority contract. The next
product slice implements the now-defined remote retirement contract without
changing `runtime.deploy` semantics. The independent `runtime.retire` domain and
strict 64 KiB wire bind a new retirement ID to the original provisioning/runtime
identity and an explicitly confirmed opaque SSH handle. They require a proven
`ServiceRetired` boundary before `RuntimeUnregistered`; any external retirement
failure keeps the runtime registered instead of creating an unmanaged live
service. The durable runtime slice now atomically completes the leased effect,
journals replayable runtime unregistration, and commits revision-3
`RuntimeUnregistered`; lost leases preserve both the planned checkpoint and live
registration. Adapter-gated daemon submission and worker settlement now enforce
the same identity and terminal-phase checks, while transport secrets are
resolved only at the adapter boundary. Forged receipts leave the revision-1
checkpoint and runtime registration intact. The native target/SSH path is now
present: the shared origin registers provisioning and retirement adapters,
uploads the validated artifact over pinned SSH, invokes the strict
`gewyvern-retire-v1` wire, and accepts only a fully bound receipt. The target
verifies its private manifest and descriptor, persists a two-phase recovery
marker, stops/disables the service, and removes only that runtime root. The
daemon now exposes the same typed request through authenticated
`retirement_v1` Unix IPC and `POST /v1/retirement` HTTPS routes. Each route keeps
the independent 64 KiB limit and remains disabled unless the production
retirement adapter is registered; a real TLS proof commits only a provisioning-
bound revision-1 checkpoint. The native CLI now provides the confirmed
`runtime retire` operation over both authenticated transports with stable
identity replay, bounded polling, credential-free progress, distinct terminal
exit codes, and a negative proof that adapter failure preserves registration.
The Avalonia Hub now provides the matching confirmed control with strict
provisioning/runtime/target identity fencing, locked fields, bounded replay,
credential-free status, and explicit failure-preserves-registration guidance.
The physical Linux stop/remove gate is complete. The retained native SSH test
provisions and health-checks an isolated systemd-user runtime, rejects a forged
provisioning identity, completes the corrected retirement, replays it
idempotently, and proves zero service, process, port, runtime-root, descriptor,
or staging residue. Its redacted evidence is
`docs/fixtures/leserpent_real_ssh_retirement_20260723.json`.

Exit: one positive and one negative proof case exists for each branch:
bootstrap failure, bootstrap success + session connect success, and deploy path
without confirmed transition. Proof evidence must be versioned, reproducible, and
bound to the `leserpent-2-architecture` intent.

## Gate 5: Durable Runtime Cutover

Move authority from the compatibility bridge into `leserpentd`.

The initial `leserpent-runtime` slice validates and executes shared
`CommandPlan` values and now persists runtime registration plus mutating plans
in one ordered SQLite journal. Restart replay rebuilds projections, completes
pending commands, seals terminal command failures, and rejects divergent stored
outcomes. Journal records and payloads are bounded; the database is private and
opened without following links. The journal now transactionally migrates v1 to
v2, records migration history, validates the claimed schema shape, and preserves
legacy replay order.

The cutover now includes typed `RuntimeRegister`,
`RuntimeRegistrationUpdate`, and `RuntimeDiscoveryIntake` commands over the
shared wire path. All are capability- and confirmation-gated, idempotent,
secret-free, strict about unknown command fields, durable across daemon restart,
and intentionally produce no external effect. Create rejects a runtime
revision; update and discovery intake require the exact current runtime
revision. Discovery intake accepts only validated successful capability and/or
status observations, applies them atomically, and records the revision to which
capabilities were bound. Canonical endpoint conflict handling matches the 1.x
identity rule: scheme and host case plus default HTTP(S) ports cannot bypass
uniqueness, while path and query remain part of the target identity. Conflict
responses identify only the owning runtime and do not echo its endpoint.

The configured 1.x Web route now preserves the original Gewyvern capability
document as a typed, bounded authority snapshot while continuing to derive its
legacy presentation list. It queries the daemon revision before update, creates
missing legacy runtimes as an explicit reconcile step, submits registration and
typed discovery observations through private authenticated IPC, and only then
commits the managed compatibility projection. Sidecar discovery failures are
reduced to `sidecar_fetch_failed`, and runtime-status failures become
`runtime_status_fetch_failed`; pairing/admin tokens and raw error payloads never
enter the Rust command. An unconfigured development host retains the managed
fallback.

The first Web read cutover now routes runtime list, runtime detail, and runtime
status through strict typed daemon `runtime_list` / `runtime_inspect` queries
when IPC is configured. Daemon name, endpoint, tags, status, and observed
capabilities and the secret-free sidecar endpoint override managed copies.
Journal-derived registration/update timestamps now override managed copies when
present, survive restart, and do not advance on idempotent replay. Sidecar
status and its bounded memory summary now share the revision-fenced durable
projection. Registration, individual refresh, recovery, Fleet refresh, and
Orchestra recovery compose their available observations into daemon authority
before writing compatibility copies. The shared runtime-status validator covers
both direct intake and scheduler completion. Legacy snapshots without authority
timestamps or sidecar status retain per-field managed fallbacks, while
token-presence and fetch-only compatibility telemetry stay local.
Managed-only runtimes stay
visible until their next registration reconcile, while a daemon-only runtime
fails closed because the adapter cannot safely invent the missing 1.x metadata.
Unknown projection fields, including secret-shaped fields, are rejected. The
current slice now moves attention, protocol-reading, recovery, and sidecar reads
onto this shared projection. Cleanup and generic unregistration now have an
explicit confirmed result contract: a daemon schema-v14 transaction fences all
target revisions, journals removal, deletes Orchestra history, and retains
idempotent operation results. The Web bridge holds a deletion reservation while
the daemon-first mutation and local compatibility cleanup run, so new sessions
and Orchestra runs cannot cross the deletion boundary. Token-presence remains
inside the local secret boundary. Control-plane state schema v2 persists that
deletion intent before daemon mutation. A restart restores the protected target
set, rejects new sessions and Orchestra runs, and a bounded background worker
replays daemon unregistration plus local cleanup until the intent converges.
Schema v1 state upgrades in memory with an empty intent set. Imported snapshots
cannot inject pending destructive work. A real Arm64 Unix run now starts the
production Rust daemon and a separate C# harness, waits until daemon
unregistration commits, force-kills the harness, and proves restart convergence.
Its retained evidence is
`docs/fixtures/leserpent_runtime_deletion_crash_20260723.json`; reproduce it with
`scripts/validation/leserpent_runtime_deletion_crash.sh`. The same script has now
passed on the physical Ubuntu x86_64 host, retained as
`docs/fixtures/leserpent_runtime_deletion_crash_linux_x86_64_20260723.json`.
A repeated fault campaign now covers the intent-persisted, daemon-committed,
and local-cleanup-persisted transitions. Each iteration starts the production
Rust daemon with an independent C# host, force-kills that host at the selected
boundary, reconstructs the formal registry from disk, and waits for background
recovery to remove both daemon and compatibility state. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_fault_campaign.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 aggregates live in
`docs/fixtures/leserpent_runtime_deletion_fault_campaign_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_fault_campaign_linux_x86_64_20260723.json`.
The completed interference gate adds concurrent normal registration and
state-save traffic while this repeated recovery campaign is running. It runs
eight unrelated registrations per crash
scenario: before daemon mutation, after daemon commit, and racing local cleanup.
Every runtime must survive in the live compatibility registry, a fresh disk
reload, and the production Rust daemon. Reproduce the platform aggregate with
`scripts/validation/leserpent_runtime_deletion_concurrency_campaign.sh`.
Retained Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_linux_x86_64_20260723.json`.
The controlled daemon-restart gate now stops `leserpentd` with `SIGTERM`,
observes one real offline recovery failure, reopens the same SQLite database,
and requires the next claim to converge while the concurrency workload remains
active. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_daemon_restart_campaign.sh`;
the Arm64 Unix and physical Ubuntu x86_64 aggregates are retained in
`docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_linux_x86_64_20260723.json`.
The unclean daemon-takeover gate now `SIGKILL`s the production daemon at every
durable deletion boundary, verifies that pre-expiry replacements are rejected,
and waits for the fixed 30-second owner lease to expire naturally before
reopening the same database. Recovery then converges under concurrent
registration and state-save traffic. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_unclean_takeover.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 latency evidence lives in
`docs/fixtures/leserpent_runtime_deletion_unclean_takeover_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_unclean_takeover_linux_x86_64_20260723.json`.
The overlapping-intent gate now persists three independent intents at the
intent-only, daemon-committed, and local-cleanup-persisted boundaries in one
state image. One host termination and one unclean daemon takeover must release
and retry every failed claim independently while preserving concurrent normal
traffic. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_overlapping_takeover.sh`;
retained Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_linux_x86_64_20260723.json`.
The repeated-takeover gate now interrupts recovery after one intent commits its
daemon mutation, kills the replacement daemon, and requires that intent to
finish local cleanup while the other two observe a real second outage. The
remaining claims must release and converge only after a second natural lease
takeover. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_repeated_takeover.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_repeated_takeover_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_repeated_takeover_linux_x86_64_20260723.json`.
The poison-isolation gate now makes the oldest pending intent fail for at least
three recovery passes while later intents continue against the production
daemon. The poison reservation must remain protected across disk reload, and
removing the scoped failure must converge the original intent without state
editing. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_poison_isolation.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_poison_isolation_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_poison_isolation_linux_x86_64_20260723.json`.
The next reliability gate runs a bounded high-cardinality recovery queue with
sparse poison intents and retains per-pass progress evidence.

Schema v3 added validated domain snapshots that preserve
projection revisions and idempotency results; startup restores the snapshot and
replays only its incremental journal suffix. Snapshot metadata and payload share
one integrity check. Schema v4 retains two generations, falls back from a
corrupted newest generation, preserves the journal suffix needed by the prior
generation, and deletes at most 1000 covered records per checkpoint. Schema v5
adds a conditional SQLite owner lease: a second live runtime is rejected,
expired ownership can be replaced atomically, and both stale reads and stale
writes are fenced on their next operation. Schema v6 adds a bounded persistent
effect queue with idempotent enqueue, atomic claim, lease
renewal, attempt-token fencing, crash redelivery, retry delay, terminal
completion/failure, and max-attempt sealing. Daemon heartbeat, typed adapter
worker loops, and status projection integration are implemented; queue
backpressure visibility and terminal task retention are now implemented, while
bounded worker concurrency remains the gate boundary. The runtime now exposes
an explicit owner heartbeat plus a
bounded synchronous worker step. Executors return `Complete`, `Retry`, or
`Reject`; the runtime alone commits the fenced lease transition, while callers
retain control over concurrency, cancellation, and shutdown.

Schema v7 freezes the durable runtime semantics. It records the introduction of
typed status-observation journal entries even though no physical table change is
needed, so a v6 binary rejects a newer database by version instead of attempting
an incompatible replay. Startup requires migration history to be exactly 1
through 7, verifies the effect-table columns and claim index, and rejects unknown
journal kinds before rebuilding projections. Complete v6 databases migrate
transactionally to v7.

Schema v8 adds the dedicated `runtime_logs` table and its
`(runtime_id, sequence)` index. Appending and per-runtime retention occur in one
owner-fenced transaction, retaining at most 4096 records per instance. The
versioned domain query limits each response to 256 records, supports monotonic
cursor continuation, and omits runtime endpoints. Migration history must now be
exactly 1 through 8; malformed pre-existing tables and indexes fail closed.

Applied refresh events now materialize their typed status effect before the
command returns. Startup reconstructs a missing effect from durable applied
command results, closing the crash window between command-journal completion and
queue insertion without duplicating an existing idempotent task.

Status-refresh completion is also durable end to end. A typed observation
carries its runtime ID and expected projection revision; the runtime validates
it on a cloned projection, then appends the replay record and completes the
leased effect in one SQLite transaction before publishing the new in-memory
state. Restart replay verifies the recorded projection outcome. Stale revisions
and malformed adapter outcomes fail the task closed without stopping the worker.

The effect queue reports exact `ready`, `leased`, `completed`, and `failed`
counts together with active capacity and saturation. Authenticated wire-v1
health includes this data through an optional field, so older response payloads
remain decodable. Terminal retention is explicit and bounded: daemon maintenance
runs every 256 heartbeats by default, retains the newest 8192 terminal tasks,
and removes at most 100 per pass without touching ready or leased work. This
also defines the persisted effect-ID idempotency horizon; producers must use
globally unique effect IDs rather than intentionally recycling compacted IDs.

Bounded synchronous concurrency is now explicit. `leserpentd` claims at most
four effect kinds per batch by default (configurable from one through 32), runs
different adapter kinds on scoped native threads, and settles outcomes back on
the single authority thread in claim order. A batch never leases two tasks for
the same adapter kind, so mutex wait time cannot consume a second task's lease.
Adapters receive a cooperative cancellation context; cancellation schedules a
retry, while a panic is contained and rejects only its claimed task. The main
signal loop passes its stop flag into batch execution.

The repeatable Linux stress harness is
`scripts/validation/leserpentd_linux_stress.sh`; it runs natively on the trusted
remote shelf and retains JSON under
`target/validation/leserpentd-linux-stress/`. The 2026-07-15 x86_64 run on Linux
6.17.0-35 proved eight-way observed parallelism with all 256 effects completed,
cooperative cancellation returning a lease to `ready` in 25 ms, strict rejection
of task 10,001 at the 10,000-active capacity, and real process-exit recovery as
attempt two after the 30-second owner lease elapsed. The saturation phase took
28.2 seconds with one transaction per task and `synchronous=FULL`.

The runtime now provides atomic enqueue batches of one through 1000 tasks.
Validation, duplicate-ID detection, existing-record idempotency checks, and the
active-capacity decision all complete before insertion; any conflict or overflow
rolls back the whole batch. A full queue still accepts exact idempotent replays.
On the same Linux host, filling all 10,000 active slots as 100 batches of 100
took 584 ms, a 48.3x improvement over 28,185 ms, without changing
`synchronous=FULL` durability.

With the schema-v7 compatibility fence, atomic batch API, Linux stress evidence,
and negative migration tests in place, the reusable control-runtime contract is
frozen as `1.0.0`. This freezes the runtime cell, not the wider Leserpent 2.0
architecture; daemon platform parity, adapters, renderers, and remote control
continue on their own status-tensor tracks.

Snapshot startup is also panic-free when every retained generation is
unusable. Recovery exhausts the bounded generation history and returns the last
structured storage error through an explicit match; a negative test constructs
two integrity-valid snapshots with unsupported domain schemas to exercise this
exact authority-startup path.

The `leserpentd` crate hosts `ControlRuntime` as a stable standalone authority.
It drives explicit owner heartbeats and bounded worker steps through a typed
adapter registry, supports finite-step smoke execution, and stays in a
heartbeat-only safe mode when no adapter is installed. Graceful Unix signal
shutdown, authenticated local and opt-in remote health, bounded synchronous
worker concurrency, panic-safe lease settlement, and physical Linux stress are
implemented. Adapter breadth remains owned by `leserpent-adapters`; Android and
Windows/Web client parity remain client transport concerns rather than daemon
lifecycle blockers.

On Unix, `leserpentd` now exposes the existing wire-v1 request and response
envelopes over a private `0600` Unix socket. Each bounded line frame carries an
IPC token sourced only from `LESERPENT_IPC_TOKEN`; token comparison is constant
time, malformed or oversized frames fail closed, and internal storage failures
are not reflected to clients. Windows named-pipe parity remains before local IPC
is cross-platform complete.

Wire v1 now includes an authenticated health request whose response is emitted
only after the runtime renews its owner lease, so `ready` proves current local
authority rather than process liveness alone. The daemon maps SIGINT and SIGTERM
to an atomic stop flag; normal stack unwinding removes the socket inode and
releases the SQLite owner row. A real process smoke verifies exit code zero and
lease release after SIGINT.

The first production integration now lives in the independent
`leserpent-adapters` crate rather than the daemon host. Its typed registry
rejects unknown effect kinds. The Gewyvern health and status adapters retain the
loopback-only HTTP default while also supporting explicit authenticated HTTPS
targets with pinned CA input and rustls hostname verification. They keep admin
tokens out of scheduler payloads, strictly frame bounded JSON responses, and
validate the `/health` JSON contract. A vertical
test proves SQLite enqueue through daemon claim and adapter execution to fenced
completion. The typed status adapter now combines `/health` and
`/v1/latest/meta` into a frontend-neutral snapshot without journaling its admin
token. `leserpentd` registers both health and status effect kinds for configured
targets, and status outcomes flow through the runtime's atomic projection and
replay path. When a target credential exists, the daemon also registers the
typed deployment effect. It requires confirmation, posts only the bounded
deployment intent to the fixed Gewyvern endpoint, binds success to the echoed
idempotency fields, and distinguishes permanent request conflicts from retryable
service failures. A SQLite-to-daemon vertical and a real authenticated TLS test
cover this path. Targets now retain only validated secret aliases; a shared
`SecretStore` resolves the allowlisted environment token immediately before
network execution, redacts debug output, zeroizes temporary values, and fails
before connecting when resolution fails. The macOS provider now calls
Security.framework directly and `leserpentd --gewyvern-admin-secret KEY`
explicitly selects account `KEY` under service
`org.gewyvern.leserpent.adapters` without environment fallback. The Linux
provider dynamically resolves libsecret/glib without development packages and
has physical Ubuntu 24.04 x86_64 evidence: ordinary SSH fails closed without a
session bus, while an isolated D-Bus plus gnome-keyring session returns a strict
clean miss. Real TLS tests cover authenticated adapter traffic and ambiguous
HTTP framing rejection. Bounded capability discovery now queries only explicitly
configured targets, canonicalizes endpoint claims, accepts safe boolean schema
extensions, and rejects scan-like payload fields or inconsistent deployment
advertisements. SQLite schema 9 adds the revision-bound capability observation
journal kind. Effect completion and projection update commit atomically, replay
restores the same state, and stale or forged observations fail closed. The
shared `runtime.refresh_capabilities` command now lowers through the same typed
plan from Leselang, CLI (`runtime refresh-capabilities`), and GUI actions. The
runtime converts its domain event into the revision-fenced discovery effect;
none of those operator surfaces can access the low-level effect queue. CLI
inspect and renderer-neutral runtime workspaces now render the same bounded
capability posture: explicit unobserved state, service/version, core booleans,
canonical endpoint paths, and sorted boolean extensions, with no target origin,
secret alias, credential, or raw adapter payload. A real TLS vertical now proves
the remote closed loop: the native CLI submits the capability command, the
daemon persists and schedules it, the real discovery adapter queries only its
configured loopback target, and a later CLI inspect reads revision 3 with the
validated projection. The adapter origin and authorization material remain
absent. Avalonia now strictly decodes and validates that projection, renders the
same bounded capability section, and binds a separately typed discovery action
to the authenticated mutation client. Rust-generated UI fixtures pass the .NET
semantic renderer and real Avalonia control probes. A Rust daemon-to-.NET TLS
vertical additionally submits the capability command at revision 3, runs the
real discovery adapter against a fixed loopback service, observes the validated
projection at revision 4 over WebSocket, and reads the two-entry workspace from
an independent process without retaining either endpoint. Avalonia keeps the
mutation fenced between those revisions so a repeated command cannot invalidate
the in-flight observation. Ambiguous network outcomes additionally require a
later full snapshot: heartbeat-only liveness never releases the mutation fence,
and a newer revision with an unchanged capability posture remains blocked. New
runtime projections now carry the exact command revision that produced the
capability observation. This removes content-based ambiguity when repeated
discovery returns identical data, remains optional for legacy snapshots, and
has explicit old-journal replay evidence without a schema migration. Secure
first-run connection and endpoint-scoped Keychain/Secret Service onboarding now
run inside the no-argument desktop product path, including bounded password
input, immediate control clearing, and token-free profile persistence. The
native Rust bundler now turns the arm64 NativeAOT output into a strict `.app`
with stable plist identity, checked icon, debug-symbol exclusion, native menu,
Dock reopen, explicit Quit, real Finder launch evidence, and strict ad-hoc
signature verification. Its self-check requires the generated plist to match
the canonical template exactly. The native release entrypoint independently
requires unique identity, executable, package-type, display-version, and
build-version fields; both version fields must match the release tool's
workspace version before any signing or notarization command can run. A shared
bounded native-header reader additionally requires the main executable,
app-bundled `leserpentd`, and every `.dylib` to be a thin 64-bit ARM Mach-O
payload, including the ARM64 CPU type, at AOT proof, bundle creation, release,
and installation boundaries. The native release entrypoint signs the daemon
and dylibs before the outer app and now enforces inside-out
Developer ID signing, Hardened Runtime, secure timestamps, Keychain-only
notary credentials, explicit acceptance, ticket stapling, and Gatekeeper
assessment. A native machine-readable preflight now binds separate main-app and
`leserpentd` executable hashes plus the entitlements hash, inventories all eight
Apple release tools, counts valid
Developer ID Application identities, and optionally verifies a named notary
Keychain profile without exposing credentials. The current retained preflight
records all tools ready but zero identities and no requested profile, so
`release_ready=false`; executing and retaining the formal Apple-backed proof
remains the only macOS release gate.
The unified native release gate can now ingest that report through
`--macos-release-preflight`, enforce a bounded and internally consistent schema,
index a normalized copy, and distinguish an external credential block from a
malformed proof or a ready Apple stage. This keeps CI green for valid evidence
collection without ever turning a blocked preflight into a ship signal.
The latest arm64 NativeAOT bundle also passes the shared product-startup probe
against a real temporary Keychain item and saved profile; the item is deleted
in-process and independently confirmed absent afterward.
The product shell now also exposes one shared connection-management flow from
the native macOS menu and remote status bar. Replacement sessions are created
before the active session is released. Forgetting requires explicit
confirmation, is fenced against a changed saved profile, and deletes only the
canonical endpoint's platform credential and non-secret profile. Managed
contract and real Avalonia control probes cover both settings and confirmation
surfaces without exposing token material.
Remote UI operations now enter through observed tasks rather than naked async
event handlers. Closing the fleet window cancels work, removes the event
subscription, fences already queued state updates, closes child workspaces, and
contains client-disposal failures without writing back into a closed control
tree.
Remembered desktop CA trust is now copied out of ambient user paths into an
application-private, content-addressed trust directory. A strict contract proves
single-PEM parsing, CA and key-usage constraints, atomic/private writes, legacy
profile migration, valid-certificate replacement rejection, symlink rejection,
bounded set-based stale-CA pruning, and recognized crash-temporary cleanup.
Ephemeral connections remain non-persistent.
Android adoption resumes after that paradigm is stable. Windows native-host
evidence is not a blocking gate.

The authenticated deployment adapter is now reachable through a shared
`runtime.deploy` command vertical. Domain policy separates deployment from
refresh authority, rejects unconfirmed execution, and validates bounded
pipeline/target input. Leselang, native CLI, and deterministic plan export lower
identically; the durable runtime derives requester and request identity from the
envelope before scheduling the fixed deployment effect. VM dispatch
acknowledgement and real adapter TLS tests preserve synchronous re-entry and wire
compatibility. Avalonia remote workspaces now expose a bounded deployment form
only for runtimes with an authenticated deployment claim. Rust UI IR declares
the fields and emits a typed renderer-neutral `submit` event; Rust lowering and
the .NET semantic renderer independently reject unknown, missing, oversized, or
invalid values. Source-generated JSON probes verify the fixed capability,
explicit confirmation, typed arguments, refresh-field omission, and
cross-renderer event shape. Mobile shells can therefore reuse the form contract
without duplicating deployment semantics.
The 1.x compatibility bridge now also validates the exact confirmed deployment
request after local runtime/capability checks but before remote network I/O. Its append-only fixture
binds route runtime identity, pipeline intent, requested-by principal,
idempotency key, confirmation, and target while rejecting unknown fields. This
freezes the side-effect boundary needed for later Rust control-plane extraction
without changing the authoritative 1.x deployment response path.
When the bridge is configured, Rust now owns the final normalization decision
and returns the canonical deployment envelope. The C# adapter and Orchestra
audit consume that exact pipeline, principal, request ID, and target; a runtime
identity fence rejects mismatched bridge output before any remote effect. The
unconfigured development path preserves the original request, while a real
cross-process test proves the packaged authority handoff.
The Rust authority now also exposes a typed deployment receipt over its private
IPC protocol. A receipt is bound to both command ID and request ID, can only
read deployment effects, and projects persisted queue state as pending,
completed, or failed. Completed receipts carry the adapter-validated Gewyvern
response, closing the daemon-side submit/execute/observe loop without exposing
generic effect outcomes. The configured 1.x host now submits that canonical
intent over an owner-private Unix socket and polls the bound receipt to preserve
the existing synchronous accepted response and error mapping. Partial daemon
configuration, unsafe socket permissions, transport failure, timeout, protocol
drift, and receipt identity mismatch fail closed; an explicitly unconfigured
development host retains the old direct adapter. Fresh daemon databases
idempotently journal configured Gewyvern targets, while restart-time endpoint
drift is rejected rather than silently replacing authority metadata.
An additional append-only compatibility fixture now freezes Orchestra's atomic
run/event persistence unit. Rust rejects mismatched run or runtime identities,
outcome drift, invalid attempts, oversized steps, and unknown fields. The bridge
can validate this envelope. Runtime journal schema 10 now owns strict Orchestra
run and event tables, atomically persists the pair, makes exact event replay
idempotent, rejects event-key payload drift, and returns the bytes read back from
SQLite through a capability-gated `orchestra_persist` IPC response. A configured
ASP.NET host now selects `DaemonOrchestraRunStore` at composition time and does
not instantiate or dual-write the managed SQLite store; the unconfigured
development path retains the legacy provider.
The same daemon boundary now exposes bounded `orchestra_history` pagination:
run pages may be fleet-wide or runtime-scoped, event pages require both runtime
and run identity, pages contain at most 64 records, and event IDs come from the
authoritative SQLite sequence rather than the legacy envelope placeholder.
This closes startup/UI read-back without introducing an unbounded history dump.
The typed `orchestra_delete` operation removes up to 128 runtime histories in
one owner-fenced transaction and reports actual run/event counts. Schema 10 also
preserves 1.x request-ID uniqueness and the 32-run per-runtime retention bound.
A real C#-to-daemon test proves persist, canonical read-back, authoritative event
sequence, delete, and empty read-back against one Rust SQLite database.

- SQLite journal, projections, snapshots, and migrations
- effect workers with backpressure and recovery
- secret-storage adapters
- local daemon lifecycle and process isolation
- compatibility API for the existing TypeScript dashboard
- import of supported 1.x state with rollback evidence

Exit: Rust is authoritative and the old service is an adapter, not the owner.

## Gate 6: Mobile And Remote Console

Extend the same contracts rather than forking product behavior.

- Android entry project and physical-device lifecycle proof
- iOS entry project after Android contract parity is stable
- mobile navigation and adaptive presentation
- authenticated HTTPS/WebSocket protocol
- reconnect, offline read cache, and explicit stale-state presentation
- push/deep-link integration through platform adapters
- optional embedded Rust feasibility study for offline execution
- production TypeScript web client parity for Windows and remaining non-native
  hosts, using the same native policy contracts and transport semantics

The Android executable entry client now composes the validating credential
vault, app-private CA/profile storage, `MobileApplicationCoordinator`, and
foreground/background callbacks. Its first native shell exposes secure setup,
connection state, and bounded runtime summaries. Host-independent conformance
proves duplicate callback coalescing, endpoint reconfiguration, failure cleanup,
and terminal disposal; a static entry-contract test locks Keystore-only token
storage and private profile boundaries. APK compilation, emulator launch,
physical-device Keystore/TLS evidence, and parameterized form-event controls
remain before Android parity is claimed.

The current desktop slice implements the event consumer and first constrained
mutation of this gate. A pure
`Leserpent.RemoteClient` library owns strict event decoding, explicit CA and
hostname verification, endpoint-bound atomic snapshot cache, stale-state
transitions, cursor reset, and an eight-attempt capped reconnect loop. The
Avalonia shell projects snapshots through the same neutral `UiDocument`
renderer used by fixtures. Runtime cards expose only revision-fenced
`runtime.refresh`, reject stale state, require explicit confirmation, and do not
retry ambiguous outcomes. A separate conformance executable proves codec,
cache, resync, and retry behavior; a real Rust daemon to .NET client TLS
vertical proves authenticated snapshot, confirmed HTTPS mutation, matching
WebSocket revision, private cache permissions, and endpoint omission. Mobile
application entry clients and device runtime evidence remain before Gate 6
completion. Desktop startup now resolves endpoint-scoped tokens from macOS
Keychain or Linux Secret Service through AOT-compatible native bindings, with a
bounded environment fallback only when no stored item exists. Deterministic
conformance proves source precedence and malformed-item fail-closed behavior.
The first mobile-independent lifecycle slice now lives in
`apps/leserpent-mobile`: it disconnects before background suspension, marks
retained state stale, hands hydrated cache state to the host before reconnect,
reloads credentials on every foreground generation, and fences delayed events
from retired sessions. A deterministic conformance runner
injects missing credentials and startup failure. Android/iOS native store
projects now compile against .NET 10 platform workloads: Android protects a
private-preferences AES-256-GCM envelope with a Keystore master key, while iOS
uses a this-device-only Keychain item. Application entry projects, simulator or
physical-device runtime conformance, and physical-device AOT evidence remain.
The shared mobile vault adapter contract now provides endpoint-hashed aliases,
strict credential CRUD validation, cancellation fencing, and deterministic
corruption tests; platform storage is therefore replaceable without moving
endpoint or token policy out of shared code.
The workspace policy layer has also moved into `Leserpent.RemoteClient`:
filtering, bounded export, incremental/full refresh planning, retry state,
snapshot comparison, and retained severity alerts are renderer-independent.
Avalonia now supplies only window/control behavior, and MobileConformance calls
the same six contracts through MobileCore's project graph to prevent a future
mobile policy fork.
The fleet and runtime-workspace `UiDocument` projections are now part of the
same shared library and reference only RendererCore. Their filtering,
capability-gated actions, parameterized forms, endpoint omission, and empty
states are no longer Avalonia-exclusive business logic. MobileConformance
executes both projections without loading a platform renderer.
Mutation revision and unknown-outcome observation fences have moved out of the
Avalonia window as well. RemoteClient now owns the runtime/revision/capability
rules, rejects heartbeat-only release, and requires a newer authoritative
snapshot after ambiguous transport failure. Desktop and mobile conformance run
the identical policy.
Remote action availability now follows that boundary too. A pure policy gives
in-flight work precedence over revision and observation fences, disables
mutation and inspection consistently while stale, and supplies bounded reasons
to desktop or mobile controls. Avalonia no longer derives remote permissions
from window state.
The availability audit found and removed two workspace-level overwrite paths:
state application and initial window creation previously used reduced live/idle
checks that could ignore an unresolved fence. Both now pass through one policy
application helper. Authority health and queue saturation projection have also
moved into RemoteClient for desktop/mobile parity.
The desktop workspace live-query policy now tolerates two transient failures with
10-second and 20-second bounded backoff, resets to its five-second cadence after
success, and stops after the third consecutive failure. The state machine remains
transport-independent and preserves single-flight, inactive-window suspension,
and explicit operator restart semantics. A successful manual or event-driven full
workspace query clears pending backoff and restores the normal cadence without
ever retrying a mutation. Query admission owns and stops the outstanding timer;
a single-flight skip preserves the existing failure count and interval instead
of impersonating a successful recovery.
The desktop runtime workspace now saves the same endpoint-free diagnostic snapshot
through the native platform picker. The UTF-8 payload is prevalidated under 512
KiB, suggested filenames discard path/control syntax, overwrite requires platform
confirmation, and failed destinations never expose their path in UI status.
The workspace query group also exports canonical structured Leselang as one
`all(inspect, history, logs)` batch. A dedicated .NET machine entry emits only the
source; the Rust parity test parses it and verifies all three typed read-query HIR
branches preserve the displayed runtime identity. The UI preview is single-instance
per workspace and copying never executes the program.

Exit: desktop and one mobile target pass the same semantic conformance suite;
platform-only presentation differences are documented.

## Gate 7: 2.0 Seal

- remove frontend-exclusive business logic
- freeze version-1 command/query/effect/UI schemas
- complete security, fuzz, failure-injection, and migration audits
- publish performance comparisons against the 1.x baseline
- document packaging, rollback, recovery, and protocol compatibility
- keep the TypeScript client only if it remains a conforming renderer

Exit: every criterion in the architecture's
[2.0 definition](leserpent-2-architecture.md#20-definition) is evidenced.

The first machine-enforced Gate 7 precursor is now
`gewyvern_validate leserpent-schema-freeze`. Its bounded candidate inventory
maps command, query, effect-plan, UI IR, and wire v1 sources to a fixed native
proof registry. The shelf currently proves 65 tests across domain, UI, wire,
runtime migration, legacy-wire migration, and managed control-plane migration
proof suites while explicitly emitting `freeze_ready=false`; promotion to `frozen`
remains forbidden until the rest of Gate 7 and Apple-backed release evidence
are reproducible.
Its companion candidate baseline pins SHA-256 fingerprints for five wire and
legacy-wire fixtures plus four renderer fixtures. This makes accidental exact
format drift fail before the semantic proof suites without pretending that the
candidate contracts have reached their final freeze.
The migration replay now covers runtime journal v1 to current, v3 snapshot
generation history, complete v6 semantics, malformed migration history, and
legacy wire normalization. The managed proof covers SQLite v1 in-place upgrade,
1.x JSON-to-SQLite Orchestra history import, concurrent durable saves, and
failed-save snapshot preservation through ten locked xUnit tests. Injected
replacement failure proves SQLite transaction rollback preserves prior rows;
injected startup migration failure proves the legacy JSON remains byte-identical
and supports both a corrected SQLite retry and explicit JSON-only operator
rollback. The Linux x64 package path now also has install/upgrade/rollback proof
on a physical Linux 6.17 host.
Its locked NativeAOT restore includes the RID-specific compiler and SQLite
assets, and the bundle smoke performs staged install, distinct-release upgrade,
explicit atomic `current` rollback, configuration/state preservation, a live
Rust compatibility request, and rolled-back service health. Unsafe or missing
release links fail closed, while an unhealthy production rollback restores the
original pair. macOS arm64 now has an equivalent user-local package proof
through the native Rust installer. It accepts thin arm64 and bounded universal
Mach-O dependencies, copies symlink-free versioned app bundles, exposes one
stable launcher, rejects escaping or unmanaged links, preserves external user
state, and proves `1.4.0 -> 1.4.1 -> 1.4.0` with a live rolled-back control
fixture. The retained evidence deliberately identifies its signature as ad-hoc
with no Team ID. Provisioned Developer ID signing, notarization, stapling, and
Gatekeeper evidence remain the platform-release gap.

## Continuous Proof Shelves

Every gate maintains:

- parser and VM fuzzing
- golden syntax/HIR/IR diagnostics
- command-origin parity tests
- authorization and confirmation tests
- idempotency and revision-conflict tests
- crash/restart/re-entry tests
- journal migration and replay tests
- UI IR snapshot and accessibility tests
- IPC/HTTP/WebSocket compatibility tests
- desktop/mobile AOT smoke tests
- latency, memory, throughput, and package-size benchmarks

The parser/VM fuzz shelf now has a named native entrypoint:
`gewyvern_validate leselang-fuzz`. A fixed replayable seed covers 2048 arbitrary
UTF-8 source cases and 2048 continuation mutations. The shelf checks lossless
reconstruction, character-safe spans, deterministic syntax JSON, HIR lowering,
bounded VM startup, fail-closed continuation decoding, and canonical image
roundtrip while retaining its configuration and transcript. Its first run found
and fixed an escape-followed-by-multibyte-character lexer panic.

The UI accessibility shelf now has the native entrypoint
`gewyvern_validate leserpent-accessibility`. It audits real Avalonia controls
across all four fixtures for stable unique Automation IDs, exact names and help
text, explicit action labels, and a 4.5 WCAG AA text-contrast floor. Managed
macOS and physical Linux/Xvfb proofs produce matching counts, while the macOS
NativeAOT job requires the same accessibility marker and metrics. The first run
raised destructive-button contrast from 3.841 to 4.723.
Accessibility and NativeAOT now restore and build through proof-local .NET
artifacts roots. A concurrent regression run proves both shelves can execute
without contending for shared reference assemblies or PDBs, and successful
runs remove intermediate graphs while retaining evidence.
The NativeAOT inventory is fail closed as well: unreadable entries, directories,
symlinks, and non-UTF-8 names cannot disappear from the bounded artifact count,
and every retained evidence-index entry must be unique and resolve to a regular
file before the shelf reports success.
Release signing snapshots, resilience log directories, and package discovery
follow the same rule: enumeration errors, symlinks, unknown payloads, and
extension-shaped directories are explicit failures rather than silently
filtered absences.

The transport shelf now has the native entrypoint
`gewyvern_validate leserpent-transport`. It retains separate transcripts for
canonical wire-v1 decoding, legacy-v1 adaptation, CLI/Leselang semantic parity,
the real authenticated native CLI-to-daemon Unix socket path, and daemon IPC
security rejection paths. Gate 6 adds a server suite for the default-off
authenticated HTTPS `POST /v1/wire` server: a real TLS roundtrip, constant-time
bearer authentication, strict bounded HTTP framing, private-key file safety,
and shared wire-v1 dispatch. A seventh suite drives the native CLI through that
endpoint with explicit CA trust and proves health, query, bounded watch,
confirmed command/idempotency, and auth-error exit semantics. An eighth suite
proves strict WebSocket authentication, required subprotocol negotiation, and
cursor parsing; the TLS server suite also proves revisioned endpoint-redacted
snapshots and explicit future-cursor resynchronization. Its summary still
excludes Windows named pipes, remote GUI, and mobile clients. macOS arm64 and a
physical Linux x86_64 host pass the same eight suites, 28 tests, and 41 declared
invariants.
The shared wire-v1 transport contract is now stable: Rust rejects unknown
versioned envelope and security-relevant projection fields just as the schema
and .NET client do. Remaining Gate 6 work is mobile-client lifecycle and device
evidence, not transport-protocol maturity.

The performance shelf now has the native entrypoint
`gewyvern_validate leserpent-benchmark`. It measures fixed-size SQLite
cold start, 256-runtime query latency, 10,000-effect batch throughput, a
1,539-node UI document's generation/diff/codec costs, a 256-log full versus
8-log incremental .NET workspace comparison, and release CLI/daemon
size. Broad fail-closed budgets guard against order-of-magnitude regressions;
raw macOS arm64 and physical Linux x86_64 results remain separate host-class
baselines with machine-readable evidence.
The .NET benchmark workload and every .NET parity suite now use proof-local
artifacts roots as well. A concurrent parity/benchmark run passes without
source-tree `obj` contention and removes both intermediate graphs afterward.
The Rust-to-.NET vertical's nested `dotnet run` processes also share a
test-local artifacts root that is removed automatically, closing the inner
process boundary that the outer parity harness cannot configure directly.

The current command-origin and recovery shelf now has the native entrypoint
`gewyvern_validate leserpent-parity-recovery`. Thirteen non-vacuous suites
currently execute 231 tests across neutral command lowering, domain
authorization/idempotency, debugger confirmation, CLI/Leselang equivalence,
VM continuation/journal re-entry, runtime SQLite recovery injection, and the
72-test .NET control-plane security shelf, authenticated remote wire boundary,
native remote CLI parity, deterministic
Avalonia remote-state conformance, a real Rust-to-.NET WebSocket plus HTTPS
mutation and Inspect/History workspace vertical, and deterministic mobile
lifecycle recovery. The workspace proof rejects endpoint disclosure in both
stdout and the endpoint-bound cache after composing revision-matched queries. The
shelf retains per-suite transcripts and rejects any run whose filtered test
count falls below its declared minimum. This proves the currently migrated
command surface, shared local/remote CLI dispatch, and GUI event
delivery/reconnect/cache plus constrained runtime-refresh semantics. Future
mobile operations still require their own parity fixtures. Cross-host retained
counts are refreshed after every vertical-contract change rather than inferred
from earlier evidence. The current macOS arm64 and physical Linux x86_64 runs
both report thirteen suites, 231 tests, and 155 declared invariants. The Linux
summary additionally binds the result to kernel `6.17.0-35-generic`,
Rust/Cargo `1.95.0`, and .NET `10.0.109`.

The HTTPS listener is intentionally opt-in:

```bash
chmod 600 /etc/leserpent/tls/server.key
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpentd --database /var/lib/leserpent/runtime.sqlite \
  --remote-listen 0.0.0.0:9443 \
  --remote-cert /etc/leserpent/tls/server.crt \
  --remote-key /etc/leserpent/tls/server.key
```

The three remote arguments must be supplied together. The certificate and key
must be regular files rather than symlinks, and the private key must grant no
group or other access on Unix. Use a CA-issued certificate and deployment
network policy; the bearer token is never passed on the command line.

The native CLI uses the same endpoint without changing command syntax:

```bash
export LESERPENT_REMOTE='https://control.example.internal:9443'
export LESERPENT_REMOTE_CA='/etc/leserpent/tls/ca.pem'
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpent --json runtime list
```

CLI endpoint and CA flags are available as `--remote` and `--remote-ca`; local
socket and remote transport selection are mutually exclusive.

Separately, `leserpentd` can monitor a remote Gewyvern API as an authenticated
adapter target. The origin has no path, the CA is explicit, and the credential
is resolved from the native platform secret store by alias:

```bash
leserpentd --database /var/lib/leserpent/runtime.sqlite \
  --gewyvern-https-target \
  runtime-a=https://gewyvern-a.example.internal:9411,/etc/leserpent/gewyvern-ca.pem \
  --gewyvern-admin-secret runtime-a-admin
```

For environment-only development, `GEWY_API_ADMIN_TOKEN` supplies the same
credential boundary when no alias is selected. Remote targets refuse to start
without one of these credential sources. The existing
`--gewyvern-target ID=LOOPBACK:PORT` remains the only plain-HTTP form.

The desktop AOT shelf now has a named native entrypoint:
`gewyvern_validate leserpent-aot`. It detects only checked host RIDs, performs
the locked restore and no-restore publish, validates Mach-O/ELF signatures and
a bounded artifact manifest, executes all four control fixtures, and retains a
versioned evidence index. macOS arm64 self-host execution is automated; Linux
x64 uses the same command with Xvfb. Windows stays unclaimed until its lock and
physical-host proof exist, but that evidence is outside the current
macOS/Linux-to-Android critical path; Windows uses the TypeScript web console
meanwhile.

## Explicit Deferrals

The roadmap does not require:

- replacing Gewyvern's Linux-first runtime
- moving Etragon into the Leserpent core
- allowing only TypeScript as the web render path for browser access
- arbitrary user-native plugins
- model access to shell, raw HTTP, or host-language reflection
- a distributed scheduler before the local journal/runtime is proven

## First Implementation Slice

The first code-bearing slice should be deliberately narrow:

1. create the Rust domain/protocol crates
2. model `runtime.list` and one idempotent `runtime.refresh` command
3. lower one Leselang function into that command
4. execute it through an in-memory effect adapter
5. invoke the same command from the Rust CLI
6. compare both paths with the current Leserpent API fixture

This proves the architecture before parser breadth or GUI migration expands.
