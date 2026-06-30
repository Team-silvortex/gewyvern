# Reference: L2TP Tunnel Surface

This shelf groups L2TP entries that describe control-plane tunnel posture and
session traffic.

Read this alongside:

- [docs/book/reference-l2tp-surface.md](docs/book/reference-l2tp-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `tunnel`
- label: `Tunnel`
- entries: `control`, `session`

## Entries

### `control`

Use `control` when the important first question is:

- “is L2TP tunnel negotiation visible?”
- “which direction carries tunnel control messages?”
- “does L2TP appear before or without the expected protected transport?”

The runtime phases are:

- `send_control_message`
- `receive_control_message`

### `session`

Use `session` when the important first question is:

- “is the L2TP data session active?”
- “are session packets directional or missing on one side?”
- “should inner payload diagnosis wait until tunnel posture is stable?”

The runtime phases are:

- `send_session_packet`
- `receive_session_packet`

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `l2tp-control`
- `l2tp-data`
- `l2tp-session`
- `l2tp-tunnel`
- `l2tp_control`
- `l2tp_data`
- `l2tp_session`
- `l2tp_tunnel`

<!-- gewyvern:entry-aliases:end -->
