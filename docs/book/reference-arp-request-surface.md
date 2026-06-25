# Reference: ARP Request Surface

Read this page when the selected ARP entry is `request`.

This surface is for local-link address resolution probes:

- Ethernet ARP hardware type `1`
- protocol type IPv4 `0x0800`
- ARP opcode `1`, commonly read as who-has

Canonical entry:

- `request`

Entry aliases:

- `who-has`
- `resolve-ip`
- `neighbor-request`

Package aliases:

- `arp-request`
- `arp_request`
- `who-has`

Operator interpretation:

- `send_who_has` means the local runtime observed an ARP request leaving the
  host or interface
- a missing follow-up `reply` entry can mean neighbor resolution, local-link
  reachability, or L2 policy is the actual problem before IP traffic starts

Read this alongside:

- [docs/book/reference-arp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `arp-request`
- `arp_request`
- `neighbor-request`
- `resolve-ip`
- `who-has`

<!-- gewyvern:entry-aliases:end -->
