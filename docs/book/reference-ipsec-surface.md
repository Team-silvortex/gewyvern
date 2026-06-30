# Reference: IPsec Protocol Surface

IPsec support gives gewyvern a first secure-path view over ESP and AH traffic.
Use it when the visible packet is a protected network-layer path rather than a
plain application session.

Read this alongside:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-ipsec-security-surface.md](docs/book/reference-ipsec-security-surface.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Entries

Default entry: `esp`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `esp` | encrypted or integrity-protected IPsec ESP traffic | IP protocol 50 ESP packet |
| `ah` | authenticated IPsec AH traffic | IP protocol 51 AH packet |

## Operator Notes

- `esp` is the default because it is the common encrypted IPsec data-plane
  surface encountered during tunnel and transport-mode debugging.
- `ah` is tracked separately because it preserves an authentication-header
  posture without the same payload confidentiality assumption as ESP.
- This surface intentionally starts at outer protocol visibility. SPI, sequence,
  SA ownership, and replay-window interpretation should layer on top later.

## Aliases

- `esp`: `esp`, `ipsec-esp`, `ipsec_esp`, `secure-encapsulation`
- `ah`: `ah`, `ipsec-ah`, `ipsec_ah`, `auth-header`, `authenticated-header`

