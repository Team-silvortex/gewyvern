# Reference: SOCKS5 Auth Surface

Use this page when you need the current exact lookup surface for
username/password-authenticated SOCKS5 flow.

## Canonical Entries

### `auth`

Aliases:

- `login`
- `userpass`

Intent:

- open the SOCKS5 socket
- send the auth-capable method greeting
- receive username/password method selection
- send auth request
- receive auth success
- send a connect request
- receive connect success

Coarse response shape:

- process binding
- route resolution
- SOCKS5 socket connect
- auth-capable method negotiation
- auth request and auth success
- connect request
- connect success reply

### `auth-denied`

Aliases:

- `login-denied`
- `userpass-denied`

Intent:

- open the SOCKS5 socket
- send the auth-capable method greeting
- receive username/password method selection
- send auth request
- receive auth denial

Coarse response shape:

- process binding
- route resolution
- SOCKS5 socket connect
- auth-capable method negotiation
- auth request and auth denial

## Operator Reading Order

Read the current SOCKS5 auth family in this order:

1. process bind
2. route resolution
3. socket connect
4. method greeting and selection
5. auth request and success
6. connect request
7. connect success

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `socks5`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `login`
- `login-denied`
- `userpass`
- `userpass-denied`

<!-- gewyvern:entry-aliases:end -->
