# Reference: PostgreSQL Error Surface

Use this page when you need the current exact lookup surface for PostgreSQL
query-error flow.

## Canonical Entries

### `error`

Aliases:

- `postgres-error`
- `postgres_error`

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

## Machine-Readable Surface Semantics

The `protocol_surface("postgres", "error")` contract now publishes
`entry_semantics` so operators and higher-level tooling can treat explicit
server error frames as a structured failure surface.

Current query-error semantics:

- `category = failure-path`
- `operator_focus = database error frame during PostgreSQL query result handling`
- `typical_signal = ErrorResponse`
- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_error`
- `primary_failure_basis = direct_protocol_signal`

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

For the cross-database comparison table, see
[docs/book/reference-database-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-database-failure-semantics.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `postgres-error`
- `postgres_error`

<!-- gewyvern:entry-aliases:end -->
