# Reference: DHCP Surface

Read this page after the generic protocol surface when the runtime path is DHCP
lease negotiation rather than an arbitrary UDP payload exchange.

Use it for:

- `dhcp` family lookup
- default entry selection for `client`
- separating generic client posture from explicit lease negotiation steps
- package aliases such as `dhcp-discover`, `dhcp_discover`, `dhcp-request`, and `dhcp_request`

Primary subpages:

- [docs/book/reference-dhcp-client-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-client-surface.md)
- [docs/book/reference-dhcp-lease-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-lease-surface.md)

Current canonical entries:

- `client` as the default entry
- `discover`
- `request`

Default entry: `client`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-dhcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-surface.md)
3. one exact DHCP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
