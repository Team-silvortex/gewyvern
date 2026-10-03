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
and flat `all`. Bounded pure `loop` adds condition-first scalar-state iteration
under the same fuel budget, before suspension or after one captured result.
Atomic results can re-enter pure scalar bodies or continue bounded multi-step
chains using typed field projections and durable lexical frames, without copying
raw host objects into continuations. Chains keep original authority and shared
budgets, with atomic admission and replay. Mixed pure/suspending branches can
return a typed scalar early or continue through another capture; cold branches
still undergo type/capability checks. Explicit `to_string`, `parse_integer` and
`parse_boolean` bridge scalar calculations and text-based host parameters with
strict formats, shared fuel and redacted conversion faults, never implicit
coercion or a host-validator bypass. Pure `recover` selects a lazy same-type
fallback for local arithmetic/parse failures without refunding fuel or catching
host, authority, resource-limit or cancellation failures. Named group-result
binding now exposes statically typed `member`/`field` projections to a pure scalar
body after the whole group succeeds, with shared fuel and atomic journal replay.
A sequential group can instead drive one atomic tail (up to 63 prefix members
plus one tail), admitting it transactionally with the final prefix acknowledgement.
The reserved identity, original authority/budgets and whole-group recovery prevent
replay from recalculating or dispatching it twice.
That successor can now be captured for a pure scalar body using schema 8. Bounded,
versioned named-member projections preserve the earlier group/alias environment;
raw receipts stay in the journal, and final calculation commits with the successor
receipt only after cumulative output checks.
Flat parallel `all` prefixes now support those same single-successor forms with
schema 9 and an all-success barrier. Their 2 to 63 prefix members remain
independently leaseable and can finish out of order. Exactly one tail is admitted
with the final successful receipt, under the original authority/shared budgets;
cumulative raw-output overflow is a durable terminal. Complete journal recovery
also verifies that every prefix member succeeded before a tail was admitted.
This is a Rust batch API capability, not native multi-presentation GUI support.
Continuation schema 10 extends sequential/parallel groups to multiple captured
atomic successors with a final pure scalar. Prefix plus longest cold chain stays
within 64 slots; all possible successor identities are reserved, while only one
selected successor is pending at a time. Each receipt and next request or final
scalar commits transactionally under original authority/shared budgets. Closed
group/result frames are validated against committed successful predecessors on
whole-journal recovery; isolated restoration is rejected. The database remains
at journal schema 10. Sequential chains work through the native single-presentation
channel; parallel starts retain the batch preflight fence.
Continuation schema 11 adds typed conditional scalar exits before the first group
capture or between captures. Every path returns the same scalar type, while cold
branches still undergo type/capability checks and reserve the longest graph.
The raw receipt and selected scalar or successor commit together after cumulative
output checks, under original authority/shared budgets. A scalar exit does not
bypass the parallel prefix's all-success barrier. Recovery verifies that the saved
body can return without another suspension, replays without recalculation, and
rejects detached restoration. Journal schema 10 is unchanged. Sequential native
debugger flows support these exits without extra presentation requests; parallel
prefixes remain Rust-batch-only.
Reusable pure helpers now declare typed scalar parameters and infer scalar results;
all declarations are checked, recursion is forbidden, and bounded hygienic expansion
uses existing HIR nodes and durable bodies rather than product-specific macros.
Selection and form-requirement results also export typed booleans for decisions
and helpers; versioned closed projection frames preserve old journals without
synthesizing missing fields. Their current operation vocabulary is still a host
adapter concern, not a guarantee of arbitrary GUI-property access.
Confirmed text/form results now export closed string fields for `expected`,
`field` and submitted `value`, usable in helpers, conditional exits and subsequent
host arguments. These are acknowledged request data, not new live GUI-property
queries. Projection vocabulary v3 preserves v1/v2 frames without synthesizing
missing fields, including through aliases and cold branches. Strings keep original
operation domains, shared copy fuel, raw-output fences and the 64 KiB image/plan
limit; saved text must match committed raw receipts. Continuation schemas 1-11 and
journal schema 10 remain unchanged. Native sequential form chains support this
without exposing private frames. Node/action/form-input kind assertion/wait pairs
now also export canonical `kind` token strings through closed vocabulary v4.
Tokens share source/wire spelling and the receiving operation's domain validation;
no case folding, enum ordinals or arbitrary properties are added. Legacy v1/v2/v3
frames stay exact without synthesized fields, including cold aliases/group members;
saved kinds must match committed raw receipts under the same budgets and transaction.
Native sequential kind-driven waits/decisions do not expose private frames.
Typed `optional_string` values now distinguish absent from present-empty text.
Explicit construction, `has_value` and lazy `value_or` support pure helpers/loops;
only four placeholder/unavailability operations export `optional_expected` and
their optional-text arguments accept it directly. Closed projection v5 preserves
v1-v4 without synthesized fields, and explicit null/string payloads reject missing
data. Original text/size/fuel/authority bounds, raw-receipt matching and transactional
replay also apply to optional locals/frames. Native defaults preserve absent/empty
semantics without exposing private frames. Arbitrary credential objects, dynamic
properties and generic nullable/container types remain unsupported.
Bounded `contains`/`starts_with`/`ends_with` predicates now inspect exact strings;
`char_at` returns one optional Unicode scalar at a zero-based index, not a byte
or grapheme offset. Out-of-range access is absent, with no implicit unwrap.
Pure helpers/loops and saved result/group bodies reuse the existing binary HIR,
complete-input scan/copy fuel, host validators and transactional replay. The
Rust native debugger proves text-selected form prefixes through the existing
correlated acknowledgement channel, not direct GUI introspection or a new ABI.
Collection iteration remains pending, followed by effectful loops with exit/skip semantics,
effectful reusable functions, and explicit host-effect recovery/cleanup. These are core language
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
