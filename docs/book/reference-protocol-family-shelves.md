# Reference: Protocol Family Shelves

Use this page when you already know you are looking for a built-in protocol
family and want the shortest path into its narrower reference shelf.

This page is the protocol-family directory for the book. It keeps the top-level
reference and book index pages smaller while giving each built-in family one
predictable place in the reference spine.

## How To Read These Shelves

The normal lookup order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. one family hub page
3. one narrower family subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

Use the protocol-surface page when you need registry, alias, or default-entry
resolution rules. Use a family hub page when you already know the protocol and
need command-family lookup.

## Current Family Shelves

### Redis

- Hub:
  [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
- Subpages:
  [docs/book/reference-redis-kv-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-kv-surface.md),
  [docs/book/reference-redis-hash-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-hash-surface.md),
  [docs/book/reference-redis-list-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-list-surface.md),
  [docs/book/reference-redis-sorted-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-sorted-set-surface.md),
  [docs/book/reference-redis-stream-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-stream-surface.md)
- Scope:
  Session/kv, hash, list, sorted-set, and stream lookup.

### FTP

- Hub:
  [docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md)
- Subpages:
  [docs/book/reference-ftp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-session-surface.md),
  [docs/book/reference-ftp-passive-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-passive-surface.md),
  [docs/book/reference-ftp-active-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-active-surface.md)
- Scope:
  Session/auth, passive transfer, and active transfer lookup.

### SMTP

- Hub:
  [docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md)
- Subpages:
  [docs/book/reference-smtp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-session-surface.md),
  [docs/book/reference-smtp-envelope-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-envelope-surface.md),
  [docs/book/reference-smtp-data-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-data-surface.md)
- Scope:
  Greeting/auth, envelope flow, and message submission lookup.

### MQTT

- Hub:
  [docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md)
- Subpages:
  [docs/book/reference-mqtt-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-session-surface.md),
  [docs/book/reference-mqtt-pubsub-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-pubsub-surface.md),
  [docs/book/reference-mqtt-qos2-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-qos2-surface.md)
- Scope:
  Session establishment, pub/sub, QoS2 continuation, and teardown lookup.

### LDAP

- Hub:
  [docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md)
- Subpages:
  [docs/book/reference-ldap-bind-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-bind-surface.md),
  [docs/book/reference-ldap-search-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-search-surface.md),
  [docs/book/reference-ldap-write-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-write-surface.md)
- Scope:
  Bind, directory read/session, and write/sync lookup.

### PostgreSQL

- Hub:
  [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
- Subpages:
  [docs/book/reference-postgres-connect-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-connect-surface.md),
  [docs/book/reference-postgres-query-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-query-surface.md),
  [docs/book/reference-postgres-error-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-error-surface.md)
- Scope:
  Connect/auth, query/session, and query-error lookup.

## Naming Conventions For Future Shelves

When adding a new family shelf, prefer this shape:

- one hub page named `reference-<family>-surface.md`
- two to five narrower subpages named `reference-<family>-<slice>-surface.md`
- one short “Scope” line in this directory page
- one stable reading order that starts from protocol resolution, then family
  hub, then family subpage, then IR lowering

This keeps the protocol-family reference shelves uniform without forcing the
top-level book pages to grow every time a new family is added.
