# Development Guide

## Quick Start

Recommended reading before changing code:

- `docs/overview.md`
- `docs/architecture.md`
- `docs/fragments.md`
- `docs/export-format.md`

Run the demo binary:

```bash
cargo run
```

Run the full test suite:

```bash
cargo test
```

## TDD Workflow

This project now follows a test-driven workflow.

Every change should follow this order:

1. write a failing test for the new behavior or regression
2. implement the minimum code needed to make it pass
3. refactor only after the test is green

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

Behavioral runtime scenarios live in:

- `tests/runtime_tdd.rs`

Shared fixtures live in:

- `tests/support/mod.rs`

These tests describe the current T1 acceptance behaviors:

- export contains `attach_plan`
- export contains `fragment_inventory`
- missing SYN-ACK remains replay-stable
- route fingerprint change rotates into a new flow

## How To Extend The Runtime

### Add a new fragment

1. add a new `FragmentDescriptor` to `builtin_registry()`
2. add rule tests for conflicts and coverage if needed
3. add a scenario test that proves the fragment's runtime effect
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
