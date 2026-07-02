# Reference: Cassandra Session Surface

The Cassandra session surface tracks native protocol startup and authentication
prompts before ordinary CQL query traffic is meaningful.

Family hub: [Cassandra surface](docs/book/reference-cassandra-surface.md)

Canonical entries: `startup`, `authenticate`

## Debugging Focus

- Client-to-server `STARTUP` frames.
- Server-to-client `AUTHENTICATE` frames.
- Route, process, and TCP lineage around initial cluster access.

## Typical Question

Use this surface when a Cassandra client opens a socket but never reaches a
stable authenticated native-protocol session.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth-required`
- `auth_required`
- `authenticate-required`
- `authenticate_required`
- `cassandra-authenticate`
- `cassandra-startup`
- `cassandra_authenticate`
- `cassandra_startup`
- `connect`
- `cql-authenticate`
- `cql-startup`
- `cql_authenticate`
- `cql_startup`
- `handshake`
- `hello`

<!-- gewyvern:entry-aliases:end -->
