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
- observe an explicit queue candidate (`250 2.0.0 Ok: queued as `)

### `data-denied`

Aliases:

- `message-denied`

Intent:

- complete the accepted envelope flow
- send `DATA`
- receive data-ready (`354`)
- send the message body terminator
- observe an explicit message-rejection candidate (`550 5.7.1 Message rejected`)

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

The intended models differ in the final server decision:

- `data` ends on queued/successful acceptance
- `data-denied` ends on message rejection after body handoff

The packet rules do not enforce this command sequence. A bare `250 2.0.0` is
also a valid NOOP/RSET response, and a bare `550` may reject a recipient before
DATA. Neither is enough for a message outcome. Rules now fail closed unless the
case-sensitive English prefixes above are observed. These are conservative
candidates, not complete SMTP decoders: other languages, reply spellings,
single-line EHLO replies and split prefixes may remain unmatched. Explicit
rejection text alone still does not establish which command caused it.

The Linux Mailpit fixture verifies two real accepted submissions after a
recipient refusal and command-order error, and negative-only coverage for
`data-denied`. Its explicit rejection positive is synthetic regression, not an
observed real post-body denial. No mailbox delivery or crash durability is proven.
See [SMTP reply semantics](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.2).

## Machine-Readable Surface Semantics

The `protocol_surface("smtp", "data-denied")` contract now publishes
`entry_semantics` so tooling can identify post-body rejection as a structured
server decision instead of a generic transfer failure.

Current denial semantics:

- `category = failure-path`
- `operator_focus = explicit SMTP message-rejection candidate; body handoff requires separate evidence`
- `typical_signal = 550 5.7.1 Message rejected`
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
[docs/book/reference-smtp-surface.md](docs/book/reference-smtp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `message`
- `message-denied`

<!-- gewyvern:entry-aliases:end -->
