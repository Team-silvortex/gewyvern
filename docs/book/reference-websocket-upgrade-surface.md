# Reference: WebSocket Upgrade Surface

The `websocket/upgrade` entry watches the HTTP handshake that turns a request/response connection into a WebSocket session.

Family hub: [WebSocket surface](docs/book/reference-websocket-surface.md)

Canonical entry: `upgrade`

## Debugging Focus

- Client sent an HTTP `GET` upgrade candidate.
- Peer returned `HTTP/1.1 101`.
- Process, route, and TCP lineage are available for correlation.

## Typical Question

Use this entry when the browser or client says the socket never opened, or when a proxy/load balancer may be stripping upgrade semantics.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `handshake`
- `http-upgrade`
- `switching-protocols`
- `websocket-upgrade`
- `websocket_upgrade`
- `ws`
- `ws-upgrade`
- `ws_upgrade`

<!-- gewyvern:entry-aliases:end -->
