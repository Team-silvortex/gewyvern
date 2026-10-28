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
| [`crates/gewylang-contract/`](../crates/gewylang-contract) | product-independent GewyLang identity and stage contract | language/version identity and source bounds |
| [`crates/gewylang-syntax/`](../crates/gewylang-syntax) | product-independent source, package, AST, parser, frontend | canonical syntax interpretation |
| [`crates/gewylang-compiler/`](../crates/gewylang-compiler) | product-independent expansion, host-generic lowering, and materializer dispatch | canonical compiler behavior and host boundaries |
| [`crates/gewylang-ir/`](../crates/gewylang-ir) | product-independent Binding IR, Analysis IR, diagnostics, history, fingerprints, invariant validation, strict wire codec, typed compatibility diff, and projection-host orchestration | stable compiler-stage data, identity, validation, comparison, and exchange contracts |
| [`crates/gewyc/`](../crates/gewyc) | GewyLang compiler CLI | diagnostic and lowering orchestration |
| [`crates/gewyvern-dev/`](../crates/gewyvern-dev) | native development, version, packaging and source-export tooling | contributor workflow, not runtime authority |
| [`crates/silvortex-bounded-io/`](../crates/silvortex-bounded-io) | product-neutral bounded files and transport deadlines | native I/O safety invariants |
| [`crates/silvortex-identity/`](../crates/silvortex-identity) | product-neutral validated protocol identities | identifier grammar and scalar wire identity |
| [`crates/gewyvern-install-contract/`](../crates/gewyvern-install-contract) | strict Gewyvern installation and retirement exchange | cross-plane lifecycle wire contract |
| [`crates/leserpent-domain/`](../crates/leserpent-domain) | shared command/query/event model | control vocabulary |
| [`crates/leselang-syntax/`](../crates/leselang-syntax) | frozen syntax acceptance snapshot, excluded from workspace | implementation comes from pinned independent Leselang |
| [`crates/leselang-runtime-core/`](../crates/leselang-runtime-core) | frozen runtime-foundation acceptance snapshot, excluded from workspace | implementation comes from pinned independent Leselang |
| [`crates/leselang-hir-core/`](../crates/leselang-hir-core) | frozen native HIR acceptance snapshot, excluded from workspace | physical product-free HIR owner is independent Leselang; no reference feature |
| [`crates/leselang-hir/`](../crates/leselang-hir) | native-core and optional product re-exports | compatibility facade; no compiler implementation |
| [`crates/leserpent-hir/`](../crates/leserpent-hir) | Leserpent effects, lowering and capability policy | product owner depending on native HIR core, never the facade |
| [`crates/leselang-vm/`](../crates/leselang-vm) | native session facade and feature-gated legacy product API re-export | no product executor implementation; not yet a fully extracted durable VM |
| [`crates/leserpent-vm/`](../crates/leserpent-vm) | Leserpent-specialized evaluator, live dispatch, memory and SQLite journals | product execution/transaction authority; excluded from the native language export |
| [`crates/leselang-host-contract/`](../crates/leselang-host-contract) | reference-host identities, capabilities and ABI values | host exchange types without a product runtime dependency |
| [`crates/leselang-command/`](../crates/leselang-command) | Leserpent payloads/codecs, pure policy planning and reference execution/recovery coordination | product adapter, not generic language core; native VM owners retain live admission, dispatch and journal commit authority |
| [`crates/leselang-ui/`](../crates/leselang-ui) | renderer-neutral Leserpent UI IR | product presentation contract, not a GUI toolkit |
| [`crates/leselang-observe/`](../crates/leselang-observe) | sanitized projections and optional reference-VM bridge | product observation; read-only does not require a VM |
| [`crates/leserpent-runtime/`](../crates/leserpent-runtime) | transactions, journal, effects, projections | durable control truth |
| [`crates/leserpent-protocol/`](../crates/leserpent-protocol) | versioned wire envelopes and transport semantics | protocol compatibility |
| [`crates/leserpent-adapters/`](../crates/leserpent-adapters) | capability-gated external effects | boundary integration |
| [`crates/leserpentd/`](../crates/leserpentd) | self-hosted authority process | daemon lifecycle and authenticated access |
| [`crates/leserpent-cli/`](../crates/leserpent-cli) | native operator client | no independent semantics |
| [`crates/leserpent-frontend-package/`](../crates/leserpent-frontend-package) | bounded frontend package validation and extraction | asset integrity, not control authority |
| [`apps/leserpent-avalonia/`](../apps/leserpent-avalonia) | desktop renderer and multi-daemon hub | no independent semantics |
| [`apps/leserpent-mobile/`](../apps/leserpent-mobile) | mobile renderer | no independent semantics |
| [`apps/leserpent/`](../apps/leserpent) | ASP.NET/TypeScript Web compatibility bridge | migration and rendering only |
| [`apps/etragon/`](../apps/etragon) | optional advisory sideplane | append-only advice, never control truth |

