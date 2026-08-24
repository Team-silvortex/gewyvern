# Reference: IMAP Protocol Surface

Use this page when you want the IMAP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current IMAP coverage into two narrower operator-facing
surfaces:

- authentication outcome flow
- mailbox selection flow

## What This Shelf Covers

The current built-in IMAP family models a staged mailbox-session conversation:

- connect and receive the IMAP banner
- send `LOGIN`
- receive `OK` or `NO`
- optionally send `SELECT`
- receive mailbox selection success

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `auth`

## IMAP Surface Map

### Authentication

- [docs/book/reference-imap-auth-surface.md](docs/book/reference-imap-auth-surface.md)
  Banner plus `LOGIN` success or denial flow.

Typical entries:

- `auth`
- `auth-denied`

### Mailbox Select

- [docs/book/reference-imap-select-surface.md](docs/book/reference-imap-select-surface.md)
  Successful login followed by `SELECT` mailbox flow.

Typical entries:

- `select`

## Reading Order

If you are validating current IMAP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-imap-surface.md](docs/book/reference-imap-surface.md)
3. the auth or select subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the IMAP family in the current `1.17.x` line.
New IMAP session branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
