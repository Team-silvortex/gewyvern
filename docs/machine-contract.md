# Machine Contract

This note defines the narrow machine-facing contract that downstream tooling
should rely on when integrating with `gewyvern`.

It exists to separate:

- the stable diagnosis spine that automation should depend on
- the wider report/presentation payloads that may still evolve

For the broader release posture, see
[docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md).

For the operator-facing surface overview, see
[docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md).

## Scope

This contract is intentionally narrow.

It is the recommended machine-facing surface for:

- automation
- rerank/enrich pipelines
- external-engine integration
- sidecar and orchestration adapters

It is not a promise that every JSON field produced by `gewyvern` is equally
stable.

## Preferred Inputs By Use Case

### Operator Automation

Prefer:

- `--summary-only --json`

This is the best entrypoint when the consumer wants one rendered conclusion per
target and does not need the full flow arrays.

### ML / Enrichment / External Engines

Prefer:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

This is the best entrypoint when the consumer wants the diagnosis spine plus
the target's materialized protocol/process summaries.

### Scan Enumeration

Prefer:

- `/v1/latest/targets`

Use `target_refs[].path_segment` as the canonical routing token for target-level
requests. Do not invent target URLs from display names.

### Rich Rendered Reports

`report.json` and scan-level report JSON are still useful, but they should be
treated as richer rendered report surfaces rather than the primary machine
contract.

Use them when you want:

- a broader rendered payload for operator tooling
- scan-level rollups and presentation-oriented context
- a convenient all-in-one report surface

Do not prefer them over `summary.json` or `analysis.json` for long-lived
automation contracts.

## Stable Core: Summary JSON

For `--summary-only --json`, downstream tools should treat the following fields
as the stable core:

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

### Summary Contract Semantics

- `primary_failure_*`
  - the main conservative diagnosis spine
- `operator_guidance_*`
  - the built-in next-step guidance surface for standalone `gewyvern`
- `ambiguous`
  - whether the current target should be treated as multi-hypothesis
- `competing_hypotheses`
  - the concrete competing explanation hints behind that ambiguity
- `augmentations`
  - additive built-in or external annotations; these do not replace the
    diagnosis spine

## Stable Core: Analysis Snapshot JSON

For:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

downstream tools should treat the following fields as the stable core:

- `target_status`
- `primary_process_profile`
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
- `external_sidecar_context`
- `process_network_profiles`
- `protocol_flows`

### Analysis Contract Semantics

- `target_status`
  - top-level rendered state such as healthy, attention, or idle
- `primary_process_profile`
  - the current best process-level rollup when one exists
- `process_network_profiles`
  - process-level grouped diagnosis summaries
- `protocol_flows`
  - the target's protocol-path summaries
- `suspect_modules`
  - additive module-level suspicion hints, not the primary contract on their
    own
- `external_sidecar_context`
  - the additive machine-facing summary of richer sidecar collaboration output
    when an external diagnosis partner publishes higher-level context

### Stable Subshape: `primary_process_profile`

When `primary_process_profile` is non-null, downstream tools should treat the
following fields as the stable subshape:

- `pid`
- `comm`
- `status`
- `ambiguous`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `competing_hypotheses`
- `operations`
- `module_kinds`
- `phases`
- `missing_transitions`
- `suspect_areas`
- `suspect_modules`
- `healthy_flows`
- `attention_flows`

### Stable Subshape: `process_network_profiles[]`

Each item in `process_network_profiles` should be treated as carrying the same
stable subshape as `primary_process_profile`.

That means downstream automation may safely depend on process-level grouped
diagnosis summaries without separately reverse-engineering their field set.

### Stable Subshape: `protocol_flows[]`

Each item in `protocol_flows` should be treated as the stable protocol-path
summary shape:

- `program_flow`
- `process`
- `operation`
- `network_module_kind`
- `network_module_kinds`
- `status`
- `failure_mode`
- `failure_detail`
- `failure_confidence`
- `failure_basis`
- `phases`
- `last_phase`
- `missing_transitions`
- `suspect_areas`

