# Leselang Language Contract

This reference is the authoritative, model-oriented contract for the currently
implemented Leselang slice. The independent language destination is defined by
the [embedding architecture](leselang-embedding.md); the
[Leserpent 2.0 architecture](leserpent-2-architecture.md) describes its first
product host. Unimplemented roadmap syntax is not part of this contract.

Leselang is an independent embeddable control language in its target design.
Leserpent is the first reference host. Its current profile provides
protocolized GUI and control automation: source lowers to typed effects,
renderer-neutral UI presentation operations, canonical exports, and durable
continuation re-entry. GUI is one host profile, not the language's ceiling;
the future nuis OS / sirius kernel shell is a deferred host profile, not a
current capability. The long-term host shape is a hostable Rust crate with
independently owned VM instances, not a process-global interpreter. By design,
no GUI framework is automatically compatible with this crate.
Each host needs a developer-owned adapter that implements the protocol standard.
Alternatively, a generated framework binding may be emitted by dedicated tooling
from the same schema. The current Rust UI contract exposes `UiAdapterManifest`
as the explicit compatibility proof for either path. Non-Rust integrations
should cross only the protocol or narrow FFI boundary. Host event loops, async
runtimes, threads, widget object models, and hand-written framework shortcuts
stay outside the language contract.

Part of the repository boundary is already executable. The
`leselang-syntax`, `leselang-host-contract`, and `leselang-hir` dependency
closures contain no Leserpent or Gewyvern product crate. The host contract owns
only stable identities, principals, revisions, capabilities, filters, and
bounded effect inputs. `leselang-command` is deliberately a Leserpent adapter;
the current VM, UI, and observe crates still consume product command/result
types and remain the next extraction seam. HIR still enumerates concrete
runtime/UI effects, and the host contract still includes runtime filters and
deployment validation. Zero product dependencies do not yet mean host-neutral
semantics or a fully independent VM. CI checks the frontend dependency closure;
the embedding architecture defines the larger extraction gate.

Status: **Gate 2 execution and syntax contracts stable at 1.0.0**. The current
vertical slice parses, lowers, authorizes, suspends,
serializes, restores, and resumes the read-only
`runtime.list`, `runtime.inspect`, `runtime.history`, and `runtime.logs` effects
plus the idempotent `runtime.refresh`, `runtime.refresh_capabilities`, and
explicitly confirmed `runtime.deploy` and `debugger.cancel` command effects,
plus the frontend-local `ui.activate`, `ui.focus`, `ui.navigate_focus`, `ui.scroll_into_view`,
`ui.assert_visible`, `ui.assert_hidden`, `ui.wait_hidden`, `ui.assert_realized`,
`ui.wait_realized`, `ui.wait_visible`, `ui.assert_focused`, `ui.wait_focused`,
`ui.assert_unfocused`, `ui.wait_unfocused`, `ui.assert_enabled`,
`ui.assert_disabled`, `ui.wait_enabled`, `ui.wait_disabled`,
`ui.open_window`, `ui.close_window`, `ui.assert_window_open`,
`ui.wait_window_open`, `ui.assert_window_closed`, `ui.wait_window_closed`,
`ui.set_selection`, `ui.assert_selection`, `ui.wait_selection`, `ui.assert_child_count`,
`ui.wait_child_count`, `ui.assert_text`, `ui.wait_text`,
`ui.assert_automation_id`, `ui.wait_automation_id`, and
`ui.assert_node_kind`, `ui.wait_node_kind`, `ui.assert_action_kind`,
`ui.wait_action_kind`,
`ui.assert_action_label`, `ui.wait_action_label`,
`ui.assert_action_available`, `ui.wait_action_available`,
`ui.assert_action_unavailable_reason`,
`ui.wait_action_unavailable_reason`, `ui.assert_form_field`,
`ui.assert_form_field_input_kind`, `ui.assert_form_field_required`,
`ui.assert_form_field_max_length`, `ui.assert_form_field_placeholder`,
`ui.wait_form_field`, `ui.wait_form_field_input_kind`,
`ui.wait_form_field_required`, `ui.wait_form_field_max_length`,
`ui.wait_form_field_placeholder`, plus
`ui.assert_accessible_name`, `ui.wait_accessible_name`, and
`ui.assert_accessible_description`, `ui.wait_accessible_description`
presentation effects.

## Canonical Program

```leselang
fn main() = runtime.list(
  environment: "production",
  cluster: none,
  role: "edge"
)
```

`runtime.list` returns a runtime list and requires the `runtime.read`
capability. Each optional filter accepts a string or `none`; empty strings are
normalized to `none` during HIR lowering. Filter strings are limited to 128 UTF-8
bytes and may not contain control characters, matching the wire/recovery contract.

The canonical single-runtime query is:

```leselang
fn main() = runtime.inspect(runtime_id: "runtime-a")
```

`runtime.inspect` requires `runtime.read`, returns exactly one typed runtime
projection, and fails with `RuntimeNotFound` when the identifier is absent. It
does not lower to a filtered list or perform hidden refresh work.

The canonical bounded history query is:

```leselang
fn main() = runtime.history(runtime_id: "runtime-a")
```

`runtime.history` requires `runtime.read` and returns at most 32 applied command
results for one runtime, ordered from newest to oldest revision. It reads stable
domain history rather than exposing persistence rows, daemon logs, or secrets.

The canonical bounded log query is:

```leselang
fn main() = runtime.logs(runtime_id: "runtime-a")
```

`runtime.logs` requires `runtime.read` and returns the newest bounded window of
at most 256 typed log records for exactly one runtime. Its canonical lowering
uses no cursor; incremental polling remains a host/watch responsibility rather
than introducing asynchronous source semantics.

The canonical mutating program is:

```leselang
fn main() = runtime.refresh(runtime_id: "runtime-a")
```

`runtime.refresh` requires `runtime.refresh`, one valid `runtime_id`, and the
expected runtime revision supplied to `Vm::start`. The VM derives stable
`leselang-command-N` and `leselang-effect-N` identifiers from the continuation
sequence and persists the complete `CommandEnvelope` before dispatch. External
effect requests accept only canonical, non-zero decimal continuation sequences;
oversized, non-numeric, padded, or exhausted identities fail during decoding.

Deployment remains a narrow typed operation:

```leselang
fn main() = runtime.deploy(
  runtime_id: "runtime-a",
  pipeline_kind: "http/request",
  target: "pid:42",
)
```

The call requires `runtime.deploy`; its presence is the auditable language-level
confirmation and lowers to `Confirmation::Confirmed`. Pipeline kind and target
are bounded before execution. Principal and idempotency identity come from the
VM host, and only the durable control runtime may materialize the fixed
`gewyvern.deployment.submit` adapter effect.

Debugger cancellation is a separate VM-authority operation:

```leselang
fn main() = debugger.cancel(session_id: "session-a")
```

`debugger.cancel` requires `debugger.control`, a valid session identifier, an
expected debugger revision, and explicit confirmation. It lowers through the
same `CommandPlan` as the renderer-neutral debugger action. The source VM
persists the command before dispatch and resumes from a typed, command-correlated
and token-free cancellation result; the target VM remains the only authority
that may cancel its continuation.

Stable-node activation is the presentation-side equivalent of a native user
click:

```leselang
fn main() = ui.activate(node_id: "runtime-runtime-a-refresh")
```

`ui.activate` requires `ui.presentation`. The VM emits a typed, identity-bound
presentation request and resumes only from the matching activation result;
`leselang-command` rejects it as frontend-local. A renderer must resolve a
realized, visible, effectively enabled semantic action and dispatch exactly one
native activation event through the same route used by a manual click. Missing,
non-action, unrealized, hidden, disabled, or platform-rejected targets fail
closed without invoking the action callback or changing control-plane state.

Stable-node focus remains a distinct, non-activating operation:

```leselang
fn main() = ui.focus(node_id: "runtime-runtime-a-refresh")
```

`ui.focus` requires `ui.presentation`. The VM emits a typed
`PresentationEnvelope` containing the principal, capability set, and validated
node ID; it never converts this effect into a domain query or command.
`leselang-command` rejects presentation effects explicitly. A renderer must
validate the target against its current `UiDocument`, require a focusable
semantic action node, and return a typed result only after native focus succeeds.
Missing, noninteractive, unrealized, or platform-rejected targets fail without
activating the action or changing control-plane state.

Native sequential focus navigation is a distinct typed operation:

```leselang
fn main() = ui.navigate_focus(
  node_id: "runtime-runtime-a-inspect",
  direction: "next",
)
```

`ui.navigate_focus` requires `ui.presentation`, a currently focused, realized
semantic action, and an exact `next`, `previous`, `first`, or `last`
direction. For sequential movement the renderer asks its native focus manager
to traverse from that stable start node; for boundary movement it resolves the
first or last realized action in its stable visual index and then applies native
focus to that control. Its typed result binds the requested start and direction
to the actual stable destination; it does not assume that virtualized platform
tab order is symmetric or that first/last equal source-tree order. Missing,
noninteractive, unrealized, or unfocused starts and rejected navigation fail
without activating any action. The operation has no coordinate, key-event, or
control-plane fallback.

Stable-node scrolling uses the same presentation boundary:

```leselang
fn main() = ui.scroll_into_view(node_id: "runtime-runtime-a")
```

`ui.scroll_into_view` requires `ui.presentation` and accepts any node present in
the current `UiDocument`, including noninteractive headings and containers. The
renderer resolves the stable ID and invokes its native bring-into-view
primitive. Missing or unrealized nodes fail explicitly. Scrolling does not
focus, select, or activate the node.

Native visibility can be asserted without frontend-side guessing:

```leselang
fn main() = ui.assert_visible(node_id: "runtime-runtime-a")
```

`ui.assert_visible` requires `ui.presentation`. The semantic node must exist,
then the renderer must prove that its native control is realized, effectively
visible through its ancestor chain, has nonzero layout bounds, and intersects
the renderer viewport. A hidden, unrealized, missing, or off-viewport target
does not produce a successful presentation result. The assertion does not
focus, scroll, select, or activate the target.

Native hidden state can be asserted as a positive predicate:

```leselang
fn main() = ui.assert_hidden(node_id: "runtime-runtime-a")
```

