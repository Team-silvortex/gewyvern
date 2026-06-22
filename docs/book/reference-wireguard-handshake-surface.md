# Reference: WireGuard Handshake Surface

Use this page when you need the current exact lookup surface for WireGuard
handshake establishment.

## Covered Entries

### `handshake`

- Protocol:
  `wireguard`
- Aliases:
  none
- Default entry:
  yes

## Operational Shape

The current `handshake` flow models:

1. bind the process and resolve the upstream route
2. send a handshake initiation
3. receive a handshake response

This is the narrowest WireGuard page to use when you want the base peer
establishment path before any cookie throttling or encrypted transport payloads.

For the broader family map, see
[docs/book/reference-wireguard-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-wireguard-surface.md).
