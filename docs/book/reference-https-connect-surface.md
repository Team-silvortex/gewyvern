# Reference: HTTPS Connect Surface

This shelf covers the HTTPS setup path before the debugger pivots into deeper
TLS interpretation.

Read this alongside:

- [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
- [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `connect`
- label: `Connect`
- entries: `connect`

## Entry

### `connect`

Use `connect` when the important first question is:

- “did this process reach the remote HTTPS service socket?”
- “is the failure before TLS handshake details become useful?”
- “should the next reading step pivot into `tls client`?”

The runtime phases are:

- `bind`
- `connect`
- `resolve_upstream`

The typical signal is a TCP connection to an HTTPS service endpoint followed by
TLS client posture in the companion surface.

## Boundary

This surface is not a full TLS decoder. It is the HTTPS-side checkpoint that
keeps request setup, socket reachability, and TLS companion navigation aligned.

