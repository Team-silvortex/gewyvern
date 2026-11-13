# External Engine Contract

This note describes the smallest useful contract between `gewyvern` and an
external analysis engine.

For the broader nearby-sidecar and stack boundary note, see
[docs/sidecar-collaboration.md](docs/sidecar-collaboration.md).

For the broader surface contract note, including which CLI flags and analysis
fields are current contract candidates for downstream use, see
[docs/surface-stability.md](docs/surface-stability.md).

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

When the sibling engine is collecting replayable supervised samples rather than
doing only online enrichment, it may instead prefer the stable training surface:

- `/v1/latest/training-example.json`
- `/v1/latest/targets/<path-segment>/training-example.json`
- `/v1/latest/training-dataset.json`
- `/v1/latest/targets/<path-segment>/training-dataset.json`

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

When the engine is consuming the training surface as a supervised dataset, it
should prefer the structured target heads under:

- `supervision.targets.diagnosis`
- `supervision.targets.guidance`
- `supervision.targets.automation`
- `supervision.targets.ranking`

Those labels exist so a sibling engine such as `etragon` can consume a more
model-oriented supervision surface without having to infer task heads from the
operator-facing summary fields.

For batch collection, the engine should prefer the dataset manifest routes first
and then follow each declared `sample_path` to fetch the concrete
`training-example.json` payloads.

The dataset manifest now also exposes:

- stable `sample_id`
- `group_key` for coarse protocol-family grouping
- multiple reproducible `split_hints`

so a sibling engine can start with the built-in deterministic buckets and later
override them with a richer trainer-side policy without losing replayability.

## Process Hook

`gewyvern` currently invokes an external engine through:

- `--external-engine-bin <path>`
- optional `--external-engine-worker <path>`
- optional `--external-engine-python-bin <path>`

The engine process is expected to accept an analysis snapshot on stdin in the
current integration mode and return a JSON object on stdout.

`gewyvern` does not assume the engine is `etragon`; `etragon` is only the
current sibling implementation used in examples.

### Failure Isolation

Each process-hook invocation, including the capability handshake, has one
five-second deadline covering stdin delivery, stdout/stderr capture and child
completion. A leader exiting does not turn inherited open pipes into success.
Stdout is limited to 1 MiB and stderr to 256 KiB; overflow is rejected immediately,
without waiting for EOF. Timeout, incomplete input or oversized output produces
the existing explicit external-engine fallback, preserving built-in diagnosis.
The handshake and analysis are separate invocations, not one shared five-second
budget. No-payload capability queries receive closed stdin.

The hook and native validation commands share
`silvortex_bounded_io::process::run_command_output`. On Unix, parent pipe handles
are nonblocking and cancellation-aware; failure signals only the private process
group created for that invocation, then reaps the direct child and joins pipe
workers. The leader is not reaped while inherited pipes remain outstanding,
avoiding stale process-group IDs during failure cleanup. Successful commands may
leave descendants only if they close captured pipes. This is resource containment,
not a security sandbox: descendants that deliberately leave the process group are
not killed, but cannot keep the host's pipe workers alive on Unix.

At most 64 I/O workers may be reserved in one process, before child creation.
Exhaustion returns a retryable I/O error rather than creating unbounded threads.
On non-Unix hosts, blocking inherited handles can retain a worker slot until they
close; returning on timeout does not replenish that slot. Native Windows process
tree cancellation still needs separate implementation and platform evidence.
OS process creation/reaping and uninterruptible kernel waits are not preemptible;
the deadline is not a hard real-time guarantee. Linux regressions cover inherited
input/output, output flooding, repeated resource recovery and process-group isolation.

### Recovery and Caching

Only successful analysis results enter the bounded snapshot cache. Failure and
circuit-open fallbacks are never cached: identical input can retry, contribute to
the failure circuit, and recover after its cooldown without restarting Gewyvern.
Cache hits on earlier successful analyses do not reset failure counters. The
default circuit opens after three failures for 30 seconds; the existing
`GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD` and
`GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS` settings control these values.

