# Reference: MySQL Connect Surface

Use this page when you need the current exact lookup surface for MySQL socket
establishment.

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

## Operator Reading Order

Read the current MySQL connect family in this order:

1. process bind
2. socket connect and establish
3. route resolution

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
- `mysql_connect`

<!-- gewyvern:entry-aliases:end -->
