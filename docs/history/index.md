# Minor Line History

This section records the project's deliberate **minor-line snapshots**.

For `gewyvern`, the middle numeric component is treated as the **minor** line:

- `v0.13.x`
- `v1.4.x`
- future lines such as `v1.5.x`

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
  convergence and `1.x` maturity lines.
- [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md)
  First deliberate convergence line before the later `1.x` posture.
- [docs/history/v1.4.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v1.4.x.md)
  Current active `1.x` line focused on protocol depth, compiler ergonomics,
  and runtime/report maturity.
- [docs/history/v1.5.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v1.5.x.md)
  Reserved next minor-line slot.

## Release-Line Ledger

| Line | Role | Status | Canonical note |
| --- | --- | --- | --- |
| `v0.10.0` | Historical validation baseline before the later convergence and `1.x` posture work | historical baseline | [docs/history/v0.10.0.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.10.0.md) |
| `v0.13.x` | First deliberate convergence line for documentation, boundaries, and release judgment | recorded | [docs/history/v0.13.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.13.x.md) |
| `v1.4.x` | Current active `1.x` maturity line | active | [docs/history/v1.4.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v1.4.x.md) |
| `v1.5.x` | Next minor line slot | reserved | [docs/history/v1.5.x.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v1.5.x.md) |

This table is the shortest answer to:

- which line was the pre-`1.x` convergence shelf?
- which line is active now?
- which line slot is next?

## How To Read These Pages

Use a minor-line snapshot when you want to answer:

- what this version line was trying to accomplish
- what the maintainers believed was already whole
- what was still intentionally incomplete
- what evidence existed for that judgment

Use other pages when you want something else:

- for the current top-level documentation map, use
  [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- for the current structured reading paths, use
  [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
- for the current active release posture, use
  [docs/v1.4-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v1.4-posture.md)
- for the shortest ledger of historical release lines, use this page
