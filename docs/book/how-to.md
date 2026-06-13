# How-To Guides

This track is task-first. Use it when you already know roughly what
`gewyvern` is and just want to get something done.

## Operate The Runtime

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
  Run a first focused target, a sweep, or the serve/API path on purpose.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  Understand `--serve`, restart, degraded mode, and API behavior.
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)
  Choose among demo, Unix socket, and TCP socket ingest.

## Validate The Current Surface

- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
  Practical validation ladder for the current CLI, compiler, registry, and
  container/runtime surfaces.
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
  Current validation matrix and local entrypoints.
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
  What the validation line has already demonstrated.
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)
  Measure current runtime and compiler-facing baselines.

## Build And Package

- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
  Contributor workflow and test layout.
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
  Linux and eBPF validation path.
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
  DEB/RPM packaging and container validation.

## Add Or Debug Protocol Packages

- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
  Build or repair a registry package without guessing where drift entered.
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Language surface and current stable subset.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Internal source layering when a package change spills into runtime/compiler
  code.

## Extend With External Analysis

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  Append-only external analysis contract.
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
  How nearby sidecars are exposed without mutating the core diagnosis spine.

## Prepare For Release Judgement

- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
  Current release posture for the active `0.14.x` line.
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
  Security and exposure boundaries.

## Next How-To Chapters

The next high-value task pages are likely:

- how to read a missing-transition diagnosis
- how to wire `etragon` as a nearby sidecar
- how to validate a packaged Linux runtime end to end
