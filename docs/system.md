# System Guide

Use this page when you need the current system-level map for `gewyvern`.

This page is intentionally a durable architecture note. It explains:

- how the current runtime is layered
- what each layer is responsible for
- where the main architectural boundaries are
- which capabilities are intentionally in scope today

This page is not the best first stop for:

- a first operator run
- exact diagnosis field lookup
- exact `gewylang` package/module lookup
- task-oriented validation steps

For those, use:

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

If you already understand the architecture and only need the exact contract
companions for the protocol shelf or compiler IR surface, jump to:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

If you want the project-level design sheets before reading the prose below,
start with:

- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)

## Role In The Shelf

Treat this page as the system-level front door for architecture reading.

Use it when you want:

- the whole runtime in one layered map
- the boundary between compiler, registry, runtime, export, and operator
  surfaces
- the shortest path toward the right deeper architecture page

Then branch like this:

- blueprint / design sheet first:
  [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
- runtime-pipeline deep dive:
  [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
- module ownership and source layering:
  [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
  and
  [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
- cross-line evolution and coordination:
  [docs/architecture-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-evolution.md)
  and
  [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)

## One-Sentence Model

`gewyvern` is a protocol-agnostic, window-bounded network debugger where a DSL
compiles into fragment-template bindings, runtime planning/probing produces a
fact stream, and the engine reconstructs transport flows, program flows, and
deterministic reasons from that evidence.

## Companion Shelves

Read this page alongside:

- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
  for the fastest project-level design sheet
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  for the runtime-pipeline and IR-bearing deep dive
- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
  for the source-cluster dependency picture
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  for file-level ownership and layering rules
- [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)
  for protocol / IR / runtime / collaboration sequencing

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
count, structured function nodes with per-function step counts, a unified graph
of entry/file/function nodes plus `include`/`use` edges, source line metadata
for those edges, and resolved include sources.

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

Parse findings now also carry front-end specific codes for common pipeline and
package-entry failures, including unknown `use()` targets, unknown package
dependencies, non-filesystem `include()` calls, invalid function bodies, and
unclosed function blocks. Those findings continue to preserve the DSL source
line, so frontends can anchor them to concrete `include()` and `use()`
statements.

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

These boundaries are intentional and important:

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

These are explicitly not part of the current core:

- generating eBPF from DSL
- treating `ProgramModel` as final DSL design
- complete protocol coverage or parser-level semantics for every protocol
- distributed multi-host runtime
- stable public schema guarantees

## Companion Guides

- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Source-layering note for the current repository layout.
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
  Broader project architecture and major component relationships.
- [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)
  Fragment capabilities, attach semantics, and evidence surfaces.
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Stable `gewylang` language surface and current preferred subset.
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
  Replay/export contract and top-level bundle shape.
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
  Contributor-oriented development workflow and local project practice.
