# Leserpent Avalonia Renderer

<p align="center">
  <img src="../../assets/branding/leserpent-icon.png" alt="Leserpent feathered serpent icon" width="220">
</p>

This directory is the replaceable .NET renderer line for the Rust
`leselang-ui` contract. It contains both the strict semantic renderer core and
the first Avalonia desktop control shell.

## Conformance

Rust generates the authoritative version-1 fixture:

```bash
cargo run --quiet -p leselang-ui --example render_conformance_fixture -- \
  apps/leserpent-avalonia/fixtures/renderer-conformance-v1.json
```

Build and execute the renderer conformance check:

```bash
dotnet build \
  apps/leserpent-avalonia/src/Leserpent.RendererConformance/Leserpent.RendererConformance.csproj

dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.RendererConformance/Leserpent.RendererConformance.csproj \
  --no-build -- \
  apps/leserpent-avalonia/fixtures/renderer-conformance-v1.json
```

The renderer rejects payloads above 2 MiB, unknown JSON members, schema or
revision drift, malformed patch shapes, duplicate IDs, cyclic moves, invalid
localized text, unlabelled actions, and runtime-binding mismatches. It mounts
the previous document, applies every incremental operation, and compares its
semantic tree with the Rust-produced next document.

`Leserpent.RendererCore` is a pure library and owns no command, persistence,
transport, endpoint, adapter, or process-entry logic. The separate
`Leserpent.RendererConformance` executable owns only bounded fixture loading and
semantic equality checks.

## Desktop control slice

Build the Avalonia 12 desktop shell and verify its real control tree without
leaving a window open:

```bash
dotnet restore \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --locked-mode

dotnet build \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-restore

dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build -- --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-conformance-v1.json
```

Omit `--verify-controls` to open the desktop window. Column, heading, text,
runtime card, workspace, section, history, log, debugger, and action nodes map to semantic
Avalonia controls. Stable node IDs and accessibility metadata map to Avalonia
Automation properties. Buttons only emit their action node ID; command lowering
remains in the shared Rust boundary.

Launching `Leserpent.Avalonia` without arguments is the normal desktop product
entry. It opens a topology Hub rooted at the desktop client. Local Orchestra and
every saved `leserpentd` authority are separate daemon cards; opening a card
creates or reuses that authority's independent session window without closing
the Hub or another daemon session. The Hub loads a bounded non-secret connection
catalog from local application data, and old single-profile state is migrated
atomically on first use. `+ Add daemon` creates another authority branch, while
each remote card has independent Open and Manage actions. Every daemon card
performs a bounded read-only `runtime_list` query and renders up to six owned
gewyvern runtime children with live revision and refresh state. Topology loading
is limited to four concurrent authorities, is cancelled with the Hub window,
and falls back only to the endpoint-bound private snapshot cache. While the Hub
is open, cards refresh every 30 seconds without overlapping a card's active
query. A renderer-neutral state machine distinguishes live, cached, retained,
and unavailable topology: a transient failure keeps the last child tree visibly
stale instead of deleting it, and a response with an older revision is rejected.
Each live refresh composes a strict authority `health` proof with `runtime_list`
in parallel. A card cannot become live unless the daemon is ready, owns the
protocol-v1 authority, and reports internally consistent queue counters. Queue
pressure is visible on the card; cached snapshots never invent missing health.
Verify the
strict wire projection with `--verify-remote-topology` and the real Hub control
tree with `--verify-hub-topology`.

`Deploy daemon` opens the native reverse-deployment workspace. It selects one
saved authenticated daemon as the deployment authority, accepts only a target,
SSH port, stable bootstrap ID, and opaque `vault:ssh:*` handle, and requires an
explicit confirmation before calling `POST /v1/bootstrap`. The workspace polls
the public handoff over ordinary wire-v1 and enables `Verify & bind session`
only after the authority publishes `Bootstrapped`. Every operation revalidates
the selected catalog entry and reloads its platform credential; raw passwords,
private keys, session tokens, and CA material never enter the form or bootstrap
IR. Verify the strict client codec with `--verify-bootstrap-client` and the real
control sequence with `--verify-bootstrap-controls`.

