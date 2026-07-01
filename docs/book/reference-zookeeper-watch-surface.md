# Reference: ZooKeeper Watch Surface

Navigation: [ZooKeeper surface](docs/book/reference-zookeeper-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `watch` entry for ZooKeeper.

Use it when clients miss updates, re-register watches unexpectedly, or receive
events in a surprising order after session movement.

## Entries

- `watch`

## Signals

- Watch registration direction.
- Watch event delivery direction.
- Process and route lineage for the watching client.

## Operator Notes

ZooKeeper watches are one-shot and session-sensitive. Correlate watch traffic
with reconnects and znode reads before treating missed events as a pure network
loss.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `set-watch`
- `set_watch`
- `setwatches`
- `watch-event`
- `watch_event`
- `zk-watch`
- `zk_watch`
- `zookeeper-watch`
- `zookeeper_watch`

<!-- gewyvern:entry-aliases:end -->
