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

The earlier local product handoff probe copied unmodified product Rust sources
into an isolated workspace and pointed their three shared-core dependencies at
the native repository. It is historical compatibility evidence, not proof of the
final pinned Git dependency, a live GUI run or all daemon/CLI integration.

## Source Owners

The new repository contains `leselang-syntax`, `leselang-runtime-core`,
`leselang-hir-core`, native `leselang-hir` and native `leselang-vm`, plus the
separate consumer, examples, pinned lockfiles, toolchain, MIT LICENSE and CI.
Default and all-features builds have no product profile or SQLite dependency.
Native foundation development belongs there from this point forward.

Gewyvern retains `leserpent-hir`, `leserpent-vm`, command/host-contract/UI/observe
adapters and the old product-enabled HIR/VM compatibility entry points. The
frozen acceptance snapshots are retired: the three local core directories contain
only navigation READMEs, remain excluded from the workspace and cannot be built.
`tests/leselang_repository_tdd.rs` checks the complete reviewed source inventory,
byte lengths and hashes against the actual Cargo-resolved upstream.
Do not develop divergent native implementations or overwrite the new repository
with another export. The old source exporter is retired; historical checkpoint
records remain evidence, not a second release or development workflow.

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
Gewyvern CI runs the pinned-source gate and `cargo dev check leselang`, which reads
the actual Cargo-resolved Git checkout rather than rebuilding a source export.

To update Leselang, review an upstream commit, update all exact pins and lockfiles,
and explicitly review the source inventory, hashes, documentation evidence and
consumer acceptance map for that commit. Run the source gate, product tests and
independent product-HIR fixture before committing. Never bypass a hash mismatch
by weakening the gate or copying upstream sources back into this repository.
A sibling-directory dependency alone is not releasable.

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
deleted. Core source snapshots are retired, but retiring compatibility packages
still requires migrating their specific API acceptance
obligations; removing ordinary product consumers does not close that gate.
`leselang-command`, `leselang-ui`, `leselang-observe` and `leselang-host-contract`
remain product adapters regardless of their historical prefix.

### Compatibility Verification Boundaries

| Entry | Configuration | Obligation |
| --- | --- | --- |
| Local `leselang-hir` | No default features | Legacy native imports, core identity, foreign-host contracts and product-exclusion compile-fail docs |
| Local `leselang-hir` | Default product profile | Released product API and `product_identity` re-export checks |
| Local `leselang-vm` | No defaults / product memory / default SQLite | Distinct native, memory-only product and durable legacy API paths |
| `leserpent-hir` / `leserpent-vm` | Product-owned profiles | Actual lowering/evaluation, frozen migration outcomes and product policy |
| Pinned native consumer and separate boundary fixture | Their own locked workspaces | Independent public packages and cross-repository integration |

The native-only local HIR suite runs once in the integration job. Eight former
per-target invocations used the identical configuration and are now covered by
that complete run; their test sources and registered targets remain unchanged.
`language_independence_tdd` rejects filtered/compile-only replacements and repeat
invocations, verifies that the covered targets remain runnable without required
features, and retains product-specific oracle gates. The full workspace job still
checks all features, which is a different API configuration, not a replacement
for native-only compile-fail tests. This consolidation does not retire either
facade or claim native durable-recovery completion.

## Pinned Consumer Verification

The [pinned consumer proof](fixtures/leselang_pinned_consumer_macos_arm64_20261104.json)
records this tool/CI migration separately from the unchanged dependency cutover.

`cargo dev check leselang` verifies both the product's three shared-core identities
and the foreign fixture's five native identities against the handoff pin. It then
validates and executes the native HIR library's private tests and the separate
`consumer/` workspace from that exact Cargo checkout, followed by the product compatibility fixture, strict lints and five
native example modes. It does not depend on a sibling checkout or copy/rewrite
sources. Identity, product-feature and dependency checks precede test execution;
an unavailable offline dependency is an error, never a fallback to an old snapshot.

