# Leselang Syntax Source Owner

This directory is excluded from the product Cargo workspace. Builds consume
Leselang 0.1.0 from the exact Git revision in the root workspace dependencies.
Language changes belong in [Team-silvortex/leselang](https://github.com/Team-silvortex/leselang).

The local source/test snapshot is retired. This directory contains navigation
only, not a Cargo package or another native implementation.

- [Pinned syntax source](https://github.com/Team-silvortex/leselang/tree/5f86dfb8dd79a28986fe5eb4cbfe6b044796f935/crates/leselang-syntax)
- [Reviewed file inventory](../../project/status/leselang-source-inventory.json)
- [Ownership and acceptance rules](../../docs/leselang-repository-handoff.md#core-snapshot-retirement)

Architecture, safety and acceptance checks use the Cargo-resolved upstream source.
File hashes and inventory completeness remain checked without restoring local copies.
