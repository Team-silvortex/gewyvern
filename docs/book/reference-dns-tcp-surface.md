# Reference: DNS TCP Surface

Use this page when you need the current exact lookup surface for TCP-carried
DNS behavior.

## Covered Entries

### `tcp`

### `dot`

- Protocol:
  `dns`
- Aliases:
  `dns-over-tls`, `dns-tcp`, `dns_over_tls`, `dns_tcp`, `dot`
- Default entry:
  no

## Operational Shape

The current `tcp` flow models:

1. bind the process and resolve the upstream route
2. send a TCP-carried DNS query toward remote port `53`
3. receive a TCP-carried DNS response

This is the narrowest DNS page to use when you need to distinguish stream-based
DNS query posture from the default UDP lookup path.

The same custom shelf also covers the encrypted-resolver alias family that
selects the TCP path first:

- `dot`
- `dns-over-tls`
- `dns_over_tls`

## Operator Reading Order

Read this page after the DNS family hub when:

- you want to validate non-default `tcp` entry resolution
- you need to distinguish TCP query behavior from UDP datagrams
- you care about transport posture before IR lowering

## Stability Notes

The current entry records coarse TCP query/response behavior and does not try
to model zone transfer or longer multi-message DNS conversations.

For the broader family map, see
[docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `dns-tcp`
- `dns-over-tls`
- `dns_over_tls`
- `dns_tcp`
- `dot`

<!-- gewyvern:entry-aliases:end -->
