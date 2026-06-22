# Reference: SMTP Data Surface

Use this page when you need the current exact lookup surface for SMTP message
body submission, queue success, and post-body denial.

## Canonical Entries

### `data`

Aliases:

- `message`

Intent:

- complete the accepted envelope flow
- send `DATA`
- receive data-ready (`354`)
- send the message body terminator
- receive queue success (`250 2.0.0`)

### `data-denied`

Aliases:

- `message-denied`

Intent:

- complete the accepted envelope flow
- send `DATA`
- receive data-ready (`354`)
- send the message body terminator
- receive post-body denial (`550`)

## Shared Response Shape

Both message-submission entries currently share the same broad staging model:

1. process binding
2. route resolution
3. TCP control-channel connect
4. banner and `EHLO`
5. auth request and auth success
6. `MAIL FROM` and sender acceptance
7. `RCPT TO` and recipient acceptance
8. `DATA` and data-ready (`354`)
9. message-body terminator transmission
10. terminal queue success or denial

The current models differ only in the final server decision:

- `data` ends on queued/successful acceptance
- `data-denied` ends on message rejection after body handoff

## Machine-Readable Surface Semantics

The `protocol_surface("smtp", "data-denied")` contract now publishes
`entry_semantics` so tooling can identify post-body rejection as a structured
server decision instead of a generic transfer failure.

Current denial semantics:

- `category = failure-path`
- `operator_focus = message rejection after SMTP body handoff`
- `typical_signal = 550`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

If you are reviewing SMTP message-submission coverage, read it in this order:

1. `rcpt`
2. `data`
3. `data-denied`

That sequence keeps envelope acceptance in front of the body-submission branch.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `smtp` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `message`
- `message-denied`

<!-- gewyvern:entry-aliases:end -->
