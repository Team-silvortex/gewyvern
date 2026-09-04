# Architecture Evolution

This page describes how the released `2.0.x` architecture should deepen
without dissolving its boundaries. It is directional guidance, not a promise
that a feature belongs to a particular version.

Start with the [canonical blueprint](architecture-blueprint.md), then use the
[coordination protocol](architecture-coordination.md) for individual changes.
Historical version narratives live under [history](history/index.md).

## From Product Stack To Debugging Fabric

The pre-2.0 work assembled three strong ideas:

- Gewyvern turns bounded kernel evidence into protocol-aware explanations.
- Leserpent turns many runtime instances into durable, recoverable authority.
- Leselang makes GUI and code control share one typed semantic boundary.

The post-2.0 task is not to add a fourth unrelated product. It is to make these
ideas compound into a replayable, protocolized network debugging fabric.

The compounding loop is:

```text
incident
  -> bounded evidence
  -> deterministic explanation
  -> typed remediation or experiment
  -> durable effect receipt
  -> new evidence
  -> replay and comparison
```

Every improvement should shorten, strengthen, or clarify this loop.

## Evolution Horizons

### Horizon 1: Protect The Released Loop

Current `2.0.x` work should:

- preserve machine and wire compatibility
- keep Linux evidence collection reliable and bounded
- keep GUI, CLI, and Leselang origin parity green
- improve startup, deployment, recovery, diagnostics, and packaging
- retain exact security, resource, and performance evidence
- keep all existing core capabilities usable without an account

This horizon changes implementation freely when necessary, but does not add a
new authority or capability family.

### Horizon 2: Remove Boundary Debt

The next architectural leverage comes from cleaner extraction, not more
surface area.

Priority seams:

1. Keep `silvortex-bounded-io` and `silvortex-identity` free of product
   semantics and preserve their single-source safety and compatibility tests.
2. Preserve the completed identity extraction: old domain paths, scalar wire
   bytes, rejection rules, and shared Rust type identity must remain stable.
3. Keep Gewyvern installer integration separate from diagnosis semantics and
   feature-gate it when independent package publication requires that split.
4. Ratchet ASP.NET mutation paths toward Rust daemon authority until the Web
   line is only a renderer and compatibility adapter.
5. Split large Leselang VM/UI and Leserpent runtime/persistence files along
   their existing public contracts.
6. Generate strict foreign-language codecs and renderer bindings from stable
   schemas where generation removes hand-maintained semantic duplication.

Extraction succeeds only when behavior and protocol remain unchanged.

### Horizon 3: Deepen The Advantage Zone

After boundaries are cleaner, deepen capabilities that reinforce the loop:

- richer protocol program models grounded in real evidence
- better causal comparison across replayed sessions
- safer experiment and remediation plans with explicit rollback
- stronger multi-authority topology views without merged authority
- more complete renderer adapters generated from the Leselang UI schema
- community-authored protocol packages with reproducible attach/replay proof

Breadth is useful when it improves the same debugging model. Breadth that needs
a new truth owner belongs outside the core.

### Horizon 4: Optional Adjacent Systems

Independent tracks may mature without becoming prerequisites:

- Etragon with reproducible model artifacts and evaluation
- Windows native clients
- production signing and notarization
- hosted collaboration or subscription services
- additional physical-device and kernel matrices

Each track must consume the open protocols and preserve local self-hosted
authority.

## Pace By Contract Layer

### Fast Moving

- protocol packages and examples
- renderer-local UX and accessibility
- diagnostics and operator guidance
- test fixtures and community evidence
- internal module extraction

### Deliberate

- GewyLang and Leselang syntax
- runtime IR and UI IR
- command/query domain additions
- adapter capability and receipt shapes
- deployment topology behavior

### Slow Moving

- evidence truth ownership
- authority and revision semantics
- durable continuation format
- replay determinism
- open-source core boundary
- wire compatibility and secret policy

The slower a layer moves, the stronger its migration and retained evidence must
be.

## Independence Targets

The monorepo is a coordination convenience, not an excuse for one inseparable
binary.

- Gewyvern remains a standalone debugger and reusable runtime/compiler core.
- GewyLang remains a reusable authoring and lowering toolchain.
- `leserpentd` remains a standalone authority service.
- Leserpent CLI remains a standalone operator surface.
- Leselang crates remain hostable by Rust and adaptable through generated
  protocol or narrow FFI boundaries.
- Avalonia, mobile, and Web remain replaceable renderers.
- Etragon remains an optional standalone advisory service.

Independent publication may come later. Architectural independence is enforced
now through one-way contracts and tests.

## Monolith Reduction

Large source files are not automatically bad, but they become dangerous when
they hide more than one authority or contract boundary.

When touching a large module:

1. identify the existing public contract
2. separate parsing, validation, state transition, persistence, and rendering
   internally
3. keep one authority entry point
4. add characterization tests before moving code
5. preserve serialized forms and error codes
6. measure compile time and runtime before and after

Do not split files merely to lower line counts. Split where tests can prove a
stable semantic seam.

## Proof-Driven Maturity

Architecture maturity is demonstrated by:

- malformed input fails closed without process loss
- cancellation and shutdown remain bounded under hostile peers
- crash recovery preserves one durable authority
- replay produces stable conclusions
- GUI, CLI, and Leselang produce equivalent command plans
- renderers reject schema and capability drift
- independent processes expose exact resource and performance baselines
- compatibility bridges can be removed from a test topology without changing
  core semantics

The [status tensor](project-status-system.md) records current evidence,
independence, blockers, and next gates. A high score is a snapshot, not a reason
to stop falsifying the architecture.

## Decision Filter

Prefer a proposed change when it answers yes to most of these:

- Does it improve a real debugging loop?
- Does it strengthen observed evidence or make uncertainty clearer?
- Does it preserve exactly one truth owner?
- Can GUI, CLI, and Leselang share the result?
- Can the result be exported, replayed, or compared?
- Does it keep privileged behavior bounded?
- Does it preserve standalone and self-hosted operation?
- Can tests prove the behavior without relying on one machine or UI?

Defer it when its main value is genericity, feature count, or imitation of an
established tool category.

## Current Thesis

The strongest path after 2.0 is:

```text
protect contracts
  -> remove reverse dependencies
  -> shrink compatibility authority
  -> deepen evidence/action replay
  -> broaden adapters and community proof
```

That path creates a category advantage. It does not ask Gewyvern to beat every
packet analyzer, proxy, observability platform, or GUI framework at its own
game. It makes the whole evidence-to-action loop something those isolated tools
do not provide.
