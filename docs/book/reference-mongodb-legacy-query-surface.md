# Reference: MongoDB Legacy Query Surface

The `mongodb/legacy-query` entry tracks older `OP_QUERY` request traffic.

Family hub: [MongoDB surface](docs/book/reference-mongodb-surface.md)

Canonical entry: `legacy-query`

## Debugging Focus

- Client-to-server `OP_QUERY` frames.
- Older drivers or compatibility gateways.
- Route, process, and TCP lineage around the query request.

## Typical Question

Use this surface when modern `OP_MSG` does not appear but the client still talks to a MongoDB-compatible endpoint.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `legacy`
- `mongo-legacy`
- `mongo-query`
- `mongo_legacy`
- `mongodb-legacy`
- `mongodb-query`
- `mongodb_legacy`
- `op-query`
- `opquery`
- `query`

<!-- gewyvern:entry-aliases:end -->
