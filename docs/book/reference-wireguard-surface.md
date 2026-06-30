# Reference: WireGuard Surface

Read this page after the generic protocol surface when the runtime path is a
WireGuard handshake instead of an arbitrary encrypted UDP payload stream.

Use it for:

- `wireguard` family lookup
- default entry selection for `handshake`
- cookie-reply continuation paths such as `cookie-reply` and `wireguard-cookie`
- encrypted payload paths such as `data`, `session`, and `wireguard-data`

Current canonical entries:

- `handshake` as the default entry
- `cookie`
- `transport`

Default entry: `handshake`

## WireGuard Surface Map

### Handshake

- [docs/book/reference-wireguard-handshake-surface.md](docs/book/reference-wireguard-handshake-surface.md)
  Peer initiation and response exchange.

Typical entries:

- `handshake`

### Cookie Reply

- [docs/book/reference-wireguard-cookie-surface.md](docs/book/reference-wireguard-cookie-surface.md)
  Peer anti-abuse continuation branch.

Typical entries:

- `cookie`

### Transport

- [docs/book/reference-wireguard-transport-surface.md](docs/book/reference-wireguard-transport-surface.md)
  Encrypted payload datagrams after setup.

Typical entries:

- `transport`

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-wireguard-surface.md](docs/book/reference-wireguard-surface.md)
3. one exact WireGuard subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
