# Project Status Tensor

This page defines the machine-readable project management protocol used from
the `1.0.0 -> 2.0.0` line onward.

The source of truth is [project/status/catalog.json](../project/status/catalog.json).
Its protocol schema is
[project/status/schema.json](../project/status/schema.json). Markdown roadmaps
explain intent; they do not maintain a second status table.

## Model

The catalog is a sparse three-dimensional tensor:

```text
architecture x module x feature -> status cell
```

- **Architecture** identifies a product or durable system boundary, such as
  Gewyvern Core, GewyLang, Leserpent 1.x, Leserpent 2.0, or Etragon.
- **Module** identifies the implementation and ownership boundary.
- **Feature** identifies the independently judged capability.

Only meaningful intersections become cells. Empty combinations are not
implicitly planned work.

Schema v2 also carries a `coverage_requirements` manifest. It maps authoritative
architecture ownership boundaries, roadmap gates, and continuous proof shelves
onto concrete cells. The manifest covers Gewyvern Core, GewyLang, the
Leserpent 1.x bridge, Leserpent 2, the Etragon sidecar, and status governance
itself. Validation is exhaustive and bidirectional: every architecture with a
cell must declare a requirement, every requirement must reference an existing
same-architecture cell, and every cell must be covered by at least one
requirement. This distinguishes a structurally valid sparse tensor from a
complete architecture map and prevents new architectures from bypassing
progress governance.

The canonical cell ID is:

```text
<architecture>/<module>/<feature>
```

## Cell Protocol

Every cell carries:

- lifecycle: current, bridge, target, or retired
- maturity: planned, incubating, developing, stabilizing, mature, deprecated,
  or blocked
- completion from 0 to 100
- confidence: low, medium, or high
- independence: internal, reusable library, standalone tool, standalone
  service, or replaceable frontend
- a versioned contract with stability and named surfaces
- dependencies and blockers
- known consumers
- present or planned evidence
- one concrete next gate

Every coverage requirement carries:

- a stable requirement ID and owning architecture
- a kind: ownership boundary, roadmap gate, or proof shelf
- an authoritative repository document
- one or more cells that provide its progress and evidence

Completion is an evidence-backed estimate, not a release promise. Confidence
shows how much trust to place in that estimate. Maturity is categorical and is
never inferred from completion alone.

## Derived Strength

The Rust status engine computes a score from:

```text
55% maturity + 45% completion - confidence penalty - blocker penalty
```

The score ranks attention; it does not replace the underlying fields.

A `mature` cell is rejected unless:

- completion is at least 85
- its contract is stable
- it has no blockers
- present test evidence exists

Present evidence paths must exist in the repository. Planned evidence may
point to the intended future location.

## Independence

Independence answers whether a part can be used outside its current assembly:

- `internal`: meaningful only inside its owning architecture
- `reusable-library`: consumable through a library/data contract
- `standalone-tool`: independently invokable command
- `standalone-service`: independently deployable service
- `replaceable-frontend`: interchangeable renderer or client

The `standalone` query reports non-internal cells only after they reach at least
stabilizing maturity. A planned standalone component remains visible in the
catalog but is not reported as usable.

## Native Commands

```bash
cargo run --bin gewyvern_status -- validate
cargo run --bin gewyvern_status -- summary
cargo run --bin gewyvern_status -- weakest --limit 10
cargo run --bin gewyvern_status -- mature
cargo run --bin gewyvern_status -- standalone
cargo run --bin gewyvern_status -- developing
cargo run --bin gewyvern_status -- summary --json
```

Queries can be narrowed:

```bash
cargo run --bin gewyvern_status -- developing \
  --architecture leserpent-2

cargo run --bin gewyvern_status -- weakest \
  --module language-vm --json
```

JSON output is the automation and model-facing surface. Human output is an
operator convenience and is not a stable parsing contract.

## Update Protocol

Any change that adds or materially changes an architecture, module, feature,
contract, dependency, blocker, or extraction boundary must update the catalog
in the same change.

Use this order:

1. add or update dimension entries
2. add or update cells
3. record contract version and stability
4. record dependencies, blockers, consumers, and evidence
5. choose the next falsifiable gate
6. run status validation and tests

Do not mark work mature because it compiles. Do not increase completion without
new evidence. Do not remove a blocker without recording the evidence that
closed it.

## Validation

```bash
cargo test --test project_status_tdd
cargo run --bin gewyvern_status -- validate
```

Validation rejects unknown dimension references, duplicate or non-canonical
cell IDs, missing contracts, missing present evidence, unknown dependencies,
self-dependencies, dependency cycles, and unsupported schema versions.
It also rejects architectures without a coverage manifest, duplicate
requirements, missing source documents, unknown or cross-architecture cell
mappings, empty mappings, and orphan cells. Coverage sources are deliberately
architecture-specific: the project blueprint owns Gewyvern Core, the GewyLang
system page owns the language shelf, the Leserpent roadmap owns both migration
bridge and 2.0 gates, and the sidecar collaboration contract owns Etragon.

## Relationship To Roadmaps

The [root roadmap](../ROADMAP.md) chooses direction. The
[Leserpent 2.0 roadmap](leserpent-2-roadmap.md) defines delivery gates. The
status tensor records where each concrete architecture-module-feature cell is
relative to those gates.

When they disagree:

1. implementation evidence determines current reality
2. the tensor must be corrected
3. the roadmap may then be adjusted

This prevents aspirational text from silently becoming reported progress.
