# Reference: FTP Passive Transfer Surface

Use this page when you need the current exact lookup surface for passive-mode
FTP transfer flows.

## Canonical Entries

### `list`

Aliases:

- `directory`

Intent:

- authenticate on the FTP control channel
- send `PASV`
- receive passive transfer readiness
- send `LIST`
- observe transfer-open and transfer-complete replies

### `retr`

Aliases:

- `download`

Intent:

- authenticate on the FTP control channel
- enter passive mode with `PASV`
- send `RETR`
- observe transfer-open and transfer-complete replies

### `stor`

Aliases:

- `upload`

Intent:

- authenticate on the FTP control channel
- enter passive mode with `PASV`
- send `STOR`
- observe transfer-open and transfer-complete replies

## Shared Response Shape

All passive entries currently share the same broad staging model:

1. process binding
2. route resolution
3. TCP control-channel connect
4. banner and auth exchange
5. `PASV` request and passive-ready reply (`227`)
6. transfer verb (`LIST`, `RETR`, or `STOR`)
7. transfer-open reply (`150`)
8. transfer-complete reply (`226`)

## Operator Reading Order

If you are reviewing passive FTP coverage, read it in this order:

1. `session`
2. `list`
3. `retr`
4. `stor`

That sequence keeps the common auth + passive negotiation context in front of
the actual transfer verb.

## Validation Surface

This surface is intended to validate and lower through the same built-in
registry path as other protocol families:

- `ftp` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md).
