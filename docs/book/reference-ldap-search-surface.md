# Reference: LDAP Search Surface

Use this page when you need the current exact lookup surface for LDAP search
and directory-session reads.

## Canonical Entries

### `search`

Aliases:

- `directory`
- `query`

Intent:

- establish the LDAP socket
- send a directory search request
- receive search results

### `session`

Aliases:

- `directory-session`

Intent:

- bind successfully
- send search traffic over the same established directory session
- receive search results

## Shared Response Shape

Both entries currently share the same broad staging model:

1. process binding
2. socket connect and establish
3. route resolution
4. optional bind exchange
5. search request
6. search result

The current entries differ in how much session setup they explicitly model:

- `search` focuses on the search request/result pair
- `session` models bind plus search as one directory-read session

## Operator Reading Order

If you are reviewing LDAP read coverage, read it in this order:

1. `bind`
2. `search`
3. `session`

That sequence keeps authentication context ahead of directory query flow.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `ldap` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md).
