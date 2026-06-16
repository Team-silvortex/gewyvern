# Reference: HTTPS Surface

Read this page after the generic protocol surface when the runtime path is an
HTTPS connect-style exchange rather than plain HTTP or arbitrary TLS traffic.

Use it for:

- `https` family lookup
- default entry selection for `connect`
- keeping TLS-protected request setup separate from raw TLS client posture

Current canonical entries:

- `connect` as the default entry

Default entry: `connect`

The current line keeps HTTPS as a compact single-slice family:

- establish or observe HTTPS request setup through the connect path
- keep the family hub small until the surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-https-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-https-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
