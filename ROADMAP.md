# gewyvern Roadmap

This roadmap now treats `v1.4.0` as the current operational line.

`gewyvern` is no longer trying to prove it deserves `1.0`. That milestone is
behind us. The current question is how to keep the `1.x` line useful, stable,
and disciplined while protocol depth, compiler ergonomics, and collaboration
surfaces continue to mature.

## Current Line

- last fully documented historical validation baseline: `v0.10.0`
- current release line: `v1.4.0`
- immediate follow-on line: `v1.5.x`
- later decision point: consider `v2.0` only if the DSL/runtime/report
  contract needs a deliberate breaking reset

## What `v1.4.0` Means

`v1.4.0` should be interpreted as:

- a usable standalone debugger/runtime
- a stable-enough `gewylang` and `gewyc` surface for real package authoring
- a predictable JSON/HTML/reporting surface for operators and automation
- a project that can collaborate with nearby layers like `etragon` and
  `leserpent` without surrendering its own runtime boundaries

It should not be interpreted as:

- a broad observability platform
- a generic control plane
- a promise that every protocol family is fully modeled
- a license to widen core surfaces without discipline

## Current Priorities

### 1. Protocol Quality Over Protocol Vanity

- deepen existing protocol families before adding shallow new ones
- improve healthy-path and failure-path modeling together
- keep pushing from “protocol matched” toward “which module failed, where, and
  why”

### 2. Compiler And Package Ergonomics

- keep `gewylang` function/package composition predictable
- continue lightweight safety-oriented type boundaries
- improve module provenance, graph output, and explain surfaces without growing
  a heavyweight static type system

### 3. Reporting And Machine Surfaces

- keep the diagnosis spine stable and readable
- keep HTML and JSON outputs aligned
- preserve additive sidecar/extensibility semantics rather than leaking
  implementation-specific assumptions into the core contracts

### 4. Cross-Project Collaboration

- keep `gewyvern` viable on its own
- let `etragon` enrich rather than replace diagnosis
- let `leserpent` orchestrate rather than dictate runtime internals

### 5. Operational Discipline

- keep container/package validation green
- keep multi-instance and readiness races visible in integration harnesses
- preserve conservative defaults around ingest, serve, and external analysis

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

## Exit Criteria For The `1.4.x` Line

The `1.4.x` line should keep converging toward:

- clearer protocol/module conclusions
- safer and more explicit extensibility boundaries
- stronger packaging and container confidence
- better compiler/package authoring ergonomics
- cleaner documentation entry points and reference surfaces

If a proposed change does not improve one of those areas, it should face a
higher bar before entering the current line.

## Guiding Principle

The path forward should bias toward:

- better diagnosis
- clearer contracts
- steadier runtime behavior
- stronger integration evidence
- less accidental surface area

and not toward novelty for its own sake.
