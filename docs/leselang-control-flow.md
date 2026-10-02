# Leselang Control Flow

This is the implemented sequential-control contract, not a claim of Bash language
parity. It extends the [language contract](leselang-language.md) without shell
execution, hidden coercions, or source-level async/await.

The post-2.0 control-flow contract is evolving at version `0.12.0`; it does not
reclassify the older stable atomic-effect contract as a complete language.

These are language-core mechanisms, not Leserpent-only GUI macros. Their
current effect vocabulary and VM integration remain product-bound; the
[embedding architecture](leselang-embedding.md) defines how they will serve
independent hosts without claiming that extraction is complete.

## Calculation And Decisions

Basic computation is executable, not source-text substitution. Scalars are
unsigned 64-bit integers, booleans, strings bounded to 4096 UTF-8 bytes, and
`none`. A pure program finishes as `Step::Done(Value::Scalar { .. })` without
allocating an effect identity or journal record.

```leselang
fn main() = bind(
  count: add(left: 2, right: 3),
  body: choose(
    when: ge(left: count, right: 5),
    then: mul(left: count, right: 2),
    otherwise: 0,
  ),
)
```

This returns the integer `10`. A pure `bind` accepts one named scalar value or a
read-only alias of an already captured result, plus `body`; the value is evaluated once, and the name is visible only in the
body. A binding may use earlier enclosing bindings, but not itself, a later
binding, or a shadowed active name. Names are ASCII identifiers of at most
64 bytes; `body` and literal/grammar keywords cannot be bound. Bindings are
immutable. Argument order does not change lexical scope or execution order.

