# Reference: DNS Error Surface

Use this page when a DNS lookup reached a resolver but the resolver answered
with a failure code.

## Covered Entries

### `error`

- Protocol:
  `dns`
- Aliases:
  `dns-error`, `dns_error`, `formerr`, `nxdomain`, `refused`,
  `resolution-failed`, `resolution_failed`, `servfail`
- Default entry:
  no

### `tcp-error`

- Protocol:
  `dns`
- Aliases:
  `dns-tcp-error`, `dns_tcp_error`, `tcp-formerr`, `tcp-nxdomain`,
  `tcp-refused`, `tcp-servfail`
- Default entry:
  no

## Operational Shape

The current error flows model:

1. bind the process and resolve the upstream route
2. observe a DNS response from the resolver
3. identify a non-success response code for common failure classes

Use `error` for UDP DNS and `tcp-error` for TCP-carried DNS. They are split
because the runtime hooks for datagram and stream packet metadata are different
and should not compete inside one template.

The stable subset currently recognizes:

- `FORMERR` (`rcode 1`)
- `SERVFAIL` (`rcode 2`)
- `NXDOMAIN` (`rcode 3`)
- `REFUSED` (`rcode 5`)

This page is intentionally a debugger surface, not a full DNS decoder. It is
meant to answer “did resolution fail at the DNS response layer?” before deeper
resolver-specific analysis.

## Operator Reading Order

Read this page after the DNS family hub when:

- TCP or UDP transport is present but the application still cannot resolve a
  name
- you need to distinguish resolver failure from socket, route, or timeout
  failure
- you want a breakpoint for negative lookup answers such as `NXDOMAIN`
- you need to keep UDP and TCP resolver failures distinct

For the broader family map, see
[docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `dns-error`
- `dns-tcp-error`
- `dns_error`
- `dns_tcp_error`
- `formerr`
- `nxdomain`
- `refused`
- `resolution-failed`
- `resolution_failed`
- `servfail`
- `tcp-formerr`
- `tcp-nxdomain`
- `tcp-refused`
- `tcp-servfail`

<!-- gewyvern:entry-aliases:end -->
