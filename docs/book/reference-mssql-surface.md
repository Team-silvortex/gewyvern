# Reference: SQL Server / TDS Surface

SQL Server support gives gewyvern a TDS packet-level view for database session setup, query traffic, responses, and server-side error tokens.

Default entry: `query`

Protocol aliases: `tds`, `sqlserver`, `sql-server`, `mssql-prelogin`, `mssql_prelogin`, `sqlserver-prelogin`, `sqlserver_prelogin`, `tds-prelogin`, `tds_prelogin`, `mssql-login`, `mssql_login`, `sqlserver-login`, `sqlserver_login`, `tds-login`, `tds_login`, `mssql-query`, `mssql_query`, `sqlserver-query`, `sqlserver_query`, `tds-query`, `tds_query`, `mssql-response`, `mssql_response`, `sqlserver-response`, `sqlserver_response`, `tds-response`, `tds_response`, `mssql-colmetadata`, `mssql_colmetadata`, `sqlserver-colmetadata`, `sqlserver_colmetadata`, `tds-colmetadata`, `tds_colmetadata`, `mssql-row`, `mssql_row`, `sqlserver-row`, `sqlserver_row`, `tds-row`, `tds_row`, `mssql-done`, `mssql_done`, `sqlserver-done`, `sqlserver_done`, `tds-done`, `tds_done`, `mssql-envchange`, `mssql_envchange`, `sqlserver-envchange`, `sqlserver_envchange`, `tds-envchange`, `tds_envchange`, `mssql-error`, `mssql_error`, `sqlserver-error`, `sqlserver_error`, `tds-error`, `tds_error`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`prelogin`](docs/book/reference-mssql-query-surface.md) tracks TDS pre-login negotiation.
- [`login`](docs/book/reference-mssql-query-surface.md) tracks TDS login/authentication packets.
- [`query`](docs/book/reference-mssql-query-surface.md) tracks SQL batch request packets.
- [`response`](docs/book/reference-mssql-query-surface.md) tracks tabular response packets.
- [`colmetadata`](docs/book/reference-mssql-token-surface.md) tracks result-set metadata tokens.
- [`row`](docs/book/reference-mssql-token-surface.md) tracks result-row tokens.
- [`done`](docs/book/reference-mssql-token-surface.md) tracks DONE-family completion tokens.
- [`envchange`](docs/book/reference-mssql-token-surface.md) tracks TDS session environment change tokens.
- [`error`](docs/book/reference-mssql-error-surface.md) tracks TDS error tokens.

## Operator Use

Start with `query` for normal SQL batch debugging. Use `prelogin` and `login` when setup or authentication is the suspect. Use `response` for directionality checks, `colmetadata` and `row` for result-set progress, `done` for completion boundaries, `envchange` for session state shifts, and `error` when SQL Server explicitly returns a failure token.

## Limits

This surface is TDS packet/token aware, not a SQL parser. It does not decode query text, result schemas, transaction state, or full token streams yet.
