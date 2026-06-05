# Runtime Architecture

Use this page when you need the runtime-pipeline deep dive.

This page is intentionally a durable internals note. It focuses on:

- the runtime pipeline
- the core IR-bearing concepts
- how plans, sessions, flows, and replay fit together

This page is not the best first stop for:

- the top-level system map
- first-run operator validation
- exact CLI/API contract lookup

For those, start with:

- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- [docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md)

## Pipeline

The current pipeline in code is:

```text
Template
  -> Fragment Registry
  -> Attach Planner
  -> Loader / Probe Path
  -> Fact Stream
  -> Transport Flows
  -> Program Flows
  -> Reason Chains
  -> Export JSON
  -> Deterministic Replay
```

## Core Concepts

### Fragment

A fragment is the smallest attachable capability unit. It includes:

- a unique fragment id
- a version
- hookpoint declarations
- emitted fact kinds
- required fact kinds
- map specifications
- capability flags

The runtime treats fragment metadata as the embryo of IR. A fragment does not
know about windowing, program reasoning, or final protocol interpretation.

### Fragment Registry

The registry owns all available fragment descriptors. It is responsible for:

- ensuring fragment ids are unique
- resolving descriptors by id
- building `AttachPlan`
- rejecting hookpoint conflicts
- rejecting fact ownership conflicts
- verifying required fact coverage

### Attach Plan

An attach plan is the read-only runtime IR for a session. It contains:

- fragment inventory
- hook graph
- fact graph
- dependency graph
- coverage report

The plan does not compile eBPF. It manages composition order, ownership, and
runtime consistency.

### Loader

The loader layer turns an attach plan into real or synthetic attach outcomes.

Current implementations include:

- `NoopLoader`
- `StaticFailureLoader`
- `LinuxProbeLoader`

The Linux probe path already supports real smoke/probe execution for:

- `tracepoint`
- `kprobe`
- `tc ingress`

Attach results are materialized into `AttachReport` and then influence runtime
behavior directly, including fact-ingest gating.

### Template

A template is the session recipe:

```text
Template = Fragment Set + Window Profile + Reason Profile + Program Model
```

Each part has a separate job:

- fragment set controls available evidence
- window profile controls session materialization bounds
- reason profile controls deterministic reduction into reason chains
- program model controls how transport evidence becomes program-flow structure

### Template Binding

`TemplateBinding` is the current compile-target skeleton for a future DSL layer.

Its role is to carry:

- a validated template
- fragment-level parameter bindings
- template-local evidence tier overrides

This boundary is intentional: the future DSL should compile into fragment
selection plus parameterization, not into generated eBPF bytecode.

### Runtime Session

`RuntimeSession` is the session-level orchestrator. It owns:

- selected template
- validated window profile
- selected reason profile
- attach plan and attach report
- ingested facts
- rejected facts
- freeze timestamp

The session is window-bounded and can be exported after freeze.

After `freeze(end)`, the materialized session is bounded to the active window
`[end - duration_ms, end]` plus the allowed late-arrival tail `lateness_ms`.
Facts outside that range are excluded from export, transport flows, program
flows, and reasons.

Facts emitted by fragments that failed to attach are rejected at ingest time and
tracked as audit records.

### Transport Flows

Transport flows are reconstructed from fact streams. They are the evidence
layer, not the final semantic aggregation. Right now they track:

- lifecycle boundaries
- route/path segments
- process identity when available
- evidence indexes
- confidence score
- `fragment_sources`

When route fingerprint changes, the current implementation rotates into a new
transport flow snapshot for that cookie.

### Program Flows

Program flows sit above transport flows. They are the beginning of the
"network-module decompilation" layer: instead of only saying that packets or
state transitions happened, they try to describe what network function a
program implementation was performing.

Declarative program rules and declarative reason rules now compile into the
same shared IR skeleton:

- flow predicate
- optional signal
- narrative template
- dedupe flag

That lets the DSL stay protocol-agnostic while keeping actual runtime evidence
strictly grounded in the selected fragment templates and their parameters.

The registry now statically validates that a binding's rule skeleton is
supported by the chosen fragment set. In other words, IR declarations are only
accepted when the current fragment inventory can actually emit the evidence
those predicates, signals, and narratives depend on.

Current built-in program-flow operations are intentionally generic:

- `connect_flow`
- `datagram_exchange`
- `unknown`

Templates may also emit custom operation ids through `ProgramModel`, which lets
the runtime start describing network-function intent without waiting for a full
external DSL.

Program flows currently include:

- bound process identity
- operation kind
- referenced transport flows
- ordered stages
- module-level narrative

### Program Model

`ProgramModel` is the current embedded rule layer that materializes
`program_flows`.

Today it is implemented as Rust data attached to templates:

- operation id
- ordered rules
- rule predicates
- optional emitted stage kinds
- optional narrative rendering

This is not the final DSL. It is the first explicit bridge from fragment/fact
evidence toward a protocol-agnostic rule-driven engine.

### Reason Chains

Reason chains are built from physical facts plus runtime structure. The current
implementation provides two built-in L1 views:

- `handshake_l1` for TCP handshake-oriented reasoning
- `udp_datagram_l1` for UDP packet and route reasoning

Both views export:

- TCP state timeline when applicable
- path segment events
- key events
- narrative lines

### Export and Replay

The export bundle contains enough state to recompute L1 offline:

- all facts
- fragment inventory
- attach plan
- attach report
- window profile
- reason profile id
- materialized transport flows
- materialized program flows
- materialized reasons

Replay parses export JSON, rebuilds a runtime session, replays facts, and
recomputes transport flows, program flows, and reasons.

## Current Limits

- eBPF programs are still hand-written C, not generated from IR
- there is no external DSL yet; `ProgramModel` is still embedded Rust data
- reason profiles are still intentionally small and conservative
- `tc egress` is not implemented in the real Linux probe path yet
- export JSON is still an internal project format, not a public stable schema

These limits are intentional. The current line still prioritizes debugger structure,
determinism, and a clean path toward protocol-agnostic modeling over breadth.
