# Leselang UI IR Contract

This document defines the implemented renderer-neutral Gate 4 UI boundary in
`crates/leselang-ui`. Avalonia, web, mobile, persistence, transport, and adapter
types are deliberately outside this contract.

The crate is the GUI automation substrate, not a frontend framework. It does
not make any GUI toolkit automatically compatible. A host renderer must expose
a developer-owned adapter for the renderer-neutral document, event, patch, and
presentation-operation schema, or use dedicated tooling to emit a generated
framework binding from that schema. This should feel like protobuf-style
interface generation: the schema is shared, but each target framework still has
an explicit adapter surface. Host widget trees may be rich, but the shared
automation contract remains typed, bounded, serializable, and free of
host-language object references. `UiAdapterManifest` is the Rust-owned handshake
for that boundary: it declares a developer-owned adapter or generated framework
binding against the current UI schema and required presentation atom set.

Status: **Gate 4 renderer-neutral slice complete, stable contract 1.0.0**.

Patch decoding rejects unknown operation/action fields, malformed referenced
node identifiers, and unsafe embedded node/form metadata before a renderer may
queue the patch. Document-dependent parent binding and graph edits remain
transactionally validated by `apply_patch` against the current revision.

## Adapter Manifest

`UiAdapterManifest` is not automatic framework discovery. It is an explicit
compatibility proof emitted by a hand-written renderer adapter or by dedicated
generator tooling. The manifest carries schema version `2`, a stable
`adapter_id`, a bounded framework label, a `binding_kind` of either
`developer_owned_adapter` or `generated_framework_binding`, the target
`ui_schema_version`, and booleans proving support for document, event, and patch
schemas. It must also list the complete `required_ui_presentation_atoms()` set:
all fifty-two current presentation atoms, including focus, window lifecycle,
wait, assertion, selection, action metadata, form metadata, and accessibility operations. Schema
version `2` also carries `presentation_atom_profiles`: one canonical profile per
atom, classifying the GUI family and effect model as mutation, assertion, or
wait so generated adapters can build a 1:1 mapping table without guessing.

Validation fails closed for unsupported schema versions, invalid adapter IDs,
missing document/event/patch support, duplicate presentation atoms, omitted
required atoms, missing or duplicate atom profiles, profile/atom mismatches,
oversized framework labels, control characters, and unknown JSON fields. The
manifest gives future Rust-native GUI hosts, FFI shims, C# renderers, TypeScript
renderers, and mobile clients the same protobuf-style binding checkpoint without
letting any host object model leak into the shared IR.
The renderer presentation conformance fixture emits both a developer-owned
Avalonia adapter manifest and a generated TypeScript/web binding manifest; the
C# conformance runner decodes them with source-generated strict JSON metadata,
round-trips them, validates the complete atom/profile set, and rejects unknown
fields or numeric enum tokens.

## Pure Flow

