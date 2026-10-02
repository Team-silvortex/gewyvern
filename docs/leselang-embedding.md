# Leselang Embedding Architecture

This is the target architecture for Leselang as an independent embeddable
control language. It is not a claim that the current runtime is already
product-independent. The [language contract](leselang-language.md) describes
implemented syntax; the [control-flow contract](leselang-control-flow.md)
separates shipped mechanisms from remaining language work.

## Identity

Leselang is intended to become fully independent of Gewyvern and Leserpent,
including its runtime, host protocol, tests, and distribution. Leserpent is the
first reference host, not the definition or owner of the language's semantics.
The Lua-like goal is embedding: an application supplies capabilities and runs
programs through a small language interface. It does not imply Lua syntax,
Lua compatibility, or the same VM implementation.

GUI automation is one host profile, not the limit of the language. A future
shell for nuis OS and sirius kernel is another host profile. That distant goal
does not require an OS implementation now, imply kernel-resident execution, or
claim current `no_std`, process-control, or shell compatibility.

## Agent-First Syntax

Agent-driven GUI and OS control is the syntax design target. Bash, Lua,
JavaScript and other languages are capability references, not syntax or
compatibility targets. New syntax must justify itself through reliable program
generation, validation, composition and recovery, not resemblance to another
language. Human review and debugging must remain clear.

The design rules for future language changes are:

- Prefer a small, regular grammar with explicit names, scopes and typed values.
  Avoid implicit string-to-code conversion, context-sensitive coercions and
  multiple shorthand forms for the same operation. Fewer tokens are useful only
  when they do not increase ambiguity or hide authority.
- Make observation, decision, action and result handling composable. Programs
  should bind typed results and stable host identities rather than parse display
  text or guess coordinates. A well-typed target still needs host validation at
  execution time; observations can become stale.
- Express waiting, cancellation, bounded iteration and recovery without a
  source-level async model. Define loop exit, iteration skip and error-handling
  semantics before choosing keywords; `if`, `for`, `break` or `try` spellings are
  not commitments. A timeout with an unknown external outcome must not become
  an implicit retry of a potentially completed action.
- Support deterministic formatting and source/HIR round trips that preserve
  meaning. Diagnostics should carry stable codes, source spans and typed
  context so an agent can repair a local error without regenerating unrelated
  effects. Recovery from execution and editing a program are distinct actions;
  edited source must not silently reuse an old continuation.
- Discover operations from versioned host schemas, not invented API names.
  Program text may request capabilities but never grants them. Credentials and
  authority remain host-owned; manual GUI actions and generated calls use the
  same policy boundary. Untrusted GUI text and host results remain data, not
  executable instructions or implicit `eval` input.

Future acceptance cases should cover generation from a declared host schema,
typed result binding, a local diagnostic repair, denied authority, a stale
target, cancellation during a wait, and restart without repeating settled
effects. These are design gates, not claims that all such language features
already exist. The current grammar remains authoritative until a versioned
change adds implementation and tests; this section introduces no new syntax.

## Ownership

| Layer | Owns | Must Not Require |
| --- | --- | --- |
| Language core | syntax, values, bindings, functions, control flow, HIR, bounded evaluation, suspension and re-entry | Gewyvern, Leserpent, GUI widgets, a daemon, a particular OS |
| Host protocol | versioned operation schemas, typed inputs/results, capability requirements, identities, cancellation and resource lifetimes | product enums, widget pointers, implicit global authority |
| Optional host profiles | GUI semantics, Leserpent operations, future OS services | changes to core syntax for each new host operation |
| Embedding host | effect execution, event loop, scheduler, storage selection, policy and resources | a second implementation of language semantics |

Dependency direction is host/profile -> language contracts and core, never
core -> product host. Rust is the initial implementation and embedding surface;
other languages use a versioned protocol or narrow FFI boundary. Frameworks
need explicit developer-owned adapters or schema-generated bindings. GUI
conformance remains declared through `UiAdapterManifest`; no GUI framework is
automatically compatible.

Host operations must be declared and validated against a bounded schema before
execution. Domain names such as `runtime.deploy` belong to the Leserpent host
profile, not an ever-growing core opcode list. This is not an untyped
`call(any_string, any_json)` escape hatch or arbitrary native-library loading.

