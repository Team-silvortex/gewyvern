# Reference: PPTP Tunnel Surface

This shelf groups PPTP entries that describe the TCP control channel and GRE
data movement.

Read this alongside:

- [docs/book/reference-pptp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pptp-surface.md)
- [docs/book/reference-gre-tunnel-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gre-tunnel-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `tunnel`
- label: `Tunnel`
- entries: `control`, `data`

## Entries

### `control`

Use `control` when the important first question is:

- “did the TCP `1723` control channel appear?”
- “does the PPTP magic cookie show up in both directions?”
- “did tunnel control succeed before GRE data should appear?”

The runtime phases are:

- `connect_control_channel`
- `send_control_message`
- `receive_control_message`

### `data`

Use `data` when the important first question is:

- “is GRE data moving after PPTP control setup?”
- “is GRE blocked in one direction?”
- “should tunnel data be debugged before inner payload semantics?”

The runtime phases are:

- `send_gre_data`
- `receive_gre_data`
