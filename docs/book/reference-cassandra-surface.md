# Reference: Cassandra Surface

Cassandra support gives gewyvern a native-protocol view for CQL traffic on the standard cluster client path.

Default entry: `query`

Protocol aliases: `cql`, `cql-startup`, `cql_startup`, `cassandra-startup`, `cassandra_startup`, `cql-authenticate`, `cql_authenticate`, `cassandra-authenticate`, `cassandra_authenticate`, `cassandra-query`, `cassandra_query`, `cql-query`, `cql_query`, `cql-result`, `cql_result`, `cassandra-result`, `cassandra_result`, `cql-error`, `cql_error`, `cassandra-error`, `cassandra_error`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`startup`](docs/book/reference-cassandra-session-surface.md) tracks native protocol session startup.
- [`authenticate`](docs/book/reference-cassandra-session-surface.md) tracks server authentication prompts.
- [`query`](docs/book/reference-cassandra-query-surface.md) tracks CQL query request frames.
- [`result`](docs/book/reference-cassandra-query-surface.md) tracks successful server result frames.
- [`error`](docs/book/reference-cassandra-error-surface.md) tracks server error frames.

## Operator Use

Start with `query` for normal CQL request debugging. Use `startup` when a client never gets a stable session, `authenticate` when the server asks for credentials, `result` to check response directionality, and `error` when Cassandra explicitly rejects or fails a request.

## Limits

This surface is opcode-aware, not CQL-parser-aware. It does not decode query strings, keyspaces, consistency levels, or result metadata yet.
