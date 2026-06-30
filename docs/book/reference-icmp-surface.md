# Reference: ICMP Surface

Read this page after the generic protocol surface when the runtime path looks
like reachability probing or an explicit network/path failure signal.

Use it for:

- `icmp` family lookup
- default entry selection for `echo`
- package aliases such as `icmp-echo`, `icmp_echo`, `ping`,
  `icmp-unreachable`, and `icmp_unreachable`
- separating active reachability probes from returned path-failure diagnostics

Primary subpages:

- [docs/book/reference-icmp-echo-surface.md](docs/book/reference-icmp-echo-surface.md)
- [docs/book/reference-icmp-failure-surface.md](docs/book/reference-icmp-failure-surface.md)

Current canonical entries:

- `echo` as the default entry
- `unreachable`

Default entry: `echo`

Operator rule:

- use `echo` when the question is whether a peer can be reached and replies
  are observed
- use `unreachable` when the important signal is a returned ICMP type 3
  failure, such as destination, host, network, or port unreachable

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-icmp-surface.md](docs/book/reference-icmp-surface.md)
3. one exact ICMP subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
