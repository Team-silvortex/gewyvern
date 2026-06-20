# Reference: DoH Overlay

Use this page when the operator intent is DNS-over-HTTPS rather than generic
HTTP request/response traffic.

`doh` is currently modeled as an alias-led overlay:

- canonical family: `http`
- canonical entry: `request`
- semantic intent: DNS resolver traffic carried inside HTTP

This means the current runtime and protocol shelf treat DoH as:

1. HTTP request/response structure
2. plus DNS resolver intent carried in the payload path

## What To Read

Read in this order when the question is “is encrypted DNS over HTTP healthy?”:

1. [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
2. [docs/book/reference-http-message-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-message-surface.md)
3. [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
4. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)

For the broader protocol reading spine, also keep
[docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)
open beside this page.

## What To Look For

In practice, DoH debugging usually splits into two layers:

- did the HTTP request reach the intended resolver endpoint correctly?
- did the DNS semantics inside that exchange produce the expected answer?

The current `anomaly-flow` hints reflect that split:

- request emission and upstream selection are described as DNS-over-HTTPS
- response problems are still anchored in the HTTP request/response spine

## Alias Contract

The current tree accepts these operator-facing spellings:

- `doh`
- `dns-over-https`
- `dns_over_https`

They currently resolve to the HTTP `request` entry rather than a standalone
family.

## Why This Is An Overlay

The `0.15.x` line keeps DoH as a composition of existing stable shelves so we
can preserve one canonical HTTP request surface while still exposing encrypted
resolver intent as a first-class reading path.
