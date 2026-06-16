# Reference: TLS Surface

Read this page after the generic protocol surface when the runtime path is raw
TLS client posture rather than a higher-level application protocol.

Use it for:

- `tls` family lookup
- default entry selection for `client`
- keeping generic handshake/client posture separate from HTTPS, IMAP, or other
  application overlays

Current canonical entries:

- `client` as the default entry

Default entry: `client`

The current line keeps TLS as a compact single-slice family:

- observe or validate client-side TLS setup
- keep the family hub small until the surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-tls-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
