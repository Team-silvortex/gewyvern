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
`leselang-runtime-core` owns admission, scalar/fuel/scope primitives,
continuation data, recovery observations and bounded watermark planning.
Optional owners revoke weak leases before guarded native cleanup.
`native::LocalVm` mirrors terminal liveness without changing `native::Vm`,
its meter or conditional Send behavior. Host work enters only while Awaiting;
`with_host_outcome` selects Pending, Cancelled, Failed or Uncertain without panic.
Scoped Seq/All polling and reply checks stop on revocation before request/result handoff; original fuel is retained. Native authority, parent-session and external cancellation remain host-owned.
Shared HIR owns typed-prefix expression/program compilation, pure execution, helpers, cold-bounded groups and reply sessions.
Parsed unrelated-host profiles prove in-memory replies, re-entry and cancellation,
not the complete independent VM or host-neutral durable recovery.
The native VM pipeline matrix now joins bounded control, helpers, typed replies,
captured successors and cancellation for scalar-counter and structured-GUI-ack
hosts. Its mixed matrix now connects atomic calls, seq/all/repeat, captured
successors and a second group using the original collectors and one meter.
It runs unchanged outside the monorepo; complete language parity, durable
recovery and reference compatibility remain open acceptance work.
The VM's native facade and source checkpoint are product-free; `leserpent-profile` selects the reference
VM, and `sqlite-journal` preserves default durable behavior. Full parity stays open.
Leserpent operation/reply DTOs now belong to `leselang-command`; released VM paths
are exact re-exports. `leserpent-protocol` uses that adapter without linking the
VM or SQLite. This narrows product protocol dependencies without claiming that
reference continuation, dispatch or durable recovery are already host-neutral.
The adapter also owns all 62 presentation-effect payload mappings and fixed wait
timeouts. The VM retains one envelope builder, its original fuel debit, request
validation and non-GUI confirmation path; conversion alone grants no authority.
Borrowed request payload checks now live in `PresentationOperation::matches_effect`.
The VM uses the existing host signature catalog to select its single GUI gate,
preserving image/identity/authority/payload/budget order and non-GUI mismatch faults.
Query/command payload correspondence is also adapter-owned, including fixed log
policy and command confirmation/identity/revision metadata. The VM still derives
trusted IDs, supplies the original revision and checks authority before matching;
these predicates do not grant authority or provide replay protection.
Control-plane reply correspondence now also lives in the adapter: runtime and
saved command identity, applied status and debugger timestamp matching no longer
belong to the VM. Owned control-plane reply-to-value conversion is also adapter-owned,
using the original declared type and moving buffers without cloning. The VM keeps
saved result types, a single query revision/output gate before conversion, dispatch
authority and terminal replay, with before/after memory and SQLite-restart tests.
Product scalar field projection is adapter-owned through `Value::project_scalar`;
the VM keeps capture versions, validation, field-access rules, budget accounting
and `LSV1405` refusal mapping. All 77 result types and 12 explicit fields are
covered, with frozen five-version capture compatibility. Projection alone neither
accepts a host reply nor makes a saved frame trustworthy.
Continuation result-type checks also reuse the HIR host signature catalog rather
than a second VM operation table. All 70 operation signatures, wrong-type restore
refusals and SQLite corruption/reopening are covered without changing frame,
authority, canonical-source or resource gates. This is reference-policy cleanup,
not completion of host-neutral durable recovery.
Borrowed correspondence checks for all 62 presentation replies now also live in
the adapter. Fixed waits, optional text, focus navigation and child-count rules
remain unchanged; legacy image-only resume and saved-envelope checks are tested
separately. Released product `Value`/`StructuredField` data, exhaustive reply
conversion and type-tag lookup are adapter-owned with exact VM re-exports.
The VM checks correspondence before conversion drops wait metadata, then fences
the result type. Borrowed leaf bounds and item counts now also belong to the
adapter. The VM retains recursive depth/field checks, aggregate and per-execution
budgets, encoded-size limits, admission and recovery, with unchanged fault order.
The leaf checker is not full domain validation or authority; the native value
model is unchanged.
Effect dispatch, grants, native schema/work limits, authentic receipts and replay
remain adapter-owned. Shared kernels preserve original budgets and fault order;
wire compatibility and complete graph-journal recovery still require acceptance.
See the embedding architecture for contracts, profiles and remaining host/recovery/parity gates.
