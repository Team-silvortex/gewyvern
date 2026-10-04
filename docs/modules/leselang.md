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
`leselang-runtime-core` now isolates admission lifecycle, fuel accounting,
clock/backoff arithmetic, shared closed scalar data/operations and faults. Scalar/list
imports are shared types with legacy wire bytes, not parallel implementations;
expression/constructor expansion costs and host validation stay in HIR/VM.
Scalar signatures, eager operations, left-value short-circuit decisions and
condition-first loop budgets, owned fold cursors, incremental list builders and
borrowed lexical frames are shared; evaluation, preflight, fuel policy, durable
capture and recovery stay in VM without syntax or wire changes.
HIR and restored-projection preflight share the lexical guard, not their type,
node-budget, legacy-field or authority policies.
Ordered scalar projections share opaque-key schema checks and the VM's field DTO;
operation domains, authority and receipt/replay checks remain adapter-owned.
Payload-free terminal observations
distinguish local cancellation, expiry, exhaustion, rejection, acceptance and
host uncertainty without replay authority or legacy wire changes; cleanup retains
known reasons and releases undelivered outputs on unwind. It is
not yet the full evaluator or generic typed host-operation engine.
