# Reference: POP3 Protocol Surface

Use this page when you want the POP3 portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current POP3 coverage into two narrower operator-facing
surfaces:

- authentication outcome flow
- mailbox listing flow

## What This Shelf Covers

The current built-in POP3 family models a staged mailbox conversation:

- connect and receive the POP3 banner
- send `USER`
- receive user acknowledgement
- send `PASS`
- receive auth success or denial
- optionally send `LIST`

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `auth`

## POP3 Surface Map

### Authentication

- [docs/book/reference-pop3-auth-surface.md](docs/book/reference-pop3-auth-surface.md)
  Banner plus `USER`/`PASS` success or denial flow.

Typical entries:

- `auth`
- `auth-denied`

### Mailbox List

- [docs/book/reference-pop3-list-surface.md](docs/book/reference-pop3-list-surface.md)
  Successful login followed by `LIST` mailbox enumeration.

Typical entries:

- `list`

## Reading Order

If you are validating current POP3 support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-pop3-surface.md](docs/book/reference-pop3-surface.md)
3. the auth or list subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the POP3 family in the current `1.17.x` line.
New POP3 session branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