```text
QueryResult::RuntimeList -> fleet_document -> UiDocument
bounded runtime log source -> leselang-observe -> RuntimeLogProjection
RuntimeLogProjection -> runtime_log_document -> UiDocument
validated EffectRequest -> leselang-observe -> DebuggerProjection
DebuggerProjection -> debugger_document -> UiDocument
DebuggerCancel CommandPlan -> inspect/dry-run -> VM cancellation
UiEvent + UiDocument + LoweringContext -> CommandPlan
UiEvent + UiDocument -> HIR Effect -> canonical Leselang
HIR Effect + UiDocument -> equivalent UiEvent
previous UiDocument + next UiDocument -> UiPatch
UiAdapterManifest -> explicit adapter or generated binding compatibility proof
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

Observed capability sections include the command revision that their discovery
completed for. This binding is projection metadata rather than an adapter claim:
it lets every renderer distinguish a command event from its later observation,
even when repeated discovery returns identical content. Legacy projections may
omit the binding and render an explicit unavailable marker instead of guessing.

The Avalonia remote shell now supplies Inspect, History, and Logs through three
concurrent, authenticated `/v1/wire` queries. Its strict transport DTO is the only layer
that can represent a runtime endpoint; the composed `RemoteWorkspaceSnapshot`
retains only renderer-neutral runtime, bounded command-history fields, and
sanitized log displays. Revision mismatch, runtime/name rebinding, unknown
fields, null required data, non-monotonic log sequences, and history/log limits
all reject the complete workspace rather than mounting partial state. Raw log
messages are admitted only up to the 64 KiB domain bound, then control characters
are normalized and the UI display is UTF-8 safely capped at 768 bytes.

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
- maximum parameterized form fields: `16`
- maximum parameterized form value: `256` bytes
- adapter manifest schema version: `2`
- maximum adapter framework label: `128` bytes
- required adapter presentation atoms: `52`
- node IDs: unique, stable, ASCII identifiers up to 128 bytes

Validation rejects duplicate or invalid IDs, control characters, invalid
localization keys, unlabelled actions, over-depth trees, and oversized graphs.
Runtime actions must match the enclosing runtime card, preventing a modified
document from redirecting an event to another runtime. Debugger actions must
likewise match the enclosing workspace's explicit session binding. Serialized IR structs
reject unknown fields rather than silently accepting producer/renderer drift.

## Events

`UiEvent` identifies a node and a typed event kind; it never carries a command.
An `activate` event carries no values. A `submit` event may carry only fields
declared by the target action's parameterized form. Forms bound their field
count, keys, localization, required state, maximum lengths, and input kinds;
unknown, missing, oversized, or invalid values fail closed in both Rust and the
.NET semantic renderer. Deployment submission then uses the same HIR effect and
`leselang-command` normalization path as textual Leselang and the native CLI.
Runtime inspect, refresh, capability refresh, deployment, and debugger
cancellation are exhaustively mapped through this path. The reverse mapper
locates an equivalent stable action node and reconstructs bounded form values,
while canonical export is owned by Rust HIR rather than a renderer. The
lowering context must fence exactly the document revision.

Avalonia's production preview follows the same ownership rule. It sends only a
strict, versioned, bounded semantic intent to the connected daemon's
authenticated `POST /v1/leselang-export` route. `leserpentd` validates the
intent, constructs the HIR effect, and returns source only after the Rust
canonical printer has parsed and lowered it back to the same effect. C# owns
neither Leselang quoting nor source templates. Parameterized form previews are
debounced and cancellable; a network or protocol failure disables copying and
never substitutes a frontend-generated program. This export route is pure and
cannot execute the represented operation.

Unknown nodes, nodes without actions, stale revisions, missing capabilities,
forged runtime or debugger-session bindings, invalid automation effects, and
effects without an action in the current document fail closed.

Semantic action equivalence is joined by fifty-two presentation atoms.
`UiPresentationOperation::Focus` maps one-to-one to `ui.focus(node_id: ...)`
and requires an interactive action.
`UiPresentationOperation::NavigateFocus` maps one-to-one to
`ui.navigate_focus(node_id: ..., direction: ...)`, requires a currently
focused interactive action, admits only `next`, `previous`, `first`, or `last`,
and returns the actual stable destination rather than inferring it from
semantic-tree order.
`UiPresentationOperation::ScrollIntoView`
maps one-to-one to `ui.scroll_into_view(node_id: ...)` and accepts any existing
semantic node. `UiPresentationOperation::AssertVisible` maps one-to-one to
`ui.assert_visible(node_id: ...)` and accepts any existing semantic node.
`UiPresentationOperation::AssertHidden` maps one-to-one to
`ui.assert_hidden(node_id: ...)` and accepts any existing semantic node.
`UiPresentationOperation::WaitHidden` maps one-to-one to
`ui.wait_hidden(node_id: ...)`, accepts any existing semantic node, and carries
the same protocol-fixed 2000 ms deadline as visibility wait.
`UiPresentationOperation::AssertRealized` maps one-to-one to
`ui.assert_realized(node_id: ...)` and accepts any existing semantic node.
`UiPresentationOperation::WaitRealized` maps one-to-one to
`ui.wait_realized(node_id: ...)`, accepts any existing semantic node, and
carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::WaitVisible` maps one-to-one to
`ui.wait_visible(node_id: ...)`, accepts any existing semantic node, and
carries its own protocol-fixed 2000 ms deadline.
`UiPresentationOperation::OpenWindow` and `CloseWindow` map one-to-one to
`ui.open_window(node_id: ...)` and `ui.close_window(node_id: ...)`. Both accept
any existing semantic node and are idempotent lifecycle mutations. Adapters may
open only a fully detached renderer surface, close only the target's containing
native window, and must not activate or focus a window implicitly.
`UiPresentationOperation::AssertFocused` maps one-to-one to
`ui.assert_focused(node_id: ...)` and requires an interactive action.
`UiPresentationOperation::WaitFocused` maps one-to-one to
`ui.wait_focused(node_id: ...)`, requires an interactive action, and carries
the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertUnfocused` maps one-to-one to
`ui.assert_unfocused(node_id: ...)` and requires an interactive action.
`UiPresentationOperation::WaitUnfocused` maps one-to-one to
`ui.wait_unfocused(node_id: ...)`, requires an interactive action, and carries
the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertEnabled` maps one-to-one to
`ui.assert_enabled(node_id: ...)` and also requires an interactive action.
`UiPresentationOperation::AssertDisabled` maps one-to-one to
`ui.assert_disabled(node_id: ...)` and requires an interactive action.
`UiPresentationOperation::WaitEnabled` maps one-to-one to
`ui.wait_enabled(node_id: ...)`, requires an interactive action, and carries
the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::WaitDisabled` maps one-to-one to
`ui.wait_disabled(node_id: ...)`, requires an interactive action, and carries
the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertWindowOpen` maps one-to-one to
`ui.assert_window_open(node_id: ...)` and accepts any existing semantic node.
`UiPresentationOperation::WaitWindowOpen` maps one-to-one to
`ui.wait_window_open(node_id: ...)`, accepts any existing semantic node, and
carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertWindowClosed` maps one-to-one to
`ui.assert_window_closed(node_id: ...)` and accepts any existing semantic node.
`UiPresentationOperation::WaitWindowClosed` maps one-to-one to
`ui.wait_window_closed(node_id: ...)`, accepts any existing semantic node, and
carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertSelection` maps one-to-one to
`ui.assert_selection(node_id: ..., state: ...)`, requires a semantic node with
selection metadata, and admits only `selected` or `unselected`.
`UiPresentationOperation::WaitSelection` maps one-to-one to
`ui.wait_selection(node_id: ..., state: ...)`, requires the same selectable
semantic node, and carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertText` maps one-to-one to
`ui.assert_text(node_id: ..., expected: ...)`, requires a text-rendering
semantic node, and carries a control-free expected value of at most 1024 UTF-8
bytes. `UiPresentationOperation::WaitText` maps one-to-one to
`ui.wait_text(node_id: ..., expected: ...)`, requires the same text-rendering
semantic node, and carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertAutomationId` maps one-to-one to
`ui.assert_automation_id(node_id: ..., expected: ...)`, accepts every existing
semantic node, and carries an expected value that must itself be a valid UI
node identifier. `UiPresentationOperation::AssertNodeKind` maps one-to-one to
`ui.assert_node_kind(node_id: ..., kind: ...)`, accepts every existing semantic
node, and carries a stable semantic renderer kind.
`UiPresentationOperation::WaitNodeKind` maps one-to-one to
`ui.wait_node_kind(node_id: ..., kind: ...)`, accepts every existing semantic
node, validates the same stable semantic renderer kind, and carries the
protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertActionKind` maps one-to-one to
`ui.assert_action_kind(node_id: ..., kind: ...)`, requires a semantic action
node, and carries the stable semantic action payload kind as
`expected_action_kind`.
`UiPresentationOperation::WaitActionKind` maps one-to-one to
`ui.wait_action_kind(node_id: ..., kind: ...)`, requires the same semantic
action node, validates the same stable semantic action payload kind as
`expected_action_kind`, and carries the protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertActionLabel` maps
one-to-one to `ui.assert_action_label(node_id: ..., expected: ...)`, requires
the same semantic action node, and compares its explicit semantic action label
through the renderer's native automation name. `UiPresentationOperation::WaitActionLabel`
maps one-to-one to `ui.wait_action_label(node_id: ..., expected: ...)`,
requires the same semantic action node, validates the same bounded expected
text, and carries the protocol-fixed 2000 ms deadline while waiting for that
explicit label to match. `UiPresentationOperation::AssertActionAvailable`
maps one-to-one to `ui.assert_action_available(node_id: ...)`, requires the
same semantic action node, and succeeds only when renderer-maintained semantic
availability has no unavailable reason.
`UiPresentationOperation::WaitActionAvailable` maps one-to-one to
`ui.wait_action_available(node_id: ...)`, requires the same semantic action node,
and carries the protocol-fixed 2000 ms deadline while waiting for
renderer-maintained semantic action availability to become true.
`UiPresentationOperation::AssertActionUnavailableReason`
maps one-to-one to
`ui.assert_action_unavailable_reason(node_id: ..., expected: ...)`, requires a
semantic action node, and compares the configured action unavailable reason or
absence without conflating it with accessibility help text.
`UiPresentationOperation::WaitActionUnavailableReason` maps one-to-one to
`ui.wait_action_unavailable_reason(node_id: ..., expected: ...)`, requires the
same semantic action node, and carries the protocol-fixed 2000 ms deadline while
waiting for the configured action unavailable reason or absence.
`UiPresentationOperation::AssertFormField` maps one-to-one to
`ui.assert_form_field(node_id: ..., field: ..., expected: ...)`, requires a
semantic deployment form action, validates the bounded form field key, and
compares the stable semantic field label fallback.
`UiPresentationOperation::AssertFormFieldInputKind` maps one-to-one to
`ui.assert_form_field_input_kind(node_id: ..., field: ..., kind: ...)`, requires
the same semantic deployment form action and bounded field key, and compares the
stable semantic form input kind (`path_token` or `trimmed_text`).
`UiPresentationOperation::AssertFormFieldRequired` maps one-to-one to
`ui.assert_form_field_required(node_id: ..., field: ..., state: ...)`, requires
the same semantic deployment form action and bounded field key, and compares the
stable semantic required state (`required` or `optional`).
`UiPresentationOperation::AssertFormFieldMaxLength` maps one-to-one to
`ui.assert_form_field_max_length(node_id: ..., field: ..., max_length: ...)`,
requires the same semantic deployment form action and bounded field key, and
compares the stable semantic maximum length as an integer value parsed from a
bounded decimal string in Leselang source.
`UiPresentationOperation::AssertFormFieldPlaceholder` maps one-to-one to
`ui.assert_form_field_placeholder(node_id: ..., field: ..., expected: ...)`,
requires the same semantic deployment form action and bounded field key, and
compares the stable semantic placeholder fallback or absence. Its `expected`
payload may be bounded text or `none` in Leselang source.
`UiPresentationOperation::WaitFormField` maps one-to-one to
`ui.wait_form_field(node_id: ..., field: ..., expected: ...)`, requires the
same semantic deployment form action and bounded field key, validates the same
bounded expected text as `ui.assert_form_field`, and carries the
protocol-fixed 2000 ms deadline while waiting for the stable semantic field
label fallback to match.
`UiPresentationOperation::WaitFormFieldInputKind` maps one-to-one to
`ui.wait_form_field_input_kind(node_id: ..., field: ..., kind: ...)`, requires
the same semantic deployment form action and bounded field key, validates the
same typed input kind as `ui.assert_form_field_input_kind`, and carries the
protocol-fixed 2000 ms deadline while waiting for the stable semantic input
kind to match.
`UiPresentationOperation::WaitFormFieldRequired` maps one-to-one to
`ui.wait_form_field_required(node_id: ..., field: ..., state: ...)`, requires
the same semantic deployment form action and bounded field key, validates the
same explicit required/optional state as `ui.assert_form_field_required`, and
carries the protocol-fixed 2000 ms deadline while waiting for the stable
semantic required bit to match.
`UiPresentationOperation::WaitFormFieldMaxLength` maps one-to-one to
`ui.wait_form_field_max_length(node_id: ..., field: ..., max_length: ...)`,
requires the same semantic deployment form action and bounded field key,
validates the same bounded decimal maximum length as
`ui.assert_form_field_max_length`, and carries the protocol-fixed 2000 ms
deadline while waiting for the stable semantic maximum length to match.
`UiPresentationOperation::WaitFormFieldPlaceholder` maps one-to-one to
`ui.wait_form_field_placeholder(node_id: ..., field: ..., expected: ...)`,
requires the same semantic deployment form action and bounded field key, accepts
bounded text or `none`, and carries the protocol-fixed 2000 ms deadline while
waiting for the stable semantic placeholder fallback or absence.
`UiPresentationOperation::AssertAccessibleName` maps one-to-one to
`ui.assert_accessible_name(node_id: ..., expected: ...)`, accepts every existing
semantic node, and uses the same expected-value bound.
`UiPresentationOperation::WaitAccessibleName` maps one-to-one to
`ui.wait_accessible_name(node_id: ..., expected: ...)`, accepts every existing
semantic node, uses the same expected-value bound, and carries the
protocol-fixed 2000 ms deadline.
`UiPresentationOperation::AssertAccessibleDescription` maps one-to-one to
`ui.assert_accessible_description(node_id: ..., expected: ...)` and requires a
semantic node with an explicitly declared accessibility description.
`UiPresentationOperation::WaitAccessibleDescription` maps one-to-one to
`ui.wait_accessible_description(node_id: ..., expected: ...)`, requires the same
explicit accessibility description metadata, uses the same expected-value bound,
and carries the protocol-fixed 2000 ms deadline. None can
become a `UiEvent` or `CommandPlan`; all fifty-two travel in
capability-gated VM presentation envelopes and return operation-specific typed
results with operation identity bound across re-entry.

