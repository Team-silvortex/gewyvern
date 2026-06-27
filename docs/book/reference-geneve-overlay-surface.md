# Reference: GENEVE Overlay Surface

This shelf groups GENEVE entries that describe UDP overlay posture and option
metadata.

Read this alongside:

- [docs/book/reference-geneve-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-geneve-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `overlay`
- label: `Overlay`
- entries: `encap`, `options`

## Entries

### `encap`

Use `encap` when the important first question is:

- “is this host sending or receiving GENEVE traffic?”
- “which side of the path sees UDP `6081`?”
- “should inner payload diagnosis wait until overlay presence is clear?”

The runtime phases are:

- `send_overlay_packet`
- `receive_overlay_packet`

### `options`

Use `options` when the important first question is:

- “does this GENEVE frame carry option metadata?”
- “could extension TLVs explain forwarding or policy behavior?”
- “is option-bearing traffic directional or missing on one side?”

The runtime phases are:

- `send_optioned_packet`
- `receive_optioned_packet`
