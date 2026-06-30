# Reference: Cassandra Query Surface

The Cassandra session/query surface tracks native protocol session setup, query request frames, and result frames.

Family hub: [Cassandra surface](docs/book/reference-cassandra-surface.md)

Canonical entries: `startup`, `query`, `result`

## Debugging Focus

- Client-to-server `STARTUP` frames.
- Client-to-server `QUERY` frames.
- Server-to-client `RESULT` frames.
- Route, process, and TCP lineage around the cluster flow.

## Typical Question

Use this surface when a Cassandra client connects but CQL traffic appears one-way, responses are missing, or a cluster gateway may be dropping native frames.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cassandra-query`
- `cassandra-result`
- `cassandra-startup`
- `cassandra_query`
- `cassandra_result`
- `cassandra_startup`
- `connect`
- `cql`
- `cql-query`
- `cql-result`
- `cql-startup`
- `cql_query`
- `cql_result`
- `cql_startup`
- `handshake`
- `hello`
- `request`
- `response`
- `rows`
- `select`
- `statement`
- `success`

<!-- gewyvern:entry-aliases:end -->
