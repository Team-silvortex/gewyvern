# Source Module Boundaries

This page is the durable source-layering note for `gewyvern`.

It describes the current `src/` layering as the project converges on a more
stable release line.

The goal is simple:

- keep `main.rs` as an entry and orchestration layer
- keep reporting and diagnosis policy out of the CLI entrypoint
- keep long-lived socket service behavior isolated from short-lived CLI runs
- keep shared formatting helpers small and reusable

This is not a public stability guarantee yet, but it is the intended internal
shape we want to preserve while the project matures.

## Companion Shelves

Read this page alongside:

- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
  for the source-cluster dependency picture
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
  for the broader layered system map
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  for runtime semantics rather than file placement

## Top-Level Shape

The current top-level runtime split is:

- `src/main.rs`
  CLI parsing, mode selection, top-level execution orchestration, and wiring
  between the major runtime subsystems.
- `src/external_analysis.rs`
  Runtime hook for append-only external analysis engines, bounded process
  execution, augmentation parsing, and integration into analysis snapshots.
- `src/data_api.rs`
  The latest-snapshot read-only API service, target route handling, target path
  encoding/decoding, and HTTP response generation for operator-facing data
  access.
- `src/serve_runtime.rs`
  Long-lived `--serve` lifecycle handling, socket session loops, scan/session
  branching, and output/API snapshot refresh orchestration.
- `src/report_runtime.rs`
  Summary, findings, scan report, HTML report, and HTTP transaction rendering.
- `src/diagnosis_runtime.rs`
  Process profile synthesis, failure semantics, competing hypotheses, and
  target-level primary diagnosis derivation.
- `src/render_utils.rs`
  Small shared formatting/render helpers used across reporting and API layers.

Alongside those entry-facing modules, the core modeling and runtime substrate is
still centered on:

- `src/dsl.rs`
  `gewylang` parsing, lowering, and package/project loading.
- `src/ir.rs`
  Shared IR semantics for predicates, signals, phase kinds, observation scope,
  transport matching, and required fact dependencies.
- `src/runtime.rs`
  Runtime event interpretation, finding generation, and flow-state evaluation.
- `src/fragment.rs`
  Fragment capability modeling and diagnostics support.
- `src/export.rs`
  Export/import JSON contracts for facts, flows, findings, and report data.
- `src/flow.rs`
  Flow-level abstractions, module-kind mapping, and stage identity helpers.
- `src/program.rs`
  Program-flow assembly and stage materialization.
- `src/protocol_profiles.rs`
  Protocol registry discovery, alias handling, and built-in protocol selection.
- `src/socket_input.rs`
  Advisory socket ingest, bounded fact collection, and listener setup helpers.
- `src/http.rs`
  HTTP/HTTP3 transaction composition helpers.

## Boundary Rules

The intended boundary rules are:

1. `main.rs` wires subsystems together; it should not own report formatting,
   diagnosis policy, or long-lived socket loops.
2. `serve_runtime.rs` may call into report and API layers, but it should not
   define diagnosis semantics itself.
3. `report_runtime.rs` may consume diagnosis summaries, but it should stay
   focused on rendering and output shaping rather than deriving runtime
   evidence.
4. `diagnosis_runtime.rs` owns interpretation policy such as
   `failure_mode/detail/confidence/basis`, ambiguity handling, competing
   hypotheses, and process-profile synthesis.
5. `external_analysis.rs` may append machine-facing augmentations, but it
   should not redefine the built-in diagnosis contract or become a second
   reporting layer.
6. `data_api.rs` exposes already-produced results; it should not become a
   second execution engine.
7. `render_utils.rs` should remain small, pure, and reusable. It is not a home
   for orchestration or policy logic.
8. `dsl.rs`, `ir.rs`, `runtime.rs`, `fragment.rs`, and `export.rs` remain the
   core compiler/runtime substrate and should stay usable outside the CLI entry
   path.

## Dependency Direction

At a high level, the intended dependency flow is:

```text
main
  -> external_analysis
  -> serve_runtime
  -> report_runtime
  -> diagnosis_runtime
  -> data_api

serve_runtime
  -> report_runtime
  -> data_api

report_runtime
  -> diagnosis_runtime
  -> render_utils

data_api
  -> render_utils

diagnosis_runtime
  -> external_analysis
  -> flow / export / runtime-facing data types
```

The core substrate remains below that layer:

```text
dsl -> ir -> runtime / fragment / export / flow / program
```

This is intentionally approximate, not a formal module system. The important
part is the direction: entry and operator-facing layers should depend on core
runtime semantics, not the other way around.

## When To Touch Which File

As a quick rule of thumb:

- If the change is about CLI flags, mode selection, or top-level wiring, start
  in `src/main.rs`.
- If the change is about socket-service lifecycle or per-session orchestration,
  start in `src/serve_runtime.rs`.
- If the change is about JSON/text/HTML output shape, start in
  `src/report_runtime.rs`.
- If the change is about which diagnosis the user should see, start in
  `src/diagnosis_runtime.rs`.
- If the change is about read-only data export over the API port, start in
  `src/data_api.rs`.
- If the change is about calling a sibling analysis engine or merging external
  augmentations, start in `src/external_analysis.rs`.
- If the change is about shared formatting helpers, start in
  `src/render_utils.rs`.
- If the change is about the language, IR, fragments, facts, or runtime event
  semantics, start in `src/dsl.rs`, `src/ir.rs`, `src/fragment.rs`,
  `src/runtime.rs`, or `src/export.rs`.

## Near-Term Intent

The current intent is not another large redesign. It is to keep
this split stable while:

- reducing `main.rs` further when a clearly separable subsystem appears
- tightening subsystem contracts instead of re-merging logic into the entrypoint
- continuing to document operator-facing behavior and internal runtime
  boundaries in parallel

If a future refactor makes one of these files disappear, that is fine. The
important part is preserving the layering principles above.