Avalonia resolves all fifty-two operations through its stable visual index. Focus
uses native `Control.Focus()`. Focus navigation requires the declared start to
own native focus, invokes the native `FocusManager.TryMoveFocus` with the typed
direction, and accepts only a distinct realized action from the same index.
The actual destination is returned, navigation never activates an action, and
missing, noninteractive, unrealized, or unfocused starts fail without changing
focus. First and last navigation resolve the renderer's stable visual-index
action boundary and then use native focus on that destination. Scrolling uses native `BringIntoView()`
without changing focus or activating an action. Visibility assertion requires
a realized control that is effectively visible, has nonzero layout bounds, and
intersects the renderer viewport. Hidden assertion reads that same complete
predicate after realization and succeeds only while it is false, so unrealized
targets do not masquerade as hidden. Hidden wait polls that same false predicate
through the cancellable dispatcher-yielding adapter until its fixed deadline,
without scrolling, focusing, hiding, or forcing realization. Realization
assertion succeeds only when the
visual index resolves the node to a live native control and never forces
virtualized content to materialize. Realization wait polls that same predicate
through a cancellable dispatcher-yielding adapter until its fixed deadline; it
does not call `BringIntoView()` or create controls. Visibility wait polls the
complete assertion predicate, including viewport intersection, through the same
cancellable adapter and never scrolls the target. Focus assertion reads native
`Control.IsFocused`; focused wait polls that same predicate without invoking
`Control.Focus()`. Unfocused assertion reads the inverse native focus predicate;
unfocused wait polls the same inverse predicate through the dispatcher-yielding
adapter without invoking `Control.Focus()` or transferring focus. Native window
deactivation is therefore a legitimate external transition to unfocused state;
the real-window verifier reports that path separately from a persistent-focus
timeout instead of treating desktop activation timing as an adapter failure.
Enabled assertion reads native effective enabled state,
including ancestors. Disabled assertion reads the same native effective enabled
state and succeeds only for disabled actions, providing a positive safety
predicate rather than relying on an enabled-assertion failure. Enabled wait
polls that same predicate through the cancellable adapter without changing
availability or invoking the action. Disabled wait polls the inverse of that
same native effective-enabled predicate through the cancellable adapter without
changing availability or invoking the action.
Window-open assertion resolves the same stable visual index and succeeds only
when the realized target and renderer surface are attached to the same native
`Window` visual tree; it never calls native activation, open, close, or focus.
Window-open wait polls that same native-window membership predicate through the
cancellable dispatcher-yielding adapter until its fixed deadline and also never
calls native activation, open, close, or focus.
Window-closed assertion reads the inverse native-window membership predicate
after realizing the same stable semantic node; detached renderer surfaces satisfy
it, but missing or unrealized nodes still fail separately. Window-closed wait
polls that same inverse predicate through the dispatcher-yielding adapter until
its fixed deadline, and a persistently open window times out without invoking a
native close API or mutating focus.
Selection assertion reads the native selected state of the realized selectable
control, while selection wait polls that same predicate through the cancellable
adapter until the protocol-fixed deadline. Mismatched, selectionless, or native
nonselectable targets fail with typed presentation errors and never focus,
activate, scroll, or select the target. Text
assertion reads native `TextBlock.Text` or string
`Button.Content` and uses exact ordinal comparison rather than semantic-IR
fallback text. Text wait polls that same native displayed-text predicate through
the cancellable dispatcher-yielding adapter until the protocol-fixed deadline,
observing semantic patch driven text transitions without focusing, scrolling, or
rewriting text. Automation ID assertion reads native platform automation identity
and requires it to match the expected stable node identifier exactly.
Node-kind assertion compares the stable semantic renderer kind and uses no
guessing, coordinates, or OCR. Node-kind wait polls that same semantic predicate
through the dispatcher-yielding adapter until its fixed deadline without
realizing, scrolling, focusing, or mutating the node. Action-kind assertion
compares the expected semantic action kind with the realized node's stable
action payload and never activates or focuses the target. Action-kind wait polls
that same action payload predicate until its fixed deadline without clicking,
activating, enabling, focusing, or mutating the action. Action-label assertion compares the explicit
semantic action label through native automation name, while action-label wait
polls that same exact predicate through the dispatcher-yielding adapter until
its fixed deadline. Both preserve focus and never click, activate, enable, or
rewrite the action. Action-available assertion reads
renderer-maintained semantic action availability and succeeds only when no
unavailable reason is present, while action-available wait polls that same
predicate through the cancellable dispatcher-yielding adapter until its fixed
deadline. Both observe external availability transitions and never focus,
activate, click, enable, or rewrite the action. Form-field assertion reads the realized node's
stable semantic deployment form metadata, compares the declared field label
fallback exactly, and never types into or submits the form. Form-field input-kind
assertion reads the same stable semantic deployment form metadata, compares the
declared field input kind exactly, and never types into or submits the form.
Form-field required assertion reads the same stable semantic deployment form
metadata, compares the declared required bit exactly, and never types into,
submits, marks, or otherwise edits the form.
Form-field max-length assertion reads the same stable semantic deployment form
metadata, compares the declared maximum length exactly, and never types into,
submits, edits, truncates, or otherwise mutates the form.
Form-field placeholder assertion reads the same stable semantic deployment form
metadata, compares the declared placeholder fallback or its absence exactly, and
never types into, submits, edits, or otherwise mutates the form.
Form-field placeholder wait polls that same stable semantic deployment form
metadata through the cancellable dispatcher-yielding adapter until the
protocol-fixed deadline, observing external placeholder transitions without
typing into, submitting, editing, or otherwise mutating the form.
Action-unavailable-reason assertion reads renderer-maintained action
availability state, compares the declared unavailable reason or its absence
exactly, and never focuses, activates, clicks, or rewrites the action.
Action-unavailable-reason wait polls that same renderer-maintained action
availability state through the cancellable dispatcher-yielding adapter until the
protocol-fixed deadline. It observes external reason transitions and clearing,
but never focuses, activates, clicks, or rewrites the action.
Accessible-name assertion independently reads
native `AutomationProperties.Name` with exact ordinal comparison. None of the
assertions mutates the control. Accessible-name wait polls the same native
`AutomationProperties.Name` predicate through the cancellable
dispatcher-yielding adapter until its fixed deadline, observing external
automation-name transitions without focusing, scrolling, rewriting text, or
mutating metadata. Accessible-description assertion independently
reads native `AutomationProperties.HelpText`, also with exact ordinal
comparison, and rejects semantic targets that never declared a description.
Accessible-description wait polls the same native HelpText predicate through the
cancellable dispatcher-yielding adapter until its fixed deadline, observing
external automation-description transitions without focus, scrolling, rewriting
text, or mutating metadata.
The real-window probe covers natural post-layout realization and visibility,
persistent unrealized and invisible timeout, external hidden transition and
persistent visible hidden-wait timeout, native application, unrealized,
hidden, unfocused, disabled, external enablement transition and persistent
disabled timeout, external disablement transition and persistent enabled
disabled-wait timeout, external focus transition and persistent
realized-unfocused timeout without implicit focus mutation, deactivation-aware
persistent-focus verification, native forward and
backward focus navigation, stable destination reporting, failure focus
preservation, and
zero action activation, native window-open assertion, dispatcher-yielding
window-open wait,
native selected/unselected assertion, dispatcher-yielding
selection wait, persistent selection mismatch timeout,
text-mismatched, external text transition, persistent text mismatch timeout,
automation-id-mismatched, node-kind-mismatched,
action-kind-mismatched, action-unavailable-reason-mismatched,
external action-unavailable-reason transition, reason clearing, persistent
action-unavailable-reason mismatch timeout,
form-field-label-mismatched,
form-field-input-kind-mismatched, form-field-required-mismatched,
form-field-max-length-mismatched,
form-field-placeholder-mismatched, external form-field-placeholder transition,
persistent form-field-placeholder mismatch timeout,
accessible-name-mismatched, external accessible-name transition,
persistent accessible-name mismatch timeout,
accessible-description-mismatched, external accessible-description transition,
persistent accessible-description mismatch timeout, still-enabled disabled-assertion mismatch,
still-visible hidden-assertion mismatch, missing, textless, and unfocusable
targets, focus preservation, remount/patch retention, and safe target removal.

