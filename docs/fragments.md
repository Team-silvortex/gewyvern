# Fragment Guide

Use this page when you need the current fragment model and attach-capability
surface.

This page is intentionally a durable runtime note. It focuses on:

- what a fragment is
- what a fragment is not allowed to do
- how descriptor fields should be interpreted
- how new fragments should be introduced

This page is not the best first stop for:

- the top-level system map
- first-run operator behavior
- exact diagnosis field lookup

For those, use:

- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)

## What A Fragment Is

A fragment is a capability unit, not a reason unit.

It is allowed to:

- attach to one or more hookpoints
- emit structured facts
- declare fact dependencies
- declare map requirements
- declare capability flags

It is not allowed to:

- interpret facts
- know the active session window
- know the reason profile
- decide program-flow semantics
- mutate template semantics

Fragments are the embryo of runtime IR. They describe what evidence can exist,
not what that evidence means at the final debugger layer.

## Current Descriptor Shape

The current Rust-side descriptor is `FragmentDescriptor` in
[src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs).

Fields:

- `id`
- `version`
- `hookpoints`
- `emits`
- `evidence_classes`
- `requires`
- `maps`
- `capabilities`

## Field Semantics

### `id`

Must be globally unique in the registry.

Recommended naming style:

- `<domain>_<role>_fragment`

Examples:

- `tcp_state_fragment`
- `udp_packet_meta_fragment`
- `route_meta_fragment`
- `sock_lineage_fragment`

### `version`

Monotonic fragment descriptor version.

Even while the project is early, version should change when descriptor
semantics or exported behavior changes in a meaningful way.

### `hookpoints`

Declares where the fragment attaches.

Current runtime enum:

- `TracePoint`
- `KProbe`
- `TCIngress`
- `TCEgress`

Current label encoding:

- `tracepoint:<name>`
- `kprobe:<name>`
- `tc:ingress`
- `tc:egress`

Two fragments may not claim the same hookpoint inside one attach plan.

### `emits`

Declares which fact kinds the fragment produces.

This is used by the registry to:

- build the fact graph
- assign fact ownership
- validate requirement coverage

The current planner treats a fact kind as having a single producer inside a
session plan.

### `requires`

Declares prerequisite fact kinds.

This is registry/runtime composition metadata, not a full dataflow engine.
Today it primarily means:

- the registry can verify coverage
- dependency edges can be built into the attach plan
- templates with invalid fragment sets can be rejected early

### `evidence_classes`

Declares how emitted fact kinds should be treated by planner diagnostics.

Current tiers:

- `CoreRequirement`
- `OptionalEnhancement`

This is what lets binding diagnostics distinguish between rules that depend on
core transport/path evidence and rules that depend on optional enrichment such
as socket-to-process lineage.

### `maps`

Declares BPF map requirements.

Current map kinds:

- `RingBuf`
- `Hash`
- `LruHash`

Current runtime usage:

- ringbuf maps are summarized into `AttachReport.ringbuf_stats`

### `capabilities`

Declares coarse capability labels for the fragment.

Current built-ins:

- `TcpState`
- `PacketMeta`
- `RouteMeta`
- `SockLineage`

These are descriptive tags, not permission boundaries.

## Built-In Fragments

The current built-in registry provides:

### `tcp_state_fragment`

- hookpoint: `tracepoint:sock/inet_sock_set_state`
- emits: `tcp_state`
- requires: none
- capability: `TcpState`

This is the primary TCP lifecycle evidence source.

### `tcp_packet_meta_fragment`

- hookpoint: `tc:ingress`
- emits: `packet_meta`
- requires: `tcp_state`
- capability: `PacketMeta`

This models TCP packet metadata as transport evidence.

### `udp_packet_meta_fragment`

- hookpoint: `tc:ingress`
- emits: `packet_meta`
- requires: none
- capability: `PacketMeta`

This is the UDP packet-evidence counterpart to `tcp_packet_meta_fragment`.

### `route_meta_fragment`

- hookpoint: `kprobe:ip_route_output_flow`
- emits: `route_decision`
- requires: none
- capability: `RouteMeta`

This contributes route/path evidence and can participate in both TCP and UDP
templates.

### `sock_lineage_fragment`

- hookpoint: `tracepoint:syscalls/sys_enter_connect`
- emits: `sock_lineage`
- requires: none
- capability: `SockLineage`

This fragment is what makes process-aware transport/program-flow reconstruction
possible. It provides the bridge from socket evidence to `pid` / `tid` /
`cgroup_id` / `comm`.

## Registry Rules

The registry currently enforces:

- unique fragment ids
- no hookpoint conflicts inside one plan
- no fact ownership conflicts inside one plan
- all required fact kinds must be covered

These invariants are specified in:

- [tests/fragment_rules_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/fragment_rules_tdd.rs)

## Attach Reports And Failures

Fragments participate in two related runtime structures:

- `AttachPlan`
- `AttachReport`

`AttachPlan` is the static composition result.

`AttachReport` is the operational outcome and currently records:

- `fragments_loaded`
- `hookpoints_attached`
- `hookpoints_failed`
- `required_fact_kinds_coverage`
- `ringbuf_stats`

The loader/runtime path can also produce structured `AttachFailure` records.
Those are converted into `hookpoints_failed` labels and then influence:

- exported debug summaries
- attach failure summaries
- fact-ingest gating

If a fragment fails to attach, facts from that fragment are rejected by the
runtime and appear in `rejected_facts`.

## How To Add A New Fragment

Use this sequence:

1. write a failing registry or scenario test
2. define the new `FragmentDescriptor`
3. register it in `builtin_registry()`
4. update templates if the fragment should be selectable
5. update runtime/export handling only if the new emitted fact kind needs it
6. verify replay and summaries still make sense

If the new fragment needs real Linux probe coverage, also extend:

- [tests/linux_smoke_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/linux_smoke_tdd.rs)
- [src/loader.rs](/Users/Shared/chroot/dev/gewyvern/src/loader.rs)

## Design Rule

If you feel tempted to put windowing, interpretation, or program policy into a
fragment, that behavior probably belongs somewhere else:

- window semantics belong to template/session runtime
- interpretation belongs to reason profiles or program models
- attach outcome handling belongs to loader/runtime
- policy belongs to gate or future intervention logic
