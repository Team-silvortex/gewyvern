# Reference: TLS Server Surface

Use this page when you need the current exact lookup surface for local TLS
server-handshake posture.

## Covered Entries

### `server`

- Protocol:
  `tls`
- Aliases:
  `acceptor`, `tls-server`, `tls_server`
- Default entry:
  no

## Operational Shape

The current `server` flow models:

1. bind the process
2. accept or advance a local TLS socket
3. receive a client hello
4. send a server hello

This is the narrowest TLS page to use when the operator question is about
server-side handshake posture before application semantics become trustworthy.

## Operator Reading Order

Read this page after the TLS family hub when:

- the local runtime is the accepting side of the TLS relationship
- you want to separate listener posture from client-initiated TLS setup
- you need a server-handshake shelf before IR lowering or overlay reasoning

For the broader family map, see
[docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `acceptor`
- `tls-server`
- `tls_server`

<!-- gewyvern:entry-aliases:end -->
