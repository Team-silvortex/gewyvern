# gewyvern Roadmap

This roadmap treats `v2.0.x` as the active stable line.
The current shared community release is `v2.0.0`.

`gewyvern` is no longer on the road to `1.0.0` or `2.0.0`; both seals are now
release history. The current question is how to preserve the released Gewyvern
core and Rust control-plane/automation contracts while community evidence grows.

## Current Line

- last fully documented historical validation baseline: `v0.10.0`
- sealed stable baseline: `v1.0.0`
- current documented checkpoint: `v2.0.0`
- active line: `v2.0.x` maintenance, operator polish, and community validation
- released architecture:
  [docs/leserpent-2-architecture.md](docs/leserpent-2-architecture.md)
- completed delivery record and maintenance roadmap:
  [docs/leserpent-2-roadmap.md](docs/leserpent-2-roadmap.md)

For the durable minor-line record, see
[docs/history/index.md](docs/history/index.md).
For the explicit `0.15.x -> 0.20.x -> 1.0.0` spine, see
[docs/history/v0.15-to-v1-roadmap.md](docs/history/v0.15-to-v1-roadmap.md).
Machine-readable current progress is tracked by the
[project status tensor](docs/project-status-system.md), not duplicated here.

## What `v2.0.x` Means Right Now

At the current `v2.0.0` checkpoint, the active line should be interpreted as:

- a usable standalone Linux-oriented debugger/runtime with a sealed core
- a stable `gewylang` and `gewyc` surface for real package authoring
- a predictable JSON/HTML/reporting surface for operators and automation
- a project that can collaborate with nearby layers like `etragon` and
  `leserpent` without surrendering its own runtime boundaries
- a system whose protocol shelves, machine contracts, lifecycle evidence, and
  Linux-host validation paths now read as one coherent release posture

It should not be interpreted as:

- a broad observability platform
- a generic control plane
- a promise that every protocol family is fully modeled
- a license to widen core surfaces without discipline
- permission to blur stable core boundaries just because `1.0.0` is shipped

## Current Priorities

### 1. `v2.0.x`: Release Maintenance And Operating Confidence

- keep release-gate, remote-Linux, and target-lab validation paths easy to rerun
- keep startup, stop, logs, persistence, and cleanup predictable under failed
  and pathological inputs
- continue simplifying docs, CLI wording, and operator entrypoints without
  reopening machine-contract drift
- improve performance and UX where the stable core is already proven
- keep adjacent apps aligned to the same mainline version and stable boundaries

The final ten-slot `v1.20.0` through `v1.20.9` closure window completed before
`v2.0.0`. Its proof shelves remain mandatory; expanded platform/device testing
and production signing/notarization continue as post-2.0 work. The completed,
machine-validated sequence remains recorded in
[docs/leserpent-2-roadmap.md](docs/leserpent-2-roadmap.md) and
[project/release/leserpent-2-patch-seal.json](project/release/leserpent-2-patch-seal.json).

Execution shelf:

- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v1.0.0-release-notes.md](docs/history/v1.0.0-release-notes.md)
- [docs/history/v2.0.0-release-notes.md](docs/history/v2.0.0-release-notes.md)

### 2. Post-`1.0.0`: Deliberate Extension Only

- any new protocol or runtime widening should arrive with clear validation
- Linux-first debugger value should remain the center of gravity
- machine-facing changes should be treated as explicit contract management
- nearby tooling may evolve faster, but core surfaces should stay narrow

### 3. `v2.0`: Shipped Rust Leserpent And Leselang Baseline

- preserve Gewyvern as the Linux-first debugger/runtime rather than absorbing
  it into a control-plane rewrite
- keep Leserpent command, query, policy, journal, effect, and replay semantics
  authoritative in Rust
- keep Leselang a synchronous functional language driven by typed,
  journaled effect suspension and re-entry
- preserve Rust CLI, Avalonia GUI, web clients, and model-generated Leselang as
  atomically replaceable through one command/query protocol
- retain the 1.x ASP.NET and TypeScript implementation as a tested migration
  bridge; prohibit a big-bang rewrite

The exact contracts and ordered gates are maintained in:

- [docs/leserpent-2-architecture.md](docs/leserpent-2-architecture.md)
- [docs/leserpent-2-roadmap.md](docs/leserpent-2-roadmap.md)

## Recently Closed Or Historical Lines

### `v0.20.x`: Final Pre-`1.0` Seal

- froze the core release story
- established repeatable release-gate entrypoints
- closed the last broad convergence loop before stable release

### `v0.19.x`: Integrated Debugger And Freeze Preparation

- aligned runtime, docs, and debugger behavior into one integrated story
- hardened reliability, cross-validation, and lifecycle evidence
- reduced accidental surface drift before the sealing line

### `v0.18.x`: Protocol Breadth And Runtime Confidence

- broadened the packaged protocol catalog into a standard-library-like shelf
- validated package-style and physical-host runtime behavior
- hardened pathological container and ingest-failure paths
- handed off to `v0.19.x` with protocol breadth ready to feed integrated
  debugger behavior

### `v0.15.x`: Operationalization

- runtime layout, config, state, and upgrade handling
- logging and operator-triage discipline
- packaged/container/runtime validation as routine practice

### `v0.16.x`: Contract Tightening

- stable event naming and logging discipline
- narrower machine/API/report contract wording
- config schema/version migration rules
- clearer stable-versus-evolving boundaries

Execution shelf:

- [docs/history/v0.16.x-checklist.md](docs/history/v0.16.x-checklist.md)

## Historical Milestones

Earlier lines still matter as reference points:

- `v0.6.x`
  stabilized package and language composition rules
- `v0.7.x`
  improved module-level diagnosis quality
- `v0.8.x`
  hardened operations and performance
- `v0.9.x`
  narrowed the surfaces operators depend on
- `v0.10.0`
  established the last fully documented early validation baseline

Those lines are part of the project's history, but they are no longer the
current release story.

From `v0.13.x` onward, that history is now tracked through explicit minor-line
snapshot pages rather than only through scattered posture notes. Start with:

- [docs/history/index.md](docs/history/index.md)
- [docs/history/v0.13.x.md](docs/history/v0.13.x.md)
- [docs/history/v0.14.x.md](docs/history/v0.14.x.md)
- [docs/history/v0.15.x.md](docs/history/v0.15.x.md)

## Exit Criteria For The `1.0.x` Era

The current stable era should keep converging toward:

- clearer protocol/module conclusions
- safer and more explicit extensibility boundaries
- stronger packaging and container confidence
- better compiler/package authoring ergonomics
- cleaner documentation entry points and reference surfaces
- a stable core that survives ordinary growth without losing discipline

If a proposed change does not improve one of those areas, it should face a
higher bar before entering the stable line.

## Guiding Principle

The path forward should bias toward:

- better diagnosis
- clearer contracts
- steadier runtime behavior
- stronger integration evidence
- less accidental surface area

and not toward novelty for its own sake.
