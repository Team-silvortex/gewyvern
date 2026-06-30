# Reference: BGP Session Surface

BGP session surfaces expose the first practical routing-control health signals:
OPEN negotiation and KEEPALIVE liveness.

## Canonical Entries

- family: `bgp`
- entries: `open`, `keepalive`
- shelf key: `session`
- DSL: [dsl/bgp_open_path.gewy](dsl/bgp_open_path.gewy)
- DSL: [dsl/bgp_keepalive_path.gewy](dsl/bgp_keepalive_path.gewy)

## Aliases

- `bgp-open`
- `bgp_open`
- `peer-open`
- `session-open`
- `bgp-keepalive`
- `bgp_keepalive`
- `keep-alive`
- `session-keepalive`

## Runtime Shape

- TCP remote port `179` identifies the BGP peer side
- BGP message type `1` models OPEN
- BGP message type `4` models KEEPALIVE
- payload offset `18` carries the BGP message type after the marker and length

## Related Pages

- [docs/book/reference-bgp-surface.md](docs/book/reference-bgp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `bgp-keepalive`
- `bgp-open`
- `bgp_keepalive`
- `bgp_open`
- `keep-alive`
- `peer-open`
- `session-keepalive`
- `session-open`

<!-- gewyvern:entry-aliases:end -->
