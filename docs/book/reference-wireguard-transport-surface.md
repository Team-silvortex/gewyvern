# Reference: WireGuard Transport Surface

Use this page when you need the current exact lookup surface for encrypted
WireGuard transport datagrams after setup.

## Covered Entries

### `transport`

- Protocol:
  `wireguard`
- Aliases:
  `data`, `session`, `wireguard-data`, `wireguard_data`
- Default entry:
  no

## Operational Shape

The current `transport` flow models:

1. bind the process and resolve the upstream route
2. send encrypted transport data
3. receive encrypted transport data

This is the narrowest WireGuard page to use when the tunnel is already carrying
payload traffic and you need the encrypted transport stage rather than the
setup handshake.

For the broader family map, see
[docs/book/reference-wireguard-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-wireguard-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `data`
- `session`
- `wireguard-data`
- `wireguard_data`

<!-- gewyvern:entry-aliases:end -->
