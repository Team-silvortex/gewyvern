# Reference: ICMP Failure Surface

Read this page when the selected ICMP entry is `unreachable`.

This surface is for returned path-failure diagnostics:

- inbound ICMP unreachable, type `3`
- destination, host, network, or port unreachable style failures

Canonical entry:

- `unreachable`

Entry aliases:

- `dest-unreachable`
- `destination-unreachable`
- `port-unreachable`
- `host-unreachable`
- `net-unreachable`

Package aliases:

- `icmp-unreachable`
- `icmp_unreachable`

Operator interpretation:

- `receive_unreachable` means the network or peer returned an explicit
  reachability failure signal
- this is stronger than silence, because the path produced a concrete ICMP
  response
- the current surface does not yet split ICMP code values into separate
  canonical entries; keep code-level analysis as a drill-down concern

Read this alongside:

- [docs/book/reference-icmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmp-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `dest-unreachable`
- `destination-unreachable`
- `host-unreachable`
- `icmp-unreachable`
- `icmp_unreachable`
- `net-unreachable`
- `port-unreachable`

<!-- gewyvern:entry-aliases:end -->
