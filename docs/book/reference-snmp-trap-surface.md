# Reference: SNMP Trap Surface

Read this page when the canonical entry is `trap`.

Canonical entry:

- `trap`

Current accepted aliases:

- `notify`
- `alert`

Protocol package aliases also remain accepted:

- `snmp-trap`
- `snmp_trap`

This slice covers one-way SNMP notification traffic rather than request and
response polling loops.

Operational posture:

- `trap` is a one-way notification send path
- it normally targets the SNMP trap receiver port `162`
- it does not wait for a protocol reply the way `inform` does

If the operator question is about trap reception on the local side rather than
trap emission, switch to:

- [docs/book/reference-snmp-manage-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-manage-surface.md)
- specifically the `trap-recv` entry on that page

Return to the family hub:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