`ui.assert_hidden` requires `ui.presentation` and any existing semantic node.
The renderer must first resolve the stable node to a realized native control,
then succeeds only when the same viewport-aware visibility predicate used by
`ui.assert_visible` is false. A missing or unrealized target fails separately,
and a still-visible target fails with a typed presentation result. The assertion
does not scroll, focus, select, hide, or activate the target.

Native hidden state can also be awaited without causing it:

```leselang
fn main() = ui.wait_hidden(node_id: "runtime-runtime-a")
```

`ui.wait_hidden` requires `ui.presentation` and any existing semantic node. Its
presentation envelope carries a protocol-fixed 2000 ms deadline and the source
has no duration argument. The frontend adapter yields its dispatcher until the
same viewport-aware native visibility predicate used by `ui.assert_visible` is
false. Missing nodes fail immediately; persistently unrealized or still-visible
controls time out. Waiting never scrolls, focuses, selects, hides, activates, or
forces realization.

Native control realization can be asserted independently of visibility:

```leselang
fn main() = ui.assert_realized(node_id: "runtime-runtime-a")
```

`ui.assert_realized` requires `ui.presentation` and any existing semantic node.
The renderer succeeds only when that stable node currently resolves to a
realized native control. A virtualized, missing, or removed target fails. The
assertion does not force materialization, scroll, focus, select, or activate the
target; it is the side-effect-free predicate used by the synchronous realization
wait.

Native realization can also be awaited without exposing an asynchronous
language model:

```leselang
fn main() = ui.wait_realized(node_id: "runtime-runtime-a")
```

`ui.wait_realized` requires `ui.presentation` and any existing semantic node.
The presentation envelope carries the protocol-fixed 2000 ms deadline; the
source intentionally has no string-encoded duration argument. The frontend
adapter yields its dispatcher between checks and succeeds only if the stable
node naturally resolves to a native control before the deadline. Missing nodes
fail immediately, persistently virtualized nodes time out, and cancellation is
honored by the host. Waiting never scrolls, focuses, selects, activates, or
otherwise forces materialization.

Native visibility can likewise be awaited without implicit scrolling:

```leselang
fn main() = ui.wait_visible(node_id: "runtime-runtime-a")
```

`ui.wait_visible` requires `ui.presentation` and any existing semantic node.
Its presentation envelope carries a protocol-fixed 2000 ms deadline and the
source has no duration argument. The frontend adapter yields its dispatcher
until the realized control is effectively visible, has nonzero bounds, and
intersects the renderer viewport. Missing nodes fail immediately; persistently
unrealized, hidden, zero-size, or off-viewport controls time out. Waiting never
calls the platform bring-into-view primitive and does not focus, select,
activate, or force realization.

Native keyboard focus can be asserted without changing it:

```leselang
fn main() = ui.assert_focused(node_id: "runtime-runtime-a-refresh")
```

`ui.assert_focused` requires `ui.presentation` and a focusable semantic action
node in the current `UiDocument`. The renderer must resolve the realized native
control and return success only when the platform reports that exact control as
focused. Missing, noninteractive, unrealized, or unfocused targets fail. The
assertion never calls the platform focus primitive and does not activate,
scroll, or otherwise mutate the target.

Native keyboard focus can also be awaited without taking it:

```leselang
fn main() = ui.wait_focused(node_id: "runtime-runtime-a-refresh")
```

`ui.wait_focused` requires `ui.presentation` and a focusable semantic action
node. Its presentation envelope carries a protocol-fixed 2000 ms deadline and
the source has no duration argument. Missing or noninteractive nodes fail
immediately; persistently unrealized or unfocused actions time out. The
frontend adapter only observes native focus while yielding its dispatcher and
never calls the platform focus primitive, activates, scrolls, or otherwise
mutates the target.

Native keyboard unfocus can be asserted without moving focus:

```leselang
fn main() = ui.assert_unfocused(node_id: "runtime-runtime-a-refresh")
```

`ui.assert_unfocused` requires `ui.presentation` and a focusable semantic
action node in the current `UiDocument`. The renderer must resolve the realized
native control and return success only when the platform reports that exact
control as not focused. Missing, noninteractive, unrealized, or still-focused
targets fail. The assertion never calls the platform focus primitive and does
not activate, scroll, or otherwise mutate the target.

Native keyboard unfocus can also be awaited without moving focus:

```leselang
fn main() = ui.wait_unfocused(node_id: "runtime-runtime-a-refresh")
```

`ui.wait_unfocused` requires `ui.presentation` and a focusable semantic action
node. Its presentation envelope carries a protocol-fixed 2000 ms deadline and
the source has no duration argument. Missing or noninteractive nodes fail
immediately; persistently unrealized or focused actions time out. The frontend
adapter only observes native focus loss while yielding its dispatcher and never
calls the platform focus primitive, activates, scrolls, or otherwise mutates
the target.

Native action availability can be asserted without activating the action:

```leselang
fn main() = ui.assert_enabled(node_id: "runtime-runtime-a-refresh")
```

`ui.assert_enabled` requires `ui.presentation` and an action node in the
current `UiDocument`. The renderer resolves its realized native control and
returns success only when the platform reports it effectively enabled,
including ancestor state. Missing, noninteractive, unrealized, or disabled
targets fail. The assertion never focuses, activates, or changes availability.

Native disabled state has its own positive assertion:

```leselang
fn main() = ui.assert_disabled(node_id: "runtime-runtime-a-refresh")
```

`ui.assert_disabled` requires `ui.presentation` and an action node in the
current `UiDocument`. The renderer resolves its realized native control and
returns success only when the platform reports it effectively disabled,
including ancestor state. Missing, noninteractive, unrealized, or still-enabled
targets fail. The assertion never focuses, activates, scrolls, enables,
disables, or submits the target.

Native action availability can also be awaited without changing it:

```leselang
fn main() = ui.wait_enabled(node_id: "runtime-runtime-a-refresh")
```

`ui.wait_enabled` requires `ui.presentation` and a semantic action node. Its
presentation envelope carries a protocol-fixed 2000 ms deadline and the source
has no duration argument. The frontend adapter yields its dispatcher until the
realized native control becomes effectively enabled, including ancestor state.
Missing or noninteractive nodes fail immediately; persistently unrealized or
disabled actions time out. Waiting never enables, focuses, activates, scrolls,
or otherwise mutates the target.

Native disabled action state can also be awaited without causing it:

```leselang
fn main() = ui.wait_disabled(node_id: "runtime-runtime-a-refresh")
```

`ui.wait_disabled` requires `ui.presentation` and a semantic action node. Its
presentation envelope carries the same protocol-fixed 2000 ms deadline as
enabled wait, and the source has no duration argument. The frontend adapter
yields its dispatcher until the realized native control becomes effectively
disabled, including ancestor state. Missing or noninteractive nodes fail
immediately; persistently unrealized or still-enabled actions time out. Waiting
never disables, enables, focuses, activates, scrolls, or otherwise mutates the
target.

Native window lifetime is controlled by two separate presentation mutations:

```leselang
fn main() = ui.open_window(node_id: "runtime-runtime-a")
fn main() = ui.close_window(node_id: "runtime-runtime-a")
```

Both require `ui.presentation` and any existing semantic node. `ui.open_window`
is idempotent for an already attached target; a renderer may create a native
window only for a fully detached surface and must not reparent an existing
surface. `ui.close_window` closes only the native window containing the target
and is idempotent when the target is already detached. Opening does not activate
or focus the window. Both effects return only after the adapter accepts the
lifecycle mutation; callers use the independent assertions below to observe
native state.

Native window attachment can be asserted without activating a window:

```leselang
fn main() = ui.assert_window_open(node_id: "runtime-runtime-a")
```

`ui.assert_window_open` requires `ui.presentation` and any existing semantic
node. The renderer resolves the stable node to a realized native control and
succeeds only when that control and the renderer surface belong to the same
native window visual tree. Missing or unrealized targets fail. The assertion
never opens, closes, activates, focuses, scrolls, selects, or submits anything.

The same native window attachment can be waited for with a protocol-fixed
deadline:

```leselang
fn main() = ui.wait_window_open(node_id: "runtime-runtime-a")
```

`ui.wait_window_open` uses the same target semantics as
`ui.assert_window_open`, but waits up to the fixed 2000 ms window-open deadline
for the target control and renderer surface to share a native window visual
tree. It never opens, closes, activates, focuses, scrolls, selects, or submits
anything.

Native window detachment can be asserted without closing a window:

```leselang
fn main() = ui.assert_window_closed(node_id: "runtime-runtime-a")
```

`ui.assert_window_closed` requires `ui.presentation` and any existing semantic
node. The renderer first resolves the stable node to a realized native control,
then succeeds only when that control is not in the same native window visual
tree as the renderer surface. A missing semantic node fails as unknown, an
unrealized semantic node fails as unrealized, and a target still attached to the
renderer window fails as still open. The assertion never opens, closes,
activates, focuses, scrolls, selects, or submits anything.

Native window detachment can also be awaited without causing it:

```leselang
fn main() = ui.wait_window_closed(node_id: "runtime-runtime-a")
```

`ui.wait_window_closed` uses the same target semantics as
`ui.assert_window_closed`, but waits up to the fixed 2000 ms window-closed
deadline for the target control and renderer surface to stop sharing a native
window visual tree. Detached renderer surfaces satisfy the predicate; a
persistently open target times out and remains open. Waiting never calls a
native close API and never activates, focuses, scrolls, selects, or submits
anything.

Native selection can be changed through one renderer-neutral mutation:

```leselang
fn main() = ui.set_selection(
  node_id: "runtime-runtime-b",
  state: "selected",
)
```

`ui.set_selection` requires `ui.presentation`, a semantic node that declares
selection metadata, and exactly `selected` or `unselected`. The VM binds both
arguments to the suspended request and typed result, so a renderer cannot
acknowledge a different target or state. Adapters mutate native selection
directly; repeated requests are idempotent, selecting and unselecting are
reversible, and the operation must not focus, activate, scroll, or submit the
target. Use `ui.assert_selection` or `ui.wait_selection` for independent
postcondition checks.

Native selection state can be asserted without activating or focusing a control:

```leselang
fn main() = ui.assert_selection(
  node_id: "runtime-runtime-a",
  state: "selected",
)
```

