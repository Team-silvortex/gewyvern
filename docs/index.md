# Documentation

This is the single documentation entry point for `gewyvern`. Choose the module
that owns your question; do not scan the repository by filename.

## Start

- New user: [first run](book/tutorial-first-run.md)
- GewyLang author: [language module](modules/gewylang.md)
- Operator: [operations module](modules/operations.md)
- Contributor: [project module](modules/project.md)
- Next major: [Leserpent 2.0 roadmap](leserpent-2-roadmap.md)
- Sequential reading: [book](book/index.md)

## Modules

| Module | Owns |
| --- | --- |
| [Runtime](modules/runtime.md) | architecture, evidence flow, service and machine contracts |
| [GewyLang](modules/gewylang.md) | language, compiler, package authoring and migration |
| [Protocols](modules/protocols.md) | registry, protocol families, package implementation |
| [Operations](modules/operations.md) | CLI, deployment, validation, security and release gates |
| [Project](modules/project.md) | development, module ownership, packaging and history |

Each module is a small manifest, not a second manual. Subject pages remain the
authority for their content.

## Document Types

- [Tutorials](book/tutorials.md): learn by completing a path.
- [How-to guides](book/how-to.md): complete one task.
- [Reference](book/reference.md): look up an exact contract.
- [Explanation](book/explanation.md): understand design and tradeoffs.
- [History](history/index.md): inspect old release context, never current truth.

## Maintenance

The ownership and placement rules live in
[Documentation System](documentation-system.md). New pages belong to exactly
one module and one document type.
