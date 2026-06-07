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

## The Four Reading Modes

The book is organized around four reading modes:

- [Tutorials](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorials.md)
  Learn by following a concrete path.
- [How-To Guides](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
  Solve one practical task at a time.
- [Reference](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
  Look up exact syntax, schema, or contract details.
- [Explanation](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)
  Understand the architecture and design rationale.
- [Documentation Conventions](/Users/Shared/chroot/dev/gewyvern/docs/book/conventions.md)
  For the house rules that keep this book coherent.

## Current Release Line

For the current `1.x` release posture, see:

- [docs/v1.4-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v1.4-posture.md)

## Role-Based Reading Paths

The shortest useful path depends on who you are.

### Operator

Start here if your question is:

- can I run this now?
- what should I read first in the output?
- how do I validate today’s runtime surface?

Recommended order:

1. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
2. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
3. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
4. [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)
5. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)

### DSL Author

Start here if your question is:

- how do I write or reuse a gewy package?
- what is the preferred stable `gewylang` subset?
- what parameter boundaries will the compiler enforce?

Recommended order:

1. [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
2. [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
3. [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
4. [docs/book/explanation-gewylang-lightweight-types.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-lightweight-types.md)
5. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)

### Contributor

Start here if your question is:

- where should I land a change?
- how do I validate it before calling it done?
- which internal boundaries should I preserve?

Recommended order:

1. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
2. [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
3. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
4. [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
5. [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)

### Reviewer

Start here if your question is:

- what does this project claim today?
- what evidence supports that claim?
- what is stable versus still evolving?

Recommended order:

1. [docs/v1.4-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v1.4-posture.md)
2. [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
3. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
4. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
5. [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)

## If You Do Not Know Where To Start

Use this fallback:

1. [docs/book/tutorials.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorials.md)
2. [docs/book/how-to.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
3. [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
4. [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)

## Scope

This book does not replace the existing documents. It gives them a clearer
shape and a predictable shelf:

- tutorials for onboarding and confidence
- how-to for practical operations
- reference for exact lookup
- explanation for deeper mental models

As the `1.4.x` line continues, new documentation should prefer landing into one of
these four tracks instead of growing the top-level `docs/` folder without a
clear reading mode.
