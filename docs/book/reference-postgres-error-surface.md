# Reference: PostgreSQL Error Surface

Use this page when you need the current exact lookup surface for PostgreSQL
query-error flow.

## Canonical Entries

### `error`

Aliases:

- none registered today

Intent:

- operate over a PostgreSQL session
- send a query
- receive server error frame

Coarse response shape:

- process binding
- route resolution
- socket connect and establish
- query send
- error receive (`E`)

## Operator Reading Order

Read the current PostgreSQL error family in this order:

1. `connect`
2. `auth`
3. `query`
4. `error`

That sequence keeps the success-oriented session path visible before the error
branch.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `postgres` family resolution
- canonical entry normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md).
