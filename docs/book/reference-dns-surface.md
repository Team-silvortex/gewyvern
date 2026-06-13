# Reference: DNS Protocol Surface

Use this page when you want the DNS portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current DNS coverage into two narrower operator-facing
surfaces:

- UDP lookup flow
- TCP query flow

## What This Shelf Covers

The current built-in DNS family models two transport variants for the same
coarse lookup conversation:

- bind the process and resolve the upstream route
- send a DNS query
- receive a DNS response

Across the subpages, the lookup contract focuses on:

- canonical entry names
- transport-specific lookup posture
- operator reading order
- validation and lowering posture

## DNS Surface Map

### UDP

- [docs/book/reference-dns-udp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-udp-surface.md)
  Datagram-style DNS lookup path.

Typical entries:

- `udp`

### TCP

- [docs/book/reference-dns-tcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-tcp-surface.md)
  TCP-carried DNS query and response path.

Typical entries:

- `tcp`

## Reading Order

If you are validating current DNS support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
3. the UDP or TCP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current DNS family in the `1.4.x` line.
New DNS transport branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
