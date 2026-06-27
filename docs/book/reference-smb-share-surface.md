# Reference: SMB Share Surface

Use this page for SMB2 tree-connect traffic after a session exists.

For the broader family map, see
[docs/book/reference-smb-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smb-surface.md).

## Canonical Entry

### `tree`

Aliases:

- `smb-tree`
- `smb2-tree`
- `tree-connect`
- `share-connect`

Intent:

- resolve the share host
- observe SMB2 TREE_CONNECT
- observe the tree-connect response

## Operator Note

If the share path is unknown or authentication may be failing, read the session
surface first, then return here once negotiation and session setup are visible.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `share-connect`
- `smb-tree`
- `smb2-tree`
- `tree-connect`

<!-- gewyvern:entry-aliases:end -->
