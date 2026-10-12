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

## Contracts

- [Domain and protocol compatibility](../../crates/leserpent-protocol/COMPATIBILITY.md)
- [Project status tensor](../project-status-system.md)
- [Host-neutral runtime foundation and standalone package checks](../../crates/leselang-runtime-core/README.md)
- [GewyLang module](gewylang.md)

Leselang and GewyLang are separate languages. GewyLang defines protocol
behavior; Leselang controls explicitly adapted hosts, including Leserpent
orchestration and UI functions today. Full runtime independence remains a
tracked target, not a completed property of every Leselang crate.
The 2.3.0-and-later phase closes complete host/recovery/compatibility gates;
physical repository extraction is deferred with no replacement version or date.
The native source checkpoint exports unchanged production code with independent
metadata, tests and CI, not a complete durable VM. Prioritize complete host
verticals and reference-adapter parity over additional isolated public helpers.
`leselang-hir` defaults to the compatible `leserpent-profile`; independent hosts
can use `default-features = false` to omit product DTOs, fixed operations and the
host-contract/identity dependency while keeping generic compilation and sessions.
Native-only CI and default compatibility tests are separate; Cargo feature
unification can re-enable the profile, and full independent VM extraction is still open.
`leselang-runtime-core` owns reusable admission, scalar/fuel/scope primitives,
continuation data, recovery observations and bounded watermark planning. Optional
local execution owners revoke weak leases before guarded native cleanup; they
do not replace session state, host authority or external cancellation delivery.
The opt-in `native::LocalVm` mirrors the original session's terminal liveness
without changing `native::Vm`, its meter or its conditional Send behavior.
Shared HIR owns typed-prefix expression/program compilation, pure execution, helpers, cold-bounded groups and reply sessions.
Parsed unrelated-host profiles prove in-memory replies, re-entry and cancellation,
not the complete independent VM or host-neutral durable recovery.
The VM's native facade and source checkpoint are product-free; `leserpent-profile` selects the reference
VM, and `sqlite-journal` preserves default durable behavior. Full parity stays open.
Effect dispatch, grants, native schema/work limits, authentic receipts and replay
remain adapter-owned. Shared kernels preserve original budgets and fault order;
wire compatibility and complete graph-journal recovery still require acceptance.
See the embedding architecture for exact contracts, implemented profiles,
remaining product/graph boundaries and complete host/recovery/parity exit proofs.
