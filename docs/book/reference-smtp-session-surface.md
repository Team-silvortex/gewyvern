# Reference: SMTP Session Surface

Use this page when you need the current exact lookup surface for SMTP session
establishment, greeting, and authentication outcomes.

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

### `auth-denied`

Aliases:

- `login-denied`

Intent:

- perform the `session` flow
- receive `EHLO` success
- send `AUTH`
- receive authentication denial (`535`)

Coarse response shape:

- same bind/route/connect scaffolding as `session`
- greeting exchange
- authenticated continuation through `AUTH` and terminal denial (`535`)

## Machine-Readable Surface Semantics

The `protocol_surface("smtp", "auth-denied")` contract now publishes
`entry_semantics` so tooling can classify explicit SMTP auth rejection before
the envelope phase begins.

Current denial semantics:

- `category = failure-path`
- `operator_focus = smtp authentication rejection after explicit AUTH exchange`
- `typical_signal = 535`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

Read the current SMTP session family in this order:

1. control-channel connect
2. banner reception (`220`)
3. `EHLO`
4. extension acknowledgement (`250`)
5. optional `AUTH`
6. auth success (`235`) or auth denial (`535`)

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `smtp`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `login`
- `login-denied`

<!-- gewyvern:entry-aliases:end -->
