# Reference: VXLAN Overlay Surface

This shelf groups VXLAN entries that describe UDP overlay posture and tenant
VNI marking.

Read this alongside:

- [docs/book/reference-vxlan-surface.md](docs/book/reference-vxlan-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `overlay`
- label: `Overlay`
- entries: `encap`, `vni`

## Entries

### `encap`

Use `encap` when the important first question is:

- “is this host sending or receiving VXLAN traffic?”
- “which side of the path sees UDP `4789`?”
- “should the inner payload be interpreted only after overlay presence is clear?”

The runtime phases are:

- `send_overlay_packet`
- `receive_overlay_packet`

### `vni`

Use `vni` when the important first question is:

- “does this packet carry the VXLAN I flag?”
- “is tenant overlay marking present in both directions?”
- “should later diagnosis group this traffic by VNI or tenant context?”

The runtime phases are:

- `send_vni_marked_packet`
- `receive_vni_marked_packet`

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `overlay`
- `tenant-overlay`
- `vni`
- `vni-overlay`
- `vxlan-tunnel`
- `vxlan-vni`
- `vxlan_tunnel`
- `vxlan_vni`

<!-- gewyvern:entry-aliases:end -->
