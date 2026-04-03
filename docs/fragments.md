# Fragment Guide

This document describes how fragments are modeled in the current runtime and how
new fragments should be introduced.

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
- mutate template semantics

## Current Descriptor Shape

The current Rust-side descriptor is `FragmentDescriptor` in `src/fragment.rs`.

Fields:

- `id`
- `version`
- `hookpoints`
- `emits`
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
- `tcp_packet_meta_fragment`
- `route_meta_fragment`

### `version`

Monotonic fragment descriptor version.

Even while the project is early, version should change when descriptor semantics
or exported behavior changes in a meaningful way.

### `hookpoints`

Declares where the fragment attaches.

Current runtime enum:

- `TracePoint`
- `KProbe`
- `TCIngress`
- `TCEgress`

Two fragments may not claim the same hookpoint inside one attach plan.

### `emits`

Declares which fact kinds the fragment produces.

This is used by the registry to:

- build fact graph
- assign fact ownership
- validate requirement coverage

The current planner treats a fact kind as having a single producer inside a
session plan.

### `requires`

Declares prerequisite fact kinds.

This does not mean runtime dataflow execution yet. In v0.04 it primarily means:

- the registry can verify coverage
- dependency edges can be built into the attach plan

### `maps`

Declares BPF map requirements.

Current map kinds:

- `RingBuf`
- `Hash`
- `LruHash`

The current runtime only summarizes ringbuf usage into `AttachReport`.

### `capabilities`

Declares coarse capability labels for the fragment.

Current built-ins:

- `TcpState`
- `PacketMeta`
- `RouteMeta`

These are descriptive tags, not permission boundaries.

## Built-In Fragments

The current built-in registry provides:

### `tcp_state_fragment`

- hookpoint: `tracepoint:sock/inet_sock_set_state`
- emits: `tcp_state`
- requires: none

### `tcp_packet_meta_fragment`

- hookpoint: `tc:ingress`
- emits: `packet_meta`
- requires: `tcp_state`

### `route_meta_fragment`

- hookpoint: `kprobe:ip_route_output_flow`
- emits: `route_decision`
- requires: `tcp_state`

## Registry Rules

The registry currently enforces:

- unique fragment ids
- no hookpoint conflicts inside one plan
- no fact ownership conflicts inside one plan
- all required fact kinds must be covered

These rules are tested in `src/fragment.rs`.

## How To Add A New Fragment

Use this sequence:

1. write a failing registry or scenario test
2. define the new `FragmentDescriptor`
3. register it in `builtin_registry()`
4. update templates if the fragment should be selectable
5. update runtime behavior only if new facts need new handling
6. ensure replay and export still make sense

## Design Rule

If you feel tempted to put windowing, interpretation, or policy into a
fragment, that behavior probably belongs somewhere else:

- window semantics belong to template/session runtime
- interpretation belongs to reason engine
- policy belongs to gate or future intervention logic