`ui.assert_selection` requires `ui.presentation`, a semantic node that declares
selection metadata, and one of the exact states `selected` or `unselected`. The
renderer resolves the stable ID and reads its native selected state rather than
trusting the semantic default. Missing, selectionless, unrealized, nonselectable,
or mismatched targets fail with typed presentation errors. The assertion never
focuses, activates, scrolls, or changes selection.

Native selection state can also be awaited without changing it:

```leselang
fn main() = ui.wait_selection(
  node_id: "runtime-runtime-b",
  state: "unselected",
)
```

`ui.wait_selection` carries the protocol-fixed 2000 ms deadline and the source
has no duration argument. The VM binds the node and selection state to the
request and result across re-entry. The frontend adapter yields its dispatcher
until the realized native selectable reaches the requested state. Missing or
selectionless nodes fail immediately; persistently unrealized, nonselectable, or
mismatched controls time out. Waiting never selects, focuses, activates, scrolls,
or otherwise mutates the target.

Stable immediate-child cardinality can be asserted without realizing a list:

```leselang
fn main() = ui.assert_child_count(node_id: "fleet-root", count: "3")
```

`ui.assert_child_count` accepts any existing semantic node and a canonical
decimal count from `0` through `4096`. The adapter reads the node's stable
semantic/visual index, including unrealized virtualized children, rather than
enumerating only materialized native controls. Missing, unrealized target, or
mismatched counts fail with typed presentation errors; observation never
focuses, scrolls, activates, or realizes children.

The same topology state can be awaited across an external patch:

```leselang
fn main() = ui.wait_child_count(node_id: "fleet-root", count: "4")
```

`ui.wait_child_count` has the protocol-fixed 2000 ms deadline and no source
duration argument. The VM binds node, count, timeout, and result across re-entry.
The frontend yields its dispatcher until a patch changes the indexed direct-child
count; a persistent mismatch times out without mutating the document or its
virtualization state.

Native displayed text can be asserted without OCR or coordinate inspection:

```leselang
fn main() = ui.assert_text(
  node_id: "fleet-title",
  expected: "Runtime fleet"
)
```

`ui.assert_text` requires `ui.presentation`, a text-rendering semantic node,
and an expected value of at most 1024 UTF-8 bytes with no control characters.
The VM binds the expected value to the request and result, while the renderer
reads the realized native `TextBlock.Text` or string `Button.Content` and
requires an exact ordinal match. Missing, textless, unrealized, or mismatched
targets fail. The assertion never focuses, activates, scrolls, or changes text.

Native displayed text can also be awaited as a synchronous language effect:

```leselang
fn main() = ui.wait_text(
  node_id: "fleet-title",
  expected: "Runtime fleet ready"
)
```

`ui.wait_text` requires `ui.presentation`, the same text-rendering semantic node
contract as `ui.assert_text`, and the same bounded expected display text. Its
presentation envelope carries a protocol-fixed 2000 ms deadline; source has no
duration argument. The frontend adapter yields its dispatcher until native
displayed text exactly matches the expected value. Missing or textless nodes fail
immediately, persistent mismatches time out, and waiting never focuses,
activates, scrolls, types, or changes text.

Native automation identity can be asserted independently of display text:

```leselang
fn main() = ui.assert_automation_id(
  node_id: "fleet-title",
  expected: "fleet-title"
)
```

`ui.assert_automation_id` requires `ui.presentation`, any existing semantic
node, and an expected value that is itself a valid UI node identifier. The VM
binds the expected automation ID to the request and result, while the renderer
reads the realized platform automation ID and requires an exact ordinal match.
Missing, unrealized, invalid-expected, or mismatched targets fail. The
assertion never focuses, activates, scrolls, or changes automation metadata.

Automation identity can also be awaited across an external native transition:

```leselang
fn main() = ui.wait_automation_id(
  node_id: "fleet-title",
  expected: "fleet-title"
)
```

`ui.wait_automation_id` uses the same capability, target, and identifier
validation as the assertion. Its typed presentation envelope carries a
protocol-fixed 2000 ms deadline and binds both identifiers across VM re-entry.
The adapter polls the realized platform automation property until it matches
exactly; a persistent mismatch times out. Waiting never realizes, focuses,
scrolls, activates, or rewrites the target.

Native semantic node kind can be asserted before model-driven automation:

```leselang
fn main() = ui.assert_node_kind(
  node_id: "fleet-title",
  kind: "heading"
)
```

`ui.assert_node_kind` requires `ui.presentation`, any existing semantic node,
and a bounded semantic kind. The accepted values are `column`, `heading`,
`text`, `runtime_card`, `runtime_workspace`, `section`, `history_entry`,
`log_entry`, `debugger_workspace`, `debugger_frame`, and `action`. The VM binds
the expected kind to the request and result, while the renderer compares it
with the stable semantic renderer kind for the realized node. Missing,
unrealized, invalid-kind, or mismatched targets fail. The assertion never
focuses, activates, scrolls, or changes semantic metadata.

The same semantic node kind can be awaited while a renderer-neutral projection
or localization package changes the mounted tree:

```leselang
fn main() = ui.wait_node_kind(
  node_id: "fleet-title",
  kind: "heading"
)
```

`ui.wait_node_kind` requires `ui.presentation`, any existing semantic node, and
the same bounded semantic kind set as `ui.assert_node_kind`. Its presentation
envelope carries a protocol-fixed 2000 ms deadline; source has no duration
argument. The frontend adapter yields its dispatcher until the stable semantic
renderer kind matches. Missing nodes fail immediately, persistent mismatches or
unrealized targets time out, and waiting never focuses, activates, scrolls,
realizes, or changes semantic metadata.

Native semantic action kind can be asserted before activation:

```leselang
fn main() = ui.assert_action_kind(
  node_id: "runtime-runtime-a-refresh",
  kind: "runtime_refresh"
)
```

`ui.assert_action_kind` requires `ui.presentation` and a semantic action node.
The accepted values are `runtime_inspect`, `runtime_refresh`,
`runtime_capabilities_refresh`, `runtime_deploy`, and `debugger_cancel`. The VM
binds the expected action kind to the request and result, while the renderer
compares it with the stable semantic action payload for the realized node.
Missing, actionless, unrealized, invalid-kind, or mismatched targets fail. The
assertion never focuses, activates, scrolls, submits a form, or changes action
metadata.

The same semantic action kind can be awaited before model-driven activation:

```leselang
fn main() = ui.wait_action_kind(
  node_id: "runtime-runtime-a-refresh",
  kind: "runtime_refresh"
)
```

`ui.wait_action_kind` requires `ui.presentation` and a semantic action node.
It accepts the same bounded action kind set as `ui.assert_action_kind`, carries
a protocol-fixed 2000 ms deadline, and has no source duration argument. The
frontend adapter yields its dispatcher until the stable semantic action payload
matches the expected kind. Missing, actionless, invalid-kind, or persistently
mismatched targets fail or time out. Waiting never focuses, activates, clicks,
scrolls, submits a form, enables the action, or changes action metadata.

Native semantic action labels can be asserted before activation:

```leselang
fn main() = ui.assert_action_label(
  node_id: "runtime-runtime-a-refresh",
  expected: "Refresh runtime"
)
```

`ui.assert_action_label` requires `ui.presentation`, a semantic action node,
and a bounded expected string. It reads the renderer's explicit semantic action
label as exposed through native automation name, not fallback OCR or button
layout text. The VM binds node and expected label to the request and result,
while the renderer compares exact ordinal text for the realized action. Missing,
actionless, unrealized, unlabelled, invalid expected text, or mismatched targets
fail. The assertion never focuses, activates, clicks, scrolls, submits a form,
enables the action, or changes label metadata.

The same semantic action label can be awaited while an external projection or
localization package changes the action metadata:

```leselang
fn main() = ui.wait_action_label(
  node_id: "runtime-runtime-a-refresh",
  expected: "Refresh runtime"
)
```

`ui.wait_action_label` requires `ui.presentation` and the same semantic action
node. Its presentation envelope carries a protocol-fixed 2000 ms deadline;
source has no duration argument. The frontend adapter yields its dispatcher
until the explicit semantic action label matches the expected native automation
name. Missing or actionless nodes fail immediately, persistent label mismatch
times out, and waiting never focuses, activates, clicks, scrolls, submits a
form, enables the action, or rewrites the label.

Semantic action availability can be asserted before activation:

```leselang
fn main() = ui.assert_action_available(
  node_id: "runtime-runtime-a-refresh"
)
```

`ui.assert_action_available` requires `ui.presentation` and a semantic action
node. It reads renderer-maintained semantic action availability, not native
effective-enabled state, and succeeds only when the action has no unavailable
reason. Missing, actionless, unrealized, or currently unavailable targets fail.
The assertion never focuses, activates, clicks, scrolls, submits a form, enables
the action, or rewrites the unavailable reason.

The same semantic action availability can be awaited while a daemon, topology
poll, or local deployment policy restores an action:

```leselang
fn main() = ui.wait_action_available(
  node_id: "runtime-runtime-a-refresh"
)
```

`ui.wait_action_available` requires `ui.presentation` and a semantic action
node. Its presentation envelope carries a protocol-fixed 2000 ms deadline;
source has no duration argument. The frontend adapter yields its dispatcher
until renderer-maintained action availability becomes true. Missing or
actionless nodes fail immediately, persistent unavailability times out, and
waiting never focuses, activates, clicks, scrolls, submits a form, enables the
action, or rewrites the unavailable reason.

Native action unavailable reasons can be asserted before retrying deployment or
refresh controls:

```leselang
fn main() = ui.assert_action_unavailable_reason(
  node_id: "runtime-runtime-a-refresh",
  expected: "Verification action is temporarily unavailable"
)
```

`ui.assert_action_unavailable_reason` requires `ui.presentation` and a semantic
action node. The `expected` parameter accepts bounded display text or `none`;
`none` means the action must currently have no unavailable reason. The VM binds
`node_id` and optional `expected` to the request and result. The renderer
compares the stable action availability reason configured for that semantic
action or its absence exactly; actionless targets, unrealized targets, invalid
reason text, or mismatched reasons fail. The assertion never focuses, activates,
scrolls, submits a form, or changes action availability.

The same action availability reason can be awaited while a daemon, topology
poll, or local deployment policy updates native control state:

