# Documentation Map

This page is the single entry point for the `gewyvern` documentation set.

If you are not sure where to start, do not browse `docs/` file by file. Start
here, pick one track, and only drill into specialist pages when you need them.

This page is the global map.

Use it when you want:

- the top-level project shelves
- the main durable docs
- the shortest route to the right book volume

If you want to read the project like a technical book, jump quickly to:

- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)

## The Short Path

For most readers, the right order is:

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
   Product status, CLI entrypoints, protocol coverage, and current release
   posture.
2. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
   System-level layering and the main runtime/compiler boundaries.
3. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
   Operator-facing first-use path for single-target scans, sweeps, reports,
   and summary reading.
4. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
   `gewylang`, package shape, compiler usage, and the preferred stable subset.
   Pair it with
   [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
   when you want the full language-doc system map.
5. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
   Day-to-day contributor workflow, test layout, and where to land changes.

That set is the intended “main shelf”. Everything else below is supporting
material.

## Book Framework

For a more structured reading experience, use the book-style tracks:

- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  Entry point for the full documentation spine and storyline.
- [docs/book/tutorials.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorials.md)
- [docs/book/how-to.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
- [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
- [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)
- [docs/book/conventions.md](/Users/Shared/chroot/dev/gewyvern/docs/book/conventions.md)
- [docs/book/structure.md](/Users/Shared/chroot/dev/gewyvern/docs/book/structure.md)
- [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)
  Design sheet for the documentation set itself.

## Core Docs

If you only want the durable top-level project shelves, read these first:

- [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
- [docs/architecture-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-evolution.md)
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
  System map for `gewylang` tutorial, guide, reference, IR, and rationale
  shelves.
- [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
  Evolution sheet for the `gewylang -> IR -> runtime` implementation spine.
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

Release/evidence pages live beside them:

- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)
- [docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md)

## Runtime Internals

- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
  Project-level architecture sheet for layers, boundaries, and evolution.
- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
  Source-cluster dependency and ownership blueprint.
- [docs/architecture-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-evolution.md)
  Design-evolution sheet for the `gewylang -> IR -> runtime -> export` spine.
- [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)
  Coordination sheet across protocol, IR, runtime, and collaboration lines.
- [docs/architecture-walkthrough-http-request.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-walkthrough-http-request.md)
  One end-to-end sample path from packaged protocol entry to sidecar/control-plane consumption.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Source ownership map across the main runtime modules.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  Long-lived `--serve`, API, and degraded-mode expectations.
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  Runtime-pipeline deep dive.
- [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)
  Fragment model and attach/runtime semantics.
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
  `ExportBundle` and replay/export shape.
- [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
  One concrete `.gewy -> binding -> runtime -> export` path.

## Operator Semantics

- [docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md)
  How to read `process_network_profiles`.
- [docs/failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/failure-semantics.md)
  Meaning of `failure_mode`, `failure_detail`, `confidence`, and `basis`.
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)
  Trust labels, advisory ingest, and PID attribution caution.
- [docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md)
  Primary CLI/report/analysis contract candidate and explicit non-contract
  areas.
- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
  Narrow machine-facing contract for `summary.json`, `analysis.json`, and API
  target routing.
- [docs/book/how-to-security-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-security-checklist.md)
  Operator preflight for API exposure, ingest trust, external-engine wiring,
  and registry-root safety.

## Compiler And Tooling

For a reading-order-first path through this area, prefer:

- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
- [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
- [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)

- [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
  Reading order and page-placement map for the `gewylang` documentation set.
- [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
  Long-lived roadmap for how source, frontend, lowering, and runtime should
  mature together.
- [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
  Narrative bridge between package/frontend structure and lowered IR/archive
  surfaces.
- [docs/book/explanation-protocol-package-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-protocol-package-spine.md)
  System bridge from protocol family/entry resolution into packaged language,
  IR, runtime, and export surfaces.
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
  JSON guide for `gewyc frontend --json` and `gewyc explain --json`.
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  Exact lookup page for lowered IR shape, `ir_lowering_delta`, and per-model
  summaries.
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Exact lookup page for protocol family/entry resolution, aliases, and
  registry behavior.
- [docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf)
  Draft formal grammar for the preferred pipeline surface.

## Extensibility And Performance

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  Minimal contract for append-only external analysis engines.
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
  How nearby sidecars stay additive instead of replacing the diagnosis spine.
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)
  Current ignored-benchmark medians and comparison workflow.
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
  Linux/eBPF validation path.
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
  Native Linux packaging layout and DEB/RPM packaging entrypoints.

## Release History

- [docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md)
  Minor-line snapshots starting at `v0.13.x`, plus the compact release-line
  ledger.
- [docs/history/v0.10.0.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.10.0.md)
  Last fully documented early validation baseline.
- [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md)
  First deliberate convergence line.
- [docs/history/v0.14.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.14.x.md)
  Current active `0.14.x` maturity line.
- [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md)
  Reserved next minor-line slot.

## Fixtures And Reference Assets

- [docs/fixtures/external_engine_input_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_input_example.json)
  Minimal external-engine input example.
- [docs/fixtures/external_engine_output_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_output_example.json)
  Minimal external-engine output example.

## Scope

This page is intentionally a map, not a second table of contents for every
chapter in the book.

Use:

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
  when you want the global shelf map
- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  when you want the reading order
- [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)
  when you want the design rules behind the docs themselves
