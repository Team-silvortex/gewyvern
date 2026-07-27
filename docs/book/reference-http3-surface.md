# Reference: HTTP/3 Protocol Surface

Use this page when you want the HTTP/3 portion of the built-in protocol shelf
as stable lookup material instead of a tutorial.

This shelf groups the current HTTP/3 coverage into four narrower operator-facing
surfaces:

- client request progression
- server response progression
- explicit connection close diagnosis
- server-side termination after response work has started

## What This Shelf Covers

The current built-in HTTP/3 family models application behavior on top of QUIC:

- progress from QUIC Initial into Handshake and CRYPTO exchange
- observe request stream activity
- observe response stream activity
- observe connection close
- observe locally emitted close after server response work

Across the subpages, the lookup contract focuses on:

- canonical entry names
- client versus server posture
- operator reading order
- validation and lowering posture
- companion jumps into `quic initial` when transport setup is more informative
  than request semantics

## Family Aliases

The current registry also accepts these family-level spellings for HTTP/3 entry
selection:

- `h3-request`
- `h3-server`
- `h3_request`
- `h3_server`
- `http3-server-response`

Default entry: `request`

## HTTP/3 Surface Map

### Request

- [docs/book/reference-http3-request-surface.md](docs/book/reference-http3-request-surface.md)
  Client-side request progression over QUIC.

Typical entries:

- `request`

### Server

- [docs/book/reference-http3-server-surface.md](docs/book/reference-http3-server-surface.md)
  Server-side response progression over QUIC.

Typical entries:

- `server`

### Connection Close

- [docs/book/reference-http3-close-surface.md](docs/book/reference-http3-close-surface.md)
  Client-side request path that terminates through peer connection close.

Typical entries:

- `close`

### Server Close

- [docs/book/reference-http3-server-close-surface.md](docs/book/reference-http3-server-close-surface.md)
  Server-side response path that ends with a locally emitted close.

Typical entries:

- `server-close`

## Reading Order

If you are validating current HTTP/3 support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
3. the request or server subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

The machine-facing `reading_companions` field uses the same jump contract; see:

- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)

## Next Useful Checks

- For one concrete runtime-facing walkthrough:
  [docs/architecture-walkthrough-http-request.md](docs/architecture-walkthrough-http-request.md)
- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Stability Note

This page is the lookup hub for the HTTP/3 family in the current `1.10.x` line.
New HTTP/3 role-specific branches should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
