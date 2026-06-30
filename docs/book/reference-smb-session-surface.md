# Reference: SMB Session Surface

Use this page for SMB2 negotiation and session setup on TCP port `445`.

For the broader family map, see
[docs/book/reference-smb-surface.md](docs/book/reference-smb-surface.md).

## Canonical Entries

### `negotiate`

Aliases:

- `smb-negotiate`
- `smb2-negotiate`
- `share-negotiate`

Intent:

- resolve the share host
- observe SMB2 NEGOTIATE
- observe the server negotiate response

### `session`

Aliases:

- `smb-session`
- `smb2-session`
- `session-setup`
- `share-session`

Intent:

- resolve the share host
- observe SMB2 SESSION_SETUP
- observe the server session response

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `session-setup`
- `share-negotiate`
- `share-session`
- `smb-negotiate`
- `smb-session`
- `smb2-negotiate`
- `smb2-session`

<!-- gewyvern:entry-aliases:end -->