On macOS and Linux, set `LESERPENT_BOOTSTRAP_CONFIG` to the absolute private
origin-config path before launching the desktop app to make Local Orchestra a
deployment authority. The managed local daemon receives that config plus a
private app-owned trust root. After server-verified binding, `Add to Hub` reads
the endpoint-bound CA record from that root, resolves the `vault:leserpentd:*`
session handle from the Rust-compatible platform secret schema, proves target
TLS/token health, then stores the target token and secret-free profile. Automatic
promotion currently requires the config's default secret service
`org.gewyvern.leserpent.adapters`; remote deployment authorities can complete
binding but cannot export their local trust or session stores into the desktop.
Verify the transaction with `--verify-bootstrap-promotion`.

`Provision gewyvern` opens the separate runtime-provisioning workspace for a
saved authenticated daemon authority. It requires a stable provisioning ID,
runtime ID, SSH target, opaque `vault:ssh:*` handle, and explicit confirmation
before calling `POST /v1/provisioning`; it never calls the existing
`runtime.deploy` debugging-pipeline operation. Submission locks the complete
identity, then performs at most 30 automatic observations by replaying the exact
request. Manual refresh observes that same attempt. A failed terminal state tells
the operator to remediate and choose a new provisioning ID, while a registered
state refreshes the Hub topology. Verify the strict codec with
`--verify-provisioning-client` and the native control sequence with
`--verify-provisioning-controls`.

On macOS and Linux, `LESERPENT_GEWYVERN_PROVISIONING_CONFIG` may point to the
absolute private `gewyvern-provisioning-config` file before app startup. Local
Orchestra then receives that native daemon config and appears as a provisioning
authority; without it, the Hub offers only saved daemon authorities and does not
pretend the local service can install Gewyvern.

Runtime children are direct workspace actions. Selecting one creates or reuses
its owning daemon session, but the workspace is not opened from the Hub's query
projection. The request remains bounded and pending until that daemon session
receives an authoritative event snapshot whose snapshot revision is at least the
Hub topology revision. Heartbeats, stale/cache-only state, removed runtimes, and
cross-daemon session state cannot release this fence.

A submitted connection token is validated and stored under the canonical HTTPS
origin in macOS Keychain or Linux Secret Service; leaving it blank reuses an
existing platform credential. The input is cleared immediately after
submission. The catalog is atomically written with private permissions and never
contains a token, and no token enters cache or UI IR; fixture paths remain
test-only entrypoints. Verify the persistence contract with
`--verify-desktop-profile` and the real setup controls with
`--verify-desktop-connect-controls`.

Remembered CA files are not referenced in place. Leserpent accepts exactly one
UTF-8 PEM certificate with CA basic constraints and, when present, certificate-
signing key usage. It canonicalizes the public certificate into the private
`trust-v1` application directory under its SHA-256 fingerprint, writes it
atomically with `0600` file and `0700` directory modes, and stores only that
managed path in the profile. Existing external-path profiles migrate on startup.
The fingerprint filename is revalidated on every launch, so replacing a managed
file with another valid CA fails closed. Successful migration or connection
switching prunes stale managed CAs and recognized crash-temporary files while
refusing unknown entries and links. Run `--verify-desktop-ca-store` for the full
positive and negative contract.

Profiles promoted from reverse bootstrap retain a
`vault:leserpent-ca:*` handle plus its private trust-store root instead of a CA
path. The RemoteClient reader accepts the Rust record format, rejects unknown
fields, links, non-private modes, digest replacement, and endpoint mismatch,
then imports the validated CA through the same managed store. Path and handle
sources are mutually exclusive. `--verify-desktop-profile` and
`--verify-desktop-ca-store` cover both profile modes.

After startup, `Connection...` in the native macOS application menu or the
connection button in the remote status bar reopens the same secure setup flow.
A replacement session must be constructed successfully before the current
session closes. `Forget saved connection...` requires a second confirmation,
revalidates that the on-disk profile has not changed, and removes only that
canonical endpoint's Keychain/Secret Service item plus the non-secret profile;
environment variables and other endpoint credentials are untouched. Verify the
maintenance boundary with `--verify-connection-maintenance` and both real
Avalonia control surfaces with `--verify-connection-management-controls`.

