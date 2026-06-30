# Reference: SQL Server / TDS Token Surface

The `mssql/colmetadata`, `mssql/row`, `mssql/done`, and `mssql/envchange` entries track stable TDS response tokens that are useful before full token-stream decoding exists.

Family hub: [SQL Server / TDS surface](docs/book/reference-mssql-surface.md)

Canonical entries: `colmetadata`, `row`, `done`, `envchange`

## Debugging Focus

- COLMETADATA tokens that show SQL Server has started describing a result set.
- ROW and NBCROW tokens that show result rows are returning.
- DONE-family tokens that mark response completion, stored procedure completion, or in-procedure completion.
- ENVCHANGE tokens that mark session environment shifts such as database, language, charset, or packet-size changes.
- Process, route, and TCP lineage around SQL Server responses.

## Typical Question

Use this surface when the transport and query packets are visible, but result-set progress, completion boundaries, or post-login session state are the confusing part.

## Limits

This is token-presence recognition at the first response token position. It does not yet decode column payload fields, row values, row counts, status bits, or nested token streams.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `column-metadata`
- `columns`
- `complete`
- `completion`
- `data-row`
- `done-token`
- `doneinproc`
- `doneproc`
- `env`
- `env-change`
- `environment`
- `metadata`
- `mssql-colmetadata`
- `mssql-done`
- `mssql-envchange`
- `mssql-row`
- `mssql_colmetadata`
- `mssql_done`
- `mssql_envchange`
- `mssql_row`
- `nbcrow`
- `record`
- `records`
- `result-shape`
- `rows`
- `schema`
- `session-change`
- `sqlserver-colmetadata`
- `sqlserver-done`
- `sqlserver-envchange`
- `sqlserver-row`
- `sqlserver_colmetadata`
- `sqlserver_done`
- `sqlserver_envchange`
- `sqlserver_row`
- `tds-colmetadata`
- `tds-done`
- `tds-envchange`
- `tds-row`
- `tds_colmetadata`
- `tds_done`
- `tds_envchange`
- `tds_row`

<!-- gewyvern:entry-aliases:end -->
