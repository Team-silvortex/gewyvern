# Reference: mDNS Probe Surface

This shelf covers mDNS probing before a host claims or advertises a local name.

Read this alongside:

- [docs/book/reference-mdns-surface.md](docs/book/reference-mdns-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `probe`
- label: `Probe`
- entries: `probe`

## Entry

### `probe`

Accepted entry aliases include `claim`, `conflict-check`, `mdns-probe`, and
`mdns_probe`.

Use `probe` when the important first question is:

- “is this host checking for local name conflicts before claiming a name?”
- “did a probe leave without a useful local-link answer?”
- “should name-conflict handling be inspected before blaming DNS resolution?”

The runtime phase is:

- `send_probe`

The typical signal is a query-shaped probe using flags `0x0000`.