Failed capability handshakes are retained for five seconds, measured after the
attempt completes, rather than forever. During that interval, successful analysis
results may be cached with conservative advisory metadata. After it expires, the
next uncached-eligible request retries the handshake and clears results derived
from the old capability state before analyzing again. An open circuit still
prevents launching either child. This is demand-driven recovery, not a background
retry loop; it also rate-limits legacy engines that do not implement capabilities.
Successfully parsed capability profiles remain cached until configuration changes.

`tests/external_analysis_recovery_tdd.rs` starts the production Gewyvern binary,
feeds identical facts through its socket service and reads the actual runtime API.
A native Rust fixture injects nonzero exit, malformed output, oversized output,
timeout and capability-only failure. The tests check fallback, circuit bypass,
recovery without restart, capability-cache refresh, unchanged built-in evidence
and retention of successful cache hits. Run this proof on Linux with:

```sh
cargo test -p gewyvern --locked --test external_analysis_recovery_tdd -- --nocapture
```

## Capability Handshake

Before trusting sidecar collaboration hints too strongly, `gewyvern` may also
ask the engine for a capability declaration through:

- `<external-engine-bin> protocol-capabilities`

The current reference sidecar shape advertises:

- `protocol_family`
- `protocol_version`
- `merge_capabilities.safe_automation_hints`
- `merge_capabilities.operator_review_hints`
- `handoff_capabilities.readiness_levels`
- `context_capabilities.published_contexts`
- `compatibility.forward_compatibility_rules`

If that capability profile is missing, uses another protocol family, or reports
an unsupported protocol version, `gewyvern` now downgrades sidecar merge hints
to conservative defaults instead of trusting stronger collaboration labels.

If a sidecar publishes a richer context surface such as
`evidence_chain_enrichment` or `diagnostic_opinion` without declaring that
surface in `context_capabilities.published_contexts`, `gewyvern` also
conservatively downgrades the collaboration posture for that context.

In practice that means:

- unknown or unverified `handoff_readiness` falls back to `advisory_only`
- evidence enrichment falls back to append-only augmentation context
- diagnostic opinion falls back to sidecar-only operator context

`gewyvern` now also derives a stable machine-facing consumption posture from the
normalized hint set, so downstream tooling can consume:

- raw `handoff_readiness`
- raw `merge_hint`
- normalized `consumption_mode`

without having to keep its own copy of the downgrade table.

This keeps the sidecar additive even when the sibling stack evolves faster than
the local `gewyvern` build.

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

If an external engine also wants to publish a higher-level diagnosis-partner
surface in the same payload, `gewyvern` now tolerates these optional top-level
objects as additive context:

- `evidence_chain_enrichment`
- `diagnostic_opinion`

When present, `gewyvern` currently folds them back into synthetic external
augmentations rather than treating them as changes to the core diagnosis spine.
It also exposes a machine-facing `external_sidecar_context` block in
`analysis.json`, `summary.json`, and `findings.json` so nearby consumers do not
have to reverse-engineer augmentation internals just to understand sidecar
handoff state.

Ready-to-use examples live here:

- [docs/fixtures/external_engine_input_example.json](docs/fixtures/external_engine_input_example.json)
- [docs/fixtures/external_engine_output_example.json](docs/fixtures/external_engine_output_example.json)

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

For those richer companion objects, a sidecar may also include:

- `handoff_readiness`
- `gewyvern_merge_hint`

These values are not part of `gewyvern`'s narrow built-in analysis contract.
They are a very-light collaboration hint that helps `gewyvern` decide whether a
sidecar result is best surfaced as:

- augmentation-only context
- augmentation plus operator-guidance context
- or a stronger sidecar-only diagnostic opinion candidate

When the engine also exposes a capability profile, `gewyvern` treats those hint
values as declared only if they appear in the capability allow-lists. Unknown
values are downgraded conservatively.

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
