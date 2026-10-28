# Legacy Leselang HIR Compatibility Facade

This product-workspace crate preserves the old product-enabled API. It is not
the public native `leselang-hir` 0.1.0 package in the independent repository.

Product code and tests use `leserpent-hir` directly. New native integrations use
[Team-silvortex/leselang](https://github.com/Team-silvortex/leselang). Only the old
VM compatibility crate still depends on this local facade; the repository gate
rejects new product consumers, including dev/build dependencies and aliases.

Existing facade conformance suites stay here until their acceptance obligations
have replacements. See the [handoff guide](../../docs/leselang-repository-handoff.md).
