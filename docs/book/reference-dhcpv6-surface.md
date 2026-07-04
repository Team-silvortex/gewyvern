# Reference: DHCPv6 Surface

Read this page when an IPv6 host is negotiating, renewing, or releasing lease
state with DHCPv6 infrastructure.

Use it for:

- `dhcpv6` family lookup
- default entry selection for `solicit`
- DHCPv6 client datagrams from UDP port 546 to server port 547
- server replies from UDP port 547 back to client port 546
- protocol aliases such as `dhcpv6-solicit`, `dhcpv6_solicit`,
  `dhcp6-solicit`, `dhcp6_solicit`, `dhcpv6-request`, `dhcpv6_request`,
  `dhcp6-request`, `dhcp6_request`, `dhcpv6-release`, `dhcpv6_release`,
  `dhcp6-release`, and `dhcp6_release`
- entry aliases such as `advertise-probe`, `lease-solicit`, `reply`,
  `lease-request`, `renew`, `lease-release`, and `release-lease`

Current canonical entries:

- `solicit` as the default entry
- `request`
- `release`

Default entry: `solicit`

Operator notes:

- The stable subset keys on DHCPv6 message type byte 0: Solicit `1`,
  Advertise `2`, Request `3`, Reply `7`, and Release `8`.
- It intentionally keeps DHCPv4 and DHCPv6 as separate families so lease
  state, port posture, and IPv6 neighbor context can be debugged without
  overloading the older DHCP surface.
- Future entries should add Renew/Rebind/Confirm only when their runtime IR
  phases have negative checks, not just alias coverage.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-dhcpv6-surface.md](docs/book/reference-dhcpv6-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

<!-- gewyvern:entry-aliases:end -->