This is not yet complete presentation automation. Multi-window ownership and
parenting policies, additional navigation modes, and additional state assertions remain future typed
operations; renderers must not emulate them with coordinates, arbitrary
scripts, or control-plane commands.

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
Automation properties. Ordinary action controls emit only their node ID;
parameterized actions generate controls from UI IR and emit a bounded typed
`submit` event after semantic revalidation. Neither path constructs domain
commands in the renderer. Its platform smoke mode renders the cross-language
fixture through the real control stack. The mounted tree applies all four patch
operations incrementally through a checked stable-ID index. A transactional
semantic candidate validates the final document before visual mutation, and
the compound fixture proves unaffected controls retain object identity. The
fleet document now declares a runtime-bound, revision-fenced Inspect action.
Rust lowers it to the shared `runtime.read` query plan; the renderer only emits
its stable node ID and cannot bind the action to another runtime. The
fleet root and history section now own bounded viewports backed by active
`VirtualizingStackPanel` instances, avoiding the unbounded outer-scroll layout
that defeats virtualization. Compiled-bound item view models now create direct
virtualized leaves only when their XAML binding enters the realized viewport.

`gewyvern_validate leserpent-accessibility` closes the renderer proof loop. It
audits every realized semantic control for a unique stable AutomationId, the
exact expected Automation Name and HelpText, and explicit labels on action
buttons. The same job enforces a 4.5 WCAG AA text-contrast floor across theme
pairs and retains one log per fixture plus a machine-readable summary. Managed
macOS and physical Linux/Xvfb runs produce identical counts; the macOS
NativeAOT shelf consumes the same audit parser.
The long-history fixture proves off-screen items remain unconstructed.
Heterogeneous container subtrees are likewise kept as stable-ID renderer models
until realized, while patches against an unrealized parent mutate only that
model. The bounded log document now passes a 48-entry sliding-window fixture;
26 log controls remain unconstructed after first layout. A typed debugger
document now proves `WaitingEffect -> Yielded` re-entry with a 40-frame sliding
window; 18 frame controls remain unconstructed after first layout.
