# How-To Guides

This track is task-first. Use it when you already know roughly what
`gewyvern` is and just want to get something done.

## Operate The Runtime

- [docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md)
  Run a single target, a sweep, or generate reports.
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
  Understand `--serve`, restart, degraded mode, and API behavior.
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)
  Choose among demo, Unix socket, and TCP socket ingest.

## Validate The Current Surface

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

## Extend With External Analysis

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  Append-only external analysis contract.
- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
  How nearby sidecars are exposed without mutating the core diagnosis spine.

## Prepare For Release Judgement

- [docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md)
  Current release-readiness gate.
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
  Security and exposure boundaries.

## Future Shape

Over time, task-specific pages should move here more explicitly, for example:

- how to author a protocol package
- how to read a missing-transition diagnosis
- how to wire `etragon` as a nearby sidecar
- how to validate a packaged Linux runtime end to end
