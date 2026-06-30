# Reference: SNMP Manage Surface

Read this page when the path is about SNMP control-plane maintenance rather than
ordinary polling, mutation, or notification send.

Canonical entries covered here:

- `engine-sync`
- `trap-recv`

Current accepted aliases:

- `engine-discovery`
- `report-sync`
- `listen-trap`
- `trap-listener`

Operational split:

- `engine-sync` models SNMPv3 engine discovery and report-driven synchronization
- `trap-recv` models the local receiver side of trap ingestion, usually on port `162`

Protocol package aliases also remain accepted:

- `snmp-engine-sync`
- `snmp_engine_sync`
- `snmp-trap-recv`
- `snmp_trap_recv`

Use `trap-recv` when:

- the runtime is acting as a local trap listener
- the interesting event is inbound trap reception
- the operator wants to distinguish local trap ingestion from outbound trap send

Use `trap` instead when the runtime is the sender of a one-way notification.
Use `inform` when a notification-style exchange should also receive an explicit
SNMP response.

Return to the family hub:

- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `engine-discovery`
- `listen-trap`
- `report-sync`
- `snmp-engine-sync`
- `snmp-trap-recv`
- `snmp_engine_sync`
- `snmp_trap_recv`
- `trap-listener`

<!-- gewyvern:entry-aliases:end -->
