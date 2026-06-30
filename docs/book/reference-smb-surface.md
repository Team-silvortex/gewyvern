# Reference: SMB Protocol Surface

Use this page when Windows or Samba file-share traffic needs a protocol-shaped
debugging path instead of plain TCP/445.

Default entry: `negotiate`

Protocol aliases: `session-setup`, `share-connect`, `share-negotiate`,
`share-session`, `smb-negotiate`, `smb-session`, `smb-tree`,
`smb2-negotiate`, `smb2-session`, `smb2-tree`, `tree-connect`

Read this alongside:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- one narrower SMB subpage
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## What This Shelf Covers

The current SMB family models the first useful SMB2 surfaces:

- dialect negotiation
- session setup
- tree connect into a concrete share

The matcher uses lightweight SMB2 direct-TCP command hints. It is not a full
SMB decoder yet, but it gives the debugger a stable file-share lifecycle.

## SMB Surface Map

### Session

- [docs/book/reference-smb-session-surface.md](docs/book/reference-smb-session-surface.md)
  Negotiate and session setup.

Typical entries:

- `negotiate`
- `session`

### Share

- [docs/book/reference-smb-share-surface.md](docs/book/reference-smb-share-surface.md)
  Tree connect into a share.

Typical entries:

- `tree`
