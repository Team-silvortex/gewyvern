# Leselang UI IR Contract

This document defines the implemented renderer-neutral Gate 4 UI boundary in
`crates/leselang-ui`. Avalonia, web, mobile, persistence, transport, and adapter
types are deliberately outside this contract.

Status: **Gate 4, evolving contract 0.1.0**.

## Pure Flow

```text
QueryResult::RuntimeList -> fleet_document -> UiDocument
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

## Identity And Bounds

- schema version: `1`
- maximum nodes: `4096`
- maximum depth: `32`
- maximum fallback text: `1024` bytes
- maximum patch operations: `8192`
- maximum encoded document or patch: `2 MiB`
- node IDs: unique, stable, ASCII identifiers up to 128 bytes

Validation rejects duplicate or invalid IDs, control characters, invalid
localization keys, unlabelled actions, over-depth trees, and oversized graphs.
Runtime actions must match the enclosing runtime card, preventing a modified
document from redirecting an event to another runtime. Serialized IR structs
reject unknown fields rather than silently accepting producer/renderer drift.

## Events

`UiEvent` identifies a node and a typed event kind; it never carries a command
or arbitrary payload. Event planning resolves only actions already declared in
the validated document. The lowering context must fence exactly the document
revision, and the resulting refresh action uses the shared
`leselang-command` normalization path.

Unknown nodes, nodes without actions, stale revisions, missing capabilities,
and forged runtime bindings fail closed.

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

The first Avalonia 12 desktop slice maps the validated renderer-core tree to
actual semantic controls. Stable IDs and accessibility metadata become
Automation properties, while action controls emit only their node ID and never
construct commands in .NET. Its platform smoke mode renders the cross-language
fixture through the real control stack. The next Gate 4 slices add live
incremental control updates, compiled bindings and virtualization, followed by
separately bounded log and debugger documents.
