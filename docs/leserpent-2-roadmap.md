# Leserpent 1.0 To 2.0 Roadmap

This is the execution roadmap for the
[Leserpent 2.0 architecture](leserpent-2-architecture.md). The architecture
defines the invariant destination; this page defines ordered delivery gates.

The roadmap is capability-gated, not date-gated. A later gate may be prototyped
early, but it cannot become authoritative before its prerequisites are green.

Current implementation checkpoint: shared release `v1.17.4`. The
[project status tensor](project-status-system.md) remains authoritative for
per-feature maturity, evidence, dependencies, and next gates.

## 2.0 Scope Freeze

The core 2.0 capability set is closed. No new core capability family may enter
the `1.x -> 2.0.0` line before release. Remaining minor versions are allowed to
finish only the already-declared Leserpent/Gewyvern/Leselang/Avalonia control
loop: multi-daemon and multi-Gewyvern orchestration, reverse deployment,
renderer-neutral GUI automation equivalence, desktop Hub/sub-window operation,
authenticated remote control, persistence and recovery, security, performance,
packaging, documentation, and release evidence.

Accepted work after the freeze is closure work: bug fixes, reliability hardening,
security checks, benchmark improvements, packaging/signing/notarization proof,
cross-language conformance, status-tensor alignment, documentation, and removing
or simplifying accidental complexity. Rejected work is scope expansion: moving
Etragon into the release gate, claiming Windows native parity, adding another
runtime language, inventing a second GUI/control DSL, making GUI frameworks
automatically compatible, or requiring complete mobile parity beyond the already
declared minimum remote/mobile entry contract.

Implementation stack rule:

- control-plane authority is Rust-first.
- Leselang's core is crate-first: parser, HIR, VM, UI protocol, and future FFI
  host surfaces belong in Rust crates rather than a separate app VM.
- GUI framework support is adapter/codegen based. No GUI framework becomes
  compatible automatically; developers either implement a developer-owned
  adapter against the protocol standard or use a dedicated generator to produce
  a generated binding for that framework.
- all native shells and renderers are C# where applicable.
- the browser/operator web client remains TypeScript-only.
- no additional UI/runtime language is allowed to own canonical control-plane
  semantics (Node.js, Python, shell scripts, etc. are explicitly excluded).

## Baseline: 1.x Bridge

The current ASP.NET, TypeScript, SQLite, Orchestra, deployment, security, and
fleet behavior remains supported while Rust replacements are introduced.

Before migration:

- preserve the 1.x API and security tests
- capture command, authorization, persistence, and projection fixtures
- classify browser-only preferences separately from domain state
- prohibit new business logic in frontend event handlers
- record representative performance and deployment baselines

Exit: the current behavior is reproducible enough to compare with Rust.

## Gate 1: Domain And Protocol Kernel

Build `leserpent-domain` and `leserpent-protocol`.

- define IDs, revisions, principals, capabilities, commands, queries, events
- define `CommandEnvelope`, dry-run plans, confirmation, and result schemas
- establish canonical JSON fixtures and compatibility policy
- implement the first Rust projection and in-memory runtime
- adapt one read-only fleet route and one idempotent mutation end to end

Exit: GUI/API and Rust fixtures agree on normalized semantics.

`CommandPlan` and `PlannedOperation` now live in `leserpent-domain`, including
schema, operation/capability consistency, and embedded capability validation.
This keeps plan ownership below Leselang, CLI, GUI, protocol, and runtime
adapters.

The current compatibility policy and fixture boundary are maintained in
[`crates/leserpent-protocol/COMPATIBILITY.md`](../crates/leserpent-protocol/COMPATIBILITY.md).

## Gate 2: Leselang Frontend And VM

Build the language without UI-specific shortcuts.

The implemented language surface and its explicit limits are maintained in the
[Leselang language contract](leselang-language.md). That contract, rather than
future examples in this roadmap, defines what callers may use today.

The syntax boundary now includes a bounded canonical formatter with stable
two-space layout, supported string escapes, declaration-order preservation, and
deterministic parse-format idempotence in the retained fuzz shelf. Native CLI
source export consumes this formatter directly. Syntax-tree JSON validates
contiguous token coverage, EOF placement, UTF-8 spans, and bounded AST depth;
token access also remains panic-free after caller mutation. This syntax v1
contract is stable while HIR and VM contracts continue to evolve independently.

- lossless parser and first-class diagnostics
- HIR, lightweight static types, effects, and capability checking
- stackless evaluator with `Done / Effect / Yield / Cancelled / Failed / Fault`
- bounded execution, cancellation, timeout, retry, and deterministic merge
- versioned continuation serialization
- effect journal and exactly-once continuation consumption
- formatter, explain output, and model-oriented language guide

The execution protocol v1 rejects unknown continuation/effect fields,
unsupported program counters, standalone structured-group images, incoherent
effect/result-type pairs, and semantically invalid effect requests. Explicit
legacy query-request decoding remains covered for journal migration.

Current evidence covers the `runtime.list` vertical slice, including a bounded
SQLite journal, transactional sequence allocation, pending recovery, and
durable first-completion replay. The dispatch outbox now adds attempt-fenced
leases, crash redelivery, multi-VM exclusion, and transactional acknowledgement.
The first mutating effect, `runtime.refresh`, now carries one stable domain
idempotency key through crash redelivery and proves single domain commit with
result replay. Typed requested cancellation and trusted wall-clock deadlines now
atomically fence ready or leased dispatches and survive restart as replayable
terminal states. Bounded semantic retry now persists deterministic not-before
clocks independently from transport attempts and preserves command idempotency
across restart. Count-based journal retention now transactionally compacts only
the oldest terminal records in bounded batches, cascades acknowledged outbox
rows, and uses secure deletion without disturbing pending or leased work.
The deterministic merge kernel now normalizes out-of-order branch completions,
selects competing terminal outcomes in declaration order, bounds recursive
structured values, and persists merged output through the existing journal.
Source-level `all` now preserves named branch order in the lossless parser,
lowers branch-local types, and authorizes the union of branch capabilities. The
schema-5 journal now reserves strict merge-group and ordered branch records,
migrates legacy schemas, and rejects malformed recovery graphs. A shared
ephemeral/SQLite primitive now validates and atomically writes the merge plan,
every branch continuation and dispatch, and all declared-order links. Injected
mid-graph failure rolls the entire SQLite transaction back; committed graphs
recover every branch after restart. Branch terminal writes now finalize the
group in the same transaction when the last branch completes, persist the
declared-order merge result, and roll the branch write back if group completion
fails. Success and permanent-failure outcomes survive restart. Retention now
treats a completed merge graph as one logical unit: ephemeral and SQLite
compaction remove its group, links, branch effects, and dispatches together,
while pending or partial graphs remain untouchable. The batch limit counts
logical units, and each graph remains physically bounded to 64 branches. The VM
now starts one-level `all` as an ordered atomic effect batch with a stable merge
token. Non-final branch completion exposes durable waiting progress, the final
branch returns the declared-order aggregate, and SQLite restart resumes only the
remaining branches. Nested `all` remains deliberately rejected before sequence
allocation; the one-level structured-execution VM contract has exited Gate 2.

The `leselang-command` boundary lowers every current authorized runtime query and
mutation into frontend-neutral `CommandPlan` values, including bounded logs,
capability refresh, confirmed deployment, and VM-authority-only debugger
cancellation. The VM, CLI, and UI consume that crate rather than constructing
domain envelopes themselves. Parity tests cover every CLI-exportable Leselang
operation, while deterministic validated plan export remains local and requires
no daemon credentials. Debugger cancellation crosses the separately audited
`leselang-observe` boundary and is deliberately rejected by the ordinary daemon
command executor.

Exit: programs can suspend, restart, re-enter, and replay deterministically.

## Gate 3: Native CLI Parity

Build the Rust `leserpent` CLI on the shared protocol.

- query, inspect, plan, apply, watch, export, and history commands
- stable JSON mode and human-readable mode
- canonical Leselang export for every mutation
- dry-run and confirmation UX
- local IPC and authenticated remote transport

The first native slice now exists in `crates/leserpent-cli` and installs the
`leserpent` binary. It connects only through authenticated wire-v1 local IPC,
reads its token only from `LESERPENT_IPC_TOKEN`, validates owner-private socket
metadata before sending credentials, and supports `health` plus filtered
`runtime list`. Human output is control-character-safe; `--json` preserves the
versioned response envelope for automation. A real binary-to-daemon test covers
both modes. `runtime refresh` now requires explicit `--yes` for mutation, supports
non-mutating `--dry-run`, optimistic revision checks, stable caller-provided
idempotency keys, and local-only canonical Leselang export. A parity test parses,
lowers, and compares the exported command. Local `--export-plan` now emits the
same normalized refresh plan used for execution and requires an explicit stable
idempotency key. A real authenticated IPC and CLI/Leselang parity fixture now
covers the dedicated `runtime inspect` query. Runtime list and inspect now also
export canonical Leselang and validated plans locally through the same lowering
used for IPC execution. Bounded `runtime history` now returns newest-first domain
results through authenticated IPC, VM re-entry, canonical export, and a parity
fixture without exposing journal rows. Bounded `runtime logs` now follows the
same execution/export/plan path and returns at most 256 records. The native CLI now adds a bounded,
revision-deduplicated watch loop over the same normalized inspect plan, with
line-flushed human and JSON output. Local Gate 3 operation breadth is complete;
authenticated remote CLI transport remains under the Gate 6 transport boundary.

Exit: every migrated operation is executable through CLI and Leselang with the
same parity fixtures.

## Gate 4: UI IR And Avalonia Vertical Slice

Build the first replaceable GUI path.

- pure `State -> UiDocument` evaluation
- typed events and `UiEvent -> CommandPlan`
- stable node identity and incremental `UiPatch`
- Avalonia renderer with compiled bindings and virtualization
- fleet list, runtime child workspace, logs, and one debugger workflow
- GUI action inspection, dry-run, Leselang export, and audit correlation

Gate 4's renderer-neutral UI IR and replaceable Avalonia renderer contracts are
complete and stable. Formal Developer ID signing and notarization are
release-assurance evidence rather than renderer blockers; mobile client work
remains owned by Gate 6.

That statement covers adapter and renderer conformance, not product reachability.
The machine-validated [GUI function-chain matrix](leserpent-gui-function-chains.md)
now audits entry, semantic lowering, transport, authority, persistence,
projection, operator feedback, and Leselang equivalence separately. Its
2026-08-26 baseline scores the Avalonia product at 100, the Rust-hosted Web target
at 0, and the supported ASP.NET Web bridge at 100, for a combined target score
of 90. Avalonia now closes a daemon-owned native Orchestra workspace for strict,
revision-fenced plan discovery, Rust-authoritative automatic execution,
queued-only cancellation, lineage-bound retry, persisted run/event drilldown,
and idempotent runtime-scoped cleanup. Guided plans remain review-only rather
than claiming missing sidecar or session authority. The desktop also closes
existing-runtime registration and registration updates through strict daemon
inspection, side-effect-free plans, field-change invalidation, explicit
confirmation, and target-runtime revision fences. The live debugger session
bridge is now also closed: `leserpentd` owns a bounded journal-backed VM session,
stops at the first effect without executing it, returns the Rust-authored
debugger document over authenticated transport, and applies cancellation only
after dry-run review and explicit confirmation. The .NET plan remains bound to
its issuing client and principal, while the desktop refuses to abandon a
waiting session through its New action. Product-hosted Leselang
presentation automation is now closed as well: **Run live** receives typed
presentation operations from the daemon-owned Rust VM, applies them to the
current product window, and returns revision- and effect-bound outcomes for
durable VM re-entry. Continuations and source never cross the acknowledgement
boundary, rejections become visible terminal failures, and the desktop caps one
run at 64 effects. The per-daemon Rust Web console remains the explicit closure
work. Expired effects converge to a terminal revision and release the
32-session daemon registry; cancellation audit remains restart-durable inside a
bounded 64-journal retention horizon. Active debugger sessions remain
process-bound, so reconstruction is tracked as later VM-host resilience rather
than claimed by this product chain.

The first renderer-neutral slice now exists in `crates/leselang-ui`. It lowers
the typed fleet projection into a bounded `UiDocument`, resolves revision-fenced
typed events through the shared `CommandPlan` path, and computes deterministic
remove/insert/move/update patches over stable node IDs. Validation rejects
duplicate IDs, oversized or over-depth trees, unlabelled actions, stale events,
and actions rebound to another runtime. No endpoint, renderer, persistence,
transport, HTML, script, or adapter type enters the IR. A separate
`UiAdapterManifest` now records explicit developer-owned or generated framework
bindings against the document, event, patch, and complete sixty-two-atom
presentation protocol plus canonical atom family/effect profiles, so future GUI
hosts have a protobuf-style compatibility checkpoint without automatic framework
admission. The Rust-generated
presentation conformance fixture now carries both adapter modes into the
Avalonia/C# strict codec, proving cross-language manifest compatibility before a
renderer is trusted. The C# manifest codec accepts only named string enum
tokens, rejecting numeric binding kinds, presentation atoms, profile families,
or profile effects before validation can treat them as known protocol values.
Broader renderer and debugger
interactions are covered by the stable v1 contract and conformance fixtures. A
framework-independent
patch application reference now fences revisions and rejects malformed graph
edits; round-trip fixtures establish the semantic renderer conformance baseline.
The runtime child workspace now combines same-revision inspect and bounded
history projections into status, snapshot, refresh, and history nodes. Torn
state fails closed, endpoint data remains outside the IR, and history changes
apply incrementally. A separately bounded log projection now lowers sanitized
typed entries without adapter or endpoint fields and applies sliding windows
incrementally. The `leselang-observe` producer now validates bounded source
batches, preserves the newest sequence window, and sanitizes display text before
UI lowering without admitting endpoint or transport fields. SQLite schema 8 now
persists a 4096-record window per runtime behind an indexed sequence cursor.
Initial reads return the newest bounded window; incremental reads return only
records after the supplied cursor. The typed query requires `runtime.read` and
round-trips through authenticated Unix IPC without endpoint disclosure. The
first read-only
debugger document now models stackless synchronous
effect waiting and re-entry with bounded logical frames, sanitized summaries,
and no continuation token or local-value exposure. The `leselang-observe`
composition boundary now converts an authoritatively validated suspended VM
effect into that projection, rejects torn revisions, and proves that
continuation tokens, principals, capabilities, idempotency keys, and absolute
scheduler deadlines never reach serialized UI state. `DebuggerCancel` now uses
the shared command plan with `debugger.control`, revision/session fencing,
explicit confirmation, safe inspection, and a non-mutating dry-run. Confirmed
execution reaches the VM's durable cancellation path without returning the
continuation token. VM journal schema 6 atomically persists the requested
cancellation and its command/session/revision audit, rejects conflicting
principal-scoped idempotency reuse, and replays the original audit after a
restart. The public record omits continuation tokens and idempotency keys, and
is pruned with its retained continuation. The waiting debugger document now
declares a session-bound cancel action that lowers through the same shared
command planner. Rust and .NET reject session rebinding, and Avalonia renders a
destructive button while emitting only its stable node ID.
The product path now exposes that document from a shared `leserpentd`
`DebuggerAuthority` over local IPC and remote TLS. A strict source-generated
.NET client validates projection/document equivalence, and the per-daemon
Avalonia debugger workspace locks session coordinates after start, refreshes
the authoritative projection, performs a non-mutating cancellation plan, and
requires explicit confirmation before mounting the audited terminal state.
`debugger.cancel(session_id: ...)` is now also a typed Leselang effect with
`debugger.control`, explicit confirmation, a persisted command-correlated
dispatch result, and restart-safe re-entry. The renderer-neutral UI maps every
current action to HIR, exports it through the Rust canonical printer, and maps
the effect back to an equivalent stable-node event. Presentation automation now
includes `ui.activate(node_id: ...)`, `ui.focus(node_id: ...)`,
`ui.navigate_focus(node_id: ..., direction: "next"|"previous"|"first"|"last")`,
`ui.scroll_into_view(node_id: ...)`, and
`ui.assert_visible(node_id: ...)`, plus `ui.assert_hidden(node_id: ...)`,
plus `ui.wait_hidden(node_id: ...)`, plus `ui.assert_realized(node_id: ...)`,
`ui.wait_realized(node_id: ...)`,
`ui.wait_visible(node_id: ...)`,
`ui.assert_focused(node_id: ...)` and
`ui.wait_focused(node_id: ...)`, plus
`ui.assert_unfocused(node_id: ...)` and
`ui.wait_unfocused(node_id: ...)`, plus
`ui.assert_enabled(node_id: ...)`, plus
`ui.assert_disabled(node_id: ...)`, plus
`ui.wait_enabled(node_id: ...)`, plus
`ui.wait_disabled(node_id: ...)`, plus
`ui.open_window(node_id: ...)`, plus
`ui.close_window(node_id: ...)`, plus
`ui.assert_window_open(node_id: ...)`, plus
`ui.wait_window_open(node_id: ...)`, plus
`ui.assert_window_closed(node_id: ...)`, plus
`ui.wait_window_closed(node_id: ...)`, plus
`ui.set_selection(node_id: ..., state: "selected"|"unselected")`, plus
`ui.assert_selection(node_id: ..., state: "selected"|"unselected")`, plus
`ui.wait_selection(node_id: ..., state: "selected"|"unselected")`, plus
`ui.assert_child_count(node_id: ..., count: "0".."4096")`, plus
`ui.wait_child_count(node_id: ..., count: "0".."4096")`, plus
`ui.assert_text(node_id: ..., expected: ...)`, plus
`ui.wait_text(node_id: ..., expected: ...)`, plus
`ui.assert_automation_id(node_id: ..., expected: ...)`, plus
`ui.wait_automation_id(node_id: ..., expected: ...)`, plus
`ui.assert_node_kind(node_id: ..., kind: ...)`, plus
`ui.wait_node_kind(node_id: ..., kind: ...)`, plus
`ui.assert_action_kind(node_id: ..., kind: ...)`, plus
`ui.wait_action_kind(node_id: ..., kind: ...)`, plus
`ui.assert_action_label(node_id: ..., expected: ...)`, plus
`ui.wait_action_label(node_id: ..., expected: ...)`, plus
`ui.assert_action_available(node_id: ...)`, plus
`ui.wait_action_available(node_id: ...)`, plus
`ui.assert_action_unavailable_reason(node_id: ..., expected: ...)`, plus
`ui.wait_action_unavailable_reason(node_id: ..., expected: ...)`, plus
`ui.submit_form(node_id: ...)`, plus
`ui.cancel_form(node_id: ...)`, plus
`ui.assert_form_field(node_id: ..., field: ..., expected: ...)`, plus
`ui.assert_form_field_input_kind(node_id: ..., field: ..., kind: ...)`, plus
`ui.assert_form_field_required(node_id: ..., field: ..., state: "required"|"optional")`, plus
`ui.assert_form_field_max_length(node_id: ..., field: ..., max_length: "...")`, plus
`ui.assert_form_field_placeholder(node_id: ..., field: ..., expected: ...)`, plus
`ui.wait_form_field(node_id: ..., field: ..., expected: ...)`, plus
`ui.wait_form_field_input_kind(node_id: ..., field: ..., kind: ...)`, plus
`ui.wait_form_field_required(node_id: ..., field: ..., state: "required"|"optional")`, plus
`ui.wait_form_field_max_length(node_id: ..., field: ..., max_length: "...")`, plus
`ui.wait_form_field_placeholder(node_id: ..., field: ..., expected: ...)`, plus
`ui.set_form_value(node_id: ..., field: ..., value: ...)`, plus
`ui.assert_form_value(node_id: ..., field: ..., expected: ...)`, plus
`ui.wait_form_value(node_id: ..., field: ..., expected: ...)`, plus
`ui.assert_accessible_name(node_id: ..., expected: ...)`, plus
`ui.wait_accessible_name(node_id: ..., expected: ...)`, plus
`ui.assert_accessible_description(node_id: ..., expected: ...)`, plus
`ui.wait_accessible_description(node_id: ..., expected: ...)`: HIR and the VM keep each
operation in a distinct typed `ui.presentation` envelope, command lowering
rejects all sixty-two, and `leselang-ui` round-trips them against the current semantic
tree. Avalonia routes activation through exactly one native button click after
rejecting missing, non-action, unrealized, hidden, or disabled targets without
invoking domain callbacks. It applies native focus or bring-into-view, proves scrolling
preserves focus, performs sequential navigation through its native focus
manager from a currently focused stable action, resolves first/last through the
stable visual-index action boundary with native focus, returns the actual stable
destination, and proves next, previous, first, and last navigation, failure
focus preservation, and zero action activation. It checks visibility against
native layout and viewport state, positively asserts hidden state with the same
predicate without treating unrealized controls as hidden, waits for external
hidden transitions without scrolling or forcing realization and rejects
persistently visible targets with a bounded timeout,
checks realization directly against the native visual index without forcing it,
waits up to the protocol-fixed 2000 ms for natural realization while yielding
the native dispatcher, waits independently for viewport-aware native visibility
without scrolling, reads native focus, waits for external native focus without
invoking the focus primitive, reads effective enabled state, waits for external
native enablement without changing action availability, positively asserts
disabled action state without changing availability, waits for external native
disablement with the same fixed deadline while preserving availability and
action state, proves the target is attached to the same native window visual
tree without activating it, waits for that same native window membership with a
fixed dispatcher-yielding deadline, proves detached window-closed state without
closing anything, times out a persistently open window-closed wait without
mutating it, writes native selected/unselected state idempotently and reversibly
without action activation or focus movement, compares native selected state,
waits for native selection mismatch timeout without implicit selection changes,
compares actual native text, waits for external native text
transitions with a fixed dispatcher-yielding deadline, compares automation ID,
waits for external native automation-ID transitions with the same bounded policy,
and compares semantic node kind,
waits for external semantic node-kind convergence, compares semantic action
kind, waits for external semantic action-kind convergence, and compares explicit
semantic action label, semantic action availability, semantic form field label, input kind, and required state,
placeholder, maximum length, accessibility name, and declared accessibility help text exactly without
mutating the target, waits for external semantic action label changes without
clicking or enabling the action, and waits for external native accessibility-name
and HelpText transitions with the same fixed dispatcher-yielding deadline.
Parameterized form lifecycle is closed by distinct submit and cancel mutations.
Avalonia binds the semantic action to the currently open native form window and
its actual Submit and Cancel buttons, raises exactly one native click, and lets
the existing handlers own validation, confirmation, revision fencing,
deployment, and cancellation. Disabled, unrealized, disposed, or already-closed
forms reject the operation, and the presentation path cannot lower directly
into a domain command or semantic action callback.
Disabled, still-enabled disabled-assertion,
still-visible hidden-assertion,
selection-mismatched, selectionless,
text-mismatched, persistent text-wait-mismatched,
automation-id-mismatched, node-kind-mismatched,
action-kind-mismatched, form-field-mismatched,
form-field-input-kind-mismatched, form-field-required-mismatched,
form-field-max-length-mismatched,
form-field-placeholder-mismatched, persistent form-field-placeholder wait-mismatched,
accessible-name-mismatched, persistent accessible-name wait-mismatched,
accessible-description-mismatched, and persistent accessible-description wait-mismatched targets fail with typed native
presentation results. The full native window lifecycle now closes and reopens a
fresh Avalonia `Window`, proves duplicate open/close idempotency, requires
visible native state for open assertions, rejects a native close that remains
visible, and rematerializes controls from the same validated `UiDocument` and
stable node IDs instead of reparenting stale toolkit objects. The current
assertion/wait symmetry gap is closed; future presentation additions must be
driven by a concrete frozen-scope automation gap rather than inferred from
coordinate-level scripting or OCR.

