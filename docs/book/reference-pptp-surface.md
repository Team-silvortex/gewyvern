# Reference: PPTP Protocol Surface

PPTP support gives gewyvern a legacy VPN tunnel view with a TCP control channel
on port `1723` and GRE-carried data. Use it when a path looks like old VPN
control-plane traffic plus GRE payload movement.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-pptp-tunnel-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pptp-tunnel-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

GRE remains the data-carrier context for PPTP, while this page owns the PPTP
control/data family split.

## Entries

Default entry: `control`.

| Entry | Focus | Typical Signal |
| --- | --- | --- |
| `control` | PPTP control channel traffic on TCP port `1723` | TCP/1723 PPTP control message |
| `data` | PPTP data traffic carried over GRE | IP protocol 47 GRE data packet |

## Operator Notes

- `control` checks whether PPTP control negotiation is present.
- `data` intentionally reuses GRE posture rather than trying to replace GRE
  analysis.
- If control appears but data does not, inspect firewall/NAT handling for GRE.

## Aliases

- `control`: `pptp-control`, `pptp_control`, `pptp-tunnel`, `pptp_tunnel`
- `data`: `pptp-data`, `pptp_data`, `pptp-gre`, `pptp_gre`
