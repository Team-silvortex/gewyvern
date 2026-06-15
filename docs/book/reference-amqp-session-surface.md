# Reference: AMQP Session Surface

Use this page when you need the current exact lookup surface for AMQP session
and publish flow.

## Canonical Entries

### `session`

Aliases:

- `connect`

Protocol aliases:

- `amqp-session`
- `amqp_session`

Intent:

- establish the AMQP socket
- send the protocol header
- receive `start`
- send `start-ok`
- send publish traffic
- receive publish acknowledgement

### `publish`

Aliases:

- `send`

Protocol aliases:

- `amqp-publish`
- `amqp_publish`

Intent:

- operate over an established AMQP session
- send publish traffic
- receive broker acknowledgement

## Shared Response Shape

Both entries currently share a publish-oriented staging model:

1. process binding
2. route resolution
3. AMQP socket connect and establish
4. optional start negotiation
5. publish send
6. acknowledgement receive

`session` keeps the broader session framing with start negotiation, while
`publish` stays focused on the publish/ack pair.

## Operator Reading Order

If you are reviewing AMQP publish coverage, read it in this order:

1. `start`
2. `session`
3. `publish`

That sequence keeps negotiation context in front of the narrower publish path.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `amqp` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `amqp-publish`
- `amqp-session`
- `amqp_publish`
- `amqp_session`
- `connect`
- `send`

<!-- gewyvern:entry-aliases:end -->
