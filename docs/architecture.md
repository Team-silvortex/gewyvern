# Runtime Architecture

## Pipeline

The current v0.04 pipeline in code is:

```text
Template
  -> Fragment Registry
  -> Attach Planner
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

The runtime treats fragment metadata as the embryo of IR. The fragment does not
know about windowing or reasoning.

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

The plan does not compile eBPF. It only manages composition order and runtime
consistency.

### Runtime Session

`RuntimeSession` is the session-level orchestrator. It owns:

- selected template
- validated window profile
- reason profile
- attach plan and attach report
- ingested facts
- freeze timestamp

The session is window-bounded and can be exported after freeze.
After `freeze(end)`, the materialized session is bounded to the active window
`[end - duration_ms, end]` plus the allowed late-arrival tail `lateness_ms`.
Facts outside that range are excluded from export, flow snapshots, and reason
chains.

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
flow snapshot for that cookie.

### Program Flows

Program flows sit above transport flows. They are the beginning of the
"network-module decompilation" layer: instead of only saying that packets or
state transitions happened, they try to describe what network function a
program implementation was performing.

The current minimal model tracks:

- bound process identity
- inferred operation kind
- referenced transport flows
- ordered stages
- module-level narrative

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
- materialized flows
- materialized reasons

Replay parses export JSON, rebuilds a runtime session, replays facts, and
recomputes transport flows, program flows, and reasons.

## Current Limits

- no real eBPF loader yet
- no ringbuf consumer yet
- no external DSL yet
- reason engine is still intentionally small and only ships TCP handshake plus
  UDP datagram views
- export JSON is implemented with a focused internal serializer/parser

These limits are intentional. The runtime currently prioritizes debugger
structure and determinism over breadth.
