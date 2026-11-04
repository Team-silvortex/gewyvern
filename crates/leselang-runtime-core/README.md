# Leselang Runtime Core Source Owner

This directory contains navigation only, not a buildable Cargo package.
Its manifest and source/test snapshots are retired; acceptance uses the pinned
file inventory and actual upstream sources, not another local implementation.
Native development belongs to the independent MIT repository.

## Documentation Owner

- [Pinned runtime-core source](https://github.com/Team-silvortex/leselang/tree/5f86dfb8dd79a28986fe5eb4cbfe6b044796f935/crates/leselang-runtime-core): authoritative implementation.
- [Pinned runtime-core guide](https://github.com/Team-silvortex/leselang/blob/5f86dfb8dd79a28986fe5eb4cbfe6b044796f935/crates/leselang-runtime-core/README.md): current package responsibilities and native checks.
- [Pinned historical provenance](https://github.com/Team-silvortex/leselang/blob/5f86dfb8dd79a28986fe5eb4cbfe6b044796f935/docs/provenance/runtime-core-baseline.md): the original detailed capture, dispatch and recovery boundaries.
- [Product embedding contracts](../../docs/leselang-embedding.md): Leserpent policy, authority, wire and durable-state obligations.
- [Repository handoff](../../docs/leselang-repository-handoff.md#runtime-documentation-ownership): current source ownership and verification rules.

The historical provenance is an extraction checkpoint, not current build instructions
or a new release claim. Product documentation tests read that archive from the
actual Cargo-resolved checkout and verify its recorded SHA-256. There is no local
body copy, sibling-checkout requirement or snapshot fallback. The reviewed document
path, revision and hash are tracked in
[the handoff metadata](../../project/leselang-repository.json).

Run native package checks in the independent repository. To check this product's
pinned integration, run `cargo dev check leselang` from the Gewyvern root.

## Remaining Acceptance

[Repository bootstrap is separate from full runtime acceptance](../../docs/leselang-embedding.md#repository-extraction-readiness).
Complete unrelated-host vertical proofs, host-neutral durable recovery and full
product parity remain separate gates, with no replacement version or date.
Removing a duplicated manual does not promote `complete_vm` or `full_extraction_gate`.