```leselang
fn main() = ui.wait_action_unavailable_reason(
  node_id: "runtime-runtime-a-refresh",
  expected: none
)
```

`ui.wait_action_unavailable_reason` requires `ui.presentation` and a semantic
action node. Its presentation envelope carries a protocol-fixed 2000 ms
deadline; source has no duration argument. The `expected` parameter has the same
bounded text-or-`none` contract as the assertion. The frontend adapter yields its
dispatcher until the renderer-maintained action unavailable reason exactly
matches the expected value or absence. Missing or actionless nodes fail
immediately, persistent mismatches time out, and waiting never focuses,
activates, scrolls, submits a form, or rewrites action availability.

Native deployment form metadata can be asserted without submitting the form:

```leselang
fn main() = ui.assert_form_field(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "Pipeline kind"
)
```

`ui.assert_form_field` requires `ui.presentation`, a semantic
`runtime_deploy` action with a bounded form, a form field key of at most 128
ASCII bytes (`A-Z`, `a-z`, `0-9`, `_`, `-`, or `.`), and the same bounded
control-free expected-text contract used by text and accessibility assertions.
The VM binds `node_id`, `field`, and `expected` to the request and result. The
renderer compares the stable semantic field label fallback with the expected
value; form-less targets, unknown fields, unrealized targets, invalid keys, or
mismatched labels fail. The assertion never focuses, types, activates, opens, or
submits the form.

The same form label metadata can be waited on when a remote schema refresh
changes the semantic form externally:

```leselang
fn main() = ui.wait_form_field(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "Pipeline kind"
)
```

`ui.wait_form_field` accepts the same bounded `node_id`, `field`, and
`expected` parameters as `ui.assert_form_field`. The VM fixes the wait deadline
at 2000 ms while binding `node_id`, `field`, `expected`, and timeout across
request and result. The renderer polls the stable semantic form field label
fallback until it matches exactly; form-less targets, unknown fields,
unrealized targets, invalid keys, invalid expected text, forged timeouts, or
persistent mismatches fail. The wait observes form schema metadata only; it
never reads current input values, focuses, types, activates, opens, edits, or
submits the form.

Native deployment form input semantics can also be asserted without submitting
the form:

```leselang
fn main() = ui.assert_form_field_input_kind(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  kind: "path_token"
)
```

`ui.assert_form_field_input_kind` requires `ui.presentation`, the same semantic
`runtime_deploy` form action and bounded field key as `ui.assert_form_field`,
and a typed `kind` of either `path_token` or `trimmed_text`. The VM binds
`node_id`, `field`, and `kind` to the request and result. The renderer compares
the stable semantic field input kind with the expected kind; form-less targets,
unknown fields, unrealized targets, invalid keys, missing kinds, or mismatched
input kinds fail. The assertion never focuses, types, activates, opens, or
submits the form.

Native deployment form input semantics can also be waited on:

```leselang
fn main() = ui.wait_form_field_input_kind(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  kind: "path_token"
)
```

`ui.wait_form_field_input_kind` accepts the same bounded `node_id`, `field`,
and typed `kind` parameters as `ui.assert_form_field_input_kind`. The VM fixes
the deadline at 2000 ms and binds the typed input kind across request and
result. The renderer polls the stable semantic form field input kind until it
matches exactly; form-less targets, unknown fields, unrealized targets, invalid
keys, missing or invalid kinds, forged timeouts, or persistent mismatches fail.
The wait observes schema metadata only and never submits or edits the form.

Native deployment form required-state metadata can also be asserted without
touching the form:

```leselang
fn main() = ui.assert_form_field_required(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  state: "required"
)
```

`ui.assert_form_field_required` requires `ui.presentation`, the same semantic
`runtime_deploy` form action and bounded field key as `ui.assert_form_field`,
and a typed `state` of either `required` or `optional`. The VM binds `node_id`,
`field`, and `state` to the request and result. The UI IR and renderer exchange
this as a boolean `required` bit, but Leselang source keeps the explicit enum to
avoid generic boolean literals. The renderer compares the stable semantic form
field required metadata with the expected state; form-less targets, unknown
fields, unrealized targets, invalid keys, missing states, or mismatched required
state fail. The assertion never focuses, types, activates, opens, marks, or
submits the form.

Native deployment form required-state metadata can also be waited on:

```leselang
fn main() = ui.wait_form_field_required(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  state: "required"
)
```

`ui.wait_form_field_required` accepts the same bounded `node_id`, `field`, and
typed `state` parameters as `ui.assert_form_field_required`. The VM fixes the
deadline at 2000 ms and binds the explicit required/optional state across
request and result. The renderer polls the stable semantic required bit until
it matches exactly; form-less targets, unknown fields, unrealized targets,
invalid keys, missing or invalid states, forged timeouts, or persistent
mismatches fail. The wait observes schema metadata only and never changes the
form.

Native deployment form maximum-length metadata can also be asserted without
touching the form:

```leselang
fn main() = ui.assert_form_field_max_length(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  max_length: "128"
)
```

`ui.assert_form_field_max_length` requires `ui.presentation`, the same semantic
`runtime_deploy` form action and bounded field key as `ui.assert_form_field`.
The `max_length` parameter is a decimal string from `1` to `256` with no leading
zeroes. HIR parses it to a typed integer, and UI IR plus renderer JSON carry it
as `max_length`. The renderer compares the stable semantic form field maximum
length with the expected value; form-less targets, unknown fields, unrealized
targets, invalid keys, missing lengths, malformed lengths, or mismatched limits
fail. The assertion never focuses, types, activates, opens, edits, or submits
the form.

Native deployment form maximum-length metadata can also be waited on:

```leselang
fn main() = ui.wait_form_field_max_length(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  max_length: "128"
)
```

`ui.wait_form_field_max_length` accepts the same bounded `node_id`, `field`,
and decimal `max_length` string as `ui.assert_form_field_max_length`. The VM
fixes the deadline at 2000 ms and binds the typed maximum length across request
and result. The renderer polls the stable semantic maximum length until it
matches exactly; form-less targets, unknown fields, unrealized targets, invalid
keys, malformed lengths, forged timeouts, or persistent mismatches fail. The
wait observes schema metadata only and never edits or submits the form.

Native deployment form placeholder metadata can also be asserted without
touching the form:

```leselang
fn main() = ui.assert_form_field_placeholder(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "http/request"
)
```

`ui.assert_form_field_placeholder` requires `ui.presentation`, the same semantic
`runtime_deploy` form action and bounded field key as `ui.assert_form_field`.
The `expected` parameter accepts bounded display text or `none`; `none` means the
field must have no semantic placeholder fallback. The VM binds `node_id`,
`field`, and optional `expected` to the request and result. The renderer compares
the stable semantic form field placeholder fallback or absence exactly; form-less
targets, unknown fields, unrealized targets, invalid keys, invalid placeholder
text, or mismatched placeholders fail. The assertion never focuses, types,
activates, opens, edits, or submits the form.

Native deployment form placeholder metadata can also be waited on when a
deployment template or remote schema refresh changes the form externally:

```leselang
fn main() = ui.wait_form_field_placeholder(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "http/request"
)
```

`ui.wait_form_field_placeholder` requires `ui.presentation`, the same semantic
`runtime_deploy` form action and bounded field key as
`ui.assert_form_field_placeholder`. The `expected` parameter accepts bounded
display text or `none`, and the VM fixes the wait deadline at 2000 ms while
binding `node_id`, `field`, optional `expected`, and timeout across request and
result. The renderer polls the stable semantic form field placeholder fallback
or absence until it matches exactly; form-less targets, unknown fields,
unrealized targets, invalid keys, invalid placeholder text, forged timeouts, or
persistent mismatches fail. The wait never focuses, types, activates, opens,
edits, or submits the form.

Native deployment form values use three separate scoped operations:

```leselang
fn main() = ui.set_form_value(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  value: "http/request"
)

fn main() = ui.assert_form_value(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "http/request"
)

fn main() = ui.wait_form_value(
  node_id: "runtime-runtime-a-deploy",
  field: "pipeline_kind",
  expected: "http/request"
)
```

All three require `ui.presentation`, a semantic `runtime_deploy` form action,
and a declared bounded field key. Values and expectations are control-free and
at most 256 UTF-8 bytes. `ui.set_form_value` additionally enforces the field's
required bit, maximum length, and `path_token` or `trimmed_text` input kind;
assert/wait may observe temporarily invalid text exactly as the user sees it.
The VM binds node, field, value/expected, and the fixed 2000 ms wait deadline
across re-entry. A renderer must target the currently open native field through
a scoped registration, reject unopened or disposed scopes, and never focus,
activate, or submit as a side effect. Repeating the same set is idempotent.

An open native deployment form can be submitted or cancelled through two
distinct lifecycle mutations:

```leselang
fn main() = ui.submit_form(node_id: "runtime-runtime-a-deploy")

fn main() = ui.cancel_form(node_id: "runtime-runtime-a-deploy")
```

Both operations require `ui.presentation` and a semantic `runtime_deploy`
action with a parameterized form. The renderer must resolve the action's
currently registered native form window and raise exactly one click on its real
Submit or Cancel button. `ui.submit_form` therefore follows the same field
validation, revision fence, confirmation, and deployment path as a manual
submission; `ui.cancel_form` follows the same native cancellation path and
returns no form values. Neither operation may lower directly into a domain
command or invoke a semantic action callback. Unopened, unrealized, disabled,
disposed, mismatched, or already-closed form scopes fail closed, so replay after
the native window closes cannot submit or cancel twice.

Native accessibility metadata can be asserted independently of display text:

```leselang
fn main() = ui.assert_accessible_name(
  node_id: "fleet-title",
  expected: "Runtime fleet"
)
```

`ui.assert_accessible_name` requires `ui.presentation`, any existing semantic
node, and the same bounded control-free expected-text contract. The renderer
must read the realized platform accessibility name and require an exact ordinal
match. It does not infer success from semantic text, focus, activate, scroll, or
change accessibility metadata.

Accessibility names can also be waited for when the native surface updates after
the semantic document is mounted:

```leselang
fn main() = ui.wait_accessible_name(
  node_id: "fleet-title",
  expected: "Runtime fleet"
)
```

