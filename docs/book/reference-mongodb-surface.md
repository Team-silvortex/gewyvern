# Reference: MongoDB Surface

MongoDB support gives gewyvern a database wire-protocol view for document-store traffic on the standard server path.

Default entry: `command`

Protocol aliases: `mongo`, `mongo-command`, `mongo_command`, `mongodb-command`, `mongodb_command`, `mongo-opmsg`, `mongodb-opmsg`, `mongo-reply`, `mongo_reply`, `mongodb-reply`, `mongodb_reply`, `mongo-response`, `mongodb-response`, `mongo-legacy`, `mongo_legacy`, `mongodb-legacy`, `mongodb_legacy`, `mongo-query`, `mongodb-query`, `mongo-query-failure`, `mongo_query_failure`, `mongodb-query-failure`, `mongodb_query_failure`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`command`](docs/book/reference-mongodb-command-surface.md) tracks modern `OP_MSG` command traffic.
- [`reply`](docs/book/reference-mongodb-reply-surface.md) tracks `OP_MSG` and legacy `OP_REPLY` response traffic.
- [`legacy-query`](docs/book/reference-mongodb-legacy-query-surface.md) tracks older `OP_QUERY` request traffic.
- [`query-failure`](docs/book/reference-mongodb-legacy-query-surface.md) tracks legacy `OP_REPLY` QueryFailure responses.

## Operator Use

Start with `command` for modern clients and general request path debugging. Use `reply` when the client reaches the server but response directionality is unclear. Use `legacy-query` only when older drivers, proxies, or compatibility paths are suspected, and use `query-failure` when that legacy path returns a server-side failure flag.

## Limits

This surface is opcode-aware, not BSON-schema-aware. It does not decode collection names, filters, sessions, or command documents yet.
