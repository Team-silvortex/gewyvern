# Export Format Contract

Use this page when you want the narrow machine-facing contract candidate for
the export bundle, not the longer replay explanation.

This page answers:

- which bundle fields replay consumers should depend on first
- which fields are replay-critical rather than convenience summaries
- what must stay true before the next tightening line

Use these nearby pages with it:

- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
- [docs/machine-surface-freeze.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-surface-freeze.md)
- [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)

## Preferred Contract

New replay and dataset consumers should depend on these first:

- top-level bundle shape
- `template_id`
- `fragment_inventory`
- `attach_plan`
- `attach_report`
- `window_profile`
- `reason_profile_id`
- `reason_profile`
- `facts`
- `flows`
- `program_flows`
- `reasons`

Those are the first-read replay-critical shelves.

## Current Stable Reads

Treat these as the current contract candidate:

| Area | Preferred read | Current status |
| --- | --- | --- |
| bundle identity | `template_id` | `blessed` |
| replay inventory | `fragment_inventory` | `blessed` |
| runtime IR posture | `attach_plan` | `blessed` |
| attach outcome posture | `attach_report` | `blessed` |
| planner/debug support | `binding_diagnostics` | `blessed` |
| runtime window semantics | `window_profile` | `blessed` |
| reason identity | `reason_profile_id` | `blessed` |
| reason semantics | `reason_profile` | `blessed` |
| physical evidence stream | `facts` | `blessed` |
| replay materialized state | `flows`, `program_flows`, `reasons` | `blessed` |
| convenience summary | `debug_summary`, `attach_failure_summary`, `rejected_fact_summary` | `compat` |

## Current Compatibility Carry-Over

These summary shelves are still useful, but should not replace replay-critical
inputs as the first machine dependency:

- `debug_summary`
- `attach_failure_summary`
- `rejected_fact_summary`

They help CLI and UI views, but replay consumers should still anchor on the
full evidence and state shelves first.

## Freeze Gate

Treat the export bundle as frozen enough for the next minor tightening step
only when:

1. replay-critical top-level fields remain explicit
2. convenience summaries remain clearly secondary
3. any new field can be explained as additive to replay
4. deterministic replay inputs remain documentable without reverse-engineering

## Earliest Tightening Reading

For the current planning posture:

- replay-critical fields should remain dependable through `0.18.x`
- convenience summaries may still evolve as long as they do not replace the
  replay spine
- future export growth should stay replay-first rather than analytics-first
