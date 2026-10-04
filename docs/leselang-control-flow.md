# Leselang Control Flow

This is the implemented sequential-control contract, not a claim of Bash language
parity. It extends the [language contract](leselang-language.md) without shell
execution, hidden coercions, or source-level async/await.

The post-2.0 control-flow contract is evolving at version `0.23.0`; it does not
reclassify the older stable atomic-effect contract as a complete language.

These are language-core mechanisms, not Leserpent-only GUI macros. Their
current effect vocabulary and VM integration remain product-bound; the
[embedding architecture](leselang-embedding.md) defines how they will serve
independent hosts without claiming that extraction is complete.

## Calculation And Decisions

Basic computation is executable, not source-text substitution. Scalars are
unsigned 64-bit integers, booleans, strings bounded to 4096 UTF-8 bytes,
`none`, and distinct [optional strings](#optional-gui-projections). The same
closed data envelope also carries [bounded string lists](#bounded-string-collections).
A pure program finishes as `Step::Done(Value::Scalar { .. })` without
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
| `len` | `value`: string or string_list | integer Unicode scalar-value count for text, entry count for lists |
| `to_string` | `value`: integer, boolean or string | string; decimal integer, lowercase boolean, or unchanged text |
| `parse_integer` | `value`: string | integer; non-empty ASCII decimal within `u64`, leading zeros accepted |
| `parse_boolean` | `value`: string | boolean; exactly `"true"` or `"false"` |
| `contains`, `starts_with`, `ends_with` | `left`: text string, `right`: pattern string | boolean; exact, case-sensitive matching, empty pattern always matches |
| `char_at` | `left`: string, `right`: integer index | optional string; zero-based Unicode scalar-value access, absent when out of range |

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
diagnostics use `LSH1401` through `LSH1413` with spans.

Only the selected, fully determined host graph is journaled, with the remaining
fuel and original deadline. Recovery consumes that graph rather than evaluating
the source condition again. A program that never suspends has no durable
continuation. A result-binding program saves its scalar environment and bounded
body alongside the selected atomic request.

## Reusable Pure Functions

```leselang
fn next_count(count: integer) = to_string(value: add(left: count, right: 1))

fn main() = bind(
  counted: ui.assert_child_count(node_id: "rows", count: "7"),
  body: ui.set_form_value(
    node_id: "form",
    field: "replicas",
    value: next_count(count: field(value: counted, name: "count")),
  ),
)
```

This requests the form text `"8"` after the host confirms seven children.
The pure helper declares scalar parameter types (`integer`, `boolean`, `string`,
`none`, `optional_string` or `string_list`) and infers its scalar return type from its body. No implicit conversion,
default parameter, positional argument, function value or closure is introduced.
A multi-function program requires exactly one parameterless `main`; it may appear
before or after helpers. Forward references are allowed. The legacy single-function
entry and its serialized syntax shape remain unchanged.

There are at most **32 declarations**, including the entry, and **8 parameters**
per helper. Names use the existing bounded ASCII identifier rules; helper names
cannot replace builtins, and parameters cannot duplicate or shadow active locals
inside their own definition. A helper sees only its parameters and definition-local
bindings, never a caller's locals or caller-owned raw host result. Pass an explicit typed `field`
projection instead. All definitions are type-checked, including unused helpers
and cold branches. Direct, mutual and cold-branch recursion are rejected, as are
calls from a helper to `main`.

Pure helper bodies and all helper arguments must be pure scalar computations.
Pure helpers may compose other pure helpers, pure loops, conversions, recovery
and conditional calculations, but cannot suspend or return a result object.
[Effectful helpers](#reusable-effectful-functions) use the same declaration and
argument rules in explicit effect-flow positions. Every call must provide
exactly the declared named parameters, each once, with matching scalar types.
Arguments run once in **parameter declaration order**, regardless of their source
order, before the body. Unused parameters still evaluate. An argument fault prevents
body evaluation; an unselected enclosing branch evaluates neither call nor arguments.

Lowering uses hygienic expansion into existing HIR `bind` nodes, not source-text
substitution. Fresh parameter/local names prevent caller capture and preserve
separate lexical scopes across repeated calls. The complete expanded graph retains
the **1024-node** and **16-level** bounds; helper bodies are bounded before cloning,
so a small source DAG cannot trigger unbounded expansion. Expanded helpers and the
entry must also fit the existing 256 KiB canonical source limit; literal repetition
cannot bypass that bound through short calls. Inlined calculations and
string copies consume the same shared fuel, with no per-call refill. Helpers in
group arguments preserve whole-group all-or-nothing preparation and host validators.

Syntax formatting preserves declaration and parameter order. HIR canonical export
emits the expanded single-entry program; recompiling it reproduces the same HIR.
Saved residual bodies contain only existing computation nodes, not helper tables
or source references. Continuation schemas 1-11 and journal schema 10 are
unchanged; restart needs no source helper definitions. Completion and successor
replay retain their original authority, fuel and deadline. The native debugger
uses the same correlated presentation acknowledgement path, not a new helper API.

Declaration/signature faults use `LSH1501`, helper recursion uses `LSH1502`, invalid
helper returns/composition use `LSH1503`, and invalid named/typed call arguments use `LSH1504`.
Expansion limits remain `LSH1405`. Syntax count limits use `LSE1201` and `LSE1202`;
malformed parameter separators/types use `LSE1203` and `LSE1204`. Diagnostics carry
source spans. Neither pure nor effectful helpers introduce function frames,
recursive calls, imports, higher-order values or a general shell.

## Reusable Effectful Functions

```leselang
fn host_ready(node: string, skip: boolean) = choose(
  when: skip,
  then: false,
  otherwise: bind(
    result: ui.assert_text(node_id: node, expected: "ready"),
    body: starts_with(left: field(value: result, name: "expected"), right: "ready"),
  ),
)

fn main() = bind(
  ok: host_ready(node: "status", skip: false),
  body: bind(
    written: ui.set_form_value(node_id: "form", field: "ready", value: to_string(value: ok)),
    body: ok,
  ),
)
```

The helper suspends for the correlated `status` assertion, returns `true`, then
the caller writes `"true"` to the form and returns that boolean after its receipt.
With `skip: true`, the helper returns `false` without an assertion; the caller
still writes `"false"`. **Every normal return** joins the same caller continuation.
A host failure, rejected receipt, deadline or cancellation does not count as a
normal return and cannot run the caller's remaining operations.

An effectful helper can be a whole-flow/tail call or the direct value of an
explicit `bind`. [Selected data-returning functions](#selected-data-returning-functions)
also admit pure choices of helper calls and data fallbacks as a binding value.
Its inferred return may be an atomic host result, a named flat
group, or bounded typed data after result captures. Atomic returns use existing
`field` projections; named group returns use existing `member` then `field`.
Returning raw result objects from a captured local remains unsupported. Multiple
captures, conditional scalar exits and group-owned chains retain their existing
shape limits; a helper is not permission to combine otherwise unsupported shapes.

Parameters remain **pure typed data**, never result objects or effectful arguments.
They run once in **parameter declaration order** before the function's first effect,
even if unused. Callers pass explicit typed projections. Definition-local result,
group, accumulator and item names are hygienically renamed, so repeated/nested
calls cannot capture or shadow caller locals, including unused quoted `fold`
item declarations. All definitions and cold returns
are type-checked; called cold effects contribute to capability preflight, while
unused helpers do not grant or require unused authority.

The compiler connects the caller to the expanded function's normal return sites
using existing HIR `bind`/`choose` nodes. It reserves **every cold return before
cloning** the caller continuation: the complete graph still fits **1024 nodes**,
**16 levels** and **256 KiB canonical source**. Recursive expansion cannot bypass
these limits. Existing host graph slots, result-frame growth, raw-output limits,
shared fuel and original authority/deadline fences apply without per-call refills.
SQL failures roll back both predecessor completion and successor admission.

**No hidden call stack** or function table is saved. Canonical HIR export erases
declarations and recompiles identically; journal restart needs no source/helper
definitions. **Continuation schemas 1-11 and journal schema 10 remain unchanged**,
as do projection vocabularies v1-v5. Old single-entry wire records retain their
shape. The Rust native debugger uses its existing revisioned presentation
acknowledgements, including wrong-node rejection and first-commit replay, not a
new GUI or function-call API.

Effectful calls are not operands of arithmetic, conditions, host arguments,
helper arguments, `recover`, pure `loop` or `fold`. A helper can be a primitive
`seq`/`all`/`repeat` member only under the
[prepared atomic member contract](#prepared-atomic-members); multi-step and
result-capturing helpers cannot replace an atomic member. Arbitrary nested
effectful `bind` values do not gain an implicit call ABI. Recursion, closures,
dynamic/higher-order calls and imports remain rejected. **This does not add
effectful loops** or host-error recovery/cleanup; those need separate lifecycle
and checkpoint contracts.

## Prepared Atomic Members

```leselang
fn focus(node: string, alternate: boolean) = bind(
  target: concat(left: "node-", right: node),
  body: choose(
    when: alternate,
    then: ui.focus(node_id: "alternate"),
    otherwise: ui.focus(node_id: target),
  ),
)

fn main() = seq(
  first: focus(node: "a", alternate: false),
  again: repeat(times: 2, body: focus(node: "b", alternate: true)),
)
```

This prepares the requests `node-a`, `alternate`, `alternate` before the first
dispatch. The final member names remain `first`, `again__iteration_1` and
`again__iteration_2`. Reusable helpers, their expanded pure `bind` preparation and
lazy `choose` selection can now be members of `seq`, flat `all` and literal-bound
`repeat`. **Every path produces exactly one atomic operation**, and every cold
path must have the **same `HostOperation` signature** and result type. Different
arguments are allowed; data-only exits, captured results, nested groups and
multi-step helpers are not. Existing primitive nested `seq`/`repeat` flattening
still works, but a group-returning helper is not an implicit flattened member.

The complete group is prepared synchronously in flattened declaration order.
Signature arguments run once in parameter declaration order, including unused
arguments; local preparation and predicates are pure. Each repeated copy pays
its own preparation cost. Locals are scoped independently and hygienic helper
names cannot leak into another member or capture caller bindings. Both selected
and cold paths are type/capability-checked; only selected preparations execute.
Host arguments still pass their original domain validators.

**A later preparation failure admits no member**, consumes no effect/group
identity and journals no execution row. All computations share fuel without
refills. The expanded **1024-node**, **16-level**, **64-effect** and canonical
source bounds remain in force; repeat counts constructor entries and helper
preparation before cloning, not just the final host call. Signature inspection
uses bounded iterative traversal, not an unchecked recursive walk of public HIR.

Only **resolved atomic requests** enter the existing journal graph, never helper
definitions, preparation expressions or branch decisions. Re-entry does not
rerun preparation, refund fuel or reset authority, revision or deadline. Native
sequences use the existing correlated single-presentation acknowledgements;
parallel groups keep the Rust VM's **all-success barrier**, including out-of-order
receipts and named `member`/`field` projections. This does not add a native GUI
batch: parallel debugger starts still fail preflight before session journals.

SQL admission failures roll back execution rows and dispatches. Durable identity
reservations are deliberately **not reused**, even when execution admission
fails. First-commit replay, cancellation and deadline fences are unchanged.
**Continuation schemas 1-11 and journal schema 10 remain unchanged**, with no new
saved function frames or projection vocabulary. Literal legacy groups keep their
wire shape. This is bounded precomputation, not result-dependent members,
effectful iteration, mixed `all`/`seq`, dynamic repetition or hidden concurrency.

## Selected Named Groups

```leselang
fn rows(alternate: boolean) = choose(
  when: alternate,
  then: seq(
    first: ui.focus(node_id: "a"),
    second: ui.assert_text(node_id: "a", expected: "ready"),
  ),
  otherwise: seq(
    first: ui.focus(node_id: "b"),
    second: ui.assert_text(node_id: "b", expected: concat(left: "re", right: "ady")),
  ),
)

fn main() = bind(
  group: rows(alternate: false),
  body: bind(
    written: ui.set_form_value(
      node_id: "form",
      field: "selected",
      value: concat(
        left: field(value: member(value: group, name: "first"), name: "node_id"),
        right: field(value: member(value: group, name: "second"), name: "expected"),
      ),
    ),
    body: field(value: written, name: "value"),
  ),
)
```

This selects the `b` sequence, waits for both correlated receipts, writes
`"bready"`, then returns that confirmed value. A direct `bind` of an inline
`choose` works too. Pure preparation, nested choices and typed helper parameters
can precede a group. Named exports require the **same group mode**, **same ordered
member names** and **same `HostOperation` signatures** on every cold return path.
The export is a **closed signature, not a union**: missing/reordered members,
different operations or mixed `seq`/`all` modes cannot provide these projections.
Existing aliases preserve this closed member set; dynamic member names and raw
captured group returns remain unsupported.

The guard is pure and evaluated once; only the **selected group** prepares its
arguments, synchronously before any dispatch. Unselected preparation faults stay
cold, but all branches retain type, literal-domain, capability and expansion
preflight. A selected guard/preparation/domain/fuel failure allocates no identity
and journals no execution row. Bounded iterative signature inspection follows
structurally validated HIR, with existing node/depth/source and 64-effect limits.
Both literal and prepared atomic groups obey the same signature contract.

Journals contain **resolved requests only**, never the selection guard or helper
definitions. **Restart does not reselect** or rerun preparation. Named projections
feed existing pure bodies, atomic tails or bounded captured/conditional chains;
parallel prefixes retain their **all-success barrier** and out-of-order receipts.
Original authority/revision/deadline and shared fuel remain attached to the chosen
graph. Receipt-plus-successor SQL failures roll back and permit retry; first-commit
replay, cancellation and output/frame limits are unchanged.

**Continuation schemas 1-11 and journal schema 10 remain unchanged**, as do closed
projection vocabularies v1-v5. Native sequential debugger acknowledgements preserve
wrong-node rejection, revisions and replay without exposing private frames.
**Native parallel starts still fail preflight before session journals**; this is
not GUI batch support. Result-dependent group capture inside an atomic chain,
group-returning helpers as atomic members, effectful loops and host-error cleanup
remain unsupported. This adds static selection/composition, not dynamic topology.

## Prepared Atomic Result Bindings

```leselang
fn main() = bind(
  seed: ui.assert_text(node_id: "status", expected: "ready"),
  body: bind(
    selected: bind(
      target: concat(left: "node-", right: field(value: seed, name: "expected")),
      body: choose(
        when: starts_with(left: target, right: "node-ready"),
        then: ui.focus(node_id: target),
        otherwise: ui.focus(node_id: "fallback"),
      ),
    ),
    body: choose(
      when: eq(left: field(value: selected, name: "node_id"), right: "fallback"),
      then: false,
      otherwise: bind(
        written: ui.set_form_value(
          node_id: "form",
          field: "target",
          value: field(value: selected, name: "node_id"),
        ),
        body: true,
      ),
    ),
  ),
)
```

After the confirmed `status` receipt, this prepares/selects one focus request,
captures its correlated result and either exits with `false` or writes the
confirmed target to the form. `bind` now accepts pure preparation/selection as
its atomic value, including a choice between atomic helper calls. **Every cold
path produces exactly one atomic operation**, with the **same `HostOperation`
signature** and result type. This uses the same bounded iterative classifier as
prepared group members, not a second call ABI. A value containing result captures,
multiple effects, a data-only exit or an implicit group cannot pass this rule.
General effectful function returns still use explicit normal-return splicing.

Preparation and guards are pure; selected helper arguments run once in declaration
order. Unselected helper arguments stay lazy, including unused arguments, while
all cold code retains type/domain/capability and expansion checks. Prefix locals
are scoped to preparation and **do not leak into the continuation**. Only enclosing
locals/results/groups and the caller body are captured. Domain or fuel failures
before initial admission allocate no identity and journal no execution row.

Prepared captures compose with existing atomic result chains, scalar early exits
and group-owned successors. **One capture reserves one atomic slot**, regardless
of cold alternatives; a group prefix plus the longest successor path still fits
in 64 steps. Parallel prefixes keep the all-success barrier and out-of-order
receipts. Group recovery checks the captured operation against the same uniform
signature, including its position in the saved chain.

The **selected request is resolved before suspension**. Restart or first-commit
replay never reruns its preparation or reselects that committed request. Future
captures remain saved HIR and are evaluated only after a validated predecessor
receipt, under original authority/revision/deadline and shared fuel. SQL failures
roll back predecessor completion and successor admission together; a retry may
recalculate an **uncommitted** pure selection from the same immutable inputs.
Simultaneous workers commit one successor, never duplicate host work.

Saved projections stay closed: **legacy fields are not synthesized**, and even an
unselected prepared path cannot read a field missing from a v1-v5 frame. Cancellation,
wrong receipts, deadlines, raw-output and frame limits retain existing fences.
**Continuation schemas 1-11 and journal schema 10 remain unchanged**; literal
legacy programs keep their wire shape. Native debugger receipts preserve revisions,
wrong-node rejection, early completion and first-commit replay without exposing
private frames. This is not a new GUI endpoint or a parallel presentation batch.
Pure operands, helper arguments, `loop`/`fold`/`recover` and result-capturing group
members still cannot hide effects. **No effectful loops or host-error cleanup**
are introduced by this atomic composition rule.

## Selected Data-Returning Functions

```leselang
fn single() = bind(
  result: ui.assert_text(node_id: "status", expected: "ready"),
  body: field(value: result, name: "expected"),
)

fn gathered() = bind(
  group: seq(
    first: ui.assert_text(node_id: "left", expected: "re"),
    second: ui.assert_text(node_id: "right", expected: "ady"),
  ),
  body: concat(
    left: field(value: member(value: group, name: "first"), name: "expected"),
    right: field(value: member(value: group, name: "second"), name: "expected"),
  ),
)

fn main() = bind(
  answer: choose(when: false, then: single(), otherwise: gathered()),
  body: bind(
    written: ui.set_form_value(node_id: "form", field: "answer", value: answer),
    body: eq(left: field(value: written, name: "value"), right: "ready"),
  ),
)
```

This selects the sequential helper, waits for both correlated receipts, then
writes its returned `"ready"` data to the caller's form. A selected helper may
own a single capture, a bounded chain or a supported sequential/parallel group;
different helper topologies can join because **only returned typed data** crosses
the call boundary. A parallel helper still observes the **all-success barrier**.

The binding value's non-pure source paths must be **`choose` or direct helper
calls**. Guards are pure; nested choices and **pure data fallbacks** are allowed.
Every arm has the same bounded data type: integer, boolean, string, none,
optional string or string list. Raw result/group selection remains under its
existing uniform operation or closed named-group contract. This does not admit
arbitrary inline result-capturing `bind` values, impure arguments/operands,
effectful loop/fold/recovery positions or multi-step atomic group members.

**Only the selected call's arguments run**, once in parameter declaration order,
including unused parameters. Cold calls still contribute capability requirements;
all definitions and cold returns are type/domain checked. Helper locals are
hygienic; a pure fallback's locals do not capture the caller continuation.
The compiler reserves **every cold return before cloning** the caller under the
same 1024-node, 16-level and 256-KiB canonical-source limits.

Normal-return splicing uses existing HIR nodes, with **no hidden call stack**.
The current selection is resolved before suspension; no helper table is saved,
and committed requests are not reselected on journal restart. Future selectors
in the caller's remaining HIR still run at their own boundaries. Shared fuel,
original authority/deadline and closed projection
fields survive each receipt. SQL failures roll back predecessor completion and
caller admission together; uncommitted pure work may be recalculated on retry.
Simultaneous workers commit one successor; first-commit replay never reruns it.
Host failures, rejected receipts, cancellation and expiry do not join the caller.
**Continuation schemas 1-11 and journal schema 10 remain unchanged**; projection
vocabularies v1-v5 and existing prepared-capture/group wire shapes stay intact.

The native debugger proves selected single/sequential helper-to-form paths with
revisioned, correlated acknowledgements and wrong-node rejection. A selected
parallel helper still fails preflight before creating a session journal; this is
not native parallel GUI support or effectful loops/host-error cleanup.

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

## Bounded Text Inspection

```leselang
fn ready(text: string) = and(
  left: starts_with(left: text, right: "ready"),
  right: and(
    left: contains(left: text, right: "ad"),
    right: ends_with(left: text, right: "!"),
  ),
)

fn main() = bind(
  status: ui.assert_text(node_id: "status", expected: "ready!"),
  body: ui.set_form_value(
    node_id: "form",
    field: "prefix",
    value: choose(
      when: ready(text: field(value: status, name: "expected")),
      then: value_or(
        left: char_at(left: field(value: status, name: "expected"), right: 0),
        right: "default",
      ),
      otherwise: "default",
    ),
  ),
)
```

After the host confirms `"ready!"`, this requests form text `"r"`. Use real
host-exported node/field IDs and `ui.presentation`. These operators inspect
provided strings or acknowledged projections, not live GUI objects.

`contains`, `starts_with` and `ends_with` take exactly two named string inputs:
`left` is the text, `right` is the pattern. Matching is **exact and case-sensitive**,
with no regular expressions, locale rules, case folding or Unicode normalization.
An empty pattern returns `true` for all three, including empty text; a non-empty
pattern cannot match empty text. Matching is lossless UTF-8 text inspection.
For example, precomposed U+00E9 and the U+0065/U+0301 sequence are different strings.

`char_at(left: text, right: index)` takes a string and an unsigned integer.
Indices are **zero-based Unicode scalar-value** positions, consistent with `len`,
not UTF-8 byte offsets or grapheme clusters. A found scalar becomes a present
`optional_string` containing its one-to-four-byte UTF-8 encoding. A combining
mark after a base letter is a separate scalar at index 1. Empty text, an index
equal to or beyond `len`, and `u64::MAX` return an absent optional string, without
wrapping or truncating the index on narrower platforms. No successful `char_at`
returns a present-empty string. Use `has_value` or lazy `value_or` explicitly;
there is no implicit unwrap or conversion into a host string parameter.

Both operands are pure, type-checked even in cold branches, and evaluated once
in **left-to-right** order regardless of their named source order. These four
operators are eager, even for empty patterns or empty input: an error in the
right operand still propagates. Enclosing `choose`, `and`, `or` and `value_or`
retain their existing laziness. An absent character is a successful result, so
`recover` does not replace it. Existing arithmetic/parse faults may be recovered;
fuel, size and host-validation failures cannot.

Input copies and scans consume the **shared fuel**, charging each input's complete
UTF-8 byte length in the existing started 64-byte blocks, even when a match or
character occurs early. A present character also pays one materialization block;
absence has no text materialization cost. Indices cannot turn a bounded scan
into index-proportional allocation or iteration. Every input still fits the
4096-byte scalar bound before inspection; returning a boolean or short character
does not bypass it. Pure helpers and loops reuse these budgets without refills.
There is no collection type or effectful loop in this slice.

Text-derived host arguments retain their **original host validators**, including
control-character rejection and narrower form-value limits. Whole-group argument
preparation remains all-or-nothing. Sequential/parallel result groups keep their
barrier and reserved-successor rules; inspection cannot dispatch a successor early.

These operators use the existing closed binary HIR node. Old operator wire bytes,
projection versions **v1-v5**, continuation schemas **1 through 11** and **journal
schema 10** are unchanged; no database migration is needed. Unknown operators
and wrong-typed saved operands are rejected. Saved bodies and optional character
locals resume without source/helper tables under the original authority, deadline,
cancellation and remaining fuel. Raw receipt validation, the 64 KiB image/plan
bound, transactional successor/terminal writes, rollback and first-commit replay
remain in force. The Rust native debugger proves text predicates driving a form
prefix through correlated acknowledgements, without exposing private frames;
this is not a new desktop scalar inspector or actual desktop-control test.

## Bounded String Collections

`string_list` is an immutable, ordered list of strings, not a generic container
or a map. It can be a local, a pure helper parameter/result, a loop accumulator,
or a durable result-chain terminal. This example filters confirmed text and
hands the result to the existing form operation:

```leselang
fn selected(parts: string_list) = fold(
  output: strings(),
  items: parts,
  item: "part",
  next: choose(
    when: starts_with(left: part, right: "ready-"),
    then: append(left: output, right: part),
    otherwise: output
  ),
  limit: 64
)

fn main() = bind(
  status: ui.assert_text(node_id: "status", expected: "ready-a,skip,ready-b"),
  body: bind(
    values: selected(parts: split(
      left: field(value: status, name: "expected"), right: ","
    )),
    body: bind(
      written: ui.set_form_value(
        node_id: "form", field: "selected", value: join(left: values, right: ";")
      ),
      body: values
    )
  )
)
```

`strings()` constructs an empty list; `strings(first: "b", second: "a")`
constructs `["b", "a"]`. Its bounded, unique local-style labels are positional
source labels, not stored keys. Entries are evaluated once in **source order**,
not alphabetical label order. Canonical source renames labels to `item0`,
`item1`, etc., preserving order. All entries must be pure strings, including
cold branches; there are no nested lists, implicit conversions or optional items.

Every list has at most **64 entries** and at most **4096 bytes** of combined
UTF-8 text, including empty entries in the count. Literal lists are checked
during lowering; computed construction, splitting and appending enforce the same
limits before growth. Overflow faults with `LSV1403`, not truncation.

| Operation | Input | Result and Semantics |
| --- | --- | --- |
| `split(left:, right:)` | string, string delimiter | Exact, case-sensitive non-overlapping split; leading, trailing and consecutive delimiters preserve empty entries. An empty delimiter splits at Unicode scalar boundaries, including empty first/last entries. Splitting empty text with an empty delimiter yields two empty entries. |
| `append(left:, right:)` | string_list, string | A new ordered list, leaving the original value unchanged. |
| `join(left:, right:)` | string_list, string delimiter | String with delimiters only between entries; an empty list produces empty text. The 4096-byte output bound is checked before allocation. |
| `item_at(left:, right:)` | string_list, integer | Zero-based `optional_string`; present-empty stays present. Out-of-range and `u64::MAX` indices are absent, never truncated or allocated proportionally to the index. |
| `len(value:)` | string_list | Number of entries; existing string `len` still counts Unicode scalars. |
| `eq` / `ne` | two string_lists | Exact order-sensitive equality; labels are not part of the value. |

`fold(output: initial, items: list, item: "part", next: expression, limit: n)`
returns the final accumulator. `output` may be any bounded data type supported
by the scalar envelope, including `optional_string` or `string_list`. `next`
must be pure and preserve the exact accumulator type. State/item names must be
distinct valid locals and cannot shadow active bindings. They exist only in
`next`, not in `items` or `initial`, and never escape into the enclosing scope.
Pure helpers use hygienically renamed accumulator/item locals.

Evaluation prepares `items` first, then `initial`, exactly once, regardless of
written named-argument order. The integer-literal `limit` is from **0 through
64**. If the list exceeds it, `LSV1406` is raised **before the first iteration**;
an empty list with `limit: 0` returns `initial` without evaluating `next`.
`next` is still type-checked for empty lists. The loop runs synchronously under
shared fuel without HIR expansion or mid-loop suspension, in list order.
`choose` supplies filtering; nested pure folds supply nested aggregation.
This is not effectful iteration, a host retry mechanism or a hidden GUI batch.

List copying/scanning charges one shared fuel unit per entry plus one per
started 64-byte block of each entry. Construction and list/string outputs also
charge materialization; each iteration charges its item before entering `next`.
Existing expression-node charges apply as well. Empty entries therefore still
cost fuel. Resource/iteration limits and fuel exhaustion are not caught by
`recover`; arithmetic/parse recovery retains its closed error set and refunds
no fuel. On every normal/fault exit, both fold locals are removed.

The existing scalar wire envelope adds `{"kind":"string_list","value":[...]}`.
The array is required, even when empty; missing/null payloads, non-string entries,
unknown fields, oversized arrays and oversized combined text are rejected.
Public value validation also checks in-memory lists before accepting them;
each entry counts toward its output item accounting (an empty list counts as one).
Deserialization does not reserve from an untrusted length hint or collect an
unbounded payload. Canonical literal construction retains its entry-node/depth
budget. The new `strings` and `fold` HIR nodes are closed, as are binary operators.
Unknown constructs reject instead of falling back to a permissive interpreter.

List locals and residual fold bodies survive re-entry without source or helper
tables. Existing continuation schemas **1 through 11**, journal schema **10**
and projection vocabularies **v1-v5** remain unchanged; no saved host field is
added or synthesized. This is an additive HIR/value vocabulary, not a promise
that old runtimes execute new operators. The **64 KiB** image/plan bound,
original authority, revisions, deadlines, cancellation, raw output budgets,
transactional rollback and first-commit replay continue to apply. Sequential
and flat `all` group results may feed a fold only after every prefix succeeds.

Lists are not accepted directly by existing host arguments. Callers must join,
index/default or otherwise compute the existing string/optional-text domain;
the original host validators remain authoritative. A bad later parameter
prevents the whole group from being admitted. The Rust native debugger proves
the confirmed-text/filter/form chain through correlated acknowledgements and
does not expose private frames. This is not a live Avalonia or browser-control
test, a desktop list inspector or multipresentation batch support.

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
apply. This does not add effectful loops, mid-loop host
suspensions or a scalar inspector to the GUI. Collection-driven pure traversal
uses the separate bounded `fold` construct above.

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

The runtime carries a typed scalar-versus-external failure class through this
calculation, rather than comparing diagnostic strings. `LSV1401`/`LSV1408` remain
the outward codes, not permission to recover an arbitrary error. Unhandled errors
are converted at the evaluation/re-entry boundary; selected recovery avoids that
temporary diagnostic allocation. See [typed recovery](leselang-embedding.md#shared-typed-calculation-recovery).

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
admits no member. Only [prepared atomic members](#prepared-atomic-members) may
wrap an atomic operation with pure preparation; recovery cannot wrap host effects.

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
| `ui.set_selection`, `ui.assert_selection`, `ui.wait_selection` | `selected` | boolean |
| `ui.assert_form_field_required`, `ui.wait_form_field_required` | `required` | boolean |
| Non-nullable text, automation-ID, action-label, accessibility, form-value and form-label assertions/waits listed below | `expected`, the acknowledged expectation | string |
| `ui.set_form_value` and all current `ui.assert_form_*` / `ui.wait_form_*` results | `field`, the form field key | string |
| `ui.set_form_value` | `value`, the acknowledged submitted value | string |
| Node-kind, action-kind and form-input-kind assertions/waits listed below | `kind`, the acknowledged canonical enum token | string |
| Placeholder and action-unavailability assertions/waits listed below | `optional_expected`, the acknowledged nullable expectation | optional_string |

Only these fields are exported. Collections, arbitrary record fields, arbitrary nullable
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
atomic captures use the durable chain rules below. An outer `choose` selects a
complete result-binding program; a [prepared atomic result binding](#prepared-atomic-result-bindings)
selects/prepares one uniform atomic call directly as a `bind` value. Neither
form hides effects in pure operands or host arguments.

## Boolean GUI Projections

```leselang
fn destination(selected: boolean, required: boolean) = choose(
  when: and(left: selected, right: not(value: required)),
  then: "run-action",
  otherwise: "next-field",
)
fn main() = bind(
  selection: ui.set_selection(node_id: "runtime-row", state: "selected"),
  body: bind(
    requirement: ui.assert_form_field_required(
      node_id: "settings-form", field: "name", state: "optional",
    ),
    body: ui.focus(node_id: destination(
      selected: field(value: selection, name: "selected"),
      required: field(value: requirement, name: "required"),
    )),
  ),
)
```

`selected` is `true` for an acknowledged `selected` state and `false` for
`unselected`. `required` is `true` for `required` and `false` for `optional`.
These are booleans, not the host argument strings: use them directly in `choose`,
boolean logic, pure helpers and pure loops, or use explicit `to_string` when a
host parameter needs text. A successful correlated acknowledgement is required;
wrong node/state replies and rejected assertions do not become `false`.
This is not an arbitrary UI read or a new query operation. In particular, an
assertion still fails when its expected state does not match the host.
Named group members export the same fields under the current group-flow rules.
Raw form objects, nullable metadata, credential objects and dynamic properties remain private.

When persisted across another suspension, selection-only results use a closed
projection frame with `projection_version: 2`. Requirement results now also
export a form `field` key and use the [v3 text vocabulary](#text-gui-projections);
their older v2 frames remain valid without that key. Operations without booleans
or text additions retain the unversioned v1 wire shape.
Unversioned v1 frames retain exactly the original five-field vocabulary;
the new reader never synthesizes absent booleans from an operation's name or
old request. A v1 frame can coexist with a new v2 frame in one chain.
Aliases and both cold conditional paths are checked against the actual stored
field set before restore or successor admission. Absent-field access, unsupported
numeric versions, extra/duplicate/reordered projection fields and incorrect scalar
types fail with `LSV1405`. Invalid JSON shapes and unknown object properties fail
with the existing codec error `LSV3003`; oversized images retain `LSV3002`.
Recovery also checks each saved boolean against its committed raw result.
Old readers reject the new field/explicit-version forms rather than discarding
them. Continuation schemas 1-6 and journal schema 10 remain unchanged.
Copying/restoring the additional scalar costs the same shared fuel, and all
authority, deadline, output and 64 KiB continuation limits still apply.

## Text GUI Projections

```leselang
fn suffix(text: string) = concat(left: text, right: "-copy")
fn main() = bind(
  written: ui.set_form_value(node_id: "settings-form", field: "name", value: "alpha"),
  body: bind(
    checked: ui.wait_form_value(
      node_id: field(value: written, name: "node_id"),
      field: field(value: written, name: "field"),
      expected: field(value: written, name: "value"),
    ),
    body: bind(
      copied: ui.set_form_value(
        node_id: "settings-form", field: "label",
        value: suffix(text: field(value: checked, name: "expected")),
      ),
      body: eq(left: field(value: copied, name: "value"), right:
        suffix(text: field(value: written, name: "value")),
      ),
    ),
  ),
)
```

The new fields are **strings**, not implicit booleans, numbers or host objects.
They expose only data in a **successful correlated acknowledgement**:

| Field | Exact Exporting Operations |
| --- | --- |
| `expected` | `ui.assert_text`, `ui.wait_text`, `ui.assert_automation_id`, `ui.wait_automation_id`, `ui.assert_action_label`, `ui.wait_action_label`, `ui.assert_form_value`, `ui.wait_form_value`, `ui.assert_form_field`, `ui.wait_form_field`, `ui.assert_accessible_name`, `ui.wait_accessible_name`, `ui.assert_accessible_description`, `ui.wait_accessible_description` |
| `field` | `ui.set_form_value`, plus assert/wait pairs for `form_value`, `form_field`, `form_field_input_kind`, `form_field_required`, `form_field_max_length`, `form_field_placeholder` |
| `value` | `ui.set_form_value` only |

`expected` is the **acknowledged expectation**, and `value` is the **acknowledged
submitted value**. Neither is a new live-property query or a promise to read
arbitrary current GUI text. An assertion/wait must still succeed against its
expected value; a wrong node, field, expectation or submitted-value reply creates
no successor. An optional placeholder/unavailability `expected` is **not exported**,
even when a particular request passes a string. There is no implicit string/`none`
union or default; use the separate typed [optional projection](#optional-gui-projections).
The separate [kind vocabulary](#kind-gui-projections)
exports only canonical kind tokens. Arbitrary credentials/record properties remain unsupported.

These fields work through immutable aliases, pure helpers, explicit conversions,
conditional scalar exits and named group members. The same type/capability and
all-success rules apply. Strings retain their operation-specific byte limits:
form values are at most **256 bytes**, form keys **128 bytes**, and general
expected text **1024 bytes**; automation IDs keep their identifier validator.
The generic scalar ceiling remains **4096 bytes**, not permission to exceed a
narrower host-argument limit. Feeding a valid long text projection into a form
value still rejects `LSV1404` before creating another request. Empty/Unicode text
is not trimmed or coerced, and raw cumulative-output checks remain in force.

Captured results that export these string fields use **`projection_version: 3`**,
except form-input-kind assertions/waits, which also export `kind` and now use v4,
and placeholder assertions/waits, which export `optional_expected` and use v5.
Their older v3 frames retain their exact fields without the newer projections.
V3 is the frozen V2 vocabulary plus `expected`, `field`, `value`, in that canonical
order after the old fields. Each operation stores exactly its own exported subset.
Unversioned **v1 and explicit v2 frames remain byte-exact** and can coexist with
new v3 frames in one chain; aliases preserve the original version and field set.
The reader **never synthesizes missing text fields** from a retained raw receipt
or request. Cold branches and conditional aliases use the actual closed field
sets, so missing legacy fields reject `LSV1405` before restore/admission. Unsupported
versions, wrong types, duplicate/missing/reordered fields and unknown properties
fail closed. Recovery checks every saved string against its **committed raw receipt**.
Older readers reject v3/new-field forms rather than dropping data.

Copying/restoring these strings consumes **shared fuel**. The complete image/plan
still fits **64 KiB**; selected group/alias growth commits `LSV3002` without a
phantom request, while an unselected cold branch creates no frames. Raw receipts,
new frames and the next request or final scalar retain the existing **one transaction**
boundary. Authority, revision, absolute deadline and raw-output budgets are unchanged.
**Continuation schemas 1-11 and journal schema 10 remain unchanged**; this is a
projection-vocabulary extension, not another continuation or database migration.

The native single-presentation debugger supports sequential text-driven form
steps with the same effect identity/session-revision checks and **no public private
frames**. Arbitrary GUI-property reads, automatic adapter discovery, secret
extraction, native parallel batches and a desktop scalar inspector are not added.
Explicit caller-provided source/values still follow the host's secret-handling
rules; the vocabulary is not a credential-redaction mechanism.

## Kind GUI Projections

```leselang
fn destination(kind: string) = choose(
  when: eq(left: kind, right: "runtime_refresh"), then: "run", otherwise: "skip",
)
fn main() = bind(
  checked: ui.assert_action_kind(node_id: "refresh-action", kind: "runtime_refresh"),
  body: bind(
    waited: ui.wait_action_kind(
      node_id: "refresh-action", kind: field(value: checked, name: "kind"),
    ),
    body: ui.focus(node_id: destination(kind: field(value: waited, name: "kind"))),
  ),
)
```

`kind` exports a **canonical enum token string** from a **successful correlated
acknowledgement**. It is the acknowledged requested kind, **not a live-property query**
or a new scalar enum type. Only these six operations export it:

| Operations | Exact Canonical Tokens |
| --- | --- |
| `ui.assert_node_kind`, `ui.wait_node_kind` | `column`, `heading`, `text`, `runtime_card`, `runtime_workspace`, `section`, `history_entry`, `log_entry`, `debugger_workspace`, `debugger_frame`, `action` |
| `ui.assert_action_kind`, `ui.wait_action_kind` | `runtime_inspect`, `runtime_refresh`, `runtime_capabilities_refresh`, `runtime_deploy`, `debugger_cancel` |
| `ui.assert_form_field_input_kind`, `ui.wait_form_field_input_kind` | `path_token`, `trimmed_text` |

Tokens share the existing source/wire enum spelling. There is **no case folding,
trimming, alias or ordinal conversion**. They can feed helpers, comparisons,
conditional exits, group-member projections and computed host arguments. Each
receiving argument still validates its **own operation domain**: a node kind such
as `heading` passed to an action-kind argument rejects `LSV1404` before a successor
is admitted. Wrong-node or wrong-kind acknowledgements similarly create no successor.
Raw `expected_kind` / `input_kind` property names and arbitrary enum properties
are not projected. Nullable metadata uses the separate optional vocabulary below.
Selection/requirement remain booleans, not kind tokens.

Captures exporting `kind` use **`projection_version: 4`**. V4 is the frozen V3
vocabulary followed by `kind`; each operation stores exactly its own exported
subset in canonical order. **Legacy v1/v2/v3 frames stay byte-exact**, including
form-input-kind v3 frames that contain `field` but no `kind`. Mixed-version chains
preserve each frame's original version, and recovery **never synthesizes missing
kind fields** from a raw receipt or pending request. Both **cold branches and
conditional aliases** are checked against actual field sets, including the
high-bit `kind` availability flag. Older readers reject v4 rather than dropping data.

Unknown/wrong-domain tokens, unsupported versions, wrong types, duplicate/missing/
reordered fields and unknown properties reject `LSV1405` before restoration. A
valid-looking same-domain forged token still fails recovery against its **committed
raw receipt**. Token copies/restoration consume **shared fuel**; raw cumulative
output, the **64 KiB** image/plan bound, authority, revision, absolute deadline and
current-effect cancellation stay unchanged. Receipts, frames and the next request
or scalar result retain **one transaction** and first-commit replay, including
write-failure rollback with no phantom successor.
**Continuation schemas 1-11 and journal schema 10 remain unchanged**.

The native sequential debugger supports kind-driven waits/decisions with correlated
effect identities and session revisions, **without exposing private frames**.
This does not add arbitrary GUI reads, native parallel batches, new enum families,
generic nullable projections or a desktop scalar inspector.

## Optional GUI Projections

```leselang
fn defaulted(value: optional_string) = value_or(left: value, right: "No hint")
fn main() = bind(
  checked: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none),
  body: bind(
    waited: ui.wait_form_field_placeholder(
      node_id: "form", field: "name",
      expected: field(value: checked, name: "optional_expected"),
    ),
    body: ui.set_form_value(
      node_id: "form", field: "label",
      value: defaulted(value: field(value: waited, name: "optional_expected")),
    ),
  ),
)
```

`optional_string` is a **distinct scalar type**, not a string/`none` coercion or a
general union/container type. `optional_string(value: none)` constructs an absent
value; `optional_string(value: "")` constructs a present empty string. **Absent and
empty are different**. Constructors accept only pure string/`none` values and
literal constructors normalize to typed literals for canonical round trips.
`has_value(value: optional)` returns a boolean. `value_or(left: optional, right:
fallback)` returns a string and **evaluates the fallback only when absent**.
Both operands must be pure and correctly typed, including a cold fallback; this
does not catch host/resource failures or introduce effectful recovery.

Typed helpers and pure loop state may use `optional_string`; same-type `eq`/`ne`
distinguish absent, empty and text. There is **no implicit lifting or unwrapping**:
plain strings/`none` cannot substitute for an optional helper parameter, and an
optional value cannot be passed to `concat`, `to_string`, a condition or a normal
string host argument. Use `has_value`/`value_or` explicitly. The existing `none`
literal, type and wire form are unchanged.

Exactly four successful correlated acknowledgements export `optional_expected`:
`ui.assert_form_field_placeholder`, `ui.wait_form_field_placeholder`,
`ui.assert_action_unavailable_reason`, `ui.wait_action_unavailable_reason`.
It is the **acknowledged nullable expectation**, **not a live-property query**.
The older `expected` string projection remains unsupported for these operations.
Only their existing **optional-text host arguments** accept the new typed optional
value directly and resolve it to the same concrete string/`none` request as before.
Filters/targets and other nullable domains are not automatically broadened.
Wrong absent/empty/text acknowledgements create no successor.

These captures use **`projection_version: 5`**. V5 is frozen V4 plus
`optional_expected`, with exact operation subsets and canonical order. **Legacy
v1-v4 frames remain byte-exact**, without synthesized optional fields; an absent
field is not a present field holding an absent value. Cold branches, group members
and conditional aliases use actual field sets, including the high-bit flag.
New scalar payloads are explicit `{"kind":"optional_string","value":null}` or
the same tag with a string `value`. **Missing payload is rejected**, as are wrong
types, unknown properties, unsupported versions and malformed field layouts.
Malformed scalar JSON retains codec error `LSV3003`; semantically invalid saved
projections reject `LSV1405`. Recovery checks even valid-looking
absent/empty replacements against the **committed raw receipt**.

Optional text retains **1024 bytes** and original control-character validation;
generic optional scalars retain **4096 bytes**. Unwrapping does not bypass a
receiving form value's **256 bytes** limit (`LSV1404`). Optional copies, fields,
locals and restoration consume **shared fuel**, including contained string bytes.
Raw output limits, the **64 KiB** image/plan bound, authority, revision, absolute
deadline and cancellation remain unchanged. Receipts, frames and a successor or
final scalar retain **one transaction**, rollback on injected write failures and
first-commit replay. **Continuation schemas 1-11 and journal schema 10 remain unchanged**.
Older readers reject new optional values/fields instead of silently losing data.

The native sequential debugger supports absent/empty/text-driven waits and explicit
form defaults **without exposing private frames**. Generic options, collections,
arbitrary GUI reads, effectful loops, native parallel batches and a desktop scalar
inspector are not added.

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

The schema-6 slice requires a pure scalar body. It permits scalar calculation, aliases,
`choose`, bounded pure `loop`, conversions and `recover`. Sequential groups can
instead drive [one atomic tail](#sequential-group-tails) using schema 7. Neither
form captures the tail; [schema 8](#captured-sequential-successors) allows one
captured successor followed by a pure scalar body. Parallel `all` groups use
[schema 9](#parallel-group-successors) for the same single-successor forms after
an all-success barrier. No form
permits raw group/member return, capture inside an atomic result chain,
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
Journal group budget overflow commits with the triggering raw receipt rather than
leaving it permanently pending: parallel aggregate item overflow saves `LSV2404`,
and an oversized aggregate saves `LSV3002` at the 8 MiB terminal-entry limit.
This also applies to existing schema-6, schema-7 and schema-8 owned group paths
and unbound journal groups; public `merge_declared` still reports its ordinary
bounded merge error. It does not change old wire markers or refund fuel.

The SQLite journal remains schema 10 because the table layout is unchanged.
Old readers reject schema 6 and the new order markers rather than silently
discarding the body. Unbound group plans and schema-1 images retain their wire
representation. The native debugger supports sequential presentation groups
through its existing correlated acknowledgement channel; the scalar is available
through the Rust VM, not a new desktop inspector. Flat `all` remains available
through the Rust batch API, not the debugger's single-presentation channel.

## Sequential Group Tails

```leselang
fn main() = bind(
  checked: seq(
    move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
    verify: ui.assert_visible(node_id: "runtime-b"),
  ),
  body: ui.focus(node_id:
    field(value: member(value: checked, name: "move"), name: "focused_node_id"),
  ),
)
```

After every prefix member succeeds, the named group results may drive exactly
one atomic tail. Its parameters and lazy `choose` predicate can read statically
typed members, including `selected` / `required` booleans. Scalar locals, group
or member aliases, pure helpers, conversions, `recover` and bounded pure loops
may prepare the tail. A conditional tail must choose atomic effects of the same
result type; its cold branch still undergoes type and capability preflight.
The final value is the tail's own typed host result, not the raw prefix aggregate.

This is limited to sequential `seq`, bounded `repeat` and their flattened or
computed members. The prefix has at most **63 members**, leaving one of the 64
graph slots for the tail. Pure group bodies keep their existing 64-member limit.
Parallel `all` tails use [schema 9](#parallel-group-successors). Mixed scalar/effect exits, nested
tail groups and effectful loops are compile errors (`LSH1412`); unsupported cold
branches are rejected too. Prefix arguments remain fully prepared before the
first suspension, not result-dependent between its members.
A further result capture uses the separate [schema-8 form](#captured-sequential-successors),
not an unbounded extension of a schema-7 tail.

Continuation **schema 7** marks every prefix and tail image with the same group
owner. The merge plan uses `group_tail` and an explicit `successor_sequence`
reservation. The VM reserves this identity after the prefix identities at
startup, but a reservation is not a dispatch: no tail request or lease exists
before the whole prefix completes. Each image still contains only its concrete
atomic operation, not a group snapshot or a durable raw-result frame.

The final prefix acknowledgement and tail admission commit in **one transaction**.
The tail is appended to the original sequential graph using the reserved identity
and a collision-free `successor` / `successor_N` branch name. Write failures roll
back both changes, allowing a retry with that same reservation. Competing workers
and repeated acknowledgements replay the first committed tail, never recalculate
its arguments or create another dispatch. Restart requires the **complete journal**;
`Vm::restore` and `Vm::restore_request` reject isolated prefix and tail images
with `LSV1409`. Public merging cannot bypass this transactional admission.

The tail inherits the original principal, capability grants, target revision,
output limit and absolute deadline. Prefix effects, scalar restoration, body
calculation and tail admission consume shared fuel; recovery never refills it.
Raw cumulative output is checked before projection and again with the tail.
Wrong replies, revision mismatch, host failure, cancellation or expiry seal the
unit without creating a tail. Pure arithmetic/parse recovery does not catch these
failures. If tail preparation fails, the final prefix result and calculation
fault (including host-argument rejection `LSV1404`) commit together and replay
after restart. Command tails retain confirmation and correlated-lease requirements.

Retention protects the live group and removes a completed prefix plus tail as
one logical record. Reopen validates the original requests, owner links, prefix
signature, reserved identity/watermark, tail type, authority, budgets and saved
terminal result without running source code. The journal remains **schema 10**;
there is no table-layout migration. Existing schema-6 pure group plans omit
`successor_sequence` and retain their old bytes. Old readers reject schema 7,
`group_tail` or the strict reservation field instead of silently dropping the tail.
The native debugger uses its existing revision- and identity-correlated presentation
channel; the public session view does not expose private group metadata.

## Captured Sequential Successors

```leselang
fn main() = bind(
  checked: seq(
    move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
    verify: ui.assert_visible(node_id: "runtime-b"),
  ),
  body: bind(
    focused: ui.focus(node_id:
      field(value: member(value: checked, name: "move"), name: "focused_node_id"),
    ),
    body: eq(
      left: field(value: focused, name: "node_id"),
      right: field(value: member(value: checked, name: "move"), name: "focused_node_id"),
    ),
  ),
)
```

A sequential prefix can now drive **one captured atomic successor**, followed
by a **pure scalar body**. That final calculation can read both the successor's
typed fields and the earlier named group members, including immutable group and
member aliases, scalar locals, booleans, pure helpers, conversions, bounded pure
loops and `recover`. Lazy `choose` can select different captured atomic operation
types if their final scalar types agree. Every path must perform that one capture
before returning; pure early exits, an uncaptured/captured mixture and tail groups
remain unsupported. A second post-group suspension now uses
[schema 10](#group-result-chains). The parallel `all` form
uses the separate [schema-9 barrier contract](#parallel-group-successors).
The **63-member prefix + one successor** graph limit is unchanged.

Continuation **schema 8** and the `group_capture` plan distinguish this form from
schema-6 pure groups and schema-7 uncaptured tails. All prefix/successor images
retain one group owner. Only the successor carries a result binding with a
`groups` environment: a bounded list of named member projections, not raw runtime
lists, node trees, handles or arbitrary objects. Each member preserves its exact
operation, closed field set and projection version. Group/member aliases retain
their declared names and types; missing fields are never synthesized, including
through aliases and cold branches. Legacy bindings omit the empty `groups` field
and preserve existing schema-1 through schema-7 bytes.

The successor admission uses the same reserved identity and final-prefix
transaction as an uncaptured tail. Snapshotting each member/alias and restoring
the final environment consume **shared fuel**; there is no restart refill.
The complete successor image must fit **64 KiB**. Oversized snapshots fail with
`LSV3002` before a successor dispatch is inserted. A preparation fault commits
with the last raw prefix receipt and is replayed after restart.

The successor's **raw receipt** is retained in its effect record. Raw prefix plus
successor output is checked **before scalar projection** (`LSV2404` on cumulative
overflow). The raw receipt and final scalar or calculation fault then commit in
**one transaction**. Failed writes leave the successor pending; successful commits,
competing workers and duplicate acknowledgements replay the saved scalar/fault
without rerunning calculation. Pure `recover` still catches only arithmetic/parse
errors, never host rejection, expiry, cancellation, authority or resource faults.
Mutating successors still require confirmation and correlated acknowledgement.

Recovery requires the **complete journal**. Reopen compares each saved group or
member projection with the original committed prefix, validates the successor
type, original authority/budgets and final scalar type, and never re-evaluates
source. Isolated schema-8 images and requests fail with `LSV1409`; public merging
cannot run this owned calculation outside the journal. The live prefix, capture
and scalar remain one protected retention unit. Journal **schema 10** and the
table layout are unchanged; old readers reject schema 8, `group_capture` or the
strict `groups` field. The native debugger advances through the existing correlated
presentation channel without publishing private projection frames.

## Parallel Group Successors

```leselang
fn main() = bind(
  inventory: all(
    edge: runtime.list(role: "edge"),
    worker: runtime.list(role: "worker"),
  ),
  body: bind(
    focused: ui.focus(node_id: to_string(value:
      field(value: member(value: inventory, name: "edge"), name: "count"),
    )),
    body: add(
      left: field(value: member(value: inventory, name: "worker"), name: "count"),
      right: field(value: member(value: inventory, name: "edge"), name: "count"),
    ),
  ),
)
```

A flat parallel `all` prefix can drive **one atomic tail** or **one captured
successor followed by a pure scalar body**. The prefix contains **2 to 63
members**, reserving the 64th graph slot for the successor. Pure schema-6 parallel
groups still permit 64 members. Computed arguments, scalar locals, group/member
aliases, closed boolean projections, pure helpers and lazy typed selection retain
the sequential forms' rules. Every prefix argument and every cold branch's type
and capability requirements are checked before any dispatch. Members cannot
calculate their parameters from earlier replies; no nested tail group, mixed
pure/host exits or effectful loop is admitted. Multiple post-group captures use
the separate [schema-10 chain contract](#group-result-chains).

Continuation **schema 9** and the explicit `parallel_group_tail` /
`parallel_group_capture` plan markers preserve a **parallel prefix + all-success
barrier + one atomic successor**. Every prefix member is independently ready and
leaseable. Replies may arrive out of order; named member order still follows the
declared signature, not completion timing. A failed, cancelled or expired member
does not create a tail. Other prefix members remain independent and the group
waits for their terminal replies, retaining the existing parallel `all` policy
rather than turning it into sequential fail-stop. Multiple failures are selected
in declared member order, not by the fastest worker.

The reserved successor identity has no effect/dispatch row before the barrier.
The **last successful prefix receipt and successor admission commit in one
transaction**; competing workers create exactly one request and duplicate prefix
acknowledgements expose that same request. Progress reads the barrier and the
admitted successor in one journal snapshot. The SQL execution-order marker remains
`parallel`: after admission all prefix receipts are already committed and only
the successor is pending, so no new table layout or migration is required.

The successor inherits the **original authority, revision, deadline and output
budget**. Parallel requests do not each grant another calculation budget: one
effect unit per prefix member is reserved from **shared fuel**, and preparation,
projection snapshot/restore, the successor and final calculation consume what
remains. Reopening with a different VM fuel setting never refills that budget.
The bounded plan and successor image must each fit **64 KiB**. Oversized member
snapshots commit `LSV3002` with the last raw prefix receipt and dispatch nothing.

All **raw prefix and successor output is checked before scalar projection**.
Cumulative overflow commits a durable `LSV2404` terminal, including when the last
parallel reply crosses the limit before tail admission. It is not left as a
permanent failed-commit/retry loop. An oversized merged prefix or captured raw
aggregate similarly commits `LSV3002` at the **8 MiB** terminal-entry byte limit,
even if every individual receipt fits. The successor's raw receipt and final scalar
or calculation fault also commit in one transaction. Failed writes roll back
both admission/completion and the triggering receipt; successful writes replay
the first saved result without re-running source or calculation. Command tails
still require confirmation and correlated lease acknowledgement.

Captured successors use the same bounded, versioned `groups` projection frames
as schema 8, including immutable aliases. Recovery requires the **complete owned
journal**, verifies exact member signatures and saved projections against the
**committed successful prefix**, and rejects an admitted tail with any missing or
unsuccessful prefix receipt. Isolated schema-9 images/requests reject `LSV1409`;
public merging cannot bypass captured calculation ownership. Prefix, successor
and terminal scalar remain one protected retention unit. **Journal schema 10**
and schema-1 through schema-8 wire bytes are unchanged; old readers reject schema
9 and the new plan markers.

This is supported through the **Rust batch API**, not an automatic GUI batching
promise. The native debugger still exposes a **single-presentation channel** and
rejects these parallel starts with `debugger_session_not_suspended` during
ephemeral preflight, before creating a session or SQLite journal. Multi-request
debugger presentation/acknowledgement remains pending.

## Group Result Chains

```leselang
fn main() = bind(
  g: seq(
    move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
    check: ui.assert_visible(node_id: "runtime-b"),
  ),
  body: bind(
    focused: ui.focus(node_id:
      field(value: member(value: g, name: "move"), name: "focused_node_id"),
    ),
    body: bind(
      verified: ui.assert_visible(node_id: field(value: focused, name: "node_id")),
      body: eq(
        left: field(value: verified, name: "node_id"),
        right: field(value: member(value: g, name: "move"), name: "focused_node_id"),
      ),
    ),
  ),
)
```

A whole named sequential or flat parallel group can drive **multiple captured
atomic successors**, followed by a pure scalar result. Each successor can use
typed fields from the original group and earlier captures. Pure preparation,
scalar locals, immutable group/member/result aliases, helpers, conversions,
bounded pure loops and pure calculation recovery keep their existing rules.
Lazy `choose` may select different atomic operations or chain lengths, but every
non-pure path must suspend before returning: **mixed scalar early exits between
suspensions remain unsupported** by schema 10, and use the separate
[schema-11 conditional contract](#group-conditional-exits). Nested/dynamic tail
groups, effectful operands and effectful loops are not admitted. Expanded helpers
must satisfy this same group-owned flow contract.
The flat prefix still prepares every member before its first suspension, never
from earlier member replies. All cold branches are type/capability-checked first.

The **prefix plus longest cold chain fits in 64 graph slots**. For two captures
this leaves at most 62 prefix members; sequential prefixes require at least one
member and parallel prefixes at least two. All possible successor identities are
reserved at group admission. `successor_sequence` retains the first reservation;
the optional `additional_successor_sequences` list stores subsequent reservations.
Unused reservations on shorter paths have no effect/dispatch rows and cannot be
reused by unrelated work. Only **one successor is pending at a time**. The original
parallel all-success barrier and independent prefix leases are unchanged.

Continuation **schema 10** with `group_dataflow` / `parallel_group_dataflow`
plans owns the complete prefix and admitted chain. This is a continuation version,
not a new database migration: **journal schema 10 is unchanged**, as are schema-1
through schema-9 wire forms. Old readers reject the new image/plan markers.
Successor bindings retain closed, versioned `groups` and atomic-result projection
frames, not raw host objects. Boolean availability masks and legacy v1 fields are
preserved exactly, including through new aliases between captures.

The **raw receipt and next request commit in one transaction**. Competing or
duplicate acknowledgements expose the same chosen successor. The final raw
receipt and scalar/calculation fault also commit together; failed writes leave
the current step pending and create no successor. Every step inherits the
**original authority, revision, absolute deadline and output budget** and consumes
the remaining **shared fuel** for calculation and projection snapshot/restore.
Restarting with more fuel never refills the chain. Commands still require explicit
confirmation and correlated lease acknowledgement.

All **raw cumulative output is checked before every admission and before scalar
projection**, including earlier captures whose returned objects are not used.
Item overflow commits `LSV2404`; an aggregate over the **8 MiB** terminal-entry
limit or a saved image/plan over **64 KiB** commits `LSV3002`. Preparation or final
calculation faults are durable too, and cannot leave an invisible next request.
Pure recovery never catches host, authority, cancellation or resource failures.

Recovery requires the **complete owned journal**. It validates every successful
prefix receipt, reserved identity, original envelope, decreasing fuel, projection
frame against committed raw receipts, and frame continuity between captures.
An admitted successor with an unsuccessful/missing predecessor is rejected.
Committed requests and scalar/fault results replay without reevaluating source.
Isolated schema-10 image/request restoration rejects `LSV1409`, and public merging
cannot run the owned capture body. The live prefix and chain remain one protected
retention unit; completed units compact together, including unused reservations.

The **native single-presentation debugger supports sequential group chains**
through its existing correlated acknowledgement/revision channel without exposing
private frames. Parallel prefixes remain a **Rust batch API** feature: native
multi-request debugger starts are rejected before creating a session or journal.
This is not a new GUI batching, at-most-once external effect, or full-shell claim.

## Group Conditional Exits

```leselang
fn main() = bind(
  g: seq(
    move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
    check: ui.assert_visible(node_id: "runtime-a"),
  ),
  body: choose(
    when: eq(
      left: field(value: member(value: g, name: "move"), name: "focused_node_id"),
      right: "runtime-home",
    ),
    then: true,
    otherwise: bind(
      focused: ui.focus(node_id:
        field(value: member(value: g, name: "move"), name: "focused_node_id"),
      ),
      body: choose(
        when: eq(left: field(value: focused, name: "node_id"), right: "runtime-b"),
        then: true,
        otherwise: bind(
          verified: ui.assert_visible(node_id: field(value: focused, name: "node_id")),
          body: eq(left: field(value: verified, name: "node_id"), right:
            field(value: member(value: g, name: "move"), name: "focused_node_id"),
          ),
        ),
      ),
    ),
  ),
)
```

A successful whole sequential or flat parallel prefix can now return a typed
scalar **before the first capture or between captures**. Lazy `choose` decides
between a pure scalar exit and another captured atomic operation, using typed
group/member/result projections and immutable aliases. Every exit has the
**same scalar type**. Pure helpers, conversions, loops and local calculation
recovery retain their existing bounded rules. No raw-host/scalar mixed returns,
effectful guards/operands, nested tail groups or effectful loops are admitted,
even on a cold branch. Expanded helpers must obey these same boundaries.

Continuation **schema 11** and `group_conditional` / `parallel_group_conditional`
plan markers identify this flow. **Journal schema 10 is unchanged**; schemas 1-10
keep their prior wire forms and semantic boundaries. Old readers reject the new
markers. A purely scalar group body still uses schema 6, and a group chain without
mixed scalar exits retains its existing schema rather than being upgraded.

The **prefix plus longest cold path fits in 64 graph slots**, even when the chosen
exit needs no successor. All cold branches undergo **type and capability preflight**;
every possible successor identity is reserved before dispatch. Unused reservations
remain rowless and cannot be reused by unrelated work. Only one selected successor
is pending at a time. A parallel prefix retains its **all-success barrier**: an
early-exit condition in one reply cannot bypass another pending or failed member.

The **raw receipt and scalar exit commit in one transaction**, just like the raw
receipt and next request on a continuing path. Duplicate acknowledgements and
conflicting workers replay the **first committed exit or successor**, not a new
decision. Failed writes leave the current request pending and create no child.
Every path keeps the **original authority, revision, absolute deadline and output
budget**, with **shared fuel** that is not refilled on restart. Commands still need
confirmation and a correlated lease acknowledgement.

**Raw cumulative output is checked before any scalar exit or admission**, including
unused earlier replies. Item overflow commits `LSV2404`; the **8 MiB** terminal
payload and **64 KiB** saved-image/plan limits commit `LSV3002`. An unselected
branch does not allocate its projection frames or execute its calculations, but
its types, permissions and longest-path reservations are still checked. Calculation
and selected-frame preparation faults commit durably without phantom work. Pure
recovery never catches host, cancellation, authority, deadline or resource faults.

Recovery requires the **complete owned journal**, validates all successful prefix
receipts and projection-frame continuity, and replays committed requests/scalars
**without reevaluating source**. A scalar terminal is only valid at a saved body
that **can return without another suspension**, not before a mandatory capture.
Missing predecessors, schema downgrades, changed envelopes and reused reservations
are rejected. Isolated image/request restoration rejects `LSV1409`. Live groups
remain one protected retention unit; completed early exits compact as a whole,
including unused reservations.

The **native single-presentation debugger supports sequential conditional flows**
through its existing correlated acknowledgement/revision channel, exposing no
private frames and no extra UI request after a scalar exit. Parallel prefixes
remain **Rust batch API** only; native multi-request starts are rejected before
creating a session or journal, even if their chosen body would return immediately.
This does not claim at-most-once external effects, automatic rollback or a full shell.

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
exactly the scalar fields exported by its projection vocabulary version, in
canonical order. See [boolean GUI projections](#boolean-gui-projections) and
[text GUI projections](#text-gui-projections) and [kind GUI projections](#kind-gui-projections)
and [optional GUI projections](#optional-gui-projections) for v1-v5 compatibility and
absent-field rejection. Raw host objects and
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

Named-shape preflight uses the core's generic parameter metadata before calculation.
Missing keys are not explicit none values; domains, schema selection and authority remain
host-owned. See [named host signatures](leselang-embedding.md#shared-named-host-signatures).
Accepted scalar types and language bounds share core preflight without coercion;
receiving-host formats and exact fuel remain unchanged. See
[scalar contracts](leselang-embedding.md#shared-accepted-scalar-contracts).
Computed source arguments bind through a shared borrowed declaration-order view;
this does not reorder submitted raw values or repair noncanonical saved frames.
See [argument binding](leselang-embedding.md#shared-borrowed-argument-binding).

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
result-capturing or multi-step computation as group members, result-dependent
parameters and dynamic repetition counts remain unsupported.
[Prepared atomic members](#prepared-atomic-members) may wrap exactly one fixed-type
operation with pure `bind`/`choose`; they cannot change those dispatch boundaries.

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

HIR shape/signature checks share the runtime core's checked structural counters.
Original limits, folded source weights and separate computation/effect budgets
remain HIR policy; a structural pass is not lexical/type/purity/capability approval.
See [structural accounting](leselang-embedding.md#shared-structural-walk-accounting).

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
scalar bodies and sequential/parallel groups driving one atomic tail or multiple
captured successors, with typed scalar exits before/between captures and whole-group
journal recovery. Selection/requirement booleans now have versioned durable
projections, alongside confirmed text/form strings and canonical node/action/input-kind
token projections, plus typed optional strings and four nullable metadata projections.
Bounded string collections, pure fold iteration and reusable effectful functions
are implemented through bounded hygienic HIR expansion. Prepared atomic result
bindings also compose pure preparation/selection with existing single captures,
chains, conditional exits and group-owned successors without new wire formats. Generic
nullable/container types, maps and heterogeneous/nested collections remain pending, followed by
bounded effectful loops with exit/skip semantics and explicit host-effect
recovery/cleanup. These are semantic requirements, not prescribed
keywords or a Bash compatibility checklist. Their syntax must satisfy the
[agent-first design rules](leselang-embedding.md#agent-first-syntax); none of
these unimplemented constructs should be generated as if supported.
Combining sequential groups with `all` is also currently rejected, not silently
parallelized. Existing host-managed retries remain separate from pure calculation
recovery. The status tensor tracks this as developing work after the 2.0 scope.
