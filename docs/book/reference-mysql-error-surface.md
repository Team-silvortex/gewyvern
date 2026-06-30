# Reference: MySQL Error Surface

Use this page when you need the current exact lookup surface for MySQL
query-error flow.

## Canonical Entries

### `error`

Aliases:

- none registered at the entry level today

Protocol aliases:

- `mysql-error`
- `mysql_error`

Intent:

- operate over a MySQL session
- send a query
- receive an error packet

Coarse response shape:

- process binding
- route resolution
- socket connect and establish
- query send
- error receive

## Machine-Readable Surface Semantics

The `protocol_surface("mysql", "error")` contract now publishes
`entry_semantics` so operators and higher-level tooling can treat explicit
database error packets as a structured failure surface.

Current query-error semantics:

- `category = failure-path`
- `operator_focus = database error response during MySQL query result handling`
- `typical_signal = ERR`
- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_error`
- `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

Read the current MySQL error family in this order:

1. `connect`
2. `query`
3. `error`

That sequence keeps the success-oriented query path visible before the error
branch.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `mysql` family resolution
- canonical entry normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md).

For the cross-database comparison table, see
[docs/book/reference-database-failure-semantics.md](docs/book/reference-database-failure-semantics.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `mysql-error`
- `mysql_error`

<!-- gewyvern:entry-aliases:end -->
