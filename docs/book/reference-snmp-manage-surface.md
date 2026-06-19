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
- `trap-recv` models the local receiver side of trap ingestion

Protocol package aliases also remain accepted:

- `snmp-engine-sync`
- `snmp_engine_sync`
- `snmp-trap-recv`
- `snmp_trap_recv`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
