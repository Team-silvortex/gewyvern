# Reference: Consul Surface

Consul support gives gewyvern a service-discovery and coordination view for
health checks, catalog lookups, service resolution, KV access, and session or
lock lifecycle traffic.

Default entry: `service`

Protocol aliases: `consul-agent`, `consul-health`, `consul_health`, `consul-catalog`, `consul_catalog`, `consul-service`, `consul_service`, `service-discovery`, `consul-kv`, `consul_kv`, `consul-session`, `consul_session`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`health`](docs/book/reference-consul-discovery-surface.md) tracks health-check queries.
- [`catalog`](docs/book/reference-consul-discovery-surface.md) tracks catalog and node/service inventory.
- [`service`](docs/book/reference-consul-discovery-surface.md) tracks service discovery and resolution.
- [`kv`](docs/book/reference-consul-state-surface.md) tracks KV read/write coordination state.
- [`session`](docs/book/reference-consul-state-surface.md) tracks session, lock, and renewal flows.

## Operator Use

Start with `service` when callers resolve the wrong endpoint. Use `health` when
healthy instances disappear. Use `catalog` when inventory differs from agent
state. Use `kv` and `session` when lock ownership or lightweight coordination
state looks unstable.

## Limits

This surface is HTTP-method-aware and operation-family-aware. It does not parse
JSON payloads, datacenter query parameters, ACL token scopes, blocking query
indexes, or service tag filters yet.