Both consumers share the existing Cargo target directory and `lean` profile.
`--dry-run` checks metadata and prints stages without running the tests or lints.
The workflow does not delete caches. Upstream owns the native workspace's full
test matrix; this product-side gate checks its actual upstream consumer and our
integration, rather than a regenerated product-versioned language workspace.

The first migration removed the exporter from CI and checked twelve upstream
acceptance files. Its proof above keeps that original scope and result count.

## Exporter Retirement

The [retirement proof](fixtures/leselang_exporter_retirement_macos_arm64_20261104.json)
records the replacement acceptance map, complete local checks and verified removal
of the unmodified old generated 2.3.0 source directory. Earlier proofs remain intact.

The old source exporter and its manifest rewriting, source copying, publication
and disposable-cache machinery are removed. The retired `package leselang`
command explains the replacement and exits without writing output; it does not
silently run a different task. Development and release ownership stay upstream.

The shared `tests/support/leselang_upstream.rs` acceptance map now covers all
39 inherited consumer test targets, two recovery helpers and the native example.
It verifies actual pinned files byte-for-byte and requires the exact registered
test inventory, including upstream's additional repository-boundary suite. The
native/VM/language gates read this source and metadata rather than exporter text.
Product-only support and benchmark exclusion are checked against actual upstream
directories and targets. No test bodies are copied or rewritten by the verifier.

Exporter-only publication tests are obsolete because publication no longer exists
here. Historical verification records retain their original hashes, versions and
flags, including the earlier `source_exporter_retired = false`. Current cleanup
state is in the handoff JSON. The retirement proof predates core snapshot removal
and retains its original flags. Compatibility facades remain; full VM acceptance
is not promoted by removing this tool or the later source copies.

## Core Inspection Ownership

Language architecture, native HIR/VM profile and safety gates now read native
core files from the actual Cargo-resolved checkout, through
`tests/support/leselang_upstream.rs`. All three cores must share the reviewed
repository, exact revision, version and checkout; duplicate/local/mutable or
split-checkout identities reject before inspection. Missing upstream files do not
fall back to frozen snapshots or a sibling checkout. Product adapters and the
local compatibility facades are still inspected here, not redirected upstream.

The three frozen core `Cargo.toml` files are removed. They were obsolete inherited
manifests outside the workspace, not independent build entries. Source/test copies
have since been removed with their evidence preserved by the inventory below. Old proof
records retain their original manifest hashes. Current source-inspection scope is
tracked in the handoff JSON, with positive/negative routing and identity acceptance
in `tests/leselang_source_ownership_tdd.rs` and the pinned-integration CI job.
This does not retire compatibility APIs, change a dependency pin or promote full
runtime acceptance. Snapshot retirement preserves the original evidence rather
than discarding checks. Runtime documentation ownership is migrated separately below.

## Migration Oracle Retirement

The old HIR `migration_oracles` target and its computation support wrapper are
removed. They compiled 64 native modules as local test code to access private
kernels. The complete 121-test inventory is preserved in the
[owner map](../crates/leserpent-hir/tests/fixtures/migration-boundaries/ownership.json):
93 checks already belong to the product HIR library, 25 belong to the native HIR
library, and three compare the scalar/fold/loop product boundaries.

Before removal, all 6,912 boundary observations were compared against the live
product, then their frozen results were recorded with source/revision provenance.
Product unit tests now compare complete results, ordered diagnostics, node counters
and remaining scope through the real external core. The original source forms and
budget/depth matrices remain in Rust. Fixture hashes, unique case ownership and
complete case consumption are checked; there is no auto-update or capture mode.
See the [recorded baseline rules](../crates/leserpent-hir/tests/fixtures/migration-boundaries/README.md).

`cargo dev check leselang` additionally runs `leselang-hir-core --lib` in the exact
resolved upstream workspace, retaining the old native checks without recompiling
snapshots in this repository. The captured owner runs passed 97 product and 29
native library tests; subsequent baseline-reader checks are additive. Product
workspace source-inclusion guards prevent restoring the old coupling. The later
snapshot retirement also replaces byte-copy comparisons with recorded-source checks.
No private native API, runtime behavior, dependency pin or full-VM claim changes.