Remote startup validation failures open a bounded, token-redacted error window
instead of terminating with an unhandled exception. `Escape` and the explicit
close button exit with status 2. Its real control metadata can be checked with
`dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- --verify-startup-error`.

The connection window can test an authority before committing the profile.
`Test connection` validates the selected CA, endpoint-scoped credential,
wire-v1 protocol version, ready state, authority ownership, and effect-queue
counter consistency. The preflight does not import the CA, save the profile, or
write the credential; those side effects remain exclusive to `Connect`.

Once connected, the trust identity bar runs the same authenticated health check
and exposes current effect-queue pressure as `QUEUE active/capacity`. Operators
can refresh it without changing remote state. Saturation is shown explicitly,
uses an assertive accessibility announcement, and remains independent from
runtime mutation fences.

Every mutation confirmation also exposes its canonical Leselang equivalent.
Refresh and capability discovery show a fixed source preview; the deployment
preview updates only while all form fields are valid. `Copy Leselang` writes
that source to the clipboard without sending a request, so GUI intent can be
reviewed, versioned, or handed to a model before execution.

The remote desktop toolbar filters the local runtime projection by name, ID,
tag, or status. Input is bounded to 128 characters and debounced; `Ctrl+F` or
`Cmd+F` focuses it, Escape clears it, and no filter text is sent to the server
or written to cache. Run the deterministic projection check with
`dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- --verify-remote-filter`.
At widths below 780 pixels, the identity, filter, and connection surfaces switch
to a compact multi-row layout without hiding origin, CA, credential provenance,
revision, or reconnect controls. Verify the breakpoint contract with
`dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- --verify-remote-layout`.
Document remounts and incremental patches preserve keyboard focus by stable UI
node ID when the focused control still exists, including replacement of an
updated action control. A removed action clears the pending target rather than
transferring focus to another mutation control. Verify all paths against real
Avalonia controls with
`dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- --verify-focus-retention apps/leserpent-avalonia/fixtures/renderer-conformance-v1.json`.

Run the named accessibility shelf across all real-control fixtures:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-accessibility
```

It requires unique stable Automation IDs, complete Automation Names, explicit
labels on every action button, exact HelpText mapping, and a WCAG AA text
contrast floor of 4.5. Evidence is retained under
`target/validation/leserpent-accessibility/`. The current minimum is 4.723;
the destructive button uses `#C44D2D` with white text instead of the previous
3.841-contrast color. This managed Release shelf restores the development lock
graph; `leserpent-aot` independently runs the same four control fixtures
against the native executable restored from the AOT lock graph.

The smoke fixture mounts revision 3, then applies remove, update, move, and
insert operations directly to the mounted control tree. Each runtime card
exposes a read-only Inspect action and a separately protected Refresh action.
Unchanged and moved controls retain object identity, while a semantic candidate
and stable-ID index fence every visual commit. Its expected output includes `nodes=18`,
`operations=4`, `reused=1`, `virtualized=1`, `active_virtualized=1`,
`initial_unrealized_nodes=17`, `accessibility_controls=18`,
`accessibility_names=18`, `accessibility_actions=6`, `accessibility_help_texts=3`,
`minimum_contrast=4.723`, and `revision=4`. The low pre-mount reuse count
is intentional: only the root control exists while the patch is applied.

The bounded-history fixture proves compiled-binding materialization beyond the
first viewport:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build -- --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-workspace-conformance-v1.json
```

Its expected output includes `nodes=39`, `operations=3`,
`initial_unrealized_nodes=32`, `remaining_unrealized_nodes=12`, and
`revision=34`.

The bounded-log fixture exercises the typed sanitized-display projection and
the dedicated monospace log control:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build -- --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-log-conformance-v1.json
```

Its expected output includes `nodes=52`, `operations=3`,
`initial_unrealized_nodes=48`, `remaining_unrealized_nodes=26`, and
`revision=2`.

