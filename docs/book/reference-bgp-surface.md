# Reference: BGP Protocol Surface

BGP gives gewyvern a routing-control session shelf for border and internal
peer debugging. The first surface focuses on session opening and keepalive
liveness before route UPDATE semantics are introduced.

## Registry Lookup

- `bgp` family lookup
- Default entry: `open`
- package aliases: `bgp-keepalive`, `bgp-open`, `bgp_keepalive`,
  `bgp_open`, `keep-alive`, `peer-open`, `session-keepalive`,
  `session-open`

## Entries

- [docs/book/reference-bgp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-bgp-session-surface.md)

## Operator Model

Use the BGP shelf when the question is whether a peer can establish and
maintain a routing-control session on TCP/179. Route advertisement and
withdrawal semantics should layer on top of this session footing.

## Book Path

Read this after:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)

Then continue with:

1. [docs/book/reference-bgp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-bgp-session-surface.md)
2. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
