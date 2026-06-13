# Reference: HTTP CONNECT Auth Surface

Use this page when you need the current exact lookup surface for proxy-auth
branches in HTTP `CONNECT` tunnel flow.

## Canonical Entries

### `auth-required`

Aliases:

- none registered at the entry level today

Intent:

- open a proxy-side socket
- send an HTTP `CONNECT` request
- receive proxy-auth-required response (`407`)

### `auth-tunnel`

Aliases:

- none registered at the entry level today

Intent:

- send an HTTP `CONNECT` request
- observe the proxy-auth-required branch
- continue through authenticated tunnel establishment (`200`)

## Shared Response Shape

Both entries currently share the same proxy-auth-oriented staging model:

1. process binding
2. route resolution
3. proxy socket connect
4. `CONNECT` request send
5. proxy auth required (`407`)
6. optional authenticated tunnel success (`200`)

The entries diverge after the `407` branch:

- `auth-required` stops at the proxy auth requirement
- `auth-tunnel` continues until the tunnel is established

## Operator Reading Order

If you are reviewing authenticated CONNECT coverage, read it in this order:

1. `auth-required`
2. `auth-tunnel`

That sequence keeps the authentication branch point visible before the eventual
tunnel success path.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `http` family resolution
- canonical entry normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md).
