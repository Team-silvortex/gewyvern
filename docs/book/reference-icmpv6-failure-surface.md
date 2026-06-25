# Reference: ICMPv6 Failure Surface

ICMPv6 failure models destination-unreachable style path rejection for IPv6
traffic.

## Canonical Entry

- family: `icmpv6`
- entry: `unreachable`
- shelf key: `failure`
- DSL: [dsl/icmpv6_unreachable_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_unreachable_path.gewy)

## Aliases

- `admin-prohibited`
- `dest-unreachable`
- `destination-unreachable`
- `icmpv6-unreachable`
- `icmpv6_unreachable`
- `no-route`
- `port-unreachable`

## Runtime Shape

- ICMPv6 type `1` is treated as the canonical destination-unreachable signal
- primary failure mode: `network_unreachable`
- failure basis: `remote_or_path_rejected`

## Related Pages

- [docs/book/reference-icmpv6-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-surface.md)
- [docs/book/reference-icmpv6-echo-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-echo-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `admin-prohibited`
- `dest-unreachable`
- `destination-unreachable`
- `icmpv6-unreachable`
- `icmpv6_unreachable`
- `no-route`
- `port-unreachable`

<!-- gewyvern:entry-aliases:end -->
