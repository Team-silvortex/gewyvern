# Reference: etcd KV Surface

Navigation: [etcd surface](docs/book/reference-etcd-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `range` and `put` entries for etcd.

Use it when an application reads unexpected values, misses a prefix, writes to
the wrong member, or observes revision behavior that does not match operator
expectations.

## Entries

- `range`
- `put`

## Signals

- KV read requests.
- KV write requests.
- Response direction after the selected KV operation.

## Operator Notes

For reads, compare quorum expectations, route target, and revision posture. For
writes, check whether a successful transport exchange actually maps to a write
path or whether the client is stuck retrying before the write reaches a leader.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `etcd-kv`
- `etcd-put`
- `etcd-range`
- `etcd_kv`
- `etcd_put`
- `etcd_range`
- `etcdctl`
- `get`
- `kv-put`
- `kv-range`
- `kv_put`
- `kv_range`
- `read`
- `set`
- `write`

<!-- gewyvern:entry-aliases:end -->
