# Reference: Hysteria2 UDP Close Surface

Use this page when the Hysteria2 session successfully authenticated, began UDP
relay work, and then collapsed through a peer-side connection close.

## Covered Entries

### `udp-close`

- Protocol:
  `hy2`
- Aliases:
  `udp-close`, `udp_close`, `hy2-udp-close`, `hy2_udp_close`,
  `hysteria2-udp-close`, `hysteria2_udp_close`, `datagram-close`,
  `datagram_close`
- Default entry:
  no

## Operational Shape

The current `udp-close` flow models:

1. bind the process and resolve the upstream route
2. send a QUIC Initial packet
3. send Initial-stage CRYPTO
4. receive a QUIC Handshake packet
5. receive Handshake-stage CRYPTO
6. send auth request stream
7. receive auth-ok stream
8. send UDP relay datagram
9. receive UDP relay datagram
10. receive `CONNECTION_CLOSE`

This is the narrowest HY2 page to use when authentication was already good and
the relay had become specifically UDP-shaped before the peer terminated the
session.

## Failure Semantics

- `category = failure-path`
- Typical signal:
  `CONNECTION_CLOSE`
- Primary failure mode:
  `peer_closed`

For the broader family map, see
[docs/book/reference-hy2-surface.md](docs/book/reference-hy2-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `datagram-close`
- `datagram_close`
- `hy2-udp-close`
- `hy2_udp_close`
- `hysteria2-udp-close`
- `hysteria2_udp_close`
- `udp-close`
- `udp_close`

<!-- gewyvern:entry-aliases:end -->
