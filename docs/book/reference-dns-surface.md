# Reference: DNS Protocol Surface

Use this page when you want the DNS portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current DNS coverage into two narrower operator-facing
surfaces:

- UDP lookup flow
- TCP query flow

## What This Shelf Covers

The current built-in DNS family models two transport variants for the same
coarse lookup conversation:

- bind the process and resolve the upstream route
- send a DNS query
- receive a DNS response

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- transport-specific lookup posture
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for DNS entry
selection:

- `dot`
- `dns-over-tls`
- `dns_over_tls`

Default entry: `udp`

## DNS Surface Map

### UDP

- [docs/book/reference-dns-udp-surface.md](docs/book/reference-dns-udp-surface.md)
  Datagram-style DNS lookup path.

Typical entries:

- `udp`

### TCP

- [docs/book/reference-dns-tcp-surface.md](docs/book/reference-dns-tcp-surface.md)
  TCP-carried DNS query and response path.

Typical entries:

- `tcp`

## Reading Order

If you are validating current DNS support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
3. the UDP or TCP subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
- For encrypted resolver intent layered on the TCP branch:
  treat `dot` as the DNS `tcp` shelf plus the TLS client handshake reading path
- For the compact DoT reading spine itself:
  [docs/book/reference-dot-overlay.md](docs/book/reference-dot-overlay.md)

## Stability Note

This page is the lookup hub for the current DNS family in the `0.15.x` line.
New DNS transport branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
