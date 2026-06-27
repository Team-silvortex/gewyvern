# Reference: VXLAN Protocol Surface

VXLAN support gives gewyvern a UDP overlay view for virtual L2 traffic carried
on port `4789`. Use it when the first debugging question is whether a tenant
overlay is present before following the inner Ethernet payload.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-vxlan-overlay-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-vxlan-overlay-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

GRE is still useful context when debugging encapsulated payload movement, but
VXLAN keeps its own overlay surface.

## Entries

Default entry: `encap`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `encap` | VXLAN overlay traffic on UDP port `4789` | UDP/4789 VXLAN packet |
| `vni` | VXLAN packets with the VNI-present flag set | VXLAN flags byte with I flag set |

## Operator Notes

- `encap` is intentionally broad and answers “is VXLAN present on this path?”
- `vni` narrows the view to tenant-marked packets where VNI analysis matters.
- Inner Ethernet, IP, or application payloads should be interpreted by their own
  protocol surfaces after the overlay posture is established.

## Aliases

- `encap`: `vxlan-tunnel`, `vxlan_tunnel`, `overlay`, `vni-overlay`
- `vni`: `vxlan-vni`, `vxlan_vni`, `vni`, `tenant-overlay`
