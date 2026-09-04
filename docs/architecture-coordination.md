# Architecture Coordination

This page defines how changes move through the released four-plane
architecture. Read the [system blueprint](architecture-blueprint.md) first.

The purpose of this protocol is to keep new work inside the project's advantage
zone: protocol-aware evidence, durable authority, equivalent automation
surfaces, and deterministic replay.

## Change Order

Cross-plane work follows one direction:

```text
operator outcome
  -> evidence requirement
  -> versioned domain contract
  -> authority and policy
  -> bounded adapter effect
  -> query/event/UI projection
  -> renderer integration
  -> replay, parity, and recovery proof
```

Not every change touches every step. A change must still begin at the earliest
step that owns its meaning. Starting later creates hidden semantics.

## Plane Responsibilities

### Evidence

Owns:

- GewyLang packages and compiler lowering
- fragment capabilities and attach planning
- facts, flows, reasons, exports, and replay

Owes the authority plane:

- stable runtime identity and capabilities
- bounded machine-readable projections
- explicit loss, confidence, and failure semantics

Must not absorb:

- fleet policy
- GUI workflows
- model advice as observed truth

### Authority

Owns:

- commands, queries, events, and revisions
- authorization, confirmation, idempotency, and scheduling
- persistence, effect journals, receipts, and recovery
- runtime registration, deployment, and retirement

Owes intent and presentation:

- one canonical operation per capability
- actionable typed rejection
- revision-bound projections
- deterministic replay behavior

Must not absorb:

- packet interpretation
- renderer layout
- unbounded framework objects

### Intent

Owns:

- Leselang syntax, HIR, effects, continuations, and command lowering
- renderer-neutral UI documents and presentation operations
- semantic equivalence between language, CLI, and GUI actions

Owes authority:

- typed and capability-declared intent
- no ambient host calls
- no frontend-only privileged operation

Must not absorb:

- a general-purpose VM
- arbitrary native object access
- hidden asynchronous source semantics

### Presentation

Owns:

- native controls and Web rendering
- local layout, focus, accessibility, animation, and platform lifecycle
- platform secret handles and endpoint profiles

Owes intent and authority:

- strict codec behavior
- complete adapter-manifest conformance
- typed action events
- stale-generation and unavailable-action rejection

Must not absorb:

- control policy
- persistence authority
- direct Gewyvern or deployment adapter access

### Advisory

Etragon may consume sanitized evidence and return append-only suggestions. Any
future model integration follows the same rule. Advice is data; it is never an
authority shortcut.

## Routing Rules

Use the first matching route:

| Change | Start Here | Required Handoff |
| --- | --- | --- |
| protocol or observation path | GewyLang package and fragment contract | Gewyvern runtime/replay proof |
| evidence interpretation | Gewyvern IR and diagnosis | machine projection and replay compatibility |
| fleet operation | `leserpent-domain` | runtime, adapter, protocol, Leselang, frontends |
| external deployment effect | domain capability and confirmation policy | adapter receipt and recovery proof |
| GUI automation atom | `leselang-ui` or `leselang-command` | adapter manifest and renderer conformance |
| native-only layout/focus behavior | renderer host | accessibility and lifecycle proof |
| transport encoding | `leserpent-protocol` | compatibility fixtures; no policy change |
| product-neutral file or deadline mechanism | `silvortex-bounded-io` | product-specific errors and policy stay with caller |
| Gewyvern installer/retirement exchange | `gewyvern-install-contract` | adapter authorization and installer effects stay outside codec |
| advisory ranking | Etragon | sanitized append-only result |

If a feature begins in C# or TypeScript but changes domain behavior, stop and
move its meaning into Rust before continuing the frontend work.

## New Operation Protocol

A new control operation is complete only when:

1. The operator outcome and non-goals are written.
2. `leserpent-domain` names the command/query, capability, identity, and
   revision semantics.
3. The Rust runtime owns policy, idempotency, confirmation, and durable state.
4. An adapter executes only the authorized bounded effect and returns a typed
   receipt.
5. IPC/HTTPS/WebSocket transport carries the same versioned meaning.
6. Leselang, CLI, and GUI can express the operation without private shortcuts.
7. Origin parity, crash recovery, stale revision, malformed input, and
   cancellation tests prove the path.

Frontend completion alone is never operation completion.

## New Observation Protocol

A new protocol observation is complete only when:

1. A real debugging question defines the evidence need.
2. A GewyLang package names the protocol path and expected behavior.
3. Existing reviewed fragments cover the need, or a new bounded fragment is
   added explicitly.
4. Compiler reports expose the lowered requirements.
5. Runtime reconstruction remains conservative when evidence is absent or
   partial.
6. Export and replay preserve the same conclusion.
7. Linux attach and malformed-input tests prove the boundary.

Protocol count without a reviewable evidence path is not architectural
progress.

## GUI And Code Equivalence

For every non-local GUI action, reviewers must be able to answer:

- Which command or query does it represent?
- Which capability admits it?
- Which expected revision fences it?
- How does CLI express it?
- How does Leselang express it?
- Which event or projection confirms it?
- What happens after cancellation, crash, reconnect, or replay?

If one answer is missing, the GUI has discovered a contract gap rather than a
reason to add local business logic.

## Compatibility Bridge Rule

The ASP.NET/TypeScript line is a bridge with a strict ratchet:

- it may adapt legacy state or routes into the Rust contracts
- it may render the same projections
- it may retain compatibility persistence needed for migration and recovery
- it may not receive a new source of semantic authority
- every touched mutation path should move closer to daemon authority
- bridge removal must be possible without changing Gewyvern or Leselang

Migration code can be long-lived. Duplicate authority cannot.

## Cross-Plane Review

Before merging a cross-plane change, verify:

1. **Truth owner**: exactly one module can commit the state.
2. **Trust boundary**: secrets and credentials cross only their declared
   adapter or platform vault.
3. **Bounds**: source, message, queue, file, effect, and retry limits are
   explicit.
4. **Failure shape**: external failures become stable machine codes without
   secret-bearing diagnostics.
5. **Replay**: the same durable inputs reproduce the same semantic result.
6. **Replaceability**: no frontend-only operation or hidden control state was
   introduced.
7. **Independence**: standalone Gewyvern and standalone `leserpentd` remain
   usable at their documented boundaries.
8. **Tensor coverage**: changed architecture, module, feature, contract, and
   evidence metadata are updated together.

## Anti-Patterns

Reject changes that:

- call Gewyvern directly from a renderer
- put authorization or revision policy in C# or TypeScript
- add a Leselang host escape for convenience
- infer success from transport completion without an authority receipt
- generate arbitrary eBPF from untrusted DSL
- let an advisory model overwrite observed diagnosis
- create a second persistence writer during migration
- add a protocol package without lowering and replay proof
- use UI clicking as the only automation contract
- make a hosted account necessary for existing self-hosted behavior

## Current 2.0.x Priorities

The released architecture is feature-complete for its declared scope. Current
coordination therefore prioritizes:

1. reliability, security, performance, and operator clarity inside existing
   contracts
2. removal of reverse utility dependencies and bridge-owned authority
3. smaller internal implementation modules behind unchanged public contracts
4. broader independent Linux, macOS, and mobile evidence
5. community integrations that consume protocols without becoming new truth
   owners

Etragon deep learning, Windows native parity, production signing, and hosted
services remain independent tracks. They must not distort the four-plane core.
