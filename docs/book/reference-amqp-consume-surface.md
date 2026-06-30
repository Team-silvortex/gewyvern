# Reference: AMQP Consume Surface

Use this page when you need the current exact lookup surface for AMQP consume
and delivery flow.

## Canonical Entries

### `consume`

Aliases:

- `receive`
- `deliver`

Protocol aliases:

- `amqp-consume`
- `amqp_consume`

Intent:

- operate over an established AMQP session
- send consume registration/request traffic
- receive broker delivery traffic

Coarse response shape:

- process binding
- route resolution
- AMQP socket connect and establish
- consume send
- delivery receive

## Operator Reading Order

Read the current AMQP consume family in this order:

1. `start`
2. `session`
3. `consume`

That sequence keeps negotiation and publish/session framing in front of the
consumer-side delivery path.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `amqp` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-amqp-surface.md](docs/book/reference-amqp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `amqp-consume`
- `amqp_consume`
- `deliver`
- `receive`

<!-- gewyvern:entry-aliases:end -->
