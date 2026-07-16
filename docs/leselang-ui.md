# Leselang UI IR Contract

This document defines the implemented renderer-neutral Gate 4 UI boundary in
`crates/leselang-ui`. Avalonia, web, mobile, persistence, transport, and adapter
types are deliberately outside this contract.

Status: **Gate 4 renderer-neutral slice complete, evolving contract 0.13.0**.

## Pure Flow

```text
QueryResult::RuntimeList -> fleet_document -> UiDocument
bounded runtime log source -> leselang-observe -> RuntimeLogProjection
RuntimeLogProjection -> runtime_log_document -> UiDocument
validated EffectRequest -> leselang-observe -> DebuggerProjection
DebuggerProjection -> debugger_document -> UiDocument
DebuggerCancel CommandPlan -> inspect/dry-run -> VM cancellation
UiEvent + UiDocument + LoweringContext -> CommandPlan
previous UiDocument + next UiDocument -> UiPatch
```

The first vertical slice renders the fleet projection into semantic columns,
headings, text, runtime cards, and actions. It does not include runtime
endpoints or arbitrary network locations. Text uses localization keys plus
bounded fallback strings; every action has an accessibility label.

The runtime child-workspace slice combines `RuntimeInspect` and
`RuntimeHistory` only when both carry the same domain revision. Its semantic
tree contains runtime status, snapshot availability, a refresh action, and the
bounded newest-first command history. Mismatched revisions fail as torn state;
endpoint and persistence details remain absent. Stable workspace and history
entry IDs allow an empty history to become an incremental insert rather than a
full document replacement.

The log slice consumes a renderer-neutral `RuntimeLogProjection`, not raw
adapter output. A trusted runtime producer supplies only revision, runtime
identity, display name, and sanitized typed entries. Batches are capped at 256
entries; sequence numbers must increase strictly; display text is capped at 768
bytes and rejects control characters. Endpoint, transport, persistence, and
arbitrary adapter fields cannot enter the projection. The `leselang-observe`
producer now accepts a deliberately narrow source record, rejects batches over
4096 records and messages over 64 KiB, validates sequence monotonicity across
the complete source batch, keeps the newest 256 records, and performs UTF-8-safe
control-character normalization and truncation. The authoritative runtime now
persists each instance's newest 4096 records in SQLite schema 8. Initial queries
return the newest bounded window in ascending sequence order; cursor queries
return only later entries. Access flows through the existing authenticated IPC
and requires `runtime.read`; endpoint data is absent from the typed response.

The debugger slice follows Leselang's synchronous stackless model. Its typed
projection carries only state, program counter, remaining resource budget,
sanitized logical frames, and an optional pending-effect or fault summary.
`WaitingEffect` must carry exactly one pending effect; `Failed` must carry
exactly one fault; every other state rejects those fields. Continuation tokens,
principals, idempotency keys, capabilities, local values, and absolute scheduler
time are deliberately absent. The `leselang-observe` integration crate validates
a suspended VM `EffectRequest` through the VM's authoritative consistency check
before producing a `WaitingEffect` projection. It rejects torn control-plane
revisions and exposes only the effect kind, optional runtime identity, relative
remaining deadline, execution position, and generated logical frame.
The first mutation contract now plans `DebuggerCancel` through the shared
`CommandPlan` envelope. It requires `debugger.control`, a matching session and
projection revision, and explicit confirmation for non-dry-run execution.
Inspection exposes only command correlation, session, revision, and dry-run
state. Dry-run leaves the VM pending effect untouched; apply invokes the VM's
durable idempotent cancellation path and returns a token-free result. VM journal
schema 6 commits the command audit and requested cancellation in one SQLite
transaction, scopes idempotency to the principal, and preserves the original
audit time across restart-safe replay. Public audit records omit continuation
tokens and idempotency keys; their lifecycle follows bounded continuation
retention through a foreign-key cascade. The waiting debugger document now
declares a session-bound cancel action.
`UiEvent` lowering routes it through the same shared command planner, and both
Rust and .NET validators reject actions rebound outside their enclosing
debugger workspace. Avalonia renders one explicit destructive button and emits
only its stable node ID; confirmation and execution stay in Rust.

