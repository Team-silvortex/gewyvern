# Reference: AMQP Publish Surface

Use this page when you need the current exact lookup surface for AMQP publish
and acknowledgement flow.

Family hub: [AMQP surface](docs/book/reference-amqp-surface.md)

## Canonical Entry

### `publish`

Aliases:

- `send`

Protocol aliases:

- `amqp-publish`
- `amqp_publish`

Intent:

- operate over an established AMQP session
- send `basic.publish` traffic
- observe broker acknowledgement

## Response Shape

1. process binding
2. route resolution
3. AMQP socket connect and establish
4. publish send
5. acknowledgement receive

## Operator Reading Order

If you are reviewing AMQP publish coverage, read it in this order:

1. `start`
2. `session`
3. `publish`

That sequence keeps negotiation and session context in front of the narrower
publish path.

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

- `amqp-publish`
- `amqp_publish`
- `send`

<!-- gewyvern:entry-aliases:end -->
