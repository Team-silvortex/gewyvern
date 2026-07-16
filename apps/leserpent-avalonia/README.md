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
  apps/leserpent-avalonia/src/Leserpent.RendererCore/Leserpent.RendererCore.csproj

dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.RendererCore/Leserpent.RendererCore.csproj \
  --no-build -- \
  apps/leserpent-avalonia/fixtures/renderer-conformance-v1.json
```

The renderer rejects payloads above 2 MiB, unknown JSON members, schema or
revision drift, malformed patch shapes, duplicate IDs, cyclic moves, invalid
localized text, unlabelled actions, and runtime-binding mismatches. It mounts
the previous document, applies every incremental operation, and compares its
semantic tree with the Rust-produced next document.

`Leserpent.RendererCore` owns no command, persistence, transport, endpoint, or
adapter logic.

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
to `Yielded` without exposing continuation tokens or local values:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build -- --verify-controls \
  apps/leserpent-avalonia/fixtures/renderer-debugger-conformance-v1.json
```

Its expected output includes `nodes=46`, `operations=6`,
`initial_unrealized_nodes=40`, `remaining_unrealized_nodes=18`, and
`revision=2`.

Fleet columns now own the window viewport through an active
`VirtualizingStackPanel`; history sections receive a separate bounded 360px
viewport. This removes the outer `ScrollViewer` pattern that would otherwise
disable nested virtualization. The renderer eagerly constructs nothing below
an unrealized virtual item: its renderer-neutral subtree remains
fully patchable in the stable-ID model, and the container shell plus descendants
are created only when the compiled-bound item enters the viewport. AOT
packaging and mobile shells remain later Gate 4 slices.
