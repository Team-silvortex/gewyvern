# System Guide

This document is the `v0.1` system-level map for `gewyvern`.

It explains how the current prototype is layered, what each layer is
responsible for, and where the main architectural boundaries are.

## One-Sentence Model

`gewyvern` is a protocol-agnostic, window-bounded network debugger where a DSL
compiles into fragment-template bindings, runtime planning/probing produces a
fact stream, and the engine reconstructs transport flows, program flows, and
deterministic reasons from that evidence.

## Layer Stack

The current system is easiest to understand as seven layers:

1. DSL and template binding
2. Fragment inventory
3. Planning and probing
4. Fact ingest and runtime gating
5. Materialized runtime IR
6. Export and replay
7. CLI and socket surfaces

## 1. DSL And Template Binding

Source files:

- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
- [src/gewyc.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc.rs)
- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)

`.gewy` files compile into `TemplateBinding`.

The compiler-facing surface now also materializes owned reports for frontends:

- `BindingReport`
- `DiagnosticsReport`
- `CompilerFindingsReport`
- `CompilerEnvelope`

The compiler pipeline is now exposed as distinct stages:

- parse into a merged pipeline/module front-end representation and then an unvalidated `TemplateBinding`
- validate the compiled binding against the fragment registry
- materialize owned frontend reports from the validated result
- surface parse/validation failures as structured compiler findings

`CompilerEnvelope` now acts as the shared aggregation surface for the current
front-end outputs:

- `binding`
- `diagnostics`
- `findings`
- `stages`

The `stages.parse` section now also carries a front-end module summary for
pipeline-based gewy packages, including DSL kind, merged step count, function
count, and resolved include sources.

The `gewyc` CLI now consumes this shared envelope and renders each subcommand as
one view over the same underlying compiler result.

It also now exposes `envelope` directly, so frontends can fetch all current
compiler-facing surfaces in a single invocation.

The staged frontend report now exposes explicit sections for:

- `parse`
- `validation`
- `diagnostics`

For parse/validation/diagnostics failures, the staged report now preserves
stage-local findings and any already-materialized stage output instead of
collapsing immediately into a single top-level error.

The `validation` section currently summarizes:

- registry source
- fragment count
- program/reason rule counts
- executed validation checks
- sampled payload offsets exposed by the current fragment set
- payload offsets required by the binding's offset-based predicates
- unsupported payload offsets that fall outside the current fragment sampling surface

That binding currently carries:

- template identity
- fragment set
- window profile
- reason profile
- program model
- fragment params
- evidence tier overrides

Important boundary:

- DSL does not compile into eBPF bytecode
- DSL compiles into fragment-template selection plus parameterization

That boundary keeps verifier pressure low and keeps kernel-facing behavior
grounded in prebuilt fragment templates.

## 2. Fragment Inventory

Source files:

- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)

Fragments are the smallest attachable capability units.

Each fragment descriptor declares:

- hookpoints
- emitted fact kinds
- required fact kinds
- maps
- capability flags
- parameter schema
- evidence classes

This is the embryo of the system IR. Fragments describe what evidence can
exist, but not the final semantic interpretation of that evidence.

## 3. Planning And Probing

Source files:

- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [src/loader.rs](/Users/Shared/chroot/dev/gewyvern/src/loader.rs)

The planner turns a fragment set into an `AttachPlan`.

That plan captures:

- fragment inventory
- hook graph
- fact graph
- dependency graph
- coverage

The loader layer then turns that plan into attach outcomes.

Current loader/probe modes:

- no-op / synthetic failure loaders
- Linux tracepoint probe
- Linux kprobe probe
- Linux tc ingress probe

Important boundary:

- planning/probing uses existing fragment templates
- it does not synthesize new kernel programs from DSL rules

## 4. Fact Ingest And Runtime Gating

Source file:

- [src/runtime.rs](/Users/Shared/chroot/dev/gewyvern/src/runtime.rs)

`RuntimeSession` is the session orchestrator.

