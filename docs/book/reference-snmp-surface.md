# Reference: SNMP Surface

Read this page after the generic protocol surface when the runtime path looks
like SNMP polling or mutation traffic.

Use it for:

- `snmp` family lookup
- bulk retrieval paths
- default entry selection for `get`
- separating read traversal from explicit mutation
- package aliases such as `snmp-bulk`, `snmp_bulk`, `snmp-get-next`, `snmp_get_next`, `snmp-set`, and `snmp_set`
- notification-oriented traffic such as `snmp-trap`, `snmp_trap`, `snmp-trap-recv`, and `snmp_trap_recv`
- security-oriented SNMPv3 traffic such as `snmp-v3-auth`, `snmp_v3_auth`, `snmp-v3-priv`, and `snmp_v3_priv`
- management-oriented traffic such as `snmp-engine-sync`, `snmp_engine_sync`, `snmp-trap-recv`, and `snmp_trap_recv`
- result-oriented traffic such as `snmp-report`, `snmp_report`, `snmp-unauthorized`, and `snmp_unauthorized`

Current registry aliases also include:

- `snmp-bulk`, `snmp_bulk`
- `snmp-engine-sync`, `snmp_engine_sync`
- `snmp-get-next`, `snmp_get_next`
- `snmp-report`, `snmp_report`
- `snmp-set`, `snmp_set`
- `snmp-trap`, `snmp_trap`
- `snmp-trap-recv`, `snmp_trap_recv`
- `snmp-unauthorized`, `snmp_unauthorized`
- `snmp-v3-auth`, `snmp_v3_auth`
- `snmp-v3-priv`, `snmp_v3_priv`
- diagnosis semantics for SNMP timeout, denial, and explicit report outcomes

Quick role map:

- `get`, `get-next`, `bulk`
  read-oriented request and reply paths
- `set`
  mutation-oriented request and reply path
- `trap`
  one-way outbound notification, usually toward port `162`
- `inform`
  notification-style path that still expects an explicit SNMP response
- `trap-recv`
  local inbound trap listener path, usually on port `162`
- `v3-auth`, `v3-priv`
  SNMPv3 security-oriented request and reply paths

Primary subpages:

- [docs/book/reference-snmp-role-matrix.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-role-matrix.md)
- [docs/book/reference-snmp-read-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-read-surface.md)
- [docs/book/reference-snmp-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-set-surface.md)
- [docs/book/reference-snmp-notify-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-notify-surface.md)
- [docs/book/reference-snmp-security-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-security-surface.md)
- [docs/book/reference-snmp-manage-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-manage-surface.md)
- [docs/book/reference-snmp-result-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-result-surface.md)
- [docs/book/reference-snmp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-failure-semantics.md)
- [docs/book/reference-management-udp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-failure-semantics.md)

Current canonical entries:

- `bulk`
- `get` as the default entry
- `get-next`
- `inform`
- `engine-sync`
- `report`
- `set`
- `trap`
- `trap-recv`
- `unauthorized`
- `v3-auth`
- `v3-priv`

Default entry: `get`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
3. [docs/book/reference-snmp-role-matrix.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-role-matrix.md)
4. one exact SNMP subpage
5. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
