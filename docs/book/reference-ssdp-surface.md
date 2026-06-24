# Reference: SSDP Surface

Read this page after the generic protocol surface when the runtime path looks
like local service discovery traffic rather than an arbitrary UDP exchange.

Use it for:

- `ssdp` family lookup
- default entry selection for `discovery`
- `notify` entry selection for device advertisements
- keeping device and service advertisement discovery distinct from HTTP control

Current canonical entries:

- `discovery` as the default entry
- `notify` with entry aliases `advertise`, `alive`, `byebye`, `ssdp-notify`,
  and `ssdp_notify`

Default entry: `discovery`

The current line treats SSDP as a compact local service-discovery cluster:

- `discovery` for active `M-SEARCH` plus HTTP-style response traffic
- `notify` for passive `NOTIFY` advertisement, alive, and byebye traffic

Operator rule:

- use `discovery` when the host is actively asking the multicast group for
  services
- use `notify` when the service is announcing availability, removal, or state

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ssdp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssdp-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
