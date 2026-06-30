# Reference: DoT Overlay

Use this page when the operator intent is DNS-over-TLS rather than generic DNS
or generic TLS by themselves.

`dot` is currently modeled as an alias-led overlay:

- canonical family: `dns`
- canonical entry: `tcp`
- transport posture: TLS client setup before query/response confidence

This means the current runtime and protocol shelf treat DoT as:

1. DNS TCP lookup structure
2. plus TLS client setup posture

## What To Read

Read in this order when the question is “is encrypted DNS itself healthy?”:

1. [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
2. [docs/book/reference-dns-tcp-surface.md](docs/book/reference-dns-tcp-surface.md)
3. [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
4. [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

For the broader protocol reading spine, also keep
[docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)
open beside this page.

## What To Look For

In practice, DoT debugging usually splits into two failure shelves:

- resolver-selection or route posture before the query is even framed
- TLS setup and protected-stream delivery before the DNS response arrives

The current `anomaly-flow` hints reflect that split:

- resolver selection problems still look like DNS `tcp` setup posture
- protected reply loss is explained as DNS-over-TLS, not plain DNS-over-TCP

## Alias Contract

The current tree accepts these operator-facing spellings:

- `dot`
- `dns-over-tls`
- `dns_over_tls`

They currently resolve to the DNS `tcp` entry rather than a standalone family.

## Why This Is An Overlay

The `0.15.x` line keeps DoT as a composition of existing stable shelves so we
can preserve one canonical DNS family while still giving encrypted-resolver
traffic a recognizable reading path.