The debugger fixture models synchronous effect re-entry from `WaitingEffect`
to `Yielded`, including removal of its session-bound cancel action, without
exposing continuation tokens or local values:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build -- --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-debugger-conformance-v1.json
```

Its expected output includes `nodes=46`, `operations=7`,
`initial_unrealized_nodes=40`, `remaining_unrealized_nodes=18`,
`initial_debugger_cancel_buttons=1`, `remaining_debugger_cancel_buttons=0`,
`initial_accessibility_actions=1`, `accessibility_valid=true`,
and `revision=2`.

Fleet columns now own the window viewport through an active
`VirtualizingStackPanel`; history sections receive a separate bounded 360px
viewport. This removes the outer `ScrollViewer` pattern that would otherwise
disable nested virtualization. The renderer eagerly constructs nothing below
an unrealized virtual item: its renderer-neutral subtree remains
fully patchable in the stable-ID model, and the container shell plus descendants
are created only when the compiled-bound item enters the viewport. Mobile
shells remain a later Gate 4 slice.

## Remote event mode

The desktop shell can consume the authenticated `leserpentd` WebSocket event
surface directly:

```bash
export LESERPENT_PRINCIPAL='operator-a' # optional audit identity
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --remote https://leserpent.example:9443 \
  --remote-ca /absolute/path/to/leserpent-ca.pem
```

`--remote` accepts an HTTPS origin only. The client derives `/v1/events`,
requires the `leserpent.events.v1` subprotocol, verifies both the explicit CA
and TLS hostname, and never accepts plaintext or redirect fallback. An optional
`--remote-cache ABSOLUTE_PATH` overrides the per-origin cache location. The
desktop identity strip displays the canonical origin, including a non-default
port, plus a short CA SHA-256 fingerprint; its tooltip and automation metadata
expose the complete fingerprint without exposing the CA path.

The desktop client first resolves the bearer token from the OS credential
store, keyed by the canonical HTTPS origin. Add it without placing the token in
shell history or process arguments:

```bash
# macOS: -w is deliberately last so security prompts for the value.
security add-generic-password -U \
  -s org.gewyvern.leserpent.remote \
  -a https://leserpent.example:9443 \
  -w

# Linux Secret Service: secret-tool reads the value from its prompt/stdin.
secret-tool store --label='Leserpent remote token' \
  service org.gewyvern.leserpent.remote \
  endpoint https://leserpent.example:9443
