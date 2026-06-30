# Reference: MongoDB Command Surface

The `mongodb/command` and `mongodb/reply` entries track the modern MongoDB request/response wire shape.

Family hub: [MongoDB surface](docs/book/reference-mongodb-surface.md)

Canonical entries: `command`, `reply`

## Debugging Focus

- Client-to-server `OP_MSG` command frames.
- Server-to-client `OP_MSG` reply frames.
- Legacy `OP_REPLY` responses for compatibility paths.
- Route, process, and TCP lineage around the database flow.

## Typical Question

Use this surface when a MongoDB client connects but commands appear one-way, responses are missing, or a proxy path may be changing wire behavior.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cmd`
- `mongo`
- `mongo-command`
- `mongo-opmsg`
- `mongo-reply`
- `mongo-response`
- `mongo_command`
- `mongo_reply`
- `mongodb-command`
- `mongodb-opmsg`
- `mongodb-reply`
- `mongodb-response`
- `mongodb_command`
- `mongodb_reply`
- `op-msg`
- `op-reply`
- `opmsg`
- `opreply`
- `request`
- `response`

<!-- gewyvern:entry-aliases:end -->
