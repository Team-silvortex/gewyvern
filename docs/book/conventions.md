# Documentation Conventions

This page defines how documentation should be organized in the current
`gewyvern` repository.

The goal is not to force every page into a rigid template. The goal is to keep
the documentation set coherent as the `0.17.x` line closes its second half.

## Primary Rule

New documentation should land into one of four reading modes:

- tutorial
- how-to
- reference
- explanation

If a page does not clearly fit one of those modes, it usually needs to be
reframed before it is added.

For the larger system map of how those modes fit together with top-level core
pages, validation pages, and history pages, see
[docs/documentation-system.md](docs/documentation-system.md).

For the book-specific part and chapter discipline, see
[docs/book/structure.md](docs/book/structure.md).

## The Four Modes

### Tutorial

Use a tutorial page when the reader is learning by following a path.

A tutorial should:

- assume less context
- proceed in a deliberate order
- optimize for confidence and momentum
- prefer one concrete path over broad branching

Good examples:

- first runtime scan
- first gewy package
- first `--serve` plus API walkthrough

### How-To

Use a how-to page when the reader already understands the system and wants to
complete a task.

A how-to should:

- start from the task
- stay practical
- avoid long theory sections
- end with an observable success condition

Good examples:

- package a Linux release
- validate the field matrix
- add an external analysis sidecar

### Reference

Use a reference page when the reader needs exact lookup.

A reference should:

- be precise
- avoid unnecessary narrative
- make contracts and formats easy to find
- prefer stable names and structures over prose

Good examples:

- DSL syntax
- JSON schema notes
- export format
- machine-facing contract

### Explanation

Use an explanation page when the reader needs design rationale or mental
models.

An explanation should:

- answer “why”
- describe tradeoffs and intent
- connect multiple modules or documents
- avoid pretending to be a quick-start guide

Good examples:

- system layering
- failure semantics
- security posture
- sidecar collaboration model

## Where To Put Things

### Keep top-level `docs/` for durable core pages

Top-level `docs/` should continue to hold the durable subject pages:

- system
- DSL
- export
- security
- machine contract
- architecture

These are the long-lived shelves.

### Use `docs/book/` for navigation and reading structure

`docs/book/` should define:

- how the material is grouped
- what order readers should follow
- what each track is for

This is the reading framework, not a second copy of the same content.

Within `docs/book/`, also decide which storyline part a page belongs to. The
current part discipline is documented in
[docs/book/structure.md](docs/book/structure.md).

## Writing Style

Prefer pages that are:

- short before they are long
- concrete before they are abstract
- explicit about boundaries
- honest about what is still evolving

Avoid:

- speculative promises
- duplicate navigation lists on every page
- mixing tutorial, reference, and explanation in one long note

## Release-Line Discipline

During the active `0.17.x` line, documentation changes should bias toward:

- clarifying current behavior
- reducing contradiction
- making the project easier to adopt intentionally
- making stable versus evolving surfaces clearer
- recording each new minor line with one durable historical snapshot

Documentation should not drift ahead of the runtime.

The docs should describe the real current system, not the imagined future
system.

## Page Expectations

Most new pages should answer three things quickly:

1. what kind of page is this?
2. who is it for?
3. what should the reader do next?

That does not require a rigid template, but it does require intentional page
shape.

## Practical Checklist

Before adding a new page, ask:

- does this belong in tutorial, how-to, reference, or explanation?
- can an existing page be expanded instead?
- does this introduce a second page that says almost the same thing?
- does it help the `0.17.x` line feel more whole?

If the answer to the last question is no, it is probably the wrong page for
this phase.

## Minor Line History

The documentation set now keeps one durable snapshot per **minor** line.

For this project, the middle numeric version component is the minor line:

- `v0.13.x`
- `v0.15.x`
- current and future lines such as `v0.17.x` when those lines actually begin

When a new minor line begins:

- add one new page under
  [docs/history](docs/history/index.md)
- describe what that line means
- describe what was already whole enough
- describe what was still intentionally incomplete
- preserve any small archive-friendly validation summaries that explain the
  line's collaboration or resilience posture, instead of relying on transient
  console output alone

If you need the compact checklist for what those companion artifacts should
look like, use:

- [docs/history/minor-line-evidence-bundle.md](docs/history/minor-line-evidence-bundle.md)

Do not create a separate history page for every patch version unless the patch
series fundamentally changes the line's meaning.
