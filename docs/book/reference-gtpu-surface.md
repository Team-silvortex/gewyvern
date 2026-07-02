# Reference: GTP-U Surface

Read this page after the generic protocol surface when the runtime path looks
like GTP-U control or tunnel liveness traffic.

Use it for:

- `gtpu` family lookup
- default entry selection for `echo`
- accepted protocol aliases such as `gtp-u` and `gtp_u`

Current canonical entries:

- `echo` as the default entry

Default entry: `echo`

Current shelf:

- [docs/book/reference-gtpu-liveness-surface.md](docs/book/reference-gtpu-liveness-surface.md)

The current line keeps GTP-U intentionally narrow: the supported path is
liveness, not full subscriber-payload decoding. That still matters for network
debugging because it answers whether the outer user-plane tunnel can exchange
basic control traffic before inner payload interpretation begins.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-gtpu-surface.md](docs/book/reference-gtpu-surface.md)
3. [docs/book/reference-gtpu-liveness-surface.md](docs/book/reference-gtpu-liveness-surface.md)
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
