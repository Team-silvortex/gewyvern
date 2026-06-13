# Reference: IMAP Select Surface

Use this page when you need the current exact lookup surface for IMAP mailbox
selection behavior.

## Covered Entries

### `select`

- Protocol:
  `imap`
- Aliases:
  `mailbox`
- Default entry:
  no

## Operational Shape

The current `select` flow builds on successful login and then extends it with
mailbox selection:

1. bind the process and resolve the upstream route
2. observe the IMAP socket transition
3. receive the IMAP banner
4. send `LOGIN`
5. receive `OK`
6. send `SELECT`
7. receive mailbox selected success

This is the narrowest IMAP page to use when you want to prove that the
conversation progressed past authentication and into mailbox access posture.

## Operator Reading Order

Read this page after the IMAP family hub when:

- you need the `mailbox` alias behavior
- you want to distinguish auth-only activity from mailbox selection
- you are validating deeper IMAP session progression before IR lowering

## Stability Notes

The current entry is intentionally narrow. It models successful mailbox
selection, not later fetch, search, or message-state operations.
