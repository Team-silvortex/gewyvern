# Reference: NATS Error Surface

Use this page when a NATS server is reachable but rejects a command or reports
that the connection is not in a valid protocol state.

For the broader family map, see
[docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md).

## Canonical Entries

### `error`

Aliases:

- `nats-error`
- `nats_error`
- `nats-server-error`
- `nats_server_error`
- `server-error`
- `server_error`
- `protocol-error`
- `protocol_error`

Intent:

- resolve the server route
- observe server `-ERR`
- mark the path as a failure signal rather than normal pub/sub activity

## Operator Notes

NATS `-ERR` responses often mean parser rejection, authorization failure, or an
invalid command sequence. Pair this entry with `connect`, `pub`, or `sub` when
you need to know which command path triggered the rejection.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `nats-error`
- `nats-server-error`
- `nats_error`
- `nats_server_error`
- `protocol-error`
- `protocol_error`
- `server-error`
- `server_error`

<!-- gewyvern:entry-aliases:end -->
