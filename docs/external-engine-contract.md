# External Engine Contract

This note describes the smallest useful contract between `gewyvern` and an
external analysis engine.

For the broader role split between `gewyvern`, `etragon`, and `leserpent`, see
[docs/collaboration-boundary.md](/Users/Shared/chroot/dev/gewyvern/docs/collaboration-boundary.md).

For the broader surface contract note, including which CLI flags and analysis
fields are current contract candidates for downstream use, see
[docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md).

The goal is to let `gewyvern` keep ownership of:

- fact ingestion
- protocol and runtime analysis
- conservative built-in conclusions
- report and API surfaces

while an external engine adds:

- advisory enrichments
- rerank hints
- ML-derived candidates
- evidence-chain enrichments
- when the learned or inferred state is sufficiently stable, more direct
  diagnostic opinions

without changing the core `gewyvern` analysis model.

## Direction

The dependency direction should stay:

- external engine depends on `gewyvern` analysis shape
- `gewyvern` depends only on a generic runtime hook

In practice:

- `etragon -> gewyvern` at the contract level
- `gewyvern -> external engine` only through a process boundary
- `leserpent` remains the multi-instance orchestrator above both

`gewyvern` should not require a specific engine implementation at build time.

## Input

The preferred input is a `gewyvern` analysis snapshot:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

An external engine will usually care about fields such as:

- `primary_module_kind`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `ambiguous`
- `competing_hypotheses`
- existing `augmentations`

An engine may consume more fields, but those are the core ones expected to be
useful for enrich/rerank behavior.

## Process Hook

`gewyvern` currently invokes an external engine through:

- `--external-engine-bin <path>`
- optional `--external-engine-worker <path>`
- optional `--external-engine-python-bin <path>`

The engine process is expected to accept an analysis snapshot on stdin in the
current integration mode and return a JSON object on stdout.

`gewyvern` does not assume the engine is `etragon`; `etragon` is only the
current sibling implementation used in examples.

## Output

The external engine should return a JSON object with:

```json
{
  "augmentations": [
    {
      "kind": "ml-candidate",
      "name": "ml_candidate_manual_review",
      "summary": "short human-readable explanation",
      "confidence": "candidate",
      "producer_stage": "candidate",
      "producer_pass": "python_baseline_worker",
      "data": {
        "module": "connection_establishment"
      }
    }
  ]
}
```

Only the top-level `augmentations` array is required.

Ready-to-use examples live here:

- [docs/fixtures/external_engine_input_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_input_example.json)
- [docs/fixtures/external_engine_output_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_output_example.json)

The input example shows the smallest `analysis.json` shape an engine is likely
to care about. The output example shows the append-only `augmentations` payload
shape that `gewyvern` expects back.

Each augmentation should be append-only. External engines should not try to
rewrite or delete built-in `gewyvern` augmentations.

## Augmentation Fields

Recommended fields for each augmentation:

- `kind`
  - coarse class such as `ml-candidate`, `recommendation`, `rerank`, `trust`
- `name`
  - stable machine-friendly identifier
- `summary`
  - short human-readable sentence
- `confidence`
  - lightweight confidence label such as `candidate`, `advisory`, `high`
- `producer_stage`
  - pipeline stage that produced it
- `producer_pass`
  - concrete pass or worker name
- `data`
  - optional machine-friendly JSON object with pass-specific details

If an engine also exposes richer sidecar-native routes or summaries, those are
best treated as companion diagnosis-partner surfaces rather than part of
`gewyvern`'s base analysis contract.

## Producer Metadata

External engines should populate:

- `producer_stage`
- `producer_pass`

whenever possible.

These fields make it much easier to:

- debug why a suggestion exists
- compare multiple passes
- merge batch results
- audit ML vs rule-based behavior later

Suggested `producer_stage` values:

- `candidate`
- `recommendation`
- `rerank`
- `external`

Suggested `producer_pass` values:

- `python_baseline_worker`
- `mock_ml_advisory`
- `targeted_escalation_rerank`

## Behavioral Rules

External engines should stay conservative:

- do not replace core `gewyvern` conclusions
- do not assume ambiguous cases are resolved unless evidence is much stronger
- prefer adding recommendations or candidate hints over rewriting facts
- preserve competing hypotheses when uncertainty remains
- treat `diagnostic_opinion` as a higher bar than recommendation or candidate output

Good external behavior:

- add `ml_candidate_targeted_escalation`
- add `ml_candidate_manual_review`
- add recommendation-style hints for orchestration

Less desirable behavior:

- force a single diagnosis when `ambiguous=true`
- erase built-in trust warnings
- reinterpret unverified lineage as strong PID attribution

## Failure Handling

If the external engine fails:

- `gewyvern` should keep built-in analysis intact
- the failure should degrade gracefully
- an `external_engine_failed` augmentation may be attached as an advisory note

External engines should therefore treat partial failure as normal and keep their
output focused on additive annotations.

## Batch Use

For live scan snapshots, the recommended pattern is:

1. read `/v1/latest/targets`
2. iterate through `target_refs[].path_segment`
3. request `/v1/latest/targets/<path-segment>/analysis.json`
4. run the external engine per target
5. optionally merge recommendations across the batch

This is the same shape used by `etragon analyze-targets-url`.

## Current Reference Implementation

The current sibling reference engine is `etragon`.

It demonstrates:

- file and URL ingestion of `analysis.json`
- target-specific and batch analysis
- candidate / recommendation / rerank layering
- Python-backed resident worker integration

But the contract described here is intended to outlive any single engine.
