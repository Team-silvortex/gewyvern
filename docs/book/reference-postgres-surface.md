# Reference: PostgreSQL Protocol Surface

Use this page when you want the PostgreSQL portion of the built-in protocol
shelf as stable lookup material instead of a tutorial.

This shelf groups the current PostgreSQL coverage into three narrower
operator-facing surfaces:

- connect and auth
- query and query-session flow
- query error flow

## What This Shelf Covers

The current built-in PostgreSQL family models a compact database conversation:

- establish the PostgreSQL socket
- optionally observe server auth challenge and password send
- send a simple query
- receive readiness or error

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## PostgreSQL Surface Map

### Connect And Auth

- [docs/book/reference-postgres-connect-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-connect-surface.md)
  Socket establishment and authentication-ready flow.

Typical entries:

- `connect`
- `auth`

### Query And Session

- [docs/book/reference-postgres-query-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-query-surface.md)
  Simple query flow and broader query-session path.

Typical entries:

- `query`
- `session`

### Error

- [docs/book/reference-postgres-error-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-error-surface.md)
  Query-error flow after PostgreSQL session establishment.

Typical entries:

- `error`

## Reading Order

If you are validating current PostgreSQL support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
3. one narrower PostgreSQL subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current PostgreSQL family in the `1.4.x`
line. New PostgreSQL command families should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
