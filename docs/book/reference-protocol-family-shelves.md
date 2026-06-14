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

### HTTP

- Hub:
  [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
- Subpages:
  [docs/book/reference-http-message-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-message-surface.md),
  [docs/book/reference-http-connect-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-connect-surface.md),
  [docs/book/reference-http-connect-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-connect-auth-surface.md)
- Scope:
  Direct request/response, CONNECT tunnel, and proxy-auth CONNECT lookup.

### SOCKS5

- Hub:
  [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)
- Subpages:
  [docs/book/reference-socks5-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-session-surface.md),
  [docs/book/reference-socks5-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-auth-surface.md),
  [docs/book/reference-socks5-denied-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-denied-surface.md)
- Scope:
  Session/connect, username-password auth, and denial-branch lookup.

### MySQL

- Hub:
  [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)
- Subpages:
  [docs/book/reference-mysql-connect-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-connect-surface.md),
  [docs/book/reference-mysql-query-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-query-surface.md),
  [docs/book/reference-mysql-error-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-error-surface.md)
- Scope:
  Connect, query/session, and query-error lookup.

### AMQP

- Hub:
  [docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md)
- Subpages:
  [docs/book/reference-amqp-start-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-start-surface.md),
  [docs/book/reference-amqp-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-session-surface.md),
  [docs/book/reference-amqp-consume-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-consume-surface.md)
- Scope:
  Start negotiation, publish/session, and consume/delivery lookup.

### SSH

- Hub:
  [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)
- Subpages:
  [docs/book/reference-ssh-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-session-surface.md),
  [docs/book/reference-ssh-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-auth-surface.md),
  [docs/book/reference-ssh-channel-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-channel-surface.md)
- Scope:
  Session startup, auth outcome, and authenticated channel-open lookup.

### RTSP

- Hub:
  [docs/book/reference-rtsp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-surface.md)
- Subpages:
  [docs/book/reference-rtsp-options-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-options-surface.md),
  [docs/book/reference-rtsp-describe-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-describe-surface.md),
  [docs/book/reference-rtsp-setup-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-setup-surface.md),
  [docs/book/reference-rtsp-play-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-play-surface.md)
- Scope:
  Probe, metadata lookup, setup, and playback-start lookup.

### QUIC

- Hub:
  [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
- Subpages:
  [docs/book/reference-quic-initial-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-initial-surface.md),
  [docs/book/reference-quic-crypto-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-crypto-surface.md),
  [docs/book/reference-quic-stream-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-stream-surface.md),
  [docs/book/reference-quic-bidi-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-bidi-surface.md)
- Scope:
  Initial, crypto-handshake, outbound-stream, and bidirectional-stream lookup.

### DNS

- Hub:
  [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
- Subpages:
  [docs/book/reference-dns-udp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-udp-surface.md),
  [docs/book/reference-dns-tcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-tcp-surface.md)
- Scope:
  Default UDP lookup and TCP query lookup.

### HTTP/3

- Hub:
  [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
- Subpages:
  [docs/book/reference-http3-request-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-request-surface.md),
  [docs/book/reference-http3-server-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-server-surface.md)
- Scope:
  Client request posture and local server response posture over QUIC.

### IMAP

- Hub:
  [docs/book/reference-imap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-surface.md)
- Subpages:
  [docs/book/reference-imap-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-auth-surface.md),
  [docs/book/reference-imap-select-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-select-surface.md)
- Scope:
  Login outcome and mailbox-selection lookup.

### SIP

- Hub:
  [docs/book/reference-sip-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-surface.md)
- Subpages:
  [docs/book/reference-sip-register-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-register-surface.md),
  [docs/book/reference-sip-invite-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-invite-surface.md),
  [docs/book/reference-sip-bye-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-bye-surface.md)
- Scope:
  Registration, call-setup, and teardown lookup.

### POP3

- Hub:
  [docs/book/reference-pop3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-surface.md)
- Subpages:
  [docs/book/reference-pop3-auth-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-auth-surface.md),
  [docs/book/reference-pop3-list-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-list-surface.md)
- Scope:
  Login outcome and mailbox-list lookup.

### Memcached

- Hub:
  [docs/book/reference-memcached-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-surface.md)
- Subpages:
  [docs/book/reference-memcached-get-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-get-surface.md),
  [docs/book/reference-memcached-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-set-surface.md)
- Scope:
  Binary get/read and set/write lookup.

## Naming Conventions For Future Shelves

When adding a new family shelf, prefer this shape:

- one hub page named `reference-<family>-surface.md`
- two to five narrower subpages named `reference-<family>-<slice>-surface.md`
- one short “Scope” line in this directory page
- one stable reading order that starts from protocol resolution, then family
  hub, then family subpage, then IR lowering

This keeps the protocol-family reference shelves uniform without forcing the
top-level book pages to grow every time a new family is added.

## Current Coverage

The first high-yield family shelves are now in place for:

- `redis`
- `ftp`
- `smtp`
- `mqtt`
- `ldap`
- `postgres`
- `http`
- `socks5`
- `mysql`
- `amqp`
- `ssh`
- `rtsp`
- `quic`
- `dns`
- `http3`
- `imap`
- `sip`
- `pop3`
- `memcached`

That means the remaining work is no longer about closing the most obvious
reference gaps. It is about choosing the next-most-useful families without
letting the reference spine grow faster than it stays coherent.

## Next Shelf Criteria

If we continue growing the protocol-family shelves, the next family should
usually be justified by at least one of these conditions:

- it has a clear staged conversation that benefits from a hub plus two or more
  narrower subpages
- it has enough aliases or operator ambiguity that a narrower shelf materially
  reduces lookup friction
- a user-facing workflow depends on it often enough that a dedicated shelf is
  easier to navigate than the generic protocol surface alone

In other words, we should now prefer workflow value over raw entry count.

## Families We Should Probably Not Split Yet

The following families should usually stay as protocol-surface entries only for
now, unless their entry count or operator ambiguity grows:

- smaller families such as `tls`, `wireguard`, `stun`, `radius`, `ntp`,
  `mdns`, `https`, `gtpu`, `dhcp`, `coap`, `ssdp`, and `snmp`
- three-entry families such as `kerberos` and `hy2`,
  unless a user-facing workflow starts depending on them heavily

Those smaller families are still covered by
[docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md);
they just do not yet justify their own mini-shelf in the book.
