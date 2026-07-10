# Minor Line History

This section records the project's deliberate **minor-line snapshots**.

For `gewyvern`, the middle numeric component is treated as the **minor** line:

- `v0.13.x`
- `v0.14.x`
- `v0.15.x`
- `v0.17.x`
- `v0.18.x`
- `v0.19.x`
- `v0.20.x`

The rule from here forward is simple:

- every new minor line gets one durable snapshot page
- that page records what the line meant, what changed in posture, and what
  evidence existed at the time
- when collaboration validation matters, keep the archive-friendly summaries
  produced by the current validation helpers instead of relying on memory
- create that page when the line really starts, not as an empty placeholder
- patch releases inside the same minor line do **not** get their own separate
  history page unless they materially redefine the line

This framework intentionally starts at `v0.13.x`.

That was the first line where the project treated documentation convergence,
surface discipline, and release judgment as first-class work instead of as
background cleanup.

## Recorded Minor Lines

- [docs/history/v0.10.0.md](docs/history/v0.10.0.md)
  Last fully documented early validation baseline before the later
  convergence and `0.14.x` maturity line.
- [docs/history/v0.13.x.md](docs/history/v0.13.x.md)
  First deliberate convergence line before the later `0.14.x` posture.
- [docs/history/1.0-readiness.md](docs/history/1.0-readiness.md)
  Archived readiness note from the earlier `v0.13.x` through `v0.15.x`
  convergence phase; useful as historical rationale, not as the current line's
  ship read.
- [docs/history/v0.14.x.md](docs/history/v0.14.x.md)
  Earlier maturity line focused on protocol depth, compiler ergonomics, and
  runtime/report maturity.
- [docs/history/v0.15.x.md](docs/history/v0.15.x.md)
  Historical line focused on carrying earlier maturity into runtime layout,
  upgrade handling, and clearer operational contracts.
- [docs/history/v0.17.x.md](docs/history/v0.17.x.md)
  Historical line focused on family-first protocol deepening plus stronger
  compiler/IR maturity without losing runtime evidence discipline.
- [docs/history/v0.17.x-midline-checklist.md](docs/history/v0.17.x-midline-checklist.md)
  Second-half checklist used to close the `0.17.x` line cleanly before the
  `0.18.x` validation and runtime-confidence line.
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)
  Historical line focused on protocol breadth, packaged/runtime confidence,
  and physical-host validation.
- [docs/history/v0.18.x-pathological-container.md](docs/history/v0.18.x-pathological-container.md)
  Physical-host pathological container evidence for runtime ingest fault
  handling and resilience degradation.
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
  Historical integration line focused on integrated debugger behavior,
  reliability hardening, cross-validation, and pre-seal convergence.
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
  Current active final pre-`1.0` sealing line focused on repeatable release
  gates, surface freeze judgment, and documentation/system coherence.
- [docs/history/v0.15-to-v1-roadmap.md](docs/history/v0.15-to-v1-roadmap.md)
  Forward-looking minor-line plan from `0.15.x` through `0.20.x`, intended to
  make `v1.0.0` the direct next step after the final pre-`1.0` seal.
- [docs/history/v0.16.x-checklist.md](docs/history/v0.16.x-checklist.md)
  Protocol-semantics and runtime-evidence execution checklist for the planned
  `0.16.x` line.
- [docs/history/minor-line-evidence-bundle.md](docs/history/minor-line-evidence-bundle.md)
  Small durable template for what validation artifacts should accompany a new
  minor-line history page.

## Release-Line Ledger

| Line | Role | Status | Canonical note |
| --- | --- | --- | --- |
| `v0.10.0` | Historical validation baseline before the later convergence and `0.14.x` posture work | historical baseline | [docs/history/v0.10.0.md](docs/history/v0.10.0.md) |
| `v0.13.x` | First deliberate convergence line for documentation, boundaries, and release judgment | recorded | [docs/history/v0.13.x.md](docs/history/v0.13.x.md) |
| `v0.14.x` | Earlier maturity line before the `0.15.x` operationalization pass | historical snapshot | [docs/history/v0.14.x.md](docs/history/v0.14.x.md) |
| `v0.15.x` | Runtime layout, upgrade shape, and operationalization baseline before later deepening lines | recorded baseline | [docs/history/v0.15.x.md](docs/history/v0.15.x.md) |
| `v0.17.x` | Protocol-cluster deepening plus compiler/IR maturity with runtime-evidence discipline | historical snapshot | [docs/history/v0.17.x.md](docs/history/v0.17.x.md) |
| `v0.18.x` | Protocol breadth, packaged/runtime confidence, and physical-host validation | historical snapshot | [docs/history/v0.18.x.md](docs/history/v0.18.x.md) |
| `v0.19.x` | Integrated debugger behavior, reliability hardening, and pre-seal convergence | historical snapshot | [docs/history/v0.19.x.md](docs/history/v0.19.x.md) |
| `v0.20.x` | Final pre-`1.0` seal with repeatable release gates and frozen core surfaces | active | [docs/history/v0.20.x.md](docs/history/v0.20.x.md) |
| `v0.15.x -> v1.0.0` | Forward roadmap through `v0.20.x` with `v1.0.0` as the intended direct successor | active roadmap | [docs/history/v0.15-to-v1-roadmap.md](docs/history/v0.15-to-v1-roadmap.md) |

This table is the shortest answer to:

- which line was the pre-`0.14.x` convergence shelf?
- which line is active now?
- how minor-line history is recorded once a new line really starts?

## How To Read These Pages

Use a minor-line snapshot when you want to answer:

- what this version line was trying to accomplish
- what the maintainers believed was already whole
- what was still intentionally incomplete
- what evidence existed for that judgment

The runtime history shelf now also preserves a machine-readable protocol-catalog
trail, including the latest two-snapshot delta summary for:

- added protocol families
- removed protocol families
- changed protocol summaries
- added entry surfaces
- removed entry surfaces
- changed entry surface contracts

For the IR-side archival baseline that can accompany one of these pages, the
repo now includes:

- [scripts/history/render_minor_line_ir_snapshot.sh](scripts/history/render_minor_line_ir_snapshot.sh)
  Thin helper that renders Markdown-ready IR history snapshot blocks from one
  or more `.gewy` inputs.
- `cargo run --quiet --bin gewyvern_validate -- three-module-stack-smoke`
  Current cross-project gate that can also emit one small `resilience-summary`
  text artifact for archive-friendly runtime-collaboration evidence.
- `cargo run --quiet --bin gewyvern_validate -- pathological-container-validation`
  Containerized malformed-client gate for runtime ingest resilience and
  post-fault service continuity.
- [docs/history/minor-line-evidence-bundle.md](docs/history/minor-line-evidence-bundle.md)
  Compact rule sheet for what to preserve alongside the history page when one
  line's validation posture needs durable companion artifacts.

Use other pages when you want something else:

- for the current top-level documentation map, use
  [docs/index.md](docs/index.md)
- for the current structured reading paths, use
  [docs/book/index.md](docs/book/index.md)
- for the current active release posture, use
  [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- for the shortest ledger of historical release lines, use this page
