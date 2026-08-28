# Explanation: Why gewylang Uses Lightweight Parameter Boundaries

`gewylang` now has a limited form of type-like inference and validation around
function reuse.

This page explains why the project chose that shape instead of building a full
static type system.

## The Short Version

`gewylang` is a structured binding language, not a general-purpose programming
language.

That means the goal is not:

- rich expression typing
- full inference across arbitrary user programs
- a heavyweight compile-time theorem about every value

The goal is narrower:

- make reusable function units safer
- catch common dangerous miscalls early
- preserve a small stable language surface

## What gewylang Is Actually For

The DSL exists to compile into `TemplateBinding`.

That binding carries:

- template identity
- fragment selection
- window profile
- reason profile
- program model
- fragment parameter bindings
- evidence tier overrides

So the central design question is not:

- how do we type an arbitrary language?

It is:

- how do we keep package/module composition predictable and safe?

## Why Function Reuse Needed More Safety

As `gewylang` gained:

- function units
- `use(...)`
- default arguments
- named arguments
- package/module composition

it became possible to write reusable helper functions that looked clean but
were easier to miscall.

Examples of mistakes we wanted to catch early:

- passing a free-form stringish value where a stable atom-like identifier is
  expected
- using an invalid `predicate`
- inventing an unsupported `stage` or `key_event`
- passing arbitrary text as `narrative`
- using an inconsistent `phase` shape

Without any boundary, those mistakes tend to drift deeper into the runtime and
become harder to diagnose.

## Why Not Build A Full Type System

A full static type system would sound attractive, but it would push the
language in the wrong direction for this project.

It would increase:

- surface area
- parser and compiler complexity
- migration burden for built-in packages
- cognitive load for contributors
- risk of turning `gewylang` into a mini language platform

That would work against the current `v1.20.x` posture, which favors convergence
and clarity over widening the language again.

## The Chosen Model

Instead of full typing, `gewylang` now infers a light parameter kind from how a
function parameter is used. Authors may also declare that lightweight kind
directly in the function signature when they want the boundary to stay explicit.

Current kinds include:

- `atom`
- `bool`
- `u64`
- `predicate`
- `narrative`
- `stage`
- `key_event`
- `phase`

The important property is that these are not abstract academic types. They are
directly tied to the runtime/compiler concepts that matter for safe reuse.

## Why This Is A Good Fit

This model fits the language because it is:

- local
- explainable
- narrow
- safety-oriented

It does not try to infer everything.

It only tries to answer:

- what kind of value is this parameter clearly being used as?
- can we reject obviously wrong values at `use(...)` time?

That gives a lot of practical benefit without changing the identity of the
language.

## Why The Boundary Is Intentionally Uneven

Not every inferred kind is equally strict for the same reason.

For example:

- `bool` and `u64` are straightforward hard validations
- `predicate` is validated through the real predicate parser
- `stage` and `key_event` are validated against the known stable names
- `narrative` is intentionally restricted to built-ins or explicit
  `static:...`

This is deliberate.

The purpose is not aesthetic consistency. The purpose is to fence the places
where a bad argument would be most misleading later.

## Why Narrative Is Restricted

`narrative` is a good example of the project's philosophy.

If arbitrary free text were always accepted, DSL authors could accidentally
turn rule descriptions into a soft, unvalidated channel that looks official but
is disconnected from the system's stable semantics.

Restricting `narrative` to:

- built-in templates
- or explicit `static:...`

keeps that surface honest:

- built-ins remain structured
- one-off text remains explicitly one-off

## Why This Helps Security

The user concern that drove this work was security, not elegance.

That matters.

This boundary helps because it stops a class of module-reuse mistakes before
they become:

- confusing runtime failures
- surprising diagnosis drift
- overly permissive package composition
- hard-to-review helper functions

In other words, it narrows the space of "looks plausible, but is actually
wrong" calls.

## Why This Still Leaves Room To Grow

This approach does not close the door on richer language features forever.

It simply says:

- the current line should only add type-like structure when it clearly improves
  safety or reuse
- the language should not become a full static programming language by
  accident

That is a good fit for the current project stage.

## Practical Rule

When deciding whether a new parameter boundary belongs in `gewylang`, ask:

1. does this catch a real high-risk miscall?
2. is the inferred kind directly tied to a stable runtime/compiler concept?
3. can the rule be explained simply in documentation and errors?
4. does it keep the language small?

If the answer is yes, it probably fits.

If the proposal mostly makes the language feel more complete in the abstract,
but does not clearly improve safety or reuse, it probably does not fit the
current design.
