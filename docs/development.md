# Development Guide

## Quick Start

Recommended reading before changing code:

- `docs/overview.md`
- `docs/architecture.md`
- `docs/fragments.md`
- `docs/export-format.md`
- `docs/headless-linux.md` when the change touches real eBPF attach/runtime work

Run the demo binary:

```bash
cargo run
```

Run the full test suite:

```bash
cargo test
```

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

Rule-level behavior lives close to the source modules:

- `src/template.rs`
- `src/fragment.rs`

These tests protect invariants such as:

- template completeness
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

## How To Extend The Runtime

### Add a new fragment

1. add a new `FragmentDescriptor` to `builtin_registry()`
2. add or extend a failing scenario in `tests/runtime_tdd.rs`
3. add rule tests for conflicts and coverage if needed
4. only then update runtime behavior

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
- runtime pipeline or boundaries -> `docs/architecture.md`
- workflow changes -> `docs/development.md`

## Practical Rule

If a code change does not come with a new or updated test, it is probably too
implicit for this project.

## Default Commands

- `cargo tdd`: run the acceptance behavior suite first
- `cargo tdd-one <name>`: iterate on one named acceptance test
- `cargo tdd-rules`: run rule/invariant specs
- `cargo test`: run the full suite before finishing

## Linux Bring-Up

When work crosses from runtime skeleton into real eBPF attach behavior, switch
to the headless Linux flow documented in `docs/headless-linux.md`.
