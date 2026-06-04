# Explanation

This track is for mental models and design rationale. Use it when you want to
understand why the project is shaped the way it is.

## Core System Shape

- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
  Main runtime/compiler boundary and system map.
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  Deeper runtime-pipeline explanation.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Current ownership map across major runtime modules.

## Semantics And Interpretation

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

## Sidecars And External Collaboration

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)

These pages explain the additive collaboration model: outside engines can help,
but they do not replace the core diagnosis spine.

## Release Readiness And Scope

- [docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md)
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)

Together these explain why the current line is still a preparation line and
what confidence has already been earned.
