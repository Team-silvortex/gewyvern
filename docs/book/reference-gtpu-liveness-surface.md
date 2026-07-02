# Reference: GTP-U Liveness Surface

This shelf covers the compact GTP-U liveness path used by the current
protocol standard library.

Read this alongside:

- [docs/book/reference-gtpu-surface.md](docs/book/reference-gtpu-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Shelf

- key: `liveness`
- label: `Liveness`
- entries: `echo`

## Entry

### `echo`

Accepted family aliases that reach this shelf include `gtp-u` and `gtp_u`.

Use `echo` when the important first question is:

- “can this endpoint exchange GTP-U liveness traffic at all?”
- “did an Echo Request leave but no Echo Response return?”
- “is the tunnel frame alive before blaming the encapsulated user payload?”

The runtime phases are:

- `send_echo_request`
- `receive_echo_response`

The typical wire signal is:

- GTP-U Echo Request `0x01`
- GTP-U Echo Response `0x02`

## Boundary

This surface does not claim to decode subscriber payloads or full GTP session
state. It gives the debugger a stable outer liveness checkpoint so TEID,
payload, and service-chain work can be added later without losing the basic
tunnel-reachability question.
