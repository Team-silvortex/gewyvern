# Reference: NTP Surface

Read this page after the generic protocol surface when the runtime path looks
like network time lookup or clock discipline rather than an arbitrary UDP
exchange.

Use it for:

- `ntp` family lookup
- default entry selection for `client`
- separating simple client posture from explicit time probe and sync paths
- package aliases such as `ntp-query`, `ntp_query`, `ntp-sync`, and `ntp_sync`
- shared management-UDP timeout and reply-missing semantics

Primary subpages:

- [docs/book/reference-ntp-client-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-client-surface.md)
- [docs/book/reference-ntp-time-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-time-surface.md)
- [docs/book/reference-management-udp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-failure-semantics.md)
- [docs/book/reference-ntp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-failure-semantics.md)

Current canonical entries:

- `client` as the default entry
- `query`
- `sync`

Default entry: `client`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ntp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-surface.md)
3. one exact NTP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
