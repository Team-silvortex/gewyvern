# Reference: Cassandra Error Surface

The `cassandra/error` entry treats native protocol `ERROR` frames as explicit server-side failure signals.

Family hub: [Cassandra surface](docs/book/reference-cassandra-surface.md)

Canonical entry: `error`

## Debugging Focus

- Server-to-client `ERROR` frames.
- Explicit denial or semantic failure returned by the Cassandra cluster.
- Route, process, and TCP lineage around the failed request.

## Typical Question

Use this surface when the transport path is alive but Cassandra reports a request-level or cluster-level failure.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cassandra-error`
- `cassandra_error`
- `cql-error`
- `cql_error`
- `denied`
- `failure`
- `server-error`

<!-- gewyvern:entry-aliases:end -->