```

`LESERPENT_REMOTE_TOKEN` remains an explicit automation fallback when no
platform item exists. A present but malformed platform item fails closed rather
than silently selecting the environment. Tokens are bounded to 32-4096
non-whitespace characters and never enter snapshot cache or UI IR. The desktop
status bar always shows credential provenance without exposing the value:
Keychain, Secret Service, platform store, or a highlighted `ENV FALLBACK` badge.
The presentation contract can be checked without loading a token using
`dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- --verify-credential-source`.

The cache atomically stores only the endpoint-redacted snapshot and matching
revision cursor. Cached data is visibly marked stale until the server confirms
the cursor; malformed, oversized, cross-origin, or symlinked cache state fails
closed. Disconnects immediately mark the mounted projection stale and retry
with capped exponential delay for at most eight attempts. After the bound is
exhausted, the status bar enables an explicit `Reconnect` action; `F5` invokes
the same read-only event-stream restart while retaining the revision cursor and
stale projection. It never retries a mutation. `resync_required` clears the
cursor before requesting a complete snapshot. Each runtime card exposes typed
`runtime.inspect` and `runtime.refresh` actions. Inspect opens one reusable
child window per runtime, with an eight-window safety bound. The child requests
`RuntimeInspect`, bounded `RuntimeHistory`, and bounded `RuntimeLogs` together
over the same authenticated `/v1/wire` transport and mounts a document only when
all three responses carry the same revision and runtime identity. Logs are
limited to 256 strictly ordered entries; raw messages are limited to 64 KiB,
control characters are normalized, and display text is UTF-8 safely capped at
768 bytes. Runtime endpoints exist only in the strict wire decode DTO and are
discarded before snapshot, history, logs, UI IR, cache, or automation state is
created. A newer live event revision reloads the matching open workspace.
Because log append does not advance the control-plane revision, each workspace
also exposes a persistent Reload button and `F5` shortcut. Malformed, torn, or
mismatched query state fails closed without retaining a partial document.
`Workspace Leselang` opens one reusable preview window containing the canonical
structured equivalent of that atomic query group: named `inspect`, `history`, and
`logs` branches inside `all`. Copying the source executes nothing. The .NET
formatter output is parsed and lowered by Rust in the cross-language parity shelf,
so GUI query equivalence is a language contract rather than display-only text.
`Live logs` is an explicit opt-in five-second refresh loop over the same atomic
Inspect/History/Logs query group. It permits only one in-flight group, pauses
without losing intent while the child window is inactive, resumes on activation,
and recovers from transient query failures with bounded 10-second and 20-second
backoff. A successful query restores the normal five-second interval; three
consecutive failures turn live refresh off and require explicit operator restart.
An operator-triggered `Reload` or newer-revision workspace query also clears a
pending backoff after it succeeds and reschedules the normal five-second poll.
Every admitted full workspace query first stops the outstanding live timer, and a
single-flight `Skipped` result is backoff-neutral rather than a false success.
`Pause live` never cancels unrelated window lifetime state or retries a mutation.
After a full snapshot, live refresh uses the last retained log sequence as a
bounded `after_sequence` cursor for up to 11 polls. The twelfth poll is always a
full resynchronization. A changed workspace revision or a full 256-entry
incremental batch triggers an immediate full fallback, and stale/non-advancing
incremental records fail closed. Manual `Reload` always requests a full snapshot.
Incremental merges retain only the newest 256 sanitized entries; the status line
states whether the successful refresh used an incremental or full snapshot.
`gewyvern_validate leserpent-benchmark` measures this path in .NET Release mode
against the full 256-entry compose, enforces p50 and allocation-ratio budgets,
and retains the exact same-host measurements with the Rust runtime/UI and binary
size evidence.
Every successful manual or live refresh compares the complete retained snapshots
locally and reports revision advance, added/expired/changed logs, new or updated
commands, and log-sequence reset. Initial and unchanged snapshots are explicit.
New entries, or retained entries whose level changes to `error` or `warning`, are
counted separately in the summary. New errors use the destructive assertive live
region, while new warnings use the prominent primary status. The initial snapshot
never re-alerts historical severity, and an unchanged refresh never repeats it.
Once raised, the highest severity remains visible across later refreshes until the
operator explicitly selects `Acknowledge`. Errors cannot be downgraded by a later
warning. Only a newly observed error uses assertive announcement; a retained error
uses polite updates. Acknowledgement clears only this local alert latch and never
changes the snapshot, filters, network state, or live-refresh request.
A runtime identity change or revision regression rejects the new snapshot, keeps
the previous document mounted, and stops live refresh rather than presenting
ambiguous chronology.
The local snapshot comparator independently rechecks the 32-entry history and
256-entry log bounds, unique command identities, strictly increasing log sequence,
and the closed log-level set. This fail-closed layer remains valid even when a
future snapshot producer does not pass through the current HTTP client codec.
Each workspace also provides a local-only log search and strict level selector.
The query is control-character sanitized and capped at 128 characters, operates
only on the retained sanitized display text, and never performs a network request
or changes the revision-consistent snapshot. `Ctrl+F` or `Cmd+F` focuses search,
`Escape` clears it, and an accessible live summary reports shown versus total
entries. Empty filtered results remain distinct from an actually empty log.
History rows include their bounded command ID so an applied revision remains
traceable to its originating operation. `Copy diagnostics` explicitly exports a
deterministic `leserpent.workspace-diagnostic/v1` snapshot containing runtime
identity, revision, command history, current filter, and only the currently
visible sanitized logs. The export is capped at 512 KiB, which covers a fully
populated 256-entry snapshot after worst-case string escaping, contains no structured
transport endpoint or principal, performs no request or command, and warns the
operator to review clipboard data before sharing it. `Save diagnostics` reuses
the same bounded bytes and opens the platform save panel with a sanitized runtime
filename and explicit overwrite confirmation. Cancellation is non-error; an
unwritable or non-replaceable destination fails closed without disclosing its path
in the UI status.
Runtime and capability refresh are blocked while state is stale, require
an explicit confirmation dialog, carries the displayed runtime
revision for optimistic concurrency, and is never retried automatically after
an ambiguous network failure. Refresh controls remain disabled during
confirmation and transport. A successful runtime refresh stays fenced until its
command revision appears on the event stream. Capability discovery remains
fenced beyond the command revision until a later observed capability projection
arrives, preventing a second command from invalidating the in-flight
observation. An unknown outcome stays fenced until a later full runtime snapshot
resolves it; revision heartbeats cannot release the fence. For capability
discovery, a snapshot carrying a newer revision but the old capability posture
remains ambiguous and blocked. New projections carry
`capabilities_observed_for_revision`, so identical discovery results still
resolve the exact command without content comparison; legacy projections omit
the field and retain the conservative behavior. Disabled reasons are exposed through
the tooltip and automation help text. Operation progress and outcomes use a
separate persistent, dismissible live-region banner, so connection heartbeats
cannot overwrite them. Confirmation dialogs focus Cancel by default and Escape
always cancels. The response projection and cache omit runtime endpoints.
Mobile-specific secure storage lifecycle remains a Gate 6 requirement.

Run the transport-independent client contract check with:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj
```