It applies:

- template window policy
- attach-result gating
- fragment-param filtering
- rejected-fact auditing

This is where the system stops being only a planner and becomes an actual
debugger runtime.

Current gating behavior includes:

- facts from fragments that failed to attach are rejected
- small UDP packet facts can be filtered by fragment params
- process lineage can be redacted by fragment params

## 5. Materialized Runtime IR

Source files:

- [src/flow.rs](/Users/Shared/chroot/dev/gewyvern/src/flow.rs)
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs)
- [src/reason.rs](/Users/Shared/chroot/dev/gewyvern/src/reason.rs)
- [src/ir.rs](/Users/Shared/chroot/dev/gewyvern/src/ir.rs)

The current runtime materializes three main IR surfaces:

### Transport Flows

Evidence-layer flow reconstruction:

- packet/state/route lineage
- lifecycle
- path segments
- process binding when available

### Program Flows

Network-function reconstruction layer:

- operation
- stages
- narrative
- referenced transport flows
- process-aware behavior view

### Reason Chains

Deterministic explanatory layer:

- key events
- narrative lines
- L1/L3 structure

Shared IR direction:

- shared predicates
- shared signals
- shared narrative templates
- shared rule skeleton

That is the current path toward a protocol-agnostic engine.

## 6. Export And Replay

Source file:

- [src/export.rs](/Users/Shared/chroot/dev/gewyvern/src/export.rs)

`ExportBundle` is the replay boundary.

It preserves:

- attach plan
- attach report
- binding diagnostics
- fragment params
- evidence overrides
- facts
- rejected-fact audit
- transport flows
- program flows
- reasons

Replay is expected to preserve the materialized debugger view, not just the raw
fact stream.

## 7. CLI And Socket Surfaces

Source files:

- [src/main.rs](/Users/Shared/chroot/dev/gewyvern/src/main.rs)
- [crates/gewyc/src/main.rs](/Users/Shared/chroot/dev/gewyvern/crates/gewyc/src/main.rs)
- [src/socket_input.rs](/Users/Shared/chroot/dev/gewyvern/src/socket_input.rs)

Current entry surfaces:

- demo runs
- DSL-driven runs
- DSL compile/diagnostics runs through `gewyc`
- shared `gewyc` render/diagnostics surface consumed by both `gewyc` and `gewyvern`
- `gewyc` is now a separate workspace crate, not just an extra binary target
- planner diagnostics
- Unix socket fact ingest
- TCP socket fact ingest
- JSON and summary output

These are outer system surfaces, not the core runtime itself.

## Current Stable System Boundaries

For `v0.1`, these boundaries are intentional and important:

- DSL compiles to `TemplateBinding`, not kernel bytecode
- all actual kernel-facing behavior comes from built-in fragment templates
- runtime semantics are window-bounded and replay-oriented
- attach outcomes affect ingest semantics directly
- transport flow and program flow are separate layers
- diagnostics are part of the system model, not just debug printouts

## Current Built-In Capability Envelope

Today the system already supports:

- TCP handshake-oriented debugging
- UDP datagram-oriented debugging
- process-aware UDP debugging
- DSL-driven application-path modeling for HTTP, TLS, QUIC, STUN, CoAP, NTP, DHCP, WireGuard, mDNS, SSDP, Redis, and DNS-over-TCP
- shared phase-kind classification across transport and datagram paths
- Linux tracepoint/kprobe/tc-ingress probe paths
- socket-based fact injection
- deterministic export/replay

## What Still Intentionally Hasn’t Happened

These are explicitly not part of `v0.1`:

- generating eBPF from DSL
- treating `ProgramModel` as final DSL design
- complete protocol coverage or parser-level semantics for every protocol
- distributed multi-host runtime
- stable public schema guarantees

## Recommended Reading Order

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
2. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
3. [docs/walkthrough.md](/Users/Shared/chroot/dev/gewyvern/docs/walkthrough.md)
4. [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
5. [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)
6. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
7. [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
8. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
