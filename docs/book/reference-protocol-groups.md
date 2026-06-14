# Reference: Protocol Groups

Use this page when you know you are in the protocol reference volume, but do
not yet know which family shelf to open.

This page is the higher-level grouping index above the family shelves.

It exists so the protocol reference material feels more like a reference
volume and less like one long flat directory.

Read this alongside:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## How To Use This Volume

The normal lookup path is:

1. group by protocol role
2. choose the family hub
3. choose the narrower family subpage
4. return to lowering/runtime reference if needed

Use this page when the question is still broad, such as:

- “is this closer to mail, directory, cache, or proxy traffic?”
- “which family shelf should I open first?”
- “what is the right hub page for this kind of protocol?”

If you already know the exact family, skip directly to:

- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Web, Proxy, And Request/Response

Use these shelves when the traffic is primarily:

- request/response
- tunnel establishment
- proxy traversal
- modern web transport

Families:

- [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
- [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
- [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)

## Messaging, Queue, And Cache

Use these shelves when the traffic is primarily:

- broker or queue negotiation
- publish/subscribe
- cache reads/writes
- stream or data-structure operations

Families:

- [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
- [docs/book/reference-memcached-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-surface.md)
- [docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md)
- [docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md)

## Database And Query

Use these shelves when the traffic is primarily:

- connection negotiation
- query/session flow
- server error or denial posture

Families:

- [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)

## Mail And Mailbox

Use these shelves when the traffic is primarily:

- SMTP submission
- IMAP mailbox control
- POP3 mailbox listing

Families:

- [docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md)
- [docs/book/reference-imap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-surface.md)
- [docs/book/reference-pop3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-surface.md)

## Identity, Directory, And Access

Use these shelves when the traffic is primarily:

- directory bind/search/write
- ticket or auth negotiation
- shell/session access

Families:

- [docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md)
- [docs/book/reference-kerberos-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)

Note:

- Kerberos currently routes through the general protocol surface and family
  contract pages rather than a dedicated hub page in this book.

## Transport, Media, And Session Control

Use these shelves when the traffic is primarily:

- handshake or session establishment
- media control
- transport-stage progression

Families:

- [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
- [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
- [docs/book/reference-rtsp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-surface.md)
- [docs/book/reference-sip-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-surface.md)
- [docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md)

## Practical Lookup Order

If you are unsure where to begin, use this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
3. [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
4. one family hub page
5. one narrower family subpage
6. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Current Thesis

For the current line, protocol reference should be easy to approach in three
steps:

- broad group
- exact family
- exact sub-surface

That is the minimum structure needed for the protocol reference volume to stay
usable as coverage keeps expanding.
