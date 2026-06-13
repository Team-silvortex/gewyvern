# Reference: LDAP Bind Surface

Use this page when you need the current exact lookup surface for LDAP bind
success and bind failure.

## Canonical Entries

### `bind`

Aliases:

- `login`
- `auth`
- `ldap-bind`
- `ldap_bind`

Intent:

- establish the LDAP socket
- send `bind`
- receive a bind response

Coarse response shape:

- process binding
- socket connect and establish
- route resolution
- bind request and bind response

### `bind-denied`

Aliases:

- `login-denied`
- `auth-denied`
- `ldap-bind-denied`
- `ldap_bind_denied`

Intent:

- establish the LDAP socket
- send `bind`
- receive explicit bind denial

Coarse response shape:

- same bind/connect/route scaffolding as `bind`
- terminal denial response instead of generic bind success

## Operator Reading Order

Read the current LDAP bind family in this order:

1. process bind
2. socket connect and establish
3. route resolution
4. `bind`
5. success or denial response

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `ldap`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md).
