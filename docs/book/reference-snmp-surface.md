# Reference: SNMP Surface

Read this page after the generic protocol surface when the runtime path looks
like SNMP polling or mutation traffic.

Use it for:

- `snmp` family lookup
- default entry selection for `get`
- separating read traversal from explicit mutation
- package aliases such as `snmp-get-next`, `snmp_get_next`, `snmp-set`, and `snmp_set`

Primary subpages:

- [docs/book/reference-snmp-read-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-read-surface.md)
- [docs/book/reference-snmp-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-set-surface.md)

Current canonical entries:

- `get` as the default entry
- `get-next`
- `set`

Default entry: `get`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
3. one exact SNMP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
