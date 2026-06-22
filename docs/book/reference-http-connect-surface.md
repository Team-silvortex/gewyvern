# Reference: HTTP CONNECT Surface

Use this page when you need the current exact lookup surface for plain HTTP
`CONNECT` tunnel establishment and denial.

## Canonical Entries

### `connect`

Aliases:

- none registered at the entry level today

Intent:

- open a proxy-side socket
- send an HTTP `CONNECT` request
- receive tunnel-established success (`200`)

### `denied`

Aliases:

- none registered at the entry level today

Intent:

- open a proxy-side socket
- send an HTTP `CONNECT` request
- receive tunnel denial (`403`)

## Shared Response Shape

Both entries currently share the same broad staging model:

1. process binding
2. route resolution
3. proxy socket connect
4. `CONNECT` request send
5. terminal success or denial response

The branch point is the terminal proxy decision:

- `connect` ends on tunnel-established success (`200`)
- `denied` ends on tunnel denial (`403`)

## Machine-Readable Surface Semantics

The `protocol_surface("http", "denied")` contract now publishes
`entry_semantics` so downstream tooling can distinguish CONNECT policy refusal
from transport setup failures.

Current denial semantics:

- `category = failure-path`
- `operator_focus = proxy tunnel refusal after CONNECT policy evaluation`
- `typical_signal = 403`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

If you are reviewing plain CONNECT coverage, read it in this order:

1. `connect`
2. `denied`

That sequence keeps the success path visible before the denial branch.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `http` family resolution
- canonical entry normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `http-connect`
- `http-connect-denied`
- `http_connect`
- `http_connect_denied`

<!-- gewyvern:entry-aliases:end -->
