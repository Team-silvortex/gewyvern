# Reference: etcd Watch And Lease Surface

Navigation: [etcd surface](docs/book/reference-etcd-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `watch` and `lease` entries for etcd.

Use it when services disappear unexpectedly, watch consumers stop receiving
events, or a client appears connected but does not maintain key liveness.

## Entries

- `watch`
- `lease`

## Signals

- Watch stream open and response/event direction.
- Lease grant, keepalive, revoke, or TTL request direction.
- Route and process lineage for the client maintaining the stream or lease.

## Operator Notes

Watch and lease failures are often lifecycle bugs rather than one-shot request
bugs. Correlate process identity, route target, and timing before treating the
raw transport as the main failure.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `etcd-lease`
- `etcd-watch`
- `etcd_lease`
- `etcd_watch`
- `grant`
- `keepalive`
- `observe`
- `revoke`
- `stream`
- `ttl`
- `watch-stream`
- `watch_stream`

<!-- gewyvern:entry-aliases:end -->
