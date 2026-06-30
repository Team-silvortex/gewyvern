# Reference: TLS Signal Surface

Read this page when the question is not “which side owns the TLS stream?” but
“what decisive TLS signal did the stream expose?”

Use it for:

- finding TLS alert records during failed negotiation or shutdown
- correlating visible certificate-chain handshakes with process, socket, and
  route lineage
- keeping byte-stable TLS signals separate from higher-level HTTPS, IMAP, SMTP,
  or proxy overlays

Canonical entries:

- `alert`
- `certificate`

Entry aliases:

- `alert`:
  `alert-record`, `close-notify`, `failure`, `handshake-alert`, `ssl-alert`,
  `ssl_alert`, `tls-alert`, `tls_alert`
- `certificate`:
  `cert`, `cert-chain`, `certificate-chain`, `ssl-certificate`,
  `ssl_certificate`, `tls-certificate`, `tls_certificate`, `x509`, `x509-chain`

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
3. this page
4. [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Alert

`tls/alert` watches TLS record content type `0x15` in either direction. Use it
when TCP establishes but the secure session ends before application bytes become
useful.

Typical operator questions:

- did the peer close with an alert rather than a TCP reset?
- did a local policy, cipher mismatch, or certificate policy cause early
  shutdown?
- which process and route owned the alert-carrying socket?

## Certificate

`tls/certificate` watches a plaintext TLS handshake record with handshake
message type `0x0b`. Use it to confirm that peer identity material was visible
to the packet path.

This is intentionally conservative. TLS 1.3 can encrypt certificate exchange
after early handshake state, and packet fragmentation can split the byte pattern.
Absence of this signal should be read as “not visible from this stable path,”
not “no certificate existed.”

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `alert-record`
- `cert`
- `cert-chain`
- `certificate-chain`
- `close-notify`
- `failure`
- `handshake-alert`
- `ssl-alert`
- `ssl-certificate`
- `ssl_alert`
- `ssl_certificate`
- `tls-alert`
- `tls-certificate`
- `tls_alert`
- `tls_certificate`
- `x509`
- `x509-chain`

<!-- gewyvern:entry-aliases:end -->
