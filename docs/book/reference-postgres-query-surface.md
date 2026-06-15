# Reference: PostgreSQL Query Surface

Use this page when you need the current exact lookup surface for PostgreSQL
simple-query flow and broader query-session behavior.

## Canonical Entries

### `query`

Aliases:

- `postgres-query`
- `postgres_query`

Intent:

- operate over a PostgreSQL session
- send a simple query
- receive ready state

### `session`

Aliases:

- `query-session`
- `auth-query`

Intent:

- establish the PostgreSQL socket
- receive auth challenge
- send password
- send a query on the same session
- receive ready state

## Shared Response Shape

Both entries currently share a query-oriented staging model:

1. process binding
2. route resolution
3. socket connect and establish
4. query send
5. ready-for-query receive

`session` extends that narrower query shape with the auth exchange before the
query itself.

## Operator Reading Order

If you are reviewing PostgreSQL query coverage, read it in this order:

1. `connect`
2. `auth`
3. `query`
4. `session`

That sequence keeps transport and auth context in front of the broader
query-session model.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `postgres` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth-query`
- `postgres-query`
- `postgres-session`
- `postgres_query`
- `postgres_session`
- `query-session`

<!-- gewyvern:entry-aliases:end -->
