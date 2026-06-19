# Reference: SNMP Notify Surface

Read this page when the canonical entry is `trap` or `inform`.

Canonical entries:

- `trap`
- `inform`

Current accepted aliases:

- `trap`: `notify`, `alert`
- `inform`: `ack-notify`, `confirm-notify`

`trap` stays one-way notification traffic.

`inform` keeps the same notification posture, but expects an explicit SNMP
response and is therefore better for paths where acknowledgement matters.

Protocol package aliases that remain accepted:

- `snmp-trap`
- `snmp_trap`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
