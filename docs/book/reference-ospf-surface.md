# Reference: OSPF Protocol Surface

OSPF gives gewyvern an internal link-state routing shelf. The first surface
covers neighbor discovery and database description exchange, which are the
earliest practical signals when an adjacency is unhealthy.

## Registry Lookup

- `ospf` family lookup
- Default entry: `hello`
- package aliases: `database-description`, `db-description`, `dd`,
  `neighbor-hello`, `ospf-dbdesc`, `ospf-hello`, `ospf_dbdesc`,
  `ospf_hello`

## Entries

- [docs/book/reference-ospf-neighbor-surface.md](docs/book/reference-ospf-neighbor-surface.md)
- [docs/book/reference-ospf-database-surface.md](docs/book/reference-ospf-database-surface.md)

## Operator Model

Use the OSPF shelf when a route is missing because routers may not have become
neighbors or may not have synchronized link-state database descriptions.

## Book Path

Read this after:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)

Then continue with:

1. [docs/book/reference-ospf-neighbor-surface.md](docs/book/reference-ospf-neighbor-surface.md)
2. [docs/book/reference-ospf-database-surface.md](docs/book/reference-ospf-database-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
