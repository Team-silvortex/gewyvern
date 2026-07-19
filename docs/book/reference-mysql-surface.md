# Reference: MySQL Protocol Surface

Use this page when you want the MySQL portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current MySQL coverage into three narrower
operator-facing surfaces:

- socket connect and auth
- query and query-session flow
- query error flow

## What This Shelf Covers

The current built-in MySQL family models a compact database conversation:

- establish the MySQL socket
- send a query
- receive either success or error

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for MySQL entry
selection:

- `mysql-auth`
- `mysql-auth-denied`
- `mysql-connect`
- `mysql-error`
- `mysql-query`
- `mysql-session`
- `mysql_auth`
- `mysql_auth_denied`
- `mysql_connect`
- `mysql_error`
- `mysql_query`
- `mysql_session`

Default entry: `session`

## MySQL Surface Map

### Connect And Auth

- [docs/book/reference-mysql-connect-surface.md](docs/book/reference-mysql-connect-surface.md)
  Socket establishment, handshake, and auth-denial flow.

Typical entries:

- `connect`
- `auth`
- `auth-denied`

### Query And Session

- [docs/book/reference-mysql-query-surface.md](docs/book/reference-mysql-query-surface.md)
  Simple query flow and broader query-session path.

Typical entries:

- `query`
- `session`

### Error

- [docs/book/reference-mysql-error-surface.md](docs/book/reference-mysql-error-surface.md)
  Query-error flow after MySQL session establishment.

Typical entries:

- `error`

## Reading Order

If you are validating current MySQL support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
3. one narrower MySQL subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Stability Note

This page is the lookup hub for the MySQL family in the current `1.4.6` line.
New MySQL command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
