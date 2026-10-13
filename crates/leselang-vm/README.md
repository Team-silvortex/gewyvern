# Leselang VM

One shared compiler/session implementation with an explicit optional Leserpent
reference specialization. This crate remains in the product workspace while
complete language, recovery and compatibility extraction gates are open.

## Build Profiles

| Profile | Cargo Features | Entry Points |
| --- | --- | --- |
| Native in-memory | `default-features = false` | `native::{syntax, hir, core, Vm, LocalVm}` |
| Reference in-memory | `default-features = false`, `leserpent-profile` | Released root `Vm`, product operations, ephemeral journal |
| Reference durable | Defaults, or `sqlite-journal` | Released root API and SQLite journal |

The default profile is unchanged. Consumers of the previous database-free root
API must explicitly enable `leserpent-profile`. `sqlite-journal` implies that
profile; there is no silent fallback when a durable journal cannot open.
Cargo features are additive, not a capability or security boundary.

## Product Protocol Ownership

`leselang-command` owns the Leserpent `EffectOperation`, `EffectResult`,
`PresentationEnvelope`, `PresentationOperation`, `PresentationResult`,
`DebuggerCancelResult`, `Value` and `StructuredField` payloads. The reference VM re-exports those exact types
at their released root paths; no wrapper, conversion, wire tag or schema change
is involved. `leserpent-protocol` consumes the adapter directly and no longer
links the VM or SQLite merely to describe a pending GUI operation.

`PresentationOperation::from_effect` also maps all 62 GUI effects in the adapter,
including their fixed wait timeouts. The VM constructs one common authority
envelope around that payload after charging the original single fuel unit, then
runs its unchanged request validation. Non-GUI effects retain the command/query
lowering path, confirmation and identity rules. The mapper is only a data copy,
not a validation certificate, and does not expand groups or run computations.
Frozen request bytes and refusal precedence are checked both before and after
this migration by `src/presentation_request_tests.rs` without requiring SQLite.

`PresentationOperation::matches_effect` owns the borrowed request payload checks
for those same 62 effects. The VM selects GUI operations through the existing
host signature catalog, then checks capability/envelope identity before matching
the payload. Image, successor grants, canonical token and effect ID still precede
that stage; budget equality still follows it. A non-GUI/GUI category mismatch
retains `LSV2013` rather than running the wrong envelope's authority checks.
Wire decoding may reject malformed principals before admission begins; native
requests retain their original VM fault order. This matcher does not validate
canonical source, restore ownership, authenticate a caller or authorize dispatch.

The adapter's `query_matches_effect` and `command_matches_effect` now own the
four query and four command payload rules. Fixed log pagination, exact filters,
deployment targets, command/idempotency IDs, revision, Leselang origin, non-dry-run
execution and operation-specific confirmation retain their released semantics.
The VM still generates expected IDs from the canonical continuation sequence and
supplies the saved revision, never expectations copied from a received command.
It checks authority first and budgets afterward; cross-category refusals and
native/wire/restore fault order are frozen by `src/control_plane_request_tests.rs`.
These borrowed predicates do not authenticate envelopes, validate canonical input
or provide replay protection. Non-language registration commands still fail closed.

`EffectResult::matches_control_plane_effect` owns the eight control-plane reply
correspondence rules in the adapter. Queries keep runtime/history/log identity
checks and legacy image-only matching; mutations require the original saved
command ID, applied status where applicable, and the debugger timestamp bound.
The VM checks its saved result type before this borrowed matcher, then keeps the
original query revision and output-limit checks before moving the reply into a
value. The matcher does not authenticate the saved envelope, enforce revisions
or budgets, validate domain records, or grant replay authority. Before/after
cross-type, refusal-order, dispatch and SQLite-restart tests preserve the original
`LSV2101`/`LSV2102`/`LSV2103` results and first-terminal replay.

`EffectResult::into_control_plane_value` now owns all eight control-plane
reply-to-value conversions. It takes the original declared result type, not
authority inferred from a received generic command result. Wrong categories and
types return None; successful conversion alone certifies neither correspondence
nor validity. The VM's single query revision/item gate runs before conversion,
retaining the log ceiling and inspect's legacy non-list policy. Vectors, strings
and command boxes move unchanged; inspect keeps its original boxing only after
the checks pass. The 77-type refusal/wire matrix and before/after buffer-identity,
query-policy, dispatch and journal tests preserve the released representation.

