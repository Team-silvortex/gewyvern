# Reference: Hysteria2 Close Surface

Read this page when the HY2 path advanced into authenticated session posture
but then collapsed through a peer-side connection close.

## Covered Entries

### `close`

- Protocol:
  `hy2`
- Aliases:
  `terminate`, `session-close`, `session_close`, `hy2-close`, `hy2_close`,
  `hysteria2-close`, `hysteria2_close`
- Default entry:
  no

## Operational Shape

The current `close` flow models:

1. bind the process and resolve the upstream route
2. send a QUIC Initial packet
3. send Initial-stage CRYPTO
4. receive a QUIC Handshake packet
5. receive Handshake-stage CRYPTO
6. send an auth request stream
7. receive an auth-ok stream
8. receive `CONNECTION_CLOSE`

This is the narrowest HY2 page to use when authentication appeared to succeed
but the secure session was terminated before relay continuity could be trusted.

## Failure Semantics

- `category = failure-path`
- Typical signal:
  `CONNECTION_CLOSE`
- Primary failure mode:
  `peer_closed`

Return to the family hub:

- [docs/book/reference-hy2-surface.md](docs/book/reference-hy2-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `hy2-close`
- `hy2_close`
- `hysteria2-close`
- `hysteria2_close`
- `session-close`
- `session_close`
- `terminate`

<!-- gewyvern:entry-aliases:end -->
