# Minor Line History

This section records the project's deliberate **minor-line snapshots**.

For `gewyvern`, the middle numeric component is treated as the **minor** line:

- `v0.13.x`
- `v0.14.x`
- future lines such as `v0.15.x`

The rule from here forward is simple:

- every new minor line gets one durable snapshot page
- that page records what the line meant, what changed in posture, and what
  evidence existed at the time
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
  Current active `0.14.x` line focused on protocol depth, compiler ergonomics,
  and runtime/report maturity.
- [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md)
  Reserved next minor-line slot.

## Release-Line Ledger

| Line | Role | Status | Canonical note |
| --- | --- | --- | --- |
| `v0.10.0` | Historical validation baseline before the later convergence and `0.14.x` posture work | historical baseline | [docs/history/v0.10.0.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.10.0.md) |
| `v0.13.x` | First deliberate convergence line for documentation, boundaries, and release judgment | recorded | [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md) |
| `v0.14.x` | Current active maturity line | active | [docs/history/v0.14.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.14.x.md) |
| `v0.15.x` | Next minor line slot | reserved | [docs/history/v0.15.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15.x.md) |

This table is the shortest answer to:

- which line was the pre-`0.14.x` convergence shelf?
- which line is active now?
- which line slot is next?

## How To Read These Pages

Use a minor-line snapshot when you want to answer:

- what this version line was trying to accomplish
- what the maintainers believed was already whole
- what was still intentionally incomplete
- what evidence existed for that judgment

For the IR-side archival baseline that can accompany one of these pages, the
repo now includes:

- [scripts/render_minor_line_ir_snapshot.sh](/Users/Shared/chroot/dev/gewyvern/scripts/render_minor_line_ir_snapshot.sh)
  Thin helper that renders Markdown-ready IR history snapshot blocks from one
  or more `.gewy` inputs.

Use other pages when you want something else:

- for the current top-level documentation map, use
  [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- for the current structured reading paths, use
  [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
- for the current active release posture, use
  [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
- for the shortest ledger of historical release lines, use this page
