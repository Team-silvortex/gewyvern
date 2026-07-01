# etragon

`etragon` is the diagnosis-partner sidecar of the `gewyvern` stack.

Its default deployment shape is close and local:

- one `etragon` works with one nearby `gewyvern`
- it consumes `gewyvern` analysis snapshots
- it adds higher-level evidence-chain enrichments and, when stable enough, more direct diagnostic opinions

It can also work as a small federated learning node for multiple nearby
`gewyvern` runtimes. In that mode, `etragon` aggregates analysis/training across
a set of runtime target indexes, while `leserpent` still owns fleet
orchestration, UI, and policy.

Its first job is deliberately small:

- accept a machine-friendly analysis snapshot from `gewyvern`
- run one or more external analysis passes
- emit extra augmentations without changing `gewyvern`'s core report surfaces

## Current shape

This first scaffold is contract-first and dependency-light.

It provides:

- `AnalysisSnapshotInput`
- `AnalysisAugmentation`
- `EngineOutput`
- `ExternalAnalysisEngine`
- `CandidateAugmenter`
- `RecommendationAugmenter`
- `RerankPass`
- `PassPipeline`
- `MockMlAdvisoryEngine`
- `MockRecommendationAugmenter`
- `MockScoreRerankPass`

The mock engine is not a real model. It exists to prove the intended
single-runtime diagnosis-partner shape for later ML or rerank passes:

- ambiguous mixed hypotheses -> `ml_candidate_multi_hypothesis`
- medium-confidence missing transition -> `ml_candidate_observe_longer`
- high-confidence direct protocol signal -> `ml_candidate_targeted_escalation`

## CLI

You can run the scaffold directly against a saved `gewyvern` analysis snapshot:

```bash
cargo run -p etragon -- analyze-json target/validation/analysis.json
```

Use `-` to read the snapshot from stdin:

```bash
cat target/validation/analysis.json | cargo run -p etragon -- analyze-json -
```

Or fetch directly from a live `gewyvern` API snapshot:

```bash
cargo run -p etragon -- analyze-url http://127.0.0.1:9910/v1/latest/analysis.json
```

For a target-specific snapshot, point `etragon` at the target route directly:

```bash
cargo run -p etragon -- analyze-url http://127.0.0.1:9910/v1/latest/targets/scan:http:request/analysis.json
```

`analyze-url` currently supports plain `http://...` endpoints so local development
and monorepo stack integration stay dependency-light.

If you want to pull the target index and analyze every current target in one pass:

```bash
cargo run -p etragon -- analyze-targets-url http://127.0.0.1:9910/v1/latest/targets
```

If you only want a subset of the current targets, add a path-segment prefix filter:

```bash
cargo run -p etragon -- analyze-targets-url http://127.0.0.1:9910/v1/latest/targets --filter scan:
```

That returns a small batch payload keyed by `path_segment`, with each target
carrying its own external augmentation output.

Batch responses also include a top-level `recommendation_summary`, which merges
augmentation names by `producer_stage` and `producer_pass`. That gives a nearby
operator, sidecar consumer, or upper-layer control plane a very-light rollup
before it drills into per-target outputs.

## Federated learning across many gewyvern runtimes

When one `etragon` should learn from multiple nearby `gewyvern` runtimes, give
it a small federation manifest:

```json
{
  "runtimes": [
    {
      "id": "gw-a",
      "targets_url": "http://127.0.0.1:9910/v1/latest/targets"
    },
    {
      "id": "gw-b",
      "targets_url": "http://127.0.0.1:9920/v1/latest/targets"
    }
  ]
}
```

Analyze the whole runtime set with one resident Python worker:

```bash
cargo run -p etragon -- analyze-python-federation-json /tmp/etragon-federation.json --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

Train across the same runtime set:

```bash
cargo run -p etragon -- train-python-federation-json /tmp/etragon-federation.json --label network_observe_longer --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

The output is a federated batch with:

- `runtime_count`
- `target_count`
- `failed_runtime_count`
- per-runtime status
- per-target output keyed by `runtime_id/path_segment`
- a merged `recommendation_summary`

This is intentionally learning aggregation, not fleet orchestration.
`etragon` may learn from many runtimes; `leserpent` still coordinates them.

