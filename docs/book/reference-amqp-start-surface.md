# Reference: AMQP Start Surface

Use this page when you need the current exact lookup surface for AMQP start and
negotiation flow.

## Canonical Entries

### `start`

Aliases:

- `login`
- `negotiate`

Protocol aliases:

- `amqp-start`
- `amqp_start`

Intent:

- establish the AMQP socket
- send the protocol header
- receive `start`
- send `start-ok`

Coarse response shape:

- process binding
- route resolution
- AMQP socket connect and establish
- protocol header send
- broker start receive
- start-ok send

## Operator Reading Order

Read the current AMQP start family in this order:

1. process bind
2. route resolution
3. socket connect and establish
4. protocol header send
5. `start` receive
6. `start-ok` send

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `amqp`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `amqp-start`
- `amqp_start`
- `login`
- `negotiate`

<!-- gewyvern:entry-aliases:end -->
