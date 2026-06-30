# Reference: OSPF Database Surface

OSPF database surfaces expose Database Description packets exchanged while
routers form or repair adjacency.

## Canonical Entry

- family: `ospf`
- entry: `dbdesc`
- shelf key: `database`
- DSL: [dsl/ospf_dbdesc_path.gewy](dsl/ospf_dbdesc_path.gewy)

## Aliases

- `database-description`
- `db-description`
- `dd`
- `ospf-dbdesc`
- `ospf_dbdesc`

## Runtime Shape

- OSPF version `2` is expected at payload offset `0`
- OSPF packet type `2` identifies Database Description traffic
- use this when neighbors appear but link-state database sync is suspect

## Related Pages

- [docs/book/reference-ospf-surface.md](docs/book/reference-ospf-surface.md)
- [docs/book/reference-ospf-neighbor-surface.md](docs/book/reference-ospf-neighbor-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `database-description`
- `db-description`
- `dd`
- `ospf-dbdesc`
- `ospf_dbdesc`

<!-- gewyvern:entry-aliases:end -->
