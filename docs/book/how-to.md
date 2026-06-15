# How-To Guides

This track is task-first. Use it when you already know roughly what
`gewyvern` is and just want to get something done.

## Book Path

This volume belongs primarily to Part VI: Operating, Validating, And
Extending.

Use it after you already have enough system context to act on purpose.

For the task discipline behind this volume, see:

- [docs/book/how-to-structure.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-structure.md)

## How To Use This Volume

A good lookup order is:

1. choose the task band
2. open the shortest task guide
3. return to reference or explanation only if you need deeper detail

The current task bands are:

- validate
- extend
- operate
- collaborate and package

When the reader does not need a narrative task guide and only needs the right
command or script, prefer these top-level operator shelves first:

- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)

## Operate The Runtime

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
  Run a first focused target, a sweep, or the serve/API path on purpose.
- [docs/book/how-to-security-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-security-checklist.md)
  Deployment-facing preflight for ingest trust, API exposure, external-engine
  wiring, and registry-root safety.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  Understand `--serve`, restart, degraded mode, and API behavior.
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)
  Choose among demo, Unix socket, and TCP socket ingest.

## Validate The Current Surface

- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
  Practical validation ladder for the current CLI, compiler, registry, and
  container/runtime surfaces.
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
  Fast script routing when you already know what kind of validation you need.
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
  Current validation matrix and local entrypoints.
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
  What the validation line has already demonstrated.
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)
  Measure current runtime and compiler-facing baselines.

## Build And Package

- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
  Contributor workflow and test layout.
- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
  Compact command shelf for local CLI and helper usage.
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
  Linux and eBPF validation path.
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
  DEB/RPM packaging and container validation.

## Add Or Debug Protocol Packages

- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
  Build or repair a registry package without guessing where drift entered.
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Language surface and current stable subset.
- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
  Stable pipeline/package authoring shape.
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
  Exact DSL vocabulary and compatibility lookup.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Internal source layering when a package change spills into runtime/compiler
  code.

## Extend With External Analysis

- [docs/book/how-to-wire-etragon-sidecar.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-wire-etragon-sidecar.md)
  Wire `etragon` as a nearby sidecar and verify the additive bridge end to end.
- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  Append-only external analysis contract.
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
  How nearby sidecars are exposed without mutating the core diagnosis spine.

## Prepare For Release Judgement

- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
  Current release posture for the active `0.14.x` line.
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
  Security and exposure boundaries.

## Where This Volume Should Grow Next

The next highest-value dedicated task guides are:

- how to validate packaged Linux runtime behavior end to end
- how to operate the serve/API path intentionally
- how to read a missing-transition diagnosis without overreacting

## Next How-To Chapters

The next high-value task pages are likely:

- how to read a missing-transition diagnosis
- how to validate a packaged Linux runtime end to end
