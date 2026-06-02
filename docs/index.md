# Documentation Map

This page is the single entry point for the `gewyvern` documentation set.

If you are not sure where to start, do not browse `docs/` file by file. Start
here, pick one track, and only drill into specialist pages when you need them.

## The Short Path

For most readers, the right order is:

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
   Product status, CLI entrypoints, protocol coverage, and current release
   posture.
2. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
   System-level layering and the main runtime/compiler boundaries.
3. [docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md)
   Operator-facing usage patterns for single-target scans, sweeps, reports, and
   summary reading.
4. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
   `gewylang`, package shape, compiler usage, and the preferred stable subset.
5. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
   Day-to-day contributor workflow, test layout, and where to land changes.

That set is the intended “main shelf”. Everything else below is supporting
material.

## Core Docs

- [docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md)
  Short release-readiness checklist for the current pre-`1.0` preparation
  path.
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
  Real-world validation matrix for the current pre-`1.0` line.
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
  Short record of what real validation has already shown in practice.
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
  Standalone security boundary, ingest trust posture, and what `gewyvern`
  should not be treated as before the `v0.13.0` preparation line closes.
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
  High-level system map and stable architecture boundaries.
- [docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md)
  Fastest operator path from command invocation to reading conclusions.
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Language guide for `.gewy`, package composition, predicates, and compiler
  usage.
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
  TDD workflow, test map, and contributor habits.

## Runtime Internals

- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  `src/` ownership map: `main`, `serve_runtime`, `report_runtime`,
  `diagnosis_runtime`, `data_api`, and `external_analysis`.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  Restart, failure, degraded-mode, and latest-snapshot expectations for
  `--serve`, API, and external-engine hook operation.
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  Runtime-pipeline deep dive.
- [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)
  Fragment model, capabilities, and attach/runtime semantics.
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
  `ExportBundle` and replay/export shape.
- [docs/walkthrough.md](/Users/Shared/chroot/dev/gewyvern/docs/walkthrough.md)
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

## Compiler And Tooling

- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
  JSON guide for `gewyc frontend --json` and `gewyc explain --json`.
- [docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf)
  Draft formal grammar for the preferred pipeline surface.

## Extensibility And Performance

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  Minimal contract for append-only external analysis engines.
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
  How `gewyvern` exposes additive nearby sidecar context without surrendering
  the core diagnosis spine.
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)
  Current ignored-benchmark medians and comparison workflow.
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
  Linux/eBPF validation path.
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
  Native Linux packaging layout and DEB/RPM packaging entrypoints.

## Fixtures And Reference Assets

- [docs/fixtures/external_engine_input_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_input_example.json)
  Minimal external-engine input example.
- [docs/fixtures/external_engine_output_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_output_example.json)
  Minimal external-engine output example.
