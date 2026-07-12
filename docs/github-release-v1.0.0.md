# gewyvern v1.0.0

`gewyvern v1.0.0` is the first sealed stable release of the project.

This release means the core system is now coherent enough to operate, review,
and extend without relying on tribal memory or ad hoc shell rituals.

## Why `v1.0.0` Matters

- the runtime, CLI, compiler, protocol shelf, and documentation now present
  one stable core story
- release judgment starts from native `gewyvern_validate` entrypoints
- Linux-host proof, eBPF attach evidence, and target-lab validation are part
  of ordinary release confidence
- `gewyvern`, `gewyc`, `etragon`, and `leserpent` now move as one shared
  mainline version

## Stable Core

`v1.0.0` stabilizes:

- the Linux-oriented local network debugger/runtime
- `gewylang` package authoring and `gewyc` compilation flow
- machine-readable JSON outputs and HTML reporting surfaces
- native release validation entrypoints and artifact indexes
- predictable lifecycle behavior around startup, stop, malformed input, logs,
  persistence, and cleanup

## Highlights

- sealed the `v0.20.x` pre-stable convergence work into a stable mainline
- promoted release validation from shell-first wrappers to native commands
- preserved remote Linux host validation as a first-class proof path
- kept pathological ingest, debugger cross-validation, and practical target-lab
  checks inside the ordinary release posture
- aligned the documentation shelves, history pages, and release narrative
- introduced the first project logo and integrated it into repo and app entry
  surfaces

## Validation Snapshot

The `v1.0.0` release posture is backed by a green path including:

- packaged/container release validation
- debugger cross-validation
- remote Linux host validation
- JSON release artifact-index validation
- suspicious HTTP, denied FTP, and denied LDAP target-lab checks
- Rust, NuGet, and frontend vulnerability scans
- focused `leserpent` and `etragon` security tests

## Recommended Starting Points

- [README.md](../README.md)
- [docs/index.md](index.md)
- [docs/history/v1.0.0.md](history/v1.0.0.md)
- [docs/history/v1.0.0-release-notes.md](history/v1.0.0-release-notes.md)
- [docs/release-checklist.md](release-checklist.md)

## What Comes Next

The immediate goal is not to redefine the product.

The goal is to strengthen `v1.0.x` through:

- reliability refinement
- operator UX improvement
- performance optimization
- disciplined, explicit post-`1.0.0` extension work
