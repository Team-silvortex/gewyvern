# Minor-Line Evidence Bundle

Use this page when the question is:

- what small evidence bundle should accompany a new minor-line history page?
- what should we archive from validation instead of leaving in terminal scrollback?
- how do we keep `v0.16.x`, `v0.17.x`, and later lines comparable?

Do not use this page as:

- the full release checklist
- the full validation matrix
- the meaning of one specific minor line

For those, use:

- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md)

## Goal

Each minor line should leave behind one small, durable bundle of evidence that
answers:

- what did we actually validate?
- what collaboration posture existed at the time?
- what resilience posture existed at the time?

The bundle should be easy to archive, easy to compare, and small enough that it
does not become its own maintenance burden.

## Minimum Bundle

When collaboration and runtime posture matter, keep at least:

1. the minor-line history page itself
2. one IR history snapshot when IR shape is part of the line's meaning
3. one archive-friendly resilience summary from the three-module stack smoke
4. any short log or text excerpt needed to explain a failing or degraded claim

This does not require a huge artifact pack.
It requires a stable, reviewable summary of what the line proved.

## Current Helper Inputs

The current tree already has helpers for two of these:

- [scripts/history/render_minor_line_ir_snapshot.sh](/Users/Shared/chroot/dev/gewyvern/scripts/history/render_minor_line_ir_snapshot.sh)
- [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)

The three-module script now emits a small `resilience-summary.txt` artifact.

If you want to keep it outside the temporary working directory, run with:

```bash
RESILIENCE_SUMMARY_PATH=/absolute/path/to/resilience-summary.txt \
  bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh
```

## Suggested Layout

One practical pattern per new line is:

```text
docs/history/
  v0.16.x.md
artifacts/history/
  v0.16.x/
    ir-snapshot.md
    resilience-summary.txt
```

The exact artifact folder may evolve.
The important part is that the history page can point to stable companion
artifacts instead of vague memories.

## Protocol Catalog Companion

From the protocol-catalog work onward, each persisted runtime/history snapshot
should also retain the machine-readable protocol shelf that existed at that
moment.

At minimum, keep:

- `protocols.json`
- `protocols/<protocol>/summary.json`
- `protocols/<protocol>/entries/<entry>/surface.json`

This matters because a minor line is not only about runtime health.
It is also about what protocol surface the line actually claimed to support.

That makes protocol-growth claims reviewable across lines like:

- `v0.15.x`
- `v0.16.x`
- `v0.17.x`

without relying on memory or prose-only changelogs.

## What Good Looks Like

A good minor-line evidence bundle is:

- small
- specific
- reproducible
- explicit about healthy versus degraded posture
- clear about which evidence came from runtime behavior versus documentation judgment

## What To Avoid

Avoid:

- giant uncurated log dumps
- one-off filenames that change every time without explanation
- history pages that claim maturity without any linked validation artifact
- keeping only console output that disappears after the session ends