The nested `process` object should be treated as the stable lightweight process
shape:

- `pid`
- `comm`

## Stable Core: Scan Header JSON

For scan-level report JSON, the stable header fields are:

- `kind`
- `name`
- `target_count`
- `scan_all`
- `total_targets`
- `healthy_targets`
- `attention_targets`
- `idle_targets`

Use these fields for top-level scan status and count logic.

## Stable Core: API Target Routing

The stable API target-routing surface is:

- `/v1/latest/targets`
- `target_refs[].name`
- `target_refs[].path_segment`
- `target_refs[].url_path`

The contract expectation is:

- `name`
  - human-facing target name
- `path_segment`
  - canonical machine-facing routing token
- `url_path`
  - ready-to-use route path for direct fetches

`target_refs[]` may also expose additive presence hints:

- `has_external_sidecar_context`
- `has_external_evidence_chain_enrichment`
- `has_external_diagnostic_opinion`

These flags do not change the routing contract. They are lightweight polling
hints that help consumers choose which targets are worth deeper sidecar-aware
fetches.

## Additive Contract: API Meta Presence Signals

For lightweight polling, `/v1/latest/meta` may also expose additive presence
signals for richer sidecar collaboration context:

- `has_external_sidecar_context`
- `has_external_evidence_chain_enrichment`
- `has_external_diagnostic_opinion`

These fields are intended as cheap routing hints:

- pollers can decide whether they need to fetch `analysis.json`
- consumers can distinguish "no sidecar context at all" from "sidecar context
  exists but only one collaboration surface is present"

`/v1/capabilities` also declares:

- `external_sidecar_context`

to indicate that the running API surface knows how to publish these additive
presence signals.

## Additive Contract: Augmentations

`augmentations` are append-only.

Downstream consumers should assume:

- the diagnosis spine remains authoritative
- augmentations add hints, enrichments, recommendations, or external opinions
- built-in and external augmentations may coexist

Recommended stable subfields on each augmentation:

- `kind`
- `name`
- `summary`
- `confidence`
- `producer_stage`
- `producer_pass`
- `data`

Only the outer augmentation shape should be treated as stable by default.
Pass-specific `data` internals should be treated as looser unless separately
documented.

## Additive Contract: External Sidecar Context

When an external engine publishes richer diagnosis-partner output such as
`evidence_chain_enrichment` or `diagnostic_opinion`, `gewyvern` may surface an
additive top-level:

- `external_sidecar_context`

This field is intended to be easier for machine consumers to read than parsing
augmentation internals directly.

Current subshape:

- `evidence_chain_enrichment`
- `diagnostic_opinion`

Each subobject may be `null` or carry:

- `summary`
- `confidence`
- `producer_stage`
- `producer_pass`
- `handoff_readiness`
- `merge_hint`

Contract expectation:

- the core diagnosis spine remains authoritative
- sidecar context is additive only
- `handoff_readiness` and `merge_hint` should be treated as collaboration hints
  rather than replacements for built-in `operator_guidance_*`

## Explicitly Non-Contract Areas

Do not treat the following as stable integration anchors:

- exact HTML layout or wording
- full `report.json` payload shape outside the diagnosis spine
- incidental ordering of decorative fields
- convenience family labels such as `*_family` helper fields unless you are
  deliberately using them as display hints
- pass-specific augmentation `data` internals
- undocumented `gewyc` JSON surfaces

## Recommended Integration Pattern

For most consumers:

1. use `summary.json` when you want one conservative result per rendered target
2. use `analysis.json` when you want flow/process detail plus enrich/rerank
   inputs
3. treat `report.json` as a richer rendered report surface, not as the primary
   long-lived automation schema
4. use `/v1/latest/targets` to enumerate machine-stable target routes
5. consume `operator_guidance_*` before inventing a separate "next action"
   layer
6. treat augmentations as additive, not as replacements for the core diagnosis
