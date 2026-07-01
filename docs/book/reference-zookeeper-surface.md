# Reference: ZooKeeper Surface

ZooKeeper support gives gewyvern a coordination-service view for ensemble
session traffic, znode reads and writes, watch delivery, and ACL or auth
denial paths.

Default entry: `read`

Protocol aliases: `zk`, `zookeeper-client`, `zk-connect`, `zk_connect`, `zookeeper-connect`, `zookeeper_connect`, `zk-read`, `zk_read`, `zookeeper-read`, `zookeeper_read`, `zk-write`, `zk_write`, `zookeeper-write`, `zookeeper_write`, `zk-watch`, `zk_watch`, `zookeeper-watch`, `zookeeper_watch`, `zk-auth-denied`, `zk_auth_denied`, `zookeeper-auth-denied`, `zookeeper_auth_denied`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`connect`](docs/book/reference-zookeeper-session-surface.md) tracks session establishment.
- [`read`](docs/book/reference-zookeeper-znode-surface.md) tracks znode read-style operations.
- [`write`](docs/book/reference-zookeeper-znode-surface.md) tracks znode mutation-style operations.
- [`watch`](docs/book/reference-zookeeper-watch-surface.md) tracks watch registration and event delivery.
- [`auth-denied`](docs/book/reference-zookeeper-session-surface.md) tracks auth and ACL denial paths.

## Operator Use

Start with `connect` when clients churn sessions or bounce between ensemble
members. Use `read` and `write` for znode data paths. Use `watch` when clients
miss updates. Use `auth-denied` when ACLs or auth schemes reject an operation.

## Limits

This surface identifies ZooKeeper family intent and direction over the stable
tcp/2181 path. It does not decode binary opcodes, zxid values, paths, ACL
records, or watch event payloads yet.
