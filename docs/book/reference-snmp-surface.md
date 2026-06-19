# Reference: SNMP Surface

Read this page after the generic protocol surface when the runtime path looks
like SNMP polling or mutation traffic.

Use it for:

- `snmp` family lookup
- bulk retrieval paths
- default entry selection for `get`
- separating read traversal from explicit mutation
- package aliases such as `snmp-bulk`, `snmp_bulk`, `snmp-get-next`, `snmp_get_next`, `snmp-set`, and `snmp_set`
- notification-oriented traffic such as `snmp-trap` and `snmp_trap`
- security-oriented SNMPv3 traffic such as `snmp-v3-auth` and `snmp-v3-priv`
- management-oriented traffic such as `snmp-engine-sync` and `snmp-trap-recv`

Primary subpages:

- [docs/book/reference-snmp-bulk-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-bulk-surface.md)
- [docs/book/reference-snmp-read-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-read-surface.md)
- [docs/book/reference-snmp-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-set-surface.md)
- [docs/book/reference-snmp-notify-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-notify-surface.md)
- [docs/book/reference-snmp-security-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-security-surface.md)
- [docs/book/reference-snmp-manage-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-manage-surface.md)

Current canonical entries:

- `bulk`
- `get` as the default entry
- `get-next`
- `inform`
- `engine-sync`
- `set`
- `trap`
- `trap-recv`
- `v3-auth`
- `v3-priv`

Default entry: `get`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
3. one exact SNMP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
