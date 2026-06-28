# gewyvern Roadmap

This roadmap now treats `v0.18.x` as the active protocol/runtime checkpoint, and
`v0.20.x` as the deliberate final pre-`1.0` seal.

`gewyvern` is no longer trying to justify itself through a vague distant
milestone. The current question is how to move through `0.18.x` to `0.20.x`
with enough discipline that `v1.0.0` can come directly after a healthy
`0.20.x` close.

## Current Line

- last fully documented historical validation baseline: `v0.10.0`
- current release line: `v0.18.x`
- current documented checkpoint: `v0.18.x` protocol/runtime confidence
- planned final pre-`1.0` line: `v0.20.x`
- target next major: `v1.0.0`
- later decision point: consider `v2.0` only if the DSL/runtime/report
  contract eventually needs a deliberate breaking reset

For the durable minor-line record, see
[docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md).
For the explicit `0.15.x -> 0.20.x -> 1.0.0` spine, see
[docs/history/v0.15-to-v1-roadmap.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15-to-v1-roadmap.md).

## What `v0.18.x` Means Right Now

At the current `v0.18.x` protocol/runtime checkpoint, the active line should be
interpreted as:

- a usable standalone debugger/runtime
- a stable-enough `gewylang` and `gewyc` surface for real package authoring
- a predictable JSON/HTML/reporting surface for operators and automation
- a project that can collaborate with nearby layers like `etragon` and
  `leserpent` without surrendering its own runtime boundaries
- a system whose protocol clusters, IR surfaces, debugger publication surfaces,
  and federation-learning posture are now being closed into one coherent
  handoff

It should not be interpreted as:

- a broad observability platform
- a generic control plane
- a promise that every protocol family is fully modeled
- a license to widen core surfaces without discipline
- permission to add late-line breadth without evidence, docs, and contract
  posture

## Current Priorities

### 1. `v0.18.x`: Protocol Depth And Runtime Confidence

- deepen priority protocol families
- strengthen grouped validation and runtime confidence
- improve multi-instance and Docker-backed evidence
- keep pushing from "protocol matched" toward "which module failed, where, and
  why"

Execution shelf:

- [docs/history/v0.15-to-v1-roadmap.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15-to-v1-roadmap.md)

### 2. `v0.19.x`: Integration And Freeze Preparation

- align docs, CLI, reports, and API wording
- settle compatibility aliases and stale shims
- reduce naming drift
- confirm cross-project boundaries with `etragon` and `leserpent`

### 3. `v0.20.x`: Final Pre-`1.0` Seal

- final security and boundary review
- final surface freeze judgment
- final documentation-book coherence pass
- final packaged/container/runtime release validation pass
- no casual widening of core surfaces

## Recently Closed Or Historical Lines

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

- [docs/history/v0.16.x-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.16.x-checklist.md)

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

- [docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md)
- [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md)
- [docs/history/v0.14.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.14.x.md)
- [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md)

## Exit Criteria For The Road To `v1.0.0`

The remaining `0.x` lines should keep converging toward:

- clearer protocol/module conclusions
- safer and more explicit extensibility boundaries
- stronger packaging and container confidence
- better compiler/package authoring ergonomics
- cleaner documentation entry points and reference surfaces
- a stable core that can survive the jump to `v1.0.0`

If a proposed change does not improve one of those areas, it should face a
higher bar before entering the remaining pre-`1.0` lines.

## Guiding Principle

The path forward should bias toward:

- better diagnosis
- clearer contracts
- steadier runtime behavior
- stronger integration evidence
- less accidental surface area

and not toward novelty for its own sake.
