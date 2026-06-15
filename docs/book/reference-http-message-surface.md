# Reference: HTTP Message Surface

Use this page when you need the current exact lookup surface for direct HTTP
request and response traffic.

## Canonical Entries

### `request`

Aliases:

- `client`
- `http-client`
- `http-request`
- `http_client`
- `http_request`

Intent:

- observe an outbound HTTP client request
- observe the corresponding inbound response

Coarse response shape:

- process binding
- route resolution
- remote socket connect and establish
- request send
- response receive

### `response`

Aliases:

- `server`

Intent:

- observe an HTTP server-side accepted connection
- receive a request
- send a response

Coarse response shape:

- process binding
- local socket accept and establish
- request receive
- response send

## Operator Reading Order

Read the current HTTP message family in this order:

1. `request`
2. `response`

That sequence keeps the client-side path in front of the corresponding
server-side path.

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `http`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `client`
- `http-client`
- `http-request`
- `http-server`
- `http_client`
- `http_request`
- `http_server`
- `server`

<!-- gewyvern:entry-aliases:end -->
