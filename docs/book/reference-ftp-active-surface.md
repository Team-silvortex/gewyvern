# Reference: FTP Active Transfer Surface

Use this page when you need the current exact lookup surface for active-mode
FTP transfer flows.

## Canonical Entries

### `active-list`

Aliases:

- `active-directory`

Intent:

- authenticate on the FTP control channel
- send `PORT`
- receive active transfer readiness
- send `LIST`
- observe transfer-open and transfer-complete replies

### `active-retr`

Aliases:

- `active-download`

Intent:

- authenticate on the FTP control channel
- send `PORT`
- receive active transfer readiness
- send `RETR`
- observe transfer-open and transfer-complete replies

### `active-stor`

Aliases:

- `active-upload`

Intent:

- authenticate on the FTP control channel
- send `PORT`
- receive active transfer readiness
- send `STOR`
- observe transfer-open and transfer-complete replies

## Shared Response Shape

All active entries currently share the same broad staging model:

1. process binding
2. route resolution
3. TCP control-channel connect
4. banner and auth exchange
5. `PORT` request and active-ready reply (`200`)
6. transfer verb (`LIST`, `RETR`, or `STOR`)
7. transfer-open reply (`150`)
8. transfer-complete reply (`226`)

## Operator Reading Order

If you are reviewing active FTP coverage, read it in this order:

1. `session`
2. `active-list`
3. `active-retr`
4. `active-stor`

That sequence keeps the shared control-channel and `PORT` negotiation context
ahead of the transfer verbs themselves.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `ftp` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `active-directory`
- `active-download`
- `active-upload`

<!-- gewyvern:entry-aliases:end -->
