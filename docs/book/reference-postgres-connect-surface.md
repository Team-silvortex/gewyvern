# Reference: PostgreSQL Connect Surface

Use this page when you need the current exact lookup surface for PostgreSQL
socket establishment and authentication flow.

## Canonical Entries

### `connect`

Aliases:

- `postgres-connect`
- `postgres_connect`

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

- `postgres-auth`
- `postgres_auth`

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

### `auth-denied`

Aliases:

- `login-denied`
- `password-denied`
- `postgres-auth-denied`
- `postgres_auth_denied`

Intent:

- establish the PostgreSQL socket
- receive server auth challenge
- send password message
- receive explicit rejection instead of ready state

Coarse response shape:

- same bind/connect/route scaffolding as `connect`
- auth challenge (`R`)
- password send
- error response (`E`)

## Operator Reading Order

Read the current PostgreSQL connect family in this order:

1. process bind
2. socket connect and establish
3. route resolution
4. auth challenge
5. password send
6. ready state or explicit error response

## Machine-readable Semantics

When selected through the JSON protocol-surface API, `auth-denied` currently
exposes these machine-readable semantics:

- category: `failure-path`
- operator focus:
  `database authentication rejection after PostgreSQL password exchange`
- typical signal: `ErrorResponse`
- primary failure mode: `server_denied`
- primary failure detail: `access_denied`
- primary failure basis: `direct_protocol_signal`

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `postgres`
- canonical entry normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `postgres-auth`
- `postgres-auth-denied`
- `postgres-connect`
- `postgres_auth`
- `postgres_auth_denied`
- `postgres_connect`
- `login-denied`
- `password-denied`

<!-- gewyvern:entry-aliases:end -->