`Value::project_scalar` now owns the product field-to-scalar mapping in the
adapter. It copies only the requested exported scalar, without serializing or
cloning whole replies; unsupported fields return None. It does not validate or
meter inputs. The VM still maps refusal to `LSV1405`, owns capture versions 1-5,
field order and semantic validation, and charges the original execution budget.
The 77-type/12-field matrix and before/after frozen capture tests preserve
optional-text states, enum tokens and legacy frame behavior. This is a reference
adapter boundary, not a generic durable-frame implementation.

Continuation result-type validation now reuses the HIR host signature catalog,
instead of maintaining a second 70-operation type table in the VM. Frame/schema,
token syntax and program counter still precede signature lookup; binding and
resource checks follow it. Structured/compute frames retain `LSV2015`, wrong
result tags retain `LSV2016`, and canonical source/sequence checks remain separate.
`src/continuation_contract_tests.rs` freezes all 70 signatures, wire and memory
restoration, cross-type refusals and SQLite corruption/reopening before and after
the change. A refused restore does not replace pending work or advance IDs, and
journal loading does not repair a corrupted result tag. This removes duplicated
reference policy, not product types or the remaining host-neutral recovery gate.

`PresentationResult::matches_effect` checks all 62 reply payloads in the adapter
using borrowed data, including original operation correspondence, fixed wait
timeouts, optional-text distinctions, child-count bounds and focus navigation.
The VM calls it before the adapter's `Value::from(reply)` moves the owned reply
into its original result shape, then checks `Value::result_type()` against the
saved result type. Strings are moved rather than copied. Legacy image-only resume still permits a
missing operation, while saved-request resume checks the original envelope.
This predicate does not authenticate an envelope or admit a continuation.
Type checks, `LSV2103` mismatch faults, terminal replay and authority gates remain
in the reference VM; characterization tests passed before and after migration.

The product value enum and its ordered, recursive fields retain their exact
Serde declarations and shared scalar type. Their legacy unknown-field decoding
policy is unchanged; this is not the strict daemon wire-v1 envelope format.
Type tags and reply conversion are data operations, not payload validation.
`Value::bounded_leaf_items` now checks the released product leaf rules in the
adapter without copying strings or traversing structures. Empty queries count
as zero and empty scalar lists as one; list/history ceilings are caller-supplied
while logs retain their fixed product limit. This is not full domain-record
validation, correspondence or admission. The VM still owns nesting, field-name
uniqueness, aggregate/per-execution item budgets and encoded size, preserving
the original `LSV2403`/`LSV2102` faults and their order. Saved navigation and
max-length values retain their legacy rules. The native value model is unchanged.

The adapter's closed matrix covers all 62 presentation operations and replies,
including strict field decoding, optional text and focus-navigation asymmetry.
Reference re-entry still consumes those original types. These are protocol data,
not grants or authenticated receipts: validation, revision fences, dispatch,
continuation ownership and journal recovery remain unchanged in the VM/host.
The reference VM still depends on product types; this removes a reverse product
dependency, not the remaining complete-core extraction or GUI parity gates.

## Native Boundary

`native::Vm` is exactly HIR's `StackedControlSession`, not a parallel evaluator.
It borrows original IR and retains one fuel meter across synchronous reply-gated
slices. Hosts provide catalogs, live policy, authenticated identities, native
dispatch, bounded captures and cancellation delivery. No timer, thread pool,
global lock, database, GUI toolkit or product DTO is required in the native
normal/build dependency closure.

