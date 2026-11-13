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
- canonical nonfatal ErrorResponse candidate (`E` with `SERROR`/`VERROR` fields)

The first `E` byte alone cannot distinguish authentication refusal from query
failure. The current TCP packet predicates deliberately use a
conservative canonical English field layout: `auth-denied` requires FATAL and
SQLSTATE class 28; `error` requires the nonfatal ERROR prefix. Both require an
observed zero SQLSTATE terminator, not merely a byte at that offset. Different field order, localized severity text,
split prefixes or fatal query failures can remain unmatched, not mislabeled.
These are packet-local candidates, not a full PostgreSQL decoder.

The real `linux-tcp-capture --data-breadth` fixture independently checks complete
length-framed server messages, SQLSTATE, row values, command completion and
ReadyForQuery. It distinguishes `28P01` authentication refusal from `22012`
division by zero and verifies a later query on the same connection. No request
payload or process identity is synthesized. A raw `receive_auth` stage only
means an authentication message was seen, not login succeeded; a ready message
alone is also not proof that the preceding query succeeded.

The schema-2 fixture also checks a failed explicit transaction: `BEGIN` yields
ReadyForQuery `T`, division by zero yields `E`, the next query is rejected with
`25P02`, and `ROLLBACK` returns to `I` before a successful query. SQL NULL and
empty text are retained as distinct row values. These states follow the
[PostgreSQL wire formats](https://www.postgresql.org/docs/16/protocol-message-formats.html);
they are framed fixture observations, not new meanings assigned to packet-only
program stages. Extended queries, arbitrary result types and production-wide
transaction correlation remain outside this bounded fixture.

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
[docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md).

For the cross-database comparison table, see
[docs/book/reference-database-failure-semantics.md](docs/book/reference-database-failure-semantics.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `postgres-error`
- `postgres_error`

<!-- gewyvern:entry-aliases:end -->
