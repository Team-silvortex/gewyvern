# Reference: OSPF Neighbor Surface

OSPF neighbor surfaces expose Hello traffic used to discover and maintain
adjacency on IP protocol 89.

## Canonical Entry

- family: `ospf`
- entry: `hello`
- shelf key: `neighbor`
- DSL: [dsl/ospf_hello_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ospf_hello_path.gewy)

## Aliases

- `neighbor-hello`
- `ospf-hello`
- `ospf_hello`

## Runtime Shape

- OSPF version `2` is expected at payload offset `0`
- OSPF packet type `1` identifies Hello traffic
- direction is modeled bidirectionally for sent and received Hello packets

## Related Pages

- [docs/book/reference-ospf-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ospf-surface.md)
- [docs/book/reference-ospf-database-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ospf-database-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `neighbor-hello`
- `ospf-hello`
- `ospf_hello`

<!-- gewyvern:entry-aliases:end -->
