# Reference: PostgreSQL Connect Surface

Use this page when you need the current exact lookup surface for PostgreSQL
socket establishment and authentication flow.

## Canonical Entries

### `connect`

Aliases:

- none registered today

Intent:

- bind the process
- observe PostgreSQL socket connect and establish
- resolve the upstream route

Coarse response shape:

- process binding
- socket connect
- established socket state
- route resolution

### `auth`

Aliases:

- none registered today

Intent:

- establish the PostgreSQL socket
- receive server auth challenge
- send password message
- receive ready state

Coarse response shape:

- same bind/connect/route scaffolding as `connect`
- auth challenge (`R`)
- password send
- ready-for-query (`Z`)

## Operator Reading Order

Read the current PostgreSQL connect family in this order:

1. process bind
2. socket connect and establish
3. route resolution
4. auth challenge
5. password send
6. ready state

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `postgres`
- canonical entry normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md).
