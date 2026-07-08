# Reference: RIP Surface

Read this page when a route appears or disappears through Routing Information
Protocol traffic.

Use it for:

- `rip` family lookup
- default entry selection for `request`
- UDP RIP datagrams on port 520
- RIP v2 request frames with command `1`
- RIP v2 response frames with command `2`
- route withdrawal or unreachable announcements using metric `16`
- protocol aliases such as `rip-request`, `rip_request`, `rip-response`,
  `rip_response`, `rip-update`, `rip_update`, `rip-unreachable`,
  `rip_unreachable`, `rip-withdrawal`, and `rip_withdrawal`
- entry aliases such as `route-request`, `routing-request`, `route-update`,
  `distance-vector-update`, `metric16`, `route-unreachable`, and
  `route-withdrawal`

Current canonical entries:

- `request` as the default entry
- `response`
- `unreachable`

Default entry: `request`

Operator notes:

- Treat this as a routing-control surface, not as generic UDP traffic. A host
  can look healthy at the socket layer while still learning an unwanted
  distance-vector update.
- The stable subset tracks RIP v2 command bytes and a first-entry metric low
  byte of `16` for unreachable routes.
- If a path flaps, read `response` and `unreachable` together before blaming
  transport loss.
- Full route prefix extraction is intentionally deferred until the protocol IR
  can carry route table deltas directly.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-rip-surface.md](docs/book/reference-rip-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

<!-- gewyvern:entry-aliases:end -->
