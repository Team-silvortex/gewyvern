# Reference: NDP Protocol Surface

NDP models IPv6 neighbor discovery as a first-class local-link diagnostic
surface. It is intentionally separate from generic ICMPv6 because operators
usually debug it as address ownership and link-layer reachability.

## Registry Lookup

- `ndp` family lookup
- Default entry: `solicit`
- package aliases: `ndp-advertise`, `ndp-solicit`, `ndp_advertise`,
  `ndp_solicit`, `neighbor-advertisement`, `neighbor-solicitation`

## Entries

- [docs/book/reference-ndp-solicit-surface.md](docs/book/reference-ndp-solicit-surface.md)
- [docs/book/reference-ndp-advertise-surface.md](docs/book/reference-ndp-advertise-surface.md)

## Operator Model

Use NDP when the diagnostic question is "can this IPv6 address be resolved on
this link?" rather than "can this endpoint answer an application request?"

## Book Path

Read this after:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)

ICMPv6 is the adjacent reachability family; NDP stays on its own neighbor
resolution shelf.

Then continue with:

1. [docs/book/reference-ndp-solicit-surface.md](docs/book/reference-ndp-solicit-surface.md)
2. [docs/book/reference-ndp-advertise-surface.md](docs/book/reference-ndp-advertise-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
