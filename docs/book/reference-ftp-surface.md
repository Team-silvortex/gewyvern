# Reference: FTP Protocol Surface

Use this page when you want the FTP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current FTP coverage into three narrower operator-facing
surfaces:

- session and authentication
- passive-mode transfer flow
- active-mode transfer flow

## What This Shelf Covers

The current built-in FTP family models the control-channel conversation first,
then layers transfer behavior on top of it.

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `session`

## FTP Surface Map

### Session And Authentication

- [docs/book/reference-ftp-session-surface.md](docs/book/reference-ftp-session-surface.md)
  Login/session establishment and explicit authentication failure flow.

Typical entries:

- `session`
- `denied`

### Passive Transfer

- [docs/book/reference-ftp-passive-surface.md](docs/book/reference-ftp-passive-surface.md)
  Passive-mode directory listing, download, and upload.

Typical entries:

- `list`
- `retr`
- `stor`

### Active Transfer

- [docs/book/reference-ftp-active-surface.md](docs/book/reference-ftp-active-surface.md)
  Active-mode directory listing, download, and upload.

Typical entries:

- `active-list`
- `active-retr`
- `active-stor`

## Reading Order

If you are validating current FTP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ftp-surface.md](docs/book/reference-ftp-surface.md)
3. one narrower FTP subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the FTP family in the current `1.4.0` line.
New FTP command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