## Python worker and resident mode

`etragon` now also ships with a small Python baseline worker at
`apps/etragon/scripts/python_baseline_worker.py`.

That worker is intentionally lightweight today: it uses the Python standard library,
emits machine-friendly augmentations, and keeps the Rust side free to swap in a
future `torch` worker later without changing the CLI contract.

You can run a one-shot Python-backed analysis like this:

```bash
cargo run -p etragon -- analyze-python-url http://127.0.0.1:9910/v1/latest/analysis.json --python-worker ./apps/etragon/scripts/python_baseline_worker.py

cargo run -p etragon -- analyze-python-targets-url http://127.0.0.1:9910/v1/latest/targets --filter scan: --python-worker ./apps/etragon/scripts/python_baseline_worker.py
```

If you want the baseline worker to keep a small online-learning memory across runs,
pass a shared state file:

```bash
cargo run -p etragon -- train-python-json /tmp/analysis.json --label http_request_followup --weight 2.5 --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- analyze-python-json /tmp/analysis.json --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- train-python-targets-url http://127.0.0.1:9910/v1/latest/targets --label http_request_followup --filter scan: --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

The first command teaches the worker a label for the current snapshot pattern. The
second command reuses that memory and can emit a learned-route candidate such as
`py_ml_candidate_learned_route`. That learned candidate now carries very-light
online-learning metadata such as `support_score`, `support_count`, `train_count`,
`score_margin`, and `last_trained_unix_ms`. The third command does the same thing
for every matching live target snapshot behind a `gewyvern /v1/latest/targets`
index.

Current canonical training labels are:

- `network_observe_longer`
- `targeted_escalation`
- `http_request_followup`

Very-light aliases are also accepted at the Rust CLI and resident training
boundary. For example:

- `observe-longer` -> `network_observe_longer`
- `escalate` -> `targeted_escalation`
- `request_followup` -> `http_request_followup`

You can also query that label dictionary directly:

```bash
cargo run -p etragon -- training-labels
```

If you want to inspect or reset the local online-memory state itself, you can now use:

```bash
cargo run -p etragon -- python-memory-info --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- python-memory-model-info --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- protocol-capabilities --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- clear-python-memory --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

`python-memory-info` returns a very-light summary including the current
`schema_version`, `model_version`, `pattern_count`, `label_count`, and the latest
`last_trained_unix_ms`. `python-memory-model-info` returns the worker protocol
version, supported commands, and supported training labels for compatibility
checks. `protocol-capabilities` returns the fuller sidecar-facing capability
document, including input/output contracts, daemon route families, resident
feature flags, IR surfaces, merge hints, handoff readiness levels, and the
worker declaration. `clear-python-memory` clears the learned pattern memory
without changing the Rust CLI contract around `analyze-*`, `train-*`, `watch-*`,
or `serve-*`.

## Memory transfer and rollback

The online learner supports portable, auditable memory transfer. This is
experience transfer for pattern memory, not a neural checkpoint transfer.

Export the current memory:

```bash
cargo run -p etragon -- python-memory-snapshot --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json > /tmp/etragon-memory-export.json
```

Plan a transfer before importing it:

```bash
cargo run -p etragon -- python-memory-transfer-plan /tmp/etragon-memory-export.json --merge --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

The transfer plan is a dry run. It checks schema/model compatibility, the
selected `replace` or `merge` strategy, current and incoming pattern counts,
overlapping patterns, new patterns, and conflicting labels. It never changes
the destination state.

When the plan looks safe, import the snapshot:

```bash
cargo run -p etragon -- import-python-memory /tmp/etragon-memory-export.json --merge --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

Use slots for rollback checkpoints:

```bash
cargo run -p etragon -- save-python-memory-slot baseline --label baseline-v1 --note "known-good lab state" --source operator --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- load-python-memory-slot baseline --merge --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json

cargo run -p etragon -- delete-python-memory-slot baseline --python-worker ./apps/etragon/scripts/python_baseline_worker.py --python-state /tmp/etragon-online-state.json
```

That dictionary now also exposes a very-light transition policy:

- `compatible_with`
- `competes_with`

The online learner uses those relationships during weighted training. Compatible
labels decay more gently, while competing labels decay more aggressively when a
new training signal arrives. That keeps the resident learner from treating every
label transition as equally destructive.

