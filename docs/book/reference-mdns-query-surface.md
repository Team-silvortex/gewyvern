# Reference: mDNS Query Surface

This shelf covers active local-link multicast name lookup.

Read this alongside:

- [docs/book/reference-mdns-surface.md](docs/book/reference-mdns-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `query`
- label: `Query`
- entries: `query`

## Entry

### `query`

Use `query` when the important first question is:

- “did this host send a multicast name question?”
- “did any local responder answer?”
- “is the name-resolution issue local-link discovery rather than unicast DNS?”

The runtime phases are:

- `send_query`
- `receive_response`

The typical signal is mDNS query flags `0x0000`, followed by a response with
flags `0x8400`.

