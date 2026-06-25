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
- “what broader protocol cluster does this family belong to?”

If you already know the exact family, skip directly to:

- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

When `cluster_hint` is available in the runtime API, it should agree with the
high-level grouping shown on this page:

- the book gives the human-facing browsing order
- `cluster_hint` gives the machine-stable cluster key and operator note

<!-- gewyvern:protocol-groups:start -->
## Web, Proxy, And Request/Response

Families:

- [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
- [docs/book/reference-https-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-https-surface.md)
- [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
- [docs/book/reference-hy2-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-hy2-surface.md)
- [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)

Cluster hint:

- key: `web-proxy-request-response`
- operator hint: Start with request/response intent, proxy handoff, and selected surface entry before drilling into transport details.
- sibling protocols: `http`, `https`, `http3`, `socks5`

## Messaging, Queue, And Cache

Families:

- [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
- [docs/book/reference-memcached-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-surface.md)
- [docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md)
- [docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md)

Cluster hint:

- key: `cache-queue-stream`
- operator hint: Check data-shape, routing or consumer role, and server-side refusal signals first; these families often fail after connect but before stable consumption semantics.
- sibling protocols: `redis`, `memcached`, `mqtt`, `amqp`

## Database And Query

Families:

- [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)

Cluster hint:

- key: `database-query-session`
- operator hint: Read auth, query, and transaction surfaces in order; the default entry is rarely enough when session state or query errors are present.
- sibling protocols: `postgres`, `mysql`

## Mail And Mailbox

Families:

- [docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md)
- [docs/book/reference-imap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-surface.md)
- [docs/book/reference-pop3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-surface.md)

Cluster hint:

- key: `mail-delivery-mailbox`
- operator hint: Separate delivery, retrieval, and mailbox state early; the same account issue can present very differently across send and read surfaces.
- sibling protocols: `smtp`, `imap`, `pop3`

## Identity, Directory, And Access

Families:

- [docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md)
- [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)
- [docs/book/reference-kerberos-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-kerberos-surface.md)
- [docs/book/reference-radius-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-surface.md)

Cluster hint:

- key: `identity-directory-access`
- operator hint: Prioritize bind, credential, authorization, and access-gate stages; these protocols tend to fail with explicit denial semantics rather than silent payload drift.
- sibling protocols: `ldap`, `ssh`, `kerberos`, `radius`

## Transport, Media, And Session Control

Families:

- [docs/book/reference-stun-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-surface.md)
- [docs/book/reference-coap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-coap-surface.md)
- [docs/book/reference-dhcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-surface.md)
- [docs/book/reference-arp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-arp-surface.md)
- [docs/book/reference-bgp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-bgp-surface.md)
- [docs/book/reference-icmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmp-surface.md)
- [docs/book/reference-icmpv6-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-icmpv6-surface.md)
- [docs/book/reference-ndp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ndp-surface.md)
- [docs/book/reference-ntp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-surface.md)
- [docs/book/reference-ospf-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ospf-surface.md)
- [docs/book/reference-gre-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gre-surface.md)
- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
- [docs/book/reference-mdns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mdns-surface.md)
- [docs/book/reference-ssdp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssdp-surface.md)
- [docs/book/reference-gtpu-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gtpu-surface.md)
- [docs/book/reference-wireguard-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-wireguard-surface.md)
- [docs/book/reference-tls-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-surface.md)
- [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
- [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
- [docs/book/reference-rtsp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-surface.md)
- [docs/book/reference-sip-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-surface.md)
- [docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md)

Cluster hint:

- key: `network-control-discovery`
- operator hint: Start with discovery scope, control role, and time or tunnel posture; many issues here are topology-sensitive rather than application-payload-specific.
- sibling protocols: `dns`, `mdns`, `ssdp`, `stun`, `coap`, `ntp`, `dhcp`, `arp`, `icmp`, `icmpv6`, `ndp`, `bgp`, `ospf`, `gre`, `snmp`, `wireguard`, `gtpu`

<!-- gewyvern:protocol-groups:end -->

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
