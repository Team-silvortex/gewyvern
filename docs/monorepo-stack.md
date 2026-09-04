# Monorepo Stack Guide

This repository ships one debugging fabric through several independently
replaceable processes and renderers. A shared checkout does not imply shared
authority: each semantic decision still has exactly one owner.

Read the [canonical architecture blueprint](architecture-blueprint.md) before
changing a cross-project boundary.

## Repository Map

| Path | Role | Authority |
| --- | --- | --- |
| [`src/`](../src) | Gewyvern runtime, API, evidence reconstruction, CLI | observed network truth |
| [`crates/gewyc/`](../crates/gewyc) | GewyLang compiler CLI | language diagnostics and lowering |
| [`crates/silvortex-bounded-io/`](../crates/silvortex-bounded-io) | product-neutral bounded files and transport deadlines | native I/O safety invariants |
| [`crates/silvortex-identity/`](../crates/silvortex-identity) | product-neutral validated protocol identities | identifier grammar and scalar wire identity |
| [`crates/gewyvern-install-contract/`](../crates/gewyvern-install-contract) | strict Gewyvern installation and retirement exchange | cross-plane lifecycle wire contract |
| [`crates/leserpent-domain/`](../crates/leserpent-domain) | shared command/query/event model | control vocabulary |
| [`crates/leselang-*`](../crates) | Leselang syntax, HIR, VM, command and UI lowering | normalized automation intent |
| [`crates/leserpent-runtime/`](../crates/leserpent-runtime) | transactions, journal, effects, projections | durable control truth |
| [`crates/leserpent-protocol/`](../crates/leserpent-protocol) | versioned wire envelopes and transport semantics | protocol compatibility |
| [`crates/leserpent-adapters/`](../crates/leserpent-adapters) | capability-gated external effects | boundary integration |
| [`crates/leserpentd/`](../crates/leserpentd) | self-hosted authority process | daemon lifecycle and authenticated access |
| [`crates/leserpent-cli/`](../crates/leserpent-cli) | native operator client | no independent semantics |
| [`apps/leserpent-avalonia/`](../apps/leserpent-avalonia) | desktop renderer and multi-daemon hub | no independent semantics |
| [`apps/leserpent-mobile/`](../apps/leserpent-mobile) | mobile renderer | no independent semantics |
| [`apps/leserpent/`](../apps/leserpent) | ASP.NET/TypeScript Web compatibility bridge | migration and rendering only |
| [`apps/etragon/`](../apps/etragon) | optional advisory sideplane | append-only advice, never control truth |

## Process Topology

```text
Avalonia / mobile / Web / CLI / Leselang
                    |
         authenticated protocol
                    |
             one leserpentd
                    |
        versioned Gewyvern contract
             /      |      \
        Gewyvern Gewyvern Gewyvern
           |        |        |
        kernel or container boundaries
```

The cardinality rules are stable:

- one kernel or container boundary maps to one Gewyvern service
- one `leserpentd` authority manages many Gewyvern services
- one client may manage many independent `leserpentd` authorities
- a renderer may disappear without changing durable control truth
- Etragon may disappear without changing evidence or control truth

## Semantic Ownership

`gewyvern` owns capture, protocol interpretation, reconstruction, reasons,
replay, and exported evidence. GewyLang describes protocol-aware evidence
programs that lower into bounded, verifier-safe runtime plans.

Rust Leserpent owns identity, capabilities, revisions, confirmation, durable
commands, effects, audit, and projections. Leselang, CLI, Avalonia, mobile, and
Web all consume this same command/query model; none may invent a private action.

The ASP.NET/TypeScript application is a supported compatibility bridge and Web
renderer. New control semantics enter the Rust domain/protocol first, then the
bridge consumes them. Managed persistence must not become a second authority.

Etragon is deliberately advisory and downweighted until its later learning
stack has independent evidence. Advice is append-only and can never rewrite an
observed fact, command result, or audit record.

## Dependency Direction

