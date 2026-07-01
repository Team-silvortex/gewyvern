# Machine Surface Freeze

Use this page when you are tightening any machine-facing `gewyvern` surface for
the next minor line.

This page is the shared freeze ritual above the narrower shelves such as:

- `gewyc` JSON compiler surfaces
- runtime config file contract
- runtime certificate policy/status surfaces
- export bundle JSON

Use these nearby pages with it:

- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/surface-stability.md](docs/surface-stability.md)
- [docs/gewyc-freeze-checklist.md](docs/gewyc-freeze-checklist.md)
- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/runtime-config-contract.md](docs/runtime-config-contract.md)
- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)
- [docs/runtime-certificate-policy-contract.md](docs/runtime-certificate-policy-contract.md)
- [docs/export-format.md](docs/export-format.md)
- [docs/export-format-contract.md](docs/export-format-contract.md)

## What Freeze Means Here

Freeze does not mean the project stops evolving.

It means:

- the preferred read path is explicit
- compatibility carry-over is documented
- real fixtures or operational checks exist
- tightening happens by named minor lines, not by accident

## Shared Freeze Questions

Before tightening any machine surface, answer these:

1. what is the canonical routing field or entrypoint?
2. what is the preferred grouped or structured read path?
3. which fields or keys are still compatibility carry-over?
4. what proves the current shape in a real artifact, fixture, or validation run?
5. what is the earliest minor line where any non-preferred field may tighten?

If one of those answers is still fuzzy, the surface is not ready to freeze.

## Surface Families

### Compiler Surfaces

Primary shelf:

- [docs/gewyc-freeze-checklist.md](docs/gewyc-freeze-checklist.md)

Use when the concern is:

- wrapper routing
- grouped `payload` shelves
- fixture-backed JSON contract discipline

### Runtime Config Surface

Primary shelf:

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/runtime-config-contract.md](docs/runtime-config-contract.md)

Freeze focus:

- config search order
- supported sections and keys
- precedence between config, environment, CLI, and legacy fallback
- schema-version behavior for current and legacy-unversioned files

Treat this surface as frozen enough for the next minor step only when:

- search order is explicit
- supported keys are enumerated
- unknown-key rejection posture is documented
- migration and copy-forward behavior are named

### Runtime Certificate Policy Surface

Primary shelf:

- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)
- [docs/runtime-certificate-policy-contract.md](docs/runtime-certificate-policy-contract.md)

Freeze focus:

- top-level status vocabulary
- reason-code stability
- operator action mapping
- relationship between inventory and interpreted policy

Treat this surface as frozen enough for the next minor step only when:

- UIs and automation can bind to `reason.code`
- status words remain narrower than prose summaries
- new checks would land additively instead of silently reshaping old meanings

### Export Bundle Surface

Primary shelf:

- [docs/export-format.md](docs/export-format.md)
- [docs/export-format-contract.md](docs/export-format-contract.md)

Freeze focus:

- deterministic replay inputs
- stable top-level bundle shape
- replay-critical semantics like window, reason, facts, flows, and reasons

Treat this surface as frozen enough for the next minor step only when:

- replay-critical fields are clearly separated from convenience summaries
- top-level shape is still documented as a contract, not a debug dump
- any new field can be explained as additive rather than replacing old replay inputs

## Shared Freeze Walk

Run freeze work in this order:

1. identify the canonical machine entrypoint
2. identify the preferred first-read grouped or structured fields
3. mark compatibility carry-over explicitly
4. confirm a real sample, fixture, or operational validation path exists
5. update the release-line note if the meaning of the line changes

## Current `0.19.x -> 0.20.x` Reading

For the current line, a good practical freeze posture is:

- `gewyc` grouped JSON shelves are becoming the preferred first read
- runtime config behavior is explicit enough that migration and search order are reviewable
- certificate policy reason codes are stable enough for panel and automation binding
- export format remains replay-first rather than becoming a generic analytics blob
- debugger cross-validation is now part of release evidence, not only a
  convenience smoke

That is enough to start tightening discipline without pretending the `1.0.0`
contract is already finished.

## Maintenance Rule

When a machine surface changes in a way that affects consumer behavior:

1. update the narrow contract page
2. update the broader freeze shelf if the preferred read path changes
3. update release-line notes when the line’s meaning becomes sharper

That keeps exact contract, release posture, and operator expectations aligned.
