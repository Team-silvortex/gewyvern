# Reference: MongoDB Legacy Query Surface

The `mongodb/legacy-query` and `mongodb/query-failure` entries track older
OP_QUERY request traffic and legacy OP_REPLY failure responses.

Family hub: [MongoDB surface](docs/book/reference-mongodb-surface.md)

Canonical entries: `legacy-query`, `query-failure`

## Debugging Focus

- Client-to-server `OP_QUERY` frames.
- Server-to-client `OP_REPLY` frames with the QueryFailure flag set.
- Older drivers or compatibility gateways.
- Route, process, and TCP lineage around the query request.

## Typical Question

Use this surface when modern `OP_MSG` does not appear but the client still talks to a MongoDB-compatible endpoint. Use `query-failure` when the endpoint responds, but the response flags indicate an explicit legacy query failure.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `failure`
- `legacy`
- `legacy-failure`
- `mongo-legacy`
- `mongo-query`
- `mongo-query-failure`
- `mongo_legacy`
- `mongo_query_failure`
- `mongodb-legacy`
- `mongodb-query`
- `mongodb-query-failure`
- `mongodb_legacy`
- `mongodb_query_failure`
- `op-query`
- `opquery`
- `query`
- `query-error`
- `query_error`

<!-- gewyvern:entry-aliases:end -->
