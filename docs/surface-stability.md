# Surface Stability

This note records the operator-facing surfaces that `gewyvern` intends to keep
stable throughout the `v0.9.x` release-freeze line.

It is not a forever compatibility promise yet. It is the practical answer to:
"which CLI flags, report fields, and analysis shapes should downstream users
start depending on now?"

## Stable-Enough CLI Surface For `v0.9.x`

The current day-to-day CLI surface that should be treated as stable enough for
scripts, operators, and automation is:

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
- `--etragon-bin`
- `--etragon-python-worker`
- `--etragon-python-bin`

still work, but they should be treated as transitional, not preferred.

## Stable-Enough Summary Fields

For `--summary-only --json`, the fields downstream users should prefer are:

- `kind`
- `name`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `ambiguous`
- `competing_hypotheses`
- `ingest_mode`
- `ingest_mode_note`
- `ingest_trust_mode`
- `pid_attribution_status`
- `pid_attribution_note`
- `augmentations`

These fields are the main `v0.9.x` contract for operator-facing summary logic.

### Object Identity Semantics

During `v0.9.x`, the intended identity semantics are:

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

## Stable-Enough Analysis Snapshot Fields

For:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

the fields downstream tools should prioritize are:

- `target_status`
- `protocol_flows`
- `process_network_profiles`
- `primary_module_kind`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `ambiguous`
- `competing_hypotheses`
- `augmentations`

This is the best machine-facing surface for enrich, rerank, and external-engine
integration during `v0.9.x`.

## Stable-Enough Scan Report Header Fields

For scan-level report JSON, the stable-enough top fields are:

- `kind`
- `name`
- `target_count`
- `scan_all`
- `total_targets`
- `healthy_targets`
- `attention_targets`
- `idle_targets`

For `v0.9.x`, `target_count` and `total_targets` intentionally carry the same
count. `target_count` is the more general object-identity field; `total_targets`
is kept as the older scan-oriented label.

## External-Engine Stability

For `v0.9.x`, the stable-enough external-engine contract is:

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

## Things Still Expected To Evolve

The following areas should still be treated as evolving:

- exact HTML layout and wording
- the full `report.json` shape outside the main diagnosis fields
- auxiliary `gewyc` JSON surfaces outside the documented compiler-facing fields
- the exact internal structure of built-in and external augmentation `data`

In other words:

- depend on the diagnosis spine
- avoid depending on incidental presentation details

## Practical Guidance

If you are integrating against `gewyvern` during `v0.9.x`:

1. prefer `summary.json` for operator-focused automation
2. prefer `analysis.json` for enrich/rerank/ML pipelines
3. prefer `target_refs[].path_segment` from the API instead of inventing target URLs
4. prefer the generic external-engine flags over legacy `--etragon-*` aliases