It checks strict Rust event decoding, monotonic revisions, stale transitions,
the reconnect bound and cursor-preserving manual resume, resync cursor reset,
malformed-cache rejection, per-origin cache binding, atomic workspace query
composition, runtime/log identity, bounded sanitized logs, and endpoint omission
without requiring a UI or network service.
Workspace filtering, diagnostic encoding, refresh/backoff planning, snapshot
comparison, and severity retention are public, renderer-independent policies in
`Leserpent.RemoteClient`; Avalonia owns only their native controls. MobileCore
references the same library and MobileConformance executes all six policy
contracts, so mobile presentation does not need to copy desktop behavior.
Fleet and runtime-workspace `UiDocument` projection now live beside those
policies in `Leserpent.RemoteClient` and depend only on `RendererCore`.
Avalonia renders the resulting document but no longer owns its filtering,
capability, action, form, endpoint-isolation, or empty-state semantics.
Mutation revision and unknown-outcome observation fences are likewise owned by
`Leserpent.RemoteClient`. The window only presents their pending reason;
heartbeats cannot release a fence, and capability mutations require a matching
later capability observation before another remote change is enabled.
Action availability is also computed in RemoteClient. Avalonia applies the
returned mutation and inspection booleans plus their bounded reason strings to
controls and workspace windows; it does not infer permissions from button or
connection visuals.
Both existing and newly opened workspace windows receive availability only
through this policy; no live/idle shortcut may overwrite an unresolved fence.
Authority health and queue-saturation presentation also come from RemoteClient,
leaving Avalonia responsible only for color and live-region behavior.
The semantic workspace projection can be checked separately with
`--verify-remote-workspace` on the Avalonia project; use
`--verify-workspace-diagnostics` for local search, level, command identity,
bounded explicit diagnostic export, live-refresh state, severity signaling, and
empty-state contracts. The same probe verifies bounded snapshot delta summaries,
initial-severity re-alert suppression, explicit severity acknowledgement and
non-downgrade, hybrid cursor/full live-log refresh, independent snapshot
chronology and bound fences, and revision-regression rejection.
The earlier `--verify-workspace-log-filter` spelling remains compatible. Against an authorized
live server, append `--connect HTTPS_ORIGIN CA_PATH CACHE_PATH --inspect
RUNTIME_ID` to verify the complete authenticated Inspect/History/Logs path, or
use `--refresh-capabilities RUNTIME_ID` to verify the typed capability mutation.

Authenticated runtime workspaces expose `Deploy pipeline` only after the strict
capability projection advertises authenticated deployment. The form bounds the
pipeline kind and optional target, repeats runtime/revision context before
confirmation, and submits `runtime_deploy` under the separate `runtime.deploy`
capability. Request identity, principal, and confirmation are not editable.
Run the Avalonia project with `--verify-deployment-contract` to check the
source-generated JSON shape, null omission, and fail-closed input validation.
Run it with `--verify-parameterized-form` to verify the renderer-neutral form
description, typed `submit` event, field whitelist, and input constraints.