`choose` requires `when`, `then`, and `otherwise`. The condition must be boolean,
and both branches must have the same HIR result type. Only the selected branch
executes. Both branches are still type-checked, and all declared host operations
contribute to the capability preflight, including operations in the unselected
branch. `Structured` is currently a coarse aggregate type, not a field-by-field
record schema. Statically bound flat groups retain their named member types for
[`member`](#named-group-result-bindings); this does not infer interchangeable
record shapes across `choose` branches. Atomic host-result bindings support only
the explicit scalar projections described in [result bindings](#result-bindings).

The primitive operations are:

| Operations | Named Inputs | Result And Rules |
| --- | --- | --- |
| `add`, `sub`, `mul`, `div`, `rem` | `left`, `right`: integers | integer; checked overflow/underflow, no division by zero; division truncates |
| `lt`, `le`, `gt`, `ge` | `left`, `right`: integers | boolean |
| `eq`, `ne` | `left`, `right`: same scalar type | boolean; no coercion |
| `and`, `or` | `left`, `right`: booleans | boolean; left-to-right short circuit |
| `not` | `value`: boolean | boolean |
| `concat` | `left`, `right`: strings | string; combined output must fit 4096 bytes |
| `len` | `value`: string | integer Unicode scalar-value count, not bytes or grapheme clusters |
| `to_string` | `value`: integer, boolean or string | string; decimal integer, lowercase boolean, or unchanged text |
| `parse_integer` | `value`: string | integer; non-empty ASCII decimal within `u64`, leading zeros accepted |
| `parse_boolean` | `value`: string | boolean; exactly `"true"` or `"false"` |

There are no signed integers, floats, infix operators, implicit truthiness or
implicit conversions in this slice. Unknown, duplicate or extra arguments are
rejected. For example, `choose(when: true, then: 7, otherwise: div(left: 1,
right: 0))` returns `7`; the unselected arithmetic is not evaluated. An invalid
type or undefined name in that same branch is still a compile-time error.

A decision can select existing host effects, including an entire sequence:

```leselang
fn main() = bind(
  ready: eq(left: add(left: 2, right: 3), right: 5),
  body: choose(
    when: ready,
    then: seq(
      focus: ui.focus(node_id: "runtime-a"),
      verify: ui.assert_visible(node_id: "runtime-a"),
    ),
    otherwise: seq(fallback: ui.focus(node_id: "runtime-b")),
  ),
)
```

Use real exported node IDs in a host. The existing debugger accepts this form
when it suspends at a debuggable effect; it still rejects a pure calculation
that finishes without suspension. Pure results are available through the Rust
VM API, not a newly introduced CLI calculator or desktop result panel.

Computation can run before the first host suspension, and after one captured
atomic result to return a scalar or prepare the next atomic operation. Local bindings can fill host-operation
arguments, including inside bounded groups, but cannot provide `repeat.times`.
General computation must wrap `seq`, `repeat`, or `all`, or occur inside an
atomic member's arguments; it cannot replace a group step with `bind`/`choose`.
An unexecuted mixed group still cannot contain unsupported nested computation.
Bounded atomic result chains can span multiple suspensions; dynamic groups and
effectful operands remain unsupported.

Each evaluated expression costs one unit of the same VM fuel used by subsequent
effects. String materialization, copying and operations additionally charge one
unit per started 64-byte block of each involved string. HIR validation bounds
the combined expanded computation/embedded-effect graph to 1024 nodes and retains
the 16-level source-call depth fence. Source and literal limits apply before work.
Checked arithmetic faults use `LSV1401`, invalid computational state uses
`LSV1402`, and oversized computed strings use `LSV1403`; fuel exhaustion remains
`LSV1001`. Rejected computed host arguments use `LSV1404`, including the original
host-validator code but not the rejected value. Invalid saved result bindings
use `LSV1405`; exhausted loop iteration limits use `LSV1406`. Invalid successor
context or missing original authority uses `LSV1407`. Invalid scalar text uses
`LSV1408`, without echoing the rejected input. Invalid group-result metadata or
missing complete group context uses `LSV1409`. Computation
diagnostics use `LSH1401` through `LSH1412` with spans.

Only the selected, fully determined host graph is journaled, with the remaining
fuel and original deadline. Recovery consumes that graph rather than evaluating
the source condition again. A program that never suspends has no durable
continuation. A result-binding program saves its scalar environment and bounded
body alongside the selected atomic request.

## Explicit Scalar Conversions

Conversions bridge typed calculations and string-valued host arguments, without
implicit coercion or a shell/template interpolation layer:

```leselang
fn main() = bind(
  counted: ui.assert_child_count(node_id: "rows", count: "7"),
  body: ui.set_form_value(
    node_id: "form",
    field: "replicas",
    value: to_string(value: add(
      left: field(value: counted, name: "count"),
      right: 1,
    )),
  ),
)
```

After the host confirms seven children, this requests the form text `"8"`.
The example requires `ui.presentation` and real host-exported node/field IDs;
it does not inspect arbitrary GUI objects. Every conversion takes exactly one
named pure `value`. Raw host results and `none` are not printable values.
`to_string` preserves strings byte-for-byte, without adding quotes, escaping,
trimming or localization. It prints integers in canonical decimal and booleans
as lowercase `true`/`false`.

`parse_integer` accepts only non-empty ASCII decimal digits in `0..=u64::MAX`.
Leading zeros in text are accepted: `parse_integer(value: "0007")` returns `7`;
integer source literals still forbid leading zeros. Signs, whitespace, separators,
alternate bases, exponents, non-ASCII digits and overflow fault with `LSV1408`.
`parse_boolean` accepts exactly `"true"` or `"false"`, without case folding or
numeric truthiness. Conversion faults name the expected format, never the input.

The 4096-byte string bound and shared fuel apply to input processing and output
materialization. Cold branches remain type-checked but are not evaluated.
Converted host arguments still pass the original host validators; converting
text does not make an invalid node ID or form value valid. Whole-group preparation
remains all-or-nothing: a later conversion failure admits no member of that group.

These operators use the existing unary HIR shape, continuation schemas 1 through
5 and journal schema 10; no storage-layout migration is needed. Saved bodies and
typed lexical frames retain the operators under the original authority, deadline
and remaining fuel. After completion, duplicate results replay the committed
value, fault or successor instead of rerunning conversion. Older readers reject
unknown operators rather than discarding them; already-materialized atomic
requests and the wire encodings of `not`/`len` are unchanged.

## Bounded Pure Loops

```leselang
fn main() = bind(target: 17, body:
  loop(size: 1,
    while: lt(left: size, right: target),
    next: mul(left: size, right: 2),
    limit: 6,
  ),
)
```

This returns `32`. `loop` takes exactly one named initial scalar state plus
`while`, `next`, and `limit`. The initial expression runs once in the enclosing
scope. Its name is then visible to the condition and next-state expression,
but not to its own initializer or outside the loop. The name follows the same
bounded, non-shadowing rules as `bind`; `while`, `next`, and `limit` cannot name
this loop's state. Argument order does not change these scopes or evaluation
order. Enclosing locals are readable and remain immutable.

`while` must be a pure boolean expression. Before each step, the VM evaluates
that condition against the current state. A false condition returns the state
without evaluating `next`. A true condition evaluates `next` once and replaces
the loop-local state with that value. Initial and next state must have the same
scalar type, including strings or `none`; no coercion, raw result object or
host operation is allowed in any component. All components are type-checked
even when the limit is zero or a branch is cold.

`limit` is a literal from **0 through 1024**, not a computed expression. It caps
state transitions, not condition checks: after exactly `limit` transitions,
the final condition still runs. If it is false the loop succeeds; if it is true
the VM returns `LSV1406`, never a silently truncated value. With `limit: 0`, an
initially false condition succeeds and an initially true condition faults.
Condition or arithmetic faults propagate normally. `choose` and short-circuit
logic remain lazy inside loops; there are no `break`/`continue` statements.

Execution iterates over one HIR body rather than cloning/unrolling it. The
1024-node graph and 16-level nesting limits still apply. Initial values, every
condition check and every next-state calculation use the shared fuel budget,
including normal string-copy costs and 4096-byte string limits. A nested loop
has its own transition cap but does not receive fresh fuel. Fuel may run out
before the transition cap, returning `LSV1001` instead.

A pure loop can select or prepare subsequent host work, including computed
group arguments, without changing whole-group all-or-nothing preparation. It
can also consume typed fields after one captured atomic result:

```leselang
fn main() = bind(result: runtime.list(), body:
  loop(count: 0,
    while: lt(left: count, right: field(value: result, name: "count")),
    next: add(left: count, right: 1),
    limit: 64,
  ),
)
```

This requires `runtime.read` and faults if more than 64 transitions are needed.
Result-binding continuation schema 2 carries the bounded loop HIR in its pure
body; the existing scalar environment and 64 KiB image limit apply. Old decoders
that do not understand the `loop` node reject it rather than discarding it.
Atomic/group schema 1 remains unchanged; the current journal is schema 10.

The loop runs synchronously; there is no mid-loop checkpoint or host effect.
After a validated result, its scalar output or fault is committed through the
existing completion transaction. Restart/retry before that commit may repeat
pure calculation; a duplicate after commit replays the original outcome. The
saved remaining fuel, cancellation and trusted scheduler deadline fences still
apply. This does not add effectful loops, collection iteration, mid-loop host
suspensions or a scalar inspector to the GUI.

## Pure Calculation Recovery

```leselang
fn main() = recover(
  value: parse_integer(value: "automatic"),
  fallback: 3,
)
```

This returns `3`. `recover` requires exactly `value` and `fallback`, both pure
expressions of the same scalar type. It evaluates `value` once and returns a
successful result unchanged, including `0`, `false`, `""` and `none`. It is not
a truthiness/defaulting operator. Named argument order does not change which
expression runs first. Only a recoverable failure evaluates `fallback`, once,
in the enclosing scope. Locals and loop state created inside a failed value do
not leak into the fallback. Both sides are type/scope checked even when cold;
non-scalar results, mismatched types and effectful operands use `LSH1411`.

The recoverable set is closed: only `LSV1401` (checked arithmetic failure) and
`LSV1408` (invalid integer/boolean text). Fuel exhaustion, string bounds, loop
iteration limits, invalid HIR, invalid saved frames and host-validation failures
are not recoverable. Cancellation, deadlines, authorization, dispatch failures
and journal errors remain outside this construct. A host error named `LSV1408`
is still a host failure, not a local parse failure. This is not catch-all,
exception-object binding, implicit retry, rollback or cleanup.

The recovery expression, failed value and selected fallback share the remaining
fuel; failed work is never refunded. Fallback failures propagate normally. An
explicit enclosing `recover` may catch a fallback's arithmetic/parse failure,
but nesting cannot hide a resource limit or replenish the budget. A large or
deep cold fallback still counts toward the 4096-byte scalar, 1024-node graph,
16-level source-call and 64 KiB continuation limits.

Recovery can prepare host parameters or run after a validated atomic result:

```leselang
fn main() = bind(counted: ui.assert_child_count(node_id: "rows", count: "0"), body:
  ui.set_form_value(node_id: "form", field: "batch_size", value: to_string(value:
    recover(
      value: div(left: 100, right: field(value: counted, name: "count")),
      fallback: 1,
    ),
  )),
)
```

With real host-exported IDs and `ui.presentation`, the successful zero-child
assertion leads to form text `"1"`. If the assertion itself fails, no fallback
form action is requested. Converted/recovered parameters still pass the original
host validators. In a prepared group, an unrecovered later argument failure
admits no member. General computation cannot become a group member.

The `recover` HIR node is stored in existing result-binding schemas 2 through 5
under the original authority, deadline and remaining fuel; atomic/group schema 1
and journal schema 10 remain unchanged. No database migration is required.
Old readers reject an unknown `recover` node rather than silently omitting it.
Before commit a retry may repeat pure work; after commit, duplicate results replay
the chosen scalar, fault or successor without recomputing the fallback. Raw host
result validation still happens before the body is evaluated. This slice does
not implement recovery around host effects or add a pure calculator to the GUI.

## Result Bindings

```leselang
fn main() = bind(
  expected: "runtime-b",
  body: bind(
    moved: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
    body: eq(
      left: field(value: moved, name: "focused_node_id"),
      right: expected,
    ),
  ),
)
```

This returns a boolean derived from the actual, validated focus destination,
not a guessed target. Use real node IDs exported by the host. The same pattern
can bind `runtime.list()` and compare `field(value: result, name: "count")`
against an integer.

An atomic result-binding `bind` takes one host call, literal or computed. Its
body returns a pure scalar or continues an [atomic result chain](#durable-result-chains).
The scalar body may contain lexical bindings, arithmetic,
short-circuit logic, lazy `choose` and bounded pure `loop`. A result can be aliased in the body but
cannot be returned as an unprojected object. `field` requires exactly `value`
and `name`; the name must be a string literal exported for that result type.
There is no dynamic property lookup, reflection, optional-field coercion or
implicit execution of `field(value: host.call(), ...)`.

| Result Types | Exported Fields | Scalar Type |
| --- | --- | --- |
| `runtime.list`, `runtime.inspect`, `runtime.history`, `runtime.logs` | `revision` | integer |
| `runtime.list`, `runtime.history`, `runtime.logs` | `count`, the length of the returned collection | integer |
| All current `ui.*` results | `node_id`, the requested node | string |
| `ui.navigate_focus` | `focused_node_id`, the actual destination | string |
| `ui.assert_child_count`, `ui.wait_child_count` | `count` | integer |
| `ui.assert_form_field_max_length`, `ui.wait_form_field_max_length` | `max_length` | integer |

Only these fields are exported. Collections, arbitrary record fields, nullable
metadata and command-result properties are not exposed by this slice. Commands
can still be bound with an unused result and a scalar body; their capability,
confirmation, revision, lease and acknowledgement requirements do not change.

At suspension, continuation **schema 2** saves the resolved atomic request,
enclosing scalar locals, capture name and pure body. The body/environment is
revalidated through the normal lexical/type rules on restoration. The full image
must fit 64 KiB; locals retain their 4096-byte string and nesting limits. Schema
1 atomic/group images keep their old representation with no `result_binding`.
Schema 2 without a pure binding and schema 1 with any binding are rejected.
The current SQLite journal is schema 10; old binaries must reject rather than
discard unknown continuation forms.

The host result must pass the original request/result, revision, item-count and
payload-size checks before projection. Wrong replies, cancellation, deadline
expiry and terminal effect failures do not become successful calculations.
The body uses the saved remaining fuel; restarting with a different startup
budget does not replenish it. Saving/restoring scalar locals also charges their
copy cost. Result objects are borrowed during evaluation, not repeatedly cloned.
The scalar outcome or calculation fault is committed by the existing completion
transaction; duplicate delivery replays the first committed output. A failed
transaction may retry pure calculation, but never dispatches another effect.

The native debugger can acknowledge this single presentation and reach its
normal completed/failed state. The scalar is returned by the Rust VM; this does
not add a scalar inspector to the desktop or expose locals in public projections.
Atomic result-bound bodies cannot capture a group or replace a group member. Further
atomic captures use the durable chain rules below. Use an outer `choose` to
select a result-binding program, rather than hiding effects in operands or host
arguments.

## Named Group Result Bindings

```leselang
fn main() = bind(
  checked: seq(
    focus: ui.focus(node_id: "runtime-a"),
    verify: ui.assert_visible(node_id: "runtime-a"),
  ),
  body: eq(
    left: field(value: member(value: checked, name: "focus"), name: "node_id"),
    right: field(value: member(value: checked, name: "verify"), name: "node_id"),
  ),
)
```

The group completes before this pure scalar body executes. `bind` can capture
`seq`, bounded `repeat` or flat `all`, with literal or computed atomic members.
`member` requires exactly a bound group reference as `value` and a literal step
`name`. It returns that member's statically known host-result type for the existing
`field` projections, not a dynamic property or arbitrary host object. Group and
member aliases are immutable and scoped to the body. A missing name, non-group
reference or non-literal name is a compile error (`LSH1412`); fields belonging to
another operation remain invalid (`LSH1409`). Cold branches are checked too.

Flattened names are unchanged: `seq(outer: seq(inner: ...))` exports
`"outer__inner"`, and `repeat(times: 2, body: ...)` exports `"iteration_1"` and
`"iteration_2"`. Member order never depends on completion timing. A parallel
group stays parallel; its body waits for all members. Sequential failures still
close blocked successors. Wrong replies, host errors, cancellation or deadlines
do not run the body, even if it contains a pure `recover` fallback.

This slice requires a pure scalar body. It permits scalar calculation, aliases,
`choose`, bounded pure `loop`, conversions and `recover`, but no group-driven
host effects, raw group/member return, capture inside an atomic result chain,
dynamic names, mixed `all`/`seq` or inferred group shape across `choose`. Every
group argument is prepared before dispatch, not from an earlier member's result.
Capability preflight includes every member, even when its result is unused.

Continuation **schema 6** marks each atomic member with its owning group token.
The merge plan uses explicit `bound_parallel` or `bound_sequential` order markers
and stores the resolved group signature, scalar environment and pure residual
body. The entire plan must fit 64 KiB before group admission. It contains no raw
result objects; lexical, string, node and nesting limits remain unchanged.
Both group orders reserve one effect unit per member from the shared remaining
fuel; the body receives what remains after all members, not a refreshed budget.
Scalar restoration and member/field evaluation also consume that budget.

Recovery requires the complete original journal. Both `Vm::restore(image)` and
`Vm::restore_request(request)` reject isolated schema-6 members with `LSV1409`;
an individually valid image cannot reconstruct the group's calculation. Journal
reopen validates owner links, exact member requests/types, original authority,
shared budgets and the saved body. The last member and final scalar or calculation
fault commit in one transaction. Failed commits may retry pure calculation;
successful commits replay the saved outcome without rerunning the body. Raw
aggregate item/byte limits apply before projection. Retention removes the whole
completed group, including single-member sequences, as one logical record.

The SQLite journal remains schema 10 because the table layout is unchanged.
Old readers reject schema 6 and the new order markers rather than silently
discarding the body. Unbound group plans and schema-1 images retain their wire
representation. The native debugger supports sequential presentation groups
through its existing correlated acknowledgement channel; the scalar is available
through the Rust VM, not a new desktop inspector. Flat `all` remains available
through the Rust batch API, not the debugger's single-presentation channel.

## Result-Driven Successor

```leselang
fn main() = bind(
  moved: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
  body: ui.focus(node_id: field(value: moved, name: "focused_node_id")),
)
```

The first validated result supplies the actual destination of the second
operation. An atomic tail effect can have pure computed arguments, enclosing
pure `bind` preparation, and lazy `choose` selection between atomic effects of
the same result type. Pure loops may prepare its arguments, but cannot execute
host operations themselves. Both conditional branches are type-checked and
included in capability preflight before the first effect.

The schema-3 representation covers this two-effect case: one captured atomic
result followed by one uncaptured atomic successor. The successor returns its
own typed host value, not a named two-field aggregate. Repeated captures and a
third suspension now use [schema 4](#durable-result-chains). Result-dependent
groups and effectful loops remain unsupported, including in cold branches.
`seq` remains the separate form for precomputed named sequences.

The first continuation uses **schema 3**, saving its bounded body and scalar
locals. The concrete successor uses schema 1 with no result binding. The original
principal, capabilities, expected target revision, output limit and absolute
deadline are preserved; successor computation consumes the remaining fuel and
never receives a fresh allowance. Both retained results also obey the existing
bounded sequence-output total. Command confirmation and correlated dispatch
acknowledgement are still required for mutating operations. Target revisions are
not automatically refreshed from an unrelated result or debugger session revision.

Journal schema 8 introduced the first completion and successor dispatch in **one
transaction**, using the existing ordered graph storage with a `result_chain`
plan; the current schema-10 journal preserves it. A failed transaction leaves the first effect pending and creates no child.
The first successful commit chooses the successor: competing or repeated
acknowledgements return that same durable request, even if their result differs.
Reopening the journal replays the chosen request without reevaluating its body.
Raw-result validation or computation failure creates no successor; cancellation,
deadline and terminal failure close the current chain. Completed chains compact
as one unit, while in-progress predecessors remain protected.

Schema 3 requires the original authority envelope, not only a continuation image.
Reopen the journal for normal recovery. A trusted embedding host may use
`Vm::restore_request` with the original `EffectRequest`; `Vm::restore(image)`
rejects schema 3 with `LSV1407` rather than synthesizing authority. Neither API
authenticates untrusted input: the embedding host owns snapshot provenance and
capability grants. A single request is not a backup of an already advanced chain.
Older schema-1/2 continuations retain their wire forms; legacy journals migrate
to the current schema, and old binaries refuse to open the newer journal.

The native debugger uses its existing one-presentation acknowledgement channel.
Acknowledging the first effect exposes the second with a new effect identity and
session revision; a changed stale acknowledgement cannot redirect it. No new
public projection of locals or private result-binding state is introduced.

## Durable Result Chains

```leselang
fn main() = bind(
  moved: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
  body: bind(
    focused: ui.focus(node_id: field(value: moved, name: "focused_node_id")),
    body: bind(
      restored: ui.focus(node_id: field(value: moved, name: "node_id")),
      body: eq(
        left: field(value: focused, name: "node_id"),
        right: field(value: moved, name: "focused_node_id"),
      ),
    ),
  ),
)
```

This performs three atomic operations and returns a boolean. A nested atomic
`bind` can read earlier results and scalar locals, then either return a pure
scalar or continue the chain. Result aliases stay read-only. `choose` may select
between chains of the same final type, with type/capability checks on both
branches and lazy execution of the selected branch. Schema 4 requires both
branches to suspend or both to be pure at each choice; mixed exits use
[schema 5](#conditional-exits) instead. Computation in host arguments and
loop components must still be pure. Dynamic groups, group-result capture inside these chains,
effectful loops, recursive calls and unprojected result returns remain rejected.

Continuation **schema 4** stores bounded lexical frames with typed result
projections. Each saved result contains its declared atomic operation type and
exactly the exported scalar fields, in canonical order. Raw host objects and
collections are not copied; no arbitrary credential or GUI-handle field is exposed.
Explicit scalar literals remain journaled locals, so host secret-handling rules
still apply. A large inventory
therefore contributes only its `revision` and `count`. Prior aliases preserve
the same projections. Restore checks field names, scalar types, duplicates,
scope/shadowing, residual HIR, capabilities and the complete 64 KiB image limit.
All saved scalar/result locals together are bounded by the 16-level scope limit;
the original 1024-node and source-call-depth limits still apply. The journal
graph has a separate hard ceiling of 64 steps, not a promise that every 64-call
nested source fits the syntax limits.

Every transition uses the same principal, capability set, target revision,
deadline and remaining fuel. Copying/restoring scalar projections costs fuel;
reopening with a larger startup budget does not replenish it. Projection access
does not expose arbitrary fields or gain new authority. Expected target revisions
remain fixed, so a later mutation may legitimately conflict after an earlier one.

Journal schema 9 adds a `dataflow` plan over the existing sequential graph. The
current successful raw result, updated plan, concrete successor request and its
frame commit atomically. Failed writes leave the old step pending. Recovery uses
the committed request, never reevaluates previous decisions, and validates each
adjacent pair's authority, budgets and inherited projections. Only the current
step can be leased. Competing workers replay the first committed transition;
progress reads use one database snapshot and reconcile obsolete local pending
steps. Cumulative output overflow stops the chain before admitting another effect.
The final atomic value or computed scalar/fault is committed with group completion.
Cancellation, deadline and terminal failures stop the whole chain; retention
preserves active prefixes and deletes completed chains as one unit.

Schema 4, like schema 3, requires the original authority envelope: reopen the
journal or use trusted-host `Vm::restore_request`, not image-only `restore`.
Journal 8 migrates to 9 without rewriting schema-1/2/3 continuations or existing
two-effect plans. New readers accept the older wire forms; old readers reject
schema 4 and journal 9. The native debugger advances one presentation and one
session revision at a time, without exposing private frames in its projections.

## Conditional Exits

```leselang
fn main() = bind(
  moved: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
  body: choose(
    when: eq(
      left: field(value: moved, name: "focused_node_id"),
      right: "runtime-home",
    ),
    then: true,
    otherwise: bind(
      focused: ui.focus(node_id: field(value: moved, name: "focused_node_id")),
      body: eq(
        left: field(value: focused, name: "node_id"),
        right: field(value: moved, name: "focused_node_id"),
      ),
    ),
  ),
)
```

A result-bound `choose` may now return a pure scalar on one branch and continue
through atomic captures on the other. Both branches must have the same scalar
type; raw result objects cannot be early-returned. Guards and host arguments are
pure. Pure local preparation, aliases and bounded scalar loops can surround an
exit, but effects inside operands, loop components or dynamic groups remain
rejected. Both branches receive type, scope and capability preflight; only the
selected branch executes, so a cold arithmetic failure or host action is not run.

Continuation **schema 5** marks residual bodies containing mixed exits, including
choices in later captures. It retains the schema-4 typed projection frame and
the same authority, fuel, deadline, scope and size limits. Once no mixed exit
remains, a successor may use schema 4. Schema-1/2/3/4 wire forms stay unchanged;
changing a schema-5 version tag to an older value is rejected. Image-only restore
is not allowed: use the journal or trusted-host `Vm::restore_request`.

Journal schema 10 distinguishes a final `Value::Scalar` from a successful raw
host result that requires a successor. A final scalar must match the residual
program's declared type and cannot have a child; an intermediate success must
have its committed successor. Early completion uses the existing transaction to
finish the current step and its group, without creating a phantom dispatch.
Recovery replays the first committed choice, never reevaluates its predicate,
and rejects wrong terminal types, cross-frame final-type changes, scalar-to-child
links, skipped mandatory captures and truncated chains.
Concurrent return/continue acknowledgements observe the same committed outcome.

Raw output is counted before scalar projection and before either continuation or
completion. A large final inventory cannot hide cumulative output overflow by
returning a single boolean or integer; overflow persists as `LSV2404`. This fence
also applies to schema-4 final scalar bodies. Cancellation, expired deadlines,
superseded leases and exhausted fuel do not become successful early returns.
Completed prefixes remain protected during execution and compact as one unit.

Journal 9 migrates to 10 without rewriting old images or dataflow plans. Older
readers reject schema 5 and journal 10. The native debugger completes immediately
on the early branch or shows just the chosen next presentation; acknowledgement
identity/session-revision checks remain unchanged. This adds no desktop scalar
inspector and does not expose private result frames in public UI projections.

## Computed Host Arguments

```leselang
fn main() = bind(
  node: concat(left: "runtime-", right: "a"),
  body: ui.navigate_focus(
    node_id: node,
    direction: choose(when: true, then: "next", otherwise: "last"),
  ),
)
```

All 70 current atomic host operations accept pure expressions and enclosing
scalar locals in their named arguments. The operation name stays fixed and
typed; no dynamic dispatch, source interpolation, or implicit conversion occurs.
The example needs a real exported node ID and the `ui.presentation` capability.

Arguments retain their existing string/`none` contracts. For example, child
counts and form maximum lengths still use bounded decimal **strings**, not
integer values; `none` is allowed only by nullable parameters. A nullable
parameter is not necessarily omittable: `ui.wait_form_field_placeholder.expected`
must still be present, even when it resolves to `none`. A `choose` inside an
argument must still have identical branch types, not a string/`none` union.
Runtime-list filters are control-free strings of at most 128 UTF-8 bytes or
`none`; source, computed values and the recovery decoder share this validator.

Preflight checks operation identity, required/unknown/duplicate parameters,
scalar types, direct literal argument domains, lexical scope, and the whole
program's authority.
Argument expressions are evaluated in signature order, independent of their
source order. All selected arguments must finish and pass the existing atomic
validators before an effect ID or journal entry is created. Invalid computed
identifiers, enum values, text lengths, or decimal bounds therefore produce a
typed fault with no partial host operation. Unselected expressions stay lazy;
their computed values are not evaluated for domain validation.

Argument materialization also charges string-copy fuel. Only the resolved
concrete request is persisted, with its principal, capability set, expected
revision, idempotency key, remaining fuel and original deadline. Retry/restart
uses that request; it does not rerun argument computation or persist a local
variable stack. Computed strings that resemble language source remain data.

## Computed Groups

```leselang
fn main() = bind(
  node: concat(left: "runtime-", right: "a"),
  body: seq(
    focus: ui.focus(node_id: node),
    verify: repeat(
      times: 2,
      body: ui.assert_visible(node_id: node),
    ),
  ),
)
```

`seq`, bounded `repeat`, and flat `all` accept computed atomic arguments. Outer
locals remain in scope across the group's parameter preparation. Unbound groups
do not retain those locals for host-result re-entry; a [group-result binding](#named-group-result-bindings)
explicitly saves the scalar environment for its pure body. Nested sequences and repetitions use the same flattening,
step naming and 64-effect limit as literal groups. Repeated expressions count
toward the 1024-node expanded computation limit, checked before cloning the
repeated body. The source-call depth limit remains 16.

Preparation runs synchronously in flattened declaration order, sharing one fuel
budget. Every selected member's parameters must finish and pass validation
before any group/effect identity is allocated or any execution row is journaled.
A bad parameter or arithmetic/fuel failure in a later member prevents the whole
group from starting. Each repeated member evaluates its own argument expressions
during preparation; an enclosing `bind` evaluates its bound value only once.
An outer `choose` still prepares only its selected group, while checking types
and capabilities in both branches before execution.

Only concrete atomic requests enter this existing journal graph; unbound computed
groups do not save a variable environment. Sequential dispatch
uses the remaining shared effect budget, and parallel groups preserve the
existing per-branch effect budgets after their shared preparation cost. Re-entry
and recovery never rerun parameter preparation, reset deadlines, or make a
blocked sequential successor eligible early. Their graph representation is
unchanged in the current schema-10 journal.

The Leserpent debugger can advance computed sequences through its existing
single-presentation acknowledgement channel. Flat `all` uses the Rust VM's
structured batch interface; this does not add multi-presentation batch support
to that debugger. Mixed `all`/`seq`, nested parallel execution, general
computation as group members, result-dependent parameters and dynamic repetition
counts remain unsupported. Use pure `bind`/`choose` inside an atomic argument,
not as a replacement for the atomic group member.

## Named Sequences

```leselang
fn main() = seq(
  reveal: ui.scroll_into_view(node_id: "runtime-runtime-a-refresh"),
  ready: ui.wait_enabled(node_id: "runtime-runtime-a-refresh"),
  activate: ui.activate(node_id: "runtime-runtime-a-refresh"),
)
```

`seq` executes one to 64 named effects in declaration order. A successor becomes
eligible only after its predecessor returns a valid successful result. Failed
effects, malformed results, cancellation, and deadline expiry stop the flow;
unexecuted successors are durably closed without being dispatched. A semantic
retry holds the current position and never releases its successor.

An unbound sequence's final value is `Value::Structured`, with named fields in declaration order.
It includes each completed effect's typed result, not a shell exit-status integer.
A one-step sequence still returns a one-field structured value. Failures return
the terminal failure rather than a partial successful value.

## Bounded Repetition

```leselang
fn main() = repeat(
  times: 3,
  body: seq(
    focus: ui.focus(node_id: "runtime-runtime-a-refresh"),
    verify: ui.assert_focused(node_id: "runtime-runtime-a-refresh"),
  ),
)
```

`times` must be an integer literal from 1 through 64. Strings, zero, negatives,
fractions, leading zeroes, and integer overflow are rejected. `times` and `body`
are the only arguments, each appears exactly once, and their order is immaterial.
Repetition is sequential, not a parallel batch or implicit polling interval.

`seq` and `repeat` may nest. HIR lowering flattens them to at most 64 atomic
effects before execution; an oversized product fails before expansion or dispatch.
Repetition names its steps `iteration_1`, `iteration_2`, and so on. Flattened names
join enclosing labels with `__`, so the example produces `iteration_1__focus`,
`iteration_1__verify`, and subsequent iterations. Expanded names must be unique
and at most 64 bytes; collisions or oversized names are errors, not silent renames.

The syntax formatter preserves `repeat`. The HIR canonical exporter emits its
equivalent flat `seq`; parsing that export reconstructs exactly the same HIR.

## Execution Safety

These rules govern host-effect groups. Pure loops do not allocate effect
identities or journal their individual scalar transitions.

- All required capabilities are checked before the first effect is created.
- The complete bounded graph is written atomically to the existing effect journal.
- Each iteration has a distinct effect/command identity. Redelivery of an iteration
  preserves its original idempotency key rather than creating a new iteration.
- Journal schema 7 stores execution order separately and verifies it against the
  serialized plan. Old graphs migrate as parallel; `all` keeps its prior behavior.
- Only the current step can be leased, even across competing VM connections. A
  caller cannot directly resume a future step to bypass the dependency fence.
- The flow shares one fuel allowance and, when timed, one absolute deadline.
  Re-entry does not reset either budget. Total retained output remains bounded.
- Completed graphs are compacted as one logical unit. In-progress graphs remain
  protected, including their already completed predecessors.

Recover a sequential program by reopening its journal. Individual continuation
images describe atomic effects; exporting one image does not export the full
control-flow graph. `pending_continuations` includes blocked future steps for
inspection, not permission to execute them. External workers must claim leases;
frontend-local hosts may apply only the request returned by `start`/re-entry.

Expected target revisions remain explicit host inputs and are not silently
advanced by `seq` or `repeat`. Repeating a mutation against an unchanged expected
revision can therefore conflict after the first mutation; the correct response
is failure, not automatic authority escalation or a hidden refresh. Result-based
revision binding is a separate future language feature.

The Leserpent debugger exposes one pending presentation operation at a time.
GUI acknowledgement advances to the next effect and increments the debugger
session revision. This session revision is independent of a command's target
revision. Cancellation requires the current session revision and current effect
identity and closes all remaining steps.

## Remaining Gaps

Agent-driven control-flow expressiveness is not complete yet. Pure scalar
bindings, conditional selection, bounded pure loops, pure arithmetic/parse
recovery, whole-group computed host arguments and captured atomic results with
durable scalar/projection locals are implemented.
Bounded multi-step result chains and conditional early exits now have transactional
admission/completion and replay. Named group-result binding now supports pure
scalar bodies with whole-group journal recovery. The next work is group-driven
host effects, additional typed projections, collection iteration,
bounded effectful loops with exit/skip semantics,
reusable functions, and explicit host-effect recovery/cleanup. These are semantic requirements, not prescribed
keywords or a Bash compatibility checklist. Their syntax must satisfy the
[agent-first design rules](leselang-embedding.md#agent-first-syntax); none of
these unimplemented constructs should be generated as if supported.
Combining sequential groups with `all` is also currently rejected, not silently
parallelized. Existing host-managed retries remain separate from pure calculation
recovery. The status tensor tracks this as developing work after the 2.0 scope.
