# Reference: GENEVE Protocol Surface

GENEVE support gives gewyvern a UDP overlay view for extensible virtual network
traffic carried on port `6081`. Use it when option metadata may be as important
as the encapsulated payload.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-geneve-overlay-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-geneve-overlay-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

VXLAN is the neighboring overlay family to compare against when option metadata
is not part of the packet shape.

## Entries

Default entry: `encap`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `encap` | GENEVE overlay traffic on UDP port `6081` | UDP/6081 GENEVE packet |
| `options` | GENEVE packets carrying option metadata | GENEVE option length bits set |

## Operator Notes

- `encap` answers whether GENEVE is present before inner payload diagnosis.
- `options` narrows on extension metadata, useful when policy or telemetry TLVs
  influence forwarding behavior.
- Treat GENEVE as an outer overlay context; the inner protocol still needs its
  own surface once payload semantics become visible.

## Aliases

- `encap`: `geneve-tunnel`, `geneve_tunnel`, `overlay-options`, `geneve-overlay`
- `options`: `geneve-options`, `geneve_options`, `geneve-tlv`, `geneve_tlv`, `optioned-overlay`
