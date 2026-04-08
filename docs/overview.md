# gewyvern Overview

`gewyvern` is a single-host, window-bounded network debugging runtime.

Its job is not to explain the whole network. Its job is to compress one bounded
network behavior into a verifiable, reviewable, replayable chain of facts,
flows, and reasons.

## What It Is

- A CLI-first debugger/runtime
- A fragment-oriented eBPF attach and fact model
- A fact-to-transport-flow-to-program-flow-to-reason pipeline
- A replayable export format for offline inspection
- A project moving toward protocol-agnostic network-function reconstruction

## What It Is Not

- A long-running observability platform
- A persistent monitoring backend
- A distributed agent mesh
- A finished DSL compiler

## Current Runtime Shape

The current `v0.1` codebase already implements the core runtime loop:

1. `Template` selects a fragment set, window profile, reason profile, and program model
2. `FragmentRegistry` validates fragment metadata and builds an `AttachPlan`
3. A loader path produces attach outcomes
4. `RuntimeSession` ingests structured facts and gates them against attach results
5. Facts are materialized into transport flows
6. Program models lift transport evidence into program flows
7. Reason profiles reduce evidence into deterministic reason chains
8. Session state is exported as replayable JSON

This is no longer only a userspace skeleton. The repository now also contains:

- real Linux probe smoke/probe paths for built-in fragment hookpoints
- socket-based fact ingestion over Unix and TCP
- process-aware flow reconstruction through `sock_lineage`

## Protocol-Agnostic Direction

`gewyvern` is not intended to become “one more TCP debugger with add-on UDP
support”. The long-term direction is:

- fragments behave like the embryo of IR
- templates compose fragments plus runtime policies
- program/network-function modeling should be driven by IR/DSL rules rather than
  protocol-specific Rust branches

The current `ProgramModel` layer is the first concrete step in that direction.
It is embedded in Rust today, but it already moves `program_flows` away from
runtime hardcoded protocol inference and toward declarative rule-driven
materialization.

The repo now also contains DSL files under
[dsl/](/Users/Shared/chroot/dev/gewyvern/dsl) that compile into
`TemplateBinding`:

- fragment selection
- fragment parameter bindings
- runtime policy selection
- program-model rules

## Code Map

- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs): templates, window profiles, reason profiles, program models
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs): current embedded rule engine for `program_flows`
- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs): fragment descriptors, registry, attach planning/reporting
- [src/loader.rs](/Users/Shared/chroot/dev/gewyvern/src/loader.rs): loader abstraction and Linux probe paths
- [src/ledger.rs](/Users/Shared/chroot/dev/gewyvern/src/ledger.rs): fact envelopes and physical fact kinds
- [src/runtime.rs](/Users/Shared/chroot/dev/gewyvern/src/runtime.rs): session orchestration, gating, flow reconstruction
- [src/reason.rs](/Users/Shared/chroot/dev/gewyvern/src/reason.rs): deterministic reason-chain generation
- [src/export.rs](/Users/Shared/chroot/dev/gewyvern/src/export.rs): export format and deterministic replay
- [src/socket_input.rs](/Users/Shared/chroot/dev/gewyvern/src/socket_input.rs): Unix/TCP socket fact ingestion

## Built-In Scenarios

The repository currently ships built-in paths for:

- `handshake_debug`
  - TCP state + packet + route reconstruction
- `udp_debug`
  - UDP datagram + route reconstruction
- `udp_process_debug`
  - UDP datagram + route + process lineage reconstruction

These scenarios are validated through TDD specs and expected to remain replay-stable.

## Recommended Reading Order

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
2. [docs/overview.md](/Users/Shared/chroot/dev/gewyvern/docs/overview.md)
3. [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
4. [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
5. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
