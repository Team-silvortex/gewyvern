# Leselang Documentation Module

Owns the independent embeddable control language, its compiler boundary, effect model,
capability checks, continuation format, and deterministic re-entry semantics.

Leserpent is the first reference host, not the language's required runtime.
The intended core is a hostable Rust crate plus a narrow protocol/FFI surface
for other languages. GUI automation is one host profile; a shell for nuis OS
and sirius kernel is a distant host profile, not an implemented capability.
No GUI framework is automatically compatible; each one needs a developer-owned
adapter or generated framework binding against the Leselang UI protocol. The
`UiAdapterManifest` is the shared proof for that binding.

## Start

1. [First GUI automation tutorial](../book/tutorial-leselang-gui-automation.md)
2. [Current language contract](../leselang-language.md)
3. [Renderer-neutral UI IR contract](../leselang-ui.md)
4. [Leserpent 2.0 architecture](../leserpent-2-architecture.md)
5. [Gate-based delivery roadmap](../leserpent-2-roadmap.md)
6. [Computation, pure recovery, bounded loops, atomic chains and named group results](../leselang-control-flow.md)
7. [Independent embedding architecture and extraction gates](../leselang-embedding.md)
8. [Runnable product-free Rust host and bounded two-instance event loop](../../crates/leselang-vm/README.md#runnable-native-host)

## Contracts

- [Domain and protocol compatibility](../../crates/leserpent-protocol/COMPATIBILITY.md)
- [Project status tensor](../project-status-system.md)
- [Host-neutral runtime foundation and standalone package checks](../../crates/leselang-runtime-core/README.md)
- [GewyLang module](gewylang.md)

## Current Boundary

Leselang and GewyLang are separate languages: GewyLang defines protocol behavior;
Leselang controls explicitly adapted hosts, including Leserpent orchestration and UI.

The 2.3.0-and-later phase closes complete host/recovery/compatibility gates;
physical repository extraction is deferred with no replacement version or date.
Native isolation is available, but full independent VM extraction is still open.
The native source checkpoint exports unchanged code with independent metadata,
tests and CI; it is not a complete durable VM or a release-publication gate.

`leselang-hir` and `leselang-vm` offer native builds without the `leserpent-profile`.
The reference profile preserves product compatibility; `sqlite-journal` selects
its durable backend. Cargo feature unification is not an authority boundary.
Shared HIR owns compilation, evaluation and reply-gated sessions. Hosts retain
live authority, authentic replies, native work bounds and external cancellation.

The command adapter owns product data, captures and the shared execution environment;
the reference VM retains execution grants, schema/identity gates and journal policy.
Adapter-only tests now execute captured successors through the shared HIR without
VM/SQLite linkage. In-memory wire round-trips do not prove durable recovery.
See [VM profiles and adapter ownership](../../crates/leselang-vm/README.md) and
[the captured-execution proof](../leselang-embedding.md#adapter-only-captured-execution-proof).

Complete two-host language coverage, host-neutral durable graph recovery and full
reference wire/authority/GUI/CLI parity remain open. Track acceptance in the status
tensor; implementation details belong in the linked contracts, not this index.