`native::LocalVm` is the optional `LocalStackedControlSession` from shared HIR.
It takes a `LocalExecutionScope` whose weak lease the host has installed in its
native authority, requests and result domains. The original session still owns
the sole meter and all evaluation/reply state. Completion/failure close the scope
before returning; cancellation and Drop close before retained-session cleanup.
An escaping native unwind closes the scope even when the caller catches it and
keeps the wrapper. Ordinary pending reply rejection preserves both input and
liveness. This wrapper is intentionally !Send/!Sync; `native::Vm` is unchanged.
It does not preempt callback-local cleanup or cancel external native collectors.
Reply validation uses shared `PendingReply::try_accept_in_scope`: an escaping
identity/policy/view/type/value panic revokes before the core drops its pending
identity, frames or input, while ordinary rejection leaves the scope open.
The same scoped entry also stops after synchronous revocation inside a normal
callback. A normal Cancelled rejection seals the enclosing session instead of
rearming a dead handle. Callback/cleanup unwind still follows the conservative
HostUncertain path; unscoped acceptance does not infer this local lifetime policy.
For externally owned scopes, HIR's legacy/prepared Seq and All collectors also
offer `try_accept_in_scope`: normal revocation seals Cancelled, drops unpublished
suffix/results and preserves the original rejected input. Ordinary refusals stay
retryable. This does not expose LocalVm's private owner or automatically cancel
an enclosing session, external dispatch, handed-off results or other executions.
Their explicit `poll_in_scope` also gates request publication and All result
handoff, stops after callback revocation and fences retained prefix/suffix cleanup.
Original fuel is never refunded; ordinary preparation refusal remains terminal.
Start/correlation and already handed-off requests still require host liveness checks.
Polling also fences retained re-entry owners, rejected restored prefixes and
failed successor correlation through the same execution path. Successful language
recovery keeps the lease open and exactly preserves the unscoped VM's fuel.
Already-consumed callback inputs and deeper evaluator temporaries remain outside
these cleanup boundaries; this is not native callback preemption or rollback.

Wrap adapter work between poll and acceptance with `LocalVm::with_host_work`.
Only Awaiting enters the callback; other states return the original input.
Normal output, including a native refusal, remains verbatim and does not accept
or resume the session. Escaping callback unwind seals the original core
HostUncertain and revokes before retained pending-state cleanup. The host still
authenticates the input, bounds work and owns external cancellation/rollback;
this boundary does not make dispatch exactly once or sandbox native callbacks.
For normal native returns that must end the execution, use `with_host_outcome`
and `native::HostWorkOutcome::{Pending, Cancelled, Failed, Uncertain}`. Only Pending
retains the session; the other variants close before pending cleanup and preserve
Cancelled, Failed or HostUncertain even if cleanup panics. Outputs remain original
host-owned data. Return collectors/queues inside Cancelled(output), then explicitly
cancel/dispose them after the lease is revoked; prior effects are not rolled back.
The outer Ok means the callback ran, not that the operation succeeded. Neither
panic nor error-code inference is needed; no wire format or automatic retry is added.

`tests/native_embedding.rs` proves two scalar-host source/helper/capture/reply
verticals and negative policy, receipt, cancellation and fuel cases.
`tests/native_host_pipeline.rs` runs a continuous atomic-program matrix for a
stateful integer counter and a structured text/visibility GUI-acknowledgement
model. Both use the same parser/compiler and native LocalVm, with loops, folds,
helper returns, captured predecessors and two effects. The matrix also exercises
every fuel budget below completion, stale/invalid receipts, live policy changes,
four cancellation boundaries and shared-IR execution isolation. It runs unchanged
in the exported standalone consumer. No new runtime API or evaluator is added.
The mixed matrix connects atomic calls, a seq/all/repeat group, a captured atomic
successor and a second seq/all group through the original prepared collectors
and group assembly. The two hosts cover twelve program combinations, reverse
parallel replies, group provenance/order/count rejection, parent cancellation,
policy revocation, shared-IR isolation and every smaller fuel budget. This is
bounded flat-group composition, not a new scheduler or durable exactly-once fence.
These proofs are not complete reference-language/group coverage, a real GUI adapter, durable recovery
or the planned full `host_embedding.rs` proof. The source checkpoint currently
exports syntax/runtime-core/HIR and this native facade, not the complete VM or
the product/reference profile. All-features builds of that checkpoint stay native.
See the [embedding contract](../../docs/leselang-embedding.md#native-vm-build-profile)
and [extraction gates](../../docs/leselang-embedding.md#repository-extraction-readiness).
