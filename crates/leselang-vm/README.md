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
The composed adapter entry checks the VM-supplied saved result type before this
borrowed matcher, then retains the original query revision and output-limit
checks before moving the reply into a value. The matcher alone does not authenticate the saved envelope, enforce revisions
or budgets, validate domain records, or grant replay authority. Before/after
cross-type, refusal-order, dispatch and SQLite-restart tests preserve the original
`LSV2101`/`LSV2102`/`LSV2103` results and first-terminal replay.

`EffectResult::into_control_plane_value` now owns all eight control-plane
reply-to-value conversions. It takes the original declared result type, not
authority inferred from a received generic command result. Wrong categories and
types return None; successful conversion alone certifies neither correspondence
nor validity. The shared atomic reply entry's query revision/item gate runs before conversion,
retaining the log ceiling and inspect's legacy non-list policy. Vectors, strings
and command boxes move unchanged; inspect keeps its original boxing only after
the checks pass. The 77-type refusal/wire matrix and before/after buffer-identity,
query-policy, dispatch and journal tests preserve the released representation.

`EffectResult::into_effect_value` now composes GUI/control-plane correspondence,
declared types, query revision/item limits and owned conversion outside the VM.
`EffectValueContext` borrows original execution data; it has no implicit defaults
and is not an authority certificate. The VM supplies its saved effect, type,
operation, revision and limit, mapping `EffectValueError` back to unchanged
`LSV2101`/`LSV2102`/`LSV2103` faults. Legacy image-only query/GUI matching, mutation
command IDs, inspect's non-list behavior and the fixed log ceiling are unchanged.
The reply is consumed on success or refusal; no buffer clones, codec, meter,
dispatch or retries are added. Full value/byte/identity/authority/lease/journal
gates remain host-owned. Frozen before/after comparisons and a VM-free 70-operation
matrix retain native/wire results, fault precedence and original buffer owners.

`Value::project_scalar` now owns the product field-to-scalar mapping in the
adapter. It copies only the requested exported scalar, without serializing or
cloning whole replies; unsupported fields return None. It does not validate or
meter inputs. The VM still maps refusal to `LSV1405` and charges the original
execution budget; product capture rules are delegated to the adapter below.
The 77-type/12-field matrix and before/after frozen capture tests preserve
optional-text states, enum tokens and legacy frame behavior. This is a reference
adapter boundary, not a generic durable-frame implementation.

`ProjectedResult` and `ProjectedField` now belong to `leselang-command` with exact
VM re-exports. Capture construction, versions 1-5, ordered scalar validation,
kind/optional-text rules and stored-field correspondence move together, with no
wire conversion or payload copies at the boundary. The shared core still owns
the opaque-key scalar field and validator. Adapter failures are payload-free;
the VM retains `LSV1405`, canonical lexical/group checks, authority, budgets and
journal recovery. All 70 operations across five vocabularies keep their bytes,
including unversioned v1. Valid projections are not raw-result type admission,
trusted version provenance or a complete host-neutral recovery implementation.

The three result/group binding specializations are also exact adapter re-exports.
`ProjectedGroupExt` now owns group capture, saved member validation, ordered
signature matching and projected-result correspondence over the unchanged core
containers. The VM explicitly supplies its 1-63 member interval and original
branch-name predicate, then maps adapter refusals to `LSV1405`. Saved validation
still precedes signature/receipt comparison. The adapter has no implicit VM cap,
authority check or journal ownership; it does not admit raw result types or
original branch arguments. Before/after VM boundaries, mixed-version groups for
all 70 operations and exact alias/buffer proofs preserve this contract.

`validate_captured_projection_accesses` now owns the product's restored-scope
assembly and closed-field access policy in the command adapter. It delegates to
the shared bounded HIR walker: saved fields never widen to the current catalog,
conditional results intersect field sets, and group aliases keep exact member
names and operations. The VM supplies node/depth ceilings; the composed adapter
entry validates structure first and canonical type/group checks afterward.
The observer alone
does not validate schemas or authenticate frames; member fallback still requires
the canonical checker. Cold branches, namespace collisions and all 70 operations
across five versions retain their original behavior, bytes and `LSV1405` mapping.

`ResultBinding` is now another exact command-adapter alias of the shared core
container. `validate_result_binding` composes structure, closed-field access and
canonical type reconstruction in the original order. Callers supply capture,
group and access limits plus the group-name predicate; scalar-only captures keep
their literal-wrapping path. `validate_result_binding_structure` remains available
for earlier structural gates. The VM retains policy defaults, schema selection,
fault mapping, authentic frame admission, budgets and journal recovery. Successful
type validation alone does not authorize restore or prove the original program.

`validate_result_binding_extension` and `validate_group_capture_correspondence`
now own product capture-retention and original-row comparison policy in the
command adapter, over the unchanged generic core walkers. Callers supply limits
and group-name policy. The VM retains raw/frame admission, schema/identity/fuel
ordering and journal transactions. `CaptureReplayError` distinguishes a mismatch
from failed projection validation, preserving contextual `LSV1407`/`LSV1409`
versus `LSV1405`. Full row scans still report late malformed values after a match;
aliases never replace required original names. Frames remain borrowed, while
selected scalar projection copies and group-validation scratch remain unchanged.
These comparisons alone do not validate whole frames, raw identity or authority.