`gewyvern_validate leserpent-parity-recovery` runs this check together with an
ignored-by-default integration test that connects the .NET client to a real
Rust TLS/WebSocket authority, applies confirmed HTTPS runtime and capability
refreshes, executes the discovery adapter against a fixed loopback capability
service, and waits for the command and observation revisions on the event
stream. The same authority is then queried
through authenticated Inspect, History, and Logs calls; the proof requires
revision 4, an observed `1.2.0` capability projection, two bounded history
entries, one sanitized log entry, and endpoint-free
stdout plus cache state. macOS arm64 and physical Linux
x86_64 retain matching eleven-suite, 134-test, 79-invariant evidence for the
current vertical contract.

## Native AOT

The preferred project-level proof entry is native Rust orchestration:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-aot
```

It detects the supported host RID, performs the locked restore and no-restore
publish, validates the native executable signature and bounded package, runs
all four control fixtures, and retains machine-readable evidence under
`target/validation/leserpent-aot/`. The lower-level commands below remain useful
for packaging diagnostics.

The NativeAOT and accessibility proof commands assign separate .NET
`--artifacts-path` roots under their evidence directories. They can therefore
run concurrently without sharing project `obj`, reference assemblies, or PDBs;
successful runs remove those intermediate trees while retaining logs and final
evidence.

The desktop shell has a checked NativeAOT profile. Restore the complete locked
RID graph first, then publish for the current host RID without another restore.
Do not cross-compile platform UI dependencies:

The project deliberately keeps separate lock graphs: ordinary desktop builds
use `packages.development.lock.json`, while `PublishAot=true` selects
`packages.lock.json` with the pinned IL compiler, linker, and RID packs. This
prevents a normal IDE or probe restore from rewriting the release graph.

```bash
RID=osx-arm64 # or linux-x64

dotnet restore \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot -p:PublishAot=true --locked-mode

dotnet publish \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot -r "$RID" --no-restore \
  -o "artifacts/leserpent-avalonia/$RID"
```

On macOS, turn that flat NativeAOT output into a Finder/Dock application with
the native Rust bundler:

```bash
cargo build --release -p leserpentd --features native-ssh
cargo run --bin gewyvern_leserpent_bundle -- \
  --publish-dir artifacts/leserpent-avalonia/osx-arm64 \
  --daemon target/release/leserpentd \
  --output artifacts/leserpent-avalonia/Leserpent.app
```

The bundler emits a deterministic `Contents/MacOS`, `Contents/Resources`, and
`Info.plist` layout with bundle identifier `org.gewyvern.leserpent`. It requires
and embeds the native Rust `leserpentd` beside the Avalonia executable, copies
native `.dylib` dependencies, omits `.pdb` and `.dSYM`, rejects symlinks,
unknown files, and non-arm64 payloads, and refuses to replace an existing
bundle. The official path omits `--version`, so both bundle version fields
inherit the root Rust workspace release automatically; downstream packagers
may still override that value explicitly. `leserpent-icon.icns` is generated
from the checked Leserpent artwork.

Install, inspect, or explicitly roll back a user-local version without shell
copy wrappers:

```bash
cargo run --bin gewyvern_leserpent_install -- install \
  --app artifacts/leserpent-avalonia/Leserpent.app
cargo run --bin gewyvern_leserpent_install -- status
cargo run --bin gewyvern_leserpent_install -- rollback
```

The stable launcher is `~/Applications/Leserpent.app`; versioned bundles live
under `~/Library/Application Support/Leserpent/Installer`. The native Rust
installer preserves application data outside that directory, rejects unmanaged
or escaping links, and keeps at least two releases so rollback remains
available. Use absolute `--root` and `--launcher` overrides for an isolated
packaging proof. Local ad-hoc signing can prove copy integrity and launch, but it
does not satisfy the Developer ID or notarization gate.

The product uses a native macOS application menu, explicit Quit, and Dock
reopen behavior; verify its code-only contract with
`--verify-desktop-lifecycle`. Developer ID signing and Apple notarization are
separate release steps and are not implied by local ad-hoc signing.

No-argument desktop startup creates an app-private loopback TLS identity and an
ephemeral local-process credential, starts the bundled Rust `leserpentd`, and
exposes that authority as the Local Orchestra branch through the same remote
client used for saved daemon profiles. Managed CA pruning retains the complete
active remote catalog plus the local authority instead of treating trust as a
single global slot.
The supervisor sends SIGTERM first so Rust releases its journal lease, then uses
a bounded forced-shutdown fallback. Verify the complete start, health,
shutdown, and immediate-restart path with:

```bash
cargo build -p leserpentd --features native-ssh
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-local-orchestra target/debug/leserpentd
```

Daemon resolution fails closed to the executable beside the app; development
overrides must name an explicit regular executable and never fall back to
`PATH`. The child receives a cleared environment containing only its ephemeral
bearer token. TLS files are atomically created as `0600` inside a non-symlink
`0700` state directory, and exported private-key buffers are zeroed after use.

The native release gate signs nested dylibs before the application, requires a
`Developer ID Application:` identity, enables Hardened Runtime and a secure
timestamp, and rejects symlinks or another bundle identity:

```bash
cargo run --bin gewyvern_leserpent_release -- preflight \
  --app artifacts/leserpent-avalonia/Leserpent.app

