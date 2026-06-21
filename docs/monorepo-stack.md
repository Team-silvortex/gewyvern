# Monorepo Stack Guide

This page explains how the local `gewyvern` repository now carries the broader
stack as one monorepo while still preserving clean role boundaries between the
core runtime and the nearby companion applications.

Use this page when the question is:

- where do `gewyvern`, `etragon`, and `leserpent` live now?
- which toolchain belongs to which subproject?
- how should local builds, tests, and stack validation work after the migration?
- what should stay shared, and what should stay separate?

## Current Layout

The repository now has one top-level root:

- `/Users/Shared/chroot/dev/gewyvern`

Within that root:

- [Cargo.toml](/Users/Shared/chroot/dev/gewyvern/Cargo.toml)
  Root Rust workspace manifest.
- [src](/Users/Shared/chroot/dev/gewyvern/src)
  Core `gewyvern` runtime, protocol work, IR, APIs, and CLI.
- [crates/gewyc](/Users/Shared/chroot/dev/gewyvern/crates/gewyc)
  Dedicated compiler CLI crate.
- [apps/etragon](/Users/Shared/chroot/dev/gewyvern/apps/etragon)
  Nearby diagnosis-partner sidecar application.
- [apps/leserpent](/Users/Shared/chroot/dev/gewyvern/apps/leserpent)
  Cross-platform control plane application.

## Project Roles

The migration did not change the architectural roles:

- `gewyvern`
  Owns runtime capture, protocol interpretation, IR lowering, report surfaces,
  and the conservative diagnosis spine.
- `etragon`
  Owns nearby external-analysis enrichment and sidecar learning surfaces.
- `leserpent`
  Owns control-plane registry, multi-runtime coordination, and operator-facing
  UI workflows.

What changed is only the repository boundary:

- they now evolve together in one checkout
- shared validation can live in one place
- docs and scripts can point at stable in-repo paths
- cross-project refactors no longer need sibling-repo choreography

## Version Posture

Current versions:

- `gewyvern`: `0.15.0`
- `etragon`: `0.1.0`
- `leserpent`: `0.1.9`

These versions are intentionally not forced into one shared number line yet.
One repository does not imply one release version.

## Toolchain Boundaries

The stack is one repository, but not one toolchain:

- `gewyvern`
  Rust workspace member
- `gewyc`
  Rust workspace member
- `etragon`
  Rust workspace member
- `leserpent`
  .NET backend plus TypeScript frontend app under `apps/`

That means:

- use `cargo` from the repository root for `gewyvern`, `gewyc`, and `etragon`
- use `dotnet` and `npm` within `apps/leserpent` for the control plane

## Common Commands

From the repository root:

```bash
# Main runtime
cargo run -- --scan-all --json --summary-only

# Compiler CLI
cargo run -p gewyc -- dsl/http_request_path.gewy --json

# Nearby sidecar
cargo run -p etragon -- --help

# Rust workspace validation
cargo test --workspace
```

For `leserpent`:

```bash
cd /Users/Shared/chroot/dev/gewyvern/apps/leserpent
npm run check:frontend
dotnet build src/Leserpent/Leserpent.csproj
```

## Validation Order

When you want confidence after a stack-wide change, use this order:

1. `cargo test --workspace`
2. `cd apps/leserpent && npm run check:frontend`
3. `dotnet build apps/leserpent/src/Leserpent/Leserpent.csproj`
4. stack scripts under [scripts](/Users/Shared/chroot/dev/gewyvern/scripts), especially:
   [scripts/demos/external_engine_roundtrip_demo.sh](/Users/Shared/chroot/dev/gewyvern/scripts/demos/external_engine_roundtrip_demo.sh)
   and
   [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)

Use the thin demos first when the question is “did the path still connect?”.
Use the fuller stack validation when the question is “does the topology still
behave as one system?”.

## Migration Rules Going Forward

Now that the stack lives in one repository, keep these rules:

- keep runtime-core work at the repository root
- keep app-specific code inside `apps/etragon` or `apps/leserpent`
- only extract shared modules when the boundary is stable enough to deserve it
- do not let the monorepo erase role boundaries
- prefer relative in-repo paths in docs and scripts over old sibling-repo paths

## What “Clean Migration” Means Here

The migration should be considered healthy when:

- there are no live references to old standalone paths like
  `/Users/Shared/chroot/dev/etragon` or `/Users/Shared/chroot/dev/leserpent`
- stack scripts default to in-repo `apps/` paths
- `etragon` participates in the root Rust workspace
- `leserpent` still builds cleanly from its new location
- docs describe one stack checkout instead of three independent repositories

## Related Pages

- [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
- [docs/book/how-to-wire-etragon-sidecar.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-wire-etragon-sidecar.md)
- [apps/README.md](/Users/Shared/chroot/dev/gewyvern/apps/README.md)
