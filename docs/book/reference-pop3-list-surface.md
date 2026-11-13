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

These are reference stages, not proof of command/reply ordering. The response
candidate recognizes `+OK <1-3 decimal digits> messages (` at the start of an
observed packet. It is not a full list parser or proof of authenticated access.

## Operator Reading Order

Read this page after the POP3 family hub when:

- you need the `mailbox` alias behavior
- you want to distinguish auth-only activity from message-list enumeration
- you are validating deeper POP3 session progression before IR lowering

## Stability Notes

The current entry is intentionally narrow. STAT counts, UIDL metadata, RETR
octet counts, deletion acknowledgements and bare `+OK` do not become list
readiness. Counts above three digits, alternate server text and status lines
split across packets may remain unmatched. Body data that exactly mimics a
signature at a packet boundary still needs stream/dialogue context; these
packet-local predicates do not provide it.

The real Linux fixture separately retrieves two seeded messages, checks LIST,
STAT and UIDL, dot-unstuffs bodies, marks a message deleted, resets and verifies
the restored list. Mailpit 1.33.0 appends an extra CRLF to each RETR body while
advertising the stored size. The fixture reports this known server discrepancy
explicitly and retains every raw captured byte; it does not certify server
conformance or deletion durability after QUIT.

For the broader family map, see
[docs/book/reference-pop3-surface.md](docs/book/reference-pop3-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `mailbox`
- `pop3-list`
- `pop3_list`

<!-- gewyvern:entry-aliases:end -->
