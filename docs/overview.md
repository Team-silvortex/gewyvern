# gewyvern Overview

`gewyvern` is a single-host, window-bounded TCP network debugger runtime.

Its job is not to explain the whole network. Its job is to compress one
network abnormality into a verifiable, reviewable, replayable chain of facts.

## What It Is

- A CLI-first debugger runtime
- A fragment-oriented eBPF session model
- A fact-to-flow-to-reason pipeline
- An exportable and replayable evidence chain

## What It Is Not

- A long-running observability platform
- A persistent monitoring system
- A distributed agent mesh
- A DSL compiler

## Current Runtime Shape

The current codebase implements the userspace runtime skeleton for v0.04:

1. `Template` selects a fragment set, window profile, and reason profile
2. `FragmentRegistry` validates fragment metadata and builds an `AttachPlan`
3. `RuntimeSession` ingests structured facts
4. Facts are materialized into `FlowSnapshot`
5. Flow evidence is reduced into `ReasonChain`
6. Session state is exported as replayable JSON

The kernel plane is still a placeholder. eBPF programs are not yet loaded by the
runtime, but the runtime structure is already organized around fragment-oriented
attach planning.

## Code Map

- `src/template.rs`: template definitions and validation
- `src/fragment.rs`: fragment descriptors, registry, attach planning
- `src/ledger.rs`: physical fact envelope and fact kinds
- `src/runtime.rs`: session orchestration and flow construction
- `src/reason.rs`: L1 reason-chain generation
- `src/export.rs`: JSON export and deterministic replay
- `tests/runtime_tdd.rs`: end-to-end T1 behavior tests

## Recommended Reading Order

1. `README.md`
2. `docs/overview.md`
3. `docs/architecture.md`
4. `docs/fragments.md`
5. `docs/export-format.md`
6. `docs/development.md`

## Primary Scenario

The built-in template is `handshake_debug`, which currently models T1:

- normal handshake
- SYN-ACK missing
- route fingerprint change

Each scenario is validated through tests and is expected to remain replay-stable.
