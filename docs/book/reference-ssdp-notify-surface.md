# Reference: SSDP Notify Surface

This shelf covers passive SSDP advertisement traffic.

Read this alongside:

- [docs/book/reference-ssdp-surface.md](docs/book/reference-ssdp-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `notify`
- label: `Notify`
- entries: `notify`

## Entry

### `notify`

Accepted entry aliases include `advertise`, `alive`, `byebye`, `ssdp-notify`,
and `ssdp_notify`.

Use `notify` when the important first question is:

- “is a device announcing availability or removal?”
- “is the service visible passively even when active discovery fails?”
- “did a byebye-style advertisement explain a disappearing device?”

The runtime phase is:

- `send_notify`

The typical signal is a `NOTIFY` datagram.


<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `advertise`
- `alive`
- `byebye`
- `ssdp-notify`
- `ssdp_notify`

<!-- gewyvern:entry-aliases:end -->
