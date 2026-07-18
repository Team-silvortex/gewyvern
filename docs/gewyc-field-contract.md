# gewyc Field Contract

Use this page when you need the field-level contract candidate for
`gewyc ... --json`.

This page answers:

- which fields new consumers should depend on first
- which fields are legacy compatibility carry-over
- which fields should be treated as presentation detail

Use these nearby pages with it:

- [docs/gewyc-json.md](docs/gewyc-json.md)
- [docs/gewyc-contract-matrix.md](docs/gewyc-contract-matrix.md)
- [docs/gewyc-sample-index.md](docs/gewyc-sample-index.md)
- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/surface-stability.md](docs/surface-stability.md)

## Status Words

This page uses four field statuses:

- `blessed`
  New consumers should depend on this field.
- `compat`
  Still supported for compatibility, but not the preferred first read.
- `derived`
  Useful, but better treated as a rendered convenience than a routing anchor.
- `evolving`
  Presently useful, but still too shape-sensitive for hard dependency.

## Version Policy

This page uses one conservative retirement policy for the `0.x` line:

- `blessed`
  Expected to stay preferred through the current minor line and into the next
  tightening line unless a future contract note says otherwise.
- `compat`
  Supported now, but new consumers should already migrate away. These should
  not disappear inside the same minor line that still documents them.
- `derived`
  May stay useful for a while, but should not become a hard machine contract
  without being promoted later.
- `evolving`
  May widen, narrow, or regroup only with deliberate post-`1.0.0` review.

Retirement rule:

1. a field is marked `compat`
2. a later minor line may mark it `strictly_legacy`
3. only after that should removal be considered

For the current `1.4.0` line, the practical reading promise remains:

- `blessed` fields are safe to adopt now
- `compat` fields are safe to keep reading now
- no field documented as `compat` on this page should be removed without an
  intentional `1.0.0`-line review or later breaking-change review

That is a planning floor, not a promise that every `compat` field will
definitely survive all the way to `1.0.0`.

## Wrapper Contract

These top-level wrapper fields are the current contract candidate for every
`gewyc ... --json` surface.

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `surface_id` | `blessed` | `1.0.0+` | Canonical routing token. |
| `schema_hint.family` | `blessed` | `1.0.0+` | Current parser family gate. |
| `schema_hint.surface` | `blessed` | `1.0.0+` | Exact surface selector. |
| `schema_hint.schema_version` | `blessed` | `1.0.0+` | Parser version gate. |
| `contract_hint.stability` | `blessed` | `1.0.0+` | Current release-line stability hint. |
| `contract_hint.compatibility` | `blessed` | `1.0.0+` | Read-strategy hint for consumers. |
| `contract_hint.legacy_fields` | `blessed` | `1.0.0+` | Legacy-payload presence hint. |
| `payload` | `blessed` | `1.0.0+` | Container for surface-specific shape. |

## Frontend Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.report.status.present` | `blessed` | `1.0.0+` | Presence gate for the rendered frontend report. |
| `payload.report.authoring` | `blessed` | `1.0.0+` | Preferred grouped authoring shelf. |
| `payload.report.counts` | `blessed` | `1.0.0+` | Preferred grouped topology/count shelf. |
| `payload.summary.kind` | `blessed` | `1.0.0+` | Short-form route for pipeline/type posture. |
| `payload.summary.focus` | `blessed` | `1.0.0+` | Focus routing signal when `--focus` is active. |
| `payload.focused_report` | `blessed` | `1.0.0+` | Shared focused detail entrypoint. |
| `payload.report.module_doc` | `compat` | `0.19.x` | Kept for detail renderers. |
| `payload.report.template_doc` | `compat` | `0.19.x` | Kept for detail renderers. |
| `payload.report.function_nodes` | `compat` | `0.19.x` | Exact graph detail, not the first route. |
| `payload.report.graph_nodes` | `compat` | `0.19.x` | Exact graph detail, not the first route. |
| `payload.report.graph_edges` | `compat` | `0.19.x` | Exact graph detail, not the first route. |
| `payload.report.expansion_previews` | `evolving` | `post-1.0.0 review` | Useful, but still a richer frontend detail shape. |

