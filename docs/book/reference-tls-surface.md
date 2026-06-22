# Reference: TLS Surface

Read this page after the generic protocol surface when the runtime path is raw
TLS client posture rather than a higher-level application protocol.

Use it for:

- `tls` family lookup
- default entry selection for `client`
- choosing between client and server handshake posture
- keeping generic handshake/client posture separate from HTTPS, IMAP, or other
  application overlays
- using `reading_companions` to decide whether the next shelf is `https
  connect`, `dns tcp`, or another higher-level overlay-led path

Current canonical entries:

- `client` as the default entry
- `server`

Default entry: `client`

The current line now splits TLS into two narrower handshake-facing shelves:

- [docs/book/reference-tls-client-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-client-surface.md)
  for outbound/client-initiated setup
- [docs/book/reference-tls-server-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-server-surface.md)
  for inbound/server-side accept-and-reply posture

Protocol aliases: none.

Entry aliases now include:

- `client`:
  `initiator`, `tls-client`, `tls_client`
- `server`:
  `acceptor`, `tls-server`, `tls_server`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-tls-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-surface.md)
3. one exact TLS subpage
4. the companion surface named by `reading_companions` when the selected entry is `client`
5. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

The exact companion contract is documented in:

- [docs/book/reference-protocol-reading-companions.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-companions.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
