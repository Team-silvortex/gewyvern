# Product HIR Boundary Probe

This isolated workspace consumes the real `leserpent-hir` product crate, which
uses only the public `leselang-hir-core` API. The wrapper re-exports that crate;
it does not include or copy production source. A separate native facade
dependency keeps `default-features = false`, even with the product crate present.
This is a compiler-backed consumer gate, not another product implementation.

Run from the repository root to reuse build artifacts:

```sh
cargo +1.98.0 test --manifest-path tests/fixtures/leselang-product-hir-boundary/Cargo.toml --locked --target-dir target --test product
cargo +1.98.0 clippy --manifest-path tests/fixtures/leselang-product-hir-boundary/Cargo.toml --locked --target-dir target --lib --test product -- -D warnings
cargo +1.98.0 fmt --manifest-path tests/fixtures/leselang-product-hir-boundary/Cargo.toml -- --check
```

Library unit tests are intentionally disabled. Historical private-kernel oracles
run in the facade's `migration_oracles` integration target. The
separate integration tests here exercise public compilation, generic IR type
identity, canonical/wire round trips, authorization and cold-branch rejection.
Use the listed targets to keep this probe focused; the ordinary product suite
already runs the profile's inline unit tests.

Keep this lock aligned with root dependency versions when refreshing packages.
Keep the separate `[workspace]`, direct product dependency and
`default-features = false`: otherwise feature unification can mask coupling.
Native and product HIR production ownership are now physically separate.
Passing this consumer gate does not claim a complete native VM, host-neutral
durable recovery or full repository extraction.
