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
runtime card, workspace, section, history, and action nodes map to semantic
Avalonia controls. Stable node IDs and accessibility metadata map to Avalonia
Automation properties. Buttons only emit their action node ID; command lowering
remains in the shared Rust boundary.

Compiled bindings, large-list virtualization, live incremental control updates,
AOT packaging, and mobile shells remain later Gate 4 slices.