## Binding Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.status.has_window` | `blessed` | `1.0.0+` | Fast posture gate. |
| `payload.status.has_reason_profile` | `blessed` | `1.0.0+` | Fast posture gate. |
| `payload.status.has_program_model` | `blessed` | `1.0.0+` | Fast posture gate. |
| `payload.counts.fragments` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.fragment_params` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.evidence_overrides` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.template_id` | `compat` | `0.19.x` | Still supported for older consumers. |
| `payload.fragments` | `compat` | `0.19.x` | Exact binding detail. |
| `payload.window` | `compat` | `0.19.x` | Exact binding detail. |
| `payload.reason_profile` | `compat` | `0.19.x` | Exact binding detail. |
| `payload.program_model` | `compat` | `0.19.x` | Exact binding detail. |
| `payload.fragment_params` | `compat` | `0.19.x` | Exact binding detail. |
| `payload.evidence_overrides` | `compat` | `0.19.x` | Exact binding detail. |

## Diagnostics Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.status.has_program_model` | `blessed` | `1.0.0+` | Fast presence gate. |
| `payload.status.has_reason_model` | `blessed` | `1.0.0+` | Fast presence gate. |
| `payload.counts.fragments` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.program_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.reason_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.template_id` | `compat` | `0.19.x` | Older consumers may still route on this. |
| `payload.program_model` | `compat` | `0.19.x` | Exact per-rule detail. |
| `payload.reason_model` | `compat` | `0.19.x` | Exact per-rule detail. |
| `payload.fragments` | `compat` | `0.19.x` | Exact fragment detail. |

## Findings Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.summary.finding_count` | `blessed` | `1.0.0+` | Fast summary gate for standalone findings. |
| `payload.summary.next_step` | `blessed` | `1.0.0+` | Preferred follow-up hint for standalone findings. |
| `payload.findings` | `blessed` | `1.0.0+` | Stable narrow contract for standalone findings. |
| `payload.findings[].stage` | `blessed` | `1.0.0+` | Stage classifier. |
| `payload.findings[].severity` | `blessed` | `1.0.0+` | Severity classifier. |
| `payload.findings[].code` | `blessed` | `1.0.0+` | Machine-readable finding identity. |
| `payload.findings[].line` | `blessed` | `1.0.0+` | Source coordinate when available. |
| `payload.findings[].column` | `blessed` | `1.0.0+` | Source coordinate when available. |
| `payload.findings[].message` | `compat` | `0.19.x` | Useful, but more presentation-oriented than `code`. |

## Stages Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.summary.finding_count` | `blessed` | `1.0.0+` | Fast phase-summary gate. |
| `payload.summary.next_step` | `blessed` | `1.0.0+` | Preferred follow-up hint before drilling into phase detail. |
| `payload.status.parse_ok` | `blessed` | `1.0.0+` | Phase gate. |
| `payload.status.validation_ok` | `blessed` | `1.0.0+` | Phase gate. |
| `payload.status.diagnostics_ok` | `blessed` | `1.0.0+` | Phase gate. |
| `payload.counts.validation_fragments` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.validation_program_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.validation_reason_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.sampled_payload_offsets` | `blessed` | `1.0.0+` | Coverage posture. |
| `payload.counts.required_payload_offsets` | `blessed` | `1.0.0+` | Coverage posture. |
| `payload.counts.unsupported_payload_offsets` | `blessed` | `1.0.0+` | Coverage posture. |
| `payload.parse` | `compat` | `0.19.x` | Detailed phase body. |
| `payload.validation` | `compat` | `0.19.x` | Detailed phase body. |
| `payload.diagnostics` | `compat` | `0.19.x` | Detailed phase body. |
| `payload.frontend` | `compat` | `0.19.x` | Useful compiler context, but not the phase gate itself. |

## Envelope Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.summary.finding_count` | `blessed` | `1.0.0+` | Fast aggregate gate before nested surface reads. |
| `payload.summary.next_step` | `blessed` | `1.0.0+` | Preferred aggregate follow-up hint. |
| `payload.status.has_binding` | `blessed` | `1.0.0+` | Summary posture gate. |
| `payload.status.has_diagnostics` | `blessed` | `1.0.0+` | Summary posture gate. |
| `payload.status.finding_count` | `blessed` | `1.0.0+` | Summary posture gate. |
| `payload.surfaces.binding` | `blessed` | `1.0.0+` | Preferred grouped entrypoint. |
| `payload.surfaces.diagnostics` | `blessed` | `1.0.0+` | Preferred grouped entrypoint. |
| `payload.surfaces.findings` | `blessed` | `1.0.0+` | Preferred grouped entrypoint. |
| `payload.surfaces.stages` | `blessed` | `1.0.0+` | Preferred grouped entrypoint. |
| `payload.binding` | `compat` | `0.19.x` | Legacy top-level compatibility mirror. |
| `payload.diagnostics` | `compat` | `0.19.x` | Legacy top-level compatibility mirror. |
| `payload.findings` | `compat` | `0.19.x` | Legacy top-level compatibility mirror. |
| `payload.stages` | `compat` | `0.19.x` | Legacy top-level compatibility mirror. |

