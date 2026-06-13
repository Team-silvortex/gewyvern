# Reference: DNS TCP Surface

Use this page when you need the current exact lookup surface for TCP-carried
DNS behavior.

## Covered Entries

### `tcp`

- Protocol:
  `dns`
- Aliases:
  `dns-tcp`, `dns_tcp`
- Default entry:
  no

## Operational Shape

The current `tcp` flow models:

1. bind the process and resolve the upstream route
2. send a TCP-carried DNS query toward remote port `53`
3. receive a TCP-carried DNS response

This is the narrowest DNS page to use when you need to distinguish stream-based
DNS query posture from the default UDP lookup path.

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
