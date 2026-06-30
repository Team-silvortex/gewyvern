# Reference: SQL Server / TDS Query Surface

The TDS session/query surface tracks setup, login, SQL batch, and tabular response packet types. Completion and environment-change tokens live in the sibling [TDS token surface](docs/book/reference-mssql-token-surface.md).

Family hub: [SQL Server / TDS surface](docs/book/reference-mssql-surface.md)

Canonical entries: `prelogin`, `login`, `query`, `response`

## Debugging Focus

- Client-to-server `PRELOGIN` packets.
- Client-to-server `LOGIN` packets.
- Client-to-server SQL batch packets.
- Server-to-client tabular response packets.
- Route, process, and TCP lineage around the database flow.

## Typical Question

Use this surface when a SQL Server client reaches the network path but setup, authentication, or query response directionality is unclear.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth`
- `authenticate`
- `batch`
- `connect`
- `handshake`
- `login7`
- `mssql-login`
- `mssql-prelogin`
- `mssql-query`
- `mssql-response`
- `mssql_login`
- `mssql_prelogin`
- `mssql_query`
- `mssql_response`
- `pre-login`
- `reply`
- `request`
- `result`
- `sql-batch`
- `sql-server`
- `sqlserver`
- `sqlserver-login`
- `sqlserver-prelogin`
- `sqlserver-query`
- `sqlserver-response`
- `sqlserver_login`
- `sqlserver_prelogin`
- `sqlserver_query`
- `sqlserver_response`
- `tabular`
- `tds`
- `tds-login`
- `tds-prelogin`
- `tds-query`
- `tds-response`
- `tds_login`
- `tds_prelogin`
- `tds_query`
- `tds_response`

<!-- gewyvern:entry-aliases:end -->
