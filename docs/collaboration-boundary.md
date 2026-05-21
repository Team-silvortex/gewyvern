# Collaboration Boundary

This note keeps the roles of `gewyvern`, `etragon`, and `leserpent` distinct.

## Runtime Truth

`gewyvern` is the single-runtime authority.

It owns:

- fact ingestion
- protocol and runtime analysis
- conservative conclusions
- report and API surfaces

It should remain the source of truth for:

- what evidence exists
- which protocol/module/stage is implicated
- whether ambiguity remains

## Diagnosis Partner

`etragon` is the near-runtime diagnosis partner.

Its intended shape is:

- one `etragon` works with one nearby `gewyvern`
- it consumes `gewyvern` analysis snapshots
- it adds higher-level evidence-chain enrichment
- it may emit a more direct `diagnostic_opinion` only when the learned state is stable enough

It should not become the fleet orchestrator.

It should stay focused on:

- evidence-chain enrichment
- learned memory
- rerank and recommendation hints
- higher-level but still additive diagnostic opinions

## Fleet Orchestrator

`leserpent` is the multi-instance orchestrator / control plane.

It is the layer that should manage:

- many `gewyvern` instances
- policy
- RBAC
- session and pipeline distribution
- audit and UI

It may consume outputs from both `gewyvern` and `etragon`, but it should not
collapse their roles together.

## Recommended Direction

The intended relationship is:

```text
etragon <-> gewyvern
leserpent -> many gewyvern instances
leserpent -> optional etragon services
```

That means:

- `gewyvern` provides runtime truth
- `etragon` provides diagnosis assistance close to that runtime
- `leserpent` performs orchestration above both

## Contract Implications

`etragon` outputs should be shaped for control-plane consumption, but they
should still remain additive to `gewyvern`.

The most important sidecar-facing layers are:

- `evidence_chain_enrichment`
- `diagnostic_opinion`
- `action_queue_hint`

These should help `leserpent` make better decisions without replacing
`gewyvern`'s base evidence model.
