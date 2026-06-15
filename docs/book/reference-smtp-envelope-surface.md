# Reference: SMTP Envelope Surface

Use this page when you need the current exact lookup surface for SMTP envelope
construction: sender acceptance, recipient acceptance, and recipient denial.

## Canonical Entries

### `mail`

Aliases:

- `sender`

Intent:

- complete session greeting and auth success
- send `MAIL FROM`
- receive sender acceptance

### `rcpt`

Aliases:

- `recipient`

Intent:

- complete the `mail` flow
- send `RCPT TO`
- receive recipient acceptance

### `rcpt-denied`

Aliases:

- `recipient-denied`

Intent:

- complete the `mail` flow
- send `RCPT TO`
- receive recipient denial (`550`)

## Shared Response Shape

All envelope entries currently share the same broad staging model:

1. process binding
2. route resolution
3. TCP control-channel connect
4. banner and `EHLO`
5. auth request and auth success
6. `MAIL FROM` and sender acceptance
7. optional `RCPT TO` and either acceptance or denial

The current success-oriented models distinguish the envelope stages by the SMTP
reply family they terminate on:

- `mail` stops at sender acceptance (`250 2.1.x`)
- `rcpt` stops at recipient acceptance (`250 2.1.5`)
- `rcpt-denied` stops at recipient denial (`550`)

## Operator Reading Order

If you are reviewing SMTP envelope coverage, read it in this order:

1. `auth`
2. `mail`
3. `rcpt`
4. `rcpt-denied`

That sequence keeps the shared greeting/auth context in front of the sender and
recipient stages.

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

- `recipient`
- `recipient-denied`
- `sender`

<!-- gewyvern:entry-aliases:end -->
