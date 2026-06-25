# Reference: IPsec Security Surface

This shelf groups IPsec entries that describe secure network-layer packet
posture.

Read this alongside:

- [docs/book/reference-ipsec-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ipsec-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `security`
- label: `Security`
- entries: `esp`, `ah`

## Entries

### `esp`

Use `esp` when the important first question is:

- “is ESP traffic visible in this direction?”
- “is the protected path present before decryptable payload exists?”
- “does route posture line up with observed IP protocol 50 packets?”

The runtime phases are:

- `resolve_secure_path`
- `send_esp_packet`
- `receive_esp_packet`

### `ah`

Use `ah` when the important first question is:

- “is AH authentication-header traffic visible?”
- “is authentication posture present without assuming ESP payload privacy?”
- “does one direction see IP protocol 51 while the other does not?”

The runtime phases are:

- `resolve_authenticated_path`
- `send_ah_packet`
- `receive_ah_packet`

## Boundary

This first IPsec layer does not infer security association ownership. It gives
operators a stable outer protocol surface so future SPI and sequence analysis
can attach without changing the family contract.


<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `ah`
- `auth-header`
- `authenticated-header`
- `esp`
- `ipsec-ah`
- `ipsec-esp`
- `ipsec_ah`
- `ipsec_esp`
- `secure-encapsulation`

<!-- gewyvern:entry-aliases:end -->
