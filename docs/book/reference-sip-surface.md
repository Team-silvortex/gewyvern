# Reference: SIP Protocol Surface

Use this page when you want the SIP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current SIP coverage into three narrower operator-facing
surfaces:

- registration flow
- invite or call setup flow
- call termination flow

## What This Shelf Covers

The current built-in SIP family models three coarse UDP control-plane actions:

- send `REGISTER` and receive a SIP response
- send `INVITE` and receive a SIP response
- send `BYE` and receive a SIP response

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## SIP Surface Map

### Register

- [docs/book/reference-sip-register-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-register-surface.md)
  Registration path over UDP.

Typical entries:

- `register`

### Invite

- [docs/book/reference-sip-invite-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-invite-surface.md)
  Call setup or session invite path over UDP.

Typical entries:

- `invite`

### Bye

- [docs/book/reference-sip-bye-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-bye-surface.md)
  Call termination path over UDP.

Typical entries:

- `bye`

## Reading Order

If you are validating current SIP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-sip-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-surface.md)
3. the register, invite, or bye subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current SIP family in the `0.15.x` line.
New SIP action branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
