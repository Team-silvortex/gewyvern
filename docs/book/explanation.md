# Explanation

This track is for mental models and design rationale. Use it when you want to
understand why the project is shaped the way it is.

## Core System Shape

If you want the shortest architecture reading order inside the explanation
track, use:

1. [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
2. [docs/system.md](docs/system.md)
3. [docs/architecture.md](docs/architecture.md)
4. [docs/architecture-blueprint-modules.md](docs/architecture-blueprint-modules.md)
5. [docs/module-boundaries.md](docs/module-boundaries.md)
6. [docs/architecture-evolution.md](docs/architecture-evolution.md)
7. [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
8. [docs/architecture-coordination.md](docs/architecture-coordination.md)

- [docs/documentation-system.md](docs/documentation-system.md)
  How the documentation set itself is layered and maintained.
- [GewyLang module](../modules/gewylang.md)
  Compact routing for language authoring, compiler, and migration docs.
- [docs/gewylang-evolution.md](docs/gewylang-evolution.md)
  How the `gewylang -> IR -> runtime` spine is supposed to mature.
- [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
  Project-level architecture sheet for subsystem and evolution boundaries.
- [docs/architecture-blueprint-modules.md](docs/architecture-blueprint-modules.md)
  Source-cluster dependency blueprint.
- [docs/architecture-evolution.md](docs/architecture-evolution.md)
  How the main `gewylang -> IR -> runtime -> export` spine is meant to mature.
- [docs/architecture-coordination.md](docs/architecture-coordination.md)
  How protocol, IR, runtime, and collaboration lines constrain each other.
- [docs/architecture-walkthrough-http-request.md](docs/architecture-walkthrough-http-request.md)
  One representative end-to-end architecture path through the current stack.
- [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
  How authored intent, runtime evidence, published surfaces, sidecars, and the
  control plane move data through the whole stack.
- [docs/system.md](docs/system.md)
  Main runtime/compiler boundary and system map.
- [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)
  One concrete `.gewy -> binding -> runtime -> export` path.
- [docs/architecture.md](docs/architecture.md)
  Deeper runtime-pipeline explanation.
- [docs/module-boundaries.md](docs/module-boundaries.md)
  Current ownership map across major runtime modules.
- [docs/book/explanation-gewylang-to-ir.md](docs/book/explanation-gewylang-to-ir.md)
  How authored packages become frontend graphs, lowered IR summaries, and
  archival snapshots before runtime begins.
- [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
  How protocol registry entries become packaged language input and then pass
  through IR, runtime, and export surfaces.

## Semantics And Interpretation

- [docs/book/explanation-conservative-diagnosis.md](docs/book/explanation-conservative-diagnosis.md)
  Why the runtime prefers conservative diagnosis over premature collapse.
- [docs/book/explanation-gewylang-lightweight-types.md](docs/book/explanation-gewylang-lightweight-types.md)
  Why `gewylang` adds lightweight parameter boundaries instead of a full type
  system.
- [docs/failure-semantics.md](docs/failure-semantics.md)
- [docs/process-profiles.md](docs/process-profiles.md)
- [docs/ingest-modes.md](docs/ingest-modes.md)

These pages explain not just what fields exist, but why the runtime remains
conservative about missing transitions, advisory ingest, and process-level
attribution.

## Security And Exposure Boundaries

- [docs/security-posture.md](docs/security-posture.md)
  What `gewyvern` should and should not be treated as.
- [docs/service-behavior.md](docs/service-behavior.md)
  How long-lived runtime behavior, API refresh, and degraded mode are expected
  to behave.
- [docs/machine-contract.md](docs/machine-contract.md)
  Narrow downstream contract that these behavior and trust boundaries are
  designed to protect.

## Sidecars And External Collaboration

- [docs/external-engine-contract.md](docs/external-engine-contract.md)
- [docs/sidecar-collaboration.md](docs/sidecar-collaboration.md)
- [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
- [docs/book/explanation-stack-topology.md](docs/book/explanation-stack-topology.md)

These pages explain the additive collaboration model: outside engines can help,
but they do not replace the core diagnosis spine.

## Release Readiness And Scope

- [docs/v0.14-posture.md](docs/v0.14-posture.md)
- [docs/field-validation.md](docs/field-validation.md)
- [docs/field-findings.md](docs/field-findings.md)

Together these explain why the current line is stable but still deliberately
scoped, and
what confidence has already been earned.
