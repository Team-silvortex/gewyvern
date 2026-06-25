# Reference: NDP Advertisement Surface

NDP advertisement models an IPv6 Neighbor Advertisement response.

## Canonical Entry

- family: `ndp`
- entry: `advertise`
- shelf key: `advertise`
- DSL: [dsl/ndp_advertise_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ndp_advertise_path.gewy)

## Aliases

- `advertisement`
- `na`
- `ndp-advertise`
- `ndp_advertise`
- `neighbor-advertisement`

## Runtime Shape

- ICMPv6 type `136` is the Neighbor Advertisement signal
- direction is modeled as remote-to-local
- category: `neighbor-resolution-path`

## Related Pages

- [docs/book/reference-ndp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ndp-surface.md)
- [docs/book/reference-ndp-solicit-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ndp-solicit-surface.md)
