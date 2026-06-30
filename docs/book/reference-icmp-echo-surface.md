# Reference: ICMP Echo Surface

Read this page when the selected ICMP entry is `echo`.

This surface is for classic reachability probes:

- an outbound ICMP echo request, type `8`
- an inbound ICMP echo reply, type `0`

Canonical entry:

- `echo`

Entry aliases:

- `echo-request`
- `echo-reply`
- `ping-check`

Package aliases:

- `icmp-echo`
- `icmp_echo`
- `ping`

Operator interpretation:

- `send_echo_request` means the local runtime observed an ICMP request
- `receive_echo_reply` means a reply returned before the window closed
- missing `receive_echo_reply` is a reachability gap, not proof of a remote
  service failure

Read this alongside:

- [docs/book/reference-icmp-surface.md](docs/book/reference-icmp-surface.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `echo-reply`
- `echo-request`
- `icmp-echo`
- `icmp_echo`
- `ping`
- `ping-check`

<!-- gewyvern:entry-aliases:end -->
