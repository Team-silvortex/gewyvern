# Reference: STUN Surface

Read this page after the generic protocol surface when the runtime looks like
STUN or TURN-flavored UDP control traffic.

Use it for:

- `stun` family lookup
- default entry selection for `binding`
- relay-oriented control flows such as `allocate` and `refresh`
- explicit binding failure paths such as `stun-binding-error`
- alias spellings such as `stun-allocate`, `stun_allocate`, `stun-refresh`, and `stun_refresh`
- shared management-UDP timeout, reply, and result-surface semantics

Primary subpages:

- [docs/book/reference-stun-binding-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-binding-surface.md)
- [docs/book/reference-stun-relay-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-relay-surface.md)
- [docs/book/reference-management-udp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-failure-semantics.md)
- [docs/book/reference-stun-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-failure-semantics.md)

Current canonical entries:

- `binding` as the default entry
- `allocate`
- `binding-error`
- `refresh`

Default entry: `binding`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-stun-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-surface.md)
3. one exact STUN subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
