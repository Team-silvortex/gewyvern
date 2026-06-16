# Reference: SSDP Surface

Read this page after the generic protocol surface when the runtime path looks
like local service discovery traffic rather than an arbitrary UDP exchange.

Use it for:

- `ssdp` family lookup
- default entry selection for `discovery`
- keeping device and service advertisement discovery distinct from HTTP control

Current canonical entries:

- `discovery` as the default entry

Default entry: `discovery`

The current line keeps SSDP as a compact single-slice family:

- discover local devices or services
- keep the family hub small until the surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ssdp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssdp-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
