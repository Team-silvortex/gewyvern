# Documentation

This is the single documentation entry point for `gewyvern`. Choose the module
that owns your question; do not scan the repository by filename.

## Start

- Choose a working outcome: [tutorial shelf](book/tutorials.md)
- CLI operator: [first real run](book/tutorial-first-run.md)
- Leserpent overview: [product page](../LESERPENT.md)
- Desktop operator: [first Leserpent session](book/tutorial-leserpent-desktop.md)
- GewyLang author: [first package](book/tutorial-gewylang-package.md)
- Leselang author: [first GUI automation](book/tutorial-leselang-gui-automation.md)
- Remote operator: [disposable deployment lab](book/tutorial-remote-deployment-lab.md)
- System architect: [canonical architecture blueprint](architecture-blueprint.md)
- Contributor: [project module](modules/project.md)
- Current release: [v2.0.0 release notes](history/v2.0.0-release-notes.md)
- Architecture change protocol: [coordination](architecture-coordination.md)
- Historical delivery record: [Leserpent 2.0 roadmap](leserpent-2-roadmap.md)
- Sequential reading: [book](book/index.md)

## Modules

| Module | Owns |
| --- | --- |
| [Runtime](modules/runtime.md) | architecture, evidence flow, service and machine contracts |
| [GewyLang](modules/gewylang.md) | language, compiler, package authoring and migration |
| [Leselang](modules/leselang.md) | orchestration syntax, HIR, effects and deterministic re-entry |
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
