# Reference: ZooKeeper Znode Surface

Navigation: [ZooKeeper surface](docs/book/reference-zookeeper-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `read` and `write` entries for ZooKeeper.

Use it when a client reads stale data, cannot create or update znodes, or sees
unexpected behavior around path existence and mutation ordering.

## Entries

- `read`
- `write`

## Signals

- Znode read request and response direction.
- Znode mutation request and response direction.
- Route and process context for the client issuing the operation.

## Operator Notes

Pair this shelf with session state. Many znode issues are caused by a client
talking to the wrong ensemble member, losing its session, or hitting ACLs before
the data operation can be trusted.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `create`
- `delete`
- `exists`
- `get`
- `get-children`
- `get_children`
- `getdata`
- `mutation`
- `set-data`
- `setdata`
- `zk`
- `zk-read`
- `zk-write`
- `zk_read`
- `zk_write`
- `zookeeper-client`
- `zookeeper-read`
- `zookeeper-write`
- `zookeeper_read`
- `zookeeper_write`

<!-- gewyvern:entry-aliases:end -->
