# Reference: mDNS Surface

Read this page after the generic protocol surface when the runtime path looks
like local-link multicast name discovery rather than a generic UDP datagram.

Use it for:

- `mdns` family lookup
- default entry selection for `query`
- keeping multicast discovery lookups separate from unicast DNS pages

Current canonical entries:

- `query` as the default entry

Default entry: `query`

The current line keeps mDNS as a compact single-slice family:

- query local multicast responders
- keep the family hub small until the surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-mdns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mdns-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
