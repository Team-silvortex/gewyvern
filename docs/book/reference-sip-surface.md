# Reference: SIP Protocol Surface

Use this page when you want the SIP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current SIP coverage into five narrower operator-facing
surfaces:

- registration flow
- invite or call setup flow
- call termination flow
- response observation flow
- rejection or failed-response flow

## What This Shelf Covers

The current built-in SIP family models five coarse UDP control-plane actions:

- send `REGISTER` and receive a SIP response
- send `INVITE` and receive a SIP response
- send `BYE` and receive a SIP response
- receive a SIP response datagram
- receive a 4xx, 5xx, or 6xx SIP failure response

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `register`

## SIP Surface Map

### Register

- [docs/book/reference-sip-register-surface.md](docs/book/reference-sip-register-surface.md)
  Registration path over UDP.

Typical entries:

- `register`

### Invite

- [docs/book/reference-sip-invite-surface.md](docs/book/reference-sip-invite-surface.md)
  Call setup or session invite path over UDP.

Typical entries:

- `invite`

### Bye

- [docs/book/reference-sip-bye-surface.md](docs/book/reference-sip-bye-surface.md)
  Call termination path over UDP.

Typical entries:

- `bye`

### Response

- [docs/book/reference-sip-response-surface.md](docs/book/reference-sip-response-surface.md)
  Response observation path over UDP.

Typical entries:

- `response`

### Denied

- [docs/book/reference-sip-denied-surface.md](docs/book/reference-sip-denied-surface.md)
  Failed or rejected SIP response path over UDP.

Typical entries:

- `denied`

## Reading Order

If you are validating current SIP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md)
3. the register, invite, bye, response, or denied subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the SIP family in the current `1.2.0` line.
New SIP action branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
