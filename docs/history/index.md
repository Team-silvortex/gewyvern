# Minor Line History

This section records the project's deliberate **minor-line snapshots**.

For `gewyvern`, the middle numeric component is treated as the **minor** line:

- `v0.13.x`
- `v0.14.x`
- `v0.15.x`

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

- [docs/history/v0.10.0.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.10.0.md)
  Last fully documented early validation baseline before the later
  convergence and `0.14.x` maturity line.
- [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md)
  First deliberate convergence line before the later `0.14.x` posture.
- [docs/history/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/history/1.0-readiness.md)
  Companion historical readiness note from the pre-`1.0` convergence phase.
- [docs/history/v0.14.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.14.x.md)
  Earlier maturity line focused on protocol depth, compiler ergonomics, and
  runtime/report maturity.
- [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md)
  Current active line focused on carrying that maturity into runtime layout,
  upgrade handling, and clearer operational contracts.
- [docs/history/v0.15-to-v1-roadmap.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15-to-v1-roadmap.md)
  Forward-looking minor-line plan from `0.15.x` through `0.20.x`, intended to
  make `v1.0.0` the direct next step after the final pre-`1.0` seal.
- [docs/history/v0.16.x-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.16.x-checklist.md)
  Contract-tightening execution checklist for the planned `0.16.x` line.
- [docs/history/minor-line-evidence-bundle.md](/Users/Shared/chroot/dev/gewyvern/docs/history/minor-line-evidence-bundle.md)
  Small durable template for what validation artifacts should accompany a new
  minor-line history page.

## Release-Line Ledger

| Line | Role | Status | Canonical note |
| --- | --- | --- | --- |
| `v0.10.0` | Historical validation baseline before the later convergence and `0.14.x` posture work | historical baseline | [docs/history/v0.10.0.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.10.0.md) |
| `v0.13.x` | First deliberate convergence line for documentation, boundaries, and release judgment | recorded | [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md) |
| `v0.14.x` | Earlier maturity line before the `0.15.x` operationalization pass | historical snapshot | [docs/history/v0.14.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.14.x.md) |
| `v0.15.x` | Current active line for runtime layout, upgrade shape, and continued protocol/compiler depth | active | [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md) |
| `v0.15.x -> v1.0.0` | Forward roadmap through `v0.20.x` with `v1.0.0` as the intended direct successor | active roadmap | [docs/history/v0.15-to-v1-roadmap.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15-to-v1-roadmap.md) |

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

- [scripts/history/render_minor_line_ir_snapshot.sh](/Users/Shared/chroot/dev/gewyvern/scripts/history/render_minor_line_ir_snapshot.sh)
  Thin helper that renders Markdown-ready IR history snapshot blocks from one
  or more `.gewy` inputs.
- [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)
  Current cross-project gate that can also emit one small `resilience-summary`
  text artifact for archive-friendly runtime-collaboration evidence.
- [docs/history/minor-line-evidence-bundle.md](/Users/Shared/chroot/dev/gewyvern/docs/history/minor-line-evidence-bundle.md)
  Compact rule sheet for what to preserve alongside the history page when one
  line's validation posture needs durable companion artifacts.

Use other pages when you want something else:

- for the current top-level documentation map, use
  [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- for the current structured reading paths, use
  [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
- for the current active release posture, use
  [docs/v0.15-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.15-posture.md)
- for the shortest ledger of historical release lines, use this page
