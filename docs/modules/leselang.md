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
2.3.0 targets extraction after complete host/compatibility gates; the native source checkpoint exports unchanged production code with independent metadata, tests and CI, not a complete durable VM.
`leselang-hir` defaults to the compatible `leserpent-profile`; independent hosts
can use `default-features = false` to omit product DTOs, fixed operations and the
host-contract/identity dependency while keeping generic compilation and sessions.
Native-only CI and default compatibility tests are separate; Cargo feature
unification can re-enable the profile, and full independent VM extraction is still open.
`leselang-runtime-core` now isolates admission lifecycle, fuel accounting,
clock/backoff arithmetic, shared closed scalar data/operations and faults. Scalar/list
imports are shared types with legacy wire bytes, not parallel implementations;
expression/constructor expansion costs and host validation stay in HIR/VM.
Shared HIR owns bounded scalar execution, lexical frames, source construction,
catalog/type inference, acyclic helper compilation and borrowed reply sessions.
Parsed unrelated-host verticals cover typed replies, re-entry, cancellation and
live policy checks without a product VM or SQLite. Generic group/helper/graph
admission APIs also exist. Flat-All sessions now retain out-of-order accepted replies
behind one complete ordered barrier. Complete original-group assembly and borrowed
member lookup connect to authenticated enclosing-control re-entry with the same
owned fuel. Flat-group catalog compilation now owns the complete source expression.
Explicit program profiles cover entry-only groups and flat-group helper returns,
sharing hygiene/quotas; prepared atomic helpers join scoped Seq/All replies across counter and headless GUI schemas with heterogeneous acknowledgements; legacy members remain direct calls.
Effect dispatch, grants, native schema/work limits, authentic receipts, durable
capture and replay remain adapter-owned; no implicit coercion or authority is added.
The reference VM can now disable its default `sqlite-journal` backend; memory execution keeps the existing evaluator, but still uses product DTOs and is not the independent VM.
Owned capture/scope, saved-frame/budget data and inherited-context agreement, checkpoint staging and monotonic watermark planning, saved-capture extension and correspondence, ordered terminal-outcome summaries, flat-group, sequential-frontier and adjacent-chain inspection, and reservation/receipt recovery fences live in runtime-core; shared HIR executes saved bodies with original fuel. SQLite shares the kernels before native validation/commit and original-watermark repair. Watermark planning preserves existing gaps and exact write decisions without allocation or commit authority. Context agreement preserves native authority/revision order and exact deadline/output inheritance without admitting a budget or handling fuel. Capture extension preserves original named bindings and the current result without authority transfer. Capture correspondence keeps complete original scans, including errors after an early match. Outcome summaries retain sequential first-stop and deferred parallel-overflow precedence without settlement authority. Frontier inspection orders complete-state, prefix and blocked-dispatch checks; chain inspection preserves parent-result-before-next-request fault order without evaluating saved bodies. Wire, authentic receipts, policy and complete independent graph-journal recovery stay adapter-owned.
See the embedding architecture for individual contracts and extraction gaps,
including nested native graph policy, product migration and durable suspension.
