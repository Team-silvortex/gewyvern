# Development Guide

Use this page when you need the current contributor workflow for `gewyvern`.

This page is intentionally a durable contributor guide. It focuses on:

- local orientation
- test workflow
- where different classes of tests live
- how to approach changes without drifting the runtime

This page is not the best first stop for:

- the top-level docs map
- the system architecture
- language or fragment semantics

For those, use:

- [docs/index.md](docs/index.md)
- [docs/system.md](docs/system.md)
- [docs/dsl.md](docs/dsl.md)
- [docs/fragments.md](docs/fragments.md)

## Role In The Shelf

Treat this page as the contributor workflow shelf.

Use it when the question is:

- how should I approach a change in this repository?
- what is the default test discipline?
- where do different classes of tests and runtime invariants live?

Do not use this page as:

- the short command reference
- the script routing map
- the packaging or Linux eBPF deep-dive page

For those, use:

- [docs/cli-recipes.md](docs/cli-recipes.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/packaging.md](docs/packaging.md)
- [docs/headless-linux.md](docs/headless-linux.md)

## Companion Shelves

- [docs/cli-recipes.md](docs/cli-recipes.md)
  for the compact command shelf
- [docs/headless-linux.md](docs/headless-linux.md)
  for Linux-only eBPF bring-up and validation
- [docs/module-boundaries.md](docs/module-boundaries.md)
  for source ownership when a change crosses subsystems

## Quick Start

Recommended reading before changing code:

- `README.md`
- `docs/index.md`
- `docs/system.md`
- `docs/dsl.md` or `docs/fragments.md`, depending on the change
- `docs/headless-linux.md` when the change touches real eBPF attach/runtime work

The shortest practical orientation path is:

1. `README.md`
2. `docs/index.md`
3. `docs/system.md`
4. `docs/dsl.md` or `docs/fragments.md`, depending on whether the change is language-facing or runtime-facing
5. `docs/module-boundaries.md` when the change crosses runtime reconstruction or reporting paths

Check the complete local toolchain and checked packaging inputs:

```bash
cargo dev doctor
```

Build the Rust workspace, Leserpent control solution, and Avalonia desktop in
parallel:

```bash
cargo dev build
```

The native workflow always uses Cargo's lock file. It reuses a fresh .NET
assets graph with `--no-restore`, performs a locked restore when project or
lock inputs changed, and reports each stage's elapsed time. Narrow an iteration
with `--scope core`, `--scope control`, or `--scope desktop`; add `--release`
for optimized output, `--restore` to force dependency verification, or
`--dry-run` to inspect the exact commands.

Run the demo binary:

```bash
cargo run
```

Run JSON demo output:

```bash
cargo run -- --demo both --json --summary-only
```

Run the full test suite:

```bash
cargo test --workspace
```

Inspect and validate the architecture-module-feature project status:

```bash
cargo run --bin gewyvern_status -- summary
cargo run --bin gewyvern_status -- validate
```

The status protocol and update rules live in
[docs/project-status-system.md](docs/project-status-system.md).

Run the primary TDD acceptance loop:

```bash
cargo tdd
```

Run a single scenario while iterating:

```bash
cargo tdd-one freeze_excludes_facts_beyond_lateness_cutoff
```

## TDD Workflow

This project should be worked in a test-driven way by default.

Every change should follow this order:

1. write a failing test for the new behavior or regression
2. implement the minimum code needed to make it pass
3. refactor only after the test is green

In practice, that means:

1. if the change is behavioral, start in `tests/runtime_tdd.rs`
2. if the change is a local invariant, start next to the source module
3. keep `cargo tdd` green before expanding scope

## Test Layout

### Rule tests

Rule-level behavior is specified in dedicated TDD files:

- `tests/template_rules_tdd.rs`
- `tests/fragment_rules_tdd.rs`

These tests protect invariants such as:

- template completeness
- program-model completeness
- hookpoint conflict rejection
- required fact coverage

### Scenario tests

Behavioral runtime scenarios live in the main acceptance spec:

- `tests/runtime_tdd.rs`

Rule-level invariants live in dedicated rule specs:

- `tests/template_rules_tdd.rs`
- `tests/fragment_rules_tdd.rs`

Shared fixtures live in:

- `tests/support/mod.rs`

These tests describe the current T1 acceptance behaviors:

- export contains `attach_plan`
- export contains `fragment_inventory`
- missing SYN-ACK remains replay-stable
- route fingerprint change rotates into a new flow
- facts beyond freeze cutoff are excluded from export and replay
- attach outcomes gate fact ingest
- rejected facts are exported and replay-stable
- UDP and process-aware program flows remain deterministic

### Environment-specific tests

Some tests are intentionally environment-specific:

- `tests/linux_smoke_tdd.rs`
  Linux-only real probe smoke/probe specs
- `tests/socket_input_tdd.rs`
  Unix socket ingest roundtrip spec
- `tests/tcp_socket_input_tdd.rs`
  TCP socket ingest roundtrip spec

In restricted environments, socket live tests may remain `ignored` because
local bind permissions are unavailable.

## How To Extend The Runtime

### Add a new fragment

1. add a new `FragmentDescriptor` to `builtin_registry()`
2. add or extend a failing scenario in `tests/runtime_tdd.rs`
3. add rule tests for conflicts and coverage if needed
4. only then update runtime behavior

### Add or change a program-flow rule

1. add a scenario test in `tests/runtime_tdd.rs`
2. update the template's `program_model`
3. only then update lower-level runtime code if the rule engine is missing a capability
4. verify export/replay stability for `program_flows`

### Add a future DSL compile target feature

1. treat `TemplateBinding` as the compile target
2. keep changes in the space of fragment selection, parameter binding, and runtime policy
3. do not introduce direct DSL-to-bytecode generation paths
4. verify the resulting binding still works through `SessionConfig`

### Add a new reason rule

1. add a scenario test in `tests/runtime_tdd.rs`
2. extend `ReasonProfile` or the reason builder
3. verify replay still recomputes the same result

### Change export format

1. add a test that covers the new export field
2. update `ExportBundle::to_json`
3. update `ExportBundle::from_json`
4. confirm replay remains deterministic

### Document the change

After behavior changes, update the matching document:

- fragment semantics -> `docs/fragments.md`
- export semantics -> `docs/export-format.md`
- runtime pipeline details -> `docs/architecture.md`
- `src/` layering and ownership -> `docs/module-boundaries.md`
- system boundaries or layering -> `docs/system.md`
- workflow changes -> `docs/development.md`
- project-facing capabilities -> `README.md`

## Practical Rule

If a code change does not come with a new or updated test, it is probably too
implicit for this project.

## Default Commands

The compact command shelf now lives in:

- [docs/cli-recipes.md](docs/cli-recipes.md)

The default contributor commands remain:

- `cargo tdd`: run the acceptance behavior suite first
- `cargo tdd-one <name>`: iterate on one named acceptance test
- `cargo tdd-rules`: run rule/invariant specs
- `bash scripts/perf/trim_workspace_disk.sh --dry-run`: preview rebuildable disk usage
- `bash scripts/perf/trim_workspace_disk.sh`: reclaim local workspace disk from build artifacts and caches
- `cargo test --workspace`: run the full suite before finishing

## Linux Bring-Up

When work touches real eBPF probe/attach behavior, switch to the headless Linux
flow documented in `docs/headless-linux.md`.

Useful commands there:

- `cargo linux-smoke`
- `cargo tdd`
- `cargo test --workspace`
