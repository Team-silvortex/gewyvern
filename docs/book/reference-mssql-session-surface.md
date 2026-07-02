# Reference: SQL Server / TDS Session Surface

The TDS session surface tracks pre-login negotiation and login/authentication packets before SQL batch traffic starts.

Family hub: [SQL Server / TDS surface](docs/book/reference-mssql-surface.md)

Canonical entries: `prelogin`, `login`

## Debugging Focus

- Client-to-server `PRELOGIN` negotiation packets.
- Client-to-server `LOGIN` and `LOGIN7` authentication packets.
- TCP lineage around setup and authentication boundaries.
- Cases where SQL batch traffic never begins because the session did not finish.

## Typical Question

Use this surface when a SQL Server client reaches the host but stalls before query traffic, or when authentication and pre-login negotiation need to be separated from later query/response debugging.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth`
- `authenticate`
- `connect`
- `handshake`
- `login7`
- `mssql-login`
- `mssql-prelogin`
- `mssql_login`
- `mssql_prelogin`
- `pre-login`
- `sqlserver-login`
- `sqlserver-prelogin`
- `sqlserver_login`
- `sqlserver_prelogin`
- `tds`
- `tds-login`
- `tds-prelogin`
- `tds_login`
- `tds_prelogin`

<!-- gewyvern:entry-aliases:end -->
