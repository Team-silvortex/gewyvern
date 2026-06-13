# Reference: SMTP Session Surface

Use this page when you need the current exact lookup surface for SMTP session
establishment, greeting, and authentication success.

## Canonical Entries

### `session`

Aliases:

- none registered today

Intent:

- observe control-channel connect
- receive the `220` banner
- send `EHLO`

Coarse response shape:

- process binding
- route resolution
- TCP control-channel state observation
- greeting exchange through banner and `EHLO`

### `auth`

Aliases:

- `login`

Intent:

- perform the `session` flow
- receive `EHLO` success
- send `AUTH`
- receive authentication success (`235`)

Coarse response shape:

- same bind/route/connect scaffolding as `session`
- greeting exchange
- authenticated continuation through `AUTH` and `235`

## Operator Reading Order

Read the current SMTP session family in this order:

1. control-channel connect
2. banner reception (`220`)
3. `EHLO`
4. extension acknowledgement (`250`)
5. optional `AUTH`
6. auth success (`235`)

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `smtp`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md).
