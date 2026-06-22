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

## Machine-Readable Surface Semantics

The `protocol_surface("ftp", "denied")` contract now publishes
`entry_semantics` so tooling can classify explicit login rejection separately
from short sessions or transport failure.

Current denial semantics:

- `category = failure-path`
- `operator_focus = ftp login rejection after USER/PASS credential exchange`
- `typical_signal = 530`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

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

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `control`
- `login`
- `login-denied`

<!-- gewyvern:entry-aliases:end -->
