# Legacy Leselang HIR Compatibility Facade

This product-workspace crate preserves the old product-enabled API. It is not
the public native `leselang-hir` 0.1.0 package in the independent repository.

Product code and tests use `leserpent-hir` directly. New native integrations use
[Team-silvortex/leselang](https://github.com/Team-silvortex/leselang). Only the old
VM compatibility crate still depends on this local facade; the repository gate
rejects new product consumers, including dev/build dependencies and aliases.

Existing facade conformance suites stay here until their acceptance obligations
have replacements. See the [handoff guide](../../docs/leselang-repository-handoff.md).

## Compatibility Acceptance

`cargo test -p leselang-hir --no-default-features` covers the released native
import path, core type identity, foreign-host contracts and compile-fail docs
that exclude product APIs. CI runs this complete suite once rather than repeating
eight selected integration targets under the same feature configuration.

The default-profile `product_identity` target separately proves that old product
types are unchanged re-exports of `leserpent-hir`. Product lowering and recorded
migration outcomes belong to that product owner. Native upstream acceptance does
not replace either local API obligation, even when test bodies match byte-for-byte.

Direct syntax, runtime-core and serde imports are development dependencies used
by these suites, not implementation dependencies of the re-exporting library.
Product test support stays optional under `leserpent-profile` so native-only
acceptance keeps its product-free dependency closure.