The first concrete cross-language renderer core now exists under
`apps/leserpent-avalonia`. Rust emits a bounded versioned JSON fixture and the
.NET 10 adapter strictly deserializes it, mounts the previous semantic tree,
applies incremental operations, mirrors runtime/action validation, and proves
equality with the Rust next document at zero build warnings. The first Avalonia
12 desktop shell now maps every current semantic node kind to real controls,
preserves stable IDs and accessibility metadata through Automation properties,
and emits only action node IDs back across the renderer boundary. A platform
smoke mode initializes the real control stack, renders the Rust fixture, and
exits cleanly. The mounted control tree now consumes remove, insert, move, and
update operations through a stable-ID visual index after a transactional
semantic candidate validates the complete patch. The compound fixture proves
unchanged and moved controls retain identity instead of rebuilding the tree.
Fleet cards now declare a runtime-bound Inspect action alongside Refresh. The
Inspect event is revision-fenced and lowers in Rust to a frontend-neutral
`runtime.read` query plan; .NET validates the same runtime binding and renders
the action without constructing a query itself. The remote desktop now executes
that plan through the authenticated wire boundary: it composes Inspect, bounded
History, and bounded sanitized Logs only at the same revision, discards
endpoint-bearing wire DTOs before creating safe state, and opens one reusable
child window per runtime. Log append does not advance the control revision, so
the child exposes explicit Reload/F5 instead of pretending event-driven freshness.
Open workspaces refresh from newer live event revisions and share the fleet
window's mutation confirmation/fencing path. A fixed eight-window bound prevents
accidental connection and window fan-out.
Fleet roots now use an active `VirtualizingStackPanel` as the window viewport,
and history sections use independent bounded viewports rather than nesting
under an unbounded outer scroller. This establishes layout virtualization;
compiled-bound item view models now defer leaf-control creation until viewport
realization. A long bounded-history fixture retains unrealized items after the
window opens, proving off-screen controls are not constructed. Heterogeneous
container subtrees now remain as patchable renderer models until their parent
enters the viewport, so runtime cards no longer force eager descendant control
creation. The desktop shell now publishes through a checked NativeAOT profile
with a single pinned runtime/compiler/linker patch set. macOS arm64 and a
physical Ubuntu x86_64 host both produce five-file self-contained native
packages and pass all real control fixtures without a managed runtime
installation. The Linux debugger fixture preserves the one-to-zero
cancel-control lifecycle under Xvfb. macOS/Linux regressions remain the desktop
priority. That application, profile, lifecycle, and release-bundle paradigm is
now stable enough for Android entry and adaptive-layout work to proceed under
Gate 6; Apple release signing remains an independent release-assurance gate.
Windows native desktop is deferred and Windows operators use the authenticated
TypeScript web console in this cycle.

The Avalonia renderer now maps bounded log entries to lazy monospace controls.
The 48-entry cross-language sliding fixture applies in three operations and
leaves 26 off-screen controls unconstructed after first layout.

The same renderer maps debugger logical frames lazily. A 40-frame fixture moves
from `WaitingEffect` to `Yielded` in seven operations, removes the cancel
control, and leaves 18 frames unconstructed after first layout. Control smoke
evidence records one realized debugger-cancel button before the patch and zero
after re-entry.

The authoritative Rust diff now updates a working document as it emits each
operation and refuses to return unless that document converges exactly on the
target. This fixes invalid move indexes previously exposed by a sliding bounded
history window and reduces that transition from 34 operations to 3.

The no-argument desktop hub now has a local Rust-authority path. It supervises
an app-bundled `leserpentd`, creates private loopback TLS material and an
ephemeral local-process token, and reaches the normal remote window only after
the shared health client proves a ready owned authority. The process boundary
uses SIGTERM-first cleanup so journal ownership is released before immediate
restart; a real .NET-to-Rust verification command covers startup, TLS health,
shutdown, and restart. The macOS bundler fails closed when the arm64 daemon
payload is absent.
The Hub now implements the architecture's actual daemon topology instead of a
single remote-launch shortcut. A bounded private catalog atomically migrates the
legacy profile, derives stable IDs from normalized authenticated authorities,
and renders Local Orchestra plus every saved `leserpentd` as independent daemon
branches. The Hub remains open while authority-keyed session windows are reused;
different daemon sessions can run concurrently without sharing runtime state.
Managed trust pruning retains the full catalog CA set and the local authority.
A real Avalonia probe verifies the hierarchy and independent Open/Manage controls.
Each daemon branch now exposes a bounded direct runtime preview. A strict shared
RemoteClient query sends the canonical read-only `runtime_list` envelope,
validates unique runtime identities and revision ownership, drops runtime
endpoints before projection, and renders at most six gewyvern children per card.
The Hub limits topology reads to four concurrent authorities, cancels them on
close, marks live versus endpoint-bound cached evidence, and preserves manual
session opening when a preview is unavailable. The local-orchestra vertical now
proves this query against a real Rust `leserpentd`, not only a fixture codec.
Topology card lifecycle is now a renderer-neutral RemoteClient state machine.
The desktop refreshes all cards every 30 seconds with the existing four-query
concurrency bound and no same-card overlap. A failed refresh retains the latest
child tree as explicitly stale, rather than erasing useful evidence; live,
cached, retained, and unavailable states remain distinct, and revision
regression is rejected before rendering.
Live daemon identity is now stronger than a successful fleet read. Each refresh
performs strict `health` and `runtime_list` requests in parallel, composes them
into one topology snapshot, and refuses the live phase without a ready owned
protocol-v1 authority proof. Validated queue pressure is visible per daemon;
cached snapshots retain no fabricated health. The real local Rust-daemon
vertical proves this composition across the TLS process boundary.
Runtime preview rows now route directly to the existing per-runtime workspace
through their owning daemon session. The session is created or reused first,
then holds a bounded pending request until an authoritative event snapshot has a
`snapshot_revision` at least as new as the Hub topology revision. Feed heartbeat
revision is tracked separately and cannot release the workspace fence. A newer
snapshot that no longer contains the runtime rejects the request, and the shared
eight-workspace limit counts pending requests as well as open windows.
The supervisor also rejects symlinked state/daemon boundaries, performs no
ambient `PATH` daemon discovery, clears the inherited child environment, writes
all TLS material with atomic owner-only creation, and zeroes exported private
key buffers after the daemon identity is persisted.

The optional Team Silvortex account path now has a reviewed platform-owned
application/profile pair (`leserpent` / `leserpent_desktop`) and the static
secret-free native client `svx_client_leserpent_desktop`. A physical Linux
x86_64 disposable-provider run applies the real migration and passes the full
21-check identity workflow, including exact redirect validation, PKCE S256,
RS256 ID-token verification, MFA assurance, UserInfo subject binding, refresh
rotation/replay containment, and consent revocation. The retained proof is
`docs/fixtures/leserpent_silvortex_oidc_provider_shadow_linux_x86_64_20260810.json`.
Gate 4 still requires a release-facing desktop run through the system browser
and platform credential vault; account identity remains separate from every
endpoint-bound `leserpentd` authority.

The desktop now provides a fail-closed
`--prove-silvortex-account <absolute-output.json>` runner for that remaining
gate. It is restricted to the reviewed client, fixed callback, HTTPS issuer,
and packaged NativeAOT executable; it refuses an existing account credential
instead of overwriting it. The runner reuses the production login, fresh-session
restore, refresh rotation, and local logout paths, then atomically writes a
private identity-free result. `--verify-silvortex-account-proof` exercises the
writer, ordering, overwrite, linked-directory, and incomplete-proof fences
without contacting an identity provider. The next step is execution against
the provisioned Team Silvortex issuer on packaged macOS, followed by Linux
Secret Service parity, rather than another simulated provider claim.

The remaining macOS proof is now operational from Finder/Dock packaging rather
than depending on a shell launcher. `gewyvern_leserpent_bundle` embeds the
reviewed public issuer under a strict optional `Info.plist` key, the release
preflight validates the same canonical HTTPS-origin contract, and Avalonia uses
that package value with reviewed client/callback constants. Packaged execution
rejects every account-related environment override, while omission leaves the
optional account disabled. The proof runner requires this package source on
macOS and records only the public configuration-source class, never the issuer.

Exit: the vertical slice contains no direct adapter or persistence access and
passes GUI/CLI/Leselang equivalence tests.

## Reverse Deployment Bootstrap (Cross-cutting Before Gate 5)

The control path for remote hosts is now first-class: operator credentials are
used to bootstrap a target-host `leserpentd`, then all subsequent mutation uses
the target-issued `leserpentd` session credential.

The slice includes these required behaviors:

- bootstrap input is a non-session bootstrap token and optional bootstrap endpoint;
  the input is bounded, logged (redacted), and cannot imply any immediate runtime
  mutation authority;
- bootstrap result is a managed `leserpentd` service identity, endpoint, and
  session credential handle that can be promoted to a saved connection profile;
- client entrypoint prefers saved profiles, opens a logical daemon session, and only
  then exposes runtime fleet projections for that daemon;
- `runtime.register` and `runtime.deploy` in that daemon use the same envelope,
  confirmation and revision semantics as local execution, with the active session
  identity treated as the authoritative `daemon_id` binding.

The model explicitly forbids crossing from bootstrap to mutation without an
explicit confirm/fence handoff. A control attempt that still uses bootstrap-only
proof must be rejected before adapter dispatch. After the first accepted session,
bootstrap inputs become non-authoritative metadata.

Failure behavior is part of the slice:

- bootstrap transport failure writes bounded bootstrap fault evidence without
  creating remote runtime authority;
- partial bootstrap artifacts are treated as disposable and do not auto-mark the
  host as trusted;
- if bootstrap succeeds but session establishment fails, status remains
  `Bootstrapped` but no mutating intent is accepted until a valid session
  binds in;
- if session establishment succeeds but `runtime.deploy` is denied, the daemon
  session remains readonly for inspected operations and returns signed rejection.

The first bootstrap kernel is implemented in Rust. `leserpent-domain` owns a
validated `Planned / Deploying / Bootstrapped / SessionBound / Failed` state
machine, bootstrap-only capability and confirmation checks, principal binding,
opaque vault credential handles, daemon/session identity matching, and the rule
that only `SessionBound` permits runtime mutation. `leserpent-protocol` owns a
separate strict 64 KiB bootstrap wire-v1 envelope rather than adding bootstrap
proof to the ordinary daemon command channel. Unit tests cover successful
handoff, pre-session mutation denial, transport failure cleanup, identity
confusion, unknown secret fields, raw-secret handles, and malformed response
state.

The native Rust SSH transport is now implemented in `leserpent-adapters`. It
requires an exact host policy and pinned SHA-256 host key, resolves bootstrap
and session credentials through separate vault handles, transfers the bounded
installer through SFTP, verifies its read-back digest and private executable
mode, and accepts only a bounded typed installer response matching the planned
bootstrap, daemon, and endpoint identities. It never invokes a system `ssh`
binary, shell script, Node, or Python. Transport and policy tests prove that
missing policy, wrong credential provider, host-key rejection, and remote
identity drift fail before session authority is granted.

The target-side native `bootstrap-install-v1` entry point is now implemented.
Its shared strict installer wire keeps the session token in zeroized private
storage and separates `installed` from `ready`. The daemon verifies its own
uploaded digest, commits an immutable generation with private file modes, and
atomically advances a non-symlink `current` marker. Unit tests cover digest
failure, symlink rejection, retained-token conflict, rollback preservation, and
idempotent replay; a subprocess proof executes the real entry point and confirms
that stdout contains no token. The SSH adapter rejects the `installed` state.

Native launchd/systemd publication and activation, timeout recovery, real SSH
cross-process ready proofs, CLI commands, and the Avalonia Hub flow are now
complete. This closes the 2.0 reverse-bootstrap scope on macOS and Linux. WinRM
is optional post-2.0 work when Windows becomes an active native target.

The next service prerequisite is also complete: each immutable generation now
creates and validates a self-signed endpoint TLS identity. The public CA PEM and
its content-bound SHA-256 return over the pinned installer channel, while the
private key stays mode `0600` on the target. Daemon startup can consume the
session token from a bounded private `--remote-token-file`, removing secrets
from launchd/systemd arguments and environments. Controller-side trust
persistence, native service activation, and authenticated health promotion to
`ready` remain. The installer now renders a retained mode `0600` launchd plist
or systemd unit inside the immutable generation. It references only private
files and state/log paths, never token text; idempotent replay rejects descriptor
tampering. The descriptor is atomically published to the profile's native
service directory before `current` advances, with symlinked directories rejected.
Loading that descriptor remains a separate privileged step, so this preparation
still reports only `installed`.

A native service-manager activation primitive verifies `current` plus the
retained and published descriptor before using absolute launchctl/systemctl
executables with shell-free, secret-free argument arrays. Activation, rollback,
and retirement share one 30-second service-manager batch budget; a timed-out
manager child is killed and reaped before the installer returns failure. The SSH
production path now calls `bootstrap-activate-v1`: after activation, an
eight-second bounded probe connects through loopback while validating the
requested TLS server name, generated CA, private session token, wire-v1 health
payload, and daemon-owned authority. Only that complete path returns `ready`;
`bootstrap-install-v1`
continues to stop at `installed` for safe preparation and process testing.

Controller trust retention now has an independent native boundary. The SSH
outcome carries the validated CA only as far as `FileBootstrapTrustStore`, which
parses it as a rustls root, binds endpoint and digest, and atomically commits a
private record without following symlinks. The draft domain/wire schema exposes
only a `vault:leserpent-ca:*` handle. Persistence must succeed before the state
can become `Bootstrapped`; failure clears all authority handles. A real-host
proof remains.

The Rust CLI now completes that consumption path. Remote options select exactly
one CA source: an explicit PEM file or a bootstrap trust root plus
`vault:leserpent-ca:*` handle. Handle resolution revalidates the private record
and requires exact endpoint identity before constructing the existing rustls
transport. Existing authenticated HTTPS, IPC, and Leselang parity verticals
remain green. Avalonia now retains the same opaque handle in its connection
profile, strictly decodes the private Rust record through RemoteClient, rejects
endpoint/digest/source confusion, and imports only the validated PEM into its
content-addressed CA store.

The Linux real-host gate is now closed. An ignored, explicit-environment Rust
vertical uploads a size-optimized x86_64 artifact through the production Russh
and SFTP adapter, activates a systemd-user daemon, proves TLS/token/authority
health, commits private controller trust, and keeps mutation disabled until the
matching session and trust identities bind. The same run proves real trust-store
rejection, one-millisecond timeout cleanup, and occupied-port health rollback.
The failed daemon leaves no unit, generation, or staging artifact, while the
primary daemon remains active with zero restarts. Evidence is retained in
`docs/fixtures/leserpent_real_ssh_bootstrap_20260722.json`; credentials and PEM
are deliberately absent.

