# Reference: WebSocket Close Surface

The `websocket/close` entry treats a close-control frame as the primary session termination signal.

Family hub: [WebSocket surface](docs/book/reference-websocket-surface.md)

Canonical entry: `close`

## Debugging Focus

- Local close frame.
- Remote close frame.
- Process and route context around teardown.

## Typical Question

Use this entry when the channel opens but immediately disappears, especially behind proxies or gateways.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `close-frame`
- `shutdown`
- `teardown`
- `websocket-close`
- `websocket_close`
- `ws-close`
- `ws_close`

<!-- gewyvern:entry-aliases:end -->
