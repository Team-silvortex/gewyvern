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
- Source ownership and extraction boundaries: [repository map](monorepo-stack.md)
- Contributor: [project module](modules/project.md)
- Current release: [v2.7.x maintenance notes](history/v2.7.x.md)
- Architecture change protocol: [coordination](architecture-coordination.md)
- Historical delivery record: [Leserpent 2.0 roadmap](leserpent-2-roadmap.md)
- Sequential reading: [book](book/index.md)

## Current Work

- Source ownership and retained compatibility: [repository map](monorepo-stack.md)
- Leselang dependency pin and product-side cleanup: [repository handoff](leselang-repository-handoff.md#product-cleanup-closeout)
- Gewyvern capability testing: [field-validation handoff](field-validation.md#post-split-handoff)
- Product CLI and service capture: [live Linux UDP capture](live-udp-capture.md)
- Installed TLS capture and readiness: [installed UDP service](installed-udp-service.md)
- Linux supervisor restart and bounded recovery: [systemd UDP recovery](systemd-udp-recovery.md)
- Real DNS target through the formal CLI: [production DNS field check](production-dns-field.md)
- Real SNMP/NTP/STUN targets through the formal CLI: [management UDP field checks](production-management-udp-field.md)
- Formal collector packet/socket limits: [UDP boundary checks](production-udp-boundaries.md)
- Formal collector across namespaces, MTU and network faults: [UDP network checks](production-udp-network.md)
- Hook visibility versus socket/application delivery: [UDP receive pressure](production-udp-receive-pressure.md)
- Bounded UDP L4 GSO/GRO normalization, provenance and loss accounting: [offload boundary](production-udp-offload.md)
- Whole offload groups across windows, multi-CPU loss/recovery, expiry, history rollover and SIGKILL: [service reliability](production-udp-offload-service.md)
- Live progress and evidence: [status entry](../STATUS.md)
- Local cache retention and cleanup: [workspace hygiene](development.md#workspace-hygiene)

These are routes to the owning records, not a second progress checklist.

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
