# Reference: MongoDB Command Surface

The `mongodb/command` entry tracks the modern client-to-server MongoDB request path.

Family hub: [MongoDB surface](docs/book/reference-mongodb-surface.md)

Canonical entry: `command`

## Debugging Focus

- Client-to-server `OP_MSG` command frames.
- Route, process, and TCP lineage around command submission.
- One-way command symptoms where the client appears to send but no matching reply arrives.
- Proxy or sidecar behavior that may alter request directionality before server handling.

## Typical Question

Use this surface when a MongoDB client connects but the request path itself is uncertain. Switch to the `reply` shelf once the question becomes response directionality or server return behavior.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cmd`
- `mongo`
- `mongo-command`
- `mongo-opmsg`
- `mongo_command`
- `mongodb-command`
- `mongodb-opmsg`
- `mongodb_command`
- `op-msg`
- `opmsg`
- `request`

<!-- gewyvern:entry-aliases:end -->
