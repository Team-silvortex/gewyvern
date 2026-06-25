# Reference: GRE Tunnel Surface

This shelf groups GRE entries that describe tunnel posture and tunnel liveness.

Read this alongside:

- [docs/book/reference-gre-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gre-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Shelf

- key: `tunnel`
- label: `Tunnel`
- entries: `encap`, `keepalive`

## Entries

### `encap`

Use `encap` when the important first question is:

- “is this host sending or receiving GRE traffic at all?”
- “which side of the path is seeing IP protocol 47?”
- “is the tunnel posture present before inner payload analysis begins?”

The runtime phases are:

- `send_encapsulated_packet`
- `receive_encapsulated_packet`

### `keepalive`

Use `keepalive` when the important first question is:

- “is this tunnel only exchanging liveness-style probes?”
- “are keepalive signals directional or missing on one side?”
- “should I refresh tunnel status before blaming the inner protocol?”

The runtime phases are:

- `send_keepalive`
- `receive_keepalive`

## Boundary

GRE does not replace inner protocol analysis. It gives the debugger a stable
outer tunnel frame so later stages can attach payload-specific interpretation
without losing the encapsulation context.


<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `encapsulation`
- `gre-keepalive`
- `gre-tunnel`
- `gre_keepalive`
- `gre_tunnel`
- `keep-alive`
- `tunnel`
- `tunnel-keepalive`

<!-- gewyvern:entry-aliases:end -->