The intended dependency flow is:

```text
GewyLang source -> Gewyvern evidence runtime -> machine evidence contract

silvortex-bounded-io -> Gewyvern / protocol / adapters / CLI / daemon
silvortex-identity -> Gewyvern install contract / leserpent-domain
gewyvern-install-contract -> Gewyvern installer / protocol / adapters

leserpent-domain
  -> Leselang HIR/VM/lowering
  -> leserpent-runtime
  -> leserpent-protocol
  -> leserpent-adapters
  -> leserpentd
  -> replaceable clients and bridges
```

Lower layers must not depend on a renderer. A transport may encode a command,
but it must not decide whether that command is allowed. An adapter may execute
an authorized effect, but it must not mint authority.

The root Gewyvern crate consumes no Leserpent or Leselang crate in its normal
dependency graph. `gewyvern-install-contract` and `leserpent-domain` share the
same validated ID types through `silvortex-identity`; old domain and protocol
paths remain compatibility re-exports with unchanged scalar wire bytes.

## Toolchain Boundaries

The repository intentionally uses three implementation toolchains:

- Rust builds Gewyvern, GewyLang, Leselang, Leserpent authority, daemon, CLI,
  adapters, protocol, native packaging coordinators, and Etragon.
- C# builds replaceable Avalonia desktop/mobile renderers and the retained
  ASP.NET compatibility host.
- TypeScript builds browser rendering and interaction code only.

Node.js, Python, and shell may assist development or validation, but they do
not carry production control-plane semantics. Native entrypoints are preferred
for product build, packaging, bootstrap, and deployment flows.

## Release Versioning

The repository publishes one shared mainline version from the root Rust
workspace. Product crates, native clients, the Web bridge, and Etragon no
longer carry independent release numbers; their package metadata inherits or
follows the root `gewyvern` release line. Protocol contract versions remain
independent because they describe compatibility rather than product releases.

## Common Commands

From the repository root:

```bash
cargo dev doctor
cargo dev check
cargo dev build
cargo test --workspace
cargo run --bin gewyvern_status -- validate
```

For the Web compatibility bridge:

```bash
cd apps/leserpent
npm run check:frontend
npm run verify:frontend-package
dotnet build src/Leserpent/Leserpent.csproj
```

For native client packaging and platform-specific checks, use the commands in
[Packaging](packaging.md) and [Release Checklist](release-checklist.md) instead
of inventing an alternate build path.

## Change Rules

When a change crosses stack boundaries:

1. Name the owning plane and authority before editing an endpoint or renderer.
2. Add or revise the neutral domain operation/query/event first.
3. Implement authority, revision, capability, and persistence behavior in Rust.
4. Expose the operation through the versioned protocol.
5. Adapt every supported renderer without adding renderer-only semantics.
6. Prove GUI, CLI, and Leselang equivalence where the action is user-visible.
7. Update the status tensor and its evidence in the same change.

Do not extract code merely because files are large. Extract a module only when
it has a named contract, one-way dependencies, focused tests, and a plausible
independent consumer. This prevents cosmetic fragmentation while the large VM,
UI lowering, runtime, persistence, and desktop classes are reduced along real
semantic seams.

## Validation Order

Use the narrowest proof that crosses the changed boundary, then widen:

1. crate or frontend unit tests
2. command/query and wire compatibility tests
3. daemon persistence and replay tests
4. GUI function-chain conformance
5. packaging and install verification
6. Linux physical-host evidence when kernel behavior changed

The status tensor is the machine-readable map of those proofs. Historical
roadmaps explain how the system arrived here; they do not override current
implementation evidence.

## Related Pages

- [Architecture Blueprint](architecture-blueprint.md)
- [Architecture Coordination](architecture-coordination.md)
- [Architecture Evolution](architecture-evolution.md)
- [Module Boundaries](module-boundaries.md)
- [Project Status Tensor](project-status-system.md)
- [Leserpent 2.0 Architecture](leserpent-2-architecture.md)
