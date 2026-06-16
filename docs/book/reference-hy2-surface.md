# Reference: Hysteria2 Surface

Read this page after the generic protocol surface when the runtime path looks
like HY2 authentication or relay traffic rather than a generic UDP or TCP
exchange.

Use it for:

- `hy2` family lookup
- default entry selection for `auth`
- relay-oriented traffic such as `udp` and `tcp`
- family aliases such as `hy2-auth`, `hysteria2-auth`, and `hysteria2`

Primary subpages:

- [docs/book/reference-hy2-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-hy2-auth-surface.md)
- [docs/book/reference-hy2-relay-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-hy2-relay-surface.md)

Current canonical entries:

- `auth` as the default entry
- `udp`
- `tcp`

Default entry: `auth`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-hy2-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-hy2-surface.md)
3. one exact HY2 subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
