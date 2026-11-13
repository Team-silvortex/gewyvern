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

An ERR tag alone cannot distinguish login refusal from a query error. The
`auth-denied` packet candidate now requires error 1045 and `#28000`; `error`
requires a sequence-1 ERR with an observed protocol-4.1 SQLSTATE field. These are
conservative initial-response prefixes, not general message decoding: other
authentication refusals, errors later in a result stream, split headers, TLS,
compression and authentication switching can remain unmatched.

The real `--mysql-memcached` fixture separately checks complete framed messages,
packet sequences and native-password authentication. It compares login refusal
`1045/28000` with syntax error `1064/42000`, then verifies later query results and
PING on the same connection. NULL and empty text remain distinct. Raw OK tags in
the existing auth/query packages are still packet observations, not independent
proof of the command that produced them. See the official
[ERR packet format](https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_basic_err_packet.html).

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
