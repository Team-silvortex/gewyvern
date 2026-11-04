# Recorded Product Boundary Oracles

These read-only JSON fixtures preserve 6,912 complete observations from the old
private scalar, fold and loop coordinators. Before removing the source-inclusion
harness, each observation was compared with the live product result using Rust
equality. Only the frozen result was recorded after that comparison passed.

Each `cases` row is `[form_index, scope_prefix, starting_nodes, depth, outcome_index]`.
`outcomes` deduplicates complete serialized tuples of result (IR/type or ordered
diagnostics), final node counter and remaining lexical bindings/types. It is not
a hash-only success marker. The product tests retain all original source forms,
scope configurations, initial budgets and depth loops; they require every recorded
case exactly once and compare the complete tuple through the real upstream core.

`ownership.json` preserves the old harness's complete 121-test inventory, original
coordinator hashes, fixture hashes and capture revisions. Its 93 product tests
already ran in `leserpent-hir`; 25 native tests now run in the Cargo-resolved
`leselang-hir-core --lib` target via `cargo dev check leselang`; the three boundary
matrices now run as product library tests. The captured owner runs passed 97 and
29 tests respectively, including additional owner-local checks.

There is no fixture update mode or generator in this repository. The temporary
capture instrumentation was removed with the old harness. Do not regenerate these
expectations from the current implementation merely to make a failure pass.
An intentional semantic change needs an explicit reviewed baseline change with
new provenance; prior capture history must remain distinguishable.

The independent repository, dependency pin, language/API versions and complete
runtime acceptance gates are unchanged. Frozen source snapshots remain only as
historical source/status evidence, not compilation inputs for these oracles.