`ui.wait_accessible_name` requires `ui.presentation`, any existing semantic
node, and the same bounded control-free expected-text contract. It carries the
protocol-fixed 2000 ms deadline. The renderer polls the realized platform
accessibility name and completes only on an exact ordinal match; unknown,
unrealized, or persistently mismatched targets time out. It does not infer
success from text, focus, activate, scroll, or mutate accessibility metadata.

Explicit accessibility descriptions use the same bounded exact-match model:

```leselang
fn main() = ui.assert_accessible_description(
  node_id: "runtime-runtime-a-inspect",
  expected: "Open the read-only runtime workspace"
)
```

`ui.assert_accessible_description` requires `ui.presentation` and a semantic
node that explicitly declares `accessibility.description`. The renderer reads
the realized platform help text and requires an exact ordinal match; a
descriptionless, unrealized, or mismatched target fails without focus,
activation, scrolling, or metadata mutation.

Explicit accessibility descriptions can also be waited for when native HelpText
changes after the control has been realized:

```leselang
fn main() = ui.wait_accessible_description(
  node_id: "runtime-runtime-a-inspect",
  expected: "Open the read-only runtime workspace"
)
```

`ui.wait_accessible_description` requires `ui.presentation` and a semantic node
that explicitly declares `accessibility.description`. It carries the
protocol-fixed 2000 ms deadline. The renderer polls realized platform help text
and completes only on an exact ordinal match; descriptionless, unrealized, or
persistently mismatched targets time out or fail without focus, activation,
scrolling, or metadata mutation.

Every atomic HIR effect has one Rust-owned canonical source representation.
Canonicalization rejects effect trees at 16 nesting levels or above and caps
the traversed graph at 16,384 nodes before recursive formatting, so forged HIR
cannot turn source normalization into stack or unbounded-work exhaustion.
Parsing and lowering that source must reproduce the same effect. GUI event
export uses this printer instead of maintaining a frontend-specific language
template.

## Grammar

