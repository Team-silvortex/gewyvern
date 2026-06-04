# gewyvern Book

This is the documentation spine for `gewyvern` and `gewylang`.

Instead of treating `docs/` as a flat folder, this book frames the project in
four reading modes:

- tutorials: learn by following a concrete path
- how-to guides: solve a specific operator or contributor task
- reference: look up a stable command, schema, or language detail
- explanation: understand why the system is shaped the way it is

If you are building, operating, or extending `gewyvern`, this is now the best
high-level entrypoint.

## Start Here

Pick the track that matches your intent:

- [Tutorials](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorials.md)
  For first-time readers who want a guided path.
- [How-To Guides](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
  For readers trying to complete a concrete task.
- [Reference](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
  For command, schema, and language lookup.
- [Explanation](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)
  For architecture, semantics, and design rationale.
- [Documentation Conventions](/Users/Shared/chroot/dev/gewyvern/docs/book/conventions.md)
  For the house rules that keep this book coherent.

## Current Release Line

For the current “complete enough to begin using on purpose” posture, see:

- [docs/v0.13-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.13-posture.md)

## Reading Strategy

- If you are evaluating the project:
  Start with Tutorials, then Explanation.
- If you are using the runtime:
  Start with How-To Guides, then Reference.
- If you are working on `gewylang`:
  Start with Tutorials, then Reference, then Explanation.
- If you are changing internals:
  Start with Explanation, then Reference, then Development-oriented how-to
  material.

## Scope

This book does not replace the existing documents. It gives them a clearer
shape and a predictable shelf:

- tutorials for onboarding and confidence
- how-to for practical operations
- reference for exact lookup
- explanation for deeper mental models

As `v0.13.0` approaches, new documentation should prefer landing into one of
these four tracks instead of growing the top-level `docs/` folder without a
clear reading mode.
