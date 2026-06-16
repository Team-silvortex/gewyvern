# Reference: SMTP Protocol Surface

Use this page when you want the SMTP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current SMTP coverage into three narrower operator-facing
surfaces:

- session establishment and greeting
- authentication and envelope building
- message body submission and denial flow

## What This Shelf Covers

The current built-in SMTP family models the control-channel conversation as a
progressive path:

- connect and receive the server banner
- send `EHLO`
- optionally authenticate
- construct the envelope with `MAIL FROM` and `RCPT TO`
- enter `DATA`
- either queue the message or observe rejection

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `session`

## SMTP Surface Map

### Session And Greeting

- [docs/book/reference-smtp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-session-surface.md)
  Initial connect, banner handling, `EHLO`, and authenticated session success.

Typical entries:

- `session`
- `auth`

### Envelope

- [docs/book/reference-smtp-envelope-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-envelope-surface.md)
  Sender acceptance, recipient acceptance, and recipient denial.

Typical entries:

- `mail`
- `rcpt`
- `rcpt-denied`

### Message Submission

- [docs/book/reference-smtp-data-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-data-surface.md)
  `DATA` readiness, message-body handoff, queue success, and post-body denial.

Typical entries:

- `data`
- `data-denied`

## Reading Order

If you are validating current SMTP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md)
3. one narrower SMTP subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current SMTP family in the `0.15.x` line.
New SMTP command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
