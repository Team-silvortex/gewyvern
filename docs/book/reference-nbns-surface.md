# Reference: NBNS Surface

Read this page when a host uses legacy NetBIOS Name Service traffic for
local-network name discovery.

Use it for:

- `nbns` family lookup
- default entry selection for `query`
- UDP NBNS datagrams on port 137
- query frames with DNS-style QR bit cleared
- response frames with DNS-style QR bit set
- negative responses with DNS-style rcode values for name error or refused
- protocol aliases such as `nbns-query`, `nbns_query`,
  `netbios-name-query`, `nbns-response`, `nbns_response`,
  `netbios-name-response`, `nbns-negative`, `nbns_negative`, and
  `netbios-name-negative`
- entry aliases such as `name-query`, `netbios-query`, `name-answer`,
  `netbios-answer`, `name-negative`, `name-not-found`, and
  `netbios-not-found`

Current canonical entries:

- `query` as the default entry
- `response`
- `negative`

Default entry: `query`

Operator notes:

- Treat NBNS as legacy local-name discovery. It is especially useful when a
  Windows-style service path works by host name on one segment but fails across
  routed or hardened environments.
- The stable subset uses header bits only: QR `0` for query, QR `1` for
  response, and QR `1` plus rcode `3` or `5` for negative answers.
- Prefer DNS, mDNS, and LLMNR surfaces when they are present; use NBNS to
  explain older fallback behavior or unexpected local broadcast chatter.
- Payload name decoding is intentionally deferred until the protocol IR can
  represent queried NetBIOS names without overloading packet predicates.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-nbns-surface.md](docs/book/reference-nbns-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

<!-- gewyvern:entry-aliases:end -->
