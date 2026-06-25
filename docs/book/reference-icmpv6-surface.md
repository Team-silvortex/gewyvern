# Reference: ICMPv6 Protocol Surface

ICMPv6 gives gewyvern an IPv6 reachability shelf that mirrors the IPv4 ICMP
surface while keeping type numbers and aliases explicit.

## Registry Lookup

- `icmpv6` family lookup
- Default entry: `echo`
- package aliases: `icmp-v6`, `icmp6`, `icmpv6-echo`,
  `icmpv6-unreachable`, `icmpv6_echo`, `icmpv6_unreachable`, `ping6`

## Entries

- [docs/book/reference-icmpv6-echo-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-echo-surface.md)
- [docs/book/reference-icmpv6-failure-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-failure-surface.md)

## Operator Model

Use the ICMPv6 shelf when a path is IPv6-native and the useful diagnostic
signal is reachability rather than application payload.

## Book Path

Read this after:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)

Then continue with:

1. [docs/book/reference-icmpv6-echo-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-echo-surface.md)
2. [docs/book/reference-icmpv6-failure-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-failure-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
