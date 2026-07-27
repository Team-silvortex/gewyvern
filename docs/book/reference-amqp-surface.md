# Reference: AMQP Protocol Surface

Use this page when you want the AMQP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current AMQP coverage into three narrower
operator-facing surfaces:

- connection start and negotiation
- session flow
- publish flow
- consume flow

## What This Shelf Covers

The current built-in AMQP family models a staged broker conversation:

- establish the AMQP socket
- send the protocol header
- receive `start`
- send `start-ok`
- publish or consume over the established session

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for AMQP entry
selection:

- `amqp-consume`
- `amqp-auth-denied`
- `amqp-publish`
- `amqp-session`
- `amqp-start`
- `amqp_auth_denied`
- `amqp_consume`
- `amqp_publish`
- `amqp_session`
- `amqp_start`

Default entry: `session`

## AMQP Surface Map

### Start And Negotiation

- [docs/book/reference-amqp-start-surface.md](docs/book/reference-amqp-start-surface.md)
  Protocol header, `start`, and `start-ok` negotiation flow.

Typical entries:

- `start`
- `auth-denied`
- `amqp-auth-denied`

### Session

- [docs/book/reference-amqp-session-surface.md](docs/book/reference-amqp-session-surface.md)
  Broader broker session framing before message transfer.

Typical entries:

- `session`

### Publish

- [docs/book/reference-amqp-publish-surface.md](docs/book/reference-amqp-publish-surface.md)
  Publish request and broker acknowledgement flow.

Typical entries:

- `publish`

### Consume

- [docs/book/reference-amqp-consume-surface.md](docs/book/reference-amqp-consume-surface.md)
  Consumer registration and delivery flow.

Typical entries:

- `consume`

## Reading Order

If you are validating current AMQP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-amqp-surface.md](docs/book/reference-amqp-surface.md)
3. one narrower AMQP subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the AMQP family in the current `1.10.x` line.
New AMQP command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
