# Frozen HIR Acceptance Snapshot

This directory is excluded from the product Cargo workspace. Builds consume
Leselang 0.1.0 from the exact Git revision in the root workspace dependencies.
Language changes belong in [Team-silvortex/leselang](https://github.com/Team-silvortex/leselang).

Historical source-inspection tests still use these files. The repository gate
compares every source/test file with the actual resolved upstream package; do not
edit them independently. See the [handoff guide](../../docs/leselang-repository-handoff.md)
before upgrading the pin or retiring the snapshots.