If you just want streaming polling output in stdout, keep `etragon` resident like this:

```bash
cargo run -p etragon -- watch-python-url http://127.0.0.1:9910/v1/latest/analysis.json --interval-ms 1000 --python-worker ./apps/etragon/scripts/python_baseline_worker.py

cargo run -p etragon -- watch-python-targets-url http://127.0.0.1:9910/v1/latest/targets --filter scan: --interval-ms 1000 --python-worker ./apps/etragon/scripts/python_baseline_worker.py
```

Use `--cycles <n>` during tests or local dry runs when you want the watch command to stop after a fixed number of polling rounds.

`--python-state <path>` also works with `watch-*` and `serve-*`, so a resident
daemon can keep learning-specific memory across restarts without changing the Rust
side interface.

`--daemon-state <path>` is separate: it persists the resident daemon's own
latest snapshot, queue hints, learning summaries, and target-level resident
state so a restarted sidecar can recover its most recent diagnosis-assist view
instead of starting from an empty in-memory summary. This file is written with a
very-light retention policy: large input/output bodies are compacted before
persist, and only a bounded set of the most recent or most relevant target
states are retained, so the resident state stays restart-friendly instead of
growing without bound.

If you want `etragon` itself to behave like a tiny resident external-analysis service,
you can also run a daemon that keeps polling `gewyvern`, reuses one Python worker,
and exposes the latest output over HTTP:

```bash
cargo run -p etragon -- serve-python-url http://127.0.0.1:9910/v1/latest/analysis.json --bind 127.0.0.1:4321 --interval-ms 1000 --python-worker ./apps/etragon/scripts/python_baseline_worker.py --daemon-state /tmp/etragon-daemon-state.json

cargo run -p etragon -- serve-python-targets-url http://127.0.0.1:9910/v1/latest/targets --bind 127.0.0.1:4321 --filter scan: --interval-ms 1000 --python-worker ./apps/etragon/scripts/python_baseline_worker.py --daemon-state /tmp/etragon-daemon-state.json
```

The resident daemon exposes:

- `/health`
- `/v1/training-labels.json`
- `/v1/memory-state.json`
- `/v1/memory-model.json`
- `/v1/protocol-capabilities.json`
- `POST /v1/memory-admin/clear`
- `/v1/latest/status`
- `/v1/latest/meta`
- `/v1/latest/recommendation-summary.json`
- `/v1/latest/federation-summary.json`
- `/v1/latest/learning-summary.json`
- `/v1/latest/evidence-chain-enrichment.json`
- `/v1/latest/diagnostic-opinion.json`
- `/v1/latest/handoff-summary.json`
- `/v1/latest/output.json`
- `POST /v1/train/latest`
- `/v1/latest/targets`
- `/v1/latest/targets/<path-segment>/meta.json`
- `/v1/latest/targets/<path-segment>/output.json`
- `/v1/latest/targets/<path-segment>/recommendation-summary.json`
- `/v1/latest/targets/<path-segment>/learning-summary.json`
- `/v1/latest/targets/<path-segment>/evidence-chain-enrichment.json`
- `/v1/latest/targets/<path-segment>/diagnostic-opinion.json`
- `/v1/latest/targets/<path-segment>/handoff-summary.json`
- `POST /v1/train/targets/<path-segment>`

The daemon also keeps a very-light input fingerprint cache, so if the upstream
`gewyvern` payload has not changed between polling cycles, `etragon` can reuse
the last output instead of re-running the Python worker.

If the daemon is running with `--python-state <path>`, those `POST /v1/train/*`
routes can accept a tiny JSON body such as `{"label":"http_request_followup","weight":2.5}`.
They train the resident online-memory worker against the latest cached snapshot,
invalidate the current cache entry, and let the next polling cycle republish a
fresh output that can include `py_ml_candidate_learned_route`.

For polling clients, `/v1/latest/meta` and `/v1/latest/output.json` also carry:

- `updated_unix_ms`
- `state_hash`
- `last_success_unix_ms`
- `last_error`
- `learning_active`
- `learned_routes`

That gives downstream systems a simple way to detect whether the resident state
has changed before they fetch or process the larger payload.

