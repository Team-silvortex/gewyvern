# Leselang Language Contract

This reference is the authoritative, model-oriented contract for the currently
implemented Leselang slice. The broader destination is defined by the
[Leserpent 2.0 architecture](leserpent-2-architecture.md); unimplemented roadmap
syntax is not part of this contract.

Status: **Gate 2 execution and syntax contracts stable at 1.0.0**. The current
vertical slice parses, lowers, authorizes, suspends,
serializes, restores, and resumes the read-only
`runtime.list`, `runtime.inspect`, `runtime.history`, and `runtime.logs` effects
plus the idempotent `runtime.refresh`, `runtime.refresh_capabilities`, and
explicitly confirmed `runtime.deploy` and `debugger.cancel` command effects,
plus the frontend-local `ui.focus`, `ui.navigate_focus`, `ui.scroll_into_view`,
`ui.assert_visible`, `ui.assert_hidden`, `ui.wait_hidden`, `ui.assert_realized`,
`ui.wait_realized`, `ui.wait_visible`, `ui.assert_focused`, `ui.wait_focused`,
`ui.assert_unfocused`, `ui.wait_unfocused`, `ui.assert_enabled`,
`ui.assert_disabled`, `ui.wait_enabled`, `ui.wait_disabled`,
    `ui.assert_window_open`, `ui.wait_window_open`,
    `ui.assert_window_closed`, `ui.wait_window_closed`,
`ui.assert_selection`, `ui.wait_selection`, `ui.assert_text`, `ui.wait_text`,
`ui.assert_automation_id`, and
`ui.assert_node_kind`, `ui.assert_action_kind`,
`ui.assert_action_unavailable_reason`,
`ui.wait_action_unavailable_reason`, `ui.assert_form_field`,
`ui.assert_form_field_input_kind`, `ui.assert_form_field_required`,
`ui.assert_form_field_max_length`, `ui.assert_form_field_placeholder`,
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
normalized to `none` during HIR lowering.

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
sequence and persists the complete `CommandEnvelope` before dispatch.

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

The first presentation operation is stable-node focus:

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
Parsing and lowering that source must reproduce the same effect. GUI event
export uses this printer instead of maintaining a frontend-specific language
template.

## Grammar

```ebnf
program       = function, EOF ;
function      = "fn", identifier, "(", ")", "=", call ;
call          = effect-call | all-call ;
effect-call   = identifier, ".", identifier, "(", [ arguments ], ")" ;
all-call      = "all", "(", branch, ",", branch, { ",", branch }, [ "," ], ")" ;
branch        = identifier, ":", call ;
arguments     = argument, { ",", argument }, [ "," ] ;
argument      = identifier, ":", value ;
value         = string | "none" | call ;
identifier    = ( letter | "_" ), { letter | digit | "_" } ;
string        = '"', { character | escape }, '"' ;
escape        = "\\", ( '"' | "\\" | "n" | "r" | "t" ) ;
```

Whitespace and `//` line comments are retained as lossless tokens, including
their byte spans. Reassembling token text must reproduce the original source.
Source is UTF-8 and limited to 256 KiB.

Serialized syntax trees validate source bounds, exact token coverage and EOF,
UTF-8-safe token/diagnostic/AST spans, and call depth before acceptance.
`token_text` and `reconstruct` return `Option`, so even a caller that later
mutates public token spans receives failure rather than a slicing panic. The
single oversized-source rejection-tree shape remains round-trip compatible.

## Canonical Formatting

`leselang_syntax::format` is the single canonical source formatter. It removes
comments and trivia, preserves declared argument and `all` branch order, keeps
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

The implemented surface deliberately excludes general expressions, local
bindings, arbitrary mutation, loops, unstructured concurrency, raw HTTP, shell
execution, and host-language reflection. Synchronous source semantics do not expose
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

## HIR And Authorization

The syntax tree lowers each implemented operation into its corresponding typed
runtime effect. Lowering rejects unknown effects, duplicate named arguments,
unknown arguments, and values with the wrong shape.

Authorization is explicit and occurs before VM execution. A caller without
the effect's required capability receives a capability diagnostic; the VM does not emit an
effect request for unauthorized code.

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

The stackless VM advances through six protocol states:

- `Done`: evaluation completed with bounded output
- `Effect`: the host must execute a typed request and resume the continuation
- `Yield`: cooperative suspension reserved by the protocol
- `Cancelled`: terminal requested cancellation or trusted deadline expiry
- `Failed`: terminal classified effect failure or exhausted semantic retries
- `Fault`: evaluation stopped with a stable VM diagnostic

For the current slice, `start` emits one typed query, command, or presentation
operation. The operation carries a continuation token and expected revision.
Read-only queries and successfully applied local presentation operations may use
the direct embedded `resume` path. Mutating commands must be leased and completed
through `acknowledge_effect`; direct mutation resume fails closed.

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
| `LSH` | HIR and authorization | unknown effect, duplicate argument, missing capability |
| `LSV` | VM, continuation, and journal | invalid image, revision conflict, persistence failure |

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

The implementation lives in `crates/leselang-syntax`, `crates/leselang-hir`,
and `crates/leselang-vm`. Delivery progress is tracked by the
[project status tensor](project-status-system.md), not inferred from future
examples in architecture documents.
