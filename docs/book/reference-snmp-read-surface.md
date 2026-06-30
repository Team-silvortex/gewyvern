# Reference: SNMP Read Surface

Read this page when the path is polling or walking rather than mutating.

Canonical entries covered here:

- `bulk`
- `get`
- `get-next`

Current accepted aliases:

- `bulk-walk`
- `query`
- `read`
- `table-read`
- `walk`
- `next`

Operational split:

- `bulk` expands one request across broader table-style retrieval
- `get` reads one requested value set
- `get-next` advances one step through ordered OID traversal

Protocol package aliases also remain accepted:

- `snmp-bulk`
- `snmp_bulk`
- `snmp-get-next`
- `snmp_get_next`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `bulk-walk`
- `next`
- `query`
- `read`
- `snmp-bulk`
- `snmp-get-next`
- `snmp_bulk`
- `snmp_get_next`
- `table-read`
- `walk`

<!-- gewyvern:entry-aliases:end -->
