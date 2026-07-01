# Reference: Consul State Surface

Navigation: [Consul surface](docs/book/reference-consul-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `kv` and `session` entries for Consul.

Use it when distributed locks, leader election, or lightweight coordination
state behaves unexpectedly.

## Entries

- `kv`
- `session`

## Signals

- KV read or write request and response direction.
- Session create, renew, destroy, or lock lifecycle request direction.
- Route and process lineage for the client using Consul as coordination state.

## Operator Notes

KV and session bugs are often timing-sensitive. Correlate client identity,
agent target, and request method before treating the failure as a simple HTTP
error.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `consul-kv`
- `consul-session`
- `consul_kv`
- `consul_session`
- `create-session`
- `create_session`
- `destroy`
- `key-value`
- `key_value`
- `kv-get`
- `kv-put`
- `lock`
- `renew`

<!-- gewyvern:entry-aliases:end -->