The adapter's borrowed `ResultView` now unifies raw/saved scalar projection,
exact name/operation member selection and owned capture construction. The VM
uses that same type without wrapping data, retaining only policy defaults,
metering and `ResultViewError` mapping (`LSV1402` access versus `LSV1405`
projection). Member selection borrows the original row; field reads copy the
selected scalar, while capture explicitly copies the saved projection. Saved
captures are neither upgraded nor implicitly revalidated during reads/copies;
raw group capture alone uses the supplied member limits/name predicate. Complete
admission remains mandatory before execution, especially for duplicate names,
raw reply identity and malformed saved schemas. Debug output contains no payload.
The 70-operation/five-version adapter lifecycle joins read, capture, wire restore,
complete binding validation and replay comparison without linking the VM.
Frozen before/after reference tests preserve fault order and pointer identity;
the existing re-entry/snapshot matrix retains every original fuel boundary.
This is product adapter isolation, not generic durable recovery or full extraction.

`leselang-command/tests/captured_execution.rs` now joins those data APIs to the
production HIR evaluator without linking the VM or SQLite, including test-only
dependencies. Parsed flat `seq`/`all` results pass through group/member aliases,
two computed atomic successors and a scalar tail. Every saved binding can make a
wire round-trip before re-entry with identical results, captures and total fuel.
The proof restores all five closed vocabularies, tests every smaller fuel grant,
and rejects malformed/cold-field frames, wrong replies and revoked fixture policy
before restoration. Scalar recovery/conditional successors do not swallow native
projection or fuel failures. The fixture supplies bounded headless replies and
native accounting; HIR owns expression execution and scope restoration. This is
not a second evaluator, real GUI dispatch, authentic receipts, graph-journal
recovery or a complete reference/native-VM parity claim. CI runs it separately.

Product request preparation now also lives in the command adapter:
`prepare_preflighted_effect_in_scope` borrows admitted raw effects, resolves
computed arguments and assembles computed `seq`/`all` members on the same HIR
executor, scope and fuel. The VM supplies its original limits and maps typed
`EffectPreparationError` variants to unchanged fault/recovery channels. No new
interpreter, fresh meter, dispatch or authorization is introduced. This low-level
hook assumes whole-expression admission; it does not validate opaque raw payloads
or enclosing group counts/names. Native hooks must not dispatch. Preparation is
declaration-ordered even for `all`; failure does not refund previous work.
Frozen before/after tests preserve requests, borrow/own behavior, fuel, scope and
fault order. The adapter-only proof now also runs computed group parameters
through wire re-entry and all fuel cutoffs, with cold refusal and late failure
checks. Authenticated dispatch and durable graph recovery remain separate gates.

The product snapshot/re-entry bridge is also shared with the command adapter.
`snapshot_preflighted_result_scope` and
`resume_preflighted_result_binding_in_scope` supply product views to the existing
core/HIR kernels; `ResultCaptureHost` supplies explicit scalar/result/group
accounting, projection-error mapping and saved-projection refusal policy. The VM
retains its original costs and group restrictions, without owning another set of
row-conversion closures. Scalars charge before copying, snapshots project before
charging, and restored saved projections remain exact borrows. Shared insertion
order and scope cleanup are unchanged, including native refusal and unwind.
Both the full adapter-only execution proof and focused policy tests use this
production bridge. It assumes prior complete admission, adds no default meter or
authority, and is not a journal codec, dispatch driver or durable recovery API.

`PreflightedResultEnvironment` now connects those shared product mechanisms for
both the VM and its VM-free proof: field/member access, recursive preparation and
snapshot construction no longer need duplicated environment implementations.
`ResultExecutionHost` extends capture policy with explicit cold/selected request
checks, prepared-data observation, selected-field policy and typed failure maps.
The host is borrowed, may be unsized, and stays the same through computed groups.
`ResultExecutionLimits` is explicit; the outer HIR evaluator uses its effect limits
and the original meter. Construction does not admit input or grant dispatch.
The VM preserves its prior admission gates, costs, faults and saved-row policy.
Focused tests cover native ownership, cold/late refusal, every fuel cutoff and
unwind cleanup; no public VM, wire, schema or durable-recovery contract changes.

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
The shared reply entry calls it before `Value::from(reply)` moves the owned reply
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
validation, correspondence or admission. `validate_value_items` now composes those
leaves with adapter-owned structured traversal using an explicit borrowed stack.
Callers supply depth/field/query ceilings and the name predicate; duplicate names
remain local to each structure, with each child checked before the next sibling.
The VM retains original limits, `LSV2403` messages, per-execution budgets and
encoded-size gates (`LSV2102` ordering unchanged). Saved navigation and max-length
values retain their legacy rules. The native value model is unchanged.

The traversal copies no payloads and uses no recursive Rust calls. Its saturating
item count is not an exact-overflow certificate or an aggregate-work/byte limit;
those remain caller gates. Frozen VM comparisons preserve all starting-depth and
field/child refusal boundaries. The VM-free capture proof uses the same validator;
the 77-type matrix, explicit policy, deep-value and hook-unwind tests cover the
adapter directly. No receipt authority, schema, journal or product version changes.

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
