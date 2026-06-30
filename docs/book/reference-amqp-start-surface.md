# Reference: AMQP Start Surface

Use this page when you need the current exact lookup surface for AMQP start,
negotiation, and explicit negotiation rejection flow.

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

### `auth-denied`

Aliases:

- `login-denied`
- `negotiate-denied`

Protocol aliases:

- `amqp-auth-denied`
- `amqp_auth_denied`

Intent:

- establish the AMQP socket
- send the protocol header
- receive `start`
- send `start-ok`
- receive broker `connection.close`

Coarse response shape:

- process binding
- route resolution
- AMQP socket connect and establish
- protocol header send
- broker start receive
- start-ok send
- broker close receive

## Operator Reading Order

Read the current AMQP start family in this order:

1. process bind
2. route resolution
3. socket connect and establish
4. protocol header send
5. `start` receive
6. `start-ok` send
7. if negotiation fails, broker `close` receive

## Machine-Readable Surface Semantics

The `protocol_surface("amqp", "auth-denied")` contract now publishes
machine-readable failure semantics in addition to the canonical entry and alias
surface.

Current semantics:

- `category = failure-path`
- `operator_focus = broker connection close after AMQP start-ok credential or mechanism negotiation`
- `typical_signal = connection.close`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `amqp`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-amqp-surface.md](docs/book/reference-amqp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `amqp-auth-denied`
- `amqp-start`
- `amqp_auth_denied`
- `amqp_start`
- `login`
- `login-denied`
- `negotiate`
- `negotiate-denied`

<!-- gewyvern:entry-aliases:end -->