This is the implemented grammar, not a template for copying another language.
New constructs follow the [agent-first syntax rules](leselang-embedding.md#agent-first-syntax):
regular typed composition, explicit effects, repairable diagnostics and safe
re-entry take precedence over Bash/Lua/JavaScript syntax compatibility.
Unimplemented design examples must not be treated as accepted source.

```ebnf
program       = function, { function }, EOF ;
function      = "fn", identifier, "(", [ parameters ], ")", "=", expression ;
parameters    = parameter, { ",", parameter }, [ "," ] ;
parameter     = identifier, ":", scalar-type ;
scalar-type   = "integer" | "boolean" | "string" | "none" | "optional_string" | "string_list" ;
expression    = string | integer | boolean | "none" | identifier | call ;
call          = effect-call | all-call | seq-call | repeat-call
              | bind-call | loop-call | fold-call | strings-call | choose-call | recover-call | binary-call | unary-call | field-call | member-call | helper-call ;
helper-call   = identifier, "(", [ arguments ], ")" ;
effect-call   = identifier, ".", identifier, "(", [ arguments ], ")" ;
all-call      = "all", "(", branch, ",", branch, { ",", branch }, [ "," ], ")" ;
seq-call      = "seq", "(", branch, { ",", branch }, [ "," ], ")" ;
repeat-call   = "repeat", "(", "times", ":", integer, ",", "body", ":", call, [ "," ], ")" ;
branch        = identifier, ":", call ;
arguments     = argument, { ",", argument }, [ "," ] ;
argument      = identifier, ":", value ;
value         = expression ;
bind-call     = "bind", "(", identifier, ":", expression, ",", "body", ":", expression, [ "," ], ")" ;
loop-call     = "loop", "(", identifier, ":", expression, ",", "while", ":", expression, ",", "next", ":", expression, ",", "limit", ":", integer, [ "," ], ")" ;
fold-call     = "fold", "(", identifier, ":", expression, ",", "items", ":", expression, ",", "item", ":", string, ",", "next", ":", expression, ",", "limit", ":", integer, [ "," ], ")" ;
strings-call  = "strings", "(", [ arguments ], ")" ;
choose-call   = "choose", "(", "when", ":", expression, ",", "then", ":", expression, ",", "otherwise", ":", expression, [ "," ], ")" ;
recover-call  = "recover", "(", "value", ":", expression, ",", "fallback", ":", expression, [ "," ], ")" ;
binary-call   = binary-op, "(", "left", ":", expression, ",", "right", ":", expression, [ "," ], ")" ;
unary-call    = unary-op, "(", "value", ":", expression, [ "," ], ")" ;
unary-op      = "not" | "len" | "to_string" | "parse_integer" | "parse_boolean" | "optional_string" | "has_value" ;
field-call    = "field", "(", "value", ":", expression, ",", "name", ":", string, [ "," ], ")" ;
member-call   = "member", "(", "value", ":", identifier, ",", "name", ":", string, [ "," ], ")" ;
binary-op     = "add" | "sub" | "mul" | "div" | "rem" | "eq" | "ne"
              | "lt" | "le" | "gt" | "ge" | "and" | "or" | "concat" | "value_or"
              | "contains" | "starts_with" | "ends_with" | "char_at"
              | "split" | "join" | "append" | "item_at" ;
boolean       = "true" | "false" ;
integer       = "0" | nonzero-digit, { digit } ;
identifier    = ( letter | "_" ), { letter | digit | "_" } ;
string        = '"', { character | escape }, '"' ;
escape        = "\\", ( '"' | "\\" | "n" | "r" | "t" ) ;
```

Whitespace and `//` line comments are retained as lossless tokens, including
their byte spans. Reassembling token text must reproduce the original source.
Source is UTF-8 and limited to 256 KiB.

Integer literals are canonical unsigned 64-bit decimals; signs, leading zeroes,
fractions, alternate bases, and overflow are rejected. Computation accepts the
full `u64` range, while `repeat.times` remains an integer literal from 1 through
64, `loop.limit` from 0 through 1024 and `fold.limit` from 0 through 64. The grammar shows canonical argument order; named arguments may be reordered
without changing evaluation order, except that `strings` entries are positional
and follow written source order, not label order. `true` and `false` are literals in value
positions and cannot name locals; legacy single-function and named-step labels may
still use these words, but new helper names and parameters cannot. `none` and `fn`
retain their existing reserved status outside parameter type annotations.
Bare value identifiers resolve only to lexical bindings, including the current
loop-local scalar state and fold-local accumulator/item. Enclosing bindings remain immutable. See
[control flow](leselang-control-flow.md) for typed calculation, `bind`, lazy
`choose`, short-circuit logic, limits, and remaining language gaps. Atomic host
operations accept pure expressions and locals as arguments, retaining their
existing string/`none` types and domain limits. This also works inside bounded
`seq`/`repeat` and flat `all`: every selected member is fully prepared before
any effect starts. See [computed groups](leselang-control-flow.md#computed-groups)
for whole-group failure, scope, expansion and recovery rules.
One atomic host result can also be bound and projected with a type-checked
`field(value: result, name: "...")`, followed by a pure scalar body or another atomic capture. See
[result bindings](leselang-control-flow.md#result-bindings) for the exact field
table and durable local environment. Pure bodies use schema 2, an uncaptured tail
uses schema 3, and [bounded result chains](leselang-control-flow.md#durable-result-chains)
use schema 4 with typed projection frames across suspensions. Mixed scalar
return/continue branches use [schema 5](leselang-control-flow.md#conditional-exits).
Named `seq`/`repeat`/flat `all` results can be bound for a pure scalar body using
`member(value: group, name: "step")` and the existing typed `field` projections.
[Group-result bindings](leselang-control-flow.md#named-group-result-bindings)
use schema 6 and require complete journal recovery; isolated member restoration
is rejected. Sequential groups can instead drive
[one atomic tail](leselang-control-flow.md#sequential-group-tails) using schema 7:
at most 63 prefix members, then one result-dependent operation. Its final prefix
acknowledgement and concrete tail request commit together with original authority
and shared budgets. [Schema 8](leselang-control-flow.md#captured-sequential-successors)
allows that successor to be captured for a pure scalar body, preserving bounded
typed member projections across the suspension. Parallel `all` prefixes support
the same one-tail/one-capture forms using
[schema 9](leselang-control-flow.md#parallel-group-successors): 2 to 63 independent
members, then an all-success barrier and one atomic successor. The Rust batch API
supports this; native multi-request debugger presentation remains pending.
[Schema 10](leselang-control-flow.md#group-result-chains) carries group/result
frames across multiple captured successors, ending in a pure scalar. Prefix plus
longest cold chain fits in 64 slots; each receipt and next request commit together
under the original authority/shared budgets. Sequential chains use the native
debugger's existing single-presentation channel.
[Schema 11](leselang-control-flow.md#group-conditional-exits) permits same-type
scalar exits before the first group successor or between captures. A lazy branch
can stop or continue, but cold paths still reserve the longest graph and all
capabilities. The prefix all-success barrier, transactional raw receipts, shared
budgets and whole-journal recovery remain unchanged. Result-dependent parameters
inside the prepared prefix remain unsupported.
The [bounded pure loop contract](leselang-control-flow.md#bounded-pure-loops)
defines condition-first exit, typed next-state updates, explicit limit faults,
and shared fuel; loops can run before suspension or in a captured result's pure body.
The [explicit scalar conversions](leselang-control-flow.md#explicit-scalar-conversions)
connect typed calculations to string-valued host parameters without implicit
coercion. `to_string` accepts integers, booleans and strings, never `none` or raw
host objects. `parse_integer` accepts non-empty ASCII decimal text within `u64`
(including leading zeros in text, not source literals); `parse_boolean` accepts
exactly `"true"` or `"false"`. Invalid text faults with `LSV1408` without echoing
the input. String bounds, shared fuel and host-argument validation still apply.
The [pure calculation recovery contract](leselang-control-flow.md#pure-calculation-recovery)
adds `recover(value: ..., fallback: ...)` for same-type pure scalars. Only local
arithmetic (`LSV1401`) and text parsing (`LSV1408`) failures select the lazy
fallback; resource limits, authorization, deadlines, cancellation and host errors
are not intercepted. Failed work consumes the shared fuel and is never refunded.
Both sides remain type-checked; this is not effectful retry, rollback or cleanup.

[Text GUI projections](leselang-control-flow.md#text-gui-projections) expose the
acknowledged `expected` text, form `field` key and submitted `value` for a closed
operation table. They are strings for helpers, conditions and subsequent arguments,
not live GUI-property queries or nullable/enum coercions. New exporting captures
use projection vocabulary v3, except form-input-kind results, which also export
`kind` and use v4; old v1/v2/v3 frames retain their exact fields and bytes.
The normal 256-byte form-value, 128-byte field-key and 1024-byte text domains,
shared fuel, raw-output limits and 64 KiB image/plan bound still apply. Continuation
schemas 1-11 and journal schema 10 need no migration.

[Kind GUI projections](leselang-control-flow.md#kind-gui-projections) expose `kind`
as the acknowledged canonical token string for node-kind, action-kind and
form-input-kind assertion/wait pairs. Helpers, branches, group members and later
arguments can reuse it, but the receiving operation's enum domain still applies;
there is no case folding, ordinal conversion or arbitrary property query. Closed
v4 frames preserve v1/v2/v3 without synthesizing absent kinds; restoration validates
the exact token domain and its committed raw receipt. Existing authority, shared
fuel, image/output bounds and transactional replay remain unchanged.

[Optional GUI projections](leselang-control-flow.md#optional-gui-projections) add a
distinct `optional_string` scalar, explicit `optional_string(value: string-or-none)`
construction, `has_value(value: optional)` and lazy `value_or(left: optional,
right: string)`. Absent and present-empty remain different. Only the four placeholder/
unavailability assertion/wait results export `optional_expected`, and only their
optional-text arguments accept it directly. Other host domains and implicit coercions
are unchanged. V5 preserves v1-v4 closed frames; explicit null/string payloads,
original 1024/4096-byte bounds, shared fuel, raw-receipt checks and atomic replay
are required. This is not a general union/container type or arbitrary GUI query.

[Bounded text inspection](leselang-control-flow.md#bounded-text-inspection) adds
exact `contains`, `starts_with` and `ends_with` predicates with string `left`/`right`
inputs. `char_at(left: string, right: integer)` returns a distinct optional string
at a zero-based Unicode scalar position, never a byte offset or grapheme index;
out-of-range access is absent. Matching is case-sensitive without normalization
or regular expressions. Both operands are pure and eager, while enclosing lazy
operators retain their rules. Complete input scans and character materialization
share the original fuel, 4096-byte string bound and host validators. Existing
binary HIR and continuation/projection/journal versions remain unchanged.

[Bounded string collections](leselang-control-flow.md#bounded-string-collections)
add `strings(...)`, `split`, `append`, `join`, `item_at` and type-preserving pure
`fold`. A `string_list` has at most 64 entries and 4096 combined UTF-8 bytes;
constructor labels preserve source order but are not keys. Fold locals are
lexically scoped, its literal limit is preflighted before iteration, and every
copy/scan/iteration consumes shared fuel. Closed list payloads, durable locals,
original host domains and first-commit replay retain existing recovery boundaries.
This is not nested/generic containers, effectful iteration or multipresentation
GUI batch support.

[Reusable pure functions](leselang-control-flow.md#reusable-pure-functions) add
typed named scalar parameters and inferred scalar results, with a parameterless
`main` entry for multi-function programs. All definitions are checked, including
unused/cold code; recursion, closures and implicit coercion are
rejected. The limits are 32 declarations and 8 parameters per helper. Arguments
evaluate once in declaration order. Hygienic HIR expansion preserves the existing
node/depth, shared fuel, string, authority and journal boundaries. Helpers can
consume explicit scalar projections and prepare host arguments, not receive raw
host objects or replace a group's atomic member with result-capturing computation.

[Reusable effectful functions](leselang-control-flow.md#reusable-effectful-functions)
extend the same declarations to whole-flow/tail calls and explicit result `bind`.
Bounded normal-return splicing connects each expanded return path to its caller
with existing HIR nodes, not a hidden call stack. Atomic/group returns use closed
projections; scalar/list returns can drive later operations. Arguments remain
pure typed data, evaluated once in declaration order. All cold paths retain type,
capability, node/depth/text and host-shape checks before admission. Hygienic locals,
transactional successor admission, original authority/deadline and shared fuel
survive journal restart without source/function tables or wire migrations.
Effectful calls inside operands, helper arguments or pure loops/folds/recovery
remain unsupported. [Prepared atomic members](leselang-control-flow.md#prepared-atomic-members)
now allow helpers and expanded pure preparation/selection in `seq`/`all`/`repeat`
when every path yields exactly one operation with the same `HostOperation`
signature. Entire groups prepare before admission; later faults dispatch nothing,
and repeat reserves complete expanded preparation bounds before cloning. Only
resolved requests reach journals, with existing named projections, all-success
barriers, cancellation/deadline/authority/fuel and transactional replay fences.
Multi-step/result-capturing helpers, group-returning helpers as members and native
parallel debugger batches remain unsupported.

[Selected named groups](leselang-control-flow.md#selected-named-groups) now export
closed member signatures through pure conditional selection, including helper
group returns. Every cold return must keep the same group mode, ordered member
names and `HostOperation` signatures. Only selected preparation runs; all cold
paths still pass type/domain/capability checks. Resolved requests alone reach
journals, so restart does not reselect or refill fuel. Existing named projections,
transactional successor replay and parallel all-success barriers remain intact;
mixed topology, result-dependent nested groups and native parallel GUI batches
remain unsupported.

[Prepared atomic result bindings](leselang-control-flow.md#prepared-atomic-result-bindings)
allow pure `bind` preparation and `choose` directly as an atomic capture value.
Every cold path must yield one uniform `HostOperation`/result signature; the same
bounded classifier serves captures and prepared group members. Preparation locals
do not escape. Resolved committed requests are not reselected on restart; future
captures run from validated predecessor projections. Existing chains, group-owned
successors, conditional exits, shared budgets and transactional retry remain intact.
Closed legacy fields cannot be synthesized or accessed through cold choices.
Hidden captures/multiple effects, pure operand effects, effectful loops and native
parallel batches remain unsupported.

[Selected data-returning functions](leselang-control-flow.md#selected-data-returning-functions)
allow pure choices of direct helper calls and pure data fallbacks as a `bind`
value. Different supported helper topologies join through the same bounded data
type, not a union of raw results. Only selected arguments run; cold authority and
return-expansion budgets still apply. Hygienic locals, transactional caller joins,
parallel barriers and existing recovery schemas remain intact. Arbitrary nested
effectful binding values and native parallel GUI batches remain unsupported.

Serialized syntax trees validate source bounds, exact token coverage and EOF,
UTF-8-safe token/diagnostic/AST spans, declaration/parameter limits, and call depth
before acceptance. `SyntaxTree.function` selects `main` when present, with other
declarations in `helpers`; new empty fields are omitted from legacy single-function
serialization.
`token_text` and `reconstruct` return `Option`, so even a caller that later
mutates public token spans receives failure rather than a slicing panic. The
single oversized-source rejection-tree shape remains round-trip compatible.

## Execution Concurrency

The [concurrency model](leselang-embedding.md#concurrency-model) separates serial
engine entry, isolated root executions and structured effect batches. Long-lived
workers can consume new shared-journal work without rerunning source; independent
journals still need host-scoped routing and command idempotency namespaces.

## Canonical Formatting

`leselang_syntax::format` is the single canonical source formatter. It removes
comments and trivia, preserves declaration, parameter, argument and `all` branch order, keeps
zero- and single-argument calls on one line, and renders wider calls with
two-space indentation plus trailing commas. Output ends with exactly one
newline and is rejected if escaping would expand it beyond 256 KiB.

Formatting requires a diagnostic-free syntax tree and is idempotent:
parsing and formatting canonical output must reproduce the same bytes. Native
CLI Leselang exports pass through this formatter rather than maintaining a
frontend-specific printer.

The deterministic fuzz shelf runs through
`gewyvern_validate leselang-fuzz`. Its fixed seed covers arbitrary multi-byte
UTF-8, malformed escapes, trivia, nesting, oversized source, HIR lowering, and
bounded VM startup. Diagnostic-free syntax also exercises deterministic,
bounded, idempotent canonical formatting. Every token and diagnostic span must
remain on UTF-8 character boundaries. A parallel continuation corpus mutates
encoded VM images and requires deterministic fail-closed decoding or canonical
roundtrip.

The implemented surface excludes arbitrary mutation,
effectful loops and generic collection iteration, unstructured concurrency,
raw HTTP, shell execution, and host-language reflection. Pure scalar computation
and bindings are available before host suspension or after one captured atomic
result, including multiple atomic captures in bounded chains, or after a whole
sequential or parallel prefix to prepare one atomic tail or a bounded captured chain
ending in a pure scalar body or a typed conditional scalar exit, not between precomputed group steps or
inside effectful loops.
Synchronous source semantics do not expose
`async`/`await`; `all` is the explicit structured-concurrency form.

The frontend accepts structured declarations such as:

```leselang
fn main() = all(
  inventory: runtime.list(role: "edge"),
  refresh: runtime.refresh(runtime_id: "runtime-a")
)
```

`all` requires two to 64 uniquely named effect branches. Syntax and HIR preserve
declaration order, each branch retains its own result type, and function
authorization requires the union of all branch capabilities. The VM starts this
form as one atomic `Step::Effects` batch with a stable merge token and ordered
named requests. Completing a non-final branch returns `Step::Waiting`; completing
the final branch returns the durable aggregate result in declaration order.
SQLite recovery resumes unfinished branches without recreating completed work.
Nested `all` remains outside the current language surface and fails with
`LSV1002` before sequence allocation or journal mutation.
Mixing `all` with `seq`/`repeat` is also outside this execution slice. `seq` and
`repeat` can nest with each other, lowering to at most 64 ordered atomic effects.
They require the union of all capabilities before the first effect is emitted.

## HIR And Authorization

The syntax tree lowers each implemented operation into its corresponding typed
runtime effect. Lowering rejects unknown effects, duplicate named arguments,
unknown arguments, and values with the wrong shape.

Authorization is explicit and occurs before VM execution. A caller without
the effect's required capability receives a capability diagnostic; the VM does not emit an
effect request for unauthorized code. Authorization derives the required set
from the typed `Effect` tree and verifies that serialized HIR metadata matches
it, so clearing or duplicating `required_capabilities` cannot reduce authority.
Every generated request is semantically revalidated before journal persistence,
and restored continuations must round-trip through the canonical effect contract.

## Execution Protocol

Authorized control-plane effects first lower through `leselang-command` into a
pure `CommandPlan`. The plan owns the required capability and either a versioned
`QueryEnvelope` or `CommandEnvelope`; frontend origin is audit metadata and does
not select a different implementation. Frontend-local effects instead become a
typed `PresentationEnvelope` and are rejected by command lowering. The VM owns
continuation and journal lifecycle, but it does not privately construct domain
command semantics or reinterpret presentation operations as commands.
`CommandPlan` JSON carries its own schema version, round-trips canonically, and
is rejected before decoding when it exceeds 64 KiB.

The stackless VM advances through eight protocol states:

- `Done`: evaluation completed with bounded output
- `Effect`: the host must execute a typed request and resume the continuation
- `Effects`: a parallel `all` batch exposes its named requests
- `Waiting`: a parallel group is awaiting other branch completions
- `Yield`: cooperative suspension reserved by the protocol
- `Cancelled`: terminal requested cancellation or trusted deadline expiry
- `Failed`: terminal classified effect failure or exhausted semantic retries
- `Fault`: evaluation stopped with a stable VM diagnostic

For an atomic effect or sequential flow, `start` emits one typed query, command,
or presentation operation. The operation carries a continuation token and expected revision.
Successful sequential re-entry returns the next `Step::Effect`, or an ordered
`Value::Structured` at the end. The full bounded graph is persisted atomically;
later operations cannot be leased or directly resumed ahead of their predecessors.
Group recovery requires the VM journal, not restoration of one isolated branch image.
Read-only queries and successfully applied local presentation operations may use
the direct embedded `resume` path. Mutating commands must be leased and completed
through `acknowledge_effect`; direct mutation resume fails closed.
An atomic result-binding program also starts as one `Effect`, but successful
re-entry evaluates the saved pure body and commits its scalar output or typed
fault. Its enclosing scalar locals and body use continuation schema 2. An atomic
tail body uses schema 3 and atomically admits one schema-1 successor with the
original authority, remaining fuel and deadline. Its final value is the successor's
typed result. Further captures use schema 4, saving only exported scalar result
projections and lexical locals for the remaining body. Their final result may be
a computed scalar or the final atomic host value. Schema 5 permits a conditional
scalar early return instead of another capture, with the same final scalar type
on both branches and type/capability checks even on cold branches.
`field(value: result, name: "selected")` exports a boolean from selection results;
`required` exports a boolean from form-field requirement results. They can feed
pure helpers, decisions and subsequent atomic arguments without implicit text
coercion. These are confirmed operation states, not arbitrary UI reads. New
selection captures carry `projection_version: 2`; requirement captures also export
a form `field` key and now use v3. Their older v2 frames and unversioned v1
frames keep their old closed field set and never synthesize absent booleans.
See [boolean GUI projections](leselang-control-flow.md#boolean-gui-projections)
for the exact operation table, compatibility and cold-path validation rules.
Non-nullable verified text/form results export `expected`, `field` and/or `value`
according to the [v3 text table](leselang-control-flow.md#text-gui-projections).
Missing legacy text fields are rejected, never reconstructed from old raw receipts.
Wrong replies cannot admit a successor; saved strings are checked against committed
raw results, remain private projection frames and consume the original shared budgets.
Image-only restoration cannot recover the original authority;
use the journal or the trusted-host `Vm::restore_request` API. Legacy schema-1/2/3/4
images retain their wire forms; the current journal schema is 10.

`Vm::start_timed` accepts scheduler `now_ms` and a bounded timeout, persists the
resulting absolute deadline, and requires `resume_at`, `claim_effect`, or
`acknowledge_effect` to supply trusted scheduler time. At or after the deadline,
the VM atomically records `Cancelled(DeadlineExceeded)` before another dispatch
or result can win. `Vm::cancel_effect` records `Cancelled(Requested)` and may
fence an active lease. Both cancellation forms are durable, idempotently replayed
terminal states. The legacy `Vm::start` remains untimed for embedded compatibility.

Debugger-driven cancellation uses `Vm::cancel_effect_audited`. Journal schema 6
commits the requested terminal state and its command, principal, origin, session,
revision, and observation-time correlation in one transaction. Replays preserve
the first record, while command reuse or principal-scoped idempotency conflicts
fail closed. The queryable audit record deliberately excludes the continuation
token and idempotency key, and is removed when bounded journal retention removes
the corresponding continuation.

Debugger session revisions advance independently of target-resource revisions.
Cancellation checks the current session revision and the projected effect identity;
advancing the GUI debugger never rewrites a command's expected resource revision.

Workers classify execution errors as `Transient` or `Permanent` through
`Vm::report_effect_error`. Permanent errors immediately become a durable
`Step::Failed`; transient errors follow a bounded `RetryPolicy`. The default is
three semantic retries with deterministic exponential delays starting at 250 ms
and capped at 30 seconds. Policy limits are 16 retries and one-hour delays.
`retry_count` is separate from transport `attempt`, so a worker crash does not
consume business retry budget. A scheduled retry persists its `ready_at_ms` and
last classified error and cannot be claimed early; retry exhaustion becomes a
typed, replayable failure. The original effect request and command idempotency
key never change.

Root admission and semantic retry share only the pure capped-delay calculation;
their policies and counters remain distinct. Waiting, lease redelivery and
semantic retry neither replenish saved fuel nor extend the execution deadline,
including after reopening with zero default VM fuel. See
[shared retry arithmetic](leselang-embedding.md#shared-retry-delay-arithmetic).

`merge_declared` is the bounded deterministic merge kernel for future structured
`all` evaluation. A `MergePlan` declares two to 64 uniquely named branches;
completions may arrive in any order, but successful `Value::Structured` fields
and competing terminal outcomes are always selected in declaration order.
Malformed, missing, duplicate, or unexpected completions fail closed. Structured
values are limited to 16 levels, branch names to 64 ASCII identifier bytes, and
the caller's output-item budget. The merged terminal value uses the existing
journal schema and remains replayable after restart. This kernel is public VM
protocol today. Source-level `all(...)` parsing, typed lowering, atomic graph
creation, durable branch dispatch, restart recovery, and final declared-order
aggregation are implemented for one nesting level.

Continuation images are versioned and capped at 64 KiB. Execution also enforces
a source-size limit, fuel limit, 24-hour maximum deadline budget, and
10,000-item output limit.
Decoding rejects unknown image and effect fields, unsupported program counters,
standalone structured `all` images, and mismatched pending-effect/result-type
pairs. Effect requests use strict current and explicit legacy-query shapes, then
must pass semantic validation before entering the VM or journal.
Restored continuations advance their token sequence so a restart cannot reuse a
live token.

`Vm::new` provides an ephemeral journal for tests and embedded one-shot use.
`Vm::open_journal` opens the SQLite effect journal for service operation. It
atomically persists pending continuations before exposing an effect, restores
them after restart, and commits the first terminal step as authoritative.
Duplicate or competing completion after a process restart replays that durable
step rather than accepting a later result. Sequence allocation is transactional
across concurrent VM connections.

Service workers use the durable dispatch outbox rather than dispatching the
returned `Step::Effect` directly. `Vm::claim_effect` leases one ready or expired
request and returns an attempt-fenced `DispatchLease`; another live VM cannot
claim it until that lease expires. `Vm::acknowledge_effect` verifies the request,
attempt, and expiration, then commits the terminal step and acknowledgement in
one transaction. A crashed worker therefore causes bounded redelivery, while a
late worker cannot overwrite a newer attempt. The `now_ms` argument is scheduler
time supplied by the trusted runtime host, never by a remote request.

Current-time observation and completion accept the full inclusive portable clock
range, including `i64::MAX`; only construction of a new future lease requires
room for its duration. `scheduler_pressure` never reaps due work. At the exact
lease expiration, completion still requires the current attempt; a competing
redelivery can fence it out, and a due execution deadline always wins. See
[scheduler clock boundaries](leselang-embedding.md#shared-scheduler-clock).

Eligible outbox work is ordered by **fewest delivery attempts**, then **numeric
admission order**, identically in ephemeral and SQLite journals. Expired leases
and due semantic retries do not monopolize lower-attempt work. Attempt counters
survive restart, and not-before clocks and sequential barriers still apply.
An exhausted delivery leaves `LSV4017` visible when no eligible work has remaining
attempts, rather than blocking healthier work or silently disappearing. This is
finite-cohort balancing, not global FIFO or per-tenant fairness under continuous
admission. See [dispatch selection](leselang-embedding.md#dispatch-selection).

Trusted hosts can configure `SchedulerLimits` for pending dispatches and active
leases. Initial admissions are transactional and whole-batch: `LSV2501` signals
temporary admission pressure, `LSV2503` an oversized batch, and `LSV2500` invalid
limits. `try_claim_effect` distinguishes `Leased`, `Idle` and `Backpressured`;
legacy `claim_effect` reports a full lease quota as `LSV2502`, never an empty
queue. Pressure does not consume attempts, and admitted continuations still
drain. Limits are host configuration that must be reapplied consistently to shared
workers, not script syntax or persisted execution state. See
[host admission and backpressure](leselang-embedding.md#host-admission-and-backpressure).
Merge validation retains `LSV2401` through `LSV2404`, distinct from scheduler
pressure; invalid merge inputs are not automatically retryable.

Hosts can opt into `RootAdmission` for one not-yet-admitted root. Its immutable
program/authority, total attempt cap and capped backoff permit retry only for
`LSV2501`. Not-before polls do not enter the VM or touch its journal. The absolute
deadline is pinned at submission, including queue wait, and is never extended
by retries. Success, permanent failure, exhaustion or pre-admission cancellation
consumes the handle; later polls return `Finished`, not another execution.
`LSV2510` identifies invalid retry policy, `LSV2511` a regressing clock without
state change, `LSV2512` exhausted attempts and `LSV2513` pre-admission expiry.
The separate read-only `terminal_reason()` reports `AdmissionEnd` without adding
fields to existing status/poll JSON or journal images. `Started` records adapter
acceptance, not output delivery or execution completion; `HostUncertain` records
callback unwinding, not rejection or permission to replay. Cancellation, observed
expiry and attempt exhaustion have distinct sticky reasons. Input is consumed
before cleanup, and undelivered output remains locally owned until cleanup succeeds.
This is a host API, not new script syntax, a hidden queue, a timer, persisted
ingress or effect replay. Hosts still bound aggregate ingress and CPU, schedule
wakeups and own accepted-root cancellation. See
[bounded host ingress](leselang-embedding.md#bounded-host-ingress).

Restoring a request or bare continuation reserves its numeric identity in the
shared durable allocator in the same transaction as its records, including
idempotent duplicate imports. Rejected imports do not advance that watermark.
Already-open workers therefore observe successful imports without restarting.
Journal opening validates one consistent snapshot and repairs lagging sequence
metadata from retained effects and groups while preserving validated cold successor
reservations; compaction never lowers it. Cold reservations are checked against the
original watermark before repair, not legitimized by higher imported identities.
Invalid state leaves metadata unchanged, and exhausted identities never wrap.
Older deleted identities cannot be reconstructed from
missing metadata. See [restore and allocator recovery](leselang-embedding.md#restore-and-allocator-recovery).

The journal is schema-versioned. Schema 4 persists indexed absolute deadlines,
retry counts, and not-before clocks. Schema 5 adds strict merge-group and ordered
branch metadata with bounded plan, graph-state, token-namespace, and terminal-step
validation during recovery. The journal can atomically create a complete merge
graph: plan, branch continuations, dispatches, and declared-order links either
all commit or all roll back, and committed branches recover after restart. The
last terminal branch and its declared-order group result also commit together;
a failed group update leaves that branch pending under its existing lease.
Success, cancellation, classified failure, fault, and deadline terminal paths
share this finalization boundary. Retention treats a completed graph as one
logical record and atomically removes its group, ordered links, branch effects,
and dispatch rows; pending and partially completed graphs are excluded.
Schema 1 and 2 records migrate as untimed, schema 1
through 3 records migrate with zero semantic retries rather than receiving
fabricated execution history, and schema 1 through 4 receive empty merge tables.
Pre-existing conflicting merge-table names fail the migration closed. Schema 5
provides the executable durable graph lifecycle for one-level source `all`,
including atomic startup, ordered progress, final aggregation, restart recovery,
and whole-group retention. Nested `all` still fails closed with `LSV1002` before
sequence allocation.
Schema 7 adds a checked execution-order column for `seq` and `repeat` graphs.
Existing schema 1 through 6 graphs migrate as parallel. Recovery rejects an
order/plan mismatch or a pending sequence with out-of-order completions or
dispatch history on blocked steps. A sequential failure closes its remaining
steps and the group in the same transaction.
Schema 8 adds result-driven two-effect chains without changing the existing
table layout: their plan uses `result_chain` with sequential dispatch ordering.
The first completion and chosen successor are committed together. Recovery
requires the original request and validates shared authority/budgets and exact
two-step membership; it never recomputes a committed successor. Schema 7 migrates
to 8, while old readers reject the newer journal version.
Schema 9 extends the same storage with bounded multi-step `dataflow` plans and
schema-4 typed projection frames. Each appended step is transactional, inherits
authority/budgets, and leaves only one eligible current effect. Recovery validates
adjacent frames and never recomputes a committed decision. Progress reads use a
single database snapshot so competing workers reconcile completed prefixes.
Existing schema-8 two-effect plans migrate unchanged.
Schema 10 adds conditional scalar exits for schema-5 frames. It validates the
final scalar type and graph position, distinguishing early completion from a raw
host result with a committed successor without reevaluating the predicate.
The current completion and group close atomically; competing workers replay the
winner. Raw output counts are checked before scalar projection so early returns
and schema-4 final scalar bodies cannot hide cumulative overflow. Schema-9
journals migrate without rewriting continuations or plans.
Schema-6 bound groups use `bound_parallel` / `bound_sequential` plans within the
existing schema-10 layout. Their bounded pure body and scalar environment are
group metadata, while each atomic member names its owner. Recovery validates
all original requests, member types, authority and budgets together. The final
member and calculated scalar/fault commit atomically; replay never recomputes a
committed body. Old readers reject these explicit wire markers. See the
[group-result contract](leselang-control-flow.md#named-group-result-bindings).
Schema-7 sequential group tails also use the schema-10 journal layout with
`group_tail` plans. A reserved successor identity becomes a concrete request only
when the last prefix acknowledgement and tail append commit together. Prefix and
tail share one owner, authority/budgets and retention unit; recovery validates
the reservation and replays committed work without re-evaluation. Legacy schema-6
pure plans omit the new reservation field. Isolated schema-7 image/request restore
fails closed. See [sequential group tails](leselang-control-flow.md#sequential-group-tails).
Schema-8 `group_capture` plans retain one captured successor after a sequential
prefix. Its binding saves only typed member projections in an optional `groups`
environment; old bindings omit that field. The raw successor receipt and final
scalar/calculation fault commit together, after cumulative raw-output checks.
Recovery verifies projections against the committed prefix, never recalculates
the final body, and requires the complete owned journal. The schema-10 layout and
whole-unit retention are unchanged. See
[captured sequential successors](leselang-control-flow.md#captured-sequential-successors).
Schema-9 `parallel_group_tail` / `parallel_group_capture` plans use the same
schema-10 journal layout. Parallel members can finish out of order; the last
successful receipt and one reserved successor commit atomically after an
all-success barrier. Raw cumulative overflow saves a replayable terminal instead
of blocking the last commit forever. Original authority/shared budgets, complete
journal recovery, committed-prefix projection validation and whole-unit retention
are unchanged. The native single-presentation debugger rejects parallel starts
before creating journals; the Rust batch API supports the language form. See
[parallel group successors](leselang-control-flow.md#parallel-group-successors).
Continuation-schema-10 `group_dataflow` / `parallel_group_dataflow` plans extend
those groups to multiple captures without changing journal schema 10. A bounded
list reserves all possible successor identities, but only the selected next step
is materialized. Each raw receipt and successor append or final scalar commits
together after cumulative raw-output checks. Recovery verifies every frame against
the successful prefix and preceding capture receipts, keeps original authority,
fuel/deadline budgets, and replays rather than recalculating. Sequential chains
use the native debugger; parallel prefixes remain Rust-batch-only. See
[group result chains](leselang-control-flow.md#group-result-chains).
Continuation-schema-11 `group_conditional` / `parallel_group_conditional` plans
allow a same-type scalar exit before/between captured successors. The selected
exit or next request commits with the raw receipt after cumulative-output checks.
Recovery validates a scalar terminal only where the saved body can return without
another suspension, never before a mandatory capture, and does not reevaluate
source. Longest-path reservations, original authority/shared budgets, the parallel
all-success barrier and whole-unit retention are unchanged. Journal schema 10
needs no migration; schemas 1-10 retain their earlier boundaries. See
[group conditional exits](leselang-control-flow.md#group-conditional-exits).
The journal
uses full synchronous commits and a five-second lock
timeout, rejects symbolic-link final paths, and creates Unix files with `0600`
permissions. It is bounded to 10,000 records, 8 MiB per terminal step, 100
dispatch attempts, a five-minute maximum lease, and 64 MiB of total logical
payload including dispatch requests and merge plans.

`Vm::compact_journal` applies explicit count-based retention to terminal
records. It never deletes pending or leased effects, always retains at least one
completed record, and removes at most 1,000 records per transaction. The default
policy retains 5,000 completed records and deletes in batches of 500. Selection
uses durable insertion order, dispatch rows are removed by foreign-key cascade,
and SQLite secure deletion is enabled so deleted payload is overwritten in
reusable pages. `CompactionReport::reclaimed_logical_bytes` reports removed
payload; it does not promise immediate physical database file shrink.

This is a durable continuation guarantee plus at-least-once dispatch. For
`runtime.refresh`, every redelivery reuses the same domain idempotency key, so
the current domain kernel commits the refresh once and replays its first result.
This is not a blanket exactly-once guarantee for arbitrary external adapters;
each future mutating effect must prove the same end-to-end contract.
Terminal replay is guaranteed only while the record remains inside the explicit
retention window; a compacted token becomes unknown (`LSV2004`). One-level
structured `all` is part of the stable execution contract; nested `all` remains
a deliberate future language extension and fails before sequence allocation.

## Diagnostics

Diagnostics use stable subsystem prefixes:

| Prefix | Owner | Examples |
| --- | --- | --- |
| `LSE` | lexer and parser | malformed input, source limit |
| `LSH` | HIR and authorization | unknown effect, duplicate argument, missing or forged capability metadata |
| `LSV` | VM, continuation, and journal | invalid image or effect identity, revision conflict, persistence failure |

Consumers must branch on diagnostic codes rather than English messages. Spans
use byte offsets into the original UTF-8 source.

## Integration Sequence

For deterministic model or CLI integration:

1. Parse source and report all syntax diagnostics.
2. Lower the syntax tree into HIR and report semantic diagnostics.
3. Authorize required capabilities before starting the VM.
4. Open the durable journal, then call `Vm::start`; pending state is committed before return.
5. Prefer `Vm::start_timed`, then call `Vm::claim_effect` using trusted scheduler time.
6. Call `Vm::acknowledge_effect` with the same lease and typed result before expiry.
7. Let expired leases become claimable after restart; never reuse an older attempt.
8. For commands, execute the persisted `CommandEnvelope` unchanged on every redelivery.
9. Treat repeated acknowledged completion as replay, not another external operation.
10. Treat `Cancelled` as a terminal result and never dispatch its operation again.
11. Report classified failures through `report_effect_error`; never retry outside the journal.
12. Run bounded `compact_journal` maintenance and treat its retention window as part of the service contract.

`Vm::resume` remains the direct embedded path for an effect that has not been
leased. Once a request is leased, completion must use `acknowledge_effect`.

The independent frontend implementation lives in `crates/leselang-syntax`,
`crates/leselang-host-contract`, and `crates/leselang-hir`. Execution and
product bindings currently live in `crates/leselang-vm`,
`crates/leselang-command`, `crates/leselang-ui`, and
`crates/leselang-observe`. Delivery progress is tracked by the
[project status tensor](project-status-system.md), not inferred from future
examples in architecture documents.
