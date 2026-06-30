# Reference: SQL Server / TDS Error Surface

The `mssql/error` entry treats a TDS error token as an explicit SQL Server failure signal.

Family hub: [SQL Server / TDS surface](docs/book/reference-mssql-surface.md)

Canonical entry: `error`

## Debugging Focus

- Server-to-client tabular response packet.
- Error token at the start of the response payload.
- Process and route context around the failed request.

## Typical Question

Use this surface when transport is alive but SQL Server returns a request-level, auth-level, or execution-level failure.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `denied`
- `error-token`
- `failure`
- `mssql-error`
- `mssql_error`
- `server-error`
- `sqlserver-error`
- `sqlserver_error`
- `tds-error`
- `tds_error`

<!-- gewyvern:entry-aliases:end -->
