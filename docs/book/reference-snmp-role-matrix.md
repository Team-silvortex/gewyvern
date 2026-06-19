# Reference: SNMP Role Matrix

Use this page when the operator already knows the traffic is SNMP, but still
needs to answer one structural question before choosing a deeper page:

- is this a read path?
- a write path?
- a notification send path?
- a notification receive path?
- a security-flavored SNMPv3 path?
- or a management/report path?

Read this alongside:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)

## Role Matrix

| role | canonical entries | exchange shape | typical port posture | read next |
| --- | --- | --- | --- | --- |
| read | `get`, `get-next`, `bulk` | request and reply | ordinary SNMP query path, usually `161` | [docs/book/reference-snmp-read-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-read-surface.md) |
| write | `set` | request and reply | ordinary SNMP mutation path, usually `161` | [docs/book/reference-snmp-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-set-surface.md) |
| notify-send | `trap` | one-way notification | outbound trap path, usually toward `162` | [docs/book/reference-snmp-trap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-trap-surface.md) |
| notify-send-with-reply | `inform` | notification plus expected response | notification-style exchange that still expects a reply | [docs/book/reference-snmp-notify-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-notify-surface.md) |
| notify-recv | `trap-recv` | inbound notification receive | local trap listener, usually on `162` | [docs/book/reference-snmp-manage-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-manage-surface.md) |
| security | `v3-auth`, `v3-priv` | request and reply | SNMPv3 authenticated or privacy-protected traffic | [docs/book/reference-snmp-security-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-security-surface.md) |
| management | `engine-sync` | request and report-style reply | SNMPv3 engine discovery and synchronization | [docs/book/reference-snmp-manage-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-manage-surface.md) |
| result / explicit signal | `report`, `unauthorized` | observed terminal result | explicit report-side signal, not plain timeout | [docs/book/reference-snmp-result-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-result-surface.md) |

## Fast Picks

- If a packet is outbound and no reply is required, start at `trap`.
- If a packet is outbound and a reply is expected, start at `inform`.
- If a packet is inbound to a local trap listener, start at `trap-recv`.
- If the operator is dealing with SNMPv3 security posture, start at `v3-auth`
  or `v3-priv`.
- If the question is no longer "which role is this?" but "what failed?",
  switch to:
  [docs/book/reference-snmp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-failure-semantics.md)

Return path:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
