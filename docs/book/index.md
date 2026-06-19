# gewyvern Book

This is the documentation spine for `gewyvern`, `gewylang`, and the nearby
stack around them.

The goal is not only to collect pages.
The goal is to make the docs readable like a book:

- start with a concrete path
- learn one system at a time
- look up exact contracts when needed
- return to explanation chapters for the deeper architecture

If you are building, operating, reviewing, or extending `gewyvern`, this is
the best high-level entrypoint.

If you want the global documentation map instead of the reading order, use:

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)

## Book Shape

This book is organized in two different ways at once:

1. by reading mode
2. by system storyline

The reading modes help you choose the right page type.
The storyline helps you read the project as one coherent system.

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
- [Book Structure](/Users/Shared/chroot/dev/gewyvern/docs/book/structure.md)
  For the part/chapter discipline that keeps the book readable as one system.

## Suggested Storyline

If you want to read this like a technical book instead of a reference shelf,
use this order:

### Part I: First Contact

1. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
2. [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)

### Part II: The Language And Compiler Spine

3. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
4. [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
5. [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
6. [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
7. [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
8. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

### Part III: The Runtime Spine

9. [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
10. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
11. [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
12. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
13. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
14. [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)

### Part IV: Protocol Packages As A System

15. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
16. [docs/book/explanation-protocol-package-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-protocol-package-spine.md)
17. [docs/architecture-walkthrough-http-request.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-walkthrough-http-request.md)

### Part V: The Broader Stack

18. [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
19. [docs/book/explanation-stack-topology.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-stack-topology.md)
20. [docs/architecture-coordination.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-coordination.md)

### Part VI: Operating, Validating, And Extending

21. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
22. [docs/book/how-to-fault-inject-runtime-resilience.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-fault-inject-runtime-resilience.md)
23. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
24. [docs/documentation-system.md](/Users/Shared/chroot/dev/gewyvern/docs/documentation-system.md)

## Current Release Line

For the current `0.15.x` release posture, see:

- [docs/v0.15-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.15-posture.md)

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
3. [docs/book/how-to-fault-inject-runtime-resilience.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-fault-inject-runtime-resilience.md)
4. [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
5. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
6. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
7. [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)
8. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)

### DSL Author

Start here if your question is:

- how do I write or reuse a gewy package?
- what is the preferred stable `gewylang` subset?
- what parameter boundaries will the compiler enforce?

Recommended order:

1. [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
2. [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
3. [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
4. [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
5. [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
6. [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
7. [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
8. [docs/book/explanation-gewylang-lightweight-types.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-lightweight-types.md)
9. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
10. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
11. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)

### Contributor

Start here if your question is:

- where should I land a change?
- how do I validate it before calling it done?
- which internal boundaries should I preserve?

Recommended order:

1. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
2. [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
3. [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
4. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
5. [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
6. [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)

### Reviewer

Start here if your question is:

- what does this project claim today?
- what evidence supports that claim?
- what is stable versus still evolving?

Recommended order:

1. [docs/v0.15-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.15-posture.md)
2. [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
3. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
4. [docs/book/how-to-fault-inject-runtime-resilience.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-fault-inject-runtime-resilience.md)
5. [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
6. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
7. [docs/book/explanation-conservative-diagnosis.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-conservative-diagnosis.md)
8. [docs/book/explanation-stack-topology.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-stack-topology.md)

## If You Do Not Know Where To Start

Use this fallback:

1. [docs/book/tutorials.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorials.md)
2. [docs/book/how-to.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
3. [docs/book/reference.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference.md)
4. [docs/book/explanation.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation.md)

## Protocol-Family Quick Paths

When you already know which protocol family you are working on, use the family
directory page instead of scanning the whole book:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
3. one family hub page such as Redis, FTP, SMTP, MQTT, LDAP, or PostgreSQL
4. one narrower family subpage
5. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Scope

This book does not replace the existing documents. It gives them a clearer
shape and a predictable shelf:

- tutorials for onboarding and confidence
- how-to for practical operations
- reference for exact lookup
- explanation for deeper mental models

As the `0.15.x` line continues, new documentation should prefer landing into one of
these four tracks instead of growing the top-level `docs/` folder without a
clear reading mode.

Use:

- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  for storyline and chapter order
- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
  for the global documentation shelves
