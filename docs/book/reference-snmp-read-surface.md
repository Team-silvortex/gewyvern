# Reference: SNMP Read Surface

Read this page when the path is polling or walking rather than mutating.

Canonical entries covered here:

- `get`
- `get-next`

Current accepted aliases:

- `query`
- `read`
- `walk`
- `next`

Operational split:

- `get` reads one requested value set
- `get-next` advances one step through ordered OID traversal

Protocol package aliases also remain accepted:

- `snmp-get-next`
- `snmp_get_next`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
