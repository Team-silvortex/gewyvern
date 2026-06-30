# Reference: Hysteria2 Surface

Read this page after the generic protocol surface when the runtime path looks
like HY2 authentication or relay traffic rather than a generic UDP or TCP
exchange.

Use it for:

- `hy2` family lookup
- default entry selection for `auth`
- relay-oriented traffic such as `udp` and `tcp`
- family aliases such as `hy2-auth`, `hy2-relay`, `hy2-stream`,
  `hy2-tcp`, `hy2-udp`, `hysteria2-auth`, and `hysteria2`

Family aliases currently accepted by the registry:

- `hy2-auth`
- `hy2-relay`
- `hy2-stream`
- `hy2-tcp`
- `hy2-udp`
- `hysteria2`
- `hysteria2-auth`
- `hysteria2-tcp`
- `hysteria2-udp`

Primary subpages:

- [docs/book/reference-hy2-auth-surface.md](docs/book/reference-hy2-auth-surface.md)
- [docs/book/reference-hy2-relay-surface.md](docs/book/reference-hy2-relay-surface.md)
- [docs/book/reference-hy2-close-surface.md](docs/book/reference-hy2-close-surface.md)
- [docs/book/reference-hy2-tcp-close-surface.md](docs/book/reference-hy2-tcp-close-surface.md)
- [docs/book/reference-hy2-udp-close-surface.md](docs/book/reference-hy2-udp-close-surface.md)

Current canonical entries:

- `auth` as the default entry
- `udp`
- `tcp`
- `close`
- `tcp-close`
- `udp-close`

Default entry: `auth`

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-hy2-surface.md](docs/book/reference-hy2-surface.md)
3. one exact HY2 subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
