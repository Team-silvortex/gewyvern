# Leselang Control Flow

This is the implemented sequential-control contract, not a claim of Bash language
parity. It extends the [language contract](leselang-language.md) without shell
execution, hidden coercions, or source-level async/await.

The post-2.0 control-flow contract is evolving at version `0.5.0`; it does not
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

This returns the integer `10`. A pure `bind` accepts exactly one named scalar value
and `body`; the value is evaluated once, and the name is visible only in the
body. A binding may use earlier enclosing bindings, but not itself, a later
binding, or a shadowed active name. Names are ASCII identifiers of at most
64 bytes; `body` and literal/grammar keywords cannot be bound. Bindings are
immutable. Argument order does not change lexical scope or execution order.

`choose` requires `when`, `then`, and `otherwise`. The condition must be boolean,
and both branches must have the same HIR result type. Only the selected branch
executes. Both branches are still type-checked, and all declared host operations
contribute to the capability preflight, including operations in the unselected
branch. `Structured` is currently a coarse aggregate type, not a field-by-field
record schema. Atomic host-result bindings support only the explicit scalar
projections described in [result bindings](#result-bindings).

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

Computation can run before the first host suspension, and in a pure scalar body
after one captured atomic result. Local bindings can fill host-operation
arguments, including inside bounded groups, but cannot provide `repeat.times`.
General computation must wrap `seq`, `repeat`, or `all`, or occur inside an
atomic member's arguments; it cannot replace a group step with `bind`/`choose`.
An unexecuted mixed group still cannot contain unsupported nested computation.
Result-driven host successors and general durable control frames remain unsupported.

Each evaluated expression costs one unit of the same VM fuel used by subsequent
effects. String materialization, copying and operations additionally charge one
unit per started 64-byte block of each involved string. HIR validation bounds
the combined expanded computation/embedded-effect graph to 1024 nodes and retains
the 16-level source-call depth fence. Source and literal limits apply before work.
Checked arithmetic faults use `LSV1401`, invalid computational state uses
`LSV1402`, and oversized computed strings use `LSV1403`; fuel exhaustion remains
`LSV1001`. Rejected computed host arguments use `LSV1404`, including the original
host-validator code but not the rejected value. Invalid saved result bindings
use `LSV1405`. Computation diagnostics use `LSH1401` through `LSH1409` with spans.

Only the selected, fully determined host graph is journaled, with the remaining
fuel and original deadline. Recovery consumes that graph rather than evaluating
the source condition again. A program that never suspends has no durable
continuation. A result-binding program saves its scalar environment and pure
body alongside the selected atomic request.

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

A result-binding `bind` takes one atomic host call, literal or computed, and a
pure body returning a scalar. The body may contain lexical bindings, arithmetic,
short-circuit logic and lazy `choose`. A result can be aliased in the body but
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
Schema 2 without a binding and schema 1 with one are rejected. SQLite journal
schema 7 is unchanged, and old binaries must reject rather than discard this
new continuation form.

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
Result-bound bodies cannot yet start another host effect, capture a group,
replace a group member, or create a second suspension. Those are rejected during
preflight, even in a cold branch. Use an outer `choose` to select a result-binding
program, rather than hiding effects in operands or host arguments.

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
locals remain in scope across the group's parameter preparation, not across
host-result re-entry. Nested sequences and repetitions use the same flattening,
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

Only concrete atomic requests enter this existing journal graph; computed
groups do not save a variable environment. Sequential dispatch
uses the remaining shared effect budget, and parallel groups preserve the
existing per-branch effect budgets after their shared preparation cost. Re-entry
and recovery never rerun parameter preparation, reset deadlines, or make a
blocked sequential successor eligible early. Journal schema 7 is unchanged.

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

The final value is `Value::Structured`, with named fields in declaration order.
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
bindings, conditional selection, whole-group computed host arguments and a
single captured atomic result with durable scalar locals are implemented. The
next work is result-driven host successors across multiple suspensions, group
result binding, additional typed projections, collection iteration,
bounded data-dependent loops with exit/skip semantics,
reusable functions, and explicit error recovery/cleanup. These are semantic requirements, not prescribed
keywords or a Bash compatibility checklist. Their syntax must satisfy the
[agent-first design rules](leselang-embedding.md#agent-first-syntax); none of
these unimplemented constructs should be generated as if supported.
Combining sequential groups with `all` is also currently rejected, not silently
parallelized. Existing host-managed retries remain separate from language-level
recovery. The status tensor tracks this as developing work after the 2.0 scope.
