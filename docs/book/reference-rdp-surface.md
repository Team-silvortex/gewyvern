# Reference: RDP Protocol Surface

Use this page when Remote Desktop traffic should be treated as a remote-access
session instead of a generic TCP/3389 flow.

Default entry: `connect`

Protocol aliases: `desktop-channel`, `desktop-connect`, `rdp-channel`,
`rdp-connect`, `rdp-data`, `x224-connect`

Read this alongside:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- one narrower RDP subpage
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## What This Shelf Covers

The current RDP family models:

- TPKT/X.224 connection establishment
- data TPDU channel traffic after setup

The matcher is intentionally lightweight and suitable for early debugger
classification before a full RDP decoder exists.

## RDP Surface Map

### Connect

- [docs/book/reference-rdp-connect-surface.md](docs/book/reference-rdp-connect-surface.md)
  X.224 connection request and confirmation.

Typical entries:

- `connect`

### Channel

- [docs/book/reference-rdp-channel-surface.md](docs/book/reference-rdp-channel-surface.md)
  Data TPDU channel traffic.

Typical entries:

- `channel`
