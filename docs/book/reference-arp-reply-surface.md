# Reference: ARP Reply Surface

Read this page when the selected ARP entry is `reply`.

This surface is for local-link address resolution answers:

- Ethernet ARP hardware type `1`
- protocol type IPv4 `0x0800`
- ARP opcode `2`, commonly read as is-at

Canonical entry:

- `reply`

Entry aliases:

- `is-at`
- `neighbor-reply`
- `mac-resolution`

Package aliases:

- `arp-reply`
- `arp_reply`
- `is-at`

Operator interpretation:

- `receive_is_at` means the local runtime observed an ARP answer returning on
  the link
- repeated requests without replies usually points below the application layer:
  VLAN, bridge, neighbor cache, proxy ARP, or L2 reachability first

Read this alongside:

- [docs/book/reference-arp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `arp-reply`
- `arp_reply`
- `is-at`
- `mac-resolution`
- `neighbor-reply`

<!-- gewyvern:entry-aliases:end -->
