# Reference: etcd Health Surface

Navigation: [etcd surface](docs/book/reference-etcd-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `health` entry for etcd.

Use it to separate cluster/member liveness from KV behavior. A failed health
path means later range, put, watch, or lease observations may be stale symptoms
rather than the root cause.

## Entries

- `health`

## Signals

- HTTP `/health` probes.
- Maintenance status-style probes over HTTP/2 or gRPC-like transport.
- Response direction after a health request.

## Operator Notes

Treat health as the trust gate for the rest of the family. If a member cannot
answer health or status consistently, prefer topology and route checks before
debugging individual KV operations.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cluster-health`
- `cluster_health`
- `etcd-health`
- `etcd-status`
- `etcd_health`
- `etcd_status`
- `healthcheck`
- `status`

<!-- gewyvern:entry-aliases:end -->
