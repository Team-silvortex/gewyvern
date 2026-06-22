# Reference: WireGuard Cookie Surface

Use this page when you need the current exact lookup surface for WireGuard
cookie-reply continuation behavior.

## Covered Entries

### `cookie`

- Protocol:
  `wireguard`
- Aliases:
  `cookie-reply`, `wireguard-cookie`, `wireguard_cookie`
- Default entry:
  no

## Operational Shape

The current `cookie` flow models:

1. bind the process and resolve the upstream route
2. send a handshake initiation
3. receive a cookie reply

This is the narrowest WireGuard page to use when the peer is rate-limiting or
asking the sender to continue with cookie-backed validation instead of replying
with a normal handshake response.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `cookie` currently
publishes:

- category:
  `continuation-path`
- operator focus:
  `peer anti-abuse continuation during WireGuard cookie reply evaluation`
- typical signal:
  `Cookie Reply`

For the broader family map, see
[docs/book/reference-wireguard-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-wireguard-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cookie-reply`
- `wireguard-cookie`
- `wireguard_cookie`

<!-- gewyvern:entry-aliases:end -->
