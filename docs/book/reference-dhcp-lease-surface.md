# Reference: DHCP Lease Surface

Read this page when the path is clearly in lease acquisition or renewal.

Canonical entries covered here:

- `discover`
- `nak`
- `request`

Current accepted aliases:

- `offer-probe`
- `lease-discover`
- `offer-denied`
- `lease-denied`
- `lease-request`
- `renew`

Operational split:

- `discover` probes for available offers
- `nak` records an explicit server refusal of the requested lease state
- `request` claims, renews, or confirms a lease

Protocol package aliases also remain accepted:

- `dhcp-discover`
- `dhcp_discover`
- `dhcp-request`
- `dhcp_request`
- `dhcp-nak`
- `dhcp_nak`

Return to the family hub:

- [docs/book/reference-dhcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-surface.md)
