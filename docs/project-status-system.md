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

Schema v3 carries a `coverage_requirements` manifest and a dated `calibration`
record. Coverage maps authoritative architecture ownership boundaries, roadmap
gates, and continuous proof shelves onto concrete cells. The manifest covers
Gewyvern Core, GewyLang, the Leserpent 1.x bridge, Leserpent 2, the Etragon
sidecar, and status governance itself. Validation is exhaustive and
bidirectional: every architecture with a cell must declare a requirement, every
requirement must reference an existing same-architecture cell, and every cell
must be covered by at least one requirement. This distinguishes a structurally
valid sparse tensor from a complete architecture map and prevents new
architectures from bypassing progress governance.

The canonical cell ID is:

```text
<architecture>/<module>/<feature>
```

## Cell Protocol

Every cell carries:

- lifecycle: current, bridge, target, or retired
- roadmap priority: critical, active, maintenance, or deferred
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
never inferred from completion alone. Priority describes current roadmap
attention, not architectural value; a deferred cell remains covered and scored
in the portfolio view.

## Derived Strength

The Rust status engine computes each cell's strength from:

```text
55% maturity + 45% completion - confidence penalty - blocker penalty
```

The score ranks attention; it does not replace the underlying fields. Schema v3
then derives two project views:

- **Delivery:** priority-weighted strength and completion, using critical = 4,
  active = 2, maintenance = 1, and deferred = 0. The JSON field
  `overall_score` is the delivery strength for backward compatibility.
- **Portfolio:** equal-weight strength and completion across every cell,
  including deferred work, exposed as `portfolio_score` and
  `portfolio_completion`.

The default `weakest`, `developing`, and `standalone` attention views exclude
deferred cells. The `deferred` view keeps them explicit instead of hiding them
or letting them distort active delivery.

Calibration also enforces coarse maturity/completion coherence: planned cells
cannot exceed 25, incubating cells cannot exceed 75, and stabilizing cells must
be at least 70. These are consistency fences, not automatic maturity inference.

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

The `standalone` query reports non-deferred, non-internal cells only after they
reach at least stabilizing maturity. A planned or deferred standalone component
remains visible in the catalog but is not reported as currently usable.

## Native Commands

```bash
cargo run --bin gewyvern_status -- validate
cargo run --bin gewyvern_status -- summary
cargo run --bin gewyvern_status -- weakest --limit 10
cargo run --bin gewyvern_status -- mature
cargo run --bin gewyvern_status -- standalone
cargo run --bin gewyvern_status -- developing
cargo run --bin gewyvern_status -- deferred
cargo run --bin gewyvern_status -- summary --json
```

Queries can be narrowed:

```bash
cargo run --bin gewyvern_status -- developing \
  --architecture leserpent-2

cargo run --bin gewyvern_status -- weakest \
  --module language-vm --json

cargo run --bin gewyvern_status -- mature \
  --priority critical --json
```

JSON output is the automation and model-facing surface. Human output is an
operator convenience and is not a stable parsing contract.

## Update Protocol

Any change that adds or materially changes an architecture, module, feature,
priority, contract, dependency, blocker, or extraction boundary must update the
catalog in the same change.

Use this order:

1. add or update dimension entries
2. add or update cells
3. assign roadmap priority from the current scope rather than component prestige
4. record contract version and stability
5. record dependencies, blockers, consumers, and evidence
6. choose the next falsifiable gate
7. update `calibration.as_of` when judgment fields are recalibrated
8. run status validation and tests

Do not mark work mature because it compiles. Do not increase completion without
new evidence. Do not remove a blocker without recording the evidence that
closed it.

## Validation

```bash
cargo test --test project_status_tdd
cargo run --bin gewyvern_status -- validate
```

Validation rejects unknown dimension references, duplicate or non-canonical
cell IDs, missing contracts, incoherent maturity/completion pairs, invalid
calibration metadata, missing present evidence, unknown dependencies,
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
