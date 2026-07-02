# Reference: Cassandra Query Surface

The Cassandra query surface tracks CQL request frames and successful server
result frames after the native protocol session is established.

Family hub: [Cassandra surface](docs/book/reference-cassandra-surface.md)

Canonical entries: `query`, `result`

## Debugging Focus

- Client-to-server `QUERY` frames.
- Server-to-client `RESULT` frames.
- Route, process, and TCP lineage around the cluster flow.

## Typical Question

Use this surface when a Cassandra client connects but CQL traffic appears
one-way, responses are missing, or a cluster gateway may be dropping native
frames after session setup.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cassandra-query`
- `cassandra-result`
- `cassandra_query`
- `cassandra_result`
- `cql`
- `cql-query`
- `cql-result`
- `cql_query`
- `cql_result`
- `request`
- `response`
- `rows`
- `select`
- `statement`
- `success`

<!-- gewyvern:entry-aliases:end -->
