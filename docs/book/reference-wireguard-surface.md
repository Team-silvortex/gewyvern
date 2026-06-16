# Reference: WireGuard Surface

Read this page after the generic protocol surface when the runtime path is a
WireGuard handshake instead of an arbitrary encrypted UDP payload stream.

Use it for:

- `wireguard` family lookup
- default entry selection for `handshake`

Current canonical entries:

- `handshake` as the default entry

Default entry: `handshake`

The current line keeps WireGuard as a compact single-slice family:

- identify a WireGuard peer handshake path
- keep the family hub tight until there is enough protocol depth to justify narrower subpages

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-wireguard-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-wireguard-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