Controller restart durability is also implemented. The daemon worker converts
only a validated terminal bootstrap outcome into a private schema-v1 checkpoint,
and runtime SQLite schema 12 commits that checkpoint in the same transaction as
effect completion. Restart leaves `Bootstrapped` read-only. The legacy internal
direct-enqueue proof reaches terminal revision 1 and binds at revision 2; the
production submission path starts at revision-1 `Planned`, reaches terminal
revision 2, and binds at revision 3. In both paths a mismatched proof preserves
the current revision, while matching daemon/session/trust authority retires the
bootstrap handle and survives another restart. Malformed adapter output is
terminally rejected without creating or advancing authoritative handoff state.
Unit coverage lives in `crates/leserpentd/src/lib.rs`.

Authenticated checkpoint query and bind-session operations are now available on
the shared IPC/HTTPS dispatch and in the native CLI. Query returns only the
public snapshot. Bind requires explicit confirmation and only a bootstrap ID;
client-supplied authority booleans, daemon identities, handles, and secrets are
unknown fields. A default-off server verifier resolves the private session and
trust handles, checks exact endpoint binding, and proves remote TLS/token health
before the runtime may publish `SessionBound`. The packaged daemon enables this
resolver with `--bootstrap-trust-root` and its platform secret store.

The bootstrap origin is now packaged. `leserpentd` accepts a private, bounded,
strict schema-v1 `--bootstrap-config` only in native-SSH builds, requires the
existing `--bootstrap-trust-root`, loads a non-writable executable artifact,
rejects duplicate daemon/session/trust identities, and registers
`SshBootstrapAdapter<NativeSshBootstrapTransport>` with the same platform
secret service and controller trust root used by session verification. Official
Avalonia Linux publishing and documented macOS bundling enable the feature.

The independent authenticated submission route is also complete. HTTPS uses
`POST /v1/bootstrap`; Unix IPC requires an explicit `bootstrap_v1` route and
never guesses from a failed ordinary-wire decode. Submission is origin-gated
and atomically persists both its effect and revision-1 `Planned` checkpoint,
with idempotent replay and divergent-identity rejection. Worker completion
advances to revision 2 in the same transaction as effect settlement. The native
CLI now exposes `bootstrap deploy ... --yes` with only a target and
`vault:ssh:*` credential handle, plus the existing inspect and bind commands.
Avalonia Hub now exposes the same authority-scoped sequence through native
controls: explicit deployment confirmation, independent `/v1/bootstrap`
submission, bounded handoff polling, and phase-gated server-verified binding.
The locally managed authority now also promotes a bound receipt into the saved
connection catalog only after endpoint-bound trust loading, Rust-compatible
session-handle resolution, and live target health proof. Remote authorities do
not export their local trust stores. Promotion deliberately does not perform
global CA collection: Hub lifecycle owns the complete saved-daemon plus Local
Orchestra retention set. Its bounded CA store validates retained fingerprints,
canonical bytes, and the complete directory snapshot before any stale entry is
deleted, so malformed or over-budget state cannot cause a partial prune.

The post-session runtime path now has a separate domain/protocol foundation rather
than overloading `runtime.deploy`. `runtime.provision` models confirmed native
installation, authenticated service readiness, installation-credential retirement,
and identity-bound runtime registration. Its independent strict 64 KiB protocol
rejects unknown fields and raw credentials. `runtime.deploy` remains only the
debugging-pipeline submission operation for an already registered Gewyvern endpoint.
Runtime SQLite schema 13 now provides shared, kind-scoped authority checkpoints.
It atomically queues provisioning with revision-1 `Planned`, settles installation
to `ServiceReady` or `Failed`, restores both after restart, retires the installation
credential at readiness, and revision-CAS gates `RuntimeRegistered`. Existing
schema-11 daemon bootstrap checkpoints migrate losslessly into the same storage
without sharing operation identities. Authenticated HTTPS now uses only
`POST /v1/provisioning`, Unix IPC uses only `provisioning_v1`, and both remain
disabled until a dedicated adapter is registered. The daemon validates adapter
identity and terminal phase before atomically settling `ServiceReady` or `Failed`;
restart retains the public service authority without restoring the install secret.
The internal Gewyvern installer wire is now separate, strict, 64 KiB-bounded,
secret-redacted, and request/ready-response identity bound. It distinguishes
`Installed` from health-proven `Ready`, validates the artifact generation and
public CA digest, and refuses credential-handle substitution. The Gewyvern target
binary implements its `gewyvern-install-v1` preparation half: it digest-checks
the bounded source artifact before mutation, creates a private immutable runtime
generation, retains secret-free replay metadata and a service plan, generates the
endpoint TLS identity, rejects symbolic-link layouts, and atomically publishes the
current generation. It returns only `Installed`. Its `gewyvern-activate-v1`
entrypoint atomically publishes and activates a launchd/systemd descriptor, and
the descriptor starts `gewyvern-service-v1` without secret arguments. The managed
service exposes rustls HTTPS and requires its private API token even on loopback.
A symmetric 30-second service-manager batch deadline bounds activation,
restoration, and retirement, including child termination and reaping on timeout.
A bounded endpoint-name TLS/token health proof is required before `Ready`; failure
restores and restarts the prior generation. The host-key-pinned native SSH
transport now shares bootstrap's Rust connection, exclusive SFTP staging, bounded
command, and timeout-cleanup substrate. A strict private daemon origin config
selects targets and opaque handles; API tokens resolve only through the platform
store. `Installed` withholds trust and a receipt, while `Ready` persists its
endpoint-bound CA before returning authority. The activation path has also passed
a real Linux systemd proof with correct-token HTTP 200, wrong-token HTTP 403, and
post-proof service/unit/runtime cleanup. Daemon settlement derives the
authority-owned registration proof from that receipt and commits effect
completion, runtime registration, and the revision-3 `RuntimeRegistered`
checkpoint in one SQLite transaction. Runtime identity conflict is rejected
before dispatch, lost leases roll the entire registration transaction back, and
legacy revision-2 Ready checkpoints promote safely after restart. The native CLI
now exposes `runtime provision` with explicit `--yes`, an operator-owned
provisioning ID, `vault:ssh:*` handle, authenticated IPC/HTTPS transport, and an
optional bounded `--wait` phase loop. Reusing the same ID is an idempotent replay;
new attempts require a new ID. Human and JSON progress surfaces omit installation
secrets, while protocol failure, terminal provisioning failure, and observation
exhaustion remain distinguishable to automation. Avalonia now provides the same
confirmed authority-scoped operation from the Hub. Its native workspace locks
the complete provisioning identity after submit, renders every bounded phase,
caps automatic observation at 30 requests, and reuses only the exact request for
manual refresh. Failed attempts remain immutable and the UI explicitly requires
a new provisioning ID after remediation. A configured private
`LESERPENT_GEWYVERN_PROVISIONING_CONFIG` also lets the managed Local Orchestra
own this operation without weakening the remote authority contract. The next
product slice implements the now-defined remote retirement contract without
changing `runtime.deploy` semantics. The independent `runtime.retire` domain and
strict 64 KiB wire bind a new retirement ID to the original provisioning/runtime
identity and an explicitly confirmed opaque SSH handle. They require a proven
`ServiceRetired` boundary before `RuntimeUnregistered`; any external retirement
failure keeps the runtime registered instead of creating an unmanaged live
service. The durable runtime slice now atomically completes the leased effect,
journals replayable runtime unregistration, and commits revision-3
`RuntimeUnregistered`; lost leases preserve both the planned checkpoint and live
registration. Adapter-gated daemon submission and worker settlement now enforce
the same identity and terminal-phase checks, while transport secrets are
resolved only at the adapter boundary. Forged receipts leave the revision-1
checkpoint and runtime registration intact. The native target/SSH path is now
present: the shared origin registers provisioning and retirement adapters,
uploads the validated artifact over pinned SSH, invokes the strict
`gewyvern-retire-v1` wire, and accepts only a fully bound receipt. The target
verifies its private manifest and descriptor, persists a two-phase recovery
marker, stops/disables the service, and removes only that runtime root. The
daemon now exposes the same typed request through authenticated
`retirement_v1` Unix IPC and `POST /v1/retirement` HTTPS routes. Each route keeps
the independent 64 KiB limit and remains disabled unless the production
retirement adapter is registered; a real TLS proof commits only a provisioning-
bound revision-1 checkpoint. The native CLI now provides the confirmed
`runtime retire` operation over both authenticated transports with stable
identity replay, bounded polling, credential-free progress, distinct terminal
exit codes, and a negative proof that adapter failure preserves registration.
The Avalonia Hub now provides the matching confirmed control with strict
provisioning/runtime/target identity fencing, locked fields, bounded replay,
credential-free status, and explicit failure-preserves-registration guidance.
Its strict 45-key specialist catalog covers all eight built-in languages; the
native verifier measures every layout, reprojects controlled progress without
changing retirement/provisioning/runtime identity, and proves that the bounded
observation ceiling cannot issue a 31st removal request.
The physical Linux stop/remove gate is complete. The retained native SSH test
provisions and health-checks an isolated systemd-user runtime, rejects a forged
provisioning identity, completes the corrected retirement, replays it
idempotently, and proves zero service, process, port, runtime-root, descriptor,
or staging residue. Its redacted evidence is
`docs/fixtures/leserpent_real_ssh_retirement_20260723.json`.

The daemon bootstrap path now has its own symmetric target-side retirement
kernel rather than borrowing Gewyvern retirement semantics. The strict,
independently bounded `bootstrap-retirement-v1` wire binds retirement,
bootstrap, daemon, generation, and profile identities. The native
`bootstrap-retire-v1` entry verifies the private current generation, manifest,
and published descriptor before service mutation; then a private
`retiring -> service_retired -> retired` marker makes stop and cleanup
restart-safe and replayable. It removes only the service descriptor, current
pointer, and executable generation while retaining state and logs for operator
recovery. Cleanup revalidates the bound manifest, current pointer, and
descriptor after service stop, so a crashed stale retirement cannot erase a
newly published generation. A real macOS process vertical proves install, retirement, cleanup,
private marker persistence, and idempotent replay. Native SSH submission and a
physical Linux cross-host retirement proof are now complete. The native
deployment outcome returns the validated generation, then the retirement
transport uploads the same bounded native
artifact through the pinned Rust SSH/SFTP transport under an operation-specific
staging path, and invokes `bootstrap-retire-v1` with the independent bounded
wire. The physical systemd-user proof rejects a forged generation before
mutation, accepts the bound retirement, accepts an exact replay, and audits zero
unit, process, listener, descriptor, current-generation, or staging residue.
State, logs, and the private terminal retirement marker remain available for
operator recovery. Redacted evidence lives in
`docs/fixtures/leserpent_real_ssh_bootstrap_retirement_20260727.json`.
The target installer also rejects any generation already bound by its bounded
private retirement index, and terminal replay refuses success if a generation,
current pointer, or descriptor has reappeared.

The privileged Linux profile has now passed the same physical gate under
system-wide systemd. Controller policy accepts only `user` or `system`; the
system path invokes the validated native staging binary through the fixed
noninteractive `/usr/bin/sudo -n --` prefix and never supplies a sudo password
or secret argv. The proof used a temporary target rule restricted to the
bootstrap staging prefix and activate/retire actions, then verified the system
unit, process, listener, staging files, and test identity residue were absent,
removed the rule, and confirmed passwordless sudo was denied again. Redacted
evidence lives in
`docs/fixtures/leserpent_real_ssh_system_profile_retirement_20260728.json`.

The controller handoff now durably preserves the installer-validated generation
and policy-bound install profile through `Bootstrapped`, restart recovery, and
`SessionBound`. New worker settlements fail closed if either authority value is
missing, while old checkpoints with neither field remain readable but cannot be
used for generation-fenced retirement. The Avalonia AOT projection applies the
same paired validation, so future CLI and desktop retirement flows can derive
target authority from the checkpoint instead of accepting client-supplied
generation or profile values.

The independent daemon-retirement domain, public command codec, private effect
codec, SSH adapter, and durable scheduler path are complete. Public commands
cannot contain target, daemon, generation, or profile authority; planning
derives all four from a matching `SessionBound` deployment checkpoint. Runtime
journal schema 20 persists daemon retirement under its own authority kind,
atomically pairs planned checkpoints with effects, and atomically pairs terminal
checkpoints with scheduler outcomes. The worker revalidates the complete
response binding, restart replay is covered, and the production bootstrap origin
registers both deployment and retirement adapters. Explicit authenticated
`daemon_retirement_v1` IPC and `/v1/daemon-retirement` HTTPS routes are now
adapter-gated, bounded, operation-specific, and proven not to collide with
Gewyvern runtime retirement even when IDs match. The native CLI now exposes the
confirmed `bootstrap retire` operation with authority-omitting input, identical
IPC/HTTPS lowering, credential-free progress, bounded waiting, and distinct
protocol/failure/timeout exit codes. Avalonia now provides a separate confirmed
`Retire daemon` workspace with a source-generated strict codec, no
client-supplied derived authority, stable identity locking, credential-free
status, and at most 30 exact-request observations. Hub daemon and Gewyvern
lifecycle actions remain visibly separate. The privileged system-profile gate
is complete; WinRM is explicitly outside the 2.0 evidence gate.

Exit: one positive and one negative proof case exists for each branch:
bootstrap failure, bootstrap success + session connect success, and deploy path
without confirmed transition. Proof evidence must be versioned, reproducible, and
bound to the `leserpent-2-architecture` intent.

## Gate 5: Durable Runtime Cutover

Move authority from the compatibility bridge into `leserpentd`.

The initial `leserpent-runtime` slice validates and executes shared
`CommandPlan` values and now persists runtime registration plus mutating plans
in one ordered SQLite journal. Restart replay rebuilds projections, completes
pending commands, seals terminal command failures, and rejects divergent stored
outcomes. Journal records and payloads are bounded; the database is private and
opened without following links. The journal now transactionally migrates v1 to
v2, records migration history, validates the claimed schema shape, and preserves
legacy replay order.

The cutover now includes typed `RuntimeRegister`,
`RuntimeRegistrationUpdate`, and `RuntimeDiscoveryIntake` commands over the
shared wire path. All are capability- and confirmation-gated, idempotent,
secret-free, strict about unknown command fields, durable across daemon restart,
and intentionally produce no external effect. Create rejects a runtime
revision; update and discovery intake require the exact current runtime
revision. Discovery intake accepts only validated successful capability and/or
status observations, applies them atomically, and records the revision to which
capabilities were bound. Canonical endpoint conflict handling matches the 1.x
identity rule: scheme and host case plus default HTTP(S) ports cannot bypass
uniqueness, while path and query remain part of the target identity. Conflict
responses identify only the owning runtime and do not echo its endpoint.

The configured 1.x Web route now preserves the original Gewyvern capability
document as a typed, bounded authority snapshot while continuing to derive its
legacy presentation list. It queries the daemon revision before update, creates
missing legacy runtimes as an explicit reconcile step, submits registration and
typed discovery observations through private authenticated IPC, and only then
commits the managed compatibility projection. Sidecar discovery failures are
reduced to `sidecar_fetch_failed`, and runtime-status failures become
`runtime_status_fetch_failed`; pairing/admin tokens and raw error payloads never
enter the Rust command. An unconfigured development host retains the managed
fallback.

