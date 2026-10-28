# Leselang Repository Handoff

The native language foundation now has a separate local repository named
`leselang`, licensed under MIT. The normal layout is sibling checkouts:

```text
dev/
  gewyvern/   # product, reference host and compatibility baseline
  leselang/   # native language foundation
```

The public remote is [Team-silvortex/leselang](https://github.com/Team-silvortex/leselang).
Its initial MIT-licensed 0.1.0 source is published at commit
`5f86dfb8dd79a28986fe5eb4cbfe6b044796f935`. Gewyvern now pins that exact Git revision
and version for all three shared native cores. Normal builds do not need a sibling
checkout. Further Git commits and pushes remain user-managed.
Machine-readable handoff state lives in
[`project/leselang-repository.json`](../project/leselang-repository.json).
The [local bootstrap proof](fixtures/leselang_repository_bootstrap_macos_arm64_20261028.json)
records independent test runs, product-core handoff checks and explicit non-claims.
Its `remote_created = false` describes the earlier bootstrap checkpoint; current
remote ownership and public visibility are recorded in the handoff state above.
The [pinned product cutover proof](fixtures/leselang_product_cutover_macos_arm64_20261028.json)
records the actual Git identity, compatibility tests, local TLS roundtrip, source
snapshot comparison and upstream CI result. It does not claim a new main-repository
remote CI run before these changes are committed and pushed.

## Source Owners

The new repository contains `leselang-syntax`, `leselang-runtime-core`,
`leselang-hir-core`, native `leselang-hir` and native `leselang-vm`, plus the
separate consumer, examples, pinned lockfiles, toolchain, MIT LICENSE and CI.
Default and all-features builds have no product profile or SQLite dependency.
Native foundation development belongs there from this point forward.

Gewyvern retains `leserpent-hir`, `leserpent-vm`, command/host-contract/UI/observe
adapters and the old product-enabled HIR/VM compatibility entry points. Existing
in-tree native sources are frozen acceptance snapshots, excluded from the Cargo
workspace and never linked into product builds. Historical source-inspection
tests still read them; `tests/leselang_repository_tdd.rs` compares their complete
source/test inventories byte for byte with the actual Cargo-resolved upstream.
Do not develop divergent native implementations or overwrite the new repository
with another export. The old source exporter reproduces the compatibility
checkpoint, not a new independent Leselang release.

The source baseline was the working tree at HEAD
`784e4aac7ca622497de40333ab1d4ade5787f84d` (`2.5.5`), including pending product
acceptance-ownership changes. The source Cargo package version was 2.5.0;
independent Leselang starts at 0.1.0, without changing Gewyvern's product version
or runtime protocols. The new repository retains source
inventory/provenance and deliberately keeps `publish = false` until a release
is prepared. Original copyright attribution is preserved.

## Dependency Cutover

The root workspace dependencies and lockfile select one immutable source identity
for syntax, runtime-core and HIR-core across every product adapter. The separate
product-HIR fixture pins the same identity in its own lockfile. Equal Rust type
names from different package identities are not equal types: the gate rejects
duplicate, local, mutable-revision and wrong-version copies.

Product compatibility facades remain here while callers migrate deliberately.
Do not enable old product features on the new native facade packages. Product
HIR/VM/command tests, facade identity/recovery tests and no-SQLite profiles verify
the boundary. Native package tests and lints run in the independent repository;
Gewyvern CI runs the pinned-source gate before its historical export verification.

To update Leselang, review an upstream commit, update all exact pins and lockfiles,
and migrate or refresh frozen acceptance snapshots from that commit. Run the
source gate, product tests and independent product-HIR fixture before committing.
Never bypass a snapshot mismatch by weakening the gate. Remove retained snapshot
files only after their historical acceptance checks have a replacement; they are
not a second development tree. A sibling-directory dependency alone is not releasable.

## Product Compatibility Cleanup

The [consumer cleanup proof](fixtures/leselang_product_compatibility_cleanup_macos_arm64_20261028.json)
is the current dependency checkpoint. The earlier cutover proof above retains
its original manifest hashes and results rather than being silently restamped.

Root tutorial and fuzz tests now depend directly on `leserpent-hir` and
`leserpent-vm`. They still exercise the independent parser with the real product
lowering and continuation codec, but no longer rely on local language facades.
The dependency gate inspects all product dependency kinds, including optional,
renamed, dev and build edges. Only the old VM-to-old-HIR compatibility edge remains
allowed; no product or root-test consumer may grow a new legacy dependency.

The separate `tests/fixtures/leselang-product-hir-boundary` workspace now imports
the real 0.1.0 HIR and VM packages from the same Git revision, with ordinary
defaults, rather than testing a default-free local facade. It checks shared IR
identity, canonical/wire roundtrips, product authority/cold-operand checks and
the native VM's original fuel, scope revocation and sticky cancellation.

Local compatibility packages and their own conformance suites have not been
deleted. Frozen source-inspection snapshots and the historical exporter also
remain. Retiring these assets requires migrating their specific acceptance
obligations; removing ordinary product consumers does not close that gate.
`leselang-command`, `leselang-ui`, `leselang-observe` and `leselang-host-contract`
remain product adapters regardless of their historical prefix.

The local product handoff probe copies unmodified product Rust sources into an
isolated workspace and points their three shared-core dependencies at the native
repository. It is useful compatibility evidence, not proof of the final pinned
Git dependency, a live GUI run or all daemon/CLI integration.

## Separate Acceptance Gates

Repository bootstrap and complete runtime acceptance are different milestones.
Complete two-host language parity, host-neutral durable graph recovery and full
reference wire/authority/GUI/CLI parity remain open. The embedding tensor remains
developing; `complete_vm` and `full_extraction_gate` are not promoted. Historical
checkpoint flags and missing planned evidence remain unchanged.
