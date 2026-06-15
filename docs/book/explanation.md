# Explanation

This track is for mental models and design rationale. Use it when you want to
understand why the project is shaped the way it is.

## Core System Shape

If you want the shortest architecture reading order inside the explanation
track, use:

1. [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
2. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
3. [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
4. [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
5. [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
6. [docs/architecture-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-evolution.md)
7. [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)

- [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)
  How the documentation set itself is layered and maintained.
- [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
  How the `gewylang` tutorial, guide, reference, compiler, and rationale
  pages fit together.
- [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
  How the `gewylang -> IR -> runtime` spine is supposed to mature.
- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
  Project-level architecture sheet for subsystem and evolution boundaries.
- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
  Source-cluster dependency blueprint.
- [docs/architecture-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-evolution.md)
  How the main `gewylang -> IR -> runtime -> export` spine is meant to mature.
- [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)
  How protocol, IR, runtime, and collaboration lines constrain each other.
- [docs/architecture-walkthrough-http-request.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-walkthrough-http-request.md)
  One representative end-to-end architecture path through the current stack.
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
  Main runtime/compiler boundary and system map.
- [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
  One concrete `.gewy -> binding -> runtime -> export` path.
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  Deeper runtime-pipeline explanation.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Current ownership map across major runtime modules.
- [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
  How authored packages become frontend graphs, lowered IR summaries, and
  archival snapshots before runtime begins.
- [docs/book/explanation-protocol-package-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-protocol-package-spine.md)
  How protocol registry entries become packaged language input and then pass
  through IR, runtime, and export surfaces.

## Semantics And Interpretation

- [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)
  Why the runtime prefers conservative diagnosis over premature collapse.
- [docs/book/explanation-gewylang-lightweight-types.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-lightweight-types.md)
  Why `gewylang` adds lightweight parameter boundaries instead of a full type
  system.
- [docs/failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/failure-semantics.md)
- [docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md)
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)

These pages explain not just what fields exist, but why the runtime remains
conservative about missing transitions, advisory ingest, and process-level
attribution.

## Security And Exposure Boundaries

- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
  What `gewyvern` should and should not be treated as.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  How long-lived runtime behavior, API refresh, and degraded mode are expected
  to behave.
- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
  Narrow downstream contract that these behavior and trust boundaries are
  designed to protect.

## Sidecars And External Collaboration

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
- [docs/book/explanation-stack-topology.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-stack-topology.md)

These pages explain the additive collaboration model: outside engines can help,
but they do not replace the core diagnosis spine.

## Release Readiness And Scope

- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)

Together these explain why the current line is stable but still deliberately
scoped, and
what confidence has already been earned.