## Execution And Safety

Source remains synchronous and function-oriented. External work yields a typed
effect and an explicit continuation; the host returns a matching result to
resume it. A GUI dispatcher, background worker, or future OS event source may
be asynchronous internally without introducing source-level `async`/`await`.

Independent VM instances must not require a process-global interpreter lock.
The host serializes access to each mutable instance and dispatches GUI work on
the framework's permitted thread. This is not a promise of unsynchronized
access to one VM or of parallel execution inside every program.

Budgets cover pure evaluation, loop iterations, allocation, nesting, output and
external-effect admission. Capabilities are explicitly granted by the host;
declaring an operation never grants its authority. A callback or FFI adapter
must obey cancellation and deadlines itself: VM fuel cannot preempt arbitrary
blocking native code, and an in-process extension is not an OS security sandbox.

Continuations must contain bounded language values and versioned, scoped
resource handles, never host stack frames or native object pointers. A restored
program must revalidate host compatibility and authority before dispatch.
Removed GUI objects or expired resources produce typed failures, not accidental
rebinding to a new object with a reused identifier.

The target core does not require SQLite, a network connection, or a daemon for
an in-memory embedding. Durable journaling is a selectable integration with
explicit recovery guarantees, not a guarantee implied by all embeddings. The
current VM still links SQLite and product types; this separation remains work.
Replay consumes recorded effect results rather than silently repeating external
work. Recovery must retain fencing and idempotency rules; a language engine
alone cannot guarantee exactly-once effects for arbitrary hosts.

## Current Boundary And Next Proof

`leselang-syntax`, `leselang-host-contract`, and `leselang-hir` have no Gewyvern
or Leserpent product crates in their dependency closures. That is a dependency
proof, not full semantic independence: HIR still names concrete runtime and UI
effects, and the host contract contains runtime selectors and deployment
validation. `leselang-vm`, `leselang-ui`, and `leselang-observe` still consume
Leserpent command/result types. `leselang-command` is intentionally the product
adapter and should remain outside the standalone core.

The extraction acceptance gate is:

1. A headless host unrelated to Gewyvern compiles, runs, suspends, resumes and
   cancels a program without any Gewyvern/Leserpent product dependency or GUI.
2. Two independent host schemas can supply different typed operations without
   modifying the parser, core HIR or VM. Unknown operations, wrong result types,
   denied capabilities and incompatible versions fail before unsafe dispatch.
3. The in-memory path needs no persistence backend. A durable backend proves
   restart, stale-result rejection and effect replay under the same protocol.
4. The existing Leserpent adapter preserves command bytes, authorization,
   revision fences, GUI/CLI/code parity and journal migration compatibility.
5. Dependency-closure tests cover the complete embedded core, not only the
   syntax crates; a standalone consumer builds without this workspace's product
   metadata or generated product assets.

The status cell `leselang/language-vm/host-neutral-embedding` tracks this target
separately from mature released Leserpent integration. Keeping code in this
monorepo temporarily does not relax these boundaries or require an immediate
repository move.

## Control Flow And The Future Shell

Pure scalar computation, immutable `bind`, lazy `choose` and whole-group computed
host arguments now precede host suspension alongside `seq`, bounded `repeat`
and flat `all`. One atomic result can now re-enter a pure scalar body using typed
field projections and durable scalar locals. Result-driven host successors and
general environments across multiple suspensions remain pending, followed by
collection iteration, budgeted
data-dependent loops with exit/skip semantics,
reusable functions, and explicit error recovery/cleanup. These are core language
semantics, not GUI macros or complete control flow; see the
[implemented control-flow contract](leselang-control-flow.md) for exact limits.

A future OS profile can supply process/service control, filesystem access,
bounded streams, pipelines, job cancellation and terminal interaction through
the same typed capability boundary. Interactive sessions and long-lived jobs
need renewable, host-authorized execution slices rather than unbounded VM
steps. Process launch is an explicit host operation, not implicit interpolation
through Bash. No particular OS API names or shell syntax are frozen here.

OS integration is a deferred direction, not a prerequisite for Leserpent
maintenance or the first independent embedding. The next proof is the small
unrelated host above, not an OS-sized rewrite.
