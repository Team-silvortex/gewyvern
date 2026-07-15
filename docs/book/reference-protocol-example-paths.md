# Reference: Protocol Example Paths

Use this page when you already know the protocol family and want the closest
real sample in the repository.

This page exists to connect the reference shelf to actual `.gewy` examples and
the one concrete architecture walkthrough we currently maintain.

Read this alongside:

- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)
- [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
- [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)

## How To Use This Page

Use this page when the question is:

- “show me the closest real `.gewy` sample for this family”
- “which sample should I open before adding one more package?”
- “which DSL file best matches the family hub I am reading?”

The normal lookup path is:

1. family hub page
2. one nearby `.gewy` example here
3. one validation or explanation page if needed

## High-Frequency Families

### HTTP

- Hub:
  [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
- DSL examples:
  - [dsl/http_request_path.gewy](dsl/http_request_path.gewy)
  - [dsl/http_server_response_path.gewy](dsl/http_server_response_path.gewy)
  - [dsl/http_connect_tunnel_path.gewy](dsl/http_connect_tunnel_path.gewy)
- Walkthrough:
  - [docs/architecture-walkthrough-http-request.md](docs/architecture-walkthrough-http-request.md)

### HTTPS

- Hub:
  [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
- DSL example:
  - [dsl/https_connect_process.gewy](dsl/https_connect_process.gewy)

### TLS

- Hub:
  [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
- DSL example:
  - [dsl/tls_client_path.gewy](dsl/tls_client_path.gewy)

### DNS

- Hub:
  [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
- DSL examples:
  - [dsl/dns_udp_process.gewy](dsl/dns_udp_process.gewy)
  - [dsl/dns_tcp_query_path.gewy](dsl/dns_tcp_query_path.gewy)

### SSH

- Hub:
  [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
- DSL examples:
  - [dsl/ssh_session_path.gewy](dsl/ssh_session_path.gewy)
  - [dsl/ssh_auth_path.gewy](dsl/ssh_auth_path.gewy)
  - [dsl/ssh_channel_session_path.gewy](dsl/ssh_channel_session_path.gewy)

### SOCKS5

- Hub:
  [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)
- DSL examples:
  - [dsl/socks5_session_path.gewy](dsl/socks5_session_path.gewy)
  - [dsl/socks5_auth_path.gewy](dsl/socks5_auth_path.gewy)
  - [dsl/socks5_denied_path.gewy](dsl/socks5_denied_path.gewy)

### PostgreSQL

- Hub:
  [docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md)
- DSL examples:
  - [dsl/postgres_simple_query_path.gewy](dsl/postgres_simple_query_path.gewy)
  - [dsl/postgres_query_session.gewy](dsl/postgres_query_session.gewy)
  - [dsl/postgres_query_error_path.gewy](dsl/postgres_query_error_path.gewy)

### MySQL

- Hub:
  [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
- DSL examples:
  - [dsl/mysql_simple_query_path.gewy](dsl/mysql_simple_query_path.gewy)
  - [dsl/mysql_query_session.gewy](dsl/mysql_query_session.gewy)
  - [dsl/mysql_query_error_path.gewy](dsl/mysql_query_error_path.gewy)

### QUIC

- Hub:
  [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)
- DSL examples:
  - [dsl/quic_client_initial_path.gewy](dsl/quic_client_initial_path.gewy)
  - [dsl/quic_crypto_handshake_path.gewy](dsl/quic_crypto_handshake_path.gewy)
  - [dsl/quic_bidi_stream_path.gewy](dsl/quic_bidi_stream_path.gewy)

### HTTP/3

- Hub:
  [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
- DSL examples:
  - [dsl/http3_request_path.gewy](dsl/http3_request_path.gewy)
  - [dsl/http3_server_response_path.gewy](dsl/http3_server_response_path.gewy)

## Broader Protocol Shelf Samples

### Control Plane And Datagram

- [dsl/stun_binding_path.gewy](dsl/stun_binding_path.gewy)
- [dsl/coap_get_path.gewy](dsl/coap_get_path.gewy)
- [dsl/dhcp_client_path.gewy](dsl/dhcp_client_path.gewy)
- [dsl/ntp_client_path.gewy](dsl/ntp_client_path.gewy)
- [dsl/snmp_get_path.gewy](dsl/snmp_get_path.gewy)

### Identity And Access

- [dsl/kerberos_as_path.gewy](dsl/kerberos_as_path.gewy)
- [dsl/ldap_bind_path.gewy](dsl/ldap_bind_path.gewy)
- [dsl/radius_access_path.gewy](dsl/radius_access_path.gewy)

### Cache, Queue, And Messaging

- [dsl/redis_ping_path.gewy](dsl/redis_ping_path.gewy)
- [dsl/memcached_get_path.gewy](dsl/memcached_get_path.gewy)
- [dsl/mqtt_connect_path.gewy](dsl/mqtt_connect_path.gewy)
- [dsl/amqp_connection_start_path.gewy](dsl/amqp_connection_start_path.gewy)

## Shortest Practical Routes

- Contract -> sample:
  family hub -> one DSL example above
- Contract -> sample -> validation:
  family hub -> one DSL example above -> [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- Contract -> sample -> system explanation:
  family hub -> one DSL example above -> [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
