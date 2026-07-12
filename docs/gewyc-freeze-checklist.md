# gewyc Contract Freeze Checklist

Use this page when you are tightening `gewyc` machine surfaces for the next
minor line.

This page is not the full schema reference. It is the shortest repeatable
freeze ritual for:

- promoting grouped fields
- keeping compatibility fields honest
- preventing fixture drift
- deciding what can tighten in the next minor line

Use these nearby pages with it:

- [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
- [docs/gewyc-contract-matrix.md](docs/gewyc-contract-matrix.md)
- [docs/gewyc-sample-index.md](docs/gewyc-sample-index.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)

## What A Freeze Means

For the current `0.x` line, a freeze does not mean “nothing can ever move”.

It means:

- wrapper routing stays dependable
- preferred grouped shelves stay readable and test-backed
- `compat` fields do not disappear casually
- fixtures and docs keep matching the real renderer

## Current `0.19.x -> 1.0.0` Checklist

Treat the compiler surface as ready for the next tightening step only when all
of the following are true:

1. wrapper fields still route every `gewyc ... --json` surface
2. grouped `payload` shelves are documented as the preferred first read
3. every promoted grouped shelf has at least one real fixture
4. fixture regression tests still prove those grouped shelves exist
5. `compat` fields are still documented wherever consumers may still depend on them
6. any planned tightening line is named explicitly in the field contract page
7. no new consumer docs tell readers to start from a `compat` field first

## Freeze Walk

Run the freeze in this order:

1. wrapper
2. grouped shelves
3. compatibility carry-over
4. fixtures
5. tests
6. release-line notes

### 1. Wrapper

Confirm these fields still exist across machine-facing surfaces:

- `surface_id`
- `schema_hint.family`
- `schema_hint.surface`
- `schema_hint.schema_version`
- `contract_hint.stability`
- `contract_hint.compatibility`
- `contract_hint.legacy_fields`
- `payload`

### 2. Grouped Shelves

Confirm each promoted surface still has a preferred grouped read path.

Current high-value grouped shelves are:

- `binding`: `payload.status`, `payload.counts`
- `stages`: `payload.status`, `payload.counts`
- `explain`: `payload.summary.stage_status`, `payload.summary.analysis`, `payload.summary.shape_notes`, `payload.summary.excerpts`
- `ir_history_snapshot`: `payload.analysis.model_compare`, `payload.analysis.history_snapshot`

### 3. Compatibility Carry-Over

For every field still marked `compat`:

- confirm it is still emitted
- confirm it is still documented
- confirm new docs do not present it as the preferred route

If a `compat` field no longer earns its cost, do not remove it silently.
Instead:

1. mark the next tightening line
2. move it toward `strictly_legacy` in the next contract revision
3. only then consider removal

## Fixture Set

The current minimum fixture spine should stay green:

- [docs/fixtures/gewyc_frontend_udp_process_debug.json](docs/fixtures/gewyc_frontend_udp_process_debug.json)
- [docs/fixtures/gewyc_stages_udp_process_debug.json](docs/fixtures/gewyc_stages_udp_process_debug.json)
- [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](docs/fixtures/gewyc_explain_validation_udp_process_debug.json)
- [docs/fixtures/gewyc_explain_parse_failure.json](docs/fixtures/gewyc_explain_parse_failure.json)
- [docs/fixtures/gewyc_explain_validation_failure.json](docs/fixtures/gewyc_explain_validation_failure.json)

These are the minimum because together they cover:

- healthy authoring shape
- healthy phase spine
- healthy umbrella explain shape
- parse failure excerpts
- validation failure excerpts

## Test Gate

At minimum, keep these green before calling the surface frozen enough for the
next line:

```bash
cargo test gewyc::tests::contract -- --nocapture
cargo test gewyc::tests::fixture_contract -- --nocapture
```

Use broader compiler surface tests when changing renderer internals, but these
two are the smallest contract-preserving gate.

## Release-Line Note

When a freeze step changes what consumers should read first:

1. update [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
2. update [docs/gewyc-contract-matrix.md](docs/gewyc-contract-matrix.md)
3. update [docs/history/v0.19.x.md](docs/history/v0.19.x.md) if it changes the meaning of the line

That keeps the contract, the examples, and the release narrative aligned.

## Exit Condition

The surface is “frozen enough” for the next minor tightening step when:

- grouped reads are clearer than compat reads
- fixtures demonstrate the preferred route
- tests lock the preferred route in place
- planned tightening lines are written down instead of implied
