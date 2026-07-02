# Reference: SSDP Discovery Surface

This shelf covers active SSDP discovery using multicast search traffic.

Read this alongside:

- [docs/book/reference-ssdp-surface.md](docs/book/reference-ssdp-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `discovery`
- label: `Discovery`
- entries: `discovery`

## Entry

### `discovery`

Use `discovery` when the important first question is:

- “did the client send an `M-SEARCH`?”
- “did a device or service return an HTTP-style SSDP response?”
- “is service discovery failing before any device-control protocol starts?”

The runtime phases are:

- `send_search`
- `receive_response`

The typical signal is `M-SEARCH` followed by an HTTP-style response.

