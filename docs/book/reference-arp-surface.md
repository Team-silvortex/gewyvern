# Reference: ARP Surface

Read this page after the generic protocol surface when the runtime path looks
like local-link IPv4 neighbor resolution.

Use it for:

- `arp` family lookup
- default entry selection for `request`
- package aliases such as `arp-request`, `arp_request`, `who-has`,
  `arp-reply`, `arp_reply`, and `is-at`
- separating a local who-has probe from a returned is-at answer

Primary subpages:

- [docs/book/reference-arp-request-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-request-surface.md)
- [docs/book/reference-arp-reply-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-reply-surface.md)

Current canonical entries:

- `request` as the default entry
- `reply`

Default entry: `request`

Operator rule:

- use `request` when the question is whether the local host asked who owns an
  IPv4 address on the link
- use `reply` when the useful signal is a returned MAC mapping for that IPv4
  address

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-arp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-surface.md)
3. one exact ARP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
