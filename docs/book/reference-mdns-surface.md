# Reference: mDNS Surface

Read this page after the generic protocol surface when the runtime path looks
like local-link multicast name discovery rather than a generic UDP datagram.

Use it for:

- `mdns` family lookup
- default entry selection for `query`
- response and probe entry selection when the local-link discovery direction is
  already known
- keeping multicast discovery lookups separate from unicast DNS pages

Current canonical entries:

- `query` as the default entry
- `response` with entry aliases `answer`, `announcement`, `mdns-response`, and
  `mdns_response`
- `probe` with entry aliases `claim`, `conflict-check`, `mdns-probe`, and
  `mdns_probe`

Default entry: `query`

The current line treats mDNS as a compact local-link discovery cluster:

- `query` for active multicast name lookup
- `response` for answer or announcement traffic
- `probe` for name-conflict probing before claiming or advertising a name

Operator rule:

- use `query` when a host is asking for local names
- use `response` when you are reading responder behavior or announcements
- use `probe` when the interesting question is whether a host is checking for
  local name conflicts before publishing

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-mdns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mdns-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
