# Sidecar Collaboration

This note explains how `gewyvern` currently collaborates with a nearby
diagnosis-partner sidecar such as `etragon`.

It is intentionally narrower than orchestration. The goal is:

- `gewyvern` keeps ownership of facts, protocol/runtime analysis, and the core
  conservative diagnosis spine
- a sidecar may add higher-level evidence-chain enrichment or diagnostic
  opinion
- `gewyvern` may surface that sidecar output as additive context without
  letting it overwrite the built-in diagnosis spine

For the base process-hook payload shape, see
[docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md).

For the narrow machine-facing fields that automation should consume, see
[docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md).

## Role Split

`gewyvern` remains authoritative for:

- `primary_failure_*`
- `operator_guidance_*`
- `ambiguous`
- `competing_hypotheses`
- process/profile/flow summaries

A sidecar may contribute:

- `evidence_chain_enrichment`
- `diagnostic_opinion`
- additive `augmentations`

These are treated as collaboration context, not as replacements for the
built-in diagnosis spine.

## Broader Stack Boundary

The nearby-sidecar story also sits inside a broader three-part stack:

- `gewyvern`
  - single-runtime authority for facts, protocol/runtime analysis, and the
    conservative diagnosis spine
- `etragon`
  - near-runtime diagnosis partner that enriches evidence chains and may emit a
    more direct `diagnostic_opinion` only when its learned state is stable
- `leserpent`
  - multi-instance control plane above both, responsible for orchestration,
    policy, UI, and fleet browsing

The intended relationship remains:

```text
etragon <-> gewyvern
leserpent -> many gewyvern instances
leserpent -> optional etragon services
```

That boundary matters because:

- `gewyvern` should remain the source of runtime truth
- `etragon` should remain additive and nearby, not become the orchestrator
- `leserpent` may consume outputs from both, but should not collapse their
  roles together

## Merge Posture

When a sidecar returns richer top-level context, `gewyvern` currently:

1. folds it into synthetic external augmentations
2. preserves collaboration hints in augmentation `data`
3. exposes a simpler machine-facing summary alongside the main JSON surfaces

Current synthetic augmentation names:

- `external_evidence_chain_enrichment`
- `external_diagnostic_opinion`

This is intentionally additive-only. `gewyvern` does not rewrite
`primary_failure_*` or `operator_guidance_*` based on sidecar input.

## Machine-Facing Surfaces

Current additive machine-facing surfaces:

- `analysis.json`
- `summary.json`
- `findings.json`

Each may expose:

- `external_sidecar_context`

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
- `consumption_mode`

This exists so nearby consumers do not need to parse augmentation internals just
to understand whether the sidecar is only advisory, clearly mergeable, or
starting to offer a stronger nearby opinion.

## API Presence Hints

For cheap polling, the API also exposes additive presence signals.

At `/v1/capabilities`:

- `external_sidecar_context`
- `external_capability_profile`
- `external_context_status`
- `external_sidecar_trust_level`
- `external_sidecar_consumption_mode`

At `/v1/latest/meta`:

- `has_external_sidecar_context`
- `has_external_evidence_chain_enrichment`
- `has_external_diagnostic_opinion`
- `has_external_capability_profile`
- `external_capability_status`
- `external_hint_status`
- `external_context_status`
- `external_sidecar_trust_level`
- `external_sidecar_consumption_mode`

At `/v1/latest/targets` on each `target_ref`:

- `has_external_sidecar_context`
- `has_external_evidence_chain_enrichment`
- `has_external_diagnostic_opinion`
- `has_external_capability_profile`
- `external_capability_status`
- `external_hint_status`
- `external_context_status`
- `external_sidecar_trust_level`
- `external_sidecar_consumption_mode`

These flags are meant to help pollers decide whether a full target-level fetch
is worth doing.

Current `external_sidecar_trust_level` bands:

- `trusted`
  - capability profile verified and collaboration hints declared
- `degraded`
  - capability profile verified, but some sidecar hints were conservatively downgraded
- `unverified`
  - sidecar context exists, but the capability profile is missing or not compatible enough to trust strongly

Current `external_sidecar_consumption_mode` values:

- `append_only`
  - nearby sidecar output should remain additive context only
- `guidance_context`
  - nearby enrichment can be shown as guidance-adjacent context
- `operator_guidance_support`
  - nearby enrichment is strongly reinforcing the current built-in guidance
- `operator_review`
  - nearby opinion should be treated as operator-review context
- `guidance_candidate`
  - nearby opinion is the strongest guidance-candidate posture currently exposed

## Human-Facing Surfaces

`gewyvern` also exposes a very-light human-readable interpretation of sidecar
collaboration state.

Current summary-line hints:

- `external_enrichment_hint`
- `external_diagnostic_opinion_hint`
- `external_collaboration_state`
- `external_operator_guidance_support`

Current scan HTML behavior:

- each target card may show `External sidecar context`
- each target card may show `External operator-guidance support`
- the scan header may roll up:
  - `mergeable sidecar targets`
  - `automation-worthy sidecar targets`
  - `advisory-only sidecar targets`

This is deliberately explanatory only. These notes are there to help an
operator understand how the sidecar result should be read relative to the
built-in `gewyvern` conclusion.

## Current Collaboration Bands

The current sidecar-related human/machine bands are intentionally coarse:

- `advisory_only`
- `mergeable`
- `automation_worthy`

Typical interpretation:

- `advisory_only`
  - useful context, but not suitable for direct merging into built-in guidance
- `mergeable`
  - strong enough to enrich nearby interpretation and operator-facing context
- `automation_worthy`
  - unusually strong nearby sidecar opinion; still additive, but worth treating
    as a high-value external signal

## Operator Guidance Support

Sidecar output may also imply that it is supporting the current built-in
operator guidance rather than merely existing alongside it.

Current human-facing support states include:

- `operator_guidance_candidate`
- `guidance_supporting_enrichment`
- `guidance_context_only`

These are explanatory notes only. They do not mutate the actual
`operator_guidance_*` fields.

## Non-Goals

This collaboration surface is not trying to make `gewyvern` an orchestrator.

It is not meant to:

- let a sidecar replace the diagnosis spine
- let a sidecar rewrite ingest trust or PID attribution semantics
- make `gewyvern` depend on a specific engine implementation at build time
- introduce a second authoritative “final diagnosis” field

The intended shape is:

- `gewyvern` provides conservative truth
- a nearby sidecar provides additive higher-level context
- consumers decide how much weight to give that external context
