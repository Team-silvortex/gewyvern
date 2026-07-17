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
dotnet build \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj

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
entry. It loads a bounded non-secret connection profile from local application
data and opens the remote console when the matching Keychain/Secret Service
token exists. First launch or profile failure opens an accessible setup window
for the HTTPS authority, CA file, and an optional protected token. A submitted
token is validated and stored under the canonical HTTPS origin in macOS
Keychain or Linux Secret Service; leaving it blank reuses an existing platform
credential. The input is cleared immediately after submission. The profile is
atomically written with private permissions and never contains a token, and no
token enters cache or UI IR; fixture paths remain test-only entrypoints. Verify
the persistence contract with `--verify-desktop-profile` and the real setup
controls with `--verify-desktop-connect-controls`.

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
3.841-contrast color.

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
without requiring a UI or network service. The semantic workspace projection can be checked separately
with `--verify-remote-workspace` on the Avalonia project. Against an authorized
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

The desktop shell has a checked NativeAOT profile. Restore the complete locked
RID graph first, then publish for the current host RID without another restore.
Do not cross-compile platform UI dependencies:

```bash
RID=osx-arm64 # or linux-x64

dotnet restore \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot --locked-mode

dotnet publish \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot -r "$RID" --no-restore \
  -o "artifacts/leserpent-avalonia/$RID"
```

On macOS, turn that flat NativeAOT output into a Finder/Dock application with
the native Rust bundler:

```bash
cargo run --bin gewyvern_leserpent_bundle -- \
  --publish-dir artifacts/leserpent-avalonia/osx-arm64 \
  --output artifacts/leserpent-avalonia/Leserpent.app \
  --version 1.2.0
```

The bundler emits a deterministic `Contents/MacOS`, `Contents/Resources`, and
`Info.plist` layout with bundle identifier `org.gewyvern.leserpent`. It copies
only the main executable and native `.dylib` dependencies, omits `.pdb` and
`.dSYM`, rejects symlinks and unknown files, and refuses to replace an existing
bundle. `leserpent-icon.icns` is generated from the checked Leserpent artwork.
The product uses a native macOS application menu, explicit Quit, and Dock
reopen behavior; verify its code-only contract with
`--verify-desktop-lifecycle`. Developer ID signing and Apple notarization are
separate release steps and are not implied by local ad-hoc signing.

The native release gate signs nested dylibs before the application, requires a
`Developer ID Application:` identity, enables Hardened Runtime and a secure
timestamp, and rejects symlinks or another bundle identity:

```bash
cargo run --bin gewyvern_leserpent_release -- sign \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --identity 'Developer ID Application: ORGANIZATION (TEAMID)'
```

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

The macOS arm64 proof produces a native arm64 Mach-O executable. Its `.app`
contains the executable, three native libraries, a valid plist, and the native
icon; the current stripped bundle is approximately 40 MiB before release
signing. The Ubuntu x86_64 physical-host
proof produces a five-file, approximately 76 MiB directory and a stripped PIE
ELF; all four control fixtures pass under Xvfb. The debugger fixture records one
realized cancel button before re-entry and zero afterward on both hosts. Other
desktop RIDs must publish and execute this smoke on their own operating system
before they are considered proven.