cargo run --bin gewyvern_leserpent_release -- sign \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --identity 'Developer ID Application: ORGANIZATION (TEAMID)'
```

`preflight` emits machine-readable readiness JSON and never reads plaintext
credentials. After storing the notary profile, pass `--keychain-profile
leserpent-notary` so it can validate the profile through `notarytool history`.
The retained macOS host evidence has every required Apple tool but no Developer
ID Application identity and no requested notary profile, so it correctly
reports `release_ready=false` rather than claiming an Apple-backed release.

Store notarization credentials through the interactive Keychain prompt; never
put an Apple ID password in a command, environment variable, or repository:

```bash
xcrun notarytool store-credentials leserpent-notary

cargo run --bin gewyvern_leserpent_release -- notarize \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --keychain-profile leserpent-notary

cargo run --bin gewyvern_leserpent_release -- verify \
  --app artifacts/leserpent-avalonia/Leserpent.app
```

Notarization creates a temporary `ditto --keepParent` ZIP, waits for an
explicit `Accepted` response, removes the archive, staples and validates the
ticket, and finishes with a Gatekeeper assessment. `verify --allow-adhoc`
exists only for local Hardened Runtime structure tests and skips Developer ID
and Gatekeeper claims. Because ad-hoc code has no Team ID, a separately signed
Hardened Runtime executable and its native libraries cannot satisfy macOS
library validation; this mode therefore reports `runtime_launch=false`. Use the
ordinary ad-hoc bundle for local Finder/UI testing, and a single Developer ID
identity for every nested library plus the app in release testing.

Normal no-argument startup and release verification share one
`DesktopProductStartup` composition boundary. The packaged proof accepts only a
profile under the current user's isolated temporary directory, requires a high
loopback port and a CA under the same temporary root, refuses an existing
credential, writes a generated one-time token to the real platform Keychain,
resolves the saved profile, and deletes the item in `finally` without printing
or persisting the token. Run the packaged executable with
`--verify-packaged-profile-startup TEMP_PROFILE`; success emits
`saved_profile=true`, `platform_keychain=true`, and `credential_cleaned=true`.
Connection management uses this same product composition boundary: switching
is fail-safe against setup errors, while forgetting is endpoint-scoped and
stale-profile fenced.

The checked RID set is currently `osx-arm64;linux-x64`. NativeAOT runtime,
compiler, linker, targeting, and app-host packs are fixed to one patch version
in the project so hosts with different .NET SDK patches consume the same lock.
macOS and Linux are the supported native desktop targets for this cycle.
Android is the next native client target; iOS follows after Android parity.
Windows native desktop is intentionally deferred, with the authenticated Web
console serving Windows operators instead.

Run the published executable through the same control smoke fixture:

```bash
artifacts/leserpent-avalonia/osx-arm64/Leserpent.Avalonia \
  --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-debugger-conformance-v1.json
```

The macOS arm64 proof produces native arm64 Mach-O Avalonia and `leserpentd`
executables. Its `.app` contains both executables, the native libraries, a valid
plist, and the native icon; the daemon payload is mandatory rather than an
optional discovery result. The Ubuntu x86_64 physical-host
proof produces a five-file, approximately 76 MiB directory and a stripped PIE
ELF; all four control fixtures pass under Xvfb. The debugger fixture records one
realized cancel button before re-entry and zero afterward on both hosts. Other
desktop RIDs must publish and execute this smoke on their own operating system
before they are considered proven.
