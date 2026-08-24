# Reference: LDAP Protocol Surface

Use this page when you want the LDAP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current LDAP coverage into three narrower operator-facing
surfaces:

- bind and bind failure
- directory query/session flow
- directory write, sync, and write failure flow

## What This Shelf Covers

The current built-in LDAP family models a directory-service session as a staged
conversation:

- establish the LDAP socket
- send `bind`
- either succeed or fail at authentication
- search the directory
- optionally modify or synchronize entries

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for LDAP entry
selection:

- `ldap-bind`
- `ldap-bind-denied`
- `ldap-constraint`
- `ldap-denied`
- `ldap-modify`
- `ldap-search`
- `ldap-session`
- `ldap-sync`
- `ldap-write`
- `ldap_bind`
- `ldap_bind_denied`
- `ldap_constraint`
- `ldap_denied`
- `ldap_modify`
- `ldap_search`
- `ldap_session`
- `ldap_sync`
- `ldap_write`

Default entry: `sync`

## LDAP Surface Map

### Bind

- [docs/book/reference-ldap-bind-surface.md](docs/book/reference-ldap-bind-surface.md)
  Bind success, bind denial, and bind-oriented login/auth aliases.

Typical entries:

- `bind`
- `bind-denied`

### Search And Session

- [docs/book/reference-ldap-search-surface.md](docs/book/reference-ldap-search-surface.md)
  Search requests, directory session flow, and read-oriented directory aliases.

Typical entries:

- `search`
- `session`

### Write And Sync

- [docs/book/reference-ldap-write-surface.md](docs/book/reference-ldap-write-surface.md)
  Modify success, denial, constraint failure, explicit write sessions, and
  sync/replication sessions.

Typical entries:

- `modify`
- `denied`
- `constraint`
- `write`
- `sync`

## Reading Order

If you are validating current LDAP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ldap-surface.md](docs/book/reference-ldap-surface.md)
3. one narrower LDAP subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the LDAP family in the current `1.17.x` line.
New LDAP command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
