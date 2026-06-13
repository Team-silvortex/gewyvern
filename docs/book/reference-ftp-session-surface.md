# Reference: FTP Session Surface

Use this page when you need the current exact lookup surface for FTP session
establishment and authentication outcomes.

## Canonical Entries

### `session`

Aliases:

- `login`
- `control`

Intent:

- observe control-channel connect
- receive the `220` banner
- send `USER`
- receive the `331` password challenge
- send `PASS`
- receive the `230` success response

Coarse response shape:

- process binding
- route resolution
- TCP control-channel state observation
- request/response payload phases for banner and auth exchange

### `denied`

Aliases:

- `login-denied`

Intent:

- observe the same setup flow as `session`
- finish on explicit authentication denial instead of success

Coarse response shape:

- same bind/route/connect scaffolding as `session`
- `USER` and `PASS` exchange
- terminal `530` denial response

## Operator Reading Order

Read the current FTP session family in this order:

1. control-channel connect
2. banner reception
3. `USER`
4. password-required reply
5. `PASS`
6. terminal success (`230`) or deny (`530`)

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `ftp`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md).
