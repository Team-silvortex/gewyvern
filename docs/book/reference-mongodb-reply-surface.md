# Reference: MongoDB Reply Surface

The `mongodb/reply` entry tracks server-to-client MongoDB response traffic.

Family hub: [MongoDB surface](docs/book/reference-mongodb-surface.md)

Canonical entry: `reply`

## Debugging Focus

- Server-to-client `OP_MSG` reply frames.
- Legacy `OP_REPLY` response compatibility paths.
- Response directionality when commands reach the server but clients still stall.
- TCP lineage around returned database flow.

## Typical Question

Use this surface when request submission looks healthy but the client does not observe a response, or when a proxy, sidecar, or network boundary may be dropping or reshaping MongoDB replies.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `mongo-reply`
- `mongo-response`
- `mongo_reply`
- `mongodb-reply`
- `mongodb-response`
- `mongodb_reply`
- `op-reply`
- `opreply`
- `response`

<!-- gewyvern:entry-aliases:end -->