## Runtime Documentation Ownership

The duplicated runtime-core manual is retired. Its local README is now a short
navigation page, not a native package guide. The original detailed acceptance
notes already live in upstream `docs/provenance/runtime-core-baseline.md`.
Before removal, the complete Boundary-to-Standalone-Package-Check body was checked
byte-for-byte against that archive; later product cutover notes remain here.

All 17 historical semantic documentation checks now read the original archive
from the actual Cargo-resolved pinned checkout. The handoff JSON records its
path, revision and SHA-256. The reader rejects a changed hash, mutable/different
revision, substitute path or missing upstream file, with no local/sibling fallback.
Current product ownership and extraction-status checks still read local guides,
not old upstream prose. The status tensor links the metadata and executable
documentation gate alongside the navigation page; no maturity score is promoted.

Historical provenance is not current build guidance. It deliberately retains
the original checkpoint's package commands and extraction status. Use the pinned
native README for upstream checks, and `cargo dev check leselang` for product
integration. A new dependency pin must review the documentation evidence together
with source/API compatibility; do not silently copy a new manual back here or
regenerate its expected hash during tests. Earlier proof files remain unchanged.

## Status Source Inventory

Status schema v4 separates local files from pinned upstream source evidence. The
[source inventory](../project/status/leselang-source-inventory.json) records all
166 native core source/test files with exact byte lengths and SHA-256. Its initial
capture checked the entire retained inventory byte-for-byte against the actual
Cargo-resolved revision before recording it. All 303 current status references
to 161 of those paths now name this inventory, keeping their original path, kind
and state rather than collapsing them into one generic handoff link.
The old syntax-directory source reference also becomes an explicit `src/lib.rs`
entry, for 304 inventory references in total; a directory left with only a README
cannot stand in for implementation evidence.

The status validator checks local inventory metadata without fetching source,
running Cargo or probing a sibling repository. The source-owner gate checks the
handoff identity and complete evidence routing; the repository gate independently
checks all 166 paths/hashes against the resolved upstream tree, including additions
and removals. See the [evidence protocol](project-status-system.md#external-source-evidence)
for bounds, failure behavior and the distinction between recorded identity and
actual test execution. The remaining consumer comparisons were migrated separately
below; no version pin, product version, runtime protocol, maturity score or
historical verification result is changed.

## Core Snapshot Retirement

All 166 local core source/test files are removed after a final full path/byte/hash
comparison with the recorded inventory and resolved upstream. Their original
2,004,299 bytes are no longer duplicated here. Three short navigation READMEs keep
old directory links useful, but no manifest, Rust source or nested backup tree may
return. The repository gate checks both this retirement and the entire upstream
inventory, including unexpected additions or missing files.

The five remaining runtime-source identity checks now verify recorded hashes.
The 39-target consumer map still compares actual executable test bodies and checks
registered targets: native-core originals are read from upstream only after their
recorded size/hash matches; retained facade originals still come from this product
checkout. The two recovery helpers and example retain their existing comparisons.
No test target, fixture body, type-identity check or execution stage is removed.
Negative tests cover changed/oversized/missing native bytes, missing inventory
entries, local-copy fallback and accidental redirection of product owners.

The open-source core policy names immutable public source URLs for the three
external cores and verifies the actual Cargo packages' MIT license metadata.
Navigation directories alone cannot satisfy that source-availability check.
Local facade/API retirement remains separate work; this cleanup does not change
syntax, runtime behavior, dependency identity or complete-VM acceptance.

## Separate Acceptance Gates

Repository bootstrap and complete runtime acceptance are different milestones.
Complete two-host language parity, host-neutral durable graph recovery and full
reference wire/authority/GUI/CLI parity remain open. The embedding tensor remains
developing; `complete_vm` and `full_extraction_gate` are not promoted. Historical
checkpoint flags and missing planned evidence remain unchanged.
