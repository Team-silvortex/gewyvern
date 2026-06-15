# Tutorials

This track is for readers who want to learn `gewyvern` and `gewylang` by
walking through a path, not by skimming every reference page.

## Recommended Order

1. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
   First operator path from repo checkout to a real run.
2. [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
   First package-authoring path for `gewylang`.
3. [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
   One full `.gewy -> frontend -> lowered IR -> archival snapshot` flow.
4. [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
   One full `.gewy -> binding -> runtime -> export` flow.
5. [docs/book/explanation-protocol-package-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-protocol-package-spine.md)
   One full `protocol package -> registry -> gewylang -> IR -> runtime` system
   view.
6. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
   The stable `gewylang` subset and package model.
7. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
   Day-to-day contributor workflow after the mental model is in place.

## Suggested Tutorial Paths

### Operator Path

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)

### gewylang Path

- [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
- [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
- [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
- [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
- [docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf)

### Runtime Internals Path

- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
- [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)

## Future Shape

This track should eventually host more explicit step-by-step tutorials, such
as:

- tracing one protocol family from DSL to report
- running `--serve` with API and external analysis sidecars

For now, the pages above are the current tutorial shelf, and the two book
tutorials are the current primary onboarding chapters.
