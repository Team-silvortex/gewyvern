# Reference: L2TP Protocol Surface

L2TP support gives gewyvern a tunnel view over UDP port `1701`. Use it when the
debugging question is whether a layer-2 tunnel is being negotiated or whether
payload-like session packets are flowing after tunnel setup.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-l2tp-tunnel-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-l2tp-tunnel-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

IPsec is often nearby in deployed VPN stacks, but L2TP keeps a separate tunnel
surface for control and session visibility.

## Entries

Default entry: `control`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `control` | L2TP tunnel control traffic on UDP port `1701` | UDP/1701 packet with L2TP control flags |
| `session` | L2TP data session traffic after tunnel setup | UDP/1701 packet without the control flag |

## Operator Notes

- `control` answers whether tunnel negotiation or maintenance is visible.
- `session` answers whether tunneled data-like packets are moving after setup.
- L2TP is often paired with IPsec in deployed VPNs; inspect IPsec surfaces when
  L2TP control is present but payload trust or protection is unclear.

## Aliases

- `control`: `l2tp-control`, `l2tp_control`, `l2tp-tunnel`, `l2tp_tunnel`
- `session`: `l2tp-session`, `l2tp_session`, `l2tp-data`, `l2tp_data`
