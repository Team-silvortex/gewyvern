# Reference: SOCKS5 Session Surface

Use this page when you need the current exact lookup surface for unauthenticated
SOCKS5 session establishment and successful proxy connect.

## Canonical Entries

### `session`

Aliases:

- `connect`
- `proxy`

Protocol aliases:

- `socks`
- `socks5-session`
- `socks5_session`

Intent:

- open the SOCKS5 socket
- send the method greeting
- receive no-auth method selection
- send a connect request
- receive connect success

Coarse response shape:

- process binding
- route resolution
- SOCKS5 socket connect
- method negotiation
- connect request
- connect success reply

## Operator Reading Order

Read the current SOCKS5 session family in this order:

1. process bind
2. route resolution
3. socket connect
4. method greeting and selection
5. connect request
6. connect success

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `socks5`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md).
