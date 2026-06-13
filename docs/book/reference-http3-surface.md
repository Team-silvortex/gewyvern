# Reference: HTTP/3 Protocol Surface

Use this page when you want the HTTP/3 portion of the built-in protocol shelf
as stable lookup material instead of a tutorial.

This shelf groups the current HTTP/3 coverage into two narrower operator-facing
surfaces:

- client request progression
- server response progression

## What This Shelf Covers

The current built-in HTTP/3 family models application behavior on top of QUIC:

- progress from QUIC Initial into Handshake and CRYPTO exchange
- observe request stream activity
- observe response stream activity
- observe connection close

Across the subpages, the lookup contract focuses on:

- canonical entry names
- client versus server posture
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for HTTP/3 entry
selection:

- `h3-request`
- `h3-server`
- `h3_request`
- `h3_server`
- `http3-server-response`

## HTTP/3 Surface Map

### Request

- [docs/book/reference-http3-request-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-request-surface.md)
  Client-side request progression over QUIC.

Typical entries:

- `request`

### Server

- [docs/book/reference-http3-server-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-server-surface.md)
  Server-side response progression over QUIC.

Typical entries:

- `server`

## Reading Order

If you are validating current HTTP/3 support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
3. the request or server subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current HTTP/3 family in the `1.4.x` line.
New HTTP/3 role-specific branches should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