`/v1/latest/handoff-summary.json` is the lightest route for nearby control
planes that only need to know whether `etragon` currently has a mergeable or
automation-worthy signal for `gewyvern`. It summarizes:

- whether `evidence_chain_enrichment` is present
- whether `diagnostic_opinion` is present
- the current `handoff_readiness`
- the current `gewyvern_merge_hint`
- the strongest currently visible enrichment/opinion band

That same `handoff_summary` object is also present in `/v1/latest/meta`, the
target index, and each target meta route so pollers can decide whether to drill
deeper without re-reading the heavier learning payloads.

`/v1/memory-state.json` is the lightest resident management route for the online
Python memory itself. It returns the worker memory summary together with a very-light
count of resident training events currently cached by the daemon.

`/v1/memory-model.json` is the companion capability route for that worker. It
returns the worker protocol version, supported commands, and supported training
labels so nearby services can sanity-check compatibility before relying on the
resident learner.

`/v1/protocol-capabilities.json` is the structured sidecar declaration. It
bundles the daemon route surface, resident feature flags, supported
input/output contracts, the worker declaration, and the currently supported
minor release snapshot line so nearby control planes can decide how deeply to
integrate without scraping multiple routes first.

That capability document now also breaks out three higher-level protocol areas:

- `ir_capabilities`: which structured-text IR surfaces exist for latest and target scopes, and which resident memory annotations can appear in learning views
- `merge_capabilities`: which `gewyvern_merge_hint` values `etragon` can currently emit, and whether summary/batch merging is supported
- `handoff_capabilities`: which readiness levels exist, which summary fields are stable enough to depend on, and which routes fan that handoff state back out

It now also carries evolution guidance for consumers:

- `ir_capabilities.stability`: which latest/target IR fields are intentionally stable for downstream consumers, and which latest fields should still be treated as experimental
- `merge_capabilities.safe_automation_hints`: the subset of merge hints that can be consumed conservatively without forcing operator review
- `merge_capabilities.operator_review_hints`: the hints that should keep a human in the loop
- `compatibility.forward_compatibility_rules`: the downgrade rules consumers should follow when new fields or new hints appear

`POST /v1/memory-admin/clear` clears the Python worker's persisted pattern memory,
clears the daemon's resident training-event history, and invalidates the current
cache so the next polling cycle can republish a fresh non-learned view.

If polling fails, the daemon now keeps running and marks the resident state as
`degraded` instead of exiting immediately. That makes it much easier to treat
`etragon` as a long-lived sidecar rather than a fragile one-shot wrapper.

`/v1/latest/status` is the lightest route for probes and upper-layer control
planes. It keeps the operational state separate from the heavier analysis
payloads.

`/v1/latest/evidence-chain-enrichment.json` and
`/v1/latest/diagnostic-opinion.json` are the lightest routes for nearby
consumers that want the sidecar's high-level diagnosis-assist layer without
re-reading the full learning summary.

Standalone evidence-chain enrichments now also carry:

- `enrichment_strength_band`
- `handoff_readiness`
- `gewyvern_merge_hint`

That gives nearby consumers a very-light way to tell whether the current
enrichment is still early, moderately reinforcing, or strongly reinforcing the
evidence chain above `gewyvern`'s base facts.

When a standalone diagnostic opinion is available, it now also carries:

- `source_scope`
- `opinion_confidence_band`
- `handoff_readiness`
- `gewyvern_merge_hint`

That gives nearby consumers a very-light way to tell whether they are looking at
a latest-snapshot or target-specific opinion, and whether the sidecar currently
considers that opinion `high` or `medium` confidence.

`/v1/latest/learning-summary.json` is the lightest route for online-learning
consumers. It answers:

- whether learned-route memory is currently active
- how many learned routes are visible in the latest summary
- which learned route is currently on top
- which canonical learned label is currently on top
- whether that learned label is compatible with or competes with the other built-in training labels
- what the current top learned label state looks like (`support_score`, `train_count`, `score_margin`, and route count)
- what the current runner-up learned label state looks like when a second label is active
- what the current confidence and stability hints look like for the learned route lead
- whether recent training feedback is actively pulling two competing labels at once
- what the current per-pattern label memory snapshot looks like for the latest analyzed shape
- what a very-light sorted summary of that per-pattern label memory looks like
- whether that learned label currently looks like it is `emerging`, `converging`, `stable`, `switching`, `volatile`, or `conflicted`
- what a one-line `learning_judgement` thinks the upper-layer consumer should do next (`ready`, `observe`, `leaning`, `watch_transition`, or `manual_review`)
- what a concrete `action_queue_hint` thinks the sidecar should queue next (`keep_observing`, `queue_transition_check`, `promote_learned_route`, or `manual_review`)
- what a daemon-level `queue_summary` says about how those learned targets currently distribute across observation, transition-watch, human-review, or automation-ready queues
- what a `queue_pressure_hint` says about whether the current learned-target pressure looks more like monitoring bias, transition pressure, promotion readiness, or human-review backlog
- what a `feedback_policy_hint` thinks the operator should do with new training feedback next (`continue_observation`, `collect_disambiguating_feedback`, `reinforce_current_label`, `promote_and_monitor`, or `pause_and_review`)
- what an `evidence_chain_enrichment` says about how the current learned route is reinforcing, contesting, or maturing the evidence chain above `gewyvern`'s base facts
- whether a more direct `diagnostic_opinion` is now justified, and when it is still intentionally withheld because the learned state is too early or too conflicted
- whether that learned label currently looks more `compatible`, `competing`, `balanced`, or `neutral`
- which labels have seen the most recent training activity, with simple event-count and total-weight rollups
- which recent training feedback events shaped the current learned route view

When the daemon is watching a target index, those target-specific routes let an
upper-layer control plane pull the latest external augmentation for just one
path segment instead of re-reading the whole batch snapshot every time.

Each target now also carries its own:

- `updated_unix_ms`
- `state_hash`
- `last_success_unix_ms`
- `last_error`
- `learning_active`
- `learned_routes`

both in the target index and in `/v1/latest/targets/<path-segment>/meta.json`.

The daemon-level recommendation summary routes also expose a richer overview:

- `recommendations`
- `top_recommendation`
- `top_candidates`

When the online learner contributes `py_ml_candidate_learned_route`, those
recommendation-summary objects can also carry very-light learning hints such as:

- `support_score`
- `train_count`
- `last_trained_unix_ms`
- `score_margin`

That gives downstream automation a fast “what should I care about first?” view
without forcing it to rank the full augmentation list itself.

That gives us a clean next step toward a real long-lived ML sidecar, without
forcing `torch` or a heavier serving stack into the first version.

## Pass model

`etragon` now treats external analysis as a small pipeline instead of a single
monolithic engine. The intended layering is:

1. `CandidateAugmenter`
2. `RecommendationAugmenter`
3. `RerankPass`

`PassPipeline` lets you stack those phases without locking the project to any
particular backend. That means future Burn, ONNX, service-backed, or hybrid
engines can reuse the same contract shape instead of rewriting the orchestration
layer.

## TDD layers

`etragon` now keeps its tests split across a few lightweight layers:

- `src/*` unit tests for contract parsing, pass behavior, and small CLI helpers
- `tests/fixtures/*.json` for stable `gewyvern analysis.json` examples
- `tests/contract_fixtures.rs` for snapshot parsing against saved fixtures
- `tests/pipeline_integration.rs` for candidate/recommendation/rerank composition
- `tests/cli_end_to_end.rs` for real binary-level `analyze-json`, `analyze-url`, and `analyze-targets-url`

For a tighter red-green loop, use:

```bash
bash apps/etragon/scripts/test_tdd.sh unit
bash apps/etragon/scripts/test_tdd.sh cli
bash apps/etragon/scripts/test_tdd.sh integration
bash apps/etragon/scripts/test_tdd.sh all
```

That keeps day-to-day pass development TDD-friendly without adding heavyweight test dependencies too early.

Every augmentation now also carries `producer_stage` and `producer_pass`, so batch outputs stay traceable as we add more candidate, recommendation, and rerank passes.

## Intended integration with gewyvern

`gewyvern` already exposes:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

Those snapshots now carry built-in `augmentations` as well. `etragon` is meant to
sit above that layer and append its own external augmentations rather than
replace `gewyvern`'s core analysis chain.
