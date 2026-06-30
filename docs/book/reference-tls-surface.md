# Reference: TLS Surface

Read this page after the generic protocol surface when the runtime path is raw
TLS client posture rather than a higher-level application protocol.

Use it for:

- `tls` family lookup
- default entry selection for `client`
- choosing between client, server, alert, and certificate handshake posture
- keeping generic handshake/client posture separate from HTTPS, IMAP, or other
  application overlays
- using `reading_companions` to decide whether the next shelf is `https
  connect`, `dns tcp`, or another higher-level overlay-led path

Current canonical entries:

- `client` as the default entry
- `server`
- `alert`
- `certificate`

Default entry: `client`

The current line now splits TLS into role shelves plus a signal shelf:

- [docs/book/reference-tls-client-surface.md](docs/book/reference-tls-client-surface.md)
  for outbound/client-initiated setup
- [docs/book/reference-tls-server-surface.md](docs/book/reference-tls-server-surface.md)
  for inbound/server-side accept-and-reply posture
- [docs/book/reference-tls-signal-surface.md](docs/book/reference-tls-signal-surface.md)
  for alert records and plaintext certificate handshake signals

Protocol aliases: none.

Entry aliases now include:

- `alert`:
  `alert-record`, `close-notify`, `failure`, `handshake-alert`, `ssl-alert`,
  `ssl_alert`, `tls-alert`, `tls_alert`
- `certificate`:
  `cert`, `cert-chain`, `certificate-chain`, `ssl-certificate`,
  `ssl_certificate`, `tls-certificate`, `tls_certificate`, `x509`, `x509-chain`
- `client`:
  `initiator`, `tls-client`, `tls_client`
- `server`:
  `acceptor`, `tls-server`, `tls_server`

TLS signal entries intentionally stay byte-stable. `alert` follows TLS record
content type `0x15`; `certificate` follows plaintext handshake message type
`0x0b` behind a TLS handshake record. Modern TLS can encrypt or fragment
certificate material, so a missing certificate signal is not proof that no
certificate was exchanged.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
3. one exact TLS subpage
4. the companion surface named by `reading_companions` when the selected entry is `client`
5. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

The exact companion contract is documented in:

- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
