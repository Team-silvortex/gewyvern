# Reference: GRE Protocol Surface

GRE support gives gewyvern a first tunnel-oriented view over IP protocol 47
traffic. Use it when a packet path looks like raw encapsulation rather than a
normal TCP or UDP application session.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-gre-tunnel-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gre-tunnel-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Entries

Default entry: `encap`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `encap` | GRE tunnel encapsulation on IP protocol 47 carrying an inner payload | IP protocol 47 GRE packet |
| `keepalive` | minimal GRE keepalive-style liveness probe on a tunnel path | GRE flags/version prefix `0x0000` |

## Operator Notes

- `encap` is intentionally broad: it marks IP protocol 47 traffic as tunnel
  posture before trying to infer the inner payload.
- `keepalive` narrows on a minimal GRE flags/version prefix, which is useful
  for separating tunnel liveness from ordinary encapsulated traffic.
- GRE should be read as a tunnel context layer. If the inner payload becomes
  observable later, follow that payload into its own protocol surface.

## Aliases

- `encap`: `encapsulation`, `gre-tunnel`, `gre_tunnel`, `tunnel`
- `keepalive`: `gre-keepalive`, `gre_keepalive`, `keep-alive`, `tunnel-keepalive`
