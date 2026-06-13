# Reference: MySQL Query Surface

Use this page when you need the current exact lookup surface for MySQL query
and query-session flow.

## Canonical Entries

### `query`

Aliases:

- none registered at the entry level today

Protocol aliases:

- `mysql-query`
- `mysql_query`

Intent:

- operate over a MySQL session
- send a query
- receive success/OK

### `session`

Aliases:

- none registered at the entry level today

Protocol aliases:

- `mysql-session`
- `mysql_session`

Intent:

- model the broader query session
- establish the MySQL socket
- send a query
- receive success/OK

## Shared Response Shape

Both entries currently share a query-oriented staging model:

1. process binding
2. route resolution
3. socket connect and establish
4. query send
5. OK response receive

`session` keeps the broader session framing, while `query` stays focused on the
request/response pair.

## Operator Reading Order

If you are reviewing MySQL query coverage, read it in this order:

1. `connect`
2. `query`
3. `session`

That sequence keeps transport setup in front of the broader session-oriented
query model.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `mysql` family resolution
- canonical entry normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md).
