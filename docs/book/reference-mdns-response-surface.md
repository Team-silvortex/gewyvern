# Reference: mDNS Response Surface

This shelf covers mDNS answer and announcement traffic.

Read this alongside:

- [docs/book/reference-mdns-surface.md](docs/book/reference-mdns-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `response`
- label: `Response`
- entries: `response`

## Entry

### `response`

Accepted entry aliases include `answer`, `announcement`, `mdns-response`, and
`mdns_response`.

Use `response` when the important first question is:

- “did a responder publish an answer or announcement?”
- “is the local name present on the wire but not consumed by the application?”
- “is this host advertising stale or conflicting local-link data?”

The runtime phase is:

- `receive_answer`

The typical signal is an mDNS response with flags `0x8400`.


<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `announcement`
- `answer`
- `mdns-response`
- `mdns_response`

<!-- gewyvern:entry-aliases:end -->
