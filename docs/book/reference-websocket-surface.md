# Reference: WebSocket Surface

WebSocket support gives gewyvern a first-class view of long-lived browser, API gateway, and service-channel traffic without pretending the payload is already application-semantic.

Default entry: `upgrade`

Protocol aliases: `ws`, `websocket-upgrade`, `websocket_upgrade`, `ws-upgrade`, `ws_upgrade`, `websocket-frame`, `websocket_frame`, `ws-frame`, `ws_frame`, `websocket-close`, `websocket_close`, `ws-close`, `ws_close`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`upgrade`](docs/book/reference-websocket-upgrade-surface.md) tracks the HTTP Upgrade handshake and the `101 Switching Protocols` response.
- [`frame`](docs/book/reference-websocket-frame-surface.md) tracks text and binary data-frame opcodes after the session is established.
- [`close`](docs/book/reference-websocket-close-surface.md) tracks close-control frames as a session termination signal.

## Operator Use

Start with `upgrade` when a client cannot establish the channel. Use `frame` when the channel opens but appears idle or asymmetric. Use `close` when the interesting event is premature teardown.

## Limits

This surface is frame-aware, not payload-schema-aware. It does not decode JSON messages, Socket.IO envelopes, or application-specific subprotocols yet.
