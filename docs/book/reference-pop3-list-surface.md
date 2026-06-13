# Reference: POP3 List Surface

Use this page when you need the current exact lookup surface for POP3 mailbox
listing behavior.

## Covered Entries

### `list`

- Protocol:
  `pop3`
- Aliases:
  `mailbox`
- Default entry:
  no

## Operational Shape

The current `list` flow builds on successful POP3 login and then extends it
with mailbox enumeration:

1. bind the process and resolve the upstream route
2. observe the POP3 socket transition
3. receive the POP3 banner
4. send `USER`
5. receive user acknowledgement
6. send `PASS`
7. receive auth success
8. send `LIST`
9. receive mailbox list readiness

This is the narrowest POP3 page to use when you want to prove that the
conversation progressed past authentication and into mailbox listing posture.

## Operator Reading Order

Read this page after the POP3 family hub when:

- you need the `mailbox` alias behavior
- you want to distinguish auth-only activity from message-list enumeration
- you are validating deeper POP3 session progression before IR lowering

## Stability Notes

The current entry is intentionally narrow. It models mailbox listing readiness,
not later retrieval or deletion commands that are not yet represented in this
family.
