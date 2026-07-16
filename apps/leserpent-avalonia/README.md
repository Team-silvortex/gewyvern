# Leserpent Avalonia Renderer

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

The smoke fixture mounts revision 3, then applies remove, update, move, and
insert operations directly to the mounted control tree. Unchanged and moved
controls retain object identity, while a semantic candidate and stable-ID index
fence every visual commit. Its expected output includes `nodes=15`,
`operations=4`, `reused=1`, `virtualized=1`, `active_virtualized=1`,
`initial_unrealized_nodes=14`, and `revision=4`. The low pre-mount reuse count
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
and `revision=2`.

Fleet columns now own the window viewport through an active
`VirtualizingStackPanel`; history sections receive a separate bounded 360px
viewport. This removes the outer `ScrollViewer` pattern that would otherwise
disable nested virtualization. The renderer eagerly constructs nothing below
an unrealized virtual item: its renderer-neutral subtree remains
fully patchable in the stable-ID model, and the container shell plus descendants
are created only when the compiled-bound item enters the viewport. Mobile
shells remain a later Gate 4 slice.

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

The checked RID set is currently `osx-arm64;linux-x64`. NativeAOT runtime,
compiler, linker, targeting, and app-host packs are fixed to one patch version
in the project so hosts with different .NET SDK patches consume the same lock.

Run the published executable through the same control smoke fixture:

```bash
artifacts/leserpent-avalonia/osx-arm64/Leserpent.Avalonia \
  --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-debugger-conformance-v1.json
```

The macOS arm64 proof produces a five-file, approximately 82 MiB self-contained
directory and a native arm64 Mach-O executable. The Ubuntu x86_64 physical-host
proof produces a five-file, approximately 76 MiB directory and a stripped PIE
ELF; all four control fixtures pass under Xvfb. The debugger fixture records one
realized cancel button before re-entry and zero afterward on both hosts. Other
desktop RIDs must publish and execute this smoke on their own operating system
before they are considered proven.