The table covers every Cargo workspace member, plus the managed client shelves.
Prefixes are not independence guarantees: see the feature boundaries below.
Source and support directories outside the crate map have separate owners:

| Paths | Owns |
| --- | --- |
| `ebpf/` | kernel probe sources, not a portable control service |
| `protocols/`, `dsl/` | checked-in protocol packages and language programs |
| `tests/`, crate-local tests, `apps/*/tests/` | regression and conformance sources; not disposable output |
| `project/status/`, `docs/fixtures/` | live status metadata and retained verification evidence |
| `docs/`, `README.md`, `LESERPENT.md`, `ROADMAP.md`, `STATUS.md` | authoritative guides and short navigation entries |
| `assets/`, `docs/assets/` | shipped assets and documentation media |
| `packaging/`, `scripts/`, `docker/`, `.github/`, `.cargo/` | build inputs, development/validation helpers and CI configuration |
| `target/`, `artifacts/`, app-local `bin/` and `obj/` | generated outputs with different retention rules, not source ownership |

Disk-retention rules live in [Workspace Hygiene](development.md#workspace-hygiene).
The [status catalog](../project/status/catalog.json) remains the only progress
source of truth; this map does not copy maturity scores or historical proof logs.

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
GewyLang source -> gewylang-contract -> gewylang-syntax
                         |                   |
                         v                   v
                    gewylang-ir       gewylang-compiler
                         ^                   |
                         |                   v
                         +------ Gewyvern semantic/analysis host
                                             |
                                             v
                        evidence runtime -> machine evidence contract

silvortex-bounded-io -> Gewyvern / protocol / adapters / CLI / daemon
silvortex-identity -> Gewyvern install contract / leserpent-domain
gewyvern-install-contract -> Gewyvern installer / protocol / adapters

Host-neutral Leselang syntax / runtime-core
  -> native HIR / native VM facade

Leserpent domain + host contract + profiled HIR
  -> command payloads / UI lowering / reference VM / observation
  -> runtime / protocol / adapters / leserpentd
  -> replaceable clients and bridges
```

These arrows show foundation-to-consumer flow, not every direct Cargo edge.
Lower layers must not depend on a renderer. A transport may encode a command,
but it must not decide whether that command is allowed. An adapter may execute
an authorized effect, but it must not mint authority.

The root Gewyvern crate consumes no Leserpent or Leselang crate in its normal
dependency graph. `gewyvern-install-contract` and `leserpent-domain` share the
same validated ID types through `silvortex-identity`; old domain and protocol
paths remain compatibility re-exports with unchanged scalar wire bytes.
GewyLang tooling that only needs source/package parsing can stop at
`gewylang-syntax`; tooling that needs function expansion and canonical lowering
can stop at `gewylang-compiler` and implement `SemanticHost`. A runtime host can
also implement `BindingMaterializer` to receive the canonical assignment stream
without moving its product model into the compiler. Runtime bindings and
evidence-aware analysis deliberately cross into Gewyvern through those explicit
adapters. `CompilerProjectionHost` returns their stable output values and
diagnostic failures to `gewylang-ir` as one coherent stage set. The concrete
Gewyvern adapter remains linked today; extracting it into a narrow crate is the
remaining physical boundary.

## Leselang Extraction Boundary

The native foundation has a separate local MIT repository named `leselang`.
Gewyvern consumes its exact published 0.1.0 Git revision for the three native
cores; ordinary builds do not require a sibling checkout. Retained source files
are non-build acceptance snapshots checked against the resolved upstream bytes.
See [repository ownership and cutover](leselang-repository-handoff.md). Repository
bootstrap does not promote complete language or durable-runtime acceptance.

Do not move all `leselang-*` crates into an independent repository as one unit.
There are two distinct layers today:

| Layer | Current boundary |
| --- | --- |
| Native language foundation | `leselang-syntax`, `leselang-runtime-core`, and HIR/VM with `default-features = false`; host-owned operations, values and synchronous re-entry |
| Leserpent reference specialization | `leserpent-profile`, host contract, command payloads, UI and observation; explicit product vocabulary and adapters |

### Implementation Owners

The arrows below mean consumer -> dependency, unlike the foundation-to-consumer
overview above. Optional compatibility edges do not move implementation ownership.

```text
leselang-vm -> leselang-hir -> leselang-hir-core -> syntax / runtime-core
leselang-hir --[leserpent-profile]--> leserpent-hir -> leselang-hir-core
leselang-vm  --[leserpent-profile]--> leserpent-vm
leserpent-vm -> leselang-command + leserpent-hir -> leselang-hir-core
```

The HIR and VM facades re-export the original owners' types, not parallel
compilers or evaluators. Product command, VM, UI, observation and daemon code
imports product owners directly; runtime/CLI tests do the same. Shared typing
and evaluation go directly to the core. Their normal/build dependency closure
excludes both language compatibility facades. Detailed lowering and recovery contracts belong
in [the embedding reference](leselang-embedding.md), not in this overview.

### Build Profiles

| Entry | Feature selection | Boundary |
| --- | --- | --- |
| Native HIR/VM facades | `default-features = false` | host-defined operations and synchronous re-entry, no Leserpent evaluator or SQLite |
| HIR facade | default `leserpent-profile` | exact product HIR types and lowering through `leserpent-hir` |
| VM facade | no defaults + `leserpent-profile` | product evaluator with an in-memory journal |
| VM facade | default `sqlite-journal` | product evaluator with SQLite; implies `leserpent-profile` |
| Observe | no defaults; optional `vm-bridge`; default `sqlite-journal` | product projections only; memory VM bridge; durable bridge respectively |

Removing SQLite alone does not remove the product profile. Command payloads,
continuation codecs, host contracts and UI IR do not become generic language
core merely by avoiding a VM dependency. Static validation is not live authority.

### Acceptance Owners

| Checks | Source owner |
| --- | --- |
| Native syntax, values, HIR and execution | native crate tests and default-free facade/isolated-consumer tests |
| Product-owned execution, scheduling and journal acceptance | [`leserpent-vm/tests/`](../crates/leserpent-vm/tests) |
| Shared product dispatch/request-construction oracles | [`leserpent-vm/tests/support/`](../crates/leserpent-vm/tests/support) |
| Released HIR/VM paths and type identity | [`leselang-hir/tests/`](../crates/leselang-hir/tests) and [`leselang-vm/tests/`](../crates/leselang-vm/tests) |
| Cross-package ownership, feature and status gates | root [`tests/`](../tests) |

Where a product acceptance body has moved, the legacy target includes that
same body through explicit facade aliases. Remaining legacy suites still need
individual classification; do not delete them wholesale. The separate
facade/product type-identity proof must keep comparing distinct import paths.
The native export support inventory contains only `native_checkpoint.rs` and
`native_group_checkpoint.rs`; product oracles are not export prerequisites.

Cargo features are additive within a resolved graph. Validate native consumers
in isolation rather than inferring independence from a default workspace build.
The current five-crate source checkpoint contains syntax, runtime-core, HIR core,
the compatible HIR facade and native VM facade; it excludes product/reference code.
It remains `complete_vm=false` and `full_extraction_gate=false`: host-neutral
durable recovery and the full-language independent host pipeline are still
open. See [embedding boundaries](leselang-embedding.md) for authoritative gates.

Keep pure language mechanisms below host adapters. Keep live authority checks,
durable product state and renderer dispatch in their current owners. A moved
codec or a successful source export is not evidence that the full VM is ready
to split.

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