The first Web read cutover now routes runtime list, runtime detail, and runtime
status through strict typed daemon `runtime_list` / `runtime_inspect` queries
when IPC is configured. Daemon name, endpoint, tags, status, and observed
capabilities and the secret-free sidecar endpoint override managed copies.
Journal-derived registration/update timestamps now override managed copies when
present, survive restart, and do not advance on idempotent replay. Sidecar
status and its bounded memory summary now share the revision-fenced durable
projection. Registration, individual refresh, recovery, Fleet refresh, and
Orchestra recovery compose their available observations into daemon authority
before writing compatibility copies. The shared runtime-status validator covers
both direct intake and scheduler completion. Legacy snapshots without authority
timestamps or sidecar status retain per-field managed fallbacks, while
token-presence and fetch-only compatibility telemetry stay local. Daemon
configuration is now an explicit read-authority cutover: managed-only runtimes
are omitted from lists and return `runtime_not_found` from detail reads. A
daemon-only runtime still fails closed because the adapter cannot safely invent
the missing 1.x metadata.
Unknown projection fields, including secret-shaped fields, are rejected. The
current slice now moves attention, protocol-reading, and recovery reads onto
this shared projection; sidecar status travels inside the projected runtime,
and the dedicated sidecar-detail route now uses it directly. Fleet summary,
attention-list, and attention-summary GETs compose from the same authoritative
runtime set;
managed recovery history remains a bounded metadata overlay, and projected
sidecar status participates in attention classification. Orchestra plan GET,
execute, retry, and session handoff share one authoritative plan projection, so
the displayed revision is the revision validated by the command path.
Per-runtime Orchestra history GETs authority-check membership before reading
durable history. Cleanup-plan GET and all matching delete routes now build from
the same daemon-authoritative runtime projection. Plan-token v2 also binds the
managed session IDs that would be removed, and deletion reservation atomically
rechecks target and session membership before persisting an intent. Empty plans
do not issue authority mutations. An internal command execution context now
combines daemon-owned runtime identity, endpoint, sidecar endpoint, membership,
and revision with local managed credential slots. Deployment, protocol reading,
individual refresh and recovery, Fleet refresh, and Orchestra recovery all use
that context; Fleet no longer enumerates managed membership. Deployment and
discovery commands carry the captured expected revision, command responses keep
daemon identity, and Orchestra submits its composed observations in one intake.
Secrets remain outside read projections, API models, diagnostics, and durable
history. Discovery intake now returns a typed receipt with the applied runtime
revision and strict runtime projection. The shared command-context coordinator
uses that exact command result for compatibility refresh writes and responses,
so no post-command query is required and managed credentials never enter
authority state. Registration now returns one typed commit receipt spanning the
registration command and optional discovery intake. It strictly verifies the
command ID, command-result identity, and envelope/projection revision
coherence when the redundant envelope field is present. The strict runtime
projection remains the revision authority. Registration uses that revision to
fence intake, whose receipt receives the same checks, and binds the initial compatibility write and
response to the final daemon projection without a post-registration query.
Receiptless authority adapters fail closed; credentials and capability-fetch
telemetry stay local. The next
cutover is complete: configured registration plans come from one daemon
snapshot and expose the planned runtime ID, expected revision, and authority
kind. Plan-token v2 binds canonical runtime and sidecar target identity,
action, authority, ID, and revision. Create planning has no effects; it may
reuse an unmigrated managed ID only as a migration hint when the daemon does
not own that ID, and deletion-reserved IDs are rejected. Registration requires
and rebuilds the daemon plan, then submits the reviewed revision directly, so
updates no longer inspect immediately before their command and credentials
remain outside plan state. The cutover is now consolidated behind one
registration execution coordinator. It validates request safety and the
rebuilt plan before effects, performs credential-bound discovery, consumes the
typed daemon receipt, writes the authority-bound compatibility projection, and
records recovery through one shared policy. Managed fallback uses the same
entry point, while the HTTP route only invokes the coordinator and maps its
typed secret-free failures. Registration command and idempotency identity now
use one versioned canonical encoding of the complete daemon command intent.
The hash covers action, runtime ID, reviewed revision, normalized runtime and
sidecar coordinates, and all tags, while its API cannot accept credentials.
Exact retries preserve identity, tags-only changes remain distinct, and the
same update at a later revision cannot collide with an older receipt. A real
Rust daemon vertical test proves both replay and later-revision rotation.
Control-plane schema v9 now durably records the sanitized command intent,
discovery observations, and attempt state before mutation. Credentials and the
review token cannot enter the record. One transport-ambiguous result is replayed
immediately with the same command ID and reviewed revision; a repeated
ambiguity remains pending across restart. Recovery planning bypasses the
daemon's advanced snapshot, reconstructs the original plan for an exact
request, and rejects overlapping or tags-different work until convergence.
Recovery uses fresh caller credentials but never repeats discovery, and clears
the record only after the typed receipt reaches the local compatibility state.
Schema migration initializes an empty queue, semantic validation rejects
tampered command IDs, and persistence import rejects unresolved intents. The
real process/socket gate now passes locally: an owner-private proxy drops two
post-commit registration responses, the first compatibility process is
force-killed, and a fresh process exactly replays the command before applying
the persisted discovery intake once with zero HTTP rediscovery. The same
entrypoint now passes on macOS arm64 and physical Linux x86_64, with the Linux
result retained as a secret-free fixture. Registration work now returns to a
preservation gate. A bounded process-local claim now owns every overlapping
name or endpoint before plan lookup, discovery, daemon mutation, or managed
fallback write. Concurrent retries in either mode fail with
`runtime_registration_in_progress`; only the winner binds credentials and
commits its projection, while an authority winner also clears the durable
intent and a delayed loser must review a fresh plan after convergence. Wire or
state evolution must keep this all-mode single-flight credential fence together
with exact replay, fresh local credential binding, and zero rediscovery across
process restart.
The registration/deletion lifecycle gate is now bidirectional. A reviewed
registration claims its planned or existing runtime ID under the same Registry
lock used by deletion reservation; deletion rejects both an active claim and a
durable pending registration. Managed and authority plans reject an existing
deletion intent, and managed commits internally bind the rebuilt plan token so
their final target cannot drift after discovery. The durable registration
intent continues the deletion fence across restart while process-local claims
cover in-flight work.
Cleanup and generic
unregistration now have an
explicit confirmed result contract: a daemon schema-v14 transaction fences all
target revisions, journals removal, deletes Orchestra history, and retains
idempotent operation results. The Web bridge holds a deletion reservation while
the daemon-first mutation and local compatibility cleanup run, so new sessions
and Orchestra runs cannot cross the deletion boundary. Token-presence remains
inside the local secret boundary. Control-plane state schema v3 persists that
deletion intent before daemon mutation and records only bounded per-intent
attempt metadata plus a closed safe failure code. Schema v1/v2 snapshots
upgrade without synthetic attempts. A restart restores the protected target
set, rejects new sessions and Orchestra runs, and a bounded background worker
replays daemon unregistration plus local cleanup until the intent converges.
Schema v1 state upgrades in memory with an empty intent set. Imported snapshots
cannot inject pending destructive work. A real Arm64 Unix run now starts the
production Rust daemon and a separate C# harness, waits until daemon
unregistration commits, force-kills the harness, and proves restart convergence.
Its retained evidence is
`docs/fixtures/leserpent_runtime_deletion_crash_20260723.json`; reproduce it with
`scripts/validation/leserpent_runtime_deletion_crash.sh`. The same script has now
passed on the physical Ubuntu x86_64 host, retained as
`docs/fixtures/leserpent_runtime_deletion_crash_linux_x86_64_20260723.json`.
A repeated fault campaign now covers the intent-persisted, daemon-committed,
and local-cleanup-persisted transitions. Each iteration starts the production
Rust daemon with an independent C# host, force-kills that host at the selected
boundary, reconstructs the formal registry from disk, and waits for background
recovery to remove both daemon and compatibility state. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_fault_campaign.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 aggregates live in
`docs/fixtures/leserpent_runtime_deletion_fault_campaign_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_fault_campaign_linux_x86_64_20260723.json`.
The completed interference gate adds concurrent normal registration and
state-save traffic while this repeated recovery campaign is running. It runs
eight unrelated registrations per crash
scenario: before daemon mutation, after daemon commit, and racing local cleanup.
Every runtime must survive in the live compatibility registry, a fresh disk
reload, and the production Rust daemon. Reproduce the platform aggregate with
`scripts/validation/leserpent_runtime_deletion_concurrency_campaign.sh`.
Retained Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_linux_x86_64_20260723.json`.
The controlled daemon-restart gate now stops `leserpentd` with `SIGTERM`,
observes one real offline recovery failure, reopens the same SQLite database,
and requires the next claim to converge while the concurrency workload remains
active. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_daemon_restart_campaign.sh`;
the Arm64 Unix and physical Ubuntu x86_64 aggregates are retained in
`docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_linux_x86_64_20260723.json`.
The unclean daemon-takeover gate now `SIGKILL`s the production daemon at every
durable deletion boundary, verifies that pre-expiry replacements are rejected,
and waits for the fixed 30-second owner lease to expire naturally before
reopening the same database. Recovery then converges under concurrent
registration and state-save traffic. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_unclean_takeover.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 latency evidence lives in
`docs/fixtures/leserpent_runtime_deletion_unclean_takeover_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_unclean_takeover_linux_x86_64_20260723.json`.
The overlapping-intent gate now persists three independent intents at the
intent-only, daemon-committed, and local-cleanup-persisted boundaries in one
state image. One host termination and one unclean daemon takeover must release
and retry every failed claim independently while preserving concurrent normal
traffic. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_overlapping_takeover.sh`;
retained Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_20260723.json`
and
`docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_linux_x86_64_20260723.json`.
The repeated-takeover gate now interrupts recovery after one intent commits its
daemon mutation, kills the replacement daemon, and requires that intent to
finish local cleanup while the other two observe a real second outage. The
remaining claims must release and converge only after a second natural lease
takeover. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_repeated_takeover.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_repeated_takeover_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_repeated_takeover_linux_x86_64_20260723.json`.
The poison-isolation gate now makes the oldest pending intent fail for at least
three recovery passes while later intents continue against the production
daemon. The poison reservation must remain protected across disk reload, and
removing the scoped failure must converge the original intent without state
editing. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_poison_isolation.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_poison_isolation_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_poison_isolation_linux_x86_64_20260723.json`.
The high-cardinality gate now runs 32 independently durable intents with four
evenly spaced poison targets. The first recovery pass converges all 28 healthy
intents, retains only poison reservations, and records retry-window timing
before reload and repair. Reproduce it with
`scripts/validation/leserpent_runtime_deletion_high_cardinality.sh`; retained
Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_high_cardinality_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_high_cardinality_linux_x86_64_20260723.json`.
The original serial first-pass baselines were 6460 ms and 7628 ms. Recovery is
now bounded to 32 claimed intents, eight concurrent authority mutations, and 64
daemon IPC connections per worker tick; successful local convergence is
committed with one strict batch save. The optimized first-pass measurements are
158 ms and 248 ms, with every isolation and durability check retained. The next
gate is also complete: `scripts/validation/leserpent_runtime_deletion_batch_persistence.sh`
commits two real daemon mutations, forces the strict local batch save to fail,
and proves complete in-memory rollback, durable reservation protection, paced
idempotent daemon and Orchestra cleanup replay, and next-pass convergence.
Retained Arm64 Unix and physical Ubuntu x86_64 evidence lives in
`docs/fixtures/leserpent_runtime_deletion_batch_persistence_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_batch_persistence_linux_x86_64_20260723.json`.
Both platforms replay each intent exactly once and converge in 1271 ms and
1289 ms. The saturated-queue gate is now complete:
`scripts/validation/leserpent_runtime_deletion_saturated_queue.sh` fills all 128
durable slots, saturates all eight authority workers, and proves 1-2 ms
cooperative shutdown without losing intents or claims. Under 17 slow targets
and eight poison intents, deferred poison is filtered before the claim limit.
Both platforms follow the same four-pass 98/68/38/8 pending trajectory while
each poison spends one initial attempt. The 1/2/4/8/16/30-second capped
schedule, safe failure code, and next-attempt deadline survive disk reload;
the deadline rejects premature claims and repaired authority converges after
it expires. Operators can inspect the same metadata, submit a guarded
revision-fenced retry-now request, and inspect a bounded durable audit trail.
Request-ID replay remains idempotent after convergence, while stale revisions
and conflicting reuse fail closed. The recovery signal reduces post-command
repair to 55 ms on Arm64 and 162 ms on physical x86_64 Linux. The APIs are
`GET /v1/persistence/runtime-deletions`,
`POST /v1/persistence/runtime-deletions/{intentId}/retry-now`, and
`GET /v1/persistence/runtime-deletion-retry-audit`. Retained evidence lives in
`docs/fixtures/leserpent_runtime_deletion_saturated_queue_20260723.json` and
`docs/fixtures/leserpent_runtime_deletion_saturated_queue_linux_x86_64_20260723.json`.
The retry/claim race gate is complete. Eight worker-first rounds, eight
operator-first rounds, and 32 simultaneous-start rounds each submit eight
operator contenders against one recovery worker. Both Arm64 and physical
x86_64 Linux retain exactly 48 authority mutations for 48 runtimes, at most one
durable retry winner per round, matching audit counts, deterministic
in-progress/revision conflicts, and complete convergence after disk reload.
The retry acknowledgement crash gate is complete. On both Arm64 and physical
x86_64 Linux, the host is force-terminated three times immediately after the
retry revision/audit commit and three times after the real Rust daemon commits
unregistration but before local cleanup. All six scenarios restore the pending
intent and audit, preserve the runtime reservation, run one recovery authority
call, converge both authorities, and retain post-convergence request-ID replay.
The retry audit retention gate is complete. Both supported architectures drive
272 acknowledged retries through concurrent 128/128/16 operator and worker
waves. The latest 256 records survive in strict linearization order, every
runtime receives one authority mutation, retained IDs replay, evicted IDs leave
the replay horizon and can identify a new intent, and the final window survives
restart without pending starvation or loss. Arm64 completes in 6913 ms and
physical x86_64 Linux in 2310 ms. The atomic rollover persistence gate is also
complete on Arm64 and physical x86_64 Linux: each platform force-terminates
three runs before write, three after observing the real temporary file, and
three after commit. All 18 restarts contain exactly 256 ordered records and one
complete previous or replacement window, never a torn or reordered mixture.
The atomic backup refresh gate is complete: Arm64 and physical x86_64 Linux
each force-terminate nine runs, deliberately corrupt all nine primary files,
and recover all nine complete 256-record previous windows. Typed, secret-free
load provenance is complete as well: every fallback reports
`backup/recovered/invalid_json` through health, remains persistence-ready, and
is explicitly degraded but operable. The first post-recovery write gate is
complete too. Arm64 and physical x86_64 Linux each force-terminate nine repair
writes; every active state is one complete old or new 256-record window, every
known-good backup remains the complete prior window, and no corrupted primary
is copied into backup. The safety-critical semantic-generation gate is now
complete for runtime deletion intents and retry audit. Arm64 and physical
x86_64 Linux each reject and recover nine schema-compatible but semantically
invalid primaries, preserve all nine known-good backups, and never promote the
invalid generation. Runtime/session graph validation is complete as well:
runtime and session identities are stable and case-insensitively unique, every
session references a registered runtime, generated saves self-check, and
explicit imports fail before replacing the live projection. The next
reliability gate for legacy Orchestra identity and runtime references is also
complete. Duplicate run IDs are rejected before SQLite `ON CONFLICT` migration,
and orphan runs are rejected instead of disappearing through restoration
filtering. The next gate validates per-runtime request-ID uniqueness and retry
lineage invariants before legacy history reaches SQLite. That gate is complete:
request IDs follow the runtime-scoped SQLite replay identity, retained parents
prove terminal same-runtime/same-plan attempt succession, and retries whose
parents crossed the 32-run retention boundary remain valid. The next gate
validates legacy Orchestra lifecycle fields, completion timestamps, and step
payloads before restoration. It is complete: known outcomes, active/completed
consistency, monotonic bounded timestamps, stable plans, and a required
256-entry step envelope are enforced while old terminal records without
`completedAt` remain readable. The next gate extends semantic validation to
runtime/session payload timestamps, required text, and nested collections
before projection restoration. It is complete: runtime and session required
fields are canonical and bounded, lifecycle timestamps are monotonic and
non-future, capability and requirement keys are unique within 256-entry
envelopes, and nested runtime status/sidecar memory counters and slot identities
are validated before projection restoration. The next gate constrains nullable
diagnostic text and proves status-source coherence without exposing untrusted
failure details through persistence health or import errors. It is complete:
managed discovery converts arbitrary upstream errors into fixed diagnostic
codes before persistence, runtime and sidecar source/timestamp/error postures
are validated as closed sets, optional diagnostic text is bounded, and
control-plane/Orchestra health exposes stable failure codes while retaining
full exceptions only in local logs. The next gate bounds legacy Orchestra
operator, revision, summary, and event payload metadata before SQLite
migration. It is complete: a shared validator protects JSON restoration,
SQLite, daemon IPC, in-memory writes, and authority readback; operator and
revision fields plus step/event summaries are bounded and canonical; event
identity, outcome, and timestamps bind to their run; Rust and C# agree on the
256-step envelope; and authority read failure aborts before legacy replacement.
The next gate validates retained Orchestra event sequence continuity,
transition legality, monotonic EventIds, and terminal-run correspondence during
history reads. It is complete: legacy eventless runs receive a deterministic
origin, SQLite and in-memory candidates are validated before publication,
adapter reads reject broken identities, IDs, time, or transition links, and
the terminal event must correspond to the run. The next gate moves append
sequence validation into the Rust persistence authority transaction and proves
cross-process rejection before commit. It is complete: exact replay remains
idempotent, while new events validate the previous outcome, legal transition,
run/event target agreement, and RFC 3339 instant monotonicity inside the same
immediate SQLite transaction. Origin events cannot claim a predecessor,
terminal runs cannot be appended to, and a real authenticated Unix-socket
test proves an illegal transition is rejected without changing retained
history. The next gate validates retained Orchestra history rows inside the
Rust persistence authority and proves corrupted sequences fail closed across
the IPC boundary. It is complete: run and event envelopes are decoded through
a private minimal projection and checked against their SQLite columns; event
cardinality is bounded by the state machine; one read transaction validates
the complete sequence before pagination; and both direct corruption tests and
an authenticated Unix-socket test prove malformed retained data returns only
the stable history failure. The next gate validates each Rust-authority run
list page against retained terminal-event correspondence in a bounded batch
without N+1 queries. It is complete: each page and its lookahead row share one
parameterized event query capped by the three-event state machine; every run
is paired with a complete validated event sequence before publication; and
direct mutation plus authenticated IPC tests prove hidden lookahead
corruption fails closed. The next gate moves exact run/event
envelope-to-column validation into the Rust append transaction so malformed
native callers cannot persist poison rows. It is complete: the authority
decodes both minimal envelopes after opening its immediate transaction and
requires exact run, runtime, request, outcome, event type, source/target
outcome, and timestamp correspondence before any replay lookup or SQL write.
New event envelopes must carry the canonical zero EventId sentinel, and their
time must be compatible with the run execution/completion interval. A
field-by-field native regression proves every mismatch rolls back to zero run
and event rows before a valid append succeeds. The next gate validates the
existing retained run and complete event chain inside the append transaction
before accepting an idempotent replay or extending that history. It is
complete: the immediate transaction reads the retained run plus one bounded
event batch, checks SQL request identity and the complete three-event
state-machine sequence, and reuses the validated last event for transition
admission instead of issuing a weaker predecessor query. Corrupted
byte-identical replay, request-identity drift, and extension over a mismatched
predecessor all fail without mutation; direct SQLite injection and an
authenticated Unix-socket regression cover both native and cross-process
boundaries. The next gate extends SQL request-id/envelope coherence to every
run-history read, including pagination lookahead. It is complete: run-specific,
runtime-filtered, and global history queries all select the nullable SQL
request ID and pass it through the same retained-run validator used by append.
Every fetched row, including the `limit + 1` lookahead, must match its envelope
before event validation or publication. Direct SQLite tests isolate
request-column drift across all three read shapes, while authenticated IPC
proves both run detail and run list return only the fixed history failure. The
next gate validates the complete post-append run/event snapshot and binds the
returned persistence receipt to that validated transaction generation before
commit. It is complete: the authority replaces separate opaque run/event/count
read-back with one retained-run read, one complete three-event-bounded batch,
and one exact target-event identity read under the same immediate transaction.
The run and complete event chain must pass the shared semantic validators, the
target event must belong to that validated batch, and its creation generation
must equal the run update generation before the receipt is constructed.
SQLite trigger fault injection proves post-write column drift and generation
drift both roll back the entire append; authenticated IPC proves the same
generation failure remains a fixed non-disclosing persistence error. Exact
replay returns the run, event, and count from the same validated snapshot. The
next gate validates the complete bounded per-runtime retention set after
append, including eviction and cascade postconditions, before commit. It is
complete: append generations now advance monotonically beyond the retained
runtime maximum even when the wall clock moves backward, and an explicit
bounded plan identifies the exact 32 retained identities plus at most one
eviction. Before commit, the authority validates every retained run and its
complete event chain in batched reads, reconciles the runtime event count, and
requires both the evicted run and its cascaded events to be absent. Native
fault injection silently ignores the planned delete and proves the new append
rolls back to the original 32-run/32-event window. A deterministic future-time
tie proves the current run remains newest and retry converges to the expected
eviction. Authenticated IPC preserves the fixed non-disclosing persistence
failure and the same retry convergence. The next gate binds multi-runtime
Orchestra deletion receipts to a validated post-delete snapshot, including
complete cascade and unrelated-runtime preservation, before commit. It is
complete: one set-based transaction derives bounded counts for every targeted
runtime and every event attached through its run, rejects SQL ownership drift,
and still permits deletion of malformed opaque envelopes. After deletion, all
target run and event rows must be absent and the SQLite total-change delta must
equal exactly the returned run count plus cascaded event count. This exact
mutation budget rejects both silently ignored deletes and trigger writes to an
unrelated runtime, rolling the entire transaction back. The same helper now
protects explicit Orchestra deletion and runtime unregistration cleanup.
Native tests prove two-runtime receipts, unrelated byte-for-byte preservation,
zero-count retry, and unregistration journal rollback. Authenticated IPC proves
the stable non-disclosing delete failure and successful retry after repair. The
next gate validates durable runtime-unregistration replay receipts against the
operation request and live Orchestra tombstone before acknowledging replay. It
is complete: replay now decodes the persisted operation request into a bounded
unique typed target set, requires an exact canonical JSON round trip, and
validates receipt counts against both the target count and Orchestra retention
and event bounds. A single SQLite read transaction derives its target IDs from
that historical request and requires target runs, directly owned events, and
events attached through target parent runs all to remain absent before the
receipt can be returned. Native fault injection proves a reintroduced
Orchestra row, a non-canonical request, and an impossible receipt all reject
replay; repair restores idempotent convergence. Authenticated IPC keeps the
failure non-disclosing and proves the same repaired retry. The next gate binds
durable unregistration operation rows to their exact runtime-journal
tombstones and live projection absence before acknowledging replay. It is
complete: first commit reads the inserted operation row back and requires one
canonical, non-terminal runtime-unregistration journal payload for every
persisted target at the exact removal timestamp before commit. Replay performs
the same bounded multiset comparison, rejecting missing, mutated, duplicated,
completed, or failed tombstones, while the control layer independently requires
every target projection to remain absent. Snapshot compaction preserves these
unregistration records while still removing ordinary covered journal rows, so
two-generation maintenance cannot orphan a valid receipt. Native trigger fault
injection proves post-insert journal corruption rolls the operation, journal,
and Orchestra cleanup back together; direct corruption proves ambiguity and
projection drift fail closed. Authenticated IPC preserves the fixed failure and
repair convergence. The next gate introduces bounded retention and an explicit
replay horizon for operation-bound unregistration journal tombstones without
allowing compaction to orphan replay receipts. It is complete: lookup, commit,
and snapshot maintenance converge the durable operation set to the latest 256
SQLite insertion-linearized rows. At capacity, the oldest operation/journal
binding is validated before its receipt is deleted under an exact mutation
budget in the same transaction as the incoming unregistration. Its journal
tombstone remains available for state reconstruction until two retained
snapshots cover it. Compaction derives every sequence protected by the retained
window and deletes at most 1000 covered ordinary or unreferenced rows, so it
cannot orphan a replayable receipt or resurrect a runtime after restart.
Unregistration timestamps advance monotonically beyond retained tombstones,
preventing same-target ambiguity during command-ID reuse. Native rollover
tests prove trigger-fault rollback, pure-replay convergence from an oversized
legacy window, oldest-first eviction, outside-horizon ID reuse, deferred
tombstone cleanup, and restart replay. The next gate promotes unregistration
operation linearization from implicit SQLite rowid ordering to a schema-owned
monotonic generation and exposes retained replay-horizon metadata. It is
complete: schema v15 migrates v14 operation rows into their original order with
contiguous generations and persists `next_generation` plus
`evicted_through_generation` in a singleton authority row. New commits allocate
and horizon eviction advances this state in the same immediate transaction as
the operation and journal mutation. Eviction orders exclusively by generation,
uses an exact operation-delete plus high-water-update budget, and rolls the
incoming intent back on an ignored delete. Schema and runtime reads require a
contiguous retained interval bounded by the two high-water values. Native
migration, oversized-window, rollover-fault, reuse, and restart tests prove the
state transition. Authenticated daemon health publishes capacity, retained
count, oldest/newest generation, next generation, and eviction high-water;
Rust CLI and the strict Avalonia source-generated codec expose and validate the
same optional protocol-v1 extension. The next gate binds each successful or
replayed runtime-unregistration receipt to its durable operation generation so
clients can correlate a specific receipt with the advertised replay horizon.
It is complete: the native result copies the nonzero schema-v15 generation
from the validated operation row on both first commit and replay, preserving
the same receipt identity. Daemon IPC and HTTPS publish it as an optional
protocol-v1 field; legacy responses decode with explicit absence rather than
generation zero. The CLI renders the generation or `legacy-unknown`, the C#
compatibility authority rejects an emitted zero, and Avalonia classifies a
receipt against authenticated health as retained, evicted, or future. Runtime,
protocol, IPC, CLI, C# security, and cross-language conformance tests freeze the
correlation. The next gate adds a bounded typed receipt lookup by command ID so
clients can recover this correlation after losing a mutation response without
replaying the mutation. It is complete: the dedicated read request carries only
principal, `runtime.read`, and one command ID. The SQLite authority converges
its bounded window and validates operation, journal, and Orchestra tombstones
in one transaction, returning an optional receipt and the transaction's replay
horizon together; the runtime separately checks projection absence. Missing
receipts return typed `null`, while corruption remains a fixed failure. IPC
tests freeze authorization, found, and not-found paths; the native CLI exposes
`runtime unregister-receipt`, and Avalonia's source-generated client rejects
future generations, invalid revisions, duplicate identities, and cleanup-count
drift. The next gate persists the unregistration command ID in each 1.x runtime
deletion intent and consults receipt lookup before retrying mutation, so host
recovery can resolve lost acknowledgements without creating a new operation
identity. It is complete: control-plane schema v4 stores one deterministic,
validated command ID per deletion intent and migrates v1-v3 state without
identity drift. Reservations carry that identity through interactive deletion
and background recovery. Recovery performs a typed receipt lookup first,
accepts only an exact target-set match, skips a duplicate mutation when the
receipt exists, and reuses the same ID on a typed miss. Lookup, horizon, command,
or target corruption fails closed. C# wire tests and recovery tests freeze
found, missing, and mismatched paths. The next gate runs a real-daemon
fault campaign that commits unregistration, drops the acknowledgement, kills
the compatibility host, and proves restart recovery converges through lookup
without issuing a second mutation. It is complete on local Arm64 and physical
Linux x86_64. Each host force-kills the compatibility process three times after
the real daemon commits but before the worker receives success. Every restart
restores the schema-v4 command identity, performs exactly one receipt lookup,
issues zero unregistration mutations, preserves the operation generation, and
survives a final disk reload. The retained evidence lives in
`docs/fixtures/leserpent_runtime_deletion_lost_ack_20260726.json` and
`docs/fixtures/leserpent_runtime_deletion_lost_ack_linux_x86_64_20260726.json`;
`scripts/validation/leserpent_runtime_deletion_lost_ack.sh` reproduces it. The
replay-horizon floor gate is also complete. Control-plane schema v5 atomically
persists the daemon's pre-mutation `next_generation` before single, bulk, or
recovery deletion can execute. A later typed miss is rejected with the fixed
`replay_ambiguous` posture when `evicted_through_generation` reaches that
floor; schema v1-v4 pending mutations migrate conservatively without invented
proof. Real Arm64 and physical Linux x86_64 campaigns force-kill the host after
daemon commit, roll all 256 retained receipts, and observe one lookup, zero
post-restart mutations, a preserved local runtime, and a durable ambiguous
intent. The revision-bound operator reconciliation gate is now complete.
Control-plane schema v6 retains a bounded reconciliation audit. Operators first
obtain a typed full-daemon snapshot, then submit the exact intent and daemon
revisions with explicit confirmation. A re-registered original identity blocks
cleanup; daemon revision drift also fails closed. When a later snapshot proves
every original identity absent, local runtime/session compatibility
projections and intent cleanup commit atomically with the audit. Orchestra
cleanup remains on its existing idempotent persistence-authority boundary
under the same deletion claim. Request identity replay survives both
convergence and restart.

The Arm64 and physical Linux x86_64 campaigns now continue past replay
ambiguity: each re-registers the original runtime identity and proves
reconciliation rejection, unregisters it, converges against daemon revision
516, reloads the control-plane state, and replays the same reconciliation
request without another mutation. Evidence lives in
`docs/fixtures/leserpent_runtime_deletion_replay_horizon_20260726.json` and
`docs/fixtures/leserpent_runtime_deletion_replay_horizon_linux_x86_64_20260726.json`;
`scripts/validation/leserpent_runtime_deletion_replay_horizon.sh` reproduces
it. The reconciliation commit crash gate is also complete. A cross-process
harness pauses before the strict save, is terminated while the production
state temporary file exists, or is terminated after the committed marker.
Every restart on Arm64 and physical Linux x86_64 restores exactly one complete
generation: either runtime, session, and ambiguous intent remain with no audit,
or all three are absent with one matching reconciliation audit. A previous
generation retries to convergence; a replacement generation replays the same
request identity after another restart. Both platforms retained nine forced
terminations, observed all three temporary-file windows, and reported no torn
generation. Evidence lives in
`docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_20260726.json`
and
`docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_linux_x86_64_20260726.json`;
`scripts/validation/leserpent_runtime_deletion_reconciliation_commit.sh`
reproduces it. The cross-authority crash gate is now complete as well. Its
test-only store wrapper delegates to the real daemon-backed Orchestra store and
emits a marker only after the Rust SQLite delete transaction returns. The parent
then force-kills after Orchestra cleanup, during the later JSON temporary-file
write, or after the JSON commit. Every Arm64 and physical Linux x86_64 restart
keeps target history absent, preserves an unrelated run and event byte-for-byte
at the typed field boundary, and restores either the complete previous or
replacement control generation. The cleanup receipt gate is complete: one
command ID is derived from the reconciliation intent and revision, and runtime
schema v16 atomically persists its canonical targets, operation generation,
deletion counts, and timestamp with the Orchestra delete. Schema v17 adds
the bounded cleanup receipt gate: a contiguous 4096-generation horizon exposes
oldest, newest, next, evicted-through, and protected-from metadata through
health and authenticated IPC. Every new receipt protects itself in the delete
transaction. A monotonic checkpoint is accepted only for a complete retained
generation range and only after the corresponding reconciliation audit is
durable; it then deletes exactly the older prefix and advances the high-water
mark in the same transaction. Restart validates that the oldest retained audit
still lies inside the horizon. Runtime schema v18 adds a durable
`checkpointed_through_generation` high-water mark. Migration from v17 seeds it
conservatively from `protected_from_generation`, never from newer unaudited
receipts. The local C# SQLite bridge uses schema v5 and the same checks, while
Rust schema-v16 receipts migrate losslessly through v17 and v18. Health
and explicit queries now expose available capacity, saturation, typed
admission posture, and admission pressure. Protected windows become warning at
512 remaining receipts and critical at 128, then blocked at zero; unprotected
rolling windows remain healthy. Every non-healthy state exposes the checkpoint
operator action. Warning clears only above 768 available receipts; critical
clears above 256 and remains warning until the 768 boundary is crossed. Exact
checkpoint lag is retained count before the first checkpoint and otherwise the
distance from checkpointed-through to newest. A full pinned horizon returns a
stable saturation error;
advancing the durable audit checkpoint compacts the covered prefix and restores
admission.

Previous control generations replay the command and receive the same generation
with `replayed=true`; replacement generations retain the command ID and
generation in their audit. Target drift under the same ID fails closed. Every
final reload retains exactly one reconciliation audit, replays both request
identities, and verifies the same durable cleanup generation. Existing cleanup
evidence lives in
`docs/fixtures/leserpent_runtime_deletion_cross_authority_20260726.json` and
`docs/fixtures/leserpent_runtime_deletion_cross_authority_linux_x86_64_20260726.json`;
`scripts/validation/leserpent_runtime_deletion_cross_authority.sh` reproduces
it. Both schema-v3 fixtures retain nine forced-termination checkpoint and
prefix-compaction proofs, then fill the horizon to one available slot and race
cleanup-first plus checkpoint-first commits. Both orders preserve a contiguous
two-receipt final window and restore healthy admission with 4094 available
slots. The schema-v3 fixtures also persist an audit without checkpointing,
observe checkpoint lag `2`, gracefully restart the real daemon on the same
journal, and reconstruct Registry state. Startup automatically checkpoints the
audited generation and reports lag `0`; the same synchronization runs after
strict audit persistence and on reconciliation request replay. Operators can
query `/v1/persistence/orchestra-cleanup-replay-status` for audited bounds,
checkpoint high-water, lag, hysteretic pressure, thresholds, and the last
automatic advancement. The physical Linux x86_64 fixture was refreshed on
2026-07-27 with the native Rust daemon and .NET harness. That campaign also
exposed a peer-disconnect defect: a force-killed client could surface
`BrokenPipe` through the IPC poll loop and terminate `leserpentd`. Accepted IPC
connection failures are now isolated like remote-server peer failures, with a
dedicated disconnect regression and bounded asynchronous test diagnostics.
This proves idempotent convergence, not a distributed transaction.

Control-plane schema v7 now persists the last trusted cleanup horizon and
pressure together with a sanitized automatic-checkpoint incident. Startup,
audit persistence, request replay, and status reads honor the durable
1/2/4/8/16/30-second retry schedule instead of busy-looping against an
unavailable daemon. Daemon-backed history loading can start in monitored
degraded mode without migrating an unavailable authority as empty. During a
prolonged outage the status API continues to report the stale critical pressure,
lag, failure count, next retry, and alert generation. The mutation-fenced
acknowledgement endpoint persists one operator acknowledgement against that
generation; restart preserves it, recovery closes the incident, and a later
outage creates a new unacknowledged generation. Complete state validation runs
before every atomic save, including monitor and acknowledgement coherence. A
deterministic outage/restart activity proves the 30-second retry ceiling and
that pressure cannot disappear merely because the daemon is offline.
Schema v8 adds a bounded generation-derived alert outbox. A hosted worker now
drives synchronization independently of the status endpoint, persists each
delivery attempt before invoking the sink, retries with the same capped
schedule, and deletes an event only after acceptance. Stable event IDs make a
crash-safe retry idempotency-capable without pretending exactly-once delivery.
The default structured-log sink and replaceable sink boundary contain no
credentials. A worker restart activity leaves daemon and sink unavailable,
reloads the pending attempt, restores both, and proves checkpoint recovery plus
outbox drainage without operator polling.

The following ownership gate is now complete. Checkpoint work requires an
owner-private
process-lifetime lease keyed by the canonical state path. Atomic owner metadata
binds PID, process start time, and a random release token; live duplicate
processes remain standby, while a newly loaded process can reclaim a
force-killed owner record. Active owners revalidate their token before
maintenance and external alert delivery, stopping safely when the record is
removed, replaced, malformed, or unsafe. Registry synchronization entry points
are ownership-gated, and already-covered audit generations no longer repeat
authority checkpoint mutations. This is intentionally not a general
active-active JSON control-plane contract. A real child-process harness proves
live exclusion, graceful release, and stale-owner recovery; a dual-host
activity proves one mutation and one notification.
The physical Ubuntu x86_64 duplicate-host campaign subsequently exposed that
exact cross-process `Process.StartTime` equality was not stable on Linux. Lease
identity now uses `/proc/<pid>/stat` start time with compatibility for existing
positive records. Three real Web hosts prove one owner, one standby, no
already-loaded standby re-entry after owner termination, and fresh-process
takeover. Retained evidence lives in
`docs/fixtures/leserpent_checkpoint_worker_duplicate_host_linux_x86_64_20260727.json`;
reproduce it with
`scripts/validation/leserpent_checkpoint_worker_duplicate_host.sh`.

Operators may now replace the default structured-log sink with a strict HTTPS
sink using `LESERPENT_CHECKPOINT_ALERT_ENDPOINT` plus an owner-private absolute
`LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE`. Inline secrets, partial configuration,
non-HTTPS endpoints, symlinks, unsafe permissions, redirects, and malformed
tokens fail closed. Wire-v1 delivery carries Bearer authentication, stable
idempotency and generation headers, and a bounded public JSON envelope.
Authenticated
`/v1/persistence/orchestra-cleanup-worker-health` now exposes only lifecycle,
lease ownership, sink mode, and sanitized delivery health; paths, endpoints,
tokens, and raw exceptions are excluded. The next gate inventories all JSON
control-plane mutation entry points and evaluates a broader process-wide
single-writer fence.

That broader process fence is complete, and the second Rust generation-fenced
slice is now implemented. Runtime journal schema v19 stores a single
monotonically increasing authority writer generation and its random writer ID.
The C# owner claims it before startup admission completes, then attaches the
ticket through one shared frame codec to registration, discovery intake,
unregistration, deployment, and Orchestra mutation IPC frames. Claim retry is
idempotent; a newer claim rejects missing and stale tickets before projection,
effect queue, receipt, or Orchestra persistence mutation. The third slice now
fences the explicit local bootstrap, provisioning, runtime-retirement, and
daemon-retirement routes before protocol decode and lets the native CLI forward
an owner-issued ticket without automatically claiming authority. The fourth
slice carries the same ticket over authenticated HTTPS in paired canonical
headers, covering wire mutations and all four dedicated remote mutation routes
while leaving wire reads and Leselang export unfenced. The inventoried external
side-effect routes now share one authority boundary. The fifth slice closes the
previous refresh and bootstrap-session-bind gaps, makes Rust request
classification compile-time exhaustive, and source-scans both the C# mutation
routes and Rust HTTPS table against contract 1.16.0. A real three-process daemon
test proves live-owner exclusion, generation advance, stale-writer rejection,
and durable writer replay across two restarts. Hot failover remains explicitly
out of scope; future route growth must preserve these executable inventory and
cold-takeover contracts.

The sixth slice adds deterministic unclean writer-claim proof on macOS and
physical Linux x86_64. A SQLite reader lock holds the real claim transaction at
pre-commit before `SIGKILL`, proving rollback to the complete old generation and
natural 30-second owner-lease takeover. A second post-commit `SIGKILL` proves
the complete new generation survives even though process cleanup never runs.
The seventh slice proves a lost successful claim response remains linearizable
when same-ID retry and competing-ID takeover start concurrently. Both legal
serial orders are explicit, generation allocation stays monotonic, the losing
ticket cannot mutate, and the final identity replays without another advance.
Physical Linux x86_64 evidence retains the stricter competitor-first history.
The eighth slice carries that lost final response through two cold daemon
restarts. The first replacement replays B/`2` before a queued C competitor
advances exactly once to `3`; the second replacement still replays C/`3`.
Stale-ticket rejection and a real mutation bind persistence evidence to writer
authority. The ninth slice combines unread claim response with daemon
`SIGKILL`, pre-expiry owner rejection, natural lease expiry, and safe recovery
of the same stale Unix socket path. The recovered writer replays generation `2`
before one queued competitor advances to `3`, and real mutation fencing follows
the final ticket. The tenth slice repeats that complete unclean cycle twice on
one database and socket, proving contiguous generations `1` through `5`, two
natural lease expiries, two same-path rebinds, and stable final mutation/replay.
The eleventh slice saturates the production 64-connection IPC batch after an
unclean recovery. All independent claims complete inside a fixed budget,
allocate contiguous generations `3` through `66`, reject old and penultimate
tickets, and leave only generation `66` able to mutate and replay. The
twelfth slice fills that same batch with 16 response-abandoned new claims and
48 readable same-ID retries. Every primary commits, every follower replays,
generations remain contiguous from `3` through `18`, and peer response failures
do not stop later claims. The thirteenth slice makes frame reads concurrent but
keeps dispatch accept-ordered and serial, then mixes 16 malformed, 16
unauthorized, 16 full-timeout slowloris, and 16 valid peers. Invalid peers
allocate no writer generation and valid progress completes within 5000 ms
across two waves. The fourteenth slice repeats that 64-peer hostile workload
twice and proves the same SQLite owner token refreshes its 30-second lease after
each batch without advancing a replayed writer generation. IPC frame reads now
poll the process stop flag every 100 ms while retaining a hard wall-clock 2000
ms peer budget even for drip-fed frames, and authority dispatch stops once
shutdown begins. A third wave of
64 active slow peers is interrupted by `SIGTERM` inside 1000 ms, releases the
owner row and socket, and permits immediate same-path restart with stable
generation replay. Physical Linux x86_64 reproduces both batches in 2234 ms and
2209 ms, then exits the 64-slow-peer shutdown wave in 165 ms with owner and
socket cleanup plus immediate restart. The fifteenth slice repeats completed
and active hostile waves across three daemon processes. Physical Linux `/proc`
shows each completed batch returning to exactly 5 open FDs and 1 task, each
64-peer shutdown wave reaching exactly 69 FDs and 65 tasks, and every exited
process removing its proc directory, owner row, and socket. SIGTERM remains
bounded at 216 ms, 207 ms, and 208 ms while generation 1 replays through both
restarts. The sixteenth slice removes avoidable same-batch response delay
without weakening authority order: readers are still joined and dispatched in
accept order, but each ready prefix is dispatched before waiting on later
readers. A Linux unit proof returns the ready first peer in 70 ms despite a
later slow peer. Three production-daemon waves then each saturate all 64 slots
with 60 slowloris peers and four valid writer reconnects. All 12 reconnects
replay generation 1 in at most 2224 ms, every wave completes in at most 2225
ms, and the same owner heartbeat advances after every wave. The seventeenth
slice moves maintenance ahead of transport polling and alternates Unix
IPC-first with HTTPS-first priority. Three physical Linux waves each place one
authenticated HTTPS runtime-list query beside 64 slow IPC peers. Every HTTPS
query completes within 2264 ms, every full wave within 2265 ms, owner heartbeat
advances after every wave, and writer generation 1 remains stable. The
eighteenth slice proves the symmetric case with three real TLS clients that
send valid bearer-authenticated headers but withhold a declared one-byte body
for the full 3-second remote read timeout. Twelve concurrent IPC queries all
complete within 3199 ms, slow HTTPS failures remain within 3156 ms, maintenance
advances after every wave, and generation remains 1. The nineteenth slice gives
TLS, HTTP-head, and body reads a shared 3-second monotonic deadline with 100 ms
stop polling, then rechecks cancellation before authority dispatch and response
write. Physical Linux `SIGTERM` during an authenticated incomplete body exits
in 10 ms, suppresses the application response, releases owner/socket state, and
immediately restarts with generation 1 replayed. The twentieth slice repeats
that proof across incomplete TLS-handshake, authenticated HTTP-header, and
authenticated-body reads. Four consecutive Linux processes retain an identical
6-FD/1-task idle baseline outside SQLite journal windows; each active phase adds
one FD and no task. A nonblocking TCP cancellation wrapper below rustls absorbs
`WouldBlock` and reports non-retryable `ConnectionAborted`, avoiding the
standard `Interrupted` retry loop. Three consecutive physical runs keep all
phase exits within 104-115 ms, remove proc/owner/socket state, and replay
generation 1 on the next same-state process. Read timeout errors retain one
immediate HTTP response attempt, while a blocked write cannot outlive the shared
deadline. Parallel lifecycle fixtures use a process-local atomic suffix rather
than relying on clock uniqueness. The twenty-first slice rotates the active
read across those same three phases while 64 additional incomplete TLS peers
remain queued in the listener backlog. Four physical Linux processes preserve
the 6-FD/1-task idle baseline and 7-FD/1-task active baseline before and after
backlog admission. Shutdowns finish in 93-110 ms, every proc/owner/socket state
is released, and generation 1 is replayed without allocation. The next boundary
combines the maximum 32 authenticated WebSocket event sessions with one stalled
request and proves bounded shutdown, fixed resources, and no late events. The
twenty-second slice completes that proof on physical Linux: resources move from
6 FDs/1 task to 38/1 at maximum event capacity and 39/1 with the stalled request,
then return to 6/1 after restart. `SIGTERM` completes in 111 ms after all 32
initial snapshots are consumed and the pre-stop queue is drained; no late
application event or stalled response escapes,
and generation 1 is replayed without allocation. The next boundary repeats
maximum-capacity connect/fanout/disconnect cycles while proving all slots are
reclaimed and IPC/HTTPS continue to progress. The twenty-third slice completes
the production-process behavior proof across three uninterrupted cycles: 96
capacity-window sessions and three immediate post-disconnect probes are
admitted, three fenced runtime registrations each fan out the next snapshot to
all 32 clients, and IPC plus HTTPS reads finish
inside their five-second budgets during every capacity window. This exposed and
fixed an admission race where a reconnect could be capacity-rejected before
closed sessions were polled; event sessions are now reclaimed before accepting
the next connection. All slots are immediately reusable and the same database
restarts with generation 1 replayed. Retaining exact idle, capacity, reclaimed,
and restart FD/task baselines on physical Linux is the remaining boundary.
The twenty-fourth slice proves slow event consumers cannot become a global
backpressure source. One maximum-capacity production process starts with 128
runtime projections and 32 authenticated clients; one client stops reading
while the other 31 consume 24 revision-bound snapshots each. All 744 healthy
deliveries complete, IPC and HTTPS remain responsive after every eight
revisions, and the non-reader is evicted by the 1 MiB bounded write buffer
without affecting writer generation or restart. Physical Linux still needs to
retain the exact slow-session FD reclamation and zero-task-amplification data
beside the preceding three-cycle resource proof.

Schema v3 added validated domain snapshots that preserve
projection revisions and idempotency results; startup restores the snapshot and
replays only its incremental journal suffix. Snapshot metadata and payload share
one integrity check. Schema v4 retains two generations, falls back from a
corrupted newest generation, preserves the journal suffix needed by the prior
generation, and deletes at most 1000 covered records per checkpoint. Schema v5
adds a conditional SQLite owner lease: a second live runtime is rejected,
expired ownership can be replaced atomically, and both stale reads and stale
writes are fenced on their next operation. Schema v6 adds a bounded persistent
effect queue with idempotent enqueue, atomic claim, lease
renewal, attempt-token fencing, crash redelivery, retry delay, terminal
completion/failure, and max-attempt sealing. Daemon heartbeat, typed adapter
worker loops, and status projection integration are implemented; queue
backpressure visibility and terminal task retention are now implemented, while
bounded worker concurrency remains the gate boundary. The runtime now exposes
an explicit owner heartbeat plus a
bounded synchronous worker step. Executors return `Complete`, `Retry`, or
`Reject`; the runtime alone commits the fenced lease transition, while callers
retain control over concurrency, cancellation, and shutdown.

Schema v7 freezes the durable runtime semantics. It records the introduction of
typed status-observation journal entries even though no physical table change is
needed, so a v6 binary rejects a newer database by version instead of attempting
an incompatible replay. Startup requires migration history to be exactly 1
through 7, verifies the effect-table columns and claim index, and rejects unknown
journal kinds before rebuilding projections. Complete v6 databases migrate
transactionally to v7.

Schema v8 adds the dedicated `runtime_logs` table and its
`(runtime_id, sequence)` index. Appending and per-runtime retention occur in one
owner-fenced transaction, retaining at most 4096 records per instance. The
versioned domain query limits each response to 256 records, supports monotonic
cursor continuation, and omits runtime endpoints. Migration history must now be
exactly 1 through 8; malformed pre-existing tables and indexes fail closed.

Applied refresh events now materialize their typed status effect before the
command returns. Startup reconstructs a missing effect from durable applied
command results, closing the crash window between command-journal completion and
queue insertion without duplicating an existing idempotent task.

Status-refresh completion is also durable end to end. A typed observation
carries its runtime ID and expected projection revision; the runtime validates
it on a cloned projection, then appends the replay record and completes the
leased effect in one SQLite transaction before publishing the new in-memory
state. Restart replay verifies the recorded projection outcome. Stale revisions
and malformed adapter outcomes fail the task closed without stopping the worker.

The effect queue reports exact `ready`, `leased`, `completed`, and `failed`
counts together with active capacity and saturation. Authenticated wire-v1
health includes this data through an optional field, so older response payloads
remain decodable. Terminal retention is explicit and bounded: daemon maintenance
runs every 256 heartbeats by default, retains the newest 8192 terminal tasks,
and removes at most 100 per pass without touching ready or leased work. This
also defines the persisted effect-ID idempotency horizon; producers must use
globally unique effect IDs rather than intentionally recycling compacted IDs.

Bounded synchronous concurrency is now explicit. `leserpentd` claims at most
four effect kinds per batch by default (configurable from one through 32), runs
different adapter kinds on scoped native threads, and settles outcomes back on
the single authority thread in claim order. A batch never leases two tasks for
the same adapter kind, so mutex wait time cannot consume a second task's lease.
Adapters receive a cooperative cancellation context; cancellation schedules a
retry, while a panic is contained and rejects only its claimed task. The main
signal loop passes its stop flag into batch execution.

The repeatable Linux stress harness is
`scripts/validation/leserpentd_linux_stress.sh`; it runs natively on the trusted
remote shelf and retains JSON under
`target/validation/leserpentd-linux-stress/`. The 2026-07-15 x86_64 run on Linux
6.17.0-35 proved eight-way observed parallelism with all 256 effects completed,
cooperative cancellation returning a lease to `ready` in 25 ms, strict rejection
of task 10,001 at the 10,000-active capacity, and real process-exit recovery as
attempt two after the 30-second owner lease elapsed. The saturation phase took
28.2 seconds with one transaction per task and `synchronous=FULL`.

The runtime now provides atomic enqueue batches of one through 1000 tasks.
Validation, duplicate-ID detection, existing-record idempotency checks, and the
active-capacity decision all complete before insertion; any conflict or overflow
rolls back the whole batch. A full queue still accepts exact idempotent replays.
On the same Linux host, filling all 10,000 active slots as 100 batches of 100
took 584 ms, a 48.3x improvement over 28,185 ms, without changing
`synchronous=FULL` durability.

With the schema-v7 compatibility fence, atomic batch API, Linux stress evidence,
and negative migration tests in place, the reusable control-runtime contract is
frozen as `1.0.0`. This freezes the runtime cell, not the wider Leserpent 2.0
architecture; daemon platform parity, adapters, renderers, and remote control
continue on their own status-tensor tracks.

Snapshot startup is also panic-free when every retained generation is
unusable. Recovery exhausts the bounded generation history and returns the last
structured storage error through an explicit match; a negative test constructs
two integrity-valid snapshots with unsupported domain schemas to exercise this
exact authority-startup path.

The `leserpentd` crate hosts `ControlRuntime` as a stable standalone authority.
It drives explicit owner heartbeats and bounded worker steps through a typed
adapter registry, supports finite-step smoke execution, and stays in a
heartbeat-only safe mode when no adapter is installed. Graceful Unix signal
shutdown, authenticated local and opt-in remote health, bounded synchronous
worker concurrency, panic-safe lease settlement, and physical Linux stress are
implemented. Adapter breadth remains owned by `leserpent-adapters`; Android and
Windows/Web client parity remain client transport concerns rather than daemon
lifecycle blockers.

On Unix, `leserpentd` now exposes the existing wire-v1 request and response
envelopes over a private `0600` Unix socket. Each bounded line frame carries an
IPC token sourced only from `LESERPENT_IPC_TOKEN`; token comparison is constant
time, malformed or oversized frames fail closed, and internal storage failures
are not reflected to clients. Windows named-pipe parity remains before local IPC
is cross-platform complete.

Wire v1 now includes an authenticated health request whose response is emitted
only after the runtime renews its owner lease, so `ready` proves current local
authority rather than process liveness alone. The daemon maps SIGINT and SIGTERM
to an atomic stop flag; normal stack unwinding removes the socket inode and
releases the SQLite owner row. A real process smoke verifies exit code zero and
lease release after SIGINT.

The first production integration now lives in the independent
`leserpent-adapters` crate rather than the daemon host. Its typed registry
rejects unknown effect kinds. The Gewyvern health and status adapters retain the
loopback-only HTTP default while also supporting explicit authenticated HTTPS
targets with pinned CA input and rustls hostname verification. They keep admin
tokens out of scheduler payloads, strictly frame bounded JSON responses, and
validate the `/health` JSON contract. A vertical
test proves SQLite enqueue through daemon claim and adapter execution to fenced
completion. The typed status adapter now combines `/health` and
`/v1/latest/meta` into a frontend-neutral snapshot without journaling its admin
token. `leserpentd` registers both health and status effect kinds for configured
targets, and status outcomes flow through the runtime's atomic projection and
replay path. When a target credential exists, the daemon also registers the
typed deployment effect. It requires confirmation, posts only the bounded
deployment intent to the fixed Gewyvern endpoint, binds success to the echoed
idempotency fields, and distinguishes permanent request conflicts from retryable
service failures. A SQLite-to-daemon vertical and a real authenticated TLS test
cover this path. Targets now retain only validated secret aliases; a shared
`SecretStore` resolves the allowlisted environment token immediately before
network execution, redacts debug output, zeroizes temporary values, and fails
before connecting when resolution fails. The macOS provider now calls
Security.framework directly and `leserpentd --gewyvern-admin-secret KEY`
explicitly selects account `KEY` under service
`org.gewyvern.leserpent.adapters` without environment fallback. The Linux
provider dynamically resolves libsecret/glib without development packages and
has physical Ubuntu 24.04 x86_64 evidence: ordinary SSH fails closed without a
session bus, while an isolated D-Bus plus gnome-keyring session returns a strict
clean miss. Real TLS tests cover authenticated adapter traffic and ambiguous
HTTP framing rejection. Bounded capability discovery now queries only explicitly
configured targets, canonicalizes endpoint claims, accepts safe boolean schema
extensions, and rejects scan-like payload fields or inconsistent deployment
advertisements. SQLite schema 9 adds the revision-bound capability observation
journal kind. Effect completion and projection update commit atomically, replay
restores the same state, and stale or forged observations fail closed. The
shared `runtime.refresh_capabilities` command now lowers through the same typed
plan from Leselang, CLI (`runtime refresh-capabilities`), and GUI actions. The
runtime converts its domain event into the revision-fenced discovery effect;
none of those operator surfaces can access the low-level effect queue. CLI
inspect and renderer-neutral runtime workspaces now render the same bounded
capability posture: explicit unobserved state, service/version, core booleans,
canonical endpoint paths, and sorted boolean extensions, with no target origin,
secret alias, credential, or raw adapter payload. A real TLS vertical now proves
the remote closed loop: the native CLI submits the capability command, the
daemon persists and schedules it, the real discovery adapter queries only its
configured loopback target, and a later CLI inspect reads revision 3 with the
validated projection. The adapter origin and authorization material remain
absent. Avalonia now strictly decodes and validates that projection, renders the
same bounded capability section, and binds a separately typed discovery action
to the authenticated mutation client. Rust-generated UI fixtures pass the .NET
semantic renderer and real Avalonia control probes. A Rust daemon-to-.NET TLS
vertical additionally submits the capability command at revision 3, runs the
real discovery adapter against a fixed loopback service, observes the validated
projection at revision 4 over WebSocket, and reads the two-entry workspace from
an independent process without retaining either endpoint. Avalonia keeps the
mutation fenced between those revisions so a repeated command cannot invalidate
the in-flight observation. Ambiguous network outcomes additionally require a
later full snapshot: heartbeat-only liveness never releases the mutation fence,
and a newer revision with an unchanged capability posture remains blocked. New
runtime projections now carry the exact command revision that produced the
capability observation. This removes content-based ambiguity when repeated
discovery returns identical data, remains optional for legacy snapshots, and
has explicit old-journal replay evidence without a schema migration. Secure
first-run connection and endpoint-scoped Keychain/Secret Service onboarding now
run inside the no-argument desktop product path, including bounded password
input, immediate control clearing, and token-free profile persistence. The
native Rust bundler now turns the arm64 NativeAOT output into a strict `.app`
with stable plist identity, checked icon, debug-symbol exclusion, native menu,
Dock reopen, explicit Quit, real Finder launch evidence, and strict ad-hoc
signature verification. Its self-check requires the generated plist to match
the canonical template exactly. The native release entrypoint independently
requires unique identity, executable, package-type, display-version, and
build-version fields; both version fields must match the release tool's
workspace version before any signing or notarization command can run. A shared
bounded native-header reader additionally requires the main executable,
app-bundled `leserpentd`, and every `.dylib` to be a thin 64-bit ARM Mach-O
payload, including the ARM64 CPU type, at AOT proof, bundle creation, release,
and installation boundaries. The native release entrypoint signs the daemon
and dylibs before the outer app and now enforces inside-out
Developer ID signing, Hardened Runtime, secure timestamps, Keychain-only
notary credentials, explicit acceptance, ticket stapling, and Gatekeeper
assessment. A native machine-readable preflight now binds separate main-app and
`leserpentd` executable hashes plus the entitlements hash, inventories all eight
Apple release tools, counts valid
Developer ID Application identities, and optionally verifies a named notary
Keychain profile without exposing credentials. The current retained preflight
records all tools ready but zero identities and no requested profile, so
`release_ready=false`; executing and retaining the formal Apple-backed proof
remains the only macOS release gate.
The native `cargo dev package desktop` path now accepts an identity and notary
profile only as a pair, runs that preflight in strict mode, and keeps signing,
notarization, stapling, and Gatekeeper assessment behind the pending-bundle
atomic publication boundary. This closes the workflow-integration gap without
claiming the still-unavailable Apple-backed host evidence.
The release binary now derives both tool inventory and execution from one fixed
set of `/usr/bin` and `/usr/sbin` paths, supplies only the standard system
`PATH`, and strips process-local Xcode selection overrides from every `xcrun`
stage. The host's reviewed `xcode-select` configuration therefore remains
usable while caller-path substitution cannot cross the preflight boundary.
The unified native release gate can now ingest that report through
`--macos-release-preflight`, enforce a bounded and internally consistent schema,
index a normalized copy, and distinguish an external credential block from a
malformed proof or a ready Apple stage. This keeps CI green for valid evidence
collection without ever turning a blocked preflight into a ship signal.
The latest arm64 NativeAOT bundle also passes the shared product-startup probe
against a real temporary Keychain item and saved profile; the item is deleted
in-process and independently confirmed absent afterward.
The product shell now also exposes one shared connection-management flow from
the native macOS menu and remote status bar. Replacement sessions are created
before the active session is released. Forgetting requires explicit
confirmation, is fenced against a changed saved profile, and deletes only the
canonical endpoint's platform credential and non-secret profile. Managed
contract and real Avalonia control probes cover both settings and confirmation
surfaces without exposing token material.
Remote UI operations now enter through observed tasks rather than naked async
event handlers. Closing the fleet window cancels work, removes the event
subscription, fences already queued state updates, closes child workspaces, and
contains client-disposal failures without writing back into a closed control
tree.
Remembered desktop CA trust is now copied out of ambient user paths into an
application-private, content-addressed trust directory. A strict contract proves
single-PEM parsing, CA and key-usage constraints, atomic/private writes, legacy
profile migration, valid-certificate replacement rejection, symlink rejection,
bounded set-based stale-CA pruning, and recognized crash-temporary cleanup.
Ephemeral connections remain non-persistent. Android now consumes that stable
profile and lifecycle paradigm through its shared mobile coordinator. Windows
native-host evidence is not a blocking gate.

The authenticated deployment adapter is now reachable through a shared
`runtime.deploy` command vertical. Domain policy separates deployment from
refresh authority, rejects unconfirmed execution, and validates bounded
pipeline/target input. Leselang, native CLI, and deterministic plan export lower
identically; the durable runtime derives requester and request identity from the
envelope before scheduling the fixed deployment effect. VM dispatch
acknowledgement and real adapter TLS tests preserve synchronous re-entry and wire
compatibility. Avalonia remote workspaces now expose a bounded deployment form
only for runtimes with an authenticated deployment claim. Rust UI IR declares
the fields and emits a typed renderer-neutral `submit` event; Rust lowering and
the .NET semantic renderer independently reject unknown, missing, oversized, or
invalid values. Source-generated JSON probes verify the fixed capability,
explicit confirmation, typed arguments, refresh-field omission, and
cross-renderer event shape. Mobile shells can therefore reuse the form contract
without duplicating deployment semantics.
The 1.x compatibility bridge now also validates the exact confirmed deployment
request after local runtime/capability checks but before remote network I/O. Its append-only fixture
binds route runtime identity, pipeline intent, requested-by principal,
idempotency key, confirmation, and target while rejecting unknown fields. This
freezes the side-effect boundary needed for later Rust control-plane extraction
without changing the authoritative 1.x deployment response path.
When the bridge is configured, Rust now owns the final normalization decision
and returns the canonical deployment envelope. The C# adapter and Orchestra
audit consume that exact pipeline, principal, request ID, and target; a runtime
identity fence rejects mismatched bridge output before any remote effect. The
unconfigured development path preserves the original request, while a real
cross-process test proves the packaged authority handoff.
The Rust authority now also exposes a typed deployment receipt over its private
IPC protocol. A receipt is bound to both command ID and request ID, can only
read deployment effects, and projects persisted queue state as pending,
completed, or failed. Completed receipts carry the adapter-validated Gewyvern
response, closing the daemon-side submit/execute/observe loop without exposing
generic effect outcomes. The configured 1.x host now submits that canonical
intent over an owner-private Unix socket and polls the bound receipt to preserve
the existing synchronous accepted response and error mapping. Partial daemon
configuration, unsafe socket permissions, transport failure, timeout, protocol
drift, and receipt identity mismatch fail closed; an explicitly unconfigured
development host retains the old direct adapter. Fresh daemon databases
idempotently journal configured Gewyvern targets, while restart-time endpoint
drift is rejected rather than silently replacing authority metadata.
An additional append-only compatibility fixture now freezes Orchestra's atomic
run/event persistence unit. Rust rejects mismatched run or runtime identities,
outcome drift, invalid attempts, oversized steps, and unknown fields. The bridge
can validate this envelope. Runtime journal schema 10 now owns strict Orchestra
run and event tables, atomically persists the pair, makes exact event replay
idempotent, rejects event-key payload drift, and returns the bytes read back from
SQLite through a capability-gated `orchestra_persist` IPC response. A configured
ASP.NET host now selects `DaemonOrchestraRunStore` at composition time and does
not instantiate or dual-write the managed SQLite store; the unconfigured
development path retains the legacy provider.
The same daemon boundary now exposes bounded `orchestra_history` pagination:
run pages may be fleet-wide or runtime-scoped, event pages require both runtime
and run identity, pages contain at most 64 records, and event IDs come from the
authoritative SQLite sequence rather than the legacy envelope placeholder.
This closes startup/UI read-back without introducing an unbounded history dump.
The typed `orchestra_delete` operation removes up to 128 runtime histories in
one owner-fenced transaction and reports actual run/event counts. Schema 10 also
preserves 1.x request-ID uniqueness and the 32-run per-runtime retention bound.
A real C#-to-daemon test proves persist, canonical read-back, authoritative event
sequence, delete, and empty read-back against one Rust SQLite database.

- SQLite journal, projections, snapshots, and migrations
- effect workers with backpressure and recovery
- secret-storage adapters
- local daemon lifecycle and process isolation
- compatibility API for the existing TypeScript dashboard
- import of supported 1.x state with rollback evidence

Exit: Rust is authoritative and the old service is an adapter, not the owner.

## Gate 6: Mobile And Remote Console

Extend the same contracts rather than forking product behavior.

- Android entry project (implemented) and physical-device lifecycle proof
- iOS native entry and simulator proof (implemented), with physical-device proof remaining
- mobile navigation and adaptive presentation
- authenticated HTTPS/WebSocket protocol
- reconnect, offline read cache, and explicit stale-state presentation
- push/deep-link integration through platform adapters
- optional embedded Rust feasibility study for offline execution
- production TypeScript web client parity for Windows and remaining non-native
  hosts, using the same native policy contracts and transport semantics

The Android executable entry client now composes the validating credential
vault, shared app-private CA/profile storage, `MobileApplicationCoordinator`, and
foreground/background callbacks. Its native shell exposes secure setup and
renders the shared fleet and workspace `UiDocument` projections through an
immutable MobileCore binding rather than a frontend-owned runtime summary. The
same binding routes typed activation and parameterized deployment submissions;
native form fields preserve the shared input constraints, remain local until a
valid `submit` event exists, and require a second explicit confirmation. Mobile
workspace queries and mutations are bound to the current foreground generation,
use the fixed `leserpent-mobile` principal, and keep revision/unknown-outcome
ownership in `RemoteMutationCoordinator`. A shared adaptive policy now
resolves Compact, Medium, and Expanded width classes after safe-area and font
scaling, enforces 48 dp touch targets, bounds wide content, rejects short
two-pane layouts, and selects one/two runtime columns. The Android projection is
runtime-first, collapses saved setup, handles system bars plus display cutouts,
and keeps its bottom setup action above IME insets. Host-independent
conformance proves this layout policy alongside duplicate callback coalescing,
while value-type plans, exact native-presentation comparison, and a shared
render-state gate keep heartbeat-only status and IME changes from rebuilding
the complete control tree. The same contract proves endpoint reconfiguration,
failure cleanup, and terminal disposal; a static entry-contract test locks both
adaptive projection
and Keystore-only/private profile boundaries. A locked .NET 10/API 36 toolchain
now produces a directly installable APK and dual-ABI AOT AAB. API 36 ARM64
emulator proof covers Compact, Medium, Expanded, short-landscape, 1.5x font,
display-cutout, IME, cold-start/hot-resume, icon packaging, and release screen
capture protection. The updated renderer-neutral first frame, rotation, and hot
resume pass on that emulator. Production signing and physical-device
safe-area/font-scale and Keystore/TLS evidence remain before Android parity is
claimed.

The iOS executable entry now follows the same composition boundary in UIKit.
Its scene owns native controls, safe-area and Dynamic Type measurements,
keyboard avoidance, foreground/background callbacks, and an app-switcher
privacy shield, while fleet/workspace projection, typed form submission,
confirmation admission, mutation fencing, and layout classification stay in
the shared libraries. Endpoint metadata uses native preferences, public CAs and
snapshot caches use endpoint-hashed app-private paths, and credentials remain
`WhenUnlockedThisDeviceOnly` Keychain items. The iOS renderer consumes only an
immutable `MobileUiDocumentBinding`; it does not derive cards from feed state or
instantiate transport clients. Compact and accessibility-equivalent narrow
widths stack both header action rows vertically, while status-only heartbeats
retain the mounted controls and their active action-source fence.
The retained iOS 26.5 matrix now installs the Debug ARM64 simulator app on an
iPhone 17 Pro and iPad Pro 13-inch, verifies Compact and Expanded/two-pane
projection, maximum accessibility text reflow, cold relaunch, hot background
resume, and native this-device-only Keychain CRUD. The same toolchain emits an
unsigned full-trim/IL-stripped `ios-arm64` AOT bundle whose Release payload is
free of the Debug proof switches. This removes simulator runtime and Keychain
evidence from the Gate 6 blocker without overstating Apple signing or physical
device readiness.

The current desktop slice implements the event consumer and first constrained
mutation of this gate. A pure
`Leserpent.RemoteClient` library owns strict event decoding, explicit CA and
hostname verification, endpoint-bound atomic snapshot cache, stale-state
transitions, cursor reset, and an eight-attempt capped reconnect loop. The
Avalonia shell projects snapshots through the same neutral `UiDocument`
renderer used by fixtures. Runtime cards expose only revision-fenced
`runtime.refresh`, reject stale state, require explicit confirmation, and do not
retry ambiguous outcomes. A separate conformance executable proves codec,
cache, resync, and retry behavior; a real Rust daemon to .NET client TLS
vertical proves authenticated snapshot, confirmed HTTPS mutation, matching
WebSocket revision, private cache permissions, and endpoint omission. Mobile
production signing and physical-device runtime evidence remain before Gate 6
completion. Desktop startup now resolves endpoint-scoped tokens from macOS
Keychain or Linux Secret Service through AOT-compatible native bindings, with a
bounded environment fallback only when no stored item exists. Deterministic
conformance proves source precedence and malformed-item fail-closed behavior.
The same vertical now proves strict Rust-to-.NET health and workspace schema
parity. Orchestra delete replay pressure is recomputed from bounded capacity,
generation, threshold, and checkpoint metadata before it reaches the UI, where
warning, critical, and blocked states require visible operator attention.
Runtime inspect accepts the complete versioned authority projection, validates
its monotonic timestamps, and still omits endpoint material from caches,
diagnostics, and renderer-neutral workspace state.
The first mobile-independent lifecycle slice now lives in
`apps/leserpent-mobile`: it disconnects before background suspension, marks
retained state stale, hands hydrated cache state to the host before reconnect,
reloads credentials on every foreground generation, and fences delayed events
from retired sessions. A deterministic conformance runner
injects missing credentials and startup failure. Android/iOS native store
projects now compile against .NET 10 platform workloads: Android protects a
private-preferences AES-256-GCM envelope with a Keystore master key, while iOS
uses a this-device-only Keychain item. Android workload/AOT packaging and the
API 36 emulator matrix are retained as machine-readable evidence. The iOS 26.5
simulator/layout/Keychain matrix and unsigned device-AOT bundle are retained as
machine-readable evidence too. Production Android and Apple signing,
physical-device runtime conformance, and physical-device AOT execution remain.
The shared mobile vault adapter contract now provides endpoint-hashed aliases,
strict credential CRUD validation, cancellation fencing, and deterministic
corruption tests; platform storage is therefore replaceable without moving
endpoint or token policy out of shared code.
The adjacent shared connection-profile store canonicalizes HTTPS authorities,
uses endpoint-hashed CA/cache paths, rejects private keys and corrupt stored
certificates, and performs durable atomic CA replacement. Native preferences
remain a minimal endpoint-only adapter on both mobile platforms.
The workspace policy layer has also moved into `Leserpent.RemoteClient`:
filtering, bounded export, incremental/full refresh planning, retry state,
snapshot comparison, and retained severity alerts are renderer-independent.
Avalonia now supplies only window/control behavior, and MobileConformance calls
the same contracts through MobileCore's project graph to prevent a future
mobile policy fork.
The fleet and runtime-workspace `UiDocument` projections are now part of the
same shared library and reference only RendererCore. Their filtering,
capability-gated actions, parameterized forms, endpoint omission, and empty
states are no longer Avalonia-exclusive business logic. MobileConformance
executes both projections without loading a platform renderer. Android now
materializes those documents as native controls through the immutable
`MobileUiDocumentBinding`; its first frame, workspace navigation, dynamic form,
typed submission, confirmation, and mutation path therefore consume the same
semantic source rather than rebuilding presentation policy from feed objects.
Mutation revision and unknown-outcome observation fences have moved out of the
Avalonia window as well. RemoteClient now owns the runtime/revision/capability
rules, rejects heartbeat-only release, and requires a newer authoritative
snapshot after ambiguous transport failure. Desktop and mobile conformance run
the identical policy.
The next Gate 7 slice moves the lifecycle owner itself. A shared
`RemoteMutationCoordinator` now admits one tokenized operation, revalidates the
runtime and deployment capability after confirmation, captures the snapshot
generation at transport admission, installs revision or observation fences,
and ignores stale operation tokens. `RemoteFeedAuthorityPolicy` additionally
prevents a cached projection followed only by heartbeat from enabling mutation
or Inspect. Invalid success payloads and unexpected failures after confirmation
are classified as unknown outcomes rather than safe rejections, so a possibly
applied command cannot be repeated before a newer authoritative snapshot.
Avalonia contains no in-flight, revision-fence, or observation-fence state;
desktop remote and mobile conformance execute the same coordinator contract.
Mutation failure completion has now crossed the boundary as well. A shared,
typed policy classifies known remote rejection, invalid local request, invalid
response, timeout, network failure, owner shutdown, and unexpected failure.
The coordinator atomically applies the corresponding clear, observation fence,
or cancellation and ignores completions from retired operation tokens, so a
window-close race cannot disturb a newer operation. Bounded single-line
operator diagnostics omit raw transport and unexpected exception text.
Avalonia now has one failure-completion call and no mutation transport-exception
branches; desktop and mobile conformance execute the same policy.
Authority-health refresh lifecycle has crossed the Gate 7 boundary as well.
`RemoteAuthorityHealthCoordinator` joins duplicate refresh requests, publishes
monotonic `Idle`, `Checking`, `Ready`, `Unavailable`, and `Stopped` states,
restores the prior projection after caller cancellation, and classifies remote
rejection, invalid request/response, transport, timeout, and unexpected failure
without exposing transport exception text. Its stop generation prevents a
loader that ignores cancellation from republishing into a retired frontend.
Avalonia contains no health in-flight state or health protocol branching;
desktop, remote, and mobile probes execute the same lifecycle contract.
Remote event lifecycle ownership has now crossed that boundary too.
`RemoteEventLifecycle` gives every start/restart a unique generation handle,
joins concurrent and repeated disposal into one completion, and releases trust
and subscriber resources exactly once. `RemoteFeedPublisher` invokes subscribers
independently, bounds failure telemetry at `int.MaxValue`, and does not expose
callback exception text. Avalonia owns neither the event cancellation source nor
the running task; desktop, remote, mobile, and NativeAOT probes execute the same
contract.
Remote UI action routing has crossed the boundary as well. The shared
`RemoteUiActionRouter` resolves opaque node IDs through their typed `UiAction`,
checks runtime-container identity and shared availability, and validates Deploy
submission events without frontend extraction of protocol fields. Avalonia now
passes the emitting renderer with every native action, so independent workspace
windows register forms and create submissions against their own documents;
closed workspace sources are fenced before mutation admission. Desktop and
mobile conformance execute the same router, while the mutation coordinator
continues to own authorization, confirmation, and revision fencing.
Remote action availability now follows that boundary too. A pure policy gives
in-flight work precedence over revision and observation fences, disables
mutation and inspection consistently while stale, and supplies bounded reasons
to desktop or mobile controls. Avalonia no longer derives remote permissions
from window state.
The availability audit found and removed two workspace-level overwrite paths:
state application and initial window creation previously used reduced live/idle
checks that could ignore an unresolved fence. Both now pass through one policy
application helper. Authority health and queue saturation projection have also
moved into RemoteClient for desktop/mobile parity.
The desktop workspace live-query policy now tolerates two transient failures with
10-second and 20-second bounded backoff, resets to its five-second cadence after
success, and stops after the third consecutive failure. The state machine remains
transport-independent and preserves single-flight, inactive-window suspension,
and explicit operator restart semantics. A successful manual or event-driven full
workspace query clears pending backoff and restores the normal cadence without
ever retrying a mutation. Query admission owns and stops the outstanding timer;
a single-flight skip preserves the existing failure count and interval instead
of impersonating a successful recovery.
The desktop runtime workspace now saves the same endpoint-free diagnostic snapshot
through the native platform picker. The UTF-8 payload is prevalidated under 512
KiB, suggested filenames discard path/control syntax, overwrite requires platform
confirmation, and failed destinations never expose their path in UI status.
The desktop Hub now applies the renderer-neutral runtime search policy across
all loaded daemon authorities. Authority matches retain a complete bounded
preview, runtime matches retain only matching children under their owning
authority, and mobile conformance consumes the same 128-character policy.
Filtering remains local-only and cannot release the topology revision fence;
the native control probe covers keyboard focus, empty state, and clear recovery.
The same Hub now exposes a native `Refresh all` action instead of hiding global
topology refresh behind F5. Startup, periodic, keyboard, and button requests
join one global operation; duplicate authority refreshes join their card's
active task. The concurrency owner is now the renderer-neutral
`RemoteTopologyRefreshCoordinator` in `Leserpent.RemoteClient`, not the
Avalonia window: it bounds the fleet at 65 authorities (64 saved remotes plus
local Orchestra), admits four loaders, validates terminal outcomes, fences
cancelled queued work before loader entry, and returns one
live/stale/unavailable summary. Busy and completion status
remain accessible, the native probe drives the real button while a background
refresh is already active, and desktop plus mobile conformance execute the same
shared contract.
The Hub now also exposes a visible `Quick tour` and F1 route to a singleton
native Learning Center; the macOS application menu reaches the same window.
Six bounded offline steps teach topology, authority setup, runtime workspaces,
focused diagnosis, safety fences, and Leselang equivalence without performing
I/O or effects. The tutorial has complete Automation ID/name/help-text coverage,
direct and sequential keyboard navigation, compact scroll-safe layout, and an
explicit auxiliary-window classification so Dock reopen still targets the Hub.
The native verification path drives the real controls, and the lifecycle probe
checks the menu/content contract without requiring a network or daemon.
The same shell now has a native language settings path from both the Hub and
macOS application menu. It shares the Web roster of 8 built-in plus 22 official
`core-ui` locales, persists only a bounded private locale preference, resolves
system-locale aliases deterministically, applies RTL flow for Arabic, Hebrew,
and Persian, and falls back to English per missing key. Avalonia no longer
renders every UI-IR `LocalizedText` through its fallback unconditionally:
visible text, form labels/placeholders, and accessibility names consume the
selected catalog while stable node/action identities remain untouched. A real
31-choice control probe, persistence probe, and NativeAOT run gate this boundary.
All seven non-English built-ins now cover the exact 26-key core semantic UI-IR
set, with separate native `TextBlock` and accessibility-name proof for each
locale. All eight built-ins include the complete offline tutorial. The six new
non-English built-in catalogs cover all 80 stable shell keys and all six tutorial
steps with Web-aligned terminology and validated format placeholders; they remain
explicit review
candidates. Connection/forget, reverse deployment, gewyvern provisioning and
retirement, daemon retirement, startup recovery, account presentation, the
remote daemon shell, remote operation/Leselang controls, and the runtime child
workspace, plus the Orchestra plan/control/history workspace, debugger live
execution, Hub dynamic cards, the existing-runtime registration editor, and
Learning Center, now establish the
specialist-dialog pattern.
Their exact 33-key, 46-key, 43-key, 45-key, 37-key, 9-key, 36-key, 56-key,
57-key, 78-key, 72-key, 33-key, 49-key, 69-key, and 61-key catalogs bring every
non-English built-in to 750 semantic keys without touching the frozen core set. They share one strict
key/value/placeholder validator. Eight-locale native layout probes cover each
dialog and the account card at the minimum Hub envelope. The remote proof also
covers compact and wide shells plus 32 refresh/deployment/cleanup/cancellation
dialog layouts and 16 registration layouts without starting a network client. Live
language changes reproject labels, controlled
phases, status, accessibility, and flow direction without rewriting operation
identities, operator input, or raw errors. Startup retains concrete sanitized
diagnostics as opaque data; account localization projects typed session status
and cannot decide authentication state. Remote localization projects typed feed,
validated authority-health, admission, and mutation-failure facts rather than
parsing English core labels. Runtime-workspace localization projects typed
snapshot changes, severity alerts, live-refresh state, filters, diagnostic
results, and query failures while preserving runtime IDs, paths, log bodies, and
bounded remote details as opaque data. Its eight-language layout probe shares the
offline remote-shell verifier and starts no network client. Hub topology phases,
refresh summaries, runtime status, and authority-health facts are likewise typed;
its eight-language probe measures the dynamic card tree while preserving daemon
names, endpoints, runtime identities, and raw failures as opaque data. The
Learning Center probe covers all six steps, navigation semantics, accessibility,
and all 48 locale-step minimum layouts. Desktop and Web preserve the same 18-key
`leserpent.language-pack/v1` compatibility floor. All 22 official v1.1.0
artifacts now carry an exact 30-key set, adding language-selection, pack-center,
and theme copy without invalidating legacy packs. The bounded desktop decoder
fences official metadata and built-in replacement, supports SHA-256 catalog
binding, separates 18-key manual compatibility imports from official catalog
installs, and checks the current official version and exact key set before any
directory creation or atomic replacement. Artifacts rejected by that official
contract leave no store on first install; rejected upgrades preserve the prior
pack and leave no temporary file. It isolates malformed siblings and
caps directory enumeration while reading picker streams asynchronously. It
overlays only mapped presentation keys before English fallback. The native
window now selects either local Orchestra or a saved daemon as its catalog
authority and downloads with only that origin's saved CA: no bearer or admin
credential is loaded or sent. The dedicated client rejects redirects,
cross-origin paths, partial or malformed v1 catalogs, and digest/locale/version
drift; request cancellation and a single-operation fence protect the controls.
Offline JSON import remains available but is not labeled catalog-authenticated.
Rust `leserpentd` now embeds the exact official catalog and 22-pack roster behind
strict public GET paths that reject bearer/admin headers, while `/v1/*` remains
authenticated; the managed Web host mirrors that fence. The Local Orchestra
vertical now proves the native client can traverse real private-CA TLS, install
the bound pack, reload it, remove it, and restart the daemon. The 2026-08-24
macOS arm64 bundle and physical Linux x86_64 NativeAOT proofs are now retained.
The Linux gate strictly revalidates its synchronized file inventory, payload and
language-asset hashes, verifier assertions, and credential absence locally. Both
packages also persist the live authority as a saved daemon, reload exactly one
managed CA through the production catalog path, reject a decoy CA, and complete
the credential-free private roundtrip without mutating persisted inputs. Only
native-speaker and long-tail pack review remain on this localization line.
The native control proof performs a real temporary import/download/remove roundtrip. The six
candidate built-in translations plus the new 12-key downloadable expansion stay
pending native-speaker review; all 22 packs remain partial beyond their exact
30-key official set rather than claiming unreviewed coverage.
Runtime-child workspace admission has now crossed the same boundary. The
renderer-neutral `RemoteWorkspaceLaunchCoordinator` owns runtime-ID validation,
the combined active/pending limit, duplicate-request revision coalescing,
heartbeat-resistant authoritative-snapshot release, terminal-state pending
cancellation, and removed-runtime rejection. Avalonia retains only native
window lookup, display, activation, and operator-facing messages. Desktop and
mobile conformance run the identical coordinator contract, while source-boundary
TDD rejects a return of the pending dictionary or launch policy to Avalonia.
The workspace query group also exports canonical structured Leselang as one
`all(inspect, history, logs)` batch. The production Avalonia preview now sends a
strict bounded intent through authenticated `POST /v1/leselang-export`;
`leserpentd` validates it and calls the Rust HIR canonical printer. A real TLS
test parses the returned source and verifies all three typed read-query branches
or the requested mutation preserve their semantic fields. C# contains no
Leselang source templates, dynamic form requests are debounced and cancellable,
and export failure never falls back to frontend-generated source. The preview is
single-instance per workspace and copying never executes the program.

Exit: native desktop and authenticated remote/browser clients pass the same
semantic conformance suite. Mobile preserves the shared entry, secure-storage,
and lifecycle contracts; physical device release parity is deferred.

## Gate 7: 2.0 Seal

- remove frontend-exclusive business logic
- freeze version-1 command/query/effect/UI schemas
- complete security, fuzz, failure-injection, and migration audits
- publish performance comparisons against the 1.x baseline
- document packaging, rollback, recovery, and protocol compatibility
- keep the TypeScript client only if it remains a conforming renderer

Exit: every criterion in the architecture's
[2.0 definition](leserpent-2-architecture.md#20-definition) is evidenced.

The first machine-enforced Gate 7 precursor is now
`gewyvern_validate leserpent-schema-freeze`. Its bounded candidate inventory
maps command, query, effect-plan, UI IR, and wire v1 sources to a fixed native
proof registry. The independently frozen
`project/release/leserpent-2-scope-freeze.json` manifest fixes the 10 declared
core capability families, accepted closure work, explicit deferrals, authority
document anchors, and live status-cell references. The shelf rejects scope
expansion, missing deferrals, stale status references, or authority drift before
running semantic proof suites. It enforces a 65-test non-vacuity floor across
domain, UI, wire, runtime migration, legacy-wire migration, and managed
control-plane migration proof suites while emitting the actual observed count;
promotion to `frozen`
remains forbidden until the rest of Gate 7 and Apple-backed release evidence are
reproducible. Scope closure is reported independently as `scope_freeze_ready=true`.
Its companion candidate baseline pins SHA-256 fingerprints for five wire and
legacy-wire fixtures plus four renderer fixtures. This makes accidental exact
format drift fail before the semantic proof suites without pretending that the
candidate contracts have reached their final freeze.
`gewyvern_validate release-gate --leserpent-proof` now runs this shelf and the
13-suite parity/recovery shelf as one opt-in release decision. Both current-run
stage flags must be true, and both evidence directories must be present in
`release-gate-artifacts.json`; stale or partially generated proof cannot satisfy
the combined stage.
The migration replay now covers runtime journal v1 to current, v3 snapshot
generation history, complete v6 semantics, malformed migration history, and
legacy wire normalization. The managed proof covers SQLite v1 in-place upgrade,
1.x JSON-to-SQLite Orchestra history import, concurrent durable saves, and
failed-save snapshot preservation through ten locked xUnit tests. Injected
replacement failure proves SQLite transaction rollback preserves prior rows;
injected startup migration failure proves the legacy JSON remains byte-identical
and supports both a corrected SQLite retry and explicit JSON-only operator
rollback. The Linux x64 package path now also has install/upgrade/rollback proof
on a physical Linux 6.17 host.
Its locked NativeAOT restore includes the RID-specific compiler and SQLite
assets, and the bundle smoke performs staged install, distinct-release upgrade,
explicit atomic `current` rollback, configuration/state preservation, a live
Rust compatibility request, and rolled-back service health. Unsafe or missing
release links fail closed, while an unhealthy production rollback restores the
original pair. macOS arm64 now has an equivalent user-local package proof
through the native Rust installer. It accepts thin arm64 and bounded universal
Mach-O dependencies, copies symlink-free versioned app bundles, exposes one
stable launcher, rejects escaping or unmanaged links, preserves external user
state, and proves `1.4.0 -> 1.4.1 -> 1.4.0` with a live rolled-back control
fixture. The retained evidence deliberately identifies its signature as ad-hoc
with no Team ID. Provisioned Developer ID signing, notarization, stapling, and
Gatekeeper evidence remain the platform-release gap.

## Continuous Proof Shelves

Every gate maintains:

- parser and VM fuzzing
- golden syntax/HIR/IR diagnostics
- command-origin parity tests
- authorization and confirmation tests
- idempotency and revision-conflict tests
- crash/restart/re-entry tests
- journal migration and replay tests
- UI IR snapshot and accessibility tests
- IPC/HTTP/WebSocket compatibility tests
- desktop/mobile AOT smoke tests
- latency, memory, throughput, and package-size benchmarks

The parser/VM fuzz shelf now has a named native entrypoint:
`gewyvern_validate leselang-fuzz`. A fixed replayable seed covers 2048 arbitrary
UTF-8 source cases and 2048 continuation mutations. The shelf checks lossless
reconstruction, character-safe spans, deterministic syntax JSON, HIR lowering,
bounded VM startup, fail-closed continuation decoding, and canonical image
roundtrip while retaining its configuration and transcript. Its first run found
and fixed an escape-followed-by-multibyte-character lexer panic.

The UI accessibility shelf now has the native entrypoint
`gewyvern_validate leserpent-accessibility`. It audits real Avalonia controls
across all four fixtures for stable unique Automation IDs, exact names and help
text, explicit action labels, and a 4.5 WCAG AA text-contrast floor. Managed
macOS and physical Linux/Xvfb proofs produce matching counts, while the macOS
NativeAOT job requires the same accessibility marker and metrics. The first run
raised destructive-button contrast from 3.841 to 4.723.
Accessibility and NativeAOT now restore and build through proof-local .NET
artifacts roots. A concurrent regression run proves both shelves can execute
without contending for shared reference assemblies or PDBs, and successful
runs remove intermediate graphs while retaining evidence.
The NativeAOT inventory is fail closed as well: unreadable entries, directories,
symlinks, and non-UTF-8 names cannot disappear from the bounded artifact count,
and every retained evidence-index entry must be unique and resolve to a regular
file before the shelf reports success.
Release signing snapshots, resilience log directories, and package discovery
follow the same rule: enumeration errors, symlinks, unknown payloads, and
extension-shaped directories are explicit failures rather than silently
filtered absences.

The transport shelf now has the native entrypoint
`gewyvern_validate leserpent-transport`. It retains separate transcripts for
canonical wire-v1 decoding, legacy-v1 adaptation, CLI/Leselang semantic parity,
the real authenticated native CLI-to-daemon Unix socket path, and daemon IPC
security rejection paths. Gate 6 adds a server suite for the default-off
authenticated HTTPS `POST /v1/wire` server: a real TLS roundtrip, constant-time
bearer authentication, strict bounded HTTP framing, private-key file safety,
and shared wire-v1 dispatch. A seventh suite drives the native CLI through that
endpoint with explicit CA trust and proves health, query, bounded watch,
confirmed command/idempotency, and auth-error exit semantics. An eighth suite
proves strict WebSocket authentication, required subprotocol negotiation, and
cursor parsing; the TLS server suite also proves revisioned endpoint-redacted
snapshots and explicit future-cursor resynchronization. Its summary still
excludes Windows named pipes, remote GUI, and mobile clients. macOS arm64 and a
physical Linux x86_64 host pass the same eight suites, 28 tests, and 41 declared
invariants.
The shared wire-v1 transport contract is now stable: Rust rejects unknown
versioned envelope and security-relevant projection fields just as the schema
and .NET client do. Native CLI, bootstrap-health, and Gewyvern adapter exchanges
also share one absolute monotonic I/O deadline, preventing trickled response
bytes from extending an operation indefinitely. Remaining Gate 6 work is
mobile-client lifecycle and device evidence, not transport-protocol maturity.

The performance shelf now has the native entrypoint
`gewyvern_validate leserpent-benchmark`. It measures fixed-size SQLite
cold start, 256-runtime query latency, 10,000-effect batch throughput, a
1,539-node UI document's generation/diff/codec costs, a 256-log full versus
8-log incremental .NET workspace comparison, and release CLI/daemon
size. Broad fail-closed budgets guard against order-of-magnitude regressions;
raw macOS arm64 and physical Linux x86_64 results remain separate host-class
baselines with machine-readable evidence.
The first post-1.10 optimization replaces repeated whole-tree searches for
same-topology UI revisions with a linear topology check and shallow-update
collection. Structural changes retain the original convergence-checked path.
The 1,539-node reference dropped from `14.536 ms` patch p50 to `2.255 ms` in
the retained macOS shelf, with the same two patch operations and encoded
document size.
The language core now has an equally explicit 64-branch benchmark spanning
syntax, HIR, VM startup, and the complete source-to-effect path. Alternating
same-host pre/post runs reduced the four median stages by approximately 42%,
33%, 52%, and 43%. The shelf rejects changed token, branch, or emitted-effect
counts and retains its own `language-benchmark.json`, turning Leselang
performance into a release contract rather than an informal observation.
The .NET benchmark workload and every .NET parity suite now use proof-local
artifacts roots as well. A concurrent parity/benchmark run passes without
source-tree `obj` contention and removes both intermediate graphs afterward.
The Rust-to-.NET vertical's nested `dotnet run` processes also share a
test-local artifacts root that is removed automatically, closing the inner
process boundary that the outer parity harness cannot configure directly.
The benchmark driver now reports each of its five human-facing phases before
execution and routes Cargo and .NET children through bounded process waits.
The small .NET projection phase has a five-minute ceiling, while shared Cargo
proof commands retain a thirty-minute cold-build allowance. Host integration
failures therefore terminate with an explicit phase/timeout rather than
silently wedging the release shelf; JSON stdout remains progress-free.

The current command-origin and recovery shelf now has the native entrypoint
`gewyvern_validate leserpent-parity-recovery`. Thirteen non-vacuous suites
currently execute 231 tests across neutral command lowering, domain
authorization/idempotency, debugger confirmation, CLI/Leselang equivalence,
VM continuation/journal re-entry, runtime SQLite recovery injection, and the
72-test .NET control-plane security shelf, authenticated remote wire boundary,
native remote CLI parity, deterministic
Avalonia remote-state conformance, a real Rust-to-.NET WebSocket plus HTTPS
mutation and Inspect/History workspace vertical, and deterministic mobile
lifecycle recovery. The workspace proof rejects endpoint disclosure in both
stdout and the endpoint-bound cache after composing revision-matched queries. The
shelf retains per-suite transcripts and rejects any run whose filtered test
count falls below its declared minimum. This proves the currently migrated
command surface, shared local/remote CLI dispatch, and GUI event
delivery/reconnect/cache plus constrained runtime-refresh semantics. Future
mobile operations still require their own parity fixtures. Cross-host retained
counts are refreshed after every vertical-contract change rather than inferred
from earlier evidence. The current macOS arm64 and physical Linux x86_64 runs
both report thirteen suites, 231 tests, and 155 declared invariants. The Linux
summary additionally binds the result to kernel `6.17.0-35-generic`,
Rust/Cargo `1.95.0`, and .NET `10.0.109`.

The HTTPS listener is intentionally opt-in:

```bash
chmod 600 /etc/leserpent/tls/server.key
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpentd --database /var/lib/leserpent/runtime.sqlite \
  --remote-listen 0.0.0.0:9443 \
  --remote-cert /etc/leserpent/tls/server.crt \
  --remote-key /etc/leserpent/tls/server.key
```

The three remote arguments must be supplied together. The certificate and key
must be regular files rather than symlinks, and the private key must grant no
group or other access on Unix. Use a CA-issued certificate and deployment
network policy; the bearer token is never passed on the command line.

The native CLI uses the same endpoint without changing command syntax:

```bash
export LESERPENT_REMOTE='https://control.example.internal:9443'
export LESERPENT_REMOTE_CA='/etc/leserpent/tls/ca.pem'
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpent --json runtime list
```

CLI endpoint and CA flags are available as `--remote` and `--remote-ca`; local
socket and remote transport selection are mutually exclusive.

Separately, `leserpentd` can monitor a remote Gewyvern API as an authenticated
adapter target. The origin has no path, the CA is explicit, and the credential
is resolved from the native platform secret store by alias:

```bash
leserpentd --database /var/lib/leserpent/runtime.sqlite \
  --gewyvern-https-target \
  runtime-a=https://gewyvern-a.example.internal:9411,/etc/leserpent/gewyvern-ca.pem \
  --gewyvern-admin-secret runtime-a-admin
```

For environment-only development, `GEWY_API_ADMIN_TOKEN` supplies the same
credential boundary when no alias is selected. Remote targets refuse to start
without one of these credential sources. The existing
`--gewyvern-target ID=LOOPBACK:PORT` remains the only plain-HTTP form.

The desktop AOT shelf now has a named native entrypoint:
`gewyvern_validate leserpent-aot`. It detects only checked host RIDs, performs
the locked restore and no-restore publish, validates Mach-O/ELF signatures and
a bounded artifact manifest, executes all four control fixtures plus the full
presentation fixture, and retains a versioned evidence index. macOS arm64 self-host execution is automated; Linux
x64 uses the same command with Xvfb. Windows stays unclaimed until its lock and
physical-host proof exist, but that evidence is outside the current
macOS/Linux-to-Android critical path; Windows uses the TypeScript web console
meanwhile.

The 2026-08-09 physical Linux x86_64 proof closes the current presentation
portability boundary: the locked `linux-x64` NativeAOT artifact passes all 54
presentation atom profiles through real Avalonia windows under Xvfb, including
child-count external-patch waiting, persistent-mismatch timeout, focus
navigation, and virtualization preservation. Its non-vacuous, secret-free
fixture is retained at
`docs/fixtures/leserpent_avalonia_presentation_native_aot_linux_x86_64_20260809.json`
and is required by the Avalonia status cell.

The subsequent 2026-08-09 activation campaign closes the 55th atom on the same
physical host. Its stripped `linux-x64` NativeAOT executable traverses the
native button click route exactly once and rejects unavailable, hidden,
non-action, and missing targets before callback invocation. The secret-free
evidence is retained at
`docs/fixtures/leserpent_avalonia_activation_native_aot_linux_x86_64_20260809.json`
and is also required by the Avalonia status cell.

The final 2026-08-09 window-reopen campaign closes the remaining native
top-level lifecycle gap on that host. The probe observes visible native state
through open, close, reopen, and reclose, proves duplicate operations are
idempotent, and verifies that closing an adapter-owned top-level causes a fresh
native tree to be materialized from the same validated `UiDocument` and stable
node identities. This prevents stale Avalonia objects from being reparented
after their original window is closed. Its secret-free evidence is retained at
`docs/fixtures/leserpent_avalonia_window_reopen_native_aot_linux_x86_64_20260809.json`
and is required by the Avalonia status cell.

## Explicit Deferrals

The roadmap does not require:

- replacing Gewyvern's Linux-first runtime
- moving Etragon into the Leserpent core
- allowing only TypeScript as the web render path for browser access
- arbitrary user-native plugins
- model access to shell, raw HTTP, or host-language reflection
- a distributed scheduler before the local journal/runtime is proven
- Windows native desktop or WinRM parity before Windows becomes an active target
- full mobile device release parity beyond the declared entry/lifecycle contract

## First Implementation Slice

The first code-bearing slice should be deliberately narrow:

1. create the Rust domain/protocol crates
2. model `runtime.list` and one idempotent `runtime.refresh` command
3. lower one Leselang function into that command
4. execute it through an in-memory effect adapter
5. invoke the same command from the Rust CLI
6. compare both paths with the current Leserpent API fixture

This proves the architecture before parser breadth or GUI migration expands.
