# Reference: Protocol Groups

Use this page when you know you are in the protocol reference volume, but do
not yet know which family shelf to open.

This page is the higher-level grouping index above the family shelves.

It exists so the protocol reference material feels more like a reference
volume and less like one long flat directory.

Read this alongside:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

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

- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

When `cluster_hint` is available in the runtime API, it should agree with the
high-level grouping shown on this page:

- the book gives the human-facing browsing order
- `cluster_hint` gives the machine-stable cluster key and operator note

<!-- gewyvern:protocol-groups:start -->
## Web, Proxy, And Request/Response

Families:

- [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
- [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
- [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
- [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
- [docs/book/reference-websocket-surface.md](docs/book/reference-websocket-surface.md)
- [docs/book/reference-graphql-surface.md](docs/book/reference-graphql-surface.md)
- [docs/book/reference-s3-surface.md](docs/book/reference-s3-surface.md)
- [docs/book/reference-otlp-surface.md](docs/book/reference-otlp-surface.md)
- [docs/book/reference-prometheus-surface.md](docs/book/reference-prometheus-surface.md)
- [docs/book/reference-loki-surface.md](docs/book/reference-loki-surface.md)
- [docs/book/reference-jaeger-surface.md](docs/book/reference-jaeger-surface.md)
- [docs/book/reference-syslog-surface.md](docs/book/reference-syslog-surface.md)
- [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)

Cluster hint:

- key: `web-proxy-request-response`
- operator hint: Start with request/response intent, proxy handoff, and selected surface entry before drilling into transport details.
- sibling protocols: `http`, `https`, `http3`, `grpc`, `websocket`, `graphql`, `s3`, `otlp`, `prometheus`, `loki`, `jaeger`, `syslog`, `socks5`

## Secure Transport And Session Setup

Families:

- [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)
- [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
- [docs/book/reference-hy2-surface.md](docs/book/reference-hy2-surface.md)
- [docs/book/reference-ipsec-surface.md](docs/book/reference-ipsec-surface.md)

Cluster hint:

- key: `secure-transport-session`
- operator hint: Bias toward handshake, cipher, tunnel, and session-establishment stages; many failures here look like setup posture before payload semantics exist.
- sibling protocols: `quic`, `tls`, `hy2`, `ipsec`

## Messaging, Queue, And Cache

Families:

- [docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md)
- [docs/book/reference-memcached-surface.md](docs/book/reference-memcached-surface.md)
- [docs/book/reference-mqtt-surface.md](docs/book/reference-mqtt-surface.md)
- [docs/book/reference-amqp-surface.md](docs/book/reference-amqp-surface.md)
- [docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md)
- [docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md)

Cluster hint:

- key: `cache-queue-stream`
- operator hint: Check data-shape, routing or consumer role, and server-side refusal signals first; these families often fail after connect but before stable consumption semantics.
- sibling protocols: `redis`, `memcached`, `mqtt`, `amqp`, `kafka`, `nats`

## Database And Query

Families:

- [docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
- [docs/book/reference-mongodb-surface.md](docs/book/reference-mongodb-surface.md)
- [docs/book/reference-cassandra-surface.md](docs/book/reference-cassandra-surface.md)
- [docs/book/reference-mssql-surface.md](docs/book/reference-mssql-surface.md)
- [docs/book/reference-elasticsearch-surface.md](docs/book/reference-elasticsearch-surface.md)
- [docs/book/reference-etcd-surface.md](docs/book/reference-etcd-surface.md)
- [docs/book/reference-zookeeper-surface.md](docs/book/reference-zookeeper-surface.md)
- [docs/book/reference-consul-surface.md](docs/book/reference-consul-surface.md)

Cluster hint:

- key: `database-query-session`
- operator hint: Read auth, query, and transaction surfaces in order; the default entry is rarely enough when session state or query errors are present.
- sibling protocols: `postgres`, `mysql`, `mongodb`, `cassandra`, `mssql`, `elasticsearch`, `etcd`, `zookeeper`, `consul`

## Mail And Mailbox

Families:

- [docs/book/reference-smtp-surface.md](docs/book/reference-smtp-surface.md)
- [docs/book/reference-imap-surface.md](docs/book/reference-imap-surface.md)
- [docs/book/reference-pop3-surface.md](docs/book/reference-pop3-surface.md)

Cluster hint:

- key: `mail-delivery-mailbox`
- operator hint: Separate delivery, retrieval, and mailbox state early; the same account issue can present very differently across send and read surfaces.
- sibling protocols: `smtp`, `imap`, `pop3`

## Identity, Directory, And Access

Families:

- [docs/book/reference-ldap-surface.md](docs/book/reference-ldap-surface.md)
- [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
- [docs/book/reference-kerberos-surface.md](docs/book/reference-kerberos-surface.md)
- [docs/book/reference-radius-surface.md](docs/book/reference-radius-surface.md)
- [docs/book/reference-smb-surface.md](docs/book/reference-smb-surface.md)
- [docs/book/reference-rdp-surface.md](docs/book/reference-rdp-surface.md)

Cluster hint:

- key: `identity-directory-access`
- operator hint: Prioritize bind, credential, authorization, and access-gate stages; these protocols tend to fail with explicit denial semantics rather than silent payload drift.
- sibling protocols: `ldap`, `ssh`, `kerberos`, `radius`, `smb`, `rdp`

## Transport, Media, And Session Control

Families:

- [docs/book/reference-stun-surface.md](docs/book/reference-stun-surface.md)
- [docs/book/reference-coap-surface.md](docs/book/reference-coap-surface.md)
- [docs/book/reference-tftp-surface.md](docs/book/reference-tftp-surface.md)
- [docs/book/reference-dhcp-surface.md](docs/book/reference-dhcp-surface.md)
- [docs/book/reference-dhcpv6-surface.md](docs/book/reference-dhcpv6-surface.md)
- [docs/book/reference-arp-surface.md](docs/book/reference-arp-surface.md)
- [docs/book/reference-bgp-surface.md](docs/book/reference-bgp-surface.md)
- [docs/book/reference-icmp-surface.md](docs/book/reference-icmp-surface.md)
- [docs/book/reference-icmpv6-surface.md](docs/book/reference-icmpv6-surface.md)
- [docs/book/reference-ndp-surface.md](docs/book/reference-ndp-surface.md)
- [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md)
- [docs/book/reference-ospf-surface.md](docs/book/reference-ospf-surface.md)
- [docs/book/reference-rip-surface.md](docs/book/reference-rip-surface.md)
- [docs/book/reference-gre-surface.md](docs/book/reference-gre-surface.md)
- [docs/book/reference-vxlan-surface.md](docs/book/reference-vxlan-surface.md)
- [docs/book/reference-geneve-surface.md](docs/book/reference-geneve-surface.md)
- [docs/book/reference-l2tp-surface.md](docs/book/reference-l2tp-surface.md)
- [docs/book/reference-pptp-surface.md](docs/book/reference-pptp-surface.md)
- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)
- [docs/book/reference-mdns-surface.md](docs/book/reference-mdns-surface.md)
- [docs/book/reference-llmnr-surface.md](docs/book/reference-llmnr-surface.md)
- [docs/book/reference-nbns-surface.md](docs/book/reference-nbns-surface.md)
- [docs/book/reference-ssdp-surface.md](docs/book/reference-ssdp-surface.md)
- [docs/book/reference-gtpu-surface.md](docs/book/reference-gtpu-surface.md)
- [docs/book/reference-wireguard-surface.md](docs/book/reference-wireguard-surface.md)
- [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
- [docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md)
- [docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md)
- [docs/book/reference-ftp-surface.md](docs/book/reference-ftp-surface.md)

Cluster hint:

- key: `network-control-discovery`
- operator hint: Start with discovery scope, control role, and time or tunnel posture; many issues here are topology-sensitive rather than application-payload-specific.
- sibling protocols: `dns`, `mdns`, `llmnr`, `nbns`, `ssdp`, `stun`, `coap`, `tftp`, `ntp`, `dhcp`, `dhcpv6`, `arp`, `icmp`, `icmpv6`, `ndp`, `bgp`, `ospf`, `rip`, `gre`, `vxlan`, `geneve`, `l2tp`, `pptp`, `snmp`, `wireguard`, `gtpu`

<!-- gewyvern:protocol-groups:end -->

## Practical Lookup Order

If you are unsure where to begin, use this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
3. [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
4. one family hub page
5. one narrower family subpage
6. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Current Thesis

For the current line, protocol reference should be easy to approach in three
steps:

- broad group
- exact family
- exact sub-surface

That is the minimum structure needed for the protocol reference volume to stay
usable as coverage keeps expanding.
