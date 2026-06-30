# Reference: Cassandra Query Surface

The Cassandra session/query surface tracks native protocol session setup,
authentication prompts, query request frames, and result frames.

Family hub: [Cassandra surface](docs/book/reference-cassandra-surface.md)

Canonical entries: `startup`, `authenticate`, `query`, `result`

## Debugging Focus

- Client-to-server `STARTUP` frames.
- Server-to-client `AUTHENTICATE` frames.
- Client-to-server `QUERY` frames.
- Server-to-client `RESULT` frames.
- Route, process, and TCP lineage around the cluster flow.

## Typical Question

Use this surface when a Cassandra client connects but CQL traffic appears one-way, responses are missing, or a cluster gateway may be dropping native frames.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth-required`
- `auth_required`
- `authenticate-required`
- `authenticate_required`
- `cassandra-authenticate`
- `cassandra-query`
- `cassandra-result`
- `cassandra-startup`
- `cassandra_authenticate`
- `cassandra_query`
- `cassandra_result`
- `cassandra_startup`
- `connect`
- `cql`
- `cql-authenticate`
- `cql-query`
- `cql-result`
- `cql-startup`
- `cql_authenticate`
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