## Explain Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.ok` | `blessed` | `1.0.0+` | Top-level success posture. |
| `payload.summary.stage_status` | `blessed` | `1.0.0+` | First routing gate for editors and panels. |
| `payload.summary.analysis` | `blessed` | `1.0.0+` | Preferred grouped analysis shelf. |
| `payload.summary.shape_notes` | `blessed` | `1.0.0+` | Preferred grouped shape-note shelf. |
| `payload.summary.excerpts` | `blessed` | `1.0.0+` | Preferred grouped excerpt shelf. |
| `payload.summary.next_step` | `blessed` | `1.0.0+` | Preferred operator next-step hint. |
| `payload.focused_report` | `blessed` | `1.0.0+` | Shared focused drilldown entrypoint. |
| `payload.frontend` | `compat` | `0.19.x` | Detailed child surface mirror. |
| `payload.binding` | `compat` | `0.19.x` | Detailed child surface mirror. |
| `payload.validation` | `compat` | `0.19.x` | Detailed child surface mirror. |
| `payload.diagnostics` | `compat` | `0.19.x` | Detailed child surface mirror. |
| `payload.findings` | `compat` | `0.19.x` | Detailed child surface mirror. |
| `payload.summary.authoring_context` | `compat` | `0.19.x` | Older flat summary field, still retained. |
| `payload.summary.lowered_binding_summary` | `compat` | `0.19.x` | Older flat summary field, still retained. |
| `payload.summary.frontend_lowering_delta` | `compat` | `0.19.x` | Older flat summary field, still retained. |
| `payload.summary.parse_source_excerpt` | `compat` | `0.19.x` | Older flat summary field, still retained. |
| `payload.summary.validation_excerpt` | `compat` | `0.19.x` | Older flat summary field, still retained. |
| `payload.summary.diagnostics_excerpt` | `compat` | `0.19.x` | Older flat summary field, still retained. |

## IR History Snapshot Surface

Recommended first reads:

| Field | Status | Earliest tightening | Notes |
| --- | --- | --- | --- |
| `payload.status.has_program_model` | `blessed` | `1.0.0+` | Presence gate. |
| `payload.status.has_reason_model` | `blessed` | `1.0.0+` | Presence gate. |
| `payload.status.has_model_compare` | `blessed` | `1.0.0+` | Presence gate. |
| `payload.counts.program_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.counts.reason_rules` | `blessed` | `1.0.0+` | Preferred grouped count. |
| `payload.analysis.model_compare` | `blessed` | `1.0.0+` | Preferred grouped comparison shelf. |
| `payload.analysis.history_snapshot` | `blessed` | `1.0.0+` | Preferred grouped archival shelf. |
| `payload.template_id` | `compat` | `0.19.x` | Exact detail identity. |
| `payload.program_model` | `compat` | `0.19.x` | Exact lowered model detail. |
| `payload.reason_model` | `compat` | `0.19.x` | Exact lowered model detail. |
| `payload.model_compare` | `compat` | `0.19.x` | Exact comparison detail. |
| `payload.history_snapshot` | `compat` | `0.19.x` | Exact archival detail. |

## Reading Rule

When in doubt:

1. route with wrapper fields
2. prefer grouped fields under `payload`
3. use `compat` fields only when you need exact old detail blocks
4. avoid building long-lived automation on `derived` or `evolving` fields first
