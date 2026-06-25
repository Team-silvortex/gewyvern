# Reference: ICMPv6 Echo Surface

ICMPv6 echo models IPv6 ping-style reachability checks.

## Canonical Entry

- family: `icmpv6`
- entry: `echo`
- shelf key: `echo`
- DSL: [dsl/icmpv6_echo_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/icmpv6_echo_path.gewy)

## Aliases

- `echo-request`
- `echo-reply`
- `icmp-v6`
- `icmp6`
- `icmpv6-echo`
- `icmpv6_echo`
- `ping6`
- `ping6-check`

## Runtime Shape

- route and process context are optional but preferred when available
- ICMPv6 type `128` is the outbound echo request
- ICMPv6 type `129` is the inbound echo reply

## Related Pages

- [docs/book/reference-icmpv6-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-surface.md)
- [docs/book/reference-icmpv6-failure-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-failure-surface.md)
- [docs/book/reference-ndp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ndp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `echo-reply`
- `echo-request`
- `icmp-v6`
- `icmp6`
- `icmpv6-echo`
- `icmpv6_echo`
- `ping6`
- `ping6-check`

<!-- gewyvern:entry-aliases:end -->
