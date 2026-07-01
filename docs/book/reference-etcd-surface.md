# Reference: etcd Surface

etcd support gives gewyvern a control-plane datastore view for Kubernetes-style
coordination traffic, including cluster health, KV range reads, writes, watch
streams, and lease lifecycle activity.

Default entry: `range`

Protocol aliases: `etcdctl`, `etcd-kv`, `etcd_kv`, `etcd-health`, `etcd_health`, `etcd-status`, `etcd_status`, `etcd-range`, `etcd_range`, `etcd-put`, `etcd_put`, `etcd-watch`, `etcd_watch`, `etcd-lease`, `etcd_lease`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`health`](docs/book/reference-etcd-health-surface.md) tracks member or cluster health probes.
- [`range`](docs/book/reference-etcd-kv-surface.md) tracks KV reads and prefix scans.
- [`put`](docs/book/reference-etcd-kv-surface.md) tracks KV writes.
- [`watch`](docs/book/reference-etcd-stream-lifecycle-surface.md) tracks watch stream creation and event flow.
- [`lease`](docs/book/reference-etcd-stream-lifecycle-surface.md) tracks lease grant, keepalive, revoke, and TTL flows.

## Operator Use

Start with `health` when member reachability is unclear. Use `range` for stale
reads, revision confusion, or prefix-scan surprises. Use `watch` when consumers
miss updates or get cancelled after compaction. Use `lease` when keys disappear,
TTL refreshes stop, or service discovery liveness looks unstable.

## Limits

This surface identifies the etcd operation family and transport direction. It
does not decode protobuf request bodies, key names, revision numbers, watch IDs,
or lease IDs yet.
