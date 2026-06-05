# Surface Stability

Use this page when you need the current compatibility-contract candidate for
operator-facing `gewyvern` surfaces.

It is the practical answer to:

- which CLI flags downstream users should depend on
- which JSON/API fields are part of the diagnosis contract
- which areas are intentionally still presentation-level or pass-specific

This is not a promise that every output byte is frozen forever.

It is the narrower and more useful promise that:

- the diagnosis spine should stop churning
- the primary CLI surface should stop drifting
- incidental presentation details should not be treated as contract

This page is not the best first stop for:

- exact diagnosis field meanings
- export bundle structure
- first-run CLI usage

For those, use:

- [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)

## Primary CLI Contract

The current day-to-day CLI surface that scripts, operators, and automation
should treat as the primary contract is:

- `--protocol <name>`
- `--entry <name>`
- `--scan-all`
- `--protocol-set <path>`
- `--pid <n>`
- `--json`
- `--summary-only`
- `--report-format html|json`
- `--out <path>`
- `--unix-socket <path>`
- `--tcp-socket <host:port>`
- `--serve`
- `--api-socket <host:port>`
- `--allow-remote-api`
- `--ingest-mode local-advisory|remote-advisory`
- `--external-engine-bin <path>`
- `--external-engine-worker <path>`
- `--external-engine-python-bin <path>`

Compatibility aliases such as:

- `--socket-trust ...`
- `--allow-remote-socket`

still work, but they should be treated as transitional, not preferred.

They are intentionally no longer part of the primary help/usage surface. New
scripts and operators should start from the preferred generic flags above.

The intended direction is:

- keep the socket-ingest aliases as supported compatibility entrypoints
- remove implementation-specific external-engine aliases before `v1.0.0`

That preserves old ingest scripts without tying the public `gewyvern` surface
to one specific external-engine implementation.

## Summary Contract

For `--summary-only --json`, the fields downstream users should prefer are:

- `kind`
- `name`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `operator_guidance_status`
- `operator_guidance_action`
- `operator_guidance_reason`
- `operator_guidance_summary`
- `ambiguous`
- `competing_hypotheses`
- `ingest_mode`
- `ingest_mode_note`
- `ingest_trust_mode`
- `pid_attribution_status`
- `pid_attribution_note`
- `augmentations`

These fields are the main operator-facing summary contract.

### Object Identity Semantics

The intended identity semantics are:

- `kind="single"`
  - this payload describes one rendered target
- `kind="scan"`
  - this payload describes a multi-target sweep
- `name`
  - canonical object name for the rendered unit when one exists
- `demo`
  - legacy-friendly single-target label kept for compatibility
- `target`
  - per-item name inside scan target arrays
- `target_count`
  - top-level count for multi-target or API snapshot listings

Practical guidance:

- prefer `kind + name` for new integrations
- treat `demo` as a compatibility label for older single-target consumers
- treat `target` as the item label inside scan result arrays
- treat `target_count` as the summary count field for scan/API listing surfaces

## Analysis Snapshot Contract

For:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

the fields downstream tools should prioritize are:

- `target_status`
- `primary_process_profile`
- `protocol_flows`
- `process_network_profiles`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `operator_guidance_status`
- `operator_guidance_action`
- `operator_guidance_reason`
- `operator_guidance_summary`
- `ambiguous`
- `competing_hypotheses`
- `suspect_modules`
- `augmentations`

This is the best machine-facing surface for enrich, rerank, and external-engine
integration.

## Scan Report Header Contract

For scan-level report JSON, the top contract fields are:

- `kind`
- `name`
- `target_count`
- `scan_all`
- `total_targets`
- `healthy_targets`
- `attention_targets`
- `idle_targets`

`target_count` and `total_targets` intentionally carry the same count.
`target_count` is the more general object-identity field; `total_targets` is
kept as the older scan-oriented label.

## External-Engine Contract

The external-engine contract is:

- `gewyvern` owns the core analysis snapshot
- external engines append to `augmentations`
- external engines should populate:
  - `kind`
  - `name`
  - `summary`
  - `confidence`
  - `producer_stage`
  - `producer_pass`
  - optional `data`

The external engine should not replace or delete built-in conclusions.

## API Target Routing Contract

For the API target-routing surface, downstream consumers should treat the
following as stable:

- `/v1/latest/targets`
- `target_refs[].name`
- `target_refs[].path_segment`
- `target_refs[].url_path`

New integrations should prefer `path_segment` and `url_path` over inventing
target routes from display names.

## Explicitly Non-Contract Areas

The following areas should still be treated as evolving and should not be used
as compatibility anchors:

- exact HTML layout and wording
- the full `report.json` shape outside the main diagnosis fields
- auxiliary `gewyc` JSON surfaces outside the documented compiler-facing fields
- the exact internal structure of built-in and external augmentation `data`

In other words:

- depend on the diagnosis spine
- avoid depending on incidental presentation details

## Practical Guidance

If you are integrating against `gewyvern`:

1. prefer `summary.json` for operator-focused automation
2. prefer `analysis.json` for enrich/rerank/ML pipelines
3. prefer `target_refs[].path_segment` from the API instead of inventing target URLs
4. prefer the generic external-engine flags over legacy `--etragon-*` aliases