## Identity And Bounds

- schema version: `1`
- maximum nodes: `4096`
- maximum depth: `32`
- maximum fallback text: `1024` bytes
- maximum patch operations: `8192`
- maximum encoded document or patch: `2 MiB`
- maximum runtime log entries: `256`
- maximum sanitized log display: `768` bytes
- maximum debugger logical frames: `64`
- maximum debugger display text: `512` bytes
- maximum debugger remaining deadline: `24 hours`
- node IDs: unique, stable, ASCII identifiers up to 128 bytes

Validation rejects duplicate or invalid IDs, control characters, invalid
localization keys, unlabelled actions, over-depth trees, and oversized graphs.
Runtime actions must match the enclosing runtime card, preventing a modified
document from redirecting an event to another runtime. Debugger actions must
likewise match the enclosing workspace's explicit session binding. Serialized IR structs
reject unknown fields rather than silently accepting producer/renderer drift.

## Events

`UiEvent` identifies a node and a typed event kind; it never carries a command
or arbitrary payload. Event planning resolves only actions already declared in
the validated document. The lowering context must fence exactly the document
revision, and both refresh and debugger-cancel actions use the shared
`leselang-command` normalization path.

Unknown nodes, nodes without actions, stale revisions, missing capabilities,
and forged runtime or debugger-session bindings fail closed.

## Patches

`diff` emits deterministic remove, insert, move, and shallow-update operations
between validated documents. Patches carry both source and target revisions and
reject revision regression. New subtrees are inserted as bounded `UiNode`
values. `apply_patch` is the framework-independent reference implementation: it
rejects stale source revisions, missing parents or targets, invalid indexes,
duplicate subtrees, root edits, cyclic moves, and non-shallow updates. Tests
prove `apply_patch(previous, diff(previous, next)) == next` for insertion,
removal, movement, and semantic updates.

The bounded Rust JSON codec is the only cross-language renderer exchange
format. A Rust-generated `previous + patch + next` fixture is consumed by the
.NET renderer core under `apps/leserpent-avalonia`; strict deserialization,
mount, incremental application, runtime binding, and final semantic equality
must all pass with warnings treated as errors.

`diff` is transactional at generation time: removals, insertions, and moves are
applied to a working document as operations are emitted. It returns only after
the working tree exactly equals the target. The bounded-history regression
fixture proves a sliding window remains executable and avoids redundant moves.

The first Avalonia 12 desktop slice maps the validated renderer-core tree to
actual semantic controls. Stable IDs and accessibility metadata become
Automation properties, while action controls emit only their node ID and never
construct commands in .NET. Its platform smoke mode renders the cross-language
fixture through the real control stack. The mounted tree applies all four patch
operations incrementally through a checked stable-ID index. A transactional
semantic candidate validates the final document before visual mutation, and
the compound fixture proves unaffected controls retain object identity. The
fleet root and history section now own bounded viewports backed by active
`VirtualizingStackPanel` instances, avoiding the unbounded outer-scroll layout
that defeats virtualization. Compiled-bound item view models now create direct
virtualized leaves only when their XAML binding enters the realized viewport.
The long-history fixture proves off-screen items remain unconstructed.
Heterogeneous container subtrees are likewise kept as stable-ID renderer models
until realized, while patches against an unrealized parent mutate only that
model. The bounded log document now passes a 48-entry sliding-window fixture;
26 log controls remain unconstructed after first layout. A typed debugger
document now proves `WaitingEffect -> Yielded` re-entry with a 40-frame sliding
window; 18 frame controls remain unconstructed after first layout.
