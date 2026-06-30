# Reference: HTTP/3 Server Close Surface

Use this page when the HTTP/3 server path matters mainly because the local
server emitted a close after request and response handling had already started.

## Covered Entries

### `server-close`

- Protocol:
  `http3`
- Aliases:
  `server-close`, `server_close`, `h3-server-close`, `h3_server_close`,
  `http3-server-close`, `http3_server_close`, `response-close`,
  `response_close`
- Default entry:
  no

## Operational Shape

The current `server-close` flow models:

1. bind the server process
2. receive a QUIC Initial packet
3. receive Initial-stage CRYPTO
4. send a QUIC Handshake packet
5. send Handshake-stage CRYPTO
6. receive a request stream
7. send a response stream
8. send `CONNECTION_CLOSE`

This is the narrowest HTTP/3 page to use when the server side did real work,
returned bytes, and then actively terminated the QUIC session instead of
leaving it open for reuse.

## Failure Semantics

- `category = failure-path`
- Typical signal:
  `CONNECTION_CLOSE`
- Primary failure mode:
  `local_closed`

For the broader family map, see
[docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `h3-server-close`
- `h3_server_close`
- `http3-server-close`
- `http3_server_close`
- `response-close`
- `response_close`
- `server-close`
- `server_close`

<!-- gewyvern:entry-aliases:end -->
