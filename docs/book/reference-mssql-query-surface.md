# Reference: SQL Server / TDS Query Surface

The TDS query surface tracks SQL batch request and tabular response packet types. Setup and login live in the sibling [TDS session surface](docs/book/reference-mssql-session-surface.md), while completion and environment-change tokens live in the sibling [TDS token surface](docs/book/reference-mssql-token-surface.md).

Family hub: [SQL Server / TDS surface](docs/book/reference-mssql-surface.md)

Canonical entries: `query`, `response`

## Debugging Focus

- Client-to-server SQL batch packets.
- Server-to-client tabular response packets.
- Route, process, and TCP lineage around the database flow.

## Typical Question

Use this surface when a SQL Server client has reached an established session but query or response directionality is unclear.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `batch`
- `mssql-query`
- `mssql-response`
- `mssql_query`
- `mssql_response`
- `reply`
- `request`
- `result`
- `sql-batch`
- `sql-server`
- `sqlserver`
- `sqlserver-query`
- `sqlserver-response`
- `sqlserver_query`
- `sqlserver_response`
- `tabular`
- `tds-query`
- `tds-response`
- `tds_query`
- `tds_response`

<!-- gewyvern:entry-aliases:end -->
