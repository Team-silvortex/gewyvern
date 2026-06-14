# Book Structure

Use this page when you need the chapter discipline for the `gewyvern` book.

This page does not explain one subsystem.
It explains how the book itself should stay organized as the project grows.

Read this alongside:

- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
- [docs/book/conventions.md](/Users/Shared/chroot/dev/gewyvern/docs/book/conventions.md)
- [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)

## Why This Page Exists

The book now has enough material that “put it somewhere under `docs/book/`”
is no longer good enough.

Without explicit structure discipline, the book will drift toward:

- duplicate explanation chapters
- tutorial pages that really belong in reference
- strong chapters hidden in the wrong part of the reading order
- new pages that are locally useful but globally disorienting

This page keeps the book readable as a real system, not just a collection.

## The Two Axes

The book is organized along two axes:

1. reading mode
2. storyline part

Reading mode answers:

- what kind of page is this?

Storyline part answers:

- where does this page belong in the whole-system reading path?

Every substantial new `docs/book/` page should be placeable on both axes.

## Reading Modes

The four reading modes remain:

- tutorial
- how-to
- reference
- explanation

Those modes define the page type.

They do not by themselves define where a page belongs in the larger book.

## Storyline Parts

The current book storyline is intentionally divided into six parts.

### Part I: First Contact

This part is for first-use confidence.

It should contain pages that help a reader:

- run something real
- see one concrete result
- understand the most immediate shape of the project

Typical pages:

- first runtime tutorial
- first package tutorial

### Part II: The Language And Compiler Spine

This part is for the authoring and lowering path.

It should contain pages that explain:

- `gewylang`
- package composition
- frontend surfaces
- lowering
- compiler-facing contracts

Typical pages:

- language guide
- package reference
- frontend/IR explanations

### Part III: The Runtime Spine

This part is for evidence, diagnosis, and export.

It should contain pages that explain:

- runtime materialization
- conservative diagnosis
- summary and analysis surfaces
- operator-facing diagnostic contracts

Typical pages:

- diagnosis reference
- runtime walkthroughs
- failure reasoning explanations

### Part IV: Protocol Packages As A System

This part is for the packaged protocol shelf.

It should contain pages that explain:

- registry resolution
- family and entry identity
- packaged protocol paths
- protocol-specific shelf navigation

Typical pages:

- protocol surface reference
- protocol package spine explanation
- family shelves and narrower family hubs

### Part V: The Broader Stack

This part is for collaboration beyond one runtime.

It should contain pages that explain:

- external-engine collaboration
- nearby sidecars
- `etragon`
- `leserpent`
- fleet/control-plane topology

Typical pages:

- sidecar collaboration
- stack topology
- multi-line coordination pages

### Part VI: Operating, Validating, And Extending

This part is for confident project stewardship.

It should contain pages that explain:

- validation workflows
- contributor routines
- maintenance and documentation rules
- release-line discipline

Typical pages:

- runtime validation how-to
- development guide
- documentation-system pages

## Placement Rules

When adding a new page, decide its part before deciding its filename.

Use these rules:

### Put it in Part I when

- the page exists mainly to help a new reader get moving
- the value comes from following one concrete path

### Put it in Part II when

- the page is mainly about authored intent, package shape, compiler structure,
  or lowering

### Put it in Part III when

- the page is mainly about runtime truth, diagnosis, export, or evidence
  interpretation

### Put it in Part IV when

- the page is mainly about protocol identity, packaged protocol families, or
  registry-driven entrypoints

### Put it in Part V when

- the page is mainly about `etragon`, `leserpent`, sidecars, orchestration, or
  collaboration boundaries

### Put it in Part VI when

- the page is mainly about operating the project responsibly over time

## Chapter Discipline

New pages should strengthen one chapter more often than they create a new one.

Create a new page only when at least one of these is true:

1. the topic has a different reading mode from the existing page
2. the topic belongs in a different storyline part
3. the existing page would become harder to read if expanded further
4. the new page adds a real boundary, not just more examples

If none of those are true, prefer extending an existing page.

## Naming Discipline

Current naming should remain deliberate:

- `tutorial-*` for tutorials
- `how-to-*` for task guides
- `reference-*` for exact lookup
- `explanation-*` for rationale and system understanding

That naming rule matters because it lets readers infer the page type before
opening it.

## Cross-Linking Discipline

Each substantial page should usually link:

1. one upstream page
2. one sibling page
3. one downstream page

That keeps the book navigable without turning every page into a giant menu.

Typical pattern:

- tutorial -> guide/reference
- explanation -> reference/explanation neighbor
- reference -> tutorial/explanation companion

## What Does Not Belong In The Book

Do not force every durable doc into `docs/book/`.

Top-level `docs/` should still hold the long-lived subject shelves such as:

- system
- architecture
- machine contract
- security posture
- documentation system

The book is the reading framework around those shelves, not a replacement for
them.

## Practical Checklist

Before adding a new `docs/book/` page, ask:

1. what reading mode is it?
2. what storyline part is it in?
3. what existing chapter is its closest neighbor?
4. does it deserve a new page, or should an existing page grow?
5. where should a reader go after finishing it?

If those answers are unclear, the page probably needs reframing before it is
added.

## Current Thesis

For the current line, the book should feel like:

- a guided path through one system
- a stable shelf for exact lookup
- a readable architecture narrative
- a maintainable structure for future contributors

That is what will let the docs keep growing without becoming another source of
entropy.
