# Reference: MySQL Connect Surface

Use this page when you need the current exact lookup surface for MySQL socket
establishment and authentication flow.

## Canonical Entries

### `connect`

Aliases:

- none registered at the entry level today

Protocol aliases:

- `mysql-connect`
- `mysql_connect`

Intent:

- bind the process
- observe MySQL socket connect and establish
- resolve the upstream route

Coarse response shape:

- process binding
- socket connect
- established socket state
- route resolution

### `auth`

Aliases:

- `mysql-auth`
- `mysql_auth`

Intent:

- bind the process
- observe MySQL socket connect and establish
- receive server greeting
- send login handshake response
- receive positive auth result

Coarse response shape:

- same bind/connect/route scaffolding as `connect`
- initial handshake greeting (`0x0a`)
- login handshake response
- ok packet (`0x00`)

### `auth-denied`

Aliases:

- `handshake-denied`
- `login-denied`
- `mysql-auth-denied`
- `mysql_auth_denied`

Intent:

- bind the process
- observe MySQL socket connect and establish
- receive server greeting
- send login handshake response
- receive explicit auth rejection

Coarse response shape:

- same bind/connect/route scaffolding as `connect`
- initial handshake greeting (`0x0a`)
- login handshake response
- error packet (`0xff`)

## Operator Reading Order

Read the current MySQL connect and auth family in this order:

1. process bind
2. socket connect and establish
3. route resolution
4. server greeting
5. login handshake response
6. ok or error result

## Machine-readable Semantics

When selected through the JSON protocol-surface API, `auth-denied` currently
exposes these machine-readable semantics:

- category: `failure-path`
- operator focus:
  `database authentication rejection during MySQL handshake response evaluation`
- typical signal: `ERR`
- primary failure mode: `server_denied`
- primary failure detail: `access_denied`
- primary failure basis: `direct_protocol_signal`

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `mysql`
- canonical entry normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `mysql-connect`
- `mysql-auth`
- `mysql-auth-denied`
- `mysql_connect`
- `mysql_auth`
- `mysql_auth_denied`
- `handshake-denied`
- `login-denied`

<!-- gewyvern:entry-aliases:end -->
